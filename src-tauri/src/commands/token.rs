use crate::database::AccessKey;
use crate::error::AppError;
use crate::services::token_service;
use crate::AppState;
use tauri::{AppHandle, State};

#[tauri::command]
pub fn list_access_keys(state: State<'_, AppState>) -> Result<Vec<AccessKey>, AppError> {
    token_service::list_access_keys(&state.db)
}

#[tauri::command]
pub fn create_access_key(state: State<'_, AppState>, name: String) -> Result<AccessKey, AppError> {
    token_service::create_access_key(&state.db, &name)
}

#[tauri::command]
pub fn delete_access_key(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), AppError> {
    token_service::delete_access_key(&state.db, &id, Some(&app))
}

#[tauri::command]
pub fn toggle_access_key(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<(), AppError> {
    token_service::toggle_access_key(&state.db, &id, enabled, Some(&app))
}
