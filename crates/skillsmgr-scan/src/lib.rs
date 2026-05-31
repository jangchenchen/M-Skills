use std::path::{Path, PathBuf};
use std::sync::Arc;

use skillsmgr_core::{AdapterPresence, ScannedInstallation, Scope, ToolAdapter};
use tokio::task::JoinSet;

#[derive(Debug, Clone)]
pub struct ScanError {
    pub adapter_id: String,
    pub scope: Scope,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub adapter_id: String,
    pub presence: AdapterPresence,
    pub supported_kinds: &'static [skillsmgr_core::ArtifactKind],
    pub writable: bool,
    pub supports_disable: bool,
    pub items: Vec<ScannedInstallation>,
    pub errors: Vec<ScanError>,
}

pub async fn scan_one(adapter: Arc<dyn ToolAdapter>, scopes: Vec<Scope>) -> ScanResult {
    let adapter_id = adapter.id().to_string();
    let presence = adapter.detect().await;
    let supported_kinds = adapter.supported_kinds();
    let writable = adapter.is_writable();
    let supports_disable = adapter.supports_disable();

    let mut items = Vec::new();
    let mut errors = Vec::new();
    for scope in scopes {
        match adapter.scan(scope.clone()).await {
            Ok(found) => items.extend(found),
            Err(error) => errors.push(ScanError {
                adapter_id: adapter_id.clone(),
                scope,
                message: error.to_string(),
            }),
        }
    }

    ScanResult {
        adapter_id,
        presence,
        supported_kinds,
        writable,
        supports_disable,
        items,
        errors,
    }
}

pub async fn scan_all(adapters: Vec<Arc<dyn ToolAdapter>>, scopes: Vec<Scope>) -> Vec<ScanResult> {
    let mut join_set = JoinSet::new();
    for adapter in adapters {
        let scopes = scopes.clone();
        join_set.spawn(async move { scan_one(adapter, scopes).await });
    }

    let mut results = Vec::new();
    while let Some(joined) = join_set.join_next().await {
        match joined {
            Ok(result) => results.push(result),
            Err(error) => results.push(ScanResult {
                adapter_id: "<join-error>".to_string(),
                presence: AdapterPresence::Missing {
                    reason: error.to_string(),
                },
                supported_kinds: &[],
                writable: false,
                supports_disable: false,
                items: Vec::new(),
                errors: Vec::new(),
            }),
        }
    }

    results.sort_by(|a, b| a.adapter_id.cmp(&b.adapter_id));
    results
}

pub fn discover_project_root(start: impl AsRef<Path>) -> Option<PathBuf> {
    let mut current: Option<&Path> = Some(start.as_ref());
    while let Some(dir) = current {
        if dir.join(".git").exists() {
            return Some(dir.to_path_buf());
        }
        current = dir.parent();
    }
    None
}

pub fn default_scopes(project_root: Option<PathBuf>) -> Vec<Scope> {
    let mut scopes = vec![Scope::Global];
    if let Some(root) = project_root {
        scopes.push(Scope::Project(root));
    }
    scopes
}

#[cfg(test)]
mod tests {
    use std::fs;

    use skillsmgr_adapters::{claude_code, codex, opencode};
    use skillsmgr_core::SourceProvenance;
    use tempfile::tempdir;

    use super::*;

    fn write_skill(root: &Path, name: &str, description: &str) {
        let dir = root.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: {description}\n---\n# {name}\n"),
        )
        .unwrap();
    }

    #[tokio::test]
    async fn aggregates_skills_from_multiple_adapters_at_global_scope() {
        let home = tempdir().unwrap();
        write_skill(
            &home.path().join(".claude/skills"),
            "polish-code",
            "Polish code",
        );
        write_skill(
            &home.path().join(".agents/skills"),
            "review-pr",
            "Review PR",
        );

        let adapters: Vec<Arc<dyn ToolAdapter>> = vec![
            Arc::new(claude_code::adapter(home.path())),
            Arc::new(codex::adapter(home.path())),
            Arc::new(opencode::adapter(home.path())),
        ];

        let results = scan_all(adapters, vec![Scope::Global]).await;

        let claude = results
            .iter()
            .find(|r| r.adapter_id == "claude-code")
            .unwrap();
        assert_eq!(claude.items.len(), 1);
        assert_eq!(claude.items[0].artifact.name, "polish-code");

        let codex = results.iter().find(|r| r.adapter_id == "codex").unwrap();
        assert_eq!(codex.items.len(), 1);
        assert_eq!(codex.items[0].artifact.name, "review-pr");

        let opencode = results.iter().find(|r| r.adapter_id == "opencode").unwrap();
        assert_eq!(opencode.items.len(), 2);
        assert!(opencode.items.iter().any(|item| {
            item.artifact.name == "polish-code"
                && item.provenance
                    == SourceProvenance::Shared {
                        from_tool: "claude-code".to_string(),
                    }
        }));
        assert!(opencode.items.iter().any(|item| {
            item.artifact.name == "review-pr"
                && item.provenance
                    == SourceProvenance::Shared {
                        from_tool: "shared-global".to_string(),
                    }
        }));
    }

    #[tokio::test]
    async fn picks_up_project_scope_skills_when_scope_supplied() {
        let home = tempdir().unwrap();
        let project = tempdir().unwrap();
        write_skill(
            &project.path().join(".claude/skills"),
            "local-skill",
            "Local",
        );

        let adapters: Vec<Arc<dyn ToolAdapter>> = vec![Arc::new(claude_code::adapter(home.path()))];

        let results = scan_all(
            adapters,
            vec![Scope::Global, Scope::Project(project.path().to_path_buf())],
        )
        .await;

        let claude = &results[0];
        assert_eq!(claude.items.len(), 1);
        assert_eq!(claude.items[0].artifact.name, "local-skill");
        assert!(matches!(
            claude.items[0].installation.target,
            skillsmgr_core::Target::ClaudeCode {
                scope: Scope::Project(_)
            }
        ));
    }

    #[tokio::test]
    async fn discover_project_root_walks_up_to_git() {
        let workspace = tempdir().unwrap();
        fs::create_dir_all(workspace.path().join(".git")).unwrap();
        let deep = workspace.path().join("a/b/c");
        fs::create_dir_all(&deep).unwrap();

        let found = discover_project_root(&deep).unwrap();
        let canonical = std::fs::canonicalize(&found).unwrap();
        let expected = std::fs::canonicalize(workspace.path()).unwrap();
        assert_eq!(canonical, expected);
    }

    #[tokio::test]
    async fn discover_project_root_returns_none_when_no_git() {
        let workspace = tempdir().unwrap();
        assert!(discover_project_root(workspace.path()).is_none());
    }
}
