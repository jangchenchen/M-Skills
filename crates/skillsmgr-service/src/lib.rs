use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;
use skillsmgr_adapters::{claude_code, codex, gemini, hermes::HermesAdapter, opencode};
use skillsmgr_core::{
    AdapterPresence, Artifact, ArtifactKind, Capability, Installation, Result, ScannedInstallation,
    Scope, SkillsMgrError, Source, SourceProvenance, Target, ToolAdapter,
};
use skillsmgr_fetch::{
    preview_github_import, preview_local_import, preview_raw_url_import, ImportCandidate,
    ImportPreview,
};
use skillsmgr_registry::{RecordEventInput, Registry, RegistryEvent};
use skillsmgr_scan::{default_scopes, discover_project_root, scan_all, ScanError, ScanResult};

#[derive(Debug, Clone)]
pub struct AdapterStatus {
    pub adapter_id: String,
    pub presence: AdapterPresence,
    pub supported_kinds: Vec<ArtifactKind>,
    pub writable: bool,
    pub supports_disable: bool,
}

#[derive(Debug, Clone)]
pub struct ArtifactGroup {
    pub name: String,
    pub kind: ArtifactKind,
    pub description: String,
    pub body: Option<String>,
    pub version: Option<String>,
    pub search_aliases: Vec<String>,
    pub capabilities: Vec<Capability>,
    pub installations: Vec<ScannedInstallation>,
    pub also_visible_to: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Inventory {
    pub groups: Vec<ArtifactGroup>,
    pub adapters: Vec<AdapterStatus>,
    pub errors: Vec<ScanError>,
}

pub struct Service {
    adapters: Vec<Arc<dyn ToolAdapter>>,
    registry: Option<tokio::sync::Mutex<Registry>>,
    home: Option<PathBuf>,
}

impl Service {
    pub fn with_adapters(adapters: Vec<Arc<dyn ToolAdapter>>) -> Self {
        Self {
            adapters,
            registry: None,
            home: None,
        }
    }

    pub fn with_adapters_and_registry(
        adapters: Vec<Arc<dyn ToolAdapter>>,
        registry: Registry,
    ) -> Self {
        Self {
            adapters,
            registry: Some(tokio::sync::Mutex::new(registry)),
            home: None,
        }
    }

    pub fn with_home(home: impl Into<PathBuf>) -> Self {
        let home = home.into();
        let adapters: Vec<Arc<dyn ToolAdapter>> = vec![
            Arc::new(claude_code::adapter(&home)),
            Arc::new(codex::adapter(&home)),
            Arc::new(opencode::adapter(&home)),
            Arc::new(gemini::adapter(&home)),
            Arc::new(HermesAdapter::from_home(&home)),
        ];
        Self {
            adapters,
            registry: None,
            home: Some(home),
        }
    }

    pub fn with_home_and_registry(home: impl Into<PathBuf>, registry: Registry) -> Self {
        let mut service = Self::with_home(home);
        service.registry = Some(tokio::sync::Mutex::new(registry));
        service
    }

    pub async fn inventory(&self, cwd: Option<&Path>) -> Inventory {
        let project_root = cwd.and_then(discover_project_root);
        self.inventory_for_scopes(default_scopes(project_root))
            .await
    }

    pub async fn inventory_for_scopes(&self, scopes: Vec<Scope>) -> Inventory {
        let results = scan_all(self.adapters.clone(), scopes).await;
        let mut inventory = build_inventory(results);
        if let Some(home) = &self.home {
            apply_claude_plugin_aliases(&mut inventory, home);
        }
        inventory
    }

    pub async fn preview_local_import(
        &self,
        path: impl AsRef<Path>,
        scopes: Vec<Scope>,
    ) -> Result<ImportPreview> {
        preview_local_import(path, &self.available_targets(scopes).await).await
    }

    pub async fn preview_github_import(
        &self,
        url: impl Into<String>,
        scopes: Vec<Scope>,
    ) -> Result<ImportPreview> {
        preview_github_import(url, &self.available_targets(scopes).await).await
    }

    pub async fn preview_raw_url_import(
        &self,
        url: impl Into<String>,
        scopes: Vec<Scope>,
    ) -> Result<ImportPreview> {
        preview_raw_url_import(url, &self.available_targets(scopes).await).await
    }

