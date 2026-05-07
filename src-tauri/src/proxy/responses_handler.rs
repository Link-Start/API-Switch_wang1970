//! POST /v1/responses — OpenAI Responses API compatibility layer.
//!
//! Converts Responses API requests to Chat Completions format,
//! forwards non-streaming to the upstream, and wraps the result
//! as a Responses API SSE event stream.

use super::auth;
use super::forwarder;
use super::router;
use super::handlers::ProxyError;
use super::server::ProxyState;
use axum::body::Body;
use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use bytes::Bytes;
use serde_json::{json, Value};
use uuid::Uuid;

// ─── input_to_messages ───────────────────────────────────────────────

/// Convert the Responses API `input` field into a Chat Completions `messages` array.
///
/// The `input` can be:
/// - A plain string → single user message
/// - A list of items (strings, or objects with `type`, `role`, `content`)
/// - An object (rare, treated as a single user message with JSON text)
fn input_to_messages(input: &Value, instructions: Option<&str>) -> Vec<Value> {
    let mut msgs: Vec<Value> = Vec::new();

    // Optional system message from `instructions`
    if let Some(inst) = instructions {
        if !inst.is_empty() {
            msgs.push(json!({ "role": "system", "content": inst }));
        }
    }

    match input {
        // Simple string
        Value::String(s) => {
            msgs.push(json!({ "role": "user", "content": s }));
        }
        // Array of items
        Value::Array(items) => {
            for item in items {
                match item {
                    Value::String(s) => {
                        msgs.push(json!({ "role": "user", "content": s }));
                    }
                    Value::Object(obj) => {
                        let typ = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
                        let role = match obj.get("role") {
                            Some(Value::String(r)) if r == "system" || r == "user" || r == "assistant" || r == "tool" => r.clone(),
                            _ => {
                                // Infer role from type
                                if matches!(typ, "message" | "function_call" | "custom_tool_call") {
                                    "assistant".to_string()
                                } else {
                                    "user".to_string()
                                }
                            }
                        };

                        // Extract text content
                        let text = match obj.get("content") {
                            Some(Value::String(s)) => s.clone(),
                            Some(Value::Array(parts)) => {
                                let mut texts = Vec::new();
                                for p in parts {
                                    match p {
                                        Value::String(s) => texts.push(s.clone()),
                                        Value::Object(o) => {
                                            let t = o.get("text")
                                                .or_else(|| o.get("input_text"))
                                                .or_else(|| o.get("output_text"))
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("");
                                            if !t.is_empty() {
                                                texts.push(t.to_string());
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                texts.join("\n")
                            }
                            _ => continue,
                        };

                        if text.is_empty() {
                            continue;
                        }

                        msgs.push(json!({ "role": role, "content": text }));
                    }
                    _ => {}
                }
            }
        }
        // Object or other — stringify as user message
        other => {
            let text = if other.is_null() {
                "Hello".to_string()
            } else {
                serde_json::to_string(other).unwrap_or_else(|_| "{}".to_string())
            };
            msgs.push(json!({ "role": "user", "content": text }));
        }
    }

    if msgs.is_empty() {
        msgs.push(json!({ "role": "user", "content": "Hello" }));
    }

    msgs
}

// ─── convert_tools ───────────────────────────────────────────────────

/// Convert Responses API tool definitions to Chat Completions format.
///
/// Responses API: `{ type: "function", name, description, parameters, strict }`
/// Chat API:      `{ type: "function", function: { name, description, parameters, strict } }`
fn convert_tools(tools: &[Value]) -> Option<Value> {
    let converted: Vec<Value> = tools
        .iter()
        .filter_map(|t| {
            let typ = t.get("type").and_then(|v| v.as_str())?;
            if typ != "function" {
                return None;
            }

            // If already in Chat format, pass through
            if t.get("function").is_some() {
                return Some(t.clone());
            }

            // Convert from Responses format
            let name = t.get("name").and_then(|v| v.as_str()).unwrap_or("tool");
            let description = t.get("description").and_then(|v| v.as_str()).unwrap_or("");
            let parameters = t.get("parameters").cloned().unwrap_or_else(|| {
                json!({ "type": "object", "properties": {} })
            });

            let mut function = json!({
                "name": name,
                "description": description,
                "parameters": parameters,
            });

            if let Some(strict) = t.get("strict") {
                function["strict"] = strict.clone();
            }

            Some(json!({ "type": "function", "function": function }))
        })
        .collect();

    if converted.is_empty() {
        None
    } else {
        Some(Value::Array(converted))
    }
}

// ─── SSE helpers ─────────────────────────────────────────────────────

fn sse_line(obj: &Value) -> Bytes {
    let line = format!("data: {}\n\n", serde_json::to_string(obj).unwrap_or_default());
    Bytes::from(line)
}

fn sse_done() -> Bytes {
    Bytes::from("data: [DONE]\n\n")
}

// ─── Handler ─────────────────────────────────────────────────────────

/// POST /v1/responses — Responses API compatibility endpoint.
///
/// Flow:
/// 1. Authenticate (reuse existing access key logic)
/// 2. Parse Responses API request
/// 3. Convert `input` → Chat messages, `tools` → Chat tools
/// 4. Forward non-streaming to upstream via existing forwarder
/// 5. Wrap result as SSE events in Responses API format
pub async fn handle_responses(
    State(state): State<ProxyState>,
    request: axum::extract::Request,
) -> Result<axum::response::Response, ProxyError> {
    let (parts, body) = request.into_parts();
    let headers = &parts.headers;

    // 1. Auth
    let access_key = auth::extract_access_key(headers, &state)
        .await
        .map_err(|err| match err {
            crate::error::AppError::Validation(_) => ProxyError::Unauthorized,
            other => ProxyError::from(other),
        })?;

    // 2. Parse request body
    let body_bytes = axum::body::to_bytes(body, 32 * 1024 * 1024)
        .await
        .map_err(|e| ProxyError::Internal(format!("Failed to read body: {e}")))?;

    let req_body: Value = serde_json::from_slice(&body_bytes)
        .map_err(|e| ProxyError::Internal(format!("Failed to parse JSON: {e}")))?;

    let response_id = format!("resp_{}", Uuid::new_v4().to_string().replace('-', "")[..24].to_string());
    let model = req_body
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("auto");

    // 3. Convert to Chat Completions format
    let messages = input_to_messages(
        req_body.get("input").unwrap_or(&Value::Null),
        req_body.get("instructions").and_then(|v| v.as_str()),
    );

    let mut chat_body = json!({
        "model": model,
        "messages": messages,
        "stream": false,
    });

    // Convert tools if present
    if let Some(tools) = req_body.get("tools").and_then(|v| v.as_array()) {
        if let Some(converted) = convert_tools(tools) {
            chat_body["tools"] = converted;
            chat_body["tool_choice"] = json!("auto");
        }
    }

    // 4. Route and forward (non-streaming)
    let requested_model = if model == "auto" || model.is_empty() {
        "auto".to_string()
    } else {
        model.to_string()
    };

    let all_entries = state.db.get_entries_for_routing()?;
    let auto_entries = state.db.get_enabled_entries_for_auto()?;
    let sort_mode = state.settings.read().await.default_sort_mode.clone();
    let resolved = router::resolve(
        &requested_model,
        &all_entries,
        &auto_entries,
        &state.circuit_breakers,
        &sort_mode,
    )
    .await;

    if resolved.is_empty() {
        return Err(ProxyError::NoAvailableProvider(requested_model));
    }

    let upstream_response = forwarder::forward_with_retry(
        &state,
        &resolved,
        &chat_body,
        headers,
        &requested_model,
        access_key.as_ref(),
        false, // non-streaming
    )
    .await;

    // 5. Build SSE response stream
    let item_id = format!("msg_{}", Uuid::new_v4().to_string().replace('-', "")[..16].to_string());

    // Collect all SSE frames into a Vec for streaming
    let mut frames: Vec<Bytes> = Vec::new();

    // response.created
    frames.push(sse_line(&json!({
        "type": "response.created",
        "response": { "id": &response_id }
    })));

    match upstream_response {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body_bytes = match axum::body::to_bytes(resp.into_body(), 32 * 1024 * 1024).await {
                Ok(b) => b,
                Err(_) => {
                    frames.push(sse_line(&json!({
                        "type": "response.failed",
                        "response": { "id": &response_id, "error": { "message": "Failed to read upstream body", "type": "upstream_error" } }
                    })));
                    frames.push(sse_done());
                    return build_sse_response(frames);
                }
            };

            if status != 200 {
                let err_text = String::from_utf8_lossy(&body_bytes).chars().take(2000).collect::<String>();
                frames.push(sse_line(&json!({
                    "type": "response.failed",
                    "response": { "id": &response_id, "error": { "message": err_text, "type": "upstream_error" } }
                })));
                frames.push(sse_done());
                return build_sse_response(frames);
            }

            // Parse upstream Chat Completions response
            let obj: Value = serde_json::from_slice(&body_bytes).unwrap_or_else(|_| {
                json!({ "choices": [{ "message": { "content": String::from_utf8_lossy(&body_bytes) } }] })
            });

            let msg = obj.get("choices")
                .and_then(|c| c.as_array())
                .and_then(|a| a.first())
                .and_then(|c| c.get("message"))
                .cloned()
                .unwrap_or_else(|| json!({}));

            let content = msg.get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            // output_text.delta with the full content
            if !content.is_empty() {
                frames.push(sse_line(&json!({
                    "type": "response.output_text.delta",
                    "delta": content
                })));
            }

            // output_item.done
            if !content.is_empty() {
                frames.push(sse_line(&json!({
                    "type": "response.output_item.done",
                    "item": {
                        "type": "message",
                        "role": "assistant",
                        "id": &item_id,
                        "content": [{ "type": "output_text", "text": content }]
                    }
                })));
            }

            // Usage
            let usage = obj.get("usage").cloned().unwrap_or_else(|| json!({}));
            let input_tokens = usage.get("prompt_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
            let output_tokens = usage.get("completion_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
            let total_tokens = usage.get("total_tokens").and_then(|v| v.as_i64()).unwrap_or(input_tokens + output_tokens);

            // response.completed
            frames.push(sse_line(&json!({
                "type": "response.completed",
                "response": {
                    "id": &response_id,
                    "usage": {
                        "input_tokens": input_tokens,
                        "output_tokens": output_tokens,
                        "total_tokens": total_tokens,
                        "input_tokens_details": null,
                        "output_tokens_details": null
                    }
                }
            })));
        }
        Err(e) => {
            frames.push(sse_line(&json!({
                "type": "response.failed",
                "response": { "id": &response_id, "error": { "message": format!("{e}"), "type": "proxy_error" } }
            })));
        }
    }

    frames.push(sse_done());
    build_sse_response(frames)
}

/// Build an SSE response from pre-collected frames using a streaming channel.
fn build_sse_response(frames: Vec<Bytes>) -> Result<axum::response::Response, ProxyError> {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(frames.len());

    // Send all frames in background, then drop the sender
    tokio::spawn(async move {
        for frame in frames {
            if tx.send(Ok(frame)).await.is_err() {
                break;
            }
        }
        // Sender dropped → stream ends
    });

    let stream = futures::stream::unfold(rx, |mut rx| async move {
        let item = rx.recv().await?;
        Some((item, rx))
    });

    let body = Body::from_stream(stream);

    let response = axum::http::Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-cache")
        .header(header::CONNECTION, "close")
        .body(body)
        .map_err(|e| ProxyError::Internal(format!("Failed to build response: {e}")))?;

    Ok(response)
}
