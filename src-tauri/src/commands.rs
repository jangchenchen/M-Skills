use skillsmgr_core::{Installation, Scope, Status, Target};
use skillsmgr_fetch::ImportPreview;
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::dto::{ImportPreviewDto, InstallationDto, InventoryDto, TargetDto};
use crate::state::AppState;

#[tauri::command]
pub async fn scan(
    cwd: Option<String>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<InventoryDto, String> {
    let cwd_path = cwd.as_deref().map(std::path::Path::new);
    let inventory = state.service.inventory(cwd_path).await;
    let dto = InventoryDto::from(&inventory);
    app.emit("scan-complete", &dto).map_err(|e| e.to_string())?;
    Ok(dto)
}

#[tauri::command]
pub async fn preview_import(
    path_or_url: String,
    state: State<'_, AppState>,
) -> Result<ImportPreviewDto, String> {
    let scopes = vec![Scope::Global];
    let is_github = path_or_url.starts_with("https://github.com")
        || path_or_url.starts_with("http://github.com");

    let preview: ImportPreview = if is_github {
        state
            .service
            .preview_github_import(&path_or_url, scopes)
            .await
    } else {
        state
            .service
            .preview_local_import(&path_or_url, scopes)
            .await
    }
    .map_err(|e| e.to_string())?;

    let dto = ImportPreviewDto::from(&preview);
    *state.pending_import.lock().await = Some(preview);
    Ok(dto)
}

#[tauri::command]
pub async fn install(
    candidate_index: usize,
    target: TargetDto,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<InstallationDto, String> {
    let candidate = {
        let guard = state.pending_import.lock().await;
        let pending = guard.as_ref().ok_or("no pending import")?;
        pending
            .candidates
            .get(candidate_index)
            .ok_or("invalid candidate index")?
            .clone()
    };

    let target: Target = target.try_into()?;
    let installation = state
        .service
        .install_from_candidate(&candidate, target, vec![Scope::Global])
        .await
        .map_err(|e| e.to_string())?;

    let dto = InstallationDto::from(&installation);
    app.emit("installation-changed", ()).map_err(|e| e.to_string())?;
    Ok(dto)
}

#[tauri::command]
pub async fn uninstall(
    installation: InstallationDto,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    let installation = installation_from_dto(installation)?;
    state
        .service
        .uninstall(&installation)
        .await
        .map_err(|e| e.to_string())?;
    app.emit("installation-changed", ()).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn enable(
    installation: InstallationDto,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let installation = installation_from_dto(installation)?;
    state
        .service
        .enable(&installation)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn disable(
    installation: InstallationDto,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let installation = installation_from_dto(installation)?;
    state
        .service
        .disable(&installation)
        .await
        .map_err(|e| e.to_string())
}

fn installation_from_dto(dto: InstallationDto) -> Result<Installation, String> {
    use std::path::PathBuf;
    use chrono::DateTime;

    let id = Uuid::parse_str(&dto.id).map_err(|e| e.to_string())?;
    let artifact_id = Uuid::parse_str(&dto.artifact_id).map_err(|e| e.to_string())?;
    let target: Target = dto.target.try_into()?;
    let status = if dto.status == "enabled" {
        Status::Enabled
    } else if dto.status == "disabled" {
        Status::Disabled
    } else if let Some(reason) = dto.status.strip_prefix("broken:") {
        Status::Broken {
            reason: reason.to_string(),
        }
    } else {
        Status::Enabled
    };
    let installed_at = DateTime::parse_from_rfc3339(&dto.installed_at)
        .map_err(|e| e.to_string())?
        .into();

    Ok(Installation {
        id,
        artifact_id,
        target,
        status,
        on_disk_path: PathBuf::from(dto.on_disk_path),
        installed_at,
        installed_version: dto.installed_version,
    })
}
