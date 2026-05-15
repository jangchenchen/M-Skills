use std::path::PathBuf;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, SkillsMgrError>;

#[derive(Debug, thiserror::Error)]
pub enum SkillsMgrError {
    #[error("artifact kind {kind:?} is not supported by target {target:?}")]
    UnsupportedKind { kind: ArtifactKind, target: Target },

    #[error("artifact conflict: {name} already exists at {path}")]
    Conflict { name: String, path: PathBuf },

    #[error("invalid artifact at {path}: {reason}")]
    InvalidArtifact { path: PathBuf, reason: String },

    #[error("filesystem error at {path}: {source}")]
    Fs {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{tool} adapter is read-only for {operation}")]
    ReadOnly {
        tool: &'static str,
        operation: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArtifactKind {
    Skill,
    Extension,
    Workflow,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Scope {
    Global,
    Project(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Target {
    ClaudeCode { scope: Scope },
    Codex { scope: Scope },
    Opencode { scope: Scope },
    Openclaw { scope: Scope },
    Hermes,
    Gemini { scope: Scope },
    Warp { scope: Scope },
    SharedGlobal,
}

impl Target {
    pub fn scope(&self) -> Option<&Scope> {
        match self {
            Target::ClaudeCode { scope }
            | Target::Codex { scope }
            | Target::Opencode { scope }
            | Target::Openclaw { scope }
            | Target::Gemini { scope }
            | Target::Warp { scope } => Some(scope),
            Target::Hermes | Target::SharedGlobal => None,
        }
    }

    pub fn tool_id(&self) -> &'static str {
        match self {
            Target::ClaudeCode { .. } => "claude-code",
            Target::Codex { .. } => "codex",
            Target::Opencode { .. } => "opencode",
            Target::Openclaw { .. } => "openclaw",
            Target::Hermes => "hermes",
            Target::Gemini { .. } => "gemini",
            Target::Warp { .. } => "warp",
            Target::SharedGlobal => "shared-global",
        }
    }

    pub fn supports_kind(&self, kind: ArtifactKind) -> bool {
        match self {
            Target::ClaudeCode { .. }
            | Target::Codex { .. }
            | Target::Opencode { .. }
            | Target::Openclaw { .. }
            | Target::Hermes
            | Target::SharedGlobal => kind == ArtifactKind::Skill,
            Target::Gemini { .. } => kind == ArtifactKind::Extension,
            Target::Warp { .. } => kind == ArtifactKind::Workflow,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    GitHub { url: String, rev: String },
    Local { path: PathBuf },
    Bundled,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub kind: ArtifactKind,
    pub source: Source,
}

impl Artifact {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        version: Option<String>,
        kind: ArtifactKind,
        source: Source,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: description.into(),
            version,
            kind,
            source,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    Enabled,
    Disabled,
    Broken { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Installation {
    pub id: Uuid,
    pub artifact_id: Uuid,
    pub target: Target,
    pub status: Status,
    pub on_disk_path: PathBuf,
    pub installed_at: DateTime<Utc>,
    pub installed_version: Option<String>,
}

impl Installation {
    pub fn enabled(artifact: &Artifact, target: Target, on_disk_path: impl Into<PathBuf>) -> Self {
        Self {
            id: Uuid::new_v4(),
            artifact_id: artifact.id,
            target,
            status: Status::Enabled,
            on_disk_path: on_disk_path.into(),
            installed_at: Utc::now(),
            installed_version: artifact.version.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdapterPresence {
    Available,
    Missing { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScannedInstallation {
    pub artifact: Artifact,
    pub installation: Installation,
}

#[async_trait]
pub trait ToolAdapter: Send + Sync {
    fn id(&self) -> &'static str;

    fn supported_kinds(&self) -> &'static [ArtifactKind];

    async fn scan(&self, scope: Scope) -> Result<Vec<ScannedInstallation>>;

    async fn install(&self, artifact: &Artifact, scope: Scope) -> Result<Installation>;

    async fn uninstall(&self, installation: &Installation) -> Result<()>;

    async fn enable(&self, installation: &Installation) -> Result<()>;

    async fn disable(&self, installation: &Installation) -> Result<()>;

    async fn detect(&self) -> AdapterPresence;
}

pub fn ensure_target_supports_kind(target: &Target, kind: ArtifactKind) -> Result<()> {
    if target.supports_kind(kind) {
        Ok(())
    } else {
        Err(SkillsMgrError::UnsupportedKind {
            kind,
            target: target.clone(),
        })
    }
}
