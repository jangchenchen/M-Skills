use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use skillsmgr_core::{
    AdapterPresence, Artifact, ArtifactKind, Capability, Installation, ScannedInstallation, Scope,
    SkillsMgrError, Source, SourceProvenance, Status, Target,
};
use skillsmgr_fetch::{
    AuditFile, AuditMetadata, AuditSeverity, AuditWarning, AuditWarningKind, ImportAudit,
    ImportPreview, ImportSource,
};
use skillsmgr_service::{AdapterStatus, ArtifactGroup, Inventory};
use skillsmgr_translate::{
    ProviderKind, RegistrySkillSummary, TranslateConfig, TranslateOutcome, TranslationValidation,
};

use crate::compatibility::{CompatibilityReview, CompatibilityRiskLevel, CompatibilityStatus};

// ── Error ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorDto {
    pub code: String,
    pub params: HashMap<String, String>,
}

impl ErrorDto {
    pub fn internal(msg: impl ToString) -> Self {
        ErrorDto {
            code: "internal".into(),
            params: [("message".into(), msg.to_string())].into(),
        }
    }
}

impl From<SkillsMgrError> for ErrorDto {
    fn from(e: SkillsMgrError) -> Self {
        let mut params: HashMap<String, String> = HashMap::new();
        let code: &str = match &e {
            SkillsMgrError::UnsupportedKind { kind, target } => {
                params.insert("kind".into(), format!("{kind:?}"));
                params.insert("target".into(), target.tool_id().into());
                "unsupportedKind"
            }
            SkillsMgrError::UnsupportedTarget { adapter_id, target } => {
                params.insert("adapterId".into(), adapter_id.clone());
                params.insert("target".into(), target.tool_id().into());
                "unsupportedTarget"
            }
            SkillsMgrError::Conflict { name, path } => {
                params.insert("name".into(), name.clone());
                params.insert("path".into(), path.to_string_lossy().into());
                "conflict"
            }
            SkillsMgrError::SourceConflict {
                name,
                existing_source,
                new_source,
            } => {
                params.insert("name".into(), name.clone());
                params.insert("existingSource".into(), format!("{existing_source:?}"));
                params.insert("newSource".into(), format!("{new_source:?}"));
                "sourceConflict"
            }
            SkillsMgrError::InvalidArtifact { path, reason } => {
                params.insert("path".into(), path.to_string_lossy().into());
                params.insert("reason".into(), reason.clone());
                "invalidArtifact"
            }
            SkillsMgrError::UnsupportedImportSource { input } => {
                params.insert("input".into(), input.clone());
                "unsupportedImportSource"
            }
            SkillsMgrError::NoSupportedArtifacts { path } => {
                params.insert("path".into(), path.to_string_lossy().into());
                "noSupportedArtifacts"
            }
            SkillsMgrError::UnsafePath { path, reason } => {
                params.insert("path".into(), path.to_string_lossy().into());
                params.insert("reason".into(), reason.clone());
                "unsafePath"
            }
            SkillsMgrError::Fs { path, source } => {
                params.insert("path".into(), path.to_string_lossy().into());
                params.insert("message".into(), source.to_string());
                "fs"
            }
            SkillsMgrError::Git { input, message } => {
                params.insert("input".into(), input.clone());
                params.insert("message".into(), message.clone());
                "git"
            }
            SkillsMgrError::Registry(msg) => {
                params.insert("message".into(), msg.clone());
                "registry"
            }
            SkillsMgrError::ReadOnly { tool, operation } => {
                params.insert("tool".into(), tool.to_string());
                params.insert("operation".into(), operation.to_string());
                "readOnly"
            }
            SkillsMgrError::TranslateProvider {
                kind,
                status,
                message,
            } => {
                params.insert("kind".into(), kind.clone());
                if let Some(s) = status {
                    params.insert("status".into(), s.to_string());
                }
                params.insert("message".into(), message.clone());
                "translateProvider"
            }
            SkillsMgrError::TranslateConfig { reason } => {
                params.insert("reason".into(), reason.clone());
                "translateConfig"
            }
            SkillsMgrError::Keyring { message } => {
                params.insert("message".into(), message.clone());
                "keyring"
            }
        };
        ErrorDto {
            code: code.into(),
            params,
        }
    }
}

