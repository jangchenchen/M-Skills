use std::path::{Path, PathBuf};

use async_trait::async_trait;
use skillsmgr_core::{
    AdapterPresence, Artifact, ArtifactKind, Installation, Result, ScannedInstallation, Scope,
    SkillsMgrError, Target, ToolAdapter,
};
use skillsmgr_parse::parse_skill_dir;
use tokio::fs;

#[derive(Debug, Clone)]
pub struct HermesAdapter {
    root: PathBuf,
}

impl HermesAdapter {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn from_home(home: impl Into<PathBuf>) -> Self {
        Self::new(home.into().join(".hermes/skills"))
    }
}

#[async_trait]
impl ToolAdapter for HermesAdapter {
    fn id(&self) -> &'static str {
        "hermes"
    }

    fn supported_kinds(&self) -> &'static [ArtifactKind] {
        &[ArtifactKind::Skill]
    }

    async fn scan(&self, scope: Scope) -> Result<Vec<ScannedInstallation>> {
        if scope != Scope::Global {
            return Ok(Vec::new());
        }

        if !fs::try_exists(&self.root)
            .await
            .map_err(|source| fs_error(&self.root, source))?
        {
            return Ok(Vec::new());
        }

        let mut scanned = Vec::new();
        let mut categories = fs::read_dir(&self.root)
            .await
            .map_err(|source| fs_error(&self.root, source))?;

        while let Some(category) = categories
            .next_entry()
            .await
            .map_err(|source| fs_error(&self.root, source))?
        {
            let category_path = category.path();
            if !category
                .file_type()
                .await
                .map_err(|source| fs_error(&category_path, source))?
                .is_dir()
            {
                continue;
            }

            let mut skills = fs::read_dir(&category_path)
                .await
                .map_err(|source| fs_error(&category_path, source))?;
            while let Some(skill) = skills
                .next_entry()
                .await
                .map_err(|source| fs_error(&category_path, source))?
            {
                let skill_path = skill.path();
                if !skill
                    .file_type()
                    .await
                    .map_err(|source| fs_error(&skill_path, source))?
                    .is_dir()
                {
                    continue;
                }

                let Ok(candidate) = parse_skill_dir(&skill_path).await else {
                    continue;
                };
                let installation =
                    Installation::enabled(&candidate.artifact, Target::Hermes, &skill_path);
                scanned.push(ScannedInstallation {
                    artifact: candidate.artifact,
                    installation,
                });
            }
        }

        Ok(scanned)
    }

    async fn install(&self, _artifact: &Artifact, _scope: Scope) -> Result<Installation> {
        Err(read_only("install"))
    }

    async fn uninstall(&self, _installation: &Installation) -> Result<()> {
        Err(read_only("uninstall"))
    }

    async fn enable(&self, _installation: &Installation) -> Result<()> {
        Err(read_only("enable"))
    }

    async fn disable(&self, _installation: &Installation) -> Result<()> {
        Err(read_only("disable"))
    }

    async fn detect(&self) -> AdapterPresence {
        if self.root.exists() {
            AdapterPresence::Available
        } else {
            AdapterPresence::Missing {
                reason: format!("{} does not exist", self.root.display()),
            }
        }
    }
}

fn read_only(operation: &'static str) -> SkillsMgrError {
    SkillsMgrError::ReadOnly {
        tool: "hermes",
        operation,
    }
}

fn fs_error(path: impl AsRef<Path>, source: std::io::Error) -> SkillsMgrError {
    SkillsMgrError::Fs {
        path: path.as_ref().to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use skillsmgr_core::{Scope, ToolAdapter};
    use tempfile::tempdir;

    use super::*;

    #[tokio::test]
    async fn scans_category_skill_directories() {
        let root = tempdir().unwrap();
        let skill = root.path().join("coding").join("review");
        fs::create_dir_all(&skill).unwrap();
        fs::write(
            skill.join("SKILL.md"),
            "---\nname: review\ndescription: Review code\n---\n# Review\n",
        )
        .unwrap();
        let adapter = HermesAdapter::new(root.path());

        let scanned = adapter.scan(Scope::Global).await.unwrap();

        assert_eq!(scanned.len(), 1);
        assert_eq!(scanned[0].artifact.name, "review");
        assert_eq!(scanned[0].installation.target, Target::Hermes);
    }

    #[tokio::test]
    async fn is_read_only_for_writes() {
        let root = tempdir().unwrap();
        let adapter = HermesAdapter::new(root.path());
        let artifact = Artifact::new(
            "review",
            "",
            None,
            ArtifactKind::Skill,
            skillsmgr_core::Source::Unknown,
        );

        let error = adapter.install(&artifact, Scope::Global).await.unwrap_err();

        assert!(matches!(
            error,
            SkillsMgrError::ReadOnly {
                tool: "hermes",
                operation: "install"
            }
        ));
    }
}
