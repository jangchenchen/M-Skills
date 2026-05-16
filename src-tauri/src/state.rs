use skillsmgr_fetch::ImportPreview;
use skillsmgr_service::Service;
use tokio::sync::Mutex;

pub struct AppState {
    pub service: Service,
    pub pending_import: Mutex<Option<ImportPreview>>,
}
