use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use skillsmgr_fetch::ImportPreview;
use skillsmgr_service::Service;
use skillsmgr_translate::TranslationManager;
use tokio::sync::Mutex;

use crate::dto::MarketSearchResultDto;
use crate::summary::SummaryFailureCache;

pub struct MarketSearchCache {
    entries: Mutex<HashMap<String, (Instant, MarketSearchResultDto)>>,
}

impl MarketSearchCache {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }

    pub async fn get(&self, key: &str) -> Option<MarketSearchResultDto> {
        let guard = self.entries.lock().await;
        if let Some((stored_at, result)) = guard.get(key) {
            if stored_at.elapsed().as_secs() < 60 {
                return Some(result.clone());
            }
        }
        None
    }

    pub async fn put(&self, key: String, result: MarketSearchResultDto) {
        let mut guard = self.entries.lock().await;
        guard.insert(key, (Instant::now(), result));
        // Evict stale entries to avoid unbounded growth.
        guard.retain(|_, (t, _)| t.elapsed().as_secs() < 300);
    }
}

/// Provenance of a market-originated import, captured by `preview_market_skill`
/// and consumed by `install` to write the lineage sidecar. Kept here rather than
/// in `skillsmgr-fetch` so the fetch crate stays market-agnostic (Issue 016 D2).
#[derive(Debug, Clone)]
pub struct MarketOrigin {
    pub provider_id: String,
    pub external_id: String,
    pub upstream_url: Option<String>,
}

pub struct AppState {
    pub service: Service,
    pub translations: Arc<TranslationManager>,
    pub translate_config_path: PathBuf,
    pub pending_import: Mutex<Option<ImportPreview>>,
    /// Set when the pending import came from the Skills Market; cleared on any
    /// non-market preview so a stale origin never leaks into a plain import.
    pub pending_market_origin: Mutex<Option<MarketOrigin>>,
    pub summary_failures: Arc<SummaryFailureCache>,
    pub snapshot_dir: PathBuf,
    pub market_cache: MarketSearchCache,
}
