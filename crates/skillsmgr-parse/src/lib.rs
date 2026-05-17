use std::path::{Path, PathBuf};

use serde::Deserialize;
use skillsmgr_core::{Artifact, ArtifactKind, Result, SkillsMgrError, Source};
use tokio::fs;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactCandidate {
    pub artifact: Artifact,
    pub root: PathBuf,
}

#[derive(Debug, Deserialize)]
struct SkillFrontmatter {
    name: Option<String>,
    description: Option<String>,
    version: Option<String>,
}

#[derive(Debug)]
struct ParsedSkillMarkdown {
    frontmatter: SkillFrontmatter,
    body: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiExtensionManifest {
    name: Option<String>,
    description: Option<String>,
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WarpWorkflow {
    name: Option<String>,
    description: Option<String>,
    command: Option<String>,
    commands: Option<Vec<String>>,
}

pub async fn sniff_artifacts(root: impl AsRef<Path>) -> Result<Vec<ArtifactCandidate>> {
    let root = root.as_ref();
    let mut candidates = Vec::new();

    if fs::try_exists(root.join("SKILL.md"))
        .await
        .map_err(|source| fs_error(root, source))?
    {
        candidates.push(parse_skill_dir(root).await?);
    }

    if fs::try_exists(root.join("gemini-extension.json"))
        .await
        .map_err(|source| fs_error(root, source))?
    {
        candidates.push(parse_gemini_extension_dir(root).await?);
    }

    let mut entries = fs::read_dir(root)
        .await
        .map_err(|source| fs_error(root, source))?;
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|source| fs_error(root, source))?
    {
        let path = entry.path();
        if is_yaml_file(&path) && looks_like_warp_workflow(&path).await? {
            candidates.push(parse_warp_workflow_file(&path).await?);
        }
    }

    Ok(candidates)
}

pub async fn parse_skill_dir(root: impl AsRef<Path>) -> Result<ArtifactCandidate> {
    let root = root.as_ref();
    let skill_path = root.join("SKILL.md");
    let content = fs::read_to_string(&skill_path)
        .await
        .map_err(|source| fs_error(&skill_path, source))?;
    let parsed = parse_skill_markdown(&content, &skill_path)?;
    let fallback_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("skill")
        .to_string();
    let name = parsed.frontmatter.name.unwrap_or(fallback_name);
    let artifact = Artifact::new(
        name,
        parsed.frontmatter.description.unwrap_or_default(),
        parsed.frontmatter.version,
        ArtifactKind::Skill,
        Source::Local {
            path: root.to_path_buf(),
        },
    )
    .with_body(parsed.body);

    Ok(ArtifactCandidate {
        artifact,
        root: root.to_path_buf(),
    })
}

pub async fn parse_gemini_extension_dir(root: impl AsRef<Path>) -> Result<ArtifactCandidate> {
    let root = root.as_ref();
    let manifest_path = root.join("gemini-extension.json");
    let content = fs::read_to_string(&manifest_path)
        .await
        .map_err(|source| fs_error(&manifest_path, source))?;
    let manifest: GeminiExtensionManifest =
        serde_json::from_str(&content).map_err(|error| invalid(&manifest_path, error))?;
    let fallback_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("gemini-extension")
        .to_string();
    let artifact = Artifact::new(
        manifest.name.unwrap_or(fallback_name),
        manifest.description.unwrap_or_default(),
        manifest.version,
        ArtifactKind::Extension,
        Source::Local {
            path: root.to_path_buf(),
        },
    );

    Ok(ArtifactCandidate {
        artifact,
        root: root.to_path_buf(),
    })
}

pub async fn parse_warp_workflow_file(path: impl AsRef<Path>) -> Result<ArtifactCandidate> {
    let path = path.as_ref();
    let content = fs::read_to_string(path)
        .await
        .map_err(|source| fs_error(path, source))?;
    let workflow: WarpWorkflow =
        serde_yaml::from_str(&content).map_err(|error| invalid(path, error))?;
    let fallback_name = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("workflow")
        .to_string();
    let description = workflow.description.unwrap_or_default();
    let artifact = Artifact::new(
        workflow.name.unwrap_or(fallback_name),
        description,
        None,
        ArtifactKind::Workflow,
        Source::Local {
            path: path.to_path_buf(),
        },
    );

    Ok(ArtifactCandidate {
        artifact,
        root: path.to_path_buf(),
    })
}

fn parse_skill_markdown(content: &str, path: &Path) -> Result<ParsedSkillMarkdown> {
    let Some(rest) = content.strip_prefix("---\n") else {
        return Ok(ParsedSkillMarkdown {
            frontmatter: SkillFrontmatter {
                name: None,
                description: None,
                version: None,
            },
            body: non_empty_body(content),
        });
    };

    let Some((yaml, body)) = rest.split_once("\n---") else {
        return Err(SkillsMgrError::InvalidArtifact {
            path: path.to_path_buf(),
            reason: "frontmatter starts with --- but has no closing ---".to_string(),
        });
    };

    let frontmatter = serde_yaml::from_str(yaml).map_err(|error| invalid(path, error))?;
    Ok(ParsedSkillMarkdown {
        frontmatter,
        body: non_empty_body(body.strip_prefix('\n').unwrap_or(body)),
    })
}

fn non_empty_body(body: &str) -> Option<String> {
    let trimmed = body.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

async fn looks_like_warp_workflow(path: &Path) -> Result<bool> {
    let content = fs::read_to_string(path)
        .await
        .map_err(|source| fs_error(path, source))?;
    let Ok(workflow) = serde_yaml::from_str::<WarpWorkflow>(&content) else {
        return Ok(false);
    };

    Ok(workflow.command.is_some() || workflow.commands.is_some())
}

fn is_yaml_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("yaml" | "yml")
    )
}

