use std::time::Duration;

use skillsmgr_core::{Installation, Scope, Status, Target};
use skillsmgr_fetch::ImportPreview;
use skillsmgr_translate::{
    build_provider, keyring_store, OpenAICompatProvider, ProviderKind, TranslationProvider,
    TranslationRequest,
};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::dto::{
    ErrorDto, ImportPreviewDto, InstallationDto, InventoryDto, TargetDto, TranslateConfigDto,
    TranslateOutcomeDto,
};
use crate::state::AppState;

#[tauri::command]
pub async fn scan(
    cwd: Option<String>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<InventoryDto, ErrorDto> {
    let cwd_path = cwd.as_deref().map(std::path::Path::new);
    let inventory = state.service.inventory(cwd_path).await;
    let dto = InventoryDto::from(&inventory);
    app.emit("scan-complete", &dto)
        .map_err(ErrorDto::internal)?;
    Ok(dto)
}

#[tauri::command]
pub async fn preview_import(
    path_or_url: String,
    state: State<'_, AppState>,
) -> Result<ImportPreviewDto, ErrorDto> {
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
    .map_err(ErrorDto::from)?;

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
) -> Result<InstallationDto, ErrorDto> {
    let candidate = {
        let guard = state.pending_import.lock().await;
        let pending = guard.as_ref().ok_or_else(|| ErrorDto {
            code: "noPendingImport".into(),
            params: Default::default(),
        })?;
        pending
            .candidates
            .get(candidate_index)
            .ok_or_else(|| ErrorDto {
                code: "invalidCandidateIndex".into(),
                params: Default::default(),
            })?
            .clone()
    };

    let target: Target = target.try_into().map_err(ErrorDto::internal)?;
    let installation = state
        .service
        .install_from_candidate(&candidate, target, vec![Scope::Global])
        .await
        .map_err(ErrorDto::from)?;

    let dto = InstallationDto::from(&installation);
    app.emit("installation-changed", ())
        .map_err(ErrorDto::internal)?;
    Ok(dto)
}

#[tauri::command]
pub async fn uninstall(
    installation: InstallationDto,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), ErrorDto> {
    let installation = installation_from_dto(installation)?;
    state
        .service
        .uninstall(&installation)
        .await
        .map_err(ErrorDto::from)?;
    app.emit("installation-changed", ())
        .map_err(ErrorDto::internal)?;
    Ok(())
}

#[tauri::command]
pub async fn enable(
    installation: InstallationDto,
    state: State<'_, AppState>,
) -> Result<(), ErrorDto> {
    let installation = installation_from_dto(installation)?;
    state
        .service
        .enable(&installation)
        .await
        .map_err(ErrorDto::from)
}

#[tauri::command]
pub async fn disable(
    installation: InstallationDto,
    state: State<'_, AppState>,
) -> Result<(), ErrorDto> {
    let installation = installation_from_dto(installation)?;
    state
        .service
        .disable(&installation)
        .await
        .map_err(ErrorDto::from)
}

#[tauri::command]
pub async fn translate_artifact(
    artifact_name: String,
    file_path: String,
    field: String,
    source_text: String,
    locale: String,
    force_refresh: Option<bool>,
    state: State<'_, AppState>,
) -> Result<TranslateOutcomeDto, ErrorDto> {
    let request = TranslationRequest {
        artifact_name,
        file_path: std::path::PathBuf::from(file_path),
        field,
        source_text,
        locale,
    };
    let outcome = state
        .translations
        .translate_or_get(request, force_refresh.unwrap_or(false))
        .await
        .map_err(ErrorDto::from)?;
    Ok(TranslateOutcomeDto::from(outcome))
}

#[tauri::command]
pub async fn clear_translation_cache(
    artifact_name: String,
    file_path: String,
    field: String,
    locale: String,
    state: State<'_, AppState>,
) -> Result<usize, ErrorDto> {
    state
        .translations
        .clear_cache(
            &artifact_name,
            std::path::Path::new(&file_path),
            &field,
            &locale,
        )
        .map_err(ErrorDto::from)
}

#[tauri::command]
pub async fn get_translate_config(
    state: State<'_, AppState>,
) -> Result<TranslateConfigDto, ErrorDto> {
    let config = skillsmgr_translate::TranslateConfig::load(&state.translate_config_path)
        .map_err(ErrorDto::from)?;
    let api_key_present = match config.provider_kind {
        ProviderKind::OpenAiCompat => {
            keyring_store::has_api_key(config.provider_kind.as_id()).map_err(ErrorDto::from)?
        }
        ProviderKind::Passthrough => false,
    };
    Ok(TranslateConfigDto::from_parts(&config, api_key_present))
}

#[tauri::command]
pub async fn set_translate_config(
    config: TranslateConfigDto,
    api_key: Option<String>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<TranslateConfigDto, ErrorDto> {
    let parsed = config.to_config()?;
    let provider_id = parsed.provider_kind.as_id().to_string();

    match api_key.as_deref() {
        Some("") => keyring_store::clear_api_key(&provider_id).map_err(ErrorDto::from)?,
        Some(secret) => keyring_store::set_api_key(&provider_id, secret).map_err(ErrorDto::from)?,
        None => {}
    }

    parsed
        .save(&state.translate_config_path)
        .map_err(ErrorDto::from)?;

    let effective_key = match parsed.provider_kind {
        ProviderKind::OpenAiCompat => {
            keyring_store::get_api_key(&provider_id).map_err(ErrorDto::from)?
        }
        ProviderKind::Passthrough => None,
    };
    let provider = build_provider(&parsed, effective_key.clone()).map_err(ErrorDto::from)?;
    state.translations.swap_provider(provider);

    let api_key_present = effective_key.is_some();
    app.emit("translate-config-changed", ())
        .map_err(ErrorDto::internal)?;
    Ok(TranslateConfigDto::from_parts(&parsed, api_key_present))
}

#[tauri::command]
pub async fn test_translate_provider(
    config: TranslateConfigDto,
    api_key: Option<String>,
) -> Result<String, ErrorDto> {
    let parsed = config.to_config()?;
    match parsed.provider_kind {
        ProviderKind::Passthrough => Ok("Hello".to_string()),
        ProviderKind::OpenAiCompat => {
            let key = match api_key {
                Some(k) if !k.is_empty() => k,
                _ => keyring_store::get_api_key(parsed.provider_kind.as_id())
                    .map_err(ErrorDto::from)?
                    .ok_or_else(|| ErrorDto {
                        code: "translateConfig".into(),
                        params: [(
                            "reason".into(),
                            "no API key provided and none stored".into(),
                        )]
                        .into(),
                    })?,
            };
            let provider = OpenAICompatProvider::new(
                parsed.base_url,
                parsed.model,
                key,
                Duration::from_millis(parsed.timeout_ms),
                parsed.max_retries,
            )
            .map_err(ErrorDto::from)?;
            let request = TranslationRequest {
                artifact_name: "__smoke__".into(),
                file_path: std::path::PathBuf::from("SKILL.md"),
                field: "body".into(),
                source_text: "Hello".into(),
                locale: "zh".into(),
            };
            provider.translate(&request).await.map_err(ErrorDto::from)
        }
    }
}

fn installation_from_dto(dto: InstallationDto) -> Result<Installation, ErrorDto> {
    use chrono::DateTime;
    use std::path::PathBuf;

    let id = Uuid::parse_str(&dto.id).map_err(ErrorDto::internal)?;
    let artifact_id = Uuid::parse_str(&dto.artifact_id).map_err(ErrorDto::internal)?;
    let target: Target = dto.target.try_into().map_err(ErrorDto::internal)?;
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
        .map_err(ErrorDto::internal)?
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
