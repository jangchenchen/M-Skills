use skillsmgr_service::Service;
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
            app.manage(AppState {
                service,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
