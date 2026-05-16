use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use gix::progress::Discard;
use gix::remote::fetch::Shallow;
use serde_json::Value as JsonValue;
use skillsmgr_core::{Artifact, ArtifactKind, Result, SkillsMgrError, Source, Target};
use skillsmgr_parse::{sniff_artifacts, ArtifactCandidate};
use tempfile::TempDir;
use tokio::fs;
use walkdir::WalkDir;

#[derive(Debug)]
pub struct ImportPreview {
    pub source: ImportSource,
    pub stage: ImportStage,
    pub candidates: Vec<ImportCandidate>,
    pub audit: ImportAudit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportSource {
    Local { path: PathBuf },
    GitHub { url: String },
}

#[derive(Debug)]
pub struct ImportStage {
    temp_dir: TempDir,
    root: PathBuf,
    pub resolved_commit_sha: Option<String>,
}

impl ImportStage {
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn into_temp_dir(self) -> TempDir {
        self.temp_dir
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportCandidate {
    pub artifact: Artifact,
    pub staged_root: PathBuf,
    pub compatible_targets: Vec<Target>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportAudit {
    pub files: Vec<AuditFile>,
    pub metadata: Vec<AuditMetadata>,
    pub warnings: Vec<AuditWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditFile {
    pub path: PathBuf,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditMetadata {
    pub path: PathBuf,
    pub fields: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditWarning {
    pub path: PathBuf,
    pub kind: AuditWarningKind,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditWarningKind {
    ExecutableCommand,
    McpConfig,
}

pub async fn preview_local_import(
    path: impl AsRef<Path>,
    available_targets: &[Target],
) -> Result<ImportPreview> {
    let source_path = path.as_ref();
    let stage = stage_local_path(source_path).await?;
    build_preview(
        ImportSource::Local {
            path: source_path.to_path_buf(),
        },
        stage,
        available_targets,
    )
    .await
}

pub async fn preview_github_import(
    url: impl Into<String>,
    available_targets: &[Target],
) -> Result<ImportPreview> {
    let url = url.into();
    if !is_github_url(&url) {
        return Err(SkillsMgrError::UnsupportedImportSource { input: url });
    }

    let stage = clone_github_source(&url).await?;
    build_preview(ImportSource::GitHub { url }, stage, available_targets).await
}

async fn build_preview(
    source: ImportSource,
    stage: ImportStage,
    available_targets: &[Target],
) -> Result<ImportPreview> {
    let raw_candidates = sniff_artifacts(stage.root()).await?;
    if raw_candidates.is_empty() {
        return Err(SkillsMgrError::NoSupportedArtifacts {
            path: stage.root().to_path_buf(),
        });
    }

    let resolved_commit_sha = stage.resolved_commit_sha.clone();
    let candidates = raw_candidates
        .into_iter()
        .map(|candidate| {
            import_candidate(
                candidate,
                &source,
                resolved_commit_sha.as_deref(),
                available_targets,
            )
        })
        .collect();
    let audit = audit_stage(stage.root()).await?;

    Ok(ImportPreview {
        source,
        stage,
        candidates,
        audit,
    })
}

fn import_candidate(
    candidate: ArtifactCandidate,
    source: &ImportSource,
    resolved_commit_sha: Option<&str>,
    available_targets: &[Target],
) -> ImportCandidate {
    let mut artifact = candidate.artifact;
    artifact.source = match source {
        ImportSource::Local { .. } => Source::Local {
            path: candidate.root.clone(),
        },
        ImportSource::GitHub { url } => Source::GitHub {
            url: url.clone(),
            rev: resolved_commit_sha.unwrap_or_default().to_string(),
        },
    };

    let compatible_targets = compatible_targets(available_targets, artifact.kind);
    ImportCandidate {
        artifact,
        staged_root: candidate.root,
        compatible_targets,
    }
}

pub fn compatible_targets(available_targets: &[Target], kind: ArtifactKind) -> Vec<Target> {
    available_targets
        .iter()
        .filter(|target| target.supports_kind(kind))
        .cloned()
        .collect()
}

async fn stage_local_path(source: &Path) -> Result<ImportStage> {
    if !fs::try_exists(source)
        .await
        .map_err(|source_error| fs_error(source, source_error))?
    {
        return Err(SkillsMgrError::InvalidArtifact {
            path: source.to_path_buf(),
            reason: "source path does not exist".to_string(),
        });
    }

    let temp_dir = tempfile::tempdir().map_err(|source_error| fs_error(source, source_error))?;
    let root = temp_dir.path().join(
        source
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("import"),
    );
    copy_path(source, &root).await?;
    Ok(ImportStage {
        temp_dir,
        root,
        resolved_commit_sha: None,
    })
}

async fn clone_github_source(url: &str) -> Result<ImportStage> {
    let temp_dir = tempfile::tempdir().map_err(|error| SkillsMgrError::Git {
        input: url.to_string(),
        message: error.to_string(),
    })?;
    let root = temp_dir.path().join("repo");
    let url = url.to_string();
    let error_url = url.clone();
    let clone_root = root.clone();

    let commit_sha = tokio::task::spawn_blocking(move || {
        let mut prepare = gix::prepare_clone(url.as_str(), &clone_root)
            .map_err(|error| git_error(&url, error))?
            .with_shallow(Shallow::DepthAtRemote(
                1.try_into().expect("non-zero depth"),
            ));
        let (mut checkout, _outcome) = prepare
            .fetch_then_checkout(Discard, &std::sync::atomic::AtomicBool::new(false))
            .map_err(|error| git_error(&url, error))?;
        let (repo, _outcome) = checkout
            .main_worktree(Discard, &std::sync::atomic::AtomicBool::new(false))
            .map_err(|error| git_error(&url, error))?;
        repo.head_commit()
            .map(|commit| commit.id().to_string())
            .map_err(|error| git_error(&url, error))
    })
    .await
    .map_err(|error| SkillsMgrError::Git {
        input: error_url,
        message: error.to_string(),
    })??;

    Ok(ImportStage {
        temp_dir,
        root,
        resolved_commit_sha: Some(commit_sha),
    })
}

async fn audit_stage(root: &Path) -> Result<ImportAudit> {
    let root = root.to_path_buf();
    let error_path = root.clone();
    tokio::task::spawn_blocking(move || audit_stage_blocking(&root))
        .await
        .map_err(|error| SkillsMgrError::Fs {
            path: error_path,
            source: std::io::Error::new(std::io::ErrorKind::Other, error),
        })?
}

fn audit_stage_blocking(root: &Path) -> Result<ImportAudit> {
    let mut files = Vec::new();
    let mut metadata = Vec::new();
    let mut warnings = Vec::new();

    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(|error| SkillsMgrError::Fs {
            path: error
                .path()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| root.to_path_buf()),
            source: error
                .into_io_error()
                .unwrap_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "walkdir error")),
        })?;
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        let relative = path.strip_prefix(root).unwrap_or(path).to_path_buf();
        let stat = entry.metadata().map_err(|source| SkillsMgrError::Fs {
            path: path.to_path_buf(),
            source: source
                .into_io_error()
                .unwrap_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "walkdir error")),
        })?;
        files.push(AuditFile {
            path: relative.clone(),
            size_bytes: stat.len(),
        });

        collect_metadata(path, &relative, &mut metadata)?;
        collect_warnings(path, &relative, &mut warnings)?;
    }

    files.sort_by(|a, b| a.path.cmp(&b.path));
    metadata.sort_by(|a, b| a.path.cmp(&b.path));
    warnings.sort_by(|a, b| a.path.cmp(&b.path).then_with(|| a.message.cmp(&b.message)));

    Ok(ImportAudit {
        files,
        metadata,
        warnings,
    })
}

fn collect_metadata(path: &Path, relative: &Path, metadata: &mut Vec<AuditMetadata>) -> Result<()> {
    match path.file_name().and_then(|name| name.to_str()) {
        Some("SKILL.md") => {
            let content = std::fs::read_to_string(path).map_err(|source| fs_error(path, source))?;
            if let Some(fields) = markdown_frontmatter_fields(&content, path)? {
                metadata.push(AuditMetadata {
                    path: relative.to_path_buf(),
                    fields,
                });
            }
        }
        Some("gemini-extension.json") => {
            let content = std::fs::read_to_string(path).map_err(|source| fs_error(path, source))?;
            let value: JsonValue = serde_json::from_str(&content).map_err(|error| {
                SkillsMgrError::InvalidArtifact {
                    path: path.to_path_buf(),
                    reason: error.to_string(),
                }
            })?;
            if let Some(object) = value.as_object() {
                let fields = object
                    .iter()
                    .filter_map(|(key, value)| scalar_field(key, value))
                    .collect::<BTreeMap<_, _>>();
                metadata.push(AuditMetadata {
                    path: relative.to_path_buf(),
                    fields,
                });
            }
        }
        _ if is_yaml_file(path) => {
            let content = std::fs::read_to_string(path).map_err(|source| fs_error(path, source))?;
            let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(&content) else {
                return Ok(());
            };
            let Some(mapping) = value.as_mapping() else {
                return Ok(());
            };
            let fields = mapping
                .iter()
                .filter_map(|(key, value)| {
                    Some((
                        key.as_str()?.to_string(),
                        value
                            .as_str()
                            .map_or_else(|| format!("{value:?}"), str::to_string),
                    ))
                })
                .collect::<BTreeMap<_, _>>();
            if fields.contains_key("command") || fields.contains_key("commands") {
                metadata.push(AuditMetadata {
                    path: relative.to_path_buf(),
                    fields,
                });
            }
        }
        _ => {}
    }

    Ok(())
}

fn collect_warnings(path: &Path, relative: &Path, warnings: &mut Vec<AuditWarning>) -> Result<()> {
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");

    if matches!(
        extension,
        "sh" | "bash" | "zsh" | "fish" | "ps1" | "bat" | "cmd"
    ) || matches!(filename, "package.json" | "Makefile" | "justfile")
    {
        warnings.push(AuditWarning {
            path: relative.to_path_buf(),
            kind: AuditWarningKind::ExecutableCommand,
            message: "file may define executable commands".to_string(),
        });
    }

    if filename == "gemini-extension.json"
        || filename.eq_ignore_ascii_case("mcp.json")
        || relative
            .components()
            .any(|component| component.as_os_str() == ".mcp")
    {
        let content = std::fs::read_to_string(path).unwrap_or_default();
        if filename.eq_ignore_ascii_case("mcp.json") || content.contains("\"mcp") {
            warnings.push(AuditWarning {
                path: relative.to_path_buf(),
                kind: AuditWarningKind::McpConfig,
                message: "file may configure MCP servers".to_string(),
            });
        }
    }

    Ok(())
}

fn markdown_frontmatter_fields(
    content: &str,
    path: &Path,
) -> Result<Option<BTreeMap<String, String>>> {
    let Some(rest) = content.strip_prefix("---\n") else {
        return Ok(None);
    };
    let Some((yaml, _body)) = rest.split_once("\n---") else {
        return Err(SkillsMgrError::InvalidArtifact {
            path: path.to_path_buf(),
            reason: "frontmatter starts with --- but has no closing ---".to_string(),
        });
    };

    let value: serde_yaml::Value =
        serde_yaml::from_str(yaml).map_err(|error| SkillsMgrError::InvalidArtifact {
            path: path.to_path_buf(),
            reason: error.to_string(),
        })?;
    let fields = value
        .as_mapping()
        .into_iter()
        .flat_map(|mapping| mapping.iter())
        .filter_map(|(key, value)| {
            Some((
                key.as_str()?.to_string(),
                value
                    .as_str()
                    .map_or_else(|| format!("{value:?}"), str::to_string),
            ))
        })
        .collect();

    Ok(Some(fields))
}

fn scalar_field(key: &str, value: &JsonValue) -> Option<(String, String)> {
    match value {
        JsonValue::String(value) => Some((key.to_string(), value.clone())),
        JsonValue::Bool(value) => Some((key.to_string(), value.to_string())),
        JsonValue::Number(value) => Some((key.to_string(), value.to_string())),
        _ => None,
    }
}

async fn copy_path(source: &Path, destination: &Path) -> Result<()> {
    let source = source.to_path_buf();
    let destination = destination.to_path_buf();
    let error_path = destination.clone();
    tokio::task::spawn_blocking(move || copy_path_blocking(&source, &destination))
        .await
        .map_err(|error| SkillsMgrError::Fs {
            path: error_path,
            source: std::io::Error::new(std::io::ErrorKind::Other, error),
        })?
}

fn copy_path_blocking(source: &Path, destination: &Path) -> Result<()> {
    let metadata = std::fs::symlink_metadata(source).map_err(|error| fs_error(source, error))?;
    if metadata.is_dir() {
        std::fs::create_dir_all(destination).map_err(|error| fs_error(destination, error))?;
        for entry in std::fs::read_dir(source).map_err(|error| fs_error(source, error))? {
            let entry = entry.map_err(|error| fs_error(source, error))?;
            copy_path_blocking(&entry.path(), &destination.join(entry.file_name()))?;
        }
    } else if metadata.is_file() {
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|error| fs_error(parent, error))?;
        }
        std::fs::copy(source, destination).map_err(|error| fs_error(destination, error))?;
    } else {
        return Err(SkillsMgrError::InvalidArtifact {
            path: source.to_path_buf(),
            reason: "symlinks and special files are not imported".to_string(),
        });
    }

