use std::path::PathBuf;
use std::sync::Arc;

use skillsmgr_fetch::ImportPreview;
use skillsmgr_service::Service;
use skillsmgr_translate::TranslationManager;
use tokio::sync::Mutex;

use crate::summary::SummaryFailureCache;

pub struct AppState {
    pub service: Service,
    pub translations: Arc<TranslationManager>,
    pub translate_config_path: PathBuf,
    pub pending_import: Mutex<Option<ImportPreview>>,
    pub summary_failures: Arc<SummaryFailureCache>,
}
