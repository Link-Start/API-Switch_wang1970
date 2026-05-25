use crate::admin::error::AdminError;
use crate::admin::state::AdminState;
use crate::commands::connection_apps::{
    execute_connection_app_from_parts, list_connection_apps, AppConfigResult, ConnectionAppItem,
};
use axum::extract::{Json, Path, State};

pub async fn list(
    State(_state): State<AdminState>,
) -> Result<Json<Vec<ConnectionAppItem>>, AdminError> {
    Ok(Json(list_connection_apps()?))
}

pub async fn execute(
    State(state): State<AdminState>,
    Path(id): Path<String>,
) -> Result<Json<AppConfigResult>, AdminError> {
    Ok(Json(
        execute_connection_app_from_parts(&state.db, &state.settings, &id, false).await?,
    ))
}
