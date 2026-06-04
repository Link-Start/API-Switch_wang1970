use crate::admin::error::AdminError;
use crate::admin::state::AdminState;
use crate::services::import_export_service::{ImportPreview, ImportResult};
use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResponse {
    pub payload: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPayloadRequest {
    pub payload: String,
}

pub async fn export_channel_model_transfer(
    State(state): State<AdminState>,
) -> Result<Json<ExportResponse>, AdminError> {
    let payload = state.server_api()?.export_channel_model_transfer()?;

    Ok(Json(ExportResponse { payload }))
}

pub async fn preview_channel_model_transfer(
    State(state): State<AdminState>,
    Json(request): Json<ImportPayloadRequest>,
) -> Result<Json<ImportPreview>, AdminError> {
    Ok(Json(
        state
            .server_api()?
            .preview_channel_model_transfer(&request.payload)?,
    ))
}

pub async fn import_channel_model_transfer(
    State(state): State<AdminState>,
    Json(request): Json<ImportPayloadRequest>,
) -> Result<Json<ImportResult>, AdminError> {
    Ok(Json(
        state
            .server_api()?
            .import_channel_model_transfer(&request.payload)?,
    ))
}
