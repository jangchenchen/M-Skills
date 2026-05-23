use std::path::Path;

use serde::{Deserialize, Serialize};
use skillsmgr_core::{Result, SkillsMgrError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderKind {
    Passthrough,
    OpenAiCompat,
}

impl ProviderKind {
    pub fn as_id(&self) -> &'static str {
        match self {
            ProviderKind::Passthrough => "passthrough",
            ProviderKind::OpenAiCompat => "openai-compat",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateConfig {
    pub provider_kind: ProviderKind,
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default)]
    pub fallback_model: Option<String>,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

fn default_base_url() -> String {
    "https://api.deepseek.com/v1".to_string()
}

fn default_model() -> String {
    "deepseek-chat".to_string()
}

fn default_timeout_ms() -> u64 {
    30_000
}

fn default_max_retries() -> u32 {
    2
}

impl Default for TranslateConfig {
    fn default() -> Self {
        TranslateConfig {
            provider_kind: ProviderKind::Passthrough,
            base_url: default_base_url(),
            model: default_model(),
            fallback_model: None,
            timeout_ms: default_timeout_ms(),
            max_retries: default_max_retries(),
        }
    }
}

impl TranslateConfig {
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(content) => toml::from_str(&content).map_err(|e| SkillsMgrError::TranslateConfig {
                reason: format!("failed to parse {}: {e}", path.display()),
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(TranslateConfig::default()),
            Err(e) => Err(SkillsMgrError::Fs {
                path: path.to_path_buf(),
                source: e,
            }),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| SkillsMgrError::Fs {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }
        let content =
            toml::to_string_pretty(self).map_err(|e| SkillsMgrError::TranslateConfig {
                reason: format!("failed to serialize config: {e}"),
            })?;
        std::fs::write(path, content).map_err(|e| SkillsMgrError::Fs {
            path: path.to_path_buf(),
            source: e,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("translate.toml");
        let config = TranslateConfig::load(&path).unwrap();
        assert_eq!(config.provider_kind, ProviderKind::Passthrough);
    }

    #[test]
    fn round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("translate.toml");
        let original = TranslateConfig {
            provider_kind: ProviderKind::OpenAiCompat,
            base_url: "https://example.com/v1".into(),
            model: "test-model".into(),
            fallback_model: Some("fallback-model".into()),
            timeout_ms: 10_000,
            max_retries: 5,
        };
        original.save(&path).unwrap();
        let loaded = TranslateConfig::load(&path).unwrap();
        assert_eq!(loaded.provider_kind, ProviderKind::OpenAiCompat);
        assert_eq!(loaded.base_url, "https://example.com/v1");
        assert_eq!(loaded.model, "test-model");
        assert_eq!(loaded.fallback_model.as_deref(), Some("fallback-model"));
        assert_eq!(loaded.timeout_ms, 10_000);
        assert_eq!(loaded.max_retries, 5);
    }

    #[test]
    fn old_config_without_fallback_model_loads_with_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("translate.toml");
        std::fs::write(
            &path,
            "provider_kind = \"passthrough\"\n\
             base_url = \"https://example.com/v1\"\n\
             model = \"test-model\"\n\
             timeout_ms = 10000\n\
             max_retries = 2\n",
        )
        .unwrap();
        let loaded = TranslateConfig::load(&path).unwrap();
        assert!(loaded.fallback_model.is_none());
    }

    #[test]
    fn malformed_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("translate.toml");
        std::fs::write(&path, "this is not toml = = =").unwrap();
        let err = TranslateConfig::load(&path).unwrap_err();
        assert!(matches!(err, SkillsMgrError::TranslateConfig { .. }));
    }
}