    pub async fn install(
        &self,
        artifact: &Artifact,
        target: Target,
        scopes_to_scan: Vec<Scope>,
    ) -> Result<Installation> {
        let scope = target_scope(&target)?;
        let adapter = self.adapter_for_target(&target)?;

        self.ensure_no_source_conflict(artifact, scopes_to_scan)
            .await?;
        let installation = adapter.install(artifact, scope).await?;
        if let Some(registry) = &self.registry {
            let mut reg = registry.lock().await;
            reg.record_installation(artifact, &installation)?;
            let _ = reg.record_event(RecordEventInput {
                event_type: "install".to_string(),
                artifact_name: Some(artifact.name.clone()),
                target: Some(installation.target.tool_id().to_string()),
                succeeded: true,
                error_message: None,
            });
        }
        Ok(installation)
    }

    /// Install from a staged import candidate. Uses `staged_root` for the file
    /// copy regardless of origin (GitHub or local), then records the original
    /// `artifact.source` in the registry.
    pub async fn install_from_candidate(
        &self,
        candidate: &ImportCandidate,
        target: Target,
        scopes: Vec<Scope>,
    ) -> Result<Installation> {
        let scope = target_scope(&target)?;
        let adapter = self.adapter_for_target(&target)?;

        let mut copy_artifact = candidate.artifact.clone();
        copy_artifact.source = Source::Local {
            path: candidate.staged_root.clone(),
        };

        self.ensure_no_source_conflict(&copy_artifact, scopes)
            .await?;
        let installation = adapter.install(&copy_artifact, scope).await?;

        if let Some(registry) = &self.registry {
            let mut reg = registry.lock().await;
            reg.record_installation(&candidate.artifact, &installation)?;
            let _ = reg.record_event(RecordEventInput {
                event_type: "install".to_string(),
                artifact_name: Some(candidate.artifact.name.clone()),
                target: Some(installation.target.tool_id().to_string()),
                succeeded: true,
                error_message: None,
            });
        }
        Ok(installation)
    }

    pub async fn uninstall(&self, installation: &Installation) -> Result<()> {
        let adapter = self.adapter_for_target(&installation.target)?;
        adapter.uninstall(installation).await?;
        if let Some(registry) = &self.registry {
            let mut reg = registry.lock().await;
            let artifact_name = reg.artifact_name_by_id(installation.artifact_id);
            reg.record_uninstall(installation.id)?;
            let _ = reg.record_event(RecordEventInput {
                event_type: "uninstall".to_string(),
                artifact_name,
                target: Some(installation.target.tool_id().to_string()),
                succeeded: true,
                error_message: None,
            });
        }
        Ok(())
    }

    pub async fn enable(&self, installation: &Installation) -> Result<()> {
        let adapter = self.adapter_for_target(&installation.target)?;
        adapter.enable(installation).await
    }

    pub async fn disable(&self, installation: &Installation) -> Result<()> {
        let adapter = self.adapter_for_target(&installation.target)?;
        adapter.disable(installation).await
    }

    /// Resolve the on-disk path an install with the given name and target would
    /// write to, without performing the install. Returns `None` when the
    /// adapter has no deterministic per-name path (read-only or non-directory
    /// adapters).
    pub fn install_path_for(&self, target: &Target, name: &str) -> Option<PathBuf> {
        let adapter = self.adapter_for_target(target).ok()?;
        let scope = target.scope()?;
        adapter.install_path_for(scope, name)
    }

    pub async fn rebuild_registry_for_scopes(&self, scopes: Vec<Scope>) -> Result<()> {
        let Some(registry) = &self.registry else {
            return Ok(());
        };
        let results = scan_all(self.adapters.clone(), scopes).await;
        let scanned = results
            .into_iter()
            .flat_map(|result| result.items)
            .collect::<Vec<_>>();
        registry.lock().await.upsert_scan_results(&scanned)
    }

    pub async fn record_event(&self, input: RecordEventInput) {
        if let Some(registry) = &self.registry {
            let _ = registry.lock().await.record_event(input);
        }
    }

    pub async fn recent_events(&self, limit: usize) -> Vec<RegistryEvent> {
        if let Some(registry) = &self.registry {
            return registry
                .lock()
                .await
                .recent_events(limit)
                .unwrap_or_default();
        }
        vec![]
    }

