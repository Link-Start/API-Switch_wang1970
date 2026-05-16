use crate::error::AppError;
use crate::proxy::protocol::get_adapter;
use crate::services::log_service::{insert_test_usage_log, TestUsageLogInput};
use crate::AppState;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Instant;
use tauri::State;

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

#[derive(Debug, Serialize, Deserialize)]
pub struct TestChatMessage {
    pub role: String,
    pub content: String,
}

#[tauri::command]
pub async fn test_chat(
    state: State<'_, AppState>,
    entry_id: String,
    messages: Vec<TestChatMessage>,
) -> Result<TestChatResponse, AppError> {
    let db = state.db.clone();

    // Get the entry directly (all entries, not just enabled ones)
    let entries = db.get_entries_for_routing_all()?;
    let entry = entries
        .iter()
        .find(|e| e.id == entry_id)
        .ok_or_else(|| AppError::NotFound(format!("Entry {entry_id} not found")))?
        .clone();

    // Get channel info
    let channel = db.get_channel(&entry.channel_id)?;

    // Get protocol adapter
    let adapter = get_adapter(&channel.api_type);

    // Build URL and transform request
    let url = adapter.build_chat_url(&channel.base_url, &entry.model);
    let mut upstream_body = json!({
        "model": entry.model,
        "messages": messages,
        "stream": false,
    });
    adapter.transform_request(&mut upstream_body, &entry.model);

    let start = Instant::now();

    // Send request directly to upstream
    let client = reqwest::Client::new();
    let request = adapter
        .apply_auth(client.post(&url), &channel.api_key)
        .json(&upstream_body);

    let response = match request.send().await {
        Ok(response) => response,
        Err(e) => {
            let latency_ms = start.elapsed().as_millis() as i64;
            let message = format!("Request failed: {e}");
            insert_test_usage_log(
                &db,
                None,
                TestUsageLogInput {
                    entry: &entry,
                    channel: &channel,
                    operation: "test_chat",
                    log_group: "test_chat",
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    latency_ms,
                    status_code: 502,
                    success: false,
                    error_message: Some(&message),
                    error_kind: Some("network_error"),
                    response_ms: None,
                    error_preview: None,
                },
            );
            return Err(AppError::Network(message));
        }
    };

    if !response.status().is_success() {
        let latency_ms = start.elapsed().as_millis() as i64;
        let status = response.status();
        let status_code = status.as_u16() as i32;
        let body = response.text().await.unwrap_or_default();
        let error_message = format!("Upstream error {status}: {body}");
        let log_message = format!("upstream_http_{}", status.as_u16());
        insert_test_usage_log(
            &db,
            None,
            TestUsageLogInput {
                entry: &entry,
                channel: &channel,
                operation: "test_chat",
                log_group: "test_chat",
                prompt_tokens: 0,
                completion_tokens: 0,
                latency_ms,
                status_code,
                success: false,
                error_message: Some(&log_message),
                error_kind: Some("http_error"),
                response_ms: None,
                error_preview: Some(&body),
            },

        );
        return Err(AppError::Proxy(error_message));
    }

    let latency_ms = start.elapsed().as_millis() as u64;

    let json_body: serde_json::Value = match response.json().await {
        Ok(body) => body,
        Err(e) => {
            let message = format!("Failed to parse response: {e}");
            insert_test_usage_log(
                &db,
                None,
                TestUsageLogInput {
                    entry: &entry,
                    channel: &channel,
                    operation: "test_chat",
                    log_group: "test_chat",
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    latency_ms: latency_ms as i64,
                    status_code: 502,
                    success: false,
                    error_message: Some(&message),
                    error_kind: Some("parse_error"),
                    response_ms: None,
                    error_preview: None,
                },
            );
            return Err(AppError::Internal(message));
        }
    };

    // Transform response if needed (e.g. Claude → OpenAI format)
    let mut json_body = json_body;
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

    // Extract usage
    let usage = json_body.get("usage").map(|u| TestChatUsage {
        prompt_tokens: u.get("prompt_tokens").and_then(|v| v.as_i64()).unwrap_or(0),
        completion_tokens: u
            .get("completion_tokens")
            .and_then(|v| v.as_i64())
            .unwrap_or(0),
        total_tokens: u.get("total_tokens").and_then(|v| v.as_i64()).unwrap_or(0),
    });

    insert_test_usage_log(
        &db,
        None,
        TestUsageLogInput {
            entry: &entry,
            channel: &channel,
            operation: "test_chat",
            log_group: "test_chat",
            prompt_tokens: usage.as_ref().map(|u| u.prompt_tokens).unwrap_or(0),
            completion_tokens: usage.as_ref().map(|u| u.completion_tokens).unwrap_or(0),
            latency_ms: latency_ms as i64,
            status_code: 200,
            success: true,
            error_message: None,
            error_kind: None,
            response_ms: Some(&latency_ms.to_string()),
            error_preview: None,
        },
    );

    Ok(TestChatResponse {
        content,
        latency_ms,
        usage,
    })
}