fn fs_error(path: impl AsRef<Path>, source: std::io::Error) -> SkillsMgrError {
    SkillsMgrError::Fs {
        path: path.as_ref().to_path_buf(),
        source,
    }
}

fn invalid(path: impl AsRef<Path>, error: impl std::error::Error) -> SkillsMgrError {
    SkillsMgrError::InvalidArtifact {
        path: path.as_ref().to_path_buf(),
        reason: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use skillsmgr_core::ArtifactKind;
    use tempfile::tempdir;

    use super::*;

    #[tokio::test]
    async fn sniffs_skill_dir() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("SKILL.md"),
            "---\nname: polish-code\ndescription: Improves code style\nversion: 0.1.0\n---\n# Body\n",
        )
        .unwrap();

        let candidates = sniff_artifacts(dir.path()).await.unwrap();

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].artifact.kind, ArtifactKind::Skill);
        assert_eq!(candidates[0].artifact.name, "polish-code");
        assert_eq!(candidates[0].artifact.description, "Improves code style");
        assert_eq!(candidates[0].artifact.version.as_deref(), Some("0.1.0"));
        assert_eq!(candidates[0].artifact.body.as_deref(), Some("# Body"));
    }

    #[tokio::test]
    async fn keeps_full_skill_body_without_frontmatter() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("SKILL.md"), "# Body\n\nUse carefully.\n").unwrap();

        let candidates = sniff_artifacts(dir.path()).await.unwrap();

        assert_eq!(
            candidates[0].artifact.body.as_deref(),
            Some("# Body\n\nUse carefully.")
        );
    }

    #[tokio::test]
    async fn sniffs_gemini_extension_dir() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("gemini-extension.json"),
            r#"{"name":"review-ext","description":"Review commands","version":"1.2.3"}"#,
        )
        .unwrap();

        let candidates = sniff_artifacts(dir.path()).await.unwrap();

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].artifact.kind, ArtifactKind::Extension);
        assert_eq!(candidates[0].artifact.name, "review-ext");
    }

    #[tokio::test]
    async fn sniffs_warp_workflow_file() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("ship.yaml"),
            "name: Ship build\ndescription: Build and test\ncommand: cargo test\n",
        )
        .unwrap();

        let candidates = sniff_artifacts(dir.path()).await.unwrap();

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].artifact.kind, ArtifactKind::Workflow);
        assert_eq!(candidates[0].artifact.name, "Ship build");
    }

    #[tokio::test]
    async fn empty_dir_has_no_candidates() {
        let dir = tempdir().unwrap();

        let candidates = sniff_artifacts(dir.path()).await.unwrap();

        assert!(candidates.is_empty());
    }
}
