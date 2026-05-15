use std::path::{Path, PathBuf};
use std::sync::Arc;

use skillsmgr_adapters::{claude_code, codex, gemini, hermes::HermesAdapter, opencode};
use skillsmgr_core::{
    AdapterPresence, Artifact, ArtifactKind, ScannedInstallation, Scope, ToolAdapter,
};
use skillsmgr_scan::{default_scopes, discover_project_root, scan_all, ScanError, ScanResult};

#[derive(Debug, Clone)]
pub struct AdapterStatus {
    pub adapter_id: String,
    pub presence: AdapterPresence,
}

#[derive(Debug, Clone)]
pub struct ArtifactGroup {
    pub name: String,
    pub kind: ArtifactKind,
    pub description: String,
    pub version: Option<String>,
    pub installations: Vec<ScannedInstallation>,
}

#[derive(Debug, Clone)]
pub struct Inventory {
    pub groups: Vec<ArtifactGroup>,
    pub adapters: Vec<AdapterStatus>,
    pub errors: Vec<ScanError>,
}

pub struct Service {
    adapters: Vec<Arc<dyn ToolAdapter>>,
}

impl Service {
    pub fn with_adapters(adapters: Vec<Arc<dyn ToolAdapter>>) -> Self {
        Self { adapters }
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
        Self { adapters }
    }

    pub async fn inventory(&self, cwd: Option<&Path>) -> Inventory {
        let project_root = cwd.and_then(discover_project_root);
        self.inventory_for_scopes(default_scopes(project_root))
            .await
    }

    pub async fn inventory_for_scopes(&self, scopes: Vec<Scope>) -> Inventory {
        let results = scan_all(self.adapters.clone(), scopes).await;
        build_inventory(results)
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
        group.installations.push(item);
        return;
    }

    groups.push(ArtifactGroup {
        name: item.artifact.name.clone(),
        kind: item.artifact.kind,
        description: item.artifact.description.clone(),
        version: item.artifact.version.clone(),
        installations: vec![item],
    });
}

fn merge_metadata(group: &mut ArtifactGroup, artifact: &Artifact) {
    if group.description.is_empty() && !artifact.description.is_empty() {
        group.description = artifact.description.clone();
    }
    if group.version.is_none() && artifact.version.is_some() {
        group.version.clone_from(&artifact.version);
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

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
        assert_eq!(polish.description, "Polish code");
        assert_eq!(polish.version.as_deref(), Some("1.0.0"));

        let review = inventory
            .groups
            .iter()
            .find(|g| g.name == "review-pr")
            .unwrap();
        assert_eq!(review.installations.len(), 1);
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
}
