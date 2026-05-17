use skillsmgr_service::Service;
use skillsmgr_translate::{
    build_provider, keyring_store, PassthroughTranslationProvider, ProviderKind, TranslateConfig,
    TranslationManager,
};
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;

mod commands;
mod dto;
mod state;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            let home = std::env::var("HOME")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|_| app.path().home_dir().unwrap());
            let service = Service::with_home(home);
            let app_data_dir = app.path().app_data_dir().unwrap();
            std::fs::create_dir_all(&app_data_dir).ok();
            let registry_path = app_data_dir.join("registry.sqlite");
            let translate_config_path = app_data_dir.join("translate.toml");
            let registry = skillsmgr_registry::Registry::open(registry_path)?;

            let config = TranslateConfig::load(&translate_config_path).unwrap_or_else(|err| {
                log::warn!("translate config: {err}; using defaults");
                TranslateConfig::default()
            });
            let api_key = match config.provider_kind {
                ProviderKind::OpenAiCompat => {
                    keyring_store::get_api_key(config.provider_kind.as_id()).unwrap_or_else(|err| {
                        log::warn!("keychain read: {err}");
                        None
                    })
                }
                ProviderKind::Passthrough => None,
            };
            let provider = build_provider(&config, api_key).unwrap_or_else(|err| {
                log::warn!("build translation provider: {err}; falling back to passthrough");
                Arc::new(PassthroughTranslationProvider)
            });

            app.manage(AppState {
                service,
                translations: TranslationManager::new(registry, provider),
                translate_config_path,
                pending_import: Mutex::new(None),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::scan,
            commands::preview_import,
            commands::install,
            commands::uninstall,
            commands::enable,
            commands::disable,
            commands::translate_artifact,
            commands::clear_translation_cache,
            commands::get_translate_config,
            commands::set_translate_config,
            commands::test_translate_provider,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
