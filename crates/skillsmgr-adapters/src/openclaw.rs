use std::path::{Path, PathBuf};

use skillsmgr_core::Target;

use crate::DirectoryLayout;

pub fn adapter(home: impl Into<PathBuf>) -> DirectoryLayout {
    DirectoryLayout::read_only_skill(
        "openclaw",
        |scope| Target::Openclaw { scope },
        home.into().join(".openclaw/skills"),
        "skills",
    )
}

pub fn workspace_adapter(root: impl Into<PathBuf>) -> DirectoryLayout {
    let root = root.into();
    DirectoryLayout::read_only_skill(
        "openclaw-workspace",
        |scope| Target::Openclaw { scope },
        root.join(".openclaw/skills"),
        "skills",
    )
}

pub fn documented_project_root(project_root: impl AsRef<Path>) -> PathBuf {
    let project_root = project_root.as_ref();
    project_root.join("skills")
}

#[cfg(test)]
mod tests {
    use skillsmgr_core::{Artifact, ArtifactKind, Scope, SkillsMgrError, Source, ToolAdapter};
    use tempfile::tempdir;

    use super::*;

    #[tokio::test]
    async fn workspace_scan_uses_documented_skills_root() {
        let workspace = tempdir().unwrap();
        std::fs::create_dir_all(workspace.path().join("skills").join("demo")).unwrap();
        std::fs::write(
            workspace
                .path()
                .join("skills")
                .join("demo")
                .join("SKILL.md"),
            "---\nname: demo\ndescription: Demo\n---\n# Demo\n",
        )
        .unwrap();
        let adapter = workspace_adapter(workspace.path());

        let scanned = adapter
            .scan(Scope::Project(workspace.path().to_path_buf()))
            .await
            .unwrap();

        assert_eq!(
            documented_project_root(workspace.path()),
            workspace.path().join("skills")
        );
        assert_eq!(scanned.len(), 1);
        assert_eq!(scanned[0].artifact.name, "demo");
        assert!(matches!(
            scanned[0].installation.target,
            Target::Openclaw {
                scope: Scope::Project(_)
            }
        ));
    }

    #[tokio::test]
    async fn openclaw_stub_is_read_only() {
        let home = tempdir().unwrap();
        let adapter = adapter(home.path());
        let artifact = Artifact::new("demo", "", None, ArtifactKind::Skill, Source::Unknown);

        let error = adapter.install(&artifact, Scope::Global).await.unwrap_err();

        assert!(matches!(
            error,
            SkillsMgrError::ReadOnly {
                tool: "openclaw",
                operation: "install"
            }
        ));
    }
}
