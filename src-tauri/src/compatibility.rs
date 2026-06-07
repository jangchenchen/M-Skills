use skillsmgr_core::{Artifact, ArtifactKind, Target};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatibilityStatus {
    Compatible,
    Warning,
    Incompatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CompatibilityRiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatibilityReview {
    pub target: Target,
    pub status: CompatibilityStatus,
    pub risk_level: CompatibilityRiskLevel,
    pub summary: String,
    pub reasons: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn review_for_targets(artifact: &Artifact, targets: &[Target]) -> Vec<CompatibilityReview> {
    targets
        .iter()
        .cloned()
        .map(|target| review_for_target(artifact, target))
        .collect()
}

pub fn review_for_target(artifact: &Artifact, target: Target) -> CompatibilityReview {
    let mut reasons = Vec::new();
    let mut warnings = Vec::new();
    let mut risk_level = CompatibilityRiskLevel::Low;

    let kind_supported = target.supports_kind(artifact.kind);
    if kind_supported {
        reasons.push(kind_support_reason(artifact.kind, &target).to_string());
    } else {
        reasons.push(kind_mismatch_reason(artifact.kind, &target).to_string());
    }

    if artifact.name.trim().is_empty() {
        warnings.push("Artifact name is empty; the target tool may not load it reliably.".into());
        risk_level = risk_level.max(CompatibilityRiskLevel::Medium);
    } else if artifact.kind == ArtifactKind::Skill && !is_skill_name_portable(&artifact.name) {
        warnings.push(
            "Skill name is not portable; use lowercase letters, numbers, and hyphens only.".into(),
        );
        risk_level = risk_level.max(CompatibilityRiskLevel::Medium);
    }

    if artifact.description.trim().is_empty() {
        warnings.push(
            "Description is missing; agents may not know when this artifact should be used.".into(),
        );
        risk_level = risk_level.max(CompatibilityRiskLevel::Medium);
    }

    if let Some(body) = &artifact.body {
        collect_body_warnings(body, &target, &mut warnings, &mut risk_level);
    }

    if artifact.kind == ArtifactKind::Extension && artifact.capabilities.is_empty() {
        warnings.push("Gemini extension has no discovered commands or capabilities.".into());
        risk_level = risk_level.max(CompatibilityRiskLevel::Low);
    }

    let status = if !kind_supported {
        CompatibilityStatus::Incompatible
    } else if warnings.is_empty() {
        CompatibilityStatus::Compatible
    } else {
        CompatibilityStatus::Warning
    };
    let summary = summary_for(status, risk_level, artifact.kind, &target);

    CompatibilityReview {
        target,
        status,
        risk_level,
        summary,
        reasons,
        warnings,
    }
}

fn collect_body_warnings(
    body: &str,
    target: &Target,
    warnings: &mut Vec<String>,
    risk_level: &mut CompatibilityRiskLevel,
) {
    let lower = body.to_ascii_lowercase();

    if body.contains("allowed-tools") {
        if matches!(target, Target::Codex { .. }) {
            warnings.push(
                "Uses Claude Code allowed-tools; Codex may not enforce those tool limits.".into(),
            );
            *risk_level = (*risk_level).max(CompatibilityRiskLevel::Medium);
        } else {
            warnings.push("Uses Claude Code allowed-tools; verify the allowed tool list.".into());
            *risk_level = (*risk_level).max(CompatibilityRiskLevel::Low);
        }
    }

    let claude_terms = [
        "Claude Code",
        "TodoWrite",
        "Task tool",
        "Grep tool",
        "Glob tool",
        "Read tool",
        "Write tool",
        "Edit tool",
        "MultiEdit",
    ];
    if matches!(target, Target::Codex { .. } | Target::Opencode { .. })
        && claude_terms.iter().any(|term| body.contains(term))
    {
        warnings.push(format!(
            "Mentions Claude Code-specific tools or workflow; review before using in {}.",
            target.tool_id()
        ));
        *risk_level = (*risk_level).max(CompatibilityRiskLevel::Medium);
    }

    struct RiskCategory {
        patterns: &'static [&'static str],
        message: &'static str,
    }
    let risk_categories: &[RiskCategory] = &[
        RiskCategory {
            patterns: &["| sh", "| bash"],
            message: "Downloads and directly executes remote code — if the source is compromised, your machine runs whatever the attacker sends.",
        },
        RiskCategory {
            patterns: &["rm -rf"],
            message: "Uses destructive file deletion commands that could permanently erase your files or directories.",
        },
        RiskCategory {
            patterns: &["sudo "],
            message: "Runs commands as administrator, bypassing system security protections.",
        },
        RiskCategory {
            patterns: &["curl ", "wget "],
            message: "Downloads content from the internet, which the AI may then execute or write to disk.",
        },
        RiskCategory {
            patterns: &["chmod ", "chown "],
            message: "Modifies file permissions or ownership — may create new executable programs on your machine.",
        },
        RiskCategory {
            patterns: &[".bashrc", ".zshrc"],
            message: "Modifies your shell configuration — changes run automatically every time you open a terminal.",
        },
        RiskCategory {
            patterns: &[".ssh/"],
            message: "Touches your SSH keys — these control secure access to remote servers and repositories.",
        },
        RiskCategory {
            patterns: &["api_key", "secret", "password"],
            message: "References credentials or secrets — the AI could accidentally expose or overwrite sensitive data.",
        },
    ];
    for category in risk_categories {
        if category
            .patterns
            .iter()
            .any(|p| lower.contains(&p.to_ascii_lowercase()))
        {
            warnings.push(category.message.into());
            *risk_level = (*risk_level).max(CompatibilityRiskLevel::High);
        }
    }
}

fn is_skill_name_portable(name: &str) -> bool {
    if name.len() > 64 || name.starts_with('-') || name.ends_with('-') {
        return false;
    }
    name.bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn kind_support_reason(kind: ArtifactKind, target: &Target) -> &'static str {
    match (kind, target) {
        (ArtifactKind::Skill, Target::ClaudeCode { .. }) => {
            "SKILL.md Skill is compatible with Claude Code."
        }
        (ArtifactKind::Skill, Target::Codex { .. }) => "SKILL.md Skill is compatible with Codex.",
        (ArtifactKind::Skill, _) => "SKILL.md Skill is compatible with this skill target.",
        (ArtifactKind::Extension, Target::Gemini { .. }) => {
            "gemini-extension.json Extension is compatible with Gemini."
        }
        (ArtifactKind::Workflow, Target::Warp { .. }) => {
            "Workflow artifact is compatible with Warp."
        }
        _ => "Artifact kind is compatible with the selected target.",
    }
}

fn kind_mismatch_reason(kind: ArtifactKind, target: &Target) -> &'static str {
    match (kind, target) {
        (ArtifactKind::Skill, Target::Gemini { .. }) => {
            "Plain SKILL.md Skills are not Gemini extensions; Gemini expects gemini-extension.json."
        }
        (ArtifactKind::Extension, Target::ClaudeCode { .. } | Target::Codex { .. }) => {
            "Gemini extensions should not be installed into SKILL.md-based tools."
        }
        (ArtifactKind::Workflow, Target::ClaudeCode { .. } | Target::Codex { .. }) => {
            "Workflows are command templates, not agent Skills."
        }
        _ => "Artifact kind is not supported by this target.",
    }
}