    Ok(())
}

fn is_github_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("https://github.com/")
        || lower.starts_with("http://github.com/")
        || lower.starts_with("git@github.com:")
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

fn git_error(source: &str, error: impl std::error::Error) -> SkillsMgrError {
    SkillsMgrError::Git {
        input: source.to_string(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use skillsmgr_core::{Scope, Target};
    use tempfile::tempdir;

    use super::*;

    fn targets() -> Vec<Target> {
        vec![
            Target::ClaudeCode {
                scope: Scope::Global,
            },
            Target::Gemini {
                scope: Scope::Global,
            },
            Target::Warp {
                scope: Scope::Global,
            },
        ]
    }

    #[tokio::test]
    async fn local_preview_stages_source_and_filters_targets() {
        let source = tempdir().unwrap();
        fs::write(
            source.path().join("SKILL.md"),
            "---\nname: demo\ndescription: Demo\n---\n# Demo\n",
        )
        .unwrap();

        let preview = preview_local_import(source.path(), &targets())
            .await
            .unwrap();

        assert_ne!(preview.stage.root(), source.path());
        assert!(preview.stage.root().join("SKILL.md").exists());
        assert_eq!(preview.candidates.len(), 1);
        assert_eq!(preview.candidates[0].artifact.kind, ArtifactKind::Skill);
        assert_eq!(
            preview.candidates[0].compatible_targets,
            vec![Target::ClaudeCode {
                scope: Scope::Global
            }]
        );
    }

    #[tokio::test]
    async fn local_preview_blocks_unknown_kind() {
        let source = tempdir().unwrap();
        fs::write(source.path().join("README.md"), "# Demo\n").unwrap();

        let error = preview_local_import(source.path(), &targets())
            .await
            .unwrap_err();

        assert!(matches!(error, SkillsMgrError::NoSupportedArtifacts { .. }));
    }

    #[tokio::test]
    async fn audit_reports_files_metadata_and_warnings() {
        let source = tempdir().unwrap();
        fs::write(
            source.path().join("SKILL.md"),
            "---\nname: demo\ndescription: Demo\n---\n# Demo\n",
        )
        .unwrap();
        fs::create_dir_all(source.path().join("scripts")).unwrap();
        fs::write(source.path().join("scripts").join("run.sh"), "echo hi\n").unwrap();

        let preview = preview_local_import(source.path(), &targets())
            .await
            .unwrap();

        assert!(preview
            .audit
            .files
            .iter()
            .any(|file| file.path == PathBuf::from("SKILL.md")));
        assert!(preview
            .audit
            .metadata
            .iter()
            .any(|metadata| metadata.fields.get("name") == Some(&"demo".to_string())));
        assert!(preview.audit.warnings.iter().any(|warning| {
            warning.kind == AuditWarningKind::ExecutableCommand
                && warning.path == PathBuf::from("scripts/run.sh")
        }));
    }

    #[test]
    fn github_candidate_records_resolved_commit_sha() {
        let source = tempdir().unwrap();
        let candidate = ArtifactCandidate {
            artifact: Artifact::new(
                "demo",
                "",
                None,
                ArtifactKind::Skill,
                Source::Local {
                    path: source.path().to_path_buf(),
                },
            ),
            root: source.path().to_path_buf(),
        };

        let candidate = import_candidate(
            candidate,
            &ImportSource::GitHub {
                url: "https://github.com/example/demo".to_string(),
            },
            Some("abc123"),
            &targets(),
        );

        assert_eq!(
            candidate.artifact.source,
            Source::GitHub {
                url: "https://github.com/example/demo".to_string(),
                rev: "abc123".to_string()
            }
        );
    }
}
