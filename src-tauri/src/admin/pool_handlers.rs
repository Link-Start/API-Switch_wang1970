use crate::admin::error::AdminError;
use crate::admin::state::AdminState;
use crate::database::dao::PaginatedResult;
use crate::database::ApiEntry;
use crate::services::pool_service;
use axum::extract::{Json, Path, Query, State};
use serde::Deserialize;
use serde_json::Value;

// ---------- Request/Response Types -----------------------------------------

#[derive(Deserialize)]
pub struct CreateEntryParams {
    pub channel_id: String,
    pub model: String,
    pub display_name: Option<String>,
    #[serde(default)]
    pub provider_logo: String,
    #[serde(default)]
    pub release_date: String,
    #[serde(default)]
    pub model_meta_zh: String,
    #[serde(default)]
    pub model_meta_en: String,
    #[serde(default)]
    pub group_name: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogMetaUpdate {
    pub id: String,
    #[serde(default)]
    pub display_name: String,
    pub provider_logo: String,
    pub release_date: String,
    pub model_meta_zh: String,
    pub model_meta_en: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReorderParams {
    pub ordered_ids: Vec<String>,
}

#[derive(Deserialize)]
pub struct TestLatencyParams {
    #[serde(default)]
    pub model_score: f64,
}

// ---------- Handlers -------------------------------------------------------

/// GET /admin/pool - List all API entries
pub async fn list(State(state): State<AdminState>) -> Result<Json<Vec<ApiEntry>>, AdminError> {
    let entries = state.server_api()?.list_entries()?;
    Ok(Json(entries))
}

#[derive(Deserialize)]
pub struct PoolPageParams {
    pub page: Option<i32>,
    pub page_size: Option<i32>,
    pub group_name: Option<String>,
    pub search: Option<String>,
    pub channel_id: Option<String>,
}

/// GET /admin/pool/paginated - List API entries with pagination
pub async fn list_paginated(
    State(state): State<AdminState>,
    Query(params): Query<PoolPageParams>,
) -> Result<Json<PaginatedResult<ApiEntry>>, AdminError> {
    let entries = state.server_api()?.list_entries_paginated(
        params.page.unwrap_or(1),
        params.page_size.unwrap_or(20),
        params.group_name.as_deref(),
        params.search.as_deref(),
        params.channel_id.as_deref(),
    )?;
    Ok(Json(entries))
}

/// POST /admin/pool - Create a new API entry
pub async fn create(
    State(state): State<AdminState>,
    Json(payload): Json<CreateEntryParams>,
) -> Result<Json<ApiEntry>, AdminError> {
    let entry = state
        .server_api()?
        .create_entry(pool_service::CreateEntryParams {
            channel_id: payload.channel_id,
            model: payload.model,
            display_name: payload.display_name,
            provider_logo: payload.provider_logo,
            release_date: payload.release_date,
            model_meta_zh: payload.model_meta_zh,
            model_meta_en: payload.model_meta_en,
            group_name: payload.group_name,
        })?;
    Ok(Json(entry))
}

/// PUT /admin/pool/:id/toggle - Toggle an entry's enabled state
pub async fn toggle(
    State(state): State<AdminState>,
    Path(id): Path<String>,
    Json(payload): Json<Value>,
) -> Result<Json<serde_json::Value>, AdminError> {
    let (enabled, pin_to_top) = match payload {
        Value::Bool(enabled) => (enabled, false),
        Value::Object(map) => (
            map.get("enabled").and_then(Value::as_bool).unwrap_or(false),
            map.get("pinToTop")
                .or_else(|| map.get("pin_to_top"))
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
        _ => (false, false),
    };
    state.server_api()?.toggle_entry(&id, enabled, pin_to_top)?;
    Ok(Json(serde_json::json!({"ok": true})))
}

/// DELETE /admin/pool/:id - Delete an entry by ID
pub async fn delete(
    State(state): State<AdminState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AdminError> {
    state.server_api()?.delete_entry(&id)?;
    Ok(Json(serde_json::json!({"ok": true})))
}

/// POST /admin/pool/reorder - Reorder entries
pub async fn reorder(
    State(state): State<AdminState>,
    Json(payload): Json<ReorderParams>,
) -> Result<Json<serde_json::Value>, AdminError> {
    state.server_api()?.reorder_entries(&payload.ordered_ids)?;
    Ok(Json(serde_json::json!({"ok": true})))
}

/// POST /admin/pool/:id/test-latency - Test latency for a specific entry
pub async fn test_latency(
    State(state): State<AdminState>,
    Path(id): Path<String>,
    payload: Option<Json<TestLatencyParams>>,
) -> Result<Json<pool_service::TestLatencyResult>, AdminError> {
    let model_score = payload
        .map(|Json(params)| params.model_score)
        .unwrap_or(0.0);
    let result = state
        .server_api()?
        .test_entry_latency(&id, model_score)
        .await?;
    Ok(Json(result))
}

/// POST /admin/pool/backfill-catalog-meta - Backfill catalog metadata for multiple entries
pub async fn backfill_catalog_meta(
    State(state): State<AdminState>,
    Json(payload): Json<Vec<CatalogMetaUpdate>>,
) -> Result<Json<serde_json::Value>, AdminError> {
    let updates: Vec<pool_service::CatalogMetaUpdate> = payload
        .into_iter()
        .map(|item| pool_service::CatalogMetaUpdate {
            id: item.id,
            display_name: item.display_name,
            provider_logo: item.provider_logo,
            release_date: item.release_date,
            model_meta_zh: item.model_meta_zh,
            model_meta_en: item.model_meta_en,
        })
        .collect();

    state.server_api()?.backfill_entry_catalog_meta(updates)?;
    Ok(Json(serde_json::json!({"ok": true})))
}

/// GET /admin/pool/groups - Get all distinct group names
pub async fn get_groups(State(state): State<AdminState>) -> Result<Json<Vec<String>>, AdminError> {
    let groups = state.server_api()?.get_all_groups()?;
    Ok(Json(groups))
}

/// PUT /admin/pool/:id/display-name - Update the display_name (alias) for an entry
pub async fn update_display_name(
    State(state): State<AdminState>,
    Path(id): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AdminError> {
    let display_name = payload
        .get("display_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    state
        .server_api()?
        .update_entry_display_name(&id, display_name)?;
    Ok(Json(serde_json::json!({"ok": true})))
}

/// PUT /admin/pool/:id/group - Update the group_name for an entry
pub async fn update_group(
    State(state): State<AdminState>,
    Path(id): Path<String>,
    Json(group_name): Json<String>,
) -> Result<Json<serde_json::Value>, AdminError> {
    state.server_api()?.update_entry_group(&id, &group_name)?;
    Ok(Json(serde_json::json!({"ok": true})))
}
