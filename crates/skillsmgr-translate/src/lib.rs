use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use skillsmgr_core::{Result, SkillsMgrError};
use skillsmgr_registry::{Registry, RegistryTranslation, SkillSummaryInput, TranslationInput};

pub use skillsmgr_registry::RegistrySkillSummary;

pub mod config;
pub mod keyring_store;
pub mod markdown_validation;
pub mod openai_compat;

pub use config::{ProviderKind, TranslateConfig};
pub use markdown_validation::{validate_markdown_fidelity, MarkdownWarning, TranslationValidation};
pub use openai_compat::OpenAICompatProvider;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationRequest {
    pub artifact_name: String,
    pub file_path: PathBuf,
    pub field: String,
    pub source_text: String,
    pub locale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheStatus {
    /// Returned a cached translation row without calling the provider.
    Hit,
    /// Called the provider because the cache was empty for this combination,
    /// or because the provider does not cache (e.g. passthrough).
    Miss,
    /// Called the provider with `force_refresh = true` and overwrote the cache.
    Refreshed,
}

impl CacheStatus {
    pub fn as_id(&self) -> &'static str {
        match self {
            CacheStatus::Hit => "hit",
            CacheStatus::Miss => "miss",
            CacheStatus::Refreshed => "refreshed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct TranslateOutcome {
    pub text: String,
    pub locale: String,
    pub field: String,
    pub source_sha256: String,
    pub cache_status: CacheStatus,
    pub provider_kind: &'static str,
    pub used_fallback: bool,
    pub validation: TranslationValidation,
}

#[async_trait]
pub trait TranslationProvider: Send + Sync {
    async fn translate(&self, request: &TranslationRequest) -> Result<String>;

    /// Whether outputs from this provider should be cached. Defaults to true.
    /// Identity/passthrough providers should override to false so a later switch
    /// to a real provider isn't masked by stale `source → source` cache rows.
    fn caches(&self) -> bool {
        true
    }

    /// Stable id surfaced to the UI as `providerKind`. Production providers
    /// must override; the default is for tests that don't care.
    fn kind(&self) -> &'static str {
        "unknown"
    }
}

pub struct TranslationManager {
    registry: Mutex<Registry>,
    provider: RwLock<Arc<dyn TranslationProvider>>,
    fallback: RwLock<Option<Arc<dyn TranslationProvider>>>,
}

impl TranslationManager {
    pub fn new(registry: Registry, provider: Arc<dyn TranslationProvider>) -> Self {
        Self {
            registry: Mutex::new(registry),
            provider: RwLock::new(provider),
            fallback: RwLock::new(None),
        }
    }

    pub fn swap_providers(
        &self,
        primary: Arc<dyn TranslationProvider>,
        fallback: Option<Arc<dyn TranslationProvider>>,
    ) {
        *self
            .provider
            .write()
            .expect("translation provider lock poisoned") = primary;
        *self
            .fallback
            .write()
            .expect("translation fallback lock poisoned") = fallback;
    }

    pub fn set_fallback(&self, fallback: Option<Arc<dyn TranslationProvider>>) {
        *self
            .fallback
            .write()
            .expect("translation fallback lock poisoned") = fallback;
    }

    pub fn source_sha256(source_text: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(source_text.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    pub fn cache_lookup(
        &self,
        artifact_name: &str,
        file_path: &Path,
        field: &str,
        source_text: &str,
        locale: &str,
    ) -> Result<Option<RegistryTranslation>> {
        let source_sha256 = Self::source_sha256(source_text);
        self.registry
            .lock()
            .expect("translation registry mutex poisoned")
            .translation(artifact_name, file_path, field, &source_sha256, locale)
    }

    pub fn clear_cache(
        &self,
        artifact_name: &str,
        file_path: &Path,
        field: &str,
        locale: &str,
    ) -> Result<usize> {
        self.registry
            .lock()
            .expect("translation registry mutex poisoned")
            .clear_translations(artifact_name, file_path, field, locale)
    }

    /// Look up a cached AI-generated skill summary by skill name + canonical
    /// SKILL.md text + locale. The text is hashed with the same SHA-256 logic
    /// translations use, so callers don't need to pre-hash.
    pub fn skill_summary_lookup(
        &self,
        skill_name: &str,
        source_text: &str,
        locale: &str,
    ) -> Result<Option<RegistrySkillSummary>> {
        let source_sha256 = Self::source_sha256(source_text);
        self.registry
            .lock()
            .expect("translation registry mutex poisoned")
            .skill_summary(skill_name, &source_sha256, locale)
    }

    /// Upsert a freshly-generated skill summary. The text is hashed inline.
    /// Stale rows for the same `(skill_name, locale)` with a different hash
    /// are purged by `Registry::upsert_skill_summary`.
    pub fn upsert_skill_summary(
        &self,
        skill_name: &str,
        source_text: &str,
        locale: &str,
        summary_json: &str,
        model: &str,
    ) -> Result<RegistrySkillSummary> {
        let source_sha256 = Self::source_sha256(source_text);
        let input = SkillSummaryInput {
            skill_name: skill_name.to_string(),
            source_sha256,
            locale: locale.to_string(),
            summary_json: summary_json.to_string(),
            model: model.to_string(),
        };
        self.registry
            .lock()
            .expect("translation registry mutex poisoned")
            .upsert_skill_summary(&input)
    }

    /// Drop every cached summary for `(skill_name, locale)`. Used when a
    /// stored row's JSON is unreadable so the caller can fall through to a
    /// fresh LLM generation without serving garbage.
    pub fn clear_skill_summary(&self, skill_name: &str, locale: &str) -> Result<usize> {
        self.registry
            .lock()
            .expect("translation registry mutex poisoned")
            .clear_skill_summary(skill_name, locale)
    }

    pub async fn translate_or_get(
        &self,
        request: TranslationRequest,
        force_refresh: bool,
    ) -> Result<TranslateOutcome> {
        let source_sha256 = Self::source_sha256(&request.source_text);

        let provider = self
            .provider
            .read()
            .expect("translation provider lock poisoned")
            .clone();
        let fallback = self
            .fallback
            .read()
            .expect("translation fallback lock poisoned")
            .clone();
        let caches = provider.caches();
        let provider_kind = provider.kind();

        if caches && !force_refresh {
            let existing = {
                let registry = self
                    .registry
                    .lock()
                    .expect("translation registry mutex poisoned");
                registry.translation(
                    &request.artifact_name,
                    &request.file_path,
                    &request.field,
                    &source_sha256,
                    &request.locale,
                )?
            };
            if let Some(existing) = existing {
                if existing.translated_text.trim().is_empty() {
                    self.registry
                        .lock()
                        .expect("translation registry mutex poisoned")
                        .clear_translations(
                            &request.artifact_name,
                            &request.file_path,
                            &request.field,
                            &request.locale,
                        )?;
                } else {
                    let validation =
                        validate_markdown_fidelity(&request.source_text, &existing.translated_text);
                    return Ok(TranslateOutcome {
                        text: existing.translated_text,
                        locale: request.locale,
                        field: request.field,
                        source_sha256,
                        cache_status: CacheStatus::Hit,
                        provider_kind,
                        used_fallback: false,
                        validation,
                    });
                }
            }
        }

        let (translated_text, used_fallback) = match provider.translate(&request).await {
            Ok(text) if !text.trim().is_empty() => (text, false),
            Ok(_) => match fallback.as_ref() {
                Some(fb) => match fb.translate(&request).await {
                    Ok(text) if !text.trim().is_empty() => (text, true),
                    Ok(_) | Err(_) => {
                        return Err(empty_translation_error(provider_kind));
                    }
                },
                None => return Err(empty_translation_error(provider_kind)),
            },
            Err(primary_err) => match fallback.as_ref() {
                Some(fb) => match fb.translate(&request).await {
                    Ok(text) if !text.trim().is_empty() => (text, true),
                    Ok(_) => return Err(primary_err),
                    Err(_fb_err) => return Err(primary_err),
                },
                None => return Err(primary_err),
            },
        };
        let cache_status = if force_refresh && caches {
            CacheStatus::Refreshed
        } else {
            CacheStatus::Miss
        };

        if caches {
            self.registry
                .lock()
                .expect("translation registry mutex poisoned")
                .upsert_translation(&TranslationInput {
                    artifact_name: request.artifact_name.clone(),
                    file_path: request.file_path.clone(),
                    field: request.field.clone(),
                    source_sha256: source_sha256.clone(),
                    locale: request.locale.clone(),
                    translated_text: translated_text.clone(),
                })?;
        }

        // Passthrough produces output identical to source, so validation is
        // trivially ok. Skip the work and avoid surprising warnings if/when
        // passthrough behavior ever changes.
        let validation = if caches {
            validate_markdown_fidelity(&request.source_text, &translated_text)
        } else {
            TranslationValidation::ok()
        };

        Ok(TranslateOutcome {
            text: translated_text,
            locale: request.locale,
            field: request.field,
            source_sha256,
            cache_status,
            provider_kind,
            used_fallback,
            validation,
        })
    }
}

fn empty_translation_error(provider_kind: &'static str) -> SkillsMgrError {
    SkillsMgrError::TranslateProvider {
        kind: provider_kind.into(),
        status: None,
        message: "provider returned empty translated text".into(),
    }
}

pub struct PassthroughTranslationProvider;

#[async_trait]
impl TranslationProvider for PassthroughTranslationProvider {
    async fn translate(&self, request: &TranslationRequest) -> Result<String> {
        Ok(request.source_text.clone())
    }

    fn caches(&self) -> bool {
        false
    }

    fn kind(&self) -> &'static str {
        ProviderKind::Passthrough.as_id()
    }
}

pub fn build_providers(
    config: &TranslateConfig,
    api_key: Option<String>,
) -> Result<(
    Arc<dyn TranslationProvider>,
    Option<Arc<dyn TranslationProvider>>,
)> {
    match config.provider_kind {
        ProviderKind::Passthrough => Ok((Arc::new(PassthroughTranslationProvider), None)),
        ProviderKind::OpenAiCompat => {
            let key = api_key.ok_or_else(|| SkillsMgrError::TranslateConfig {
                reason: "openai-compat provider requires an API key".into(),
            })?;
            let primary = OpenAICompatProvider::new(
                config.base_url.clone(),
                config.model.clone(),
                key.clone(),
                Duration::from_millis(config.timeout_ms),
                config.max_retries,
            )?;
            let fallback = match config.fallback_model.as_ref() {
                Some(name) if !name.trim().is_empty() => {
                    let fb = OpenAICompatProvider::new(
                        config.base_url.clone(),
                        name.clone(),
                        key,
                        Duration::from_millis(config.timeout_ms),
                        config.max_retries,
                    )?;
                    Some(Arc::new(fb) as Arc<dyn TranslationProvider>)
                }
                _ => None,
            };
            Ok((Arc::new(primary), fallback))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use skillsmgr_registry::Registry;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn sample_request() -> TranslationRequest {
        TranslationRequest {
            artifact_name: "demo".to_string(),
            file_path: PathBuf::from("SKILL.md"),
            field: "body".to_string(),
            source_text: "Hello".to_string(),
            locale: "zh".to_string(),
        }
    }

    struct StubProvider {
        label: &'static str,
    }

    #[async_trait]
    impl TranslationProvider for StubProvider {
        async fn translate(&self, request: &TranslationRequest) -> Result<String> {
            Ok(format!("{}:{}", self.label, request.source_text))
        }

        fn kind(&self) -> &'static str {
            self.label
        }
    }

    struct CountingProvider {
        calls: AtomicUsize,
    }

    impl CountingProvider {
        fn new() -> Self {
            Self {
                calls: AtomicUsize::new(0),
            }
        }
    }

    struct FailingProvider;
    struct EmptyProvider;

    #[async_trait]
    impl TranslationProvider for FailingProvider {
        async fn translate(&self, _request: &TranslationRequest) -> Result<String> {
            Err(SkillsMgrError::TranslateProvider {
                kind: "failing".into(),
                status: Some(503),
                message: "upstream unavailable".into(),
            })
        }

        fn kind(&self) -> &'static str {
            "failing"
        }
    }

    #[async_trait]
    impl TranslationProvider for EmptyProvider {
        async fn translate(&self, _request: &TranslationRequest) -> Result<String> {
            Ok("  \n\t".to_string())
        }

        fn kind(&self) -> &'static str {
            "empty"
        }
    }

    #[async_trait]
    impl TranslationProvider for CountingProvider {
        async fn translate(&self, request: &TranslationRequest) -> Result<String> {
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(format!("call-{n}:{}", request.source_text))
        }

        fn kind(&self) -> &'static str {
            "counting"
        }
    }

    #[tokio::test]
    async fn cache_miss_then_hit_returns_same_text_with_updated_status() {
        let registry = Registry::in_memory().unwrap();
        let manager = TranslationManager::new(registry, Arc::new(StubProvider { label: "first" }));
        let request = sample_request();

        let first = manager
            .translate_or_get(request.clone(), false)
            .await
            .unwrap();
        assert_eq!(first.text, "first:Hello");
        assert_eq!(first.cache_status, CacheStatus::Miss);
        assert_eq!(first.provider_kind, "first");

        let second = manager.translate_or_get(request, false).await.unwrap();
        assert_eq!(second.text, "first:Hello");
        assert_eq!(second.cache_status, CacheStatus::Hit);
        assert_eq!(second.provider_kind, "first");
    }

    #[tokio::test]
    async fn force_refresh_bypasses_cache_and_overwrites() {
        let registry = Registry::in_memory().unwrap();
        let provider = Arc::new(CountingProvider::new());
        let manager = TranslationManager::new(registry, provider.clone());
        let request = sample_request();

        let first = manager
            .translate_or_get(request.clone(), false)
            .await
            .unwrap();
        assert_eq!(first.text, "call-0:Hello");
        assert_eq!(first.cache_status, CacheStatus::Miss);

        // Second call without force: cache hit, provider not called again.
        let cached = manager
            .translate_or_get(request.clone(), false)
            .await
            .unwrap();
        assert_eq!(cached.text, "call-0:Hello");
        assert_eq!(cached.cache_status, CacheStatus::Hit);
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);

        // force_refresh: provider called again, cache overwritten.
        let refreshed = manager
            .translate_or_get(request.clone(), true)
            .await
            .unwrap();
        assert_eq!(refreshed.text, "call-1:Hello");
        assert_eq!(refreshed.cache_status, CacheStatus::Refreshed);
        assert_eq!(provider.calls.load(Ordering::SeqCst), 2);

        // Subsequent normal call now reads the refreshed value from cache.
        let after = manager.translate_or_get(request, false).await.unwrap();
        assert_eq!(after.text, "call-1:Hello");
        assert_eq!(after.cache_status, CacheStatus::Hit);
        assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn failed_force_refresh_leaves_existing_cache_intact() {
        let registry = Registry::in_memory().unwrap();
        let manager = TranslationManager::new(registry, Arc::new(StubProvider { label: "old" }));
        let request = sample_request();

        let first = manager
            .translate_or_get(request.clone(), false)
            .await
            .unwrap();
        assert_eq!(first.text, "old:Hello");
        assert_eq!(first.cache_status, CacheStatus::Miss);

        manager.swap_providers(Arc::new(FailingProvider), None);
        let err = manager
            .translate_or_get(request.clone(), true)
            .await
            .unwrap_err();
        match err {
            SkillsMgrError::TranslateProvider { kind, status, .. } => {
                assert_eq!(kind, "failing");
                assert_eq!(status, Some(503));
            }
            other => panic!("expected TranslateProvider, got {other:?}"),
        }

        manager.swap_providers(Arc::new(StubProvider { label: "new" }), None);
        let cached = manager.translate_or_get(request, false).await.unwrap();
        assert_eq!(cached.text, "old:Hello");
        assert_eq!(cached.cache_status, CacheStatus::Hit);
        assert_eq!(cached.provider_kind, "new");
    }

    #[tokio::test]
    async fn passthrough_outcome_is_miss_with_passthrough_kind_and_no_cache_row() {
        let registry = Registry::in_memory().unwrap();
        let manager = TranslationManager::new(registry, Arc::new(PassthroughTranslationProvider));
        let request = sample_request();

        let result = manager
            .translate_or_get(request.clone(), false)
            .await
            .unwrap();
        assert_eq!(result.text, "Hello");
        assert_eq!(result.cache_status, CacheStatus::Miss);
        assert_eq!(result.provider_kind, "passthrough");

        // Even with force_refresh, passthrough stays Miss (it doesn't cache, so
        // "refreshed" would imply an overwrite that never happens).
        let forced = manager
            .translate_or_get(request.clone(), true)
            .await
            .unwrap();
        assert_eq!(forced.cache_status, CacheStatus::Miss);

        let cached = manager
            .cache_lookup(
                &request.artifact_name,
                &request.file_path,
                &request.field,
                &request.source_text,
                &request.locale,
            )
            .unwrap();
        assert!(cached.is_none(), "passthrough must not populate cache");
    }

    #[tokio::test]
    async fn switching_from_passthrough_to_real_provider_does_not_serve_stale_source() {
        let registry = Registry::in_memory().unwrap();
        let manager = TranslationManager::new(registry, Arc::new(PassthroughTranslationProvider));
        let request = sample_request();

        // Under passthrough, output equals source.
        let passthrough_out = manager
            .translate_or_get(request.clone(), false)
            .await
            .unwrap();
        assert_eq!(passthrough_out.text, "Hello");
        assert_eq!(passthrough_out.provider_kind, "passthrough");

        // Switch to a real provider — next call must NOT serve the cached "Hello"
        // and provider_kind must follow the new provider.
        manager.swap_providers(Arc::new(StubProvider { label: "real" }), None);
        let real_out = manager.translate_or_get(request, false).await.unwrap();
        assert_eq!(real_out.text, "real:Hello");
        assert_eq!(real_out.cache_status, CacheStatus::Miss);
        assert_eq!(real_out.provider_kind, "real");
    }

    #[tokio::test]
    async fn clear_cache_removes_matching_entry_and_forces_retranslation() {
        let registry = Registry::in_memory().unwrap();
        let provider = Arc::new(CountingProvider::new());
        let manager = TranslationManager::new(registry, provider.clone());
        let request = sample_request();

        // Populate cache.
        manager
            .translate_or_get(request.clone(), false)
            .await
            .unwrap();
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);

        // Clear and verify the cache row is gone.
        let deleted = manager
            .clear_cache(
                &request.artifact_name,
                &request.file_path,
                &request.field,
                &request.locale,
            )
            .unwrap();
        assert_eq!(deleted, 1);
        let still_there = manager
            .cache_lookup(
                &request.artifact_name,
                &request.file_path,
                &request.field,
                &request.source_text,
                &request.locale,
            )
            .unwrap();
        assert!(still_there.is_none());

        // Next translate hits the provider again.
        let after = manager.translate_or_get(request, false).await.unwrap();
        assert_eq!(after.text, "call-1:Hello");
        assert_eq!(after.cache_status, CacheStatus::Miss);
        assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn clear_cache_does_not_touch_other_skills_or_locales() {
        let registry = Registry::in_memory().unwrap();
        let manager = TranslationManager::new(registry, Arc::new(StubProvider { label: "x" }));

        let req_a_zh = TranslationRequest {
            artifact_name: "a".into(),
            file_path: PathBuf::from("SKILL.md"),
            field: "body".into(),
            source_text: "Hello".into(),
            locale: "zh".into(),
        };
        let req_a_ja = TranslationRequest {
            locale: "ja".into(),
            ..req_a_zh.clone()
        };
        let req_b_zh = TranslationRequest {
            artifact_name: "b".into(),
            ..req_a_zh.clone()
        };

        manager
            .translate_or_get(req_a_zh.clone(), false)
            .await
            .unwrap();
        manager
            .translate_or_get(req_a_ja.clone(), false)
            .await
            .unwrap();
        manager
            .translate_or_get(req_b_zh.clone(), false)
            .await
            .unwrap();

        let deleted = manager
            .clear_cache(
                &req_a_zh.artifact_name,
                &req_a_zh.file_path,
                &req_a_zh.field,
                &req_a_zh.locale,
            )
            .unwrap();
        assert_eq!(deleted, 1);

        assert!(manager
            .cache_lookup(
                &req_a_zh.artifact_name,
                &req_a_zh.file_path,
                &req_a_zh.field,
                &req_a_zh.source_text,
                &req_a_zh.locale,
            )
            .unwrap()
            .is_none());
        assert!(manager
            .cache_lookup(
                &req_a_ja.artifact_name,
                &req_a_ja.file_path,
                &req_a_ja.field,
                &req_a_ja.source_text,
                &req_a_ja.locale,
            )
            .unwrap()
            .is_some());
        assert!(manager
            .cache_lookup(
                &req_b_zh.artifact_name,
                &req_b_zh.file_path,
                &req_b_zh.field,
                &req_b_zh.source_text,
                &req_b_zh.locale,
            )
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn swap_provider_changes_translation_source_and_kind() {
        let registry = Registry::in_memory().unwrap();
        let manager = TranslationManager::new(registry, Arc::new(StubProvider { label: "first" }));

        let req_a = TranslationRequest {
            artifact_name: "demo".into(),
            file_path: PathBuf::from("SKILL.md"),
            field: "body".into(),
            source_text: "alpha".into(),
            locale: "zh".into(),
        };
        let first = manager.translate_or_get(req_a, false).await.unwrap();
        assert_eq!(first.text, "first:alpha");
        assert_eq!(first.provider_kind, "first");

        manager.swap_providers(Arc::new(StubProvider { label: "second" }), None);

        let req_b = TranslationRequest {
            artifact_name: "demo".into(),
            file_path: PathBuf::from("SKILL.md"),
            field: "body".into(),
            source_text: "beta".into(),
            locale: "zh".into(),
        };
        let second = manager.translate_or_get(req_b, false).await.unwrap();
        assert_eq!(second.text, "second:beta");
        assert_eq!(second.provider_kind, "second");
    }

    #[tokio::test]
    async fn source_sha256_is_returned_and_stable_for_same_text() {
        let registry = Registry::in_memory().unwrap();
        let manager = TranslationManager::new(registry, Arc::new(StubProvider { label: "x" }));

        let out = manager
            .translate_or_get(sample_request(), false)
            .await
            .unwrap();
        assert_eq!(
            out.source_sha256,
            TranslationManager::source_sha256("Hello")
        );
        assert_eq!(out.locale, "zh");
        assert_eq!(out.field, "body");
    }

    #[tokio::test]
    async fn validation_surfaces_warnings_when_provider_drops_structure() {
        // A provider that strips markdown structure on purpose.
        struct StrippingProvider;
        #[async_trait]
        impl TranslationProvider for StrippingProvider {
            async fn translate(&self, _request: &TranslationRequest) -> Result<String> {
                Ok("译文丢了链接".to_string())
            }
        }

        let registry = Registry::in_memory().unwrap();
        let manager = TranslationManager::new(registry, Arc::new(StrippingProvider));

        let request = TranslationRequest {
            artifact_name: "demo".into(),
            file_path: PathBuf::from("SKILL.md"),
            field: "body".into(),
            source_text: "See [docs](https://example.com)".into(),
            locale: "zh".into(),
        };
        let out = manager.translate_or_get(request, false).await.unwrap();
        assert!(!out.validation.ok);
        assert!(out
            .validation
            .warnings
            .iter()
            .any(|w| matches!(w, MarkdownWarning::LinkCount { .. })));
    }

    #[tokio::test]
    async fn force_refresh_reports_validation_for_refreshed_output() {
        // First provider populates a clean cache row; the second simulates a
        // retranslation that drops markdown structure.
        struct LinkDroppingProvider;
        #[async_trait]
        impl TranslationProvider for LinkDroppingProvider {
            async fn translate(&self, _request: &TranslationRequest) -> Result<String> {
                Ok("译文丢了链接".to_string())
            }
        }

        let registry = Registry::in_memory().unwrap();
        let manager = TranslationManager::new(registry, Arc::new(StubProvider { label: "clean" }));

        let request = TranslationRequest {
            artifact_name: "demo".into(),
            file_path: PathBuf::from("SKILL.md"),
            field: "body".into(),
            source_text: "See [docs](https://example.com)".into(),
            locale: "zh".into(),
        };
        let miss = manager
            .translate_or_get(request.clone(), false)
            .await
            .unwrap();
        assert_eq!(miss.cache_status, CacheStatus::Miss);
        assert!(miss.validation.ok);

        manager.swap_providers(Arc::new(LinkDroppingProvider), None);
        let refreshed = manager.translate_or_get(request, true).await.unwrap();
        assert_eq!(refreshed.cache_status, CacheStatus::Refreshed);
        assert!(!refreshed.validation.ok);
        assert!(refreshed
            .validation
            .warnings
            .iter()
            .any(|w| matches!(w, MarkdownWarning::LinkCount { .. })));
    }

    #[tokio::test]
    async fn passthrough_validation_is_skipped_and_always_ok() {
        let registry = Registry::in_memory().unwrap();
        let manager = TranslationManager::new(registry, Arc::new(PassthroughTranslationProvider));
        let request = TranslationRequest {
            artifact_name: "demo".into(),
            file_path: PathBuf::from("SKILL.md"),
            field: "body".into(),
            source_text: "See [docs](https://example.com)".into(),
            locale: "zh".into(),
        };
        let out = manager.translate_or_get(request, false).await.unwrap();
        assert!(out.validation.ok);
        assert!(out.validation.warnings.is_empty());
    }

    #[tokio::test]
    async fn empty_provider_output_is_an_error_and_is_not_cached() {
        let registry = Registry::in_memory().unwrap();
        let manager = TranslationManager::new(registry, Arc::new(EmptyProvider));
        let request = sample_request();

        let err = manager
            .translate_or_get(request.clone(), false)
            .await
            .unwrap_err();
        match err {
            SkillsMgrError::TranslateProvider { kind, message, .. } => {
                assert_eq!(kind, "empty");
                assert_eq!(message, "provider returned empty translated text");
            }
            other => panic!("expected TranslateProvider, got {other:?}"),
        }

        let cached = manager
            .cache_lookup(
                &request.artifact_name,
                &request.file_path,
                &request.field,
                &request.source_text,
                &request.locale,
            )
            .unwrap();
        assert!(cached.is_none());
    }

    #[tokio::test]
    async fn empty_primary_output_uses_fallback_translation() {
        let registry = Registry::in_memory().unwrap();
        let manager = TranslationManager::new(registry, Arc::new(EmptyProvider));
        manager.set_fallback(Some(Arc::new(StubProvider { label: "fallback" })));

        let outcome = manager
            .translate_or_get(sample_request(), false)
            .await
            .unwrap();

        assert_eq!(outcome.text, "fallback:Hello");
        assert!(outcome.used_fallback);
    }

    #[tokio::test]
    async fn blank_cache_entry_is_ignored_and_replaced() {
        let registry = Registry::in_memory().unwrap();
        registry
            .upsert_translation(&TranslationInput {
                artifact_name: "demo".into(),
                file_path: PathBuf::from("SKILL.md"),
                field: "body".into(),
                source_sha256: TranslationManager::source_sha256("Hello"),
                locale: "zh".into(),
                translated_text: " \n".into(),
            })
            .unwrap();
        let manager = TranslationManager::new(registry, Arc::new(StubProvider { label: "fresh" }));

        let outcome = manager
            .translate_or_get(sample_request(), false)
            .await
            .unwrap();

        assert_eq!(outcome.text, "fresh:Hello");
        assert_eq!(outcome.cache_status, CacheStatus::Miss);
    }

    #[tokio::test]
    async fn primary_failure_falls_back_and_outcome_marks_used_fallback() {
        let registry = Registry::in_memory().unwrap();
        let manager = TranslationManager::new(registry, Arc::new(FailingProvider));
        manager.set_fallback(Some(Arc::new(StubProvider { label: "fallback" })));
        let request = sample_request();

        let outcome = manager
            .translate_or_get(request.clone(), false)
            .await
            .unwrap();

        assert_eq!(outcome.text, "fallback:Hello");
        assert!(outcome.used_fallback);
        assert_eq!(outcome.cache_status, CacheStatus::Miss);

        let cached = manager
            .cache_lookup(
                &request.artifact_name,
                &request.file_path,
                &request.field,
                &request.source_text,
                &request.locale,
            )
            .unwrap()
            .unwrap();
        assert_eq!(cached.translated_text, "fallback:Hello");
    }

    #[tokio::test]
    async fn primary_and_fallback_both_fail_returns_primary_error() {
        struct OtherFailingProvider;
        #[async_trait]
        impl TranslationProvider for OtherFailingProvider {
            async fn translate(&self, _request: &TranslationRequest) -> Result<String> {
                Err(SkillsMgrError::TranslateProvider {
                    kind: "other-failing".into(),
                    status: Some(429),
                    message: "rate limited".into(),
                })
            }

            fn kind(&self) -> &'static str {
                "other-failing"
            }
        }

        let registry = Registry::in_memory().unwrap();
        let manager = TranslationManager::new(registry, Arc::new(FailingProvider));
        manager.set_fallback(Some(Arc::new(OtherFailingProvider)));

        let err = manager
            .translate_or_get(sample_request(), false)
            .await
            .unwrap_err();
        match err {
            SkillsMgrError::TranslateProvider { kind, status, .. } => {
                assert_eq!(kind, "failing");
                assert_eq!(status, Some(503));
            }
            other => panic!("expected TranslateProvider, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn no_fallback_configured_returns_primary_error_directly() {
        let registry = Registry::in_memory().unwrap();
        let manager = TranslationManager::new(registry, Arc::new(FailingProvider));

        let err = manager
            .translate_or_get(sample_request(), false)
            .await
            .unwrap_err();
        match err {
            SkillsMgrError::TranslateProvider { kind, .. } => {
                assert_eq!(kind, "failing");
            }
            other => panic!("expected TranslateProvider, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn cache_hit_does_not_call_fallback() {
        let registry = Registry::in_memory().unwrap();
        let manager =
            TranslationManager::new(registry, Arc::new(StubProvider { label: "primary" }));
        let request = sample_request();

        manager
            .translate_or_get(request.clone(), false)
            .await
            .unwrap();

        let fallback = Arc::new(CountingProvider::new());
        manager.swap_providers(Arc::new(FailingProvider), Some(fallback.clone()));

        let outcome = manager.translate_or_get(request, false).await.unwrap();
        assert_eq!(outcome.text, "primary:Hello");
        assert_eq!(outcome.cache_status, CacheStatus::Hit);
        assert!(!outcome.used_fallback);
        assert_eq!(fallback.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn used_fallback_is_false_on_primary_success() {
        let registry = Registry::in_memory().unwrap();
        let manager =
            TranslationManager::new(registry, Arc::new(StubProvider { label: "primary" }));
        manager.set_fallback(Some(Arc::new(StubProvider { label: "fallback" })));

        let outcome = manager
            .translate_or_get(sample_request(), false)
            .await
            .unwrap();
        assert_eq!(outcome.text, "primary:Hello");
        assert!(!outcome.used_fallback);
    }
}
