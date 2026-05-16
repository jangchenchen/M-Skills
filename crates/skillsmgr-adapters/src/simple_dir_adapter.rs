use std::path::{Path, PathBuf};

use async_trait::async_trait;
use skillsmgr_core::{
    ensure_target_supports_kind, AdapterPresence, Artifact, ArtifactKind, Installation, Result,
    ScannedInstallation, Scope, SkillsMgrError, Source, SourceProvenance, Target, ToolAdapter,
};
use skillsmgr_parse::{parse_gemini_extension_dir, parse_skill_dir};
use tokio::fs;
use toml_edit::{value, ArrayOfTables, DocumentMut, Item, Table};

#[derive(Debug, Clone)]
pub struct DirectoryLayout {
    id: &'static str,
    kind: ArtifactKind,
    read_only: bool,
    config_path: Option<PathBuf>,
    target_for_scope: fn(Scope) -> Target,
    roots: Vec<SourceRoot>,
    project_relative_root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SourceRoot {
    path: PathBuf,
    provenance: SourceProvenance,
}

impl SourceRoot {
    pub fn owned(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            provenance: SourceProvenance::Owned,
        }
    }

    pub fn shared(path: impl Into<PathBuf>, from_tool: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            provenance: SourceProvenance::Shared {
                from_tool: from_tool.into(),
            },
        }
    }
}

impl DirectoryLayout {
    pub fn skill(
        id: &'static str,
        target_for_scope: fn(Scope) -> Target,
        global_root: impl Into<PathBuf>,
        project_relative_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            id,
            kind: ArtifactKind::Skill,
            read_only: false,
            config_path: None,
            target_for_scope,
            roots: vec![SourceRoot::owned(global_root)],
            project_relative_root: project_relative_root.into(),
        }
    }

    pub fn read_only_skill(
        id: &'static str,
        target_for_scope: fn(Scope) -> Target,
        global_root: impl Into<PathBuf>,
        project_relative_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            id,
            kind: ArtifactKind::Skill,
            read_only: true,
            config_path: None,
            target_for_scope,
            roots: vec![SourceRoot::owned(global_root)],
            project_relative_root: project_relative_root.into(),
        }
    }

    pub fn extension(
        id: &'static str,
        target_for_scope: fn(Scope) -> Target,
        global_root: impl Into<PathBuf>,
        project_relative_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            id,
            kind: ArtifactKind::Extension,
            read_only: false,
            config_path: None,
            target_for_scope,
            roots: vec![SourceRoot::owned(global_root)],
            project_relative_root: project_relative_root.into(),
        }
    }

    pub fn skill_with_roots(
        id: &'static str,
        target_for_scope: fn(Scope) -> Target,
        roots: Vec<SourceRoot>,
        project_relative_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            id,
            kind: ArtifactKind::Skill,
            read_only: false,
            config_path: None,
            target_for_scope,
            roots,
            project_relative_root: project_relative_root.into(),
        }
    }

    pub fn with_config_path(mut self, config_path: impl Into<PathBuf>) -> Self {
        self.config_path = Some(config_path.into());
        self
    }

    fn roots_for_scope(&self, scope: &Scope) -> Vec<SourceRoot> {
        match scope {
            Scope::Global => self.roots.clone(),
            Scope::Project(project_root) => vec![SourceRoot::owned(
                project_root.join(&self.project_relative_root),
            )],
        }
    }

    fn owned_root_for_scope(&self, scope: &Scope) -> PathBuf {
        match scope {
            Scope::Global => self
                .roots
                .iter()
                .find(|root| matches!(root.provenance, SourceProvenance::Owned))
                .map(|root| root.path.clone())
                .unwrap_or_else(|| self.roots[0].path.clone()),
            Scope::Project(project_root) => project_root.join(&self.project_relative_root),
        }
    }

    fn target_for_scope(&self, scope: Scope) -> Target {
        (self.target_for_scope)(scope)
    }
}

