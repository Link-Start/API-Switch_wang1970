use crate::error::AppError;
use crate::services::import_export_service::{ImportPreview, ImportResult};
use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn export_channel_model_transfer(
    app: crate::AppEventHandle,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    let api = crate::server_api::ServerApi::new(state.inner().clone(), Some(app));
    api.export_channel_model_transfer()
}

#[tauri::command]
pub fn preview_channel_model_transfer(
    app: crate::AppEventHandle,
    state: State<'_, AppState>,
    payload: String,
) -> Result<ImportPreview, AppError> {
    let api = crate::server_api::ServerApi::new(state.inner().clone(), Some(app));
    api.preview_channel_model_transfer(&payload)
}

#[tauri::command]
pub fn import_channel_model_transfer(
    app: crate::AppEventHandle,
    state: State<'_, AppState>,
    payload: String,
) -> Result<ImportResult, AppError> {
    let api = crate::server_api::ServerApi::new(state.inner().clone(), Some(app));
    api.import_channel_model_transfer(&payload)
}