    pub async fn stale_installation_count(&self) -> usize {
        if let Some(registry) = &self.registry {
            return registry
                .lock()
                .await
                .stale_installation_count()
                .unwrap_or(0);
        }
        0
    }

    /// Targets that this Service can install into right now. An adapter is
    /// excluded entirely when `detect()` reports `Missing` — we don't want the
    /// import wizard to offer Claude Code as an install target on a machine
    /// where `~/.claude` doesn't exist.
    pub async fn available_targets(&self, scopes: Vec<Scope>) -> Vec<Target> {
        let mut targets = Vec::new();
        for adapter in &self.adapters {
            if !matches!(adapter.detect().await, AdapterPresence::Available) {
                continue;
            }
            for scope in scopes.clone() {
                for kind in adapter.supported_kinds() {
                    if let Some(target) = target_for_adapter(adapter.id(), scope.clone(), *kind) {
                        push_unique_target(&mut targets, target);
                    }
                }
            }
        }
        targets
    }

    fn adapter_for_target(&self, target: &Target) -> Result<Arc<dyn ToolAdapter>> {
        let tool_id = target.tool_id();
        self.adapters
            .iter()
            .find(|adapter| adapter.id() == tool_id)
            .cloned()
            .ok_or_else(|| SkillsMgrError::UnsupportedTarget {
                adapter_id: tool_id.to_string(),
                target: target.clone(),
            })
    }

    async fn ensure_no_source_conflict(
        &self,
        artifact: &Artifact,
        scopes: Vec<Scope>,
    ) -> Result<()> {
        let inventory = self.inventory_for_scopes(scopes).await;
        for group in inventory.groups {
            if group.name != artifact.name || group.kind != artifact.kind {
                continue;
            }

            for item in group.installations {
                if item.artifact.source != Source::Unknown
                    && artifact.source != Source::Unknown
                    && item.artifact.source != artifact.source
                {
                    return Err(SkillsMgrError::SourceConflict {
                        name: artifact.name.clone(),
                        existing_source: item.artifact.source,
                        new_source: artifact.source.clone(),
                    });
                }
            }
        }

        Ok(())
    }
}

fn target_scope(target: &Target) -> Result<Scope> {
    target
        .scope()
        .cloned()
        .ok_or_else(|| SkillsMgrError::UnsupportedTarget {
            adapter_id: target.tool_id().to_string(),
            target: target.clone(),
        })
}

fn target_for_adapter(adapter_id: &str, scope: Scope, kind: ArtifactKind) -> Option<Target> {
    let target = match adapter_id {
        "claude-code" => Target::ClaudeCode { scope },
        "codex" => Target::Codex { scope },
        "opencode" => Target::Opencode { scope },
        "gemini" => Target::Gemini { scope },
        _ => return None,
    };
    target.supports_kind(kind).then_some(target)
}

fn push_unique_target(targets: &mut Vec<Target>, target: Target) {
    if !targets.contains(&target) {
        targets.push(target);
    }
}

fn build_inventory(results: Vec<ScanResult>) -> Inventory {
    let mut groups: Vec<ArtifactGroup> = Vec::new();
    let mut adapters = Vec::with_capacity(results.len());
    let mut errors = Vec::new();

    for result in results {
        adapters.push(AdapterStatus {
            adapter_id: result.adapter_id,
            presence: result.presence,
            supported_kinds: result.supported_kinds.to_vec(),
            writable: result.writable,
            supports_disable: result.supports_disable,
        });
        errors.extend(result.errors);

        for item in result.items {
            insert_into_group(&mut groups, item);
        }
    }

    groups.sort_by(|a, b| (a.kind, &a.name).cmp(&(b.kind, &b.name)));
    Inventory {
        groups,
        adapters,
        errors,
    }
}

fn insert_into_group(groups: &mut Vec<ArtifactGroup>, item: ScannedInstallation) {
    if let Some(group) = groups
        .iter_mut()
        .find(|group| group.kind == item.artifact.kind && group.name == item.artifact.name)
    {
        merge_metadata(group, &item.artifact);
        if merge_shared_visibility(group, &item) {
            return;
        }
        group.installations.push(item);
        return;
    }

    let also_visible_to = shared_visibility(&item).into_iter().collect();
    let search_aliases = item.artifact.search_aliases.clone();
    groups.push(ArtifactGroup {
        name: item.artifact.name.clone(),
        kind: item.artifact.kind,
        description: item.artifact.description.clone(),
        body: item.artifact.body.clone(),
        version: item.artifact.version.clone(),
        search_aliases,
        capabilities: item.artifact.capabilities.clone(),
        installations: vec![item],
        also_visible_to,
    });
}

fn merge_shared_visibility(group: &mut ArtifactGroup, item: &ScannedInstallation) -> bool {
    let same_path = group
        .installations
        .iter()
        .position(|existing| existing.installation.on_disk_path == item.installation.on_disk_path);

    let Some(index) = same_path else {
        return false;
    };

    if let Some(tool) = shared_visibility(item) {
        push_unique(&mut group.also_visible_to, tool);
    }

    if item.provenance == SourceProvenance::Owned
        && group.installations[index].provenance != SourceProvenance::Owned
    {
        group.installations[index] = item.clone();
    }

    true
}

fn shared_visibility(item: &ScannedInstallation) -> Option<String> {
    match &item.provenance {
        SourceProvenance::Owned => None,
        SourceProvenance::Shared { .. } => Some(item.installation.target.tool_id().to_string()),
    }
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
        values.sort();
    }
}