#[async_trait]
impl ToolAdapter for DirectoryLayout {
    fn id(&self) -> &'static str {
        self.id
    }

    fn supported_kinds(&self) -> &'static [ArtifactKind] {
        match self.kind {
            ArtifactKind::Skill => &[ArtifactKind::Skill],
            ArtifactKind::Extension => &[ArtifactKind::Extension],
            ArtifactKind::Workflow => &[ArtifactKind::Workflow],
        }
    }

    async fn scan(&self, scope: Scope) -> Result<Vec<ScannedInstallation>> {
        let mut scanned = Vec::new();
        for source_root in self.roots_for_scope(&scope) {
            let root = source_root.path;
            if !fs::try_exists(&root)
                .await
                .map_err(|source| fs_error(&root, source))?
            {
                continue;
            }

            let mut entries = fs::read_dir(&root)
                .await
                .map_err(|source| fs_error(&root, source))?;

            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(|source| fs_error(&root, source))?
            {
                let path = entry.path();
                if !entry
                    .file_type()
                    .await
                    .map_err(|source| fs_error(&path, source))?
                    .is_dir()
                {
                    continue;
                }

                let candidate = match self.kind {
                    ArtifactKind::Skill => parse_skill_dir(&path).await,
                    ArtifactKind::Extension => parse_gemini_extension_dir(&path).await,
                    ArtifactKind::Workflow => continue,
                };

                let Ok(candidate) = candidate else {
                    continue;
                };

                let installation = Installation::enabled(
                    &candidate.artifact,
                    self.target_for_scope(scope.clone()),
                    &path,
                );
                scanned.push(ScannedInstallation {
                    artifact: candidate.artifact,
                    installation,
                    provenance: source_root.provenance.clone(),
                });
            }
        }

        Ok(scanned)
    }

    async fn install(&self, artifact: &Artifact, scope: Scope) -> Result<Installation> {
        if self.read_only {
            return Err(self.read_only_error("install"));
        }

        let target = self.target_for_scope(scope.clone());
        ensure_target_supports_kind(&target, artifact.kind)?;

        let root = self.owned_root_for_scope(&scope);
        let destination = root.join(&artifact.name);
        if fs::try_exists(&destination)
            .await
            .map_err(|source| fs_error(&destination, source))?
        {
            return Err(SkillsMgrError::Conflict {
                name: artifact.name.clone(),
                path: destination,
            });
        }

        fs::create_dir_all(&destination)
            .await
            .map_err(|source| fs_error(&destination, source))?;
        if let Source::Local { path } = &artifact.source {
            copy_dir_contents(path, &destination)?;
        }
        Ok(Installation::enabled(artifact, target, destination))
    }

    async fn uninstall(&self, installation: &Installation) -> Result<()> {
        if self.read_only {
            return Err(self.read_only_error("uninstall"));
        }

        if installation.target.tool_id() != self.id {
            return Err(SkillsMgrError::UnsupportedTarget {
                adapter_id: self.id.to_string(),
                target: installation.target.clone(),
            });
        }
        let managed_root =
            self.owned_root_for_scope(installation.target.scope().ok_or_else(|| {
                SkillsMgrError::UnsupportedTarget {
                    adapter_id: self.id.to_string(),
                    target: installation.target.clone(),
                }
            })?);
        ensure_managed_child(&managed_root, &installation.on_disk_path)?;

        if installation.on_disk_path.exists() {
            fs::remove_dir_all(&installation.on_disk_path)
                .await
                .map_err(|source| fs_error(&installation.on_disk_path, source))?;
        }
        Ok(())
    }

    async fn enable(&self, installation: &Installation) -> Result<()> {
        if self.read_only {
            return Err(self.read_only_error("enable"));
        }
        if installation.target.tool_id() != self.id {
            return Err(SkillsMgrError::UnsupportedTarget {
                adapter_id: self.id.to_string(),
                target: installation.target.clone(),
            });
        }

        if let Some(config_path) = &self.config_path {
            set_skill_config_enabled(config_path, installation, true).await?;
        }
        Ok(())
    }

    async fn disable(&self, installation: &Installation) -> Result<()> {
        if self.read_only {
            return Err(self.read_only_error("disable"));
        }
        if installation.target.tool_id() != self.id {
            return Err(SkillsMgrError::UnsupportedTarget {
                adapter_id: self.id.to_string(),
                target: installation.target.clone(),
            });
        }

        let Some(config_path) = &self.config_path else {
            return Err(self.read_only_error("disable"));
        };
        set_skill_config_enabled(config_path, installation, false).await
    }

    async fn detect(&self) -> AdapterPresence {
        // MVP: Available means at least one configured source directory exists.
        if self.roots.iter().any(|root| root.path.exists()) {
            AdapterPresence::Available
        } else {
            AdapterPresence::Missing {
                reason: format!("no configured {} source directories exist", self.id),
            }
        }
    }
}

impl DirectoryLayout {
    fn read_only_error(&self, operation: &'static str) -> SkillsMgrError {
        SkillsMgrError::ReadOnly {
            tool: self.id,
            operation,
        }
    }
}

fn fs_error(path: impl AsRef<Path>, source: std::io::Error) -> SkillsMgrError {
    SkillsMgrError::Fs {
        path: path.as_ref().to_path_buf(),
        source,
    }
}

