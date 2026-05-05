// Handler for test chat via admin API
use crate::admin::error::AdminError;
use crate::admin::state::AdminState;
use crate::proxy::protocol::get_adapter;
use axum::{extract::{Json, State}};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use serde_json::json;

#[derive(Debug, Serialize, Deserialize)]
pub struct TestChatRequest {
    pub entry_id: String,
    pub messages: Vec<TestChatMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TestChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct TestChatResponse {
    pub content: String,
    pub latency_ms: u64,
    pub usage: Option<TestChatUsage>,
}

#[derive(Debug, Serialize)]
pub struct TestChatUsage {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

pub async fn test_chat(
    State(state): State<AdminState>,
    Json(payload): Json<TestChatRequest>,
) -> Result<Json<TestChatResponse>, AdminError> {
    // Ensure runtime is available
    let runtime = state.runtime.as_ref().ok_or_else(|| {
        AdminError::BadRequest("Admin runtime not initialized".to_string())
    })?;
    let db = runtime.db.clone();

    // Get all entries for routing (including disabled)
    let entries = db.get_entries_for_routing_all()?;
    let entry = entries
        .iter()
        .find(|e| e.id == payload.entry_id)
        .ok_or_else(|| AdminError::NotFound(format!("Entry {} not found", payload.entry_id)))?
        .clone();

    // Get channel info
    let channel = db.get_channel(&entry.channel_id)?;

    // Get protocol adapter
    let adapter = get_adapter(&channel.api_type);

    // Build URL and request body
    let url = adapter.build_chat_url(&channel.base_url, &entry.model);
    let mut upstream_body = json!({
        "model": entry.model,
        "messages": payload.messages,
        "stream": false,
    });
    adapter.transform_request(&mut upstream_body, &entry.model);

    let start = Instant::now();
    let client = reqwest::Client::new();
    let request = adapter.apply_auth(client.post(&url), &channel.api_key).json(&upstream_body);
    let response = request
        .send()
        .await
        .map_err(|e| AdminError::Internal(format!("Network request failed: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AdminError::Internal(format!("Upstream error {status}: {body}")));
    }

    let latency_ms = start.elapsed().as_millis() as u64;
    let mut json_body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AdminError::Internal(format!("Failed to parse response: {}", e)))?;
    // Transform response if needed (e.g., Claude -> OpenAI format)
    adapter.transform_response(&mut json_body);

    // Extract content
    let content = json_body
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();

    // Extract usage if present
    let usage = json_body.get("usage").map(|u| TestChatUsage {
        prompt_tokens: u.get("prompt_tokens").and_then(|v| v.as_i64()).unwrap_or(0),
        completion_tokens: u.get("completion_tokens").and_then(|v| v.as_i64()).unwrap_or(0),
        total_tokens: u.get("total_tokens").and_then(|v| v.as_i64()).unwrap_or(0),
    });

    Ok(Json(TestChatResponse { content, latency_ms, usage }))
}