fn merge_metadata(group: &mut ArtifactGroup, artifact: &Artifact) {
    if group.description.is_empty() && !artifact.description.is_empty() {
        group.description = artifact.description.clone();
    }
    if group.body.is_none() && artifact.body.is_some() {
        group.body.clone_from(&artifact.body);
    }
    if group.version.is_none() && artifact.version.is_some() {
        group.version.clone_from(&artifact.version);
    }
    if group.capabilities.is_empty() && !artifact.capabilities.is_empty() {
        group.capabilities = artifact.capabilities.clone();
    }
    push_unique_aliases(
        &mut group.search_aliases,
        artifact.search_aliases.iter().cloned(),
    );
}

fn apply_claude_plugin_aliases(inv: &mut Inventory, home: &Path) {
    let aliases_by_skill_name = claude_plugin_skill_aliases(home);
    if aliases_by_skill_name.is_empty() {
        return;
    }

    for group in &mut inv.groups {
        if group.kind != ArtifactKind::Skill {
            continue;
        }
        let Some(aliases) = aliases_by_skill_name.get(&group.name) else {
            continue;
        };
        push_unique_aliases(&mut group.search_aliases, aliases.iter().cloned());
        for installation in &mut group.installations {
            if installation.artifact.kind == ArtifactKind::Skill
                && installation.artifact.name == group.name
            {
                push_unique_aliases(
                    &mut installation.artifact.search_aliases,
                    aliases.iter().cloned(),
                );
            }
        }
    }
}

fn claude_plugin_skill_aliases(home: &Path) -> HashMap<String, Vec<String>> {
    let mut aliases_by_skill_name = HashMap::new();
    let installed = read_installed_claude_plugins(home);
    for plugin in installed {
        for skill in read_claude_plugin_skills(&plugin.install_path) {
            let mut aliases = vec![
                plugin.package_id.clone(),
                plugin.install_path.to_string_lossy().to_string(),
            ];
            aliases.extend(plugin.package_id.split('@').map(str::to_string));
            aliases.extend(plugin.version.iter().cloned());
            aliases.extend(plugin.git_commit_sha.iter().cloned());
            aliases.extend(skill.aliases);
            push_unique_aliases(
                aliases_by_skill_name
                    .entry(skill.name)
                    .or_insert_with(Vec::new),
                aliases,
            );
        }
    }
    aliases_by_skill_name
}

#[derive(Debug, Clone)]
struct InstalledClaudePlugin {
    package_id: String,
    install_path: PathBuf,
    version: Option<String>,
    git_commit_sha: Option<String>,
}