fn copy_dir_contents(source: &Path, destination: &Path) -> Result<()> {
    for entry in std::fs::read_dir(source).map_err(|error| fs_error(source, error))? {
        let entry = entry.map_err(|error| fs_error(source, error))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|error| fs_error(&source_path, error))?;

        if file_type.is_dir() {
            std::fs::create_dir_all(&destination_path)
                .map_err(|error| fs_error(&destination_path, error))?;
            copy_dir_contents(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            std::fs::copy(&source_path, &destination_path)
                .map_err(|error| fs_error(&destination_path, error))?;
        }
    }

    Ok(())
}

fn ensure_managed_child(managed_root: &Path, path: &Path) -> Result<()> {
    if path.parent() != Some(managed_root) {
        return Err(SkillsMgrError::UnsafePath {
            path: path.to_path_buf(),
            reason: format!("path is not directly under {}", managed_root.display()),
        });
    }
    Ok(())
}

async fn set_skill_config_enabled(
    config_path: &Path,
    installation: &Installation,
    enabled: bool,
) -> Result<()> {
    let config_path = config_path.to_path_buf();
    let error_path = config_path.clone();
    let installation = installation.clone();
    tokio::task::spawn_blocking(move || {
        set_skill_config_enabled_blocking(&config_path, &installation, enabled)
    })
    .await
    .map_err(|error| SkillsMgrError::Fs {
        path: error_path,
        source: std::io::Error::new(std::io::ErrorKind::Other, error),
    })?
}

fn set_skill_config_enabled_blocking(
    config_path: &Path,
    installation: &Installation,
    enabled: bool,
) -> Result<()> {
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| fs_error(parent, error))?;
    }

    let content = match std::fs::read_to_string(config_path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(fs_error(config_path, error)),
    };
    let mut document =
        content
            .parse::<DocumentMut>()
            .map_err(|error| SkillsMgrError::InvalidArtifact {
                path: config_path.to_path_buf(),
                reason: error.to_string(),
            })?;

    ensure_skills_config_array(&mut document);
    let path = installation.on_disk_path.to_string_lossy().to_string();
    let name = installation
        .on_disk_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_string();
    let configs = document["skills"]["config"]
        .as_array_of_tables_mut()
        .expect("skills.config is an array of tables");

    let mut found = false;
    for table in configs.iter_mut() {
        if table
            .get("path")
            .and_then(Item::as_str)
            .is_some_and(|existing| existing == path)
        {
            table["enabled"] = value(enabled);
            found = true;
            break;
        }
    }

    if !found {
        let mut table = Table::new();
        table["name"] = value(name);
        table["path"] = value(path);
        table["enabled"] = value(enabled);
        configs.push(table);
    }

    std::fs::write(config_path, document.to_string()).map_err(|error| fs_error(config_path, error))
}