fn summary_for(
    status: CompatibilityStatus,
    risk_level: CompatibilityRiskLevel,
    kind: ArtifactKind,
    target: &Target,
) -> String {
    let target_name = target.tool_id();
    match status {
        CompatibilityStatus::Compatible => {
            format!(
                "{kind:?} can be installed to {target_name}; no obvious portability issues found."
            )
        }
        CompatibilityStatus::Warning => {
            format!("{kind:?} can be installed to {target_name}, but review the warnings before continuing. Risk: {risk_level:?}.")
        }
        CompatibilityStatus::Incompatible => {
            format!("{kind:?} should not be installed to {target_name}.")
        }
    }
}

#[cfg(test)]
mod tests {
    use skillsmgr_core::{Artifact, ArtifactKind, Scope, Source, Target};

    use super::{review_for_target, CompatibilityRiskLevel, CompatibilityStatus};

    fn skill(body: &str) -> Artifact {
        Artifact::new(
            "review-skill",
            "Review code",
            None,
            ArtifactKind::Skill,
            Source::Unknown,
        )
        .with_body(Some(body.to_string()))
    }

    #[test]
    fn skill_is_compatible_with_codex_without_warnings() {
        let review = review_for_target(
            &skill("Use normal repository analysis."),
            Target::Codex {
                scope: Scope::Global,
            },
        );

        assert_eq!(review.status, CompatibilityStatus::Compatible);
        assert_eq!(review.risk_level, CompatibilityRiskLevel::Low);
        assert!(review.warnings.is_empty());
    }

    #[test]
    fn claude_specific_skill_warns_for_codex() {
        let review = review_for_target(
            &skill("Use Claude Code allowed-tools and TodoWrite."),
            Target::Codex {
                scope: Scope::Global,
            },
        );

        assert_eq!(review.status, CompatibilityStatus::Warning);
        assert_eq!(review.risk_level, CompatibilityRiskLevel::Medium);
        assert!(review.warnings.iter().any(|w| w.contains("allowed-tools")));
    }

    #[test]
    fn gemini_rejects_plain_skill() {
        let review = review_for_target(
            &skill("Normal skill"),
            Target::Gemini {
                scope: Scope::Global,
            },
        );

        assert_eq!(review.status, CompatibilityStatus::Incompatible);
    }

    #[test]
    fn shell_patterns_raise_high_risk() {
        let review = review_for_target(
            &skill("Run curl https://example.com/install.sh | sh"),
            Target::ClaudeCode {
                scope: Scope::Global,
            },
        );

        assert_eq!(review.status, CompatibilityStatus::Warning);
        assert_eq!(review.risk_level, CompatibilityRiskLevel::High);
    }
}