#[derive(Debug)]
struct ClaudePluginSkill {
    name: String,
    aliases: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct InstalledClaudePluginsFile {
    #[serde(default)]
    plugins: HashMap<String, Vec<InstalledClaudePluginRecord>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstalledClaudePluginRecord {
    install_path: PathBuf,
    version: Option<String>,
    git_commit_sha: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ClaudePluginManifest {
    name: Option<String>,
    description: Option<String>,
    version: Option<String>,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    skills: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ClaudeMarketplaceManifest {
    name: Option<String>,
    id: Option<String>,
    #[serde(default)]
    plugins: Vec<ClaudeMarketplacePlugin>,
}

#[derive(Debug, Deserialize)]
struct ClaudeMarketplacePlugin {
    name: Option<String>,
    description: Option<String>,
    version: Option<String>,
    #[serde(default)]
    keywords: Vec<String>,
    category: Option<String>,
}

fn read_installed_claude_plugins(home: &Path) -> Vec<InstalledClaudePlugin> {
    let path = home.join(".claude/plugins/installed_plugins.json");
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Ok(parsed) = serde_json::from_str::<InstalledClaudePluginsFile>(&content) else {
        return Vec::new();
    };

    let mut plugins = Vec::new();
    for (package_id, records) in parsed.plugins {
        for record in records {
            plugins.push(InstalledClaudePlugin {
                package_id: package_id.clone(),
                install_path: record.install_path,
                version: record.version,
                git_commit_sha: record.git_commit_sha,
            });
        }
    }
    plugins
}

fn read_claude_plugin_skills(install_path: &Path) -> Vec<ClaudePluginSkill> {
    let plugin_manifest =
        read_json::<ClaudePluginManifest>(&install_path.join(".claude-plugin/plugin.json"));
    let marketplace_manifest = read_json::<ClaudeMarketplaceManifest>(
        &install_path.join(".claude-plugin/marketplace.json"),
    );
    let skills = plugin_manifest
        .as_ref()
        .map(|manifest| manifest.skills.clone())
        .unwrap_or_default();

    skills
        .into_iter()
        .filter_map(|skill_path| {
            let skill_name = skill_name_from_plugin_path(&skill_path)?;
            let mut aliases = vec![skill_path];
            if let Some(manifest) = &plugin_manifest {
                aliases.extend(manifest.name.iter().cloned());
                aliases.extend(manifest.description.iter().cloned());
                aliases.extend(manifest.version.iter().cloned());
                aliases.extend(manifest.keywords.iter().cloned());
            }
            if let Some(marketplace) = &marketplace_manifest {
                aliases.extend(marketplace.name.iter().cloned());
                aliases.extend(marketplace.id.iter().cloned());
                for plugin in &marketplace.plugins {
                    aliases.extend(plugin.name.iter().cloned());
                    aliases.extend(plugin.description.iter().cloned());
                    aliases.extend(plugin.version.iter().cloned());
                    aliases.extend(plugin.keywords.iter().cloned());
                    aliases.extend(plugin.category.iter().cloned());
                }
            }
            Some(ClaudePluginSkill {
                name: skill_name,
                aliases,
            })
        })
        .collect()
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn skill_name_from_plugin_path(path: &str) -> Option<String> {
    let trimmed = path.trim().trim_end_matches('/');
    let name = Path::new(trimmed)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())?;
    Some(name.to_string())
}

fn push_unique_aliases(values: &mut Vec<String>, aliases: impl IntoIterator<Item = String>) {
    for alias in aliases {
        let alias = alias.trim();
        if alias.is_empty() {
            continue;
        }
        if !values.iter().any(|existing| existing == alias) {
            values.push(alias.to_string());
        }
    }
    values.sort();
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::sync::Arc;

    use tempfile::tempdir;

    use super::*;

    fn write_skill(root: &Path, name: &str, description: &str, version: Option<&str>) {
        let dir = root.join(name);
        fs::create_dir_all(&dir).unwrap();
        let version_line = version
            .map(|v| format!("version: {v}\n"))
            .unwrap_or_default();
        fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: {description}\n{version_line}---\n# {name}\n"),
        )
        .unwrap();
    }

    #[tokio::test]
    async fn inventory_groups_same_named_skill_across_tools() {
        let home = tempdir().unwrap();
        write_skill(
            &home.path().join(".claude/skills"),
            "polish-code",
            "Polish code",
            Some("1.0.0"),
        );
        write_skill(
            &home.path().join(".config/opencode/skills"),
            "polish-code",
            "",
            None,
        );
        write_skill(
            &home.path().join(".agents/skills"),
            "review-pr",
            "Review PR",
            None,
        );

        let service = Service::with_home(home.path());
        let inventory = service.inventory_for_scopes(vec![Scope::Global]).await;

        let polish = inventory
            .groups
            .iter()
            .find(|g| g.name == "polish-code")
            .unwrap();
        assert_eq!(polish.installations.len(), 2);
        assert_eq!(polish.also_visible_to, vec!["opencode"]);
        assert_eq!(polish.description, "Polish code");
        assert_eq!(polish.body.as_deref(), Some("# polish-code"));
        assert_eq!(polish.version.as_deref(), Some("1.0.0"));

        let review = inventory
            .groups
            .iter()
            .find(|g| g.name == "review-pr")
            .unwrap();
        assert_eq!(review.installations.len(), 1);
        assert_eq!(review.also_visible_to, vec!["opencode"]);
    }

    #[tokio::test]
    async fn inventory_keeps_owned_installation_when_shared_scan_hits_same_path() {
        let home = tempdir().unwrap();
        write_skill(
            &home.path().join(".claude/skills"),
            "polish-code",
            "Polish code",
            Some("1.0.0"),
        );

        let service = Service::with_home(home.path());
        let inventory = service.inventory_for_scopes(vec![Scope::Global]).await;

        let polish = inventory
            .groups
            .iter()
            .find(|g| g.name == "polish-code")
            .unwrap();

        assert_eq!(polish.installations.len(), 1);
        assert_eq!(
            polish.installations[0].installation.target.tool_id(),
            "claude-code"
        );
        assert_eq!(polish.also_visible_to, vec!["opencode"]);
    }

    #[tokio::test]
    async fn inventory_indexes_claude_plugin_package_aliases_for_skills() {
        let home = tempdir().unwrap();
        write_skill(
            &home.path().join(".claude/skills"),
            "karpathy-guidelines",
            "Behavioral coding guidelines",
            None,
        );

        let plugin_root = home
            .path()
            .join(".claude/plugins/cache/karpathy-skills/andrej-karpathy-skills/1.0.0");
        fs::create_dir_all(plugin_root.join(".claude-plugin")).unwrap();
        fs::write(
            plugin_root.join(".claude-plugin/plugin.json"),
            serde_json::json!({
                "name": "andrej-karpathy-skills",
                "description": "Behavioral guidelines to reduce common LLM coding mistakes",
                "version": "1.0.0",
                "keywords": ["guidelines", "karpathy"],
                "skills": ["./skills/karpathy-guidelines"]
            })
            .to_string(),
        )
        .unwrap();
        fs::write(
            plugin_root.join(".claude-plugin/marketplace.json"),
            serde_json::json!({
                "name": "karpathy-skills",
                "id": "karpathy-skills",
                "plugins": [
                    {
                        "name": "andrej-karpathy-skills",
                        "description": "Think Before Coding, Simplicity First",
                        "version": "1.0.0",
                        "keywords": ["best-practices", "coding"],
                        "category": "workflow"
                    }
                ]
            })
            .to_string(),
        )
        .unwrap();
        fs::create_dir_all(home.path().join(".claude/plugins")).unwrap();
        fs::write(
            home.path().join(".claude/plugins/installed_plugins.json"),
            serde_json::json!({
                "version": 2,
                "plugins": {
                    "andrej-karpathy-skills@karpathy-skills": [
                        {
                            "scope": "user",
                            "installPath": plugin_root,
                            "version": "1.0.0",
                            "gitCommitSha": "2c606141936f1eeef17fa3043a72095b4765b9c2"
                        }
                    ]
                }
            })
            .to_string(),
        )
        .unwrap();

        let service = Service::with_home(home.path());
        let inventory = service.inventory_for_scopes(vec![Scope::Global]).await;

        let group = inventory
            .groups
            .iter()
            .find(|g| g.name == "karpathy-guidelines")
            .unwrap();

        assert!(group
            .search_aliases
            .contains(&"andrej-karpathy-skills@karpathy-skills".to_string()));
        assert!(group
            .search_aliases
            .contains(&"andrej-karpathy-skills".to_string()));
        assert!(group
            .search_aliases
            .contains(&"karpathy-skills".to_string()));
        assert!(group.installations.iter().any(|item| item
            .artifact
            .search_aliases
            .contains(&"andrej-karpathy-skills@karpathy-skills".to_string())));
    }

    #[tokio::test]
    async fn inventory_reports_adapter_presence_for_each_tool() {
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude/skills")).unwrap();

        let service = Service::with_home(home.path());
        let inventory = service.inventory_for_scopes(vec![Scope::Global]).await;

        let claude = inventory
            .adapters
            .iter()
            .find(|a| a.adapter_id == "claude-code")
            .unwrap();
        assert!(matches!(claude.presence, AdapterPresence::Available));

        let hermes = inventory
            .adapters
            .iter()
            .find(|a| a.adapter_id == "hermes")
            .unwrap();
        assert!(matches!(hermes.presence, AdapterPresence::Missing { .. }));
    }

    #[tokio::test]
    async fn available_targets_excludes_missing_adapters() {
        let home = tempdir().unwrap();
        // Claude Code present; Hermes deliberately absent (no ~/.hermes).
        fs::create_dir_all(home.path().join(".claude/skills")).unwrap();

        let service = Service::with_home(home.path());
        let targets = service.available_targets(vec![Scope::Global]).await;

        let tools: Vec<&str> = targets.iter().map(Target::tool_id).collect();
        assert!(tools.contains(&"claude-code"));
        assert!(!tools.contains(&"hermes"));
    }

    #[tokio::test]
    async fn preview_import_filters_targets_from_service_adapters() {
        let home = tempdir().unwrap();
        // available_targets now filters by adapter presence, so the Claude Code
        // root must exist for it to be offered.
        fs::create_dir_all(home.path().join(".claude/skills")).unwrap();
        let source = tempdir().unwrap();
        fs::write(source.path().join("SKILL.md"), "# Demo\n").unwrap();
        let service = Service::with_adapters(vec![
            Arc::new(claude_code::adapter(home.path())),
            Arc::new(gemini::adapter(home.path())),
        ]);

        let preview = service
            .preview_local_import(source.path(), vec![Scope::Global])
            .await
            .unwrap();

        assert_eq!(preview.candidates.len(), 1);
        assert_eq!(
            preview.candidates[0].compatible_targets,
            vec![Target::ClaudeCode {
                scope: Scope::Global
            }]
        );
    }

    #[tokio::test]
    async fn install_routes_to_selected_adapter() {
        let home = tempdir().unwrap();
        let source = tempdir().unwrap();
        fs::write(source.path().join("SKILL.md"), "# Demo\n").unwrap();
        let service = Service::with_adapters(vec![Arc::new(claude_code::adapter(home.path()))]);
        let artifact = Artifact::new(
            "demo",
            "",
            None,
            ArtifactKind::Skill,
            skillsmgr_core::Source::Local {
                path: source.path().to_path_buf(),
            },
        );

        let installation = service
            .install(
                &artifact,
                Target::ClaudeCode {
                    scope: Scope::Global,
                },
                vec![Scope::Global],
            )
            .await
            .unwrap();

        assert!(installation.on_disk_path.join("SKILL.md").exists());
        assert_eq!(
            installation.target,
            Target::ClaudeCode {
                scope: Scope::Global
            }
        );
    }

    #[tokio::test]
    async fn install_refuses_same_name_from_different_source() {
        let home = tempdir().unwrap();
        write_skill(
            &home.path().join(".claude/skills"),
            "demo",
            "Existing",
            None,
        );
        let new_source = tempdir().unwrap();
        fs::write(new_source.path().join("SKILL.md"), "# Demo\n").unwrap();
        let service = Service::with_adapters(vec![Arc::new(claude_code::adapter(home.path()))]);
        let artifact = Artifact::new(
            "demo",
            "",
            None,
            ArtifactKind::Skill,
            skillsmgr_core::Source::Local {
                path: new_source.path().to_path_buf(),
            },
        );

        let error = service
            .install(
                &artifact,
                Target::ClaudeCode {
                    scope: Scope::Global,
                },
                vec![Scope::Global],
            )
            .await
            .unwrap_err();

        assert!(matches!(error, SkillsMgrError::SourceConflict { .. }));
    }

    #[tokio::test]
    async fn uninstall_routes_to_selected_adapter() {
        let home = tempdir().unwrap();
        let source = tempdir().unwrap();
        fs::write(source.path().join("SKILL.md"), "# Demo\n").unwrap();
        let service = Service::with_adapters(vec![Arc::new(gemini::adapter(home.path()))]);
        let artifact = Artifact::new(
            "demo",
            "",
            None,
            ArtifactKind::Extension,
            skillsmgr_core::Source::Local {
                path: source.path().to_path_buf(),
            },
        );
        fs::write(
            source.path().join("gemini-extension.json"),
            r#"{"name":"demo"}"#,
        )
        .unwrap();
        let installation = service
            .install(
                &artifact,
                Target::Gemini {
                    scope: Scope::Global,
                },
                vec![Scope::Global],
            )
            .await
            .unwrap();

        service.uninstall(&installation).await.unwrap();

        assert!(!installation.on_disk_path.exists());
    }

    #[tokio::test]
    async fn mvp_adapters_install_and_uninstall() {
        let home = tempdir().unwrap();
        let adapters: Vec<Arc<dyn ToolAdapter>> = vec![
            Arc::new(claude_code::adapter(home.path())),
            Arc::new(codex::adapter(home.path())),
            Arc::new(opencode::adapter(home.path())),
            Arc::new(gemini::adapter(home.path())),
        ];
        let cases = vec![
            (
                ArtifactKind::Skill,
                Target::ClaudeCode {
                    scope: Scope::Global,
                },
                "claude-demo",
            ),
            (
                ArtifactKind::Skill,
                Target::Codex {
                    scope: Scope::Global,
                },
                "codex-demo",
            ),
            (
                ArtifactKind::Skill,
                Target::Opencode {
                    scope: Scope::Global,
                },
                "opencode-demo",
            ),
            (
                ArtifactKind::Extension,
                Target::Gemini {
                    scope: Scope::Global,
                },
                "gemini-demo",
            ),
        ];
        let service = Service::with_adapters(adapters);

        for (kind, target, name) in cases {
            let source = tempdir().unwrap();
            match kind {
                ArtifactKind::Skill => {
                    fs::write(source.path().join("SKILL.md"), "# Demo\n").unwrap();
                }
                ArtifactKind::Extension => {
                    fs::write(
                        source.path().join("gemini-extension.json"),
                        format!(r#"{{"name":"{name}"}}"#),
                    )
                    .unwrap();
                }
                ArtifactKind::Workflow => unreachable!(),
            }
            let artifact = Artifact::new(
                name,
                "",
                None,
                kind,
                skillsmgr_core::Source::Local {
                    path: source.path().to_path_buf(),
                },
            );

            let installation = service
                .install(&artifact, target, vec![Scope::Global])
                .await
                .unwrap();
            assert!(installation.on_disk_path.exists());

            service.uninstall(&installation).await.unwrap();
            assert!(!installation.on_disk_path.exists());
        }
    }

    #[tokio::test]
    async fn registry_rebuilds_from_service_scan() {
        let home = tempdir().unwrap();
        write_skill(
            &home.path().join(".claude/skills"),
            "demo",
            "Demo",
            Some("1.0.0"),
        );
        let registry = skillsmgr_registry::Registry::in_memory().unwrap();
        let service = Service::with_adapters_and_registry(
            vec![Arc::new(claude_code::adapter(home.path()))],
            registry,
        );

        service
            .rebuild_registry_for_scopes(vec![Scope::Global])
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn inventory_adapter_status_carries_writable_and_kinds() {
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".claude/skills")).unwrap();

        let service = Service::with_home(home.path());
        let inventory = service.inventory_for_scopes(vec![Scope::Global]).await;

        let claude = inventory
            .adapters
            .iter()
            .find(|a| a.adapter_id == "claude-code")
            .unwrap();
        assert!(claude.writable);
        assert!(claude.supported_kinds.contains(&ArtifactKind::Skill));

        let hermes = inventory
            .adapters
            .iter()
            .find(|a| a.adapter_id == "hermes")
            .unwrap();
        assert!(!hermes.writable);
    }

    #[tokio::test]
    async fn build_inventory_produces_correct_group_counts() {
        let home = tempdir().unwrap();
        let claude_root = home.path().join(".claude/skills");
        write_skill(&claude_root, "alpha", "Alpha skill", None);
        write_skill(&claude_root, "beta", "Beta skill", None);

        let service = Service::with_adapters(vec![Arc::new(claude_code::adapter(home.path()))]);
        let inventory = service.inventory_for_scopes(vec![Scope::Global]).await;

        assert_eq!(inventory.groups.len(), 2);
        let owned_count: usize = inventory
            .groups
            .iter()
            .flat_map(|g| &g.installations)
            .filter(|si| matches!(si.provenance, SourceProvenance::Owned))
            .count();
        assert_eq!(owned_count, 2);
    }
}
