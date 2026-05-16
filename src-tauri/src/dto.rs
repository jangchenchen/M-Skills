use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use skillsmgr_core::{
    AdapterPresence, Artifact, ArtifactKind, Installation, Scope, ScannedInstallation, Source,
    SourceProvenance, Status, Target,
};
use skillsmgr_fetch::{
    AuditFile, AuditMetadata, AuditWarning, AuditWarningKind, ImportAudit, ImportPreview,
    ImportSource,
};
use skillsmgr_service::{AdapterStatus, ArtifactGroup, Inventory};

// ── Inventory ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryDto {
    pub groups: Vec<ArtifactGroupDto>,
    pub adapters: Vec<AdapterStatusDto>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactGroupDto {
    pub name: String,
    pub kind: String,
    pub description: String,
    pub version: Option<String>,
    pub installations: Vec<ScannedInstallationDto>,
    pub also_visible_to: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannedInstallationDto {
    pub artifact: ArtifactDto,
    pub installation: InstallationDto,
    pub provenance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub kind: String,
    pub source: SourceDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SourceDto {
    GitHub { url: String, rev: String },
    Local { path: String },
    Bundled,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallationDto {
    pub id: String,
    pub artifact_id: String,
    pub target: TargetDto,
    pub status: String,
    pub on_disk_path: String,
    pub installed_at: String,
    pub installed_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetDto {
    pub tool: String,
    pub scope: ScopeDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ScopeDto {
    Global,
    Project { path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterStatusDto {
    pub adapter_id: String,
    pub presence: PresenceDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PresenceDto {
    Available,
    Missing { reason: String },
}

// ── Import preview ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreviewDto {
    pub source: ImportSourceDto,
    pub commit_sha: Option<String>,
    pub candidates: Vec<ImportCandidateDto>,
    pub audit: ImportAuditDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ImportSourceDto {
    Local { path: String },
    GitHub { url: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCandidateDto {
    pub index: usize,
    pub artifact: ArtifactDto,
    pub compatible_targets: Vec<TargetDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportAuditDto {
    pub files: Vec<AuditFileDto>,
    pub metadata: Vec<AuditMetadataDto>,
    pub warnings: Vec<AuditWarningDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditFileDto {
    pub path: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditMetadataDto {
    pub path: String,
    pub fields: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditWarningDto {
    pub path: String,
    pub kind: String,
    pub message: String,
}

// ── From impls (domain → DTO) ─────────────────────────────────────────────────

impl From<&Inventory> for InventoryDto {
    fn from(inv: &Inventory) -> Self {
        InventoryDto {
            groups: inv.groups.iter().map(ArtifactGroupDto::from).collect(),
            adapters: inv.adapters.iter().map(AdapterStatusDto::from).collect(),
            errors: inv
                .errors
                .iter()
                .map(|e| format!("{}: {}", e.adapter_id, e.message))
                .collect(),
        }
    }
}

impl From<&ArtifactGroup> for ArtifactGroupDto {
    fn from(g: &ArtifactGroup) -> Self {
        ArtifactGroupDto {
            name: g.name.clone(),
            kind: kind_str(g.kind),
            description: g.description.clone(),
            version: g.version.clone(),
            installations: g.installations.iter().map(ScannedInstallationDto::from).collect(),
            also_visible_to: g.also_visible_to.clone(),
        }
    }
}

impl From<&ScannedInstallation> for ScannedInstallationDto {
    fn from(si: &ScannedInstallation) -> Self {
        ScannedInstallationDto {
            artifact: ArtifactDto::from(&si.artifact),
            installation: InstallationDto::from(&si.installation),
            provenance: match &si.provenance {
                SourceProvenance::Owned => "owned".to_string(),
                SourceProvenance::Shared { from_tool } => format!("shared:{from_tool}"),
            },
        }
    }
}

impl From<&Artifact> for ArtifactDto {
    fn from(a: &Artifact) -> Self {
        ArtifactDto {
            id: a.id.to_string(),
            name: a.name.clone(),
            description: a.description.clone(),
            version: a.version.clone(),
            kind: kind_str(a.kind),
            source: SourceDto::from(&a.source),
        }
    }
}

impl From<&Source> for SourceDto {
    fn from(s: &Source) -> Self {
        match s {
            Source::GitHub { url, rev } => SourceDto::GitHub {
                url: url.clone(),
                rev: rev.clone(),
            },
            Source::Local { path } => SourceDto::Local {
                path: path.to_string_lossy().to_string(),
            },
            Source::Bundled => SourceDto::Bundled,
            Source::Unknown => SourceDto::Unknown,
        }
    }
}

impl From<&Installation> for InstallationDto {
    fn from(i: &Installation) -> Self {
        InstallationDto {
            id: i.id.to_string(),
            artifact_id: i.artifact_id.to_string(),
            target: TargetDto::from(&i.target),
            status: match &i.status {
                Status::Enabled => "enabled".to_string(),
                Status::Disabled => "disabled".to_string(),
                Status::Broken { reason } => format!("broken:{reason}"),
            },
            on_disk_path: i.on_disk_path.to_string_lossy().to_string(),
            installed_at: i.installed_at.to_rfc3339(),
            installed_version: i.installed_version.clone(),
        }
    }
}

impl From<&Target> for TargetDto {
    fn from(t: &Target) -> Self {
        let tool = t.tool_id().to_string();
        let scope = match t.scope() {
            Some(Scope::Global) | None => ScopeDto::Global,
            Some(Scope::Project(p)) => ScopeDto::Project {
                path: p.to_string_lossy().to_string(),
            },
        };
        TargetDto { tool, scope }
    }
}

impl TryFrom<TargetDto> for Target {
    type Error = String;

    fn try_from(dto: TargetDto) -> Result<Self, Self::Error> {
        let scope = match dto.scope {
            ScopeDto::Global => Scope::Global,
            ScopeDto::Project { path } => Scope::Project(PathBuf::from(path)),
        };
        match dto.tool.as_str() {
            "claude-code" => Ok(Target::ClaudeCode { scope }),
            "codex" => Ok(Target::Codex { scope }),
            "opencode" => Ok(Target::Opencode { scope }),
            "openclaw" => Ok(Target::Openclaw { scope }),
            "hermes" => Ok(Target::Hermes),
            "gemini" => Ok(Target::Gemini { scope }),
            "warp" => Ok(Target::Warp { scope }),
            "shared-global" => Ok(Target::SharedGlobal),
            other => Err(format!("unknown tool: {other}")),
        }
    }
}

impl From<&AdapterStatus> for AdapterStatusDto {
    fn from(s: &AdapterStatus) -> Self {
        AdapterStatusDto {
            adapter_id: s.adapter_id.clone(),
            presence: match &s.presence {
                AdapterPresence::Available => PresenceDto::Available,
                AdapterPresence::Missing { reason } => PresenceDto::Missing {
                    reason: reason.clone(),
                },
            },
        }
    }
}

impl From<&ImportPreview> for ImportPreviewDto {
    fn from(p: &ImportPreview) -> Self {
        ImportPreviewDto {
            source: ImportSourceDto::from(&p.source),
            commit_sha: p.stage.resolved_commit_sha.clone(),
            candidates: p
                .candidates
                .iter()
                .enumerate()
                .map(|(i, c)| ImportCandidateDto {
                    index: i,
                    artifact: ArtifactDto::from(&c.artifact),
                    compatible_targets: c.compatible_targets.iter().map(TargetDto::from).collect(),
                })
                .collect(),
            audit: ImportAuditDto::from(&p.audit),
        }
    }
}

impl From<&ImportSource> for ImportSourceDto {
    fn from(s: &ImportSource) -> Self {
        match s {
            ImportSource::Local { path } => ImportSourceDto::Local {
                path: path.to_string_lossy().to_string(),
            },
            ImportSource::GitHub { url } => ImportSourceDto::GitHub { url: url.clone() },
        }
    }
}

impl From<&ImportAudit> for ImportAuditDto {
    fn from(a: &ImportAudit) -> Self {
        ImportAuditDto {
            files: a.files.iter().map(AuditFileDto::from).collect(),
            metadata: a.metadata.iter().map(AuditMetadataDto::from).collect(),
            warnings: a.warnings.iter().map(AuditWarningDto::from).collect(),
        }
    }
}

impl From<&AuditFile> for AuditFileDto {
    fn from(f: &AuditFile) -> Self {
        AuditFileDto {
            path: f.path.to_string_lossy().to_string(),
            size_bytes: f.size_bytes,
        }
    }
}

impl From<&AuditMetadata> for AuditMetadataDto {
    fn from(m: &AuditMetadata) -> Self {
        AuditMetadataDto {
            path: m.path.to_string_lossy().to_string(),
            fields: m.fields.clone(),
        }
    }
}

impl From<&AuditWarning> for AuditWarningDto {
    fn from(w: &AuditWarning) -> Self {
        AuditWarningDto {
            path: w.path.to_string_lossy().to_string(),
            kind: match w.kind {
                AuditWarningKind::ExecutableCommand => "ExecutableCommand".to_string(),
                AuditWarningKind::McpConfig => "McpConfig".to_string(),
            },
            message: w.message.clone(),
        }
    }
}

fn kind_str(k: ArtifactKind) -> String {
    match k {
        ArtifactKind::Skill => "Skill".to_string(),
        ArtifactKind::Extension => "Extension".to_string(),
        ArtifactKind::Workflow => "Workflow".to_string(),
    }
}