fn ensure_skills_config_array(document: &mut DocumentMut) {
    if !document.as_table().contains_key("skills") {
        document["skills"] = Item::Table(Table::new());
    }

    if !document["skills"]
        .as_table_like()
        .is_some_and(|table| table.contains_key("config"))
    {
        document["skills"]["config"] = Item::ArrayOfTables(ArrayOfTables::new());
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use skillsmgr_core::{Artifact, ArtifactKind, Scope, Source, SourceProvenance, ToolAdapter};
    use tempfile::tempdir;

    use super::*;

    #[tokio::test]
    async fn scans_skill_directories() {
        let home = tempdir().unwrap();
        let root = home.path().join("skills");
        fs::create_dir_all(root.join("one")).unwrap();
        fs::write(
            root.join("one").join("SKILL.md"),
            "---\nname: one\ndescription: First\n---\n# One\n",
        )
        .unwrap();
        let adapter = DirectoryLayout::skill(
            "test",
            |scope| Target::Codex { scope },
            &root,
            ".agents/skills",
        );

        let scanned = adapter.scan(Scope::Global).await.unwrap();

        assert_eq!(scanned.len(), 1);
        assert_eq!(scanned[0].artifact.name, "one");
        assert_eq!(scanned[0].installation.target.tool_id(), "codex");
        assert_eq!(scanned[0].provenance, SourceProvenance::Owned);
    }

    #[tokio::test]
    async fn refuses_wrong_kind_install() {
        let home = tempdir().unwrap();
        let adapter = DirectoryLayout::skill(
            "test",
            |scope| Target::Codex { scope },
            home.path().join("skills"),
            ".agents/skills",
        );
        let artifact = Artifact::new("ext", "", None, ArtifactKind::Extension, Source::Unknown);

        let error = adapter.install(&artifact, Scope::Global).await.unwrap_err();

        assert!(matches!(
            error,
            SkillsMgrError::UnsupportedKind {
                kind: ArtifactKind::Extension,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn install_copies_local_source_contents() {
        let home = tempdir().unwrap();
        let source = tempdir().unwrap();
        fs::write(source.path().join("SKILL.md"), "# Demo\n").unwrap();
        fs::create_dir_all(source.path().join("scripts")).unwrap();
        fs::write(source.path().join("scripts").join("run.sh"), "echo hi\n").unwrap();
        let adapter = DirectoryLayout::skill(
            "test",
            |scope| Target::Codex { scope },
            home.path().join("skills"),
            ".agents/skills",
        );
        let artifact = Artifact::new(
            "demo",
            "",
            None,
            ArtifactKind::Skill,
            Source::Local {
                path: source.path().to_path_buf(),
            },
        );

        let installation = adapter.install(&artifact, Scope::Global).await.unwrap();

        assert!(installation.on_disk_path.join("SKILL.md").exists());
        assert!(installation
            .on_disk_path
            .join("scripts")
            .join("run.sh")
            .exists());
    }

    #[tokio::test]
    async fn uninstall_refuses_path_outside_managed_layout() {
        let home = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let adapter = DirectoryLayout::skill(
            "codex",
            |scope| Target::Codex { scope },
            home.path().join("skills"),
            ".agents/skills",
        );
        let artifact = Artifact::new("demo", "", None, ArtifactKind::Skill, Source::Unknown);
        let installation = Installation::enabled(
            &artifact,
            Target::Codex {
                scope: Scope::Global,
            },
            outside.path().join("demo"),
        );

        let error = adapter.uninstall(&installation).await.unwrap_err();

        assert!(matches!(error, SkillsMgrError::UnsafePath { .. }));
    }

    #[tokio::test]
    async fn codex_disable_and_enable_write_skills_config_entries() {
        let home = tempdir().unwrap();
        let config = home.path().join(".codex/config.toml");
        let adapter = DirectoryLayout::skill(
            "codex",
            |scope| Target::Codex { scope },
            home.path().join(".agents/skills"),
            ".agents/skills",
        )
        .with_config_path(&config);
        let artifact = Artifact::new("demo", "", None, ArtifactKind::Skill, Source::Unknown);
        let installation = Installation::enabled(
            &artifact,
            Target::Codex {
                scope: Scope::Global,
            },
            home.path().join(".agents/skills/demo"),
        );

        adapter.disable(&installation).await.unwrap();
        let disabled = fs::read_to_string(&config).unwrap();
        assert!(disabled.contains("[[skills.config]]"));
        assert!(disabled.contains("name = \"demo\""));
        assert!(disabled.contains("enabled = false"));

        adapter.enable(&installation).await.unwrap();
        let enabled = fs::read_to_string(&config).unwrap();
        assert!(enabled.contains("enabled = true"));
    }

    #[tokio::test]
    async fn read_only_layout_scans_but_refuses_writes() {
        let home = tempdir().unwrap();
        let root = home.path().join("skills");
        fs::create_dir_all(root.join("one")).unwrap();
        fs::write(root.join("one").join("SKILL.md"), "# One\n").unwrap();
        let adapter = DirectoryLayout::read_only_skill(
            "readonly",
            |scope| Target::Openclaw { scope },
            &root,
            ".openclaw/skills",
        );

        let scanned = adapter.scan(Scope::Global).await.unwrap();
        let error = adapter
            .uninstall(&scanned[0].installation)
            .await
            .unwrap_err();

        assert_eq!(scanned.len(), 1);
        assert!(matches!(
            error,
            SkillsMgrError::ReadOnly {
                tool: "readonly",
                operation: "uninstall"
            }
        ));
    }

    #[tokio::test]
    async fn scans_owned_and_shared_global_roots() {
        let home = tempdir().unwrap();
        let owned_root = home.path().join("owned");
        let shared_root = home.path().join("shared");
        fs::create_dir_all(owned_root.join("owned-skill")).unwrap();
        fs::create_dir_all(shared_root.join("shared-skill")).unwrap();
        fs::write(owned_root.join("owned-skill").join("SKILL.md"), "# Owned\n").unwrap();
        fs::write(
            shared_root.join("shared-skill").join("SKILL.md"),
            "# Shared\n",
        )
        .unwrap();
        let adapter = DirectoryLayout::skill_with_roots(
            "multi",
            |scope| Target::Opencode { scope },
            vec![
                SourceRoot::owned(&owned_root),
                SourceRoot::shared(&shared_root, "claude-code"),
            ],
            ".opencode/skills",
        );

        let scanned = adapter.scan(Scope::Global).await.unwrap();

        assert_eq!(scanned.len(), 2);
        assert!(scanned.iter().any(|item| {
            item.artifact.name == "owned-skill" && item.provenance == SourceProvenance::Owned
        }));
        assert!(scanned.iter().any(|item| {
            item.artifact.name == "shared-skill"
                && item.provenance
                    == SourceProvenance::Shared {
                        from_tool: "claude-code".to_string(),
                    }
        }));
    }
}
