use std::path::PathBuf;

use skillsmgr_fetch::ImportPreview;
use skillsmgr_service::Service;
use skillsmgr_translate::TranslationManager;
use tokio::sync::Mutex;

pub struct AppState {
    pub service: Service,
    pub translations: TranslationManager,
    pub translate_config_path: PathBuf,
    pub pending_import: Mutex<Option<ImportPreview>>,
}