// ── Skill draft (Issue 007 Batch 2) ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineageDto {
    /// "fork" or "adaptation".
    pub source_kind: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub source_tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub source_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub source_url: Option<String>,
    pub source_hash: String,
    pub parent_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NameConflictDto {
    pub existing_path: String,
    pub target_tool: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillDraftPreviewDto {
    pub original_name: String,
    pub original_content: String,
    pub adapted_name: String,
    pub adapted_description: String,
    pub adapted_version: Option<String>,
    pub adapted_content: String,
    pub target: TargetDto,
    pub lineage: LineageDto,
    pub compatibility_reviews: Vec<CompatibilityReviewDto>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name_conflict: Option<NameConflictDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForkPreviewRequestDto {
    pub artifact: ArtifactDto,
    pub target: TargetDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveCustomSkillEditRequestDto {
    pub content: String,
    pub target: TargetDto,
    pub lineage: LineageDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmDraftInstallRequestDto {
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub content: String,
    pub target: TargetDto,
    pub lineage: LineageDto,
}

// ── LLM rewrite (Issue 007 Batch 3) ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RewriteSkillRequestDto {
    pub artifact: ArtifactDto,
    /// One of `RewriteMode::as_id()`.
    pub mode: String,
    pub user_instruction: String,
    pub locale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RewriteSkillOutcomeDto {
    pub draft_body: String,
    pub summary: String,
    pub notes: Vec<String>,
    pub provider_kind: String,
    pub model: String,
    pub compatibility_reviews: Vec<CompatibilityReviewDto>,
}

// ── Smart Add intent (Issue 011) ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillIntentOutcomeDto {
    pub is_install_request: bool,
    pub search_query: Option<String>,
    pub reason: Option<String>,
    pub provider_kind: String,
    pub model: String,
}

// ── AI skill summary (auto-generated post-install) ───────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSummaryRequestDto {
    pub artifact: ArtifactDto,
    pub locale: String,
    #[serde(default)]
    pub force_refresh: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSummaryDto {
    pub commands: Vec<String>,
    pub capabilities: String,
    pub use_cases: Vec<String>,
    pub examples: Vec<String>,
    pub locale: String,
    pub provider_kind: String,
    pub model: String,
    pub generated_at: String,
    /// "hit" when served from the registry cache, "miss" when freshly generated.
    pub cache_status: String,
}

impl SkillSummaryDto {
    pub fn from_record(
        record: &RegistrySkillSummary,
        provider_kind: &str,
        cache_status: &str,
    ) -> std::result::Result<Self, ErrorDto> {
        let outcome: crate::summary::SkillSummaryOutcome =
            serde_json::from_str(&record.summary_json).map_err(|e| ErrorDto {
                code: "summarizeParseFailed".into(),
                params: [("reason".into(), e.to_string())].into(),
            })?;
        Ok(SkillSummaryDto::from_parts(
            &outcome,
            &record.locale,
            provider_kind,
            &record.model,
            &record.generated_at.to_rfc3339(),
            cache_status,
        ))
    }

    pub fn from_parts(
        outcome: &crate::summary::SkillSummaryOutcome,
        locale: &str,
        provider_kind: &str,
        model: &str,
        generated_at: &str,
        cache_status: &str,
    ) -> Self {
        SkillSummaryDto {
            commands: outcome.commands.clone(),
            capabilities: outcome.capabilities.clone(),
            use_cases: outcome.use_cases.clone(),
            examples: outcome.examples.clone(),
            locale: locale.to_string(),
            provider_kind: provider_kind.to_string(),
            model: model.to_string(),
            generated_at: generated_at.to_string(),
            cache_status: cache_status.to_string(),
        }
    }
}

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
    pub body: Option<String>,
    pub version: Option<String>,
    pub capabilities: Vec<CapabilityDto>,
    pub installations: Vec<ScannedInstallationDto>,
    pub also_visible_to: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityDto {
    pub name: String,
    pub description: String,
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
    pub body: Option<String>,
    pub version: Option<String>,
    pub kind: String,
    pub source: SourceDto,
    pub capabilities: Vec<CapabilityDto>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub lineage: Option<LineageDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SourceDto {
    GitHub { url: String, rev: String },
    Url { url: String },
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
    RawUrl { url: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCandidateDto {
    pub index: usize,
    pub artifact: ArtifactDto,
    pub compatible_targets: Vec<TargetDto>,
    pub compatibility_reviews: Vec<CompatibilityReviewDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityReviewDto {
    pub target: TargetDto,
    pub status: CompatibilityStatusDto,
    pub risk_level: CompatibilityRiskLevelDto,
    pub summary: String,
    pub reasons: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CompatibilityStatusDto {
    Compatible,
    Warning,
    Incompatible,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum CompatibilityRiskLevelDto {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportAuditDto {
    pub files: Vec<AuditFileDto>,
    pub metadata: Vec<AuditMetadataDto>,
    pub warnings: Vec<AuditWarningDto>,
    pub risk_level: AuditSeverityDto,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AuditSeverityDto {
    Low,
    Medium,
    High,
}

impl From<AuditSeverity> for AuditSeverityDto {
    fn from(s: AuditSeverity) -> Self {
        match s {
            AuditSeverity::Low => AuditSeverityDto::Low,
            AuditSeverity::Medium => AuditSeverityDto::Medium,
            AuditSeverity::High => AuditSeverityDto::High,
        }
    }
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
    pub severity: AuditSeverityDto,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallOutcomeDto {
    pub target: TargetDto,
    pub ok: bool,
    pub installation: Option<InstallationDto>,
    pub error: Option<ErrorDto>,
}

// ── Review ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReviewRatingDto {
    Safe,
    Caution,
    Conflict,
}

impl From<crate::review::ReviewRating> for ReviewRatingDto {
    fn from(r: crate::review::ReviewRating) -> Self {
        match r {
            crate::review::ReviewRating::Safe => ReviewRatingDto::Safe,
            crate::review::ReviewRating::Caution => ReviewRatingDto::Caution,
            crate::review::ReviewRating::Conflict => ReviewRatingDto::Conflict,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewConflictDto {
    pub name: String,
    pub kind: String,
    pub tool: String,
    pub reason_kind: String,
    pub reason: String,
}

impl From<crate::review::ReviewConflict> for ReviewConflictDto {
    fn from(c: crate::review::ReviewConflict) -> Self {
        ReviewConflictDto {
            name: c.name,
            kind: c.kind,
            tool: c.tool,
            reason_kind: c.reason_kind,
            reason: c.reason,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewOutcomeDto {
    pub rating: ReviewRatingDto,
    pub summary: String,
    pub skill_purpose: String,
    pub conflicts: Vec<ReviewConflictDto>,
    pub provider_kind: String,
    pub model: String,
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
            body: g.body.clone(),
            version: g.version.clone(),
            capabilities: g.capabilities.iter().map(CapabilityDto::from).collect(),
            installations: g
                .installations
                .iter()
                .map(ScannedInstallationDto::from)
                .collect(),
            also_visible_to: g.also_visible_to.clone(),
        }
    }
}

impl From<&Capability> for CapabilityDto {
    fn from(c: &Capability) -> Self {
        CapabilityDto {
            name: c.name.clone(),
            description: c.description.clone(),
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
        let lineage = read_lineage_sidecar(&a.source);
        ArtifactDto {
            id: a.id.to_string(),
            name: a.name.clone(),
            description: a.description.clone(),
            body: a.body.clone(),
            version: a.version.clone(),
            kind: kind_str(a.kind),
            source: SourceDto::from(&a.source),
            capabilities: a.capabilities.iter().map(CapabilityDto::from).collect(),
            lineage,
        }
    }
}

fn read_lineage_sidecar(source: &Source) -> Option<LineageDto> {
    let Source::Local { path } = source else {
        return None;
    };
    let sidecar = path.join(".m-skills.json");
    let bytes = std::fs::read(&sidecar).ok()?;
    serde_json::from_slice::<LineageDto>(&bytes).ok()
}

impl From<&Source> for SourceDto {
    fn from(s: &Source) -> Self {
        match s {
            Source::GitHub { url, rev } => SourceDto::GitHub {
                url: url.clone(),
                rev: rev.clone(),
            },
            Source::Url { url } => SourceDto::Url { url: url.clone() },
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
                    compatibility_reviews: crate::compatibility::review_for_targets(
                        &c.artifact,
                        &review_targets(),
                    )
                    .iter()
                    .map(CompatibilityReviewDto::from)
                    .collect(),
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
            ImportSource::RawUrl { url } => ImportSourceDto::RawUrl { url: url.clone() },
        }
    }
}

impl From<&ImportAudit> for ImportAuditDto {
    fn from(a: &ImportAudit) -> Self {
        ImportAuditDto {
            files: a.files.iter().map(AuditFileDto::from).collect(),
            metadata: a.metadata.iter().map(AuditMetadataDto::from).collect(),
            warnings: a.warnings.iter().map(AuditWarningDto::from).collect(),
            risk_level: a.risk_level.into(),
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
                AuditWarningKind::DangerousShellPattern => "DangerousShellPattern".to_string(),
                AuditWarningKind::PromptInjection => "PromptInjection".to_string(),
                AuditWarningKind::LargePayload => "LargePayload".to_string(),
            },
            severity: w.severity.into(),
            message: w.message.clone(),
        }
    }
}

impl From<&CompatibilityReview> for CompatibilityReviewDto {
    fn from(r: &CompatibilityReview) -> Self {
        CompatibilityReviewDto {
            target: TargetDto::from(&r.target),
            status: r.status.into(),
            risk_level: r.risk_level.into(),
            summary: r.summary.clone(),
            reasons: r.reasons.clone(),
            warnings: r.warnings.clone(),
        }
    }
}

impl From<CompatibilityStatus> for CompatibilityStatusDto {
    fn from(s: CompatibilityStatus) -> Self {
        match s {
            CompatibilityStatus::Compatible => CompatibilityStatusDto::Compatible,
            CompatibilityStatus::Warning => CompatibilityStatusDto::Warning,
            CompatibilityStatus::Incompatible => CompatibilityStatusDto::Incompatible,
        }
    }
}

impl From<CompatibilityRiskLevel> for CompatibilityRiskLevelDto {
    fn from(r: CompatibilityRiskLevel) -> Self {
        match r {
            CompatibilityRiskLevel::Low => CompatibilityRiskLevelDto::Low,
            CompatibilityRiskLevel::Medium => CompatibilityRiskLevelDto::Medium,
            CompatibilityRiskLevel::High => CompatibilityRiskLevelDto::High,
        }
    }
}

fn review_targets() -> Vec<Target> {
    vec![
        Target::ClaudeCode {
            scope: Scope::Global,
        },
        Target::Codex {
            scope: Scope::Global,
        },
        Target::Gemini {
            scope: Scope::Global,
        },
    ]
}

// ── Translate config ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslateOutcomeDto {
    pub text: String,
    pub locale: String,
    pub field: String,
    pub source_sha256: String,
    pub cache_status: String,
    pub provider_kind: String,
    pub used_fallback: bool,
    pub validation: TranslationValidation,
}

impl From<TranslateOutcome> for TranslateOutcomeDto {
    fn from(outcome: TranslateOutcome) -> Self {
        TranslateOutcomeDto {
            text: outcome.text,
            locale: outcome.locale,
            field: outcome.field,
            source_sha256: outcome.source_sha256,
            cache_status: outcome.cache_status.as_id().to_string(),
            provider_kind: outcome.provider_kind.to_string(),
            used_fallback: outcome.used_fallback,
            validation: outcome.validation,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslateConfigDto {
    pub provider_kind: String,
    pub base_url: String,
    pub model: String,
    pub fallback_model: Option<String>,
    pub timeout_ms: u64,
    pub max_retries: u32,
    pub api_key_present: bool,
}

impl TranslateConfigDto {
    pub fn from_parts(config: &TranslateConfig, api_key_present: bool) -> Self {
        TranslateConfigDto {
            provider_kind: config.provider_kind.as_id().to_string(),
            base_url: config.base_url.clone(),
            model: config.model.clone(),
            fallback_model: config.fallback_model.clone(),
            timeout_ms: config.timeout_ms,
            max_retries: config.max_retries,
            api_key_present,
        }
    }

    pub fn to_config(&self) -> Result<TranslateConfig, ErrorDto> {
        let provider_kind = match self.provider_kind.as_str() {
            "passthrough" => ProviderKind::Passthrough,
            "openai-compat" => ProviderKind::OpenAiCompat,
            other => {
                return Err(ErrorDto {
                    code: "translateConfig".into(),
                    params: [("reason".into(), format!("unknown provider: {other}"))].into(),
                })
            }
        };
        Ok(TranslateConfig {
            provider_kind,
            base_url: self.base_url.clone(),
            model: self.model.clone(),
            fallback_model: self
                .fallback_model
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            timeout_ms: self.timeout_ms,
            max_retries: self.max_retries,
        })
    }
}

fn kind_str(k: ArtifactKind) -> String {
    match k {
        ArtifactKind::Skill => "Skill".to_string(),
        ArtifactKind::Extension => "Extension".to_string(),
        ArtifactKind::Workflow => "Workflow".to_string(),
    }
}
