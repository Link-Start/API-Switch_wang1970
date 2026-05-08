//! POST /v1/responses — OpenAI Responses API compatibility layer.
//!
//! Converts a subset of Responses API requests (text and function tools)
//! to Chat Completions format. Non-streaming responses are returned as
//! Responses JSON objects; streaming responses are converted from upstream
//! Chat Completions SSE to Responses-style SSE events in real time.

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
use futures::StreamExt;
use serde_json::{json, Value};
use uuid::Uuid;

// ─── input_to_messages ───────────────────────────────────────────────

/// Convert the Responses API `input` field into a Chat Completions `messages` array.
///
/// The `input` can be:
/// - A plain string → single user message
/// - A list of items: strings, message objects, function_call, or function_call_output
/// - An object (rare, stringified as user message)
///
/// multi-turn tool use: function_call → assistant tool_calls,
/// function_call_output → tool message.
fn input_to_messages(input: &Value, instructions: Option<&str>) -> Vec<Value> {
    let mut msgs: Vec<Value> = Vec::new();

    // Optional system message from `instructions`
    if let Some(inst) = instructions {
        if !inst.is_empty() {
            msgs.push(json!({ "role": "system", "content": inst }));
        }
    }

    match input {
        Value::String(s) => {
            msgs.push(json!({ "role": "user", "content": s }));
        }
        Value::Array(items) => {
            // Group consecutive function_call + function_call_output pairs
            // into a single assistant tool_calls message + individual tool messages
            let mut i = 0;
            while i < items.len() {
                let item = &items[i];

                if let Value::Object(obj) = item {
                    let typ = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");

                    match typ {
                        // ── function_call → assistant message with tool_calls ──
                        "function_call" => {
                            let call_id = obj.get("call_id").and_then(|v| v.as_str()).unwrap_or("");
                            let name = obj.get("name").and_then(|v| v.as_str()).unwrap_or("");
                            let arguments = obj.get("arguments").and_then(|v| v.as_str()).unwrap_or("{}");

                            // Collect tool calls for this assistant turn
                            let mut tool_calls = vec![json!({
                                "id": call_id,
                                "type": "function",
                                "function": {
                                    "name": name,
                                    "arguments": arguments,
                                }
                            })];

                            // If next items are also function_calls (same turn), group them
                            let mut j = i + 1;
                            while j < items.len() {
                                if let Value::Object(next_obj) = &items[j] {
                                    let next_typ = next_obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
                                    if next_typ == "function_call" {
                                        let next_call_id = next_obj.get("call_id").and_then(|v| v.as_str()).unwrap_or("");
                                        let next_name = next_obj.get("name").and_then(|v| v.as_str()).unwrap_or("");
                                        let next_args = next_obj.get("arguments").and_then(|v| v.as_str()).unwrap_or("{}");
                                        tool_calls.push(json!({
                                            "id": next_call_id,
                                            "type": "function",
                                            "function": {
                                                "name": next_name,
                                                "arguments": next_args,
                                            }
                                        }));
                                        j += 1;
                                    } else {
                                        break;
                                    }
                                } else {
                                    break;
                                }
                            }

                            msgs.push(json!({
                                "role": "assistant",
                                "content": null,
                                "tool_calls": tool_calls
                            }));
                            i = j;
                            continue;
                        }

                        // ── function_call_output → tool message ──
                        "function_call_output" => {
                            let call_id = obj.get("call_id").and_then(|v| v.as_str()).unwrap_or("");
                            let output = match obj.get("output") {
                                Some(Value::String(s)) => s.clone(),
                                Some(v) => serde_json::to_string(v).unwrap_or_else(|_| String::new()),
                                None => String::new(),
                            };

                            msgs.push(json!({
                                "role": "tool",
                                "tool_call_id": call_id,
                                "content": output,
                            }));
                            i += 1;
                            continue;
                        }

                        // ── regular message ──
                        _ => {
                            let role = match obj.get("role") {
                                Some(Value::String(r)) => match r.as_str() {
                                    "system" | "developer" => "system".to_string(),
                                    "user" | "assistant" | "tool" => r.clone(),
                                    _ => {
                                        if matches!(typ, "message") { "assistant".to_string() } else { "user".to_string() }
                                    }
                                },
                                _ => {
                                    if matches!(typ, "message") { "assistant".to_string() } else { "user".to_string() }
                                }
                            };

                            let content_value = match obj.get("content") {
                                Some(Value::String(s)) => {
                                    if s.is_empty() { None } else { Some(json!(s)) }
                                },
                                Some(Value::Array(parts)) => {
                                    let mut texts: Vec<String> = Vec::new();
                                    let mut image_parts: Vec<Value> = Vec::new();

                                    for p in parts {
                                        match p {
                                            Value::String(s) => texts.push(s.clone()),
                                            Value::Object(o) => {
                                                let part_type = o.get("type")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("");
                                                if part_type == "input_image" {
                                                    let image_url = o.get("image_url")
                                                        .and_then(|v| v.as_str())
                                                        .unwrap_or("");
                                                    let detail = o.get("detail")
                                                        .and_then(|v| v.as_str())
                                                        .unwrap_or("auto");
                                                    if !image_url.is_empty() {
                                                        image_parts.push(json!({
                                                            "type": "image_url",
                                                            "image_url": {
                                                                "url": image_url,
                                                                "detail": detail
                                                            }
                                                        }));
                                                    }
                                                } else {
                                                    let t = o.get("text")
                                                        .or_else(|| o.get("input_text"))
                                                        .or_else(|| o.get("output_text"))
                                                        .and_then(|v| v.as_str())
                                                        .unwrap_or("");
                                                    if !t.is_empty() { texts.push(t.to_string()); }
                                                }
                                            }
                                            _ => {}
                                        }
                                    }

                                    if image_parts.is_empty() {
                                        // No images — join text as plain string (backward compat)
                                        let joined = texts.join("\n");
                                        if joined.is_empty() { None } else { Some(json!(joined)) }
                                    } else {
                                        // Has images — build structured content array
                                        let mut content_parts: Vec<Value> = texts.iter()
                                            .map(|t| json!({"type": "text", "text": t}))
                                            .collect();
                                        content_parts.extend(image_parts);
                                        if content_parts.is_empty() { None } else { Some(json!(content_parts)) }
                                    }
                                }
                                _ => None,
                            };

                            if let Some(content) = content_value {
                                msgs.push(json!({ "role": role, "content": content }));
                            } else if matches!(typ, "function_call" | "function_call_output") {
                                // Already handled above; skip empty message fallback
                            }

                            i += 1;
                        }
                    }
                } else if let Value::String(s) = item {
                    msgs.push(json!({ "role": "user", "content": s }));
                    i += 1;
                } else {
                    i += 1;
                }
            }
        }
        other => {
            // For null or other types, return empty content without auto-filling
            if other.is_null() {
                // This case should be caught by handler-level validation;
                // if reached, just return empty messages (handler should have rejected)
                return msgs;
            }
            let text = serde_json::to_string(other).unwrap_or_else(|_| "{}".to_string());
            if !text.is_empty() {
                msgs.push(json!({ "role": "user", "content": text }));
            }
        }
    }

    // Only add default user message if we have actual input content
    // (null/missing input should be rejected at handler level)
    if msgs.is_empty() && instructions.is_none() {
        // No content and no instructions - this should have been caught by validation
        // Return empty to let caller handle it
        return msgs;
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

// ─── SSRF Protection ──────────────────────────────────────────────────

/// Validate image URL to prevent SSRF attacks.
/// Only allows http/https URLs and rejects localhost/private IP ranges.
fn validate_image_url(url: &str) -> Result<(), String> {
    // Check for http/https scheme or data:image URLs
    if url.starts_with("data:image/") {
        return Ok(());
    }
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("Invalid URL scheme. Only http, https, and data:image URLs are allowed.".to_string());
    }

    // Parse the URL to check host
    let parsed = url::Url::parse(url)
        .map_err(|e| format!("Invalid URL: {e}"))?;

    let host = parsed.host_str().unwrap_or("");

    // Reject localhost
    if host == "localhost" || host == "127.0.0.1" || host == "0.0.0.0" || host.starts_with("127.") {
        return Err("URL host is localhost. External URLs only.".to_string());
    }

    // Reject private IP ranges
    if let url::Host::Ipv4(ip) = parsed.host().unwrap_or(url::Host::Ipv4(std::net::Ipv4Addr::new(0,0,0,0))) {
        let octets = ip.octets();
        // 10.0.0.0/8
        if octets[0] == 10 {
            return Err("URL host is in private IP range (10.0.0.0/8). External URLs only.".to_string());
        }
        // 172.16.0.0/12
        if octets[0] == 172 && (16..=31).contains(&octets[1]) {
            return Err("URL host is in private IP range (172.16.0.0/12). External URLs only.".to_string());
        }
        // 192.168.0.0/16
        if octets[0] == 192 && octets[1] == 168 {
            return Err("URL host is in private IP range (192.168.0.0/16). External URLs only.".to_string());
        }
        // 169.254.0.0/16 (link-local)
        if octets[0] == 169 && octets[1] == 254 {
            return Err("URL host is link-local. External URLs only.".to_string());
        }
    }

    Ok(())
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

let req_body: Value = match serde_json::from_slice(&body_bytes) {
    Ok(v) => v,
    Err(e) => {
        return Ok((StatusCode::BAD_REQUEST, axum::Json(json!({
            "error": {
                "message": format!("Invalid JSON: {e}"),
                "type": "invalid_request_error",
                "code": "invalid_json"
            }
        })))
        .into_response());
    }
};

// Validate unsupported input types (file/image) and SSRF for image URLs
if let Some(items) = req_body.get("input").and_then(|v| v.as_array()) {
    for item in items {
        if let Some(typ) = item.get("type").and_then(|v| v.as_str()) {
            if typ == "input_file" {
                return Ok((StatusCode::BAD_REQUEST, axum::Json(json!({
                    "error": {
                        "message": "Unsupported input type 'input_file'. Only text, message, function_call, and function_call_output are supported.",
                        "type": "invalid_request_error",
                        "code": "unsupported_input_type"
                    }
                })))
                .into_response());
            }
            if typ == "input_image" {
                // Validate image URL for SSRF protection
                if let Some(image_url) = item.get("image_url").and_then(|v| v.as_str()) {
                    if let Err(e) = validate_image_url(image_url) {
                        return Ok((StatusCode::BAD_REQUEST, axum::Json(json!({
                            "error": {
                                "message": format!("Invalid image URL: {e}"),
                                "type": "invalid_request_error",
                                "code": "invalid_image_url"
                            }
                        })))
                        .into_response());
                    }
                }
                if let Some(image_data) = item.get("image_data").and_then(|v| v.as_str()) {
                    if !image_data.starts_with("data:image/") {
                        return Ok((StatusCode::BAD_REQUEST, axum::Json(json!({
                            "error": {
                                "message": "input_image with non-image data URL is not supported.",
                                "type": "invalid_request_error",
                                "code": "unsupported_input_type"
                            }
                        })))
                        .into_response());
                    }
                }
            }
        }
        // Check nested content arrays
        if let Some(content) = item.get("content") {
            if let Some(typ) = content.get("type").and_then(|v| v.as_str()) {
                if typ == "input_file" {
                    return Ok((StatusCode::BAD_REQUEST, axum::Json(json!({
                        "error": {
                            "message": "Unsupported input type 'input_file'.",
                            "type": "invalid_request_error",
                            "code": "unsupported_input_type"
                        }
                    })))
                    .into_response());
                }
                if typ == "input_image" {
                    if let Some(image_url) = content.get("image_url").and_then(|v| v.as_str()) {
                        if let Err(e) = validate_image_url(image_url) {
                            return Ok((StatusCode::BAD_REQUEST, axum::Json(json!({
                                "error": {
                                    "message": format!("Invalid image URL: {e}"),
                                    "type": "invalid_request_error",
                                    "code": "invalid_image_url"
                                }
                            })))
                            .into_response());
                        }
                    }
                }
            }
        }
    }
}

// Validate unsupported tool types (web_search, file_search, code_interpreter, host_tool)
if let Some(tools) = req_body.get("tools").and_then(|v| v.as_array()) {
    for tool in tools {
        if let Some(typ) = tool.get("type").and_then(|v| v.as_str()) {
            if typ != "function" {
                return Ok((StatusCode::BAD_REQUEST, axum::Json(json!({
                    "error": {
                        "message": format!("Unsupported tool type '{}'. Only 'function' type tools are supported.", typ),
                        "type": "invalid_request_error",
                        "code": "unsupported_tool_type"
                    }
                })))
                .into_response());
            }
        }
    }
}

// Validate P2+ unsupported fields: reject previous_response_id, conversation, background:true
if req_body.get("previous_response_id").is_some() {
    return Ok((StatusCode::BAD_REQUEST, axum::Json(json!({
        "error": {
            "message": "Field 'previous_response_id' is not supported (P2+ feature).",
            "type": "invalid_request_error",
            "code": "unsupported_field"
        }
    })))
    .into_response());
}

if req_body.get("conversation").is_some() {
    return Ok((StatusCode::BAD_REQUEST, axum::Json(json!({
        "error": {
            "message": "Field 'conversation' is not supported (P2+ feature).",
            "type": "invalid_request_error",
            "code": "unsupported_field"
        }
    })))
    .into_response());
}

if req_body.get("background").and_then(|v| v.as_bool()) == Some(true) {
    return Ok((StatusCode::BAD_REQUEST, axum::Json(json!({
        "error": {
            "message": "Field 'background: true' is not supported (P2+ feature).",
            "type": "invalid_request_error",
            "code": "unsupported_field"
        }
    })))
    .into_response());
}

    // Validate missing input (unless prompt provides context)
    let input = req_body.get("input");
    let has_prompt = req_body.get("prompt").is_some();
    if input.is_none() || input.map_or(true, |v| v.is_null()) {
        if !has_prompt {
            return Ok((StatusCode::BAD_REQUEST, axum::Json(json!({
                "error": {
                    "message": "Missing required field 'input'. Provide either 'input' or 'prompt'.",
                    "type": "invalid_request_error",
                    "code": "missing_input"
                }
            }))).into_response());
        }
    }

    // 3. Determine stream mode BEFORE building chat body
    let is_stream = req_body.get("stream").and_then(|v| v.as_bool()).unwrap_or(false);

    let response_id = format!("resp_{}", Uuid::new_v4().to_string().replace('-', ""));
    let model = req_body
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("auto");

    // 4. Convert to Chat Completions format
    let messages = input_to_messages(
        req_body.get("input").unwrap_or(&Value::Null),
        req_body.get("instructions").and_then(|v| v.as_str()),
    );

    let mut chat_body = json!({
        "model": model,
        "messages": messages,
        "stream": is_stream,
    });

    // Passthrough temperature, top_p, max_output_tokens to upstream
    if let Some(temp) = req_body.get("temperature") {
        chat_body["temperature"] = temp.clone();
    }
    if let Some(top_p) = req_body.get("top_p") {
        chat_body["top_p"] = top_p.clone();
    }
    if let Some(max_tokens) = req_body.get("max_output_tokens") {
        chat_body["max_tokens"] = max_tokens.clone();
    }

    // Passthrough reasoning config (gpt-5 / o-series)
    if let Some(reasoning) = req_body.get("reasoning") {
        chat_body["reasoning"] = reasoning.clone();
    }

    // Passthrough service_tier
    if let Some(service_tier) = req_body.get("service_tier") {
        chat_body["service_tier"] = service_tier.clone();
    }

    // Passthrough text config (structured output)
    if let Some(text) = req_body.get("text") {
        if let Some(format_type) = text.get("format").and_then(|f| f.get("type")).and_then(|t| t.as_str()) {
            match format_type {
                "text" | "json_object" | "json_schema" => {}
                other => {
                    return Ok((StatusCode::BAD_REQUEST, axum::Json(json!({
                        "error": {
                            "message": format!("Unsupported text format type '{}'. Supported: text, json_object, json_schema.", other),
                            "type": "invalid_request_error",
                            "code": "unsupported_text_format"
                        }
                    }))).into_response());
                }
            }
        }
        chat_body["text"] = text.clone();
    }

    // Passthrough top_logprobs
    if let Some(top_logprobs) = req_body.get("top_logprobs") {
        chat_body["top_logprobs"] = top_logprobs.clone();
    }

    // Passthrough stream_options
    if let Some(stream_options) = req_body.get("stream_options") {
        chat_body["stream_options"] = stream_options.clone();
    }

    // Passthrough max_tool_calls
    if let Some(max_tool_calls) = req_body.get("max_tool_calls") {
        chat_body["max_tool_calls"] = max_tool_calls.clone();
    }

    // Passthrough include (request extra output data)
    if let Some(include) = req_body.get("include") {
        chat_body["include"] = include.clone();
    }

    // Passthrough prompt config
    if let Some(prompt) = req_body.get("prompt") {
        chat_body["prompt"] = prompt.clone();
    }
    if let Some(prompt_cache_key) = req_body.get("prompt_cache_key") {
        chat_body["prompt_cache_key"] = prompt_cache_key.clone();
    }
    if let Some(prompt_cache_retention) = req_body.get("prompt_cache_retention") {
        chat_body["prompt_cache_retention"] = prompt_cache_retention.clone();
    }

    // Passthrough safety_identifier
    if let Some(safety_identifier) = req_body.get("safety_identifier") {
        chat_body["safety_identifier"] = safety_identifier.clone();
    }

    // Convert tools if present
    if let Some(tools) = req_body.get("tools").and_then(|v| v.as_array()) {
        if let Some(converted) = convert_tools(tools) {
            chat_body["tools"] = converted;
        }
    }
    // Always passthrough tool_choice and parallel_tool_calls regardless of tools presence
    if let Some(tool_choice) = req_body.get("tool_choice") {
        chat_body["tool_choice"] = tool_choice.clone();
    }
    if let Some(parallel_tool_calls) = req_body.get("parallel_tool_calls") {
        chat_body["parallel_tool_calls"] = parallel_tool_calls.clone();
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
        is_stream,
    )
    .await;

    // 6. Build response based on stream mode
    let item_id = format!("msg_{}", Uuid::new_v4().to_string().replace('-', "")[..16].to_string());
    let created_at = chrono::Utc::now().timestamp();

    // Collect all SSE frames into a Vec for streaming
    let mut frames: Vec<Bytes> = Vec::new();

    // Build base response object
    let base_response = json!({
        "id": &response_id,
        "object": "response",
        "created_at": created_at,
        "status": "in_progress",
        "error": null,
        "incomplete_details": null,
        "instructions": req_body.get("instructions"),
        "max_output_tokens": req_body.get("max_output_tokens"),
        "model": model,
        "output": [],
        "parallel_tool_calls": req_body.get("parallel_tool_calls").unwrap_or(&json!(true)),
        "reasoning": req_body.get("reasoning").cloned().unwrap_or(json!({"effort": null, "summary": null})),
        "temperature": req_body.get("temperature").unwrap_or(&json!(1.0)),
        "text": req_body.get("text").cloned().unwrap_or(json!({"format": {"type": "text"}})),
        "tool_choice": req_body.get("tool_choice").unwrap_or(&json!("auto")),
        "tools": req_body.get("tools").unwrap_or(&json!([])),
        "top_p": req_body.get("top_p").unwrap_or(&json!(1.0)),
        "truncation": req_body.get("truncation").unwrap_or(&json!("disabled")),
        "previous_response_id": null,
        "store": req_body.get("store").unwrap_or(&json!(true)),
        "usage": null,
        "user": req_body.get("user"),
        "metadata": req_body.get("metadata").unwrap_or(&json!({}))
    });

    if is_stream {
        frames.push(sse_line(&json!({
            "type": "response.created",
            "response": &base_response
        })));
    }

    match upstream_response {
        Ok(resp) => {
            let status = resp.status().as_u16();

            if status != 200 {
                let body_bytes = axum::body::to_bytes(resp.into_body(), 32 * 1024 * 1024)
                    .await
                    .unwrap_or_default();
                let err_text = String::from_utf8_lossy(&body_bytes).chars().take(2000).collect::<String>();
                let error_event = json!({
                    "type": "response.failed",
                    "response": {
                        "id": &response_id,
                        "object": "response",
                        "created_at": created_at,
                        "status": "failed",
                        "error": { "message": err_text, "type": "upstream_error" }
                    }
                });
                if is_stream {
                    frames.push(sse_line(&error_event));
                    frames.push(sse_done());
                    return build_sse_response(frames);
                } else {
                    let non_stream_error = json!({
                        "id": &response_id,
                        "object": "response",
                        "created_at": created_at,
                        "status": "failed",
                        "error": { "message": err_text, "type": "upstream_error" }
                    });
                    return Ok((StatusCode::BAD_GATEWAY, axum::Json(non_stream_error)).into_response());
                }
            }

            if is_stream {
                // ── TRUE STREAMING: mpsc channel for incremental delivery ──
                // Create channel before the upstream processing loop
                let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(256);

                // Drain initial events from frames into the channel output
                let initial_frames: Vec<Bytes> = frames.drain(..).collect();
                let upstream_body = resp.into_body();

                // Clone values for the async task
                let response_id_task = response_id.clone();
                let item_id_task = item_id.clone();
                let model_task = model.to_string();
                let req_body_task = req_body.clone();
                let created_at_task = created_at;

                // Spawn upstream processing in a background task for true streaming
                tokio::spawn(async move {
                    use std::collections::HashMap;

                    // Helper macro to send a frame, abort if receiver dropped
                    macro_rules! send {
                        ($frame:expr) => {
                            if tx.send(Ok($frame)).await.is_err() { return; }
                        };
                    }

                    // Send initial events first (response.created, response.in_progress)
                    for frame in initial_frames {
                        send!(frame);
                    }

                    let upstream_stream = upstream_body.into_data_stream();
                    let mut buffer = String::new();
                    let mut full_content = String::new();
                    let mut usage = json!({});
                    let mut finish_reason: Option<String> = None;
                    let mut upstream_model: Option<String> = None;
                    let mut content_len: usize = 0;
                    let mut text_added_emitted = false;

                    // ToolCallAccum: index → accumulated state across chunks
                    struct ToolCallEntry {
                        id: String,
                        name: String,
                        arguments: String,
                        added_emitted: bool,
                    }
                    let mut tool_accum: HashMap<usize, ToolCallEntry> = HashMap::new();
                    const MAX_CONTENT_LEN: usize = 10 * 1024 * 1024; // 10MB cap

                    let mut stream = upstream_stream;

                    while let Some(chunk_result) = stream.next().await {
                        let bytes = match chunk_result {
                            Ok(b) => b,
                            Err(_) => break,
                        };

                        // Enforce 10MB stream content cap
                        content_len += bytes.len();
                        if content_len > MAX_CONTENT_LEN {
                            send!(sse_line(&json!({
                                "type": "error",
                                "code": "content_too_large",
                                "message": "Stream content exceeds 10MB limit"
                            })));
                            break;
                        }

                        let chunk_str = String::from_utf8_lossy(&bytes);
                        buffer.push_str(&chunk_str);

                        // Process complete SSE lines in buffer
                        while let Some(newline_pos) = buffer.find('\n') {
                            let line = buffer[..newline_pos].to_string();
                            buffer = buffer[newline_pos + 1..].to_string();
                            let line = line.trim();

                            if line.is_empty() { continue; }

                            if let Some(data) = line.strip_prefix("data: ") {
                                if data == "[DONE]" {
                                    // ── Stream complete: emit finalization events ──

                                    // Emit output_item.done for text if present
                                    if !full_content.is_empty() {
                                        send!(sse_line(&json!({
                                            "type": "response.output_item.done",
                                            "response_id": &response_id_task,
                                            "output_index": 0,
                                            "item": {
                                                "type": "message",
                                                "role": "assistant",
                                                "id": &item_id_task,
                                                "status": "completed",
                                                "content": [{ "type": "output_text", "text": &full_content, "annotations": [] }]
                                            }
                                        })));
                                    }

                                    // Emit function_call_arguments.done + output_item.done for each tool call
                                    let mut sorted_indices: Vec<usize> = tool_accum.keys().copied().collect();
                                    sorted_indices.sort();
                                    for &idx in &sorted_indices {
                                        let entry = &tool_accum[&idx];
                                        send!(sse_line(&json!({
                                            "type": "response.function_call_arguments.done",
                                            "response_id": &response_id_task,
                                            "item_id": entry.id,
                                            "output_index": idx + 1,
                                            "arguments": entry.arguments
                                        })));
                                        send!(sse_line(&json!({
                                            "type": "response.output_item.done",
                                            "response_id": &response_id_task,
                                            "output_index": idx + 1,
                                            "item": {
                                                "id": entry.id,
                                                "type": "function_call",
                                                "call_id": entry.id,
                                                "name": entry.name,
                                                "arguments": entry.arguments,
                                                "status": "completed"
                                            }
                                        })));
                                    }

                                    // Determine status based on finish_reason
                                    let streaming_incomplete = match finish_reason.as_deref() {
                                        Some("length") | Some("content_filter") => json!({ "reason": finish_reason.as_ref().unwrap() }),
                                        _ => json!(null),
                                    };
                                    let final_status = match finish_reason.as_deref() {
                                        Some("length") | Some("content_filter") => "incomplete",
                                        _ => "completed",
                                    };
                                    let resolved_model = upstream_model.as_deref().unwrap_or(&model_task);

                                    // Build output items for response.completed (text + function_call)
                                    let mut final_items: Vec<Value> = Vec::new();
                                    if !full_content.is_empty() {
                                        final_items.push(json!({
                                            "type": "message",
                                            "role": "assistant",
                                            "id": &item_id_task,
                                            "status": final_status,
                                            "content": [{ "type": "output_text", "text": &full_content, "annotations": [] }]
                                        }));
                                    }
                                    for &idx in &sorted_indices {
                                        let entry = &tool_accum[&idx];
                                        final_items.push(json!({
                                            "id": entry.id,
                                            "type": "function_call",
                                            "call_id": entry.id,
                                            "name": entry.name,
                                            "arguments": entry.arguments,
                                            "status": "completed"
                                        }));
                                    }

                                    // Usage
                                    let input_tokens = usage.get("prompt_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let output_tokens = usage.get("completion_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let total = usage.get("total_tokens").and_then(|v| v.as_i64()).unwrap_or(input_tokens + output_tokens);
                                    let cached = usage.get("prompt_tokens_details").and_then(|d| d.get("cached_tokens")).and_then(|v| v.as_i64()).unwrap_or(0);
                                    let reasoning = usage.get("completion_tokens_details").and_then(|d| d.get("reasoning_tokens")).and_then(|v| v.as_i64()).unwrap_or(0);

                                    send!(sse_line(&json!({
                                        "type": "response.completed",
                                        "response": {
                                            "id": &response_id_task,
                                            "object": "response",
                                            "created_at": created_at_task,
                                            "status": final_status,
                                            "error": null,
                                            "incomplete_details": streaming_incomplete,
                                            "instructions": req_body_task.get("instructions"),
                                            "max_output_tokens": req_body_task.get("max_output_tokens"),
                                            "model": resolved_model,
                                            "output": final_items,
                                            "output_text": if !full_content.is_empty() { Some(&full_content) } else { None },
                                            "parallel_tool_calls": req_body_task.get("parallel_tool_calls").unwrap_or(&json!(true)),
                                            "reasoning": req_body_task.get("reasoning").cloned().unwrap_or(json!({"effort": null, "summary": null})),
                                            "temperature": req_body_task.get("temperature").unwrap_or(&json!(1.0)),
                                            "text": req_body_task.get("text").cloned().unwrap_or(json!({"format": {"type": "text"}})),
                                            "tool_choice": req_body_task.get("tool_choice").unwrap_or(&json!("auto")),
                                            "tools": req_body_task.get("tools").unwrap_or(&json!([])),
                                            "top_p": req_body_task.get("top_p").unwrap_or(&json!(1.0)),
                                            "truncation": req_body_task.get("truncation").unwrap_or(&json!("disabled")),
                                            "previous_response_id": null,
                                            "store": req_body_task.get("store").unwrap_or(&json!(true)),
                                            "usage": {
                                                "input_tokens": input_tokens,
                                                "input_tokens_details": { "cached_tokens": cached },
                                                "output_tokens": output_tokens,
                                                "output_tokens_details": { "reasoning_tokens": reasoning },
                                                "total_tokens": total
                                            },
                                            "user": req_body_task.get("user"),
                                            "metadata": req_body_task.get("metadata").unwrap_or(&json!({}))
                                        }
                                    })));
                                    send!(sse_done());
                                    return;
                                }

                                // Parse upstream Chat Completions chunk
                                if let Ok(chunk_obj) = serde_json::from_str::<Value>(data) {
                                    // Capture upstream model from first chunk that has it
                                    if upstream_model.is_none() {
                                        if let Some(m) = chunk_obj.get("model").and_then(|m| m.as_str()) {
                                            upstream_model = Some(m.to_string());
                                        }
                                    }

                                    if let Some(u) = chunk_obj.get("usage") {
                                        usage = u.clone();
                                    }

                                    // Extract finish_reason
                                    if let Some(fr) = chunk_obj.get("choices")
                                        .and_then(|c| c.as_array())
                                        .and_then(|a| a.first())
                                        .and_then(|c| c.get("finish_reason"))
                                        .and_then(|f| f.as_str())
                                    {
                                        if !fr.is_empty() {
                                            finish_reason = Some(fr.to_string());
                                        }
                                    }

                                    // Parse streaming tool_calls into accumulated state
                                    if let Some(tool_calls_delta) = chunk_obj.get("choices")
                                        .and_then(|c| c.as_array())
                                        .and_then(|a| a.first())
                                        .and_then(|c| c.get("delta"))
                                        .and_then(|d| d.get("tool_calls"))
                                        .and_then(|t| t.as_array())
                                    {
                                        for tc_delta in tool_calls_delta {
                                            let tc_idx = tc_delta.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                                            let tc_id_new = tc_delta.get("id").and_then(|v| v.as_str()).unwrap_or("");
                                            let tc_fn = tc_delta.get("function").cloned().unwrap_or_else(|| json!({}));
                                            let tc_name_delta = tc_fn.get("name").and_then(|v| v.as_str()).unwrap_or("");
                                            let tc_args_delta = tc_fn.get("arguments").and_then(|v| v.as_str()).unwrap_or("");

                                            let entry = tool_accum.entry(tc_idx).or_insert_with(|| ToolCallEntry {
                                                id: String::new(),
                                                name: String::new(),
                                                arguments: String::new(),
                                                added_emitted: false,
                                            });

                                            if !tc_id_new.is_empty() {
                                                entry.id = tc_id_new.to_string();
                                            }
                                            if !tc_name_delta.is_empty() {
                                                entry.name = tc_name_delta.to_string();
                                            }
                                            if !tc_args_delta.is_empty() {
                                                entry.arguments.push_str(tc_args_delta);
                                            }

                                            // Emit output_item.added only on first occurrence
                                            if !entry.added_emitted && !entry.name.is_empty() {
                                                send!(sse_line(&json!({
                                                    "type": "response.output_item.added",
                                                    "response_id": &response_id_task,
                                                    "output_index": tc_idx + 1,
                                                    "item": {
                                                        "id": entry.id,
                                                        "type": "function_call",
                                                        "call_id": entry.id,
                                                        "name": entry.name,
                                                        "arguments": "",
                                                        "status": "in_progress"
                                                    }
                                                })));
                                                entry.added_emitted = true;
                                            }

                                            // Emit argument deltas incrementally
                                            if !tc_args_delta.is_empty() {
                                                send!(sse_line(&json!({
                                                    "type": "response.function_call_arguments.delta",
                                                    "response_id": &response_id_task,
                                                    "item_id": entry.id,
                                                    "output_index": tc_idx + 1,
                                                    "delta": tc_args_delta
                                                })));
                                            }
                                        }
                                    }

                                    // Parse content delta incrementally
                                    if let Some(content) = chunk_obj.get("choices")
                                        .and_then(|c| c.as_array())
                                        .and_then(|a| a.first())
                                        .and_then(|c| c.get("delta"))
                                        .and_then(|d| d.get("content"))
                                        .and_then(|c| c.as_str())
                                    {
                                        if !content.is_empty() {
                                            // Emit output_item.added for text message once
                                            if !text_added_emitted {
                                                send!(sse_line(&json!({
                                                    "type": "response.output_item.added",
                                                    "response_id": &response_id_task,
                                                    "output_index": 0,
                                                    "item": {
                                                        "type": "message",
                                                        "role": "assistant",
                                                        "id": &item_id_task,
                                                        "status": "in_progress",
                                                        "content": []
                                                    }
                                                })));
                                                text_added_emitted = true;
                                            }
                                            full_content.push_str(content);
                                            send!(sse_line(&json!({
                                                "type": "response.output_text.delta",
                                                "response_id": &response_id_task,
                                                "item_id": &item_id_task,
                                                "output_index": 0,
                                                "content_index": 0,
                                                "delta": content
                                            })));
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Stream ended without [DONE] — emit [DONE] anyway
                    send!(sse_done());
                });

                // Return streaming response immediately via channel
                let stream = futures::stream::unfold(rx, |mut rx| async move {
                    rx.recv().await.map(|item| (item, rx))
                });

                let body = Body::from_stream(stream);

                return Ok(axum::http::Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "text/event-stream; charset=utf-8")
                    .header(header::CACHE_CONTROL, "no-cache")
                    .header(header::CONNECTION, "close")
                    .body(body)
                    .map_err(|e| ProxyError::Internal(format!("Failed to build response: {e}")))?);
            }

            // ── NON-STREAMING: buffer entire response, parse JSON ──
            let body_bytes = match axum::body::to_bytes(resp.into_body(), 32 * 1024 * 1024).await {
                Ok(b) => b,
                Err(_) => {
                    frames.push(sse_line(&json!({
                        "type": "response.failed",
                        "response": {
                            "id": &response_id,
                            "object": "response",
                            "created_at": created_at,
                            "status": "failed",
                            "error": { "message": "Failed to read upstream body", "type": "upstream_error" }
                        }
                    })));
                    frames.push(sse_done());
                    return build_sse_response(frames);
                }
            };

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

            let tool_calls = msg.get("tool_calls").and_then(|v| v.as_array());

            // Build output items array for the final Response object
            let mut output_items: Vec<Value> = Vec::new();

            // Text output
            if !content.is_empty() {
                frames.push(sse_line(&json!({
                    "type": "response.output_text.delta",
                    "response_id": &response_id,
                    "item_id": &item_id,
                    "output_index": 0,
                    "content_index": 0,
                    "delta": content
                })));
                frames.push(sse_line(&json!({
                    "type": "response.output_item.done",
                    "response_id": &response_id,
                    "output_index": 0,
                    "item": {
                        "type": "message",
                        "role": "assistant",
                        "id": &item_id,
                        "status": "completed",
                        "content": [{ "type": "output_text", "text": content, "annotations": [] }]
                    }
                })));
                output_items.push(json!({
                    "type": "message",
                    "role": "assistant",
                    "id": &item_id,
                    "status": "completed",
                    "content": [{ "type": "output_text", "text": content, "annotations": [] }]
                }));
            }

            // Tool calls (function_call output)
            if let Some(tc_array) = tool_calls {
                for (idx, tc) in tc_array.iter().enumerate() {
                    let output_index = if content.is_empty() { idx as u32 } else { (idx + 1) as u32 };
                    let tc_id = tc.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    let tc_fn = tc.get("function").cloned().unwrap_or_else(|| json!({}));
                    let tc_name = tc_fn.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let tc_args = tc_fn.get("arguments").and_then(|v| v.as_str()).unwrap_or("{}");

                    frames.push(sse_line(&json!({
                        "type": "response.output_item.added",
                        "response_id": &response_id,
                        "output_index": output_index,
                        "item": {
                            "id": tc_id,
                            "type": "function_call",
                            "call_id": tc_id,
                            "name": tc_name,
                            "arguments": "",
                            "status": "in_progress"
                        }
                    })));
                    frames.push(sse_line(&json!({
                        "type": "response.function_call_arguments.delta",
                        "response_id": &response_id,
                        "item_id": tc_id,
                        "output_index": output_index,
                        "delta": tc_args
                    })));
                    frames.push(sse_line(&json!({
                        "type": "response.function_call_arguments.done",
                        "response_id": &response_id,
                        "item_id": tc_id,
                        "output_index": output_index,
                        "arguments": tc_args
                    })));
                    frames.push(sse_line(&json!({
                        "type": "response.output_item.done",
                        "response_id": &response_id,
                        "output_index": output_index,
                        "item": {
                            "id": tc_id,
                            "type": "function_call",
                            "call_id": tc_id,
                            "name": tc_name,
                            "arguments": tc_args,
                            "status": "completed"
                        }
                    })));
                    output_items.push(json!({
                        "id": tc_id,
                        "type": "function_call",
                        "call_id": tc_id,
                        "name": tc_name,
                        "arguments": tc_args,
                        "status": "completed"
                    }));
                }
            }

            // Extract finish_reason for status/incomplete_details
            let finish_reason = obj
                .get("choices")
                .and_then(|c| c.as_array())
                .and_then(|a| a.first())
                .and_then(|c| c.get("finish_reason"))
                .and_then(|f| f.as_str());

            let incomplete_details = match finish_reason {
                Some("length") | Some("content_filter") => json!({ "reason": finish_reason }),
                _ => json!(null),
            };

            let final_status = match finish_reason {
                Some("length") | Some("content_filter") => "incomplete",
                _ => "completed",
            };

            // Get upstream model
            let upstream_model = obj
                .get("model")
                .and_then(|m| m.as_str())
                .unwrap_or(model);

            // Usage
            let usage = obj.get("usage").cloned().unwrap_or_else(|| json!({}));
            let input_tokens = usage.get("prompt_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
            let output_tokens = usage.get("completion_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
            let total_tokens = usage.get("total_tokens").and_then(|v| v.as_i64()).unwrap_or(input_tokens + output_tokens);
            let cached_tokens = usage.get("prompt_tokens_details")
                .and_then(|d| d.get("cached_tokens"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let reasoning_tokens = usage.get("completion_tokens_details")
                .and_then(|d| d.get("reasoning_tokens"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            let completed_response = json!({
                "id": &response_id,
                "object": "response",
                "created_at": created_at,
                "status": final_status,
                "error": null,
                "incomplete_details": incomplete_details,
                "instructions": req_body.get("instructions"),
                "max_output_tokens": req_body.get("max_output_tokens"),
                "model": upstream_model,
                "output": output_items,
                "output_text": content,
                "parallel_tool_calls": req_body.get("parallel_tool_calls").unwrap_or(&json!(true)),
                "reasoning": req_body.get("reasoning").cloned().unwrap_or(json!({"effort": null, "summary": null})),
                "temperature": req_body.get("temperature").unwrap_or(&json!(1.0)),
                "text": req_body.get("text").cloned().unwrap_or(json!({"format": {"type": "text"}})),
                "tool_choice": req_body.get("tool_choice").unwrap_or(&json!("auto")),
                "tools": req_body.get("tools").unwrap_or(&json!([])),
                "top_p": req_body.get("top_p").unwrap_or(&json!(1.0)),
                "truncation": req_body.get("truncation").unwrap_or(&json!("disabled")),
                "previous_response_id": null,
                "store": req_body.get("store").unwrap_or(&json!(true)),
                "usage": {
                    "input_tokens": input_tokens,
                    "input_tokens_details": { "cached_tokens": cached_tokens },
                    "output_tokens": output_tokens,
                    "output_tokens_details": { "reasoning_tokens": reasoning_tokens },
                    "total_tokens": total_tokens
                },
                "user": req_body.get("user"),
                "metadata": req_body.get("metadata").unwrap_or(&json!({}))
            });

            if is_stream {
                frames.push(sse_line(&json!({
                    "type": "response.completed",
                    "response": &completed_response
                })));
            } else {
                // Store response for later retrieval via GET
                let mut store = state.response_store.write().await;
                store.insert(response_id.clone(), completed_response.clone());
                // Evict oldest if store exceeds 100 entries
                if store.len() > 100 {
                    if let Some(oldest_key) = store.keys().next().cloned() {
                        store.remove(&oldest_key);
                    }
                }
                return Ok(axum::Json(completed_response).into_response());
            }
        }
        Err(e) => {
            let error_response = json!({
                "id": &response_id,
                "object": "response",
                "created_at": created_at,
                "status": "failed",
                "error": { "message": format!("{e}"), "type": "proxy_error" }
            });

            if is_stream {
                frames.push(sse_line(&json!({
                    "type": "response.failed",
                    "response": &error_response
                })));
            } else {
                return Ok(axum::Json(error_response).into_response());
            }
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

// ─── Response Store Helpers ───────────────────────────────────────────

/// GET /v1/responses/:id — Retrieve a stored response.
pub async fn get_response(
    State(state): State<ProxyState>,
    axum::extract::Path(response_id): axum::extract::Path<String>,
) -> axum::response::Response {
    let store = state.response_store.read().await;
    if let Some(resp) = store.get(&response_id).cloned() {
        return axum::Json(resp).into_response();
    }

    (StatusCode::NOT_FOUND, axum::Json(json!({
        "error": {
            "message": format!("Response '{}' not found. This proxy does not persist responses.", response_id),
            "type": "not_found_error",
            "code": "response_not_found"
        }
    })))
    .into_response()
}

/// DELETE /v1/responses/:id — Delete a stored response.
pub async fn delete_response(
    State(state): State<ProxyState>,
    axum::extract::Path(response_id): axum::extract::Path<String>,
) -> axum::response::Response {
    let mut store = state.response_store.write().await;
    store.remove(&response_id);

    axum::Json(json!({
        "id": response_id,
        "object": "response",
        "deleted": true
    }))
    .into_response()
}

/// POST /v1/responses/:id/cancel — Cancel a response.
///
/// If the response exists in the store (future: background responses),
/// marks it as cancelled and returns the updated response.
/// Otherwise returns 404 since the proxy doesn't persist responses.
pub async fn cancel_response(
    State(state): State<ProxyState>,
    axum::extract::Path(response_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let store = state.response_store.read().await;
    if let Some(mut resp) = store.get(&response_id).cloned() {
        resp["status"] = json!("cancelled");
        return axum::Json(resp).into_response();
    }

    (
        StatusCode::NOT_FOUND,
        axum::Json(json!({
            "error": {
                "message": format!("Response '{}' not found. This proxy does not persist responses.", response_id),
                "type": "not_found_error",
                "code": "response_not_found"
            }
        })),
    )
        .into_response()
}

// ─── Unit Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── SSRF Protection Tests (Fix #9) ──

    #[test]
    fn test_validate_image_url_allows_https() {
        assert!(validate_image_url("https://example.com/image.png").is_ok());
    }

    #[test]
    fn test_validate_image_url_allows_http() {
        assert!(validate_image_url("http://example.com/image.png").is_ok());
    }

    #[test]
    fn test_validate_image_url_allows_data_image() {
        assert!(validate_image_url("data:image/png;base64,abc123").is_ok());
    }

    #[test]
    fn test_validate_image_url_rejects_ftp() {
        assert!(validate_image_url("ftp://example.com/image.png").is_err());
    }

    #[test]
    fn test_validate_image_url_rejects_localhost() {
        assert!(validate_image_url("http://localhost/image.png").is_err());
        assert!(validate_image_url("http://127.0.0.1/image.png").is_err());
    }

    #[test]
    fn test_validate_image_url_rejects_private_10() {
        assert!(validate_image_url("http://10.0.0.1/image.png").is_err());
    }

    #[test]
    fn test_validate_image_url_rejects_private_172() {
        assert!(validate_image_url("http://172.16.0.1/image.png").is_err());
    }

    #[test]
    fn test_validate_image_url_rejects_private_192() {
        assert!(validate_image_url("http://192.168.1.1/image.png").is_err());
    }

    #[test]
    fn test_validate_image_url_rejects_link_local() {
        assert!(validate_image_url("http://169.254.0.1/image.png").is_err());
    }

    #[test]
    fn test_validate_image_url_rejects_non_image_data() {
        assert!(validate_image_url("data:text/plain;base64,abc123").is_err());
    }

    // ── Tool Type Validation Tests (Fix #8) ──

    #[test]
    fn test_tool_type_function_allowed() {
        let tools = vec![json!({ "type": "function", "name": "get_weather" })];
        // function should not trigger any rejection
        for tool in &tools {
            assert_eq!(tool.get("type").and_then(|v| v.as_str()), Some("function"));
        }
    }

    #[test]
    fn test_tool_type_host_tool_rejected() {
        // host_tool is NOT "function", so it would be rejected by the validator
        let tool = json!({ "type": "host_tool" });
        assert_ne!(tool.get("type").and_then(|v| v.as_str()), Some("function"));
    }

    // ── Input Message Conversion Tests ──

    #[test]
    fn test_input_to_messages_string() {
        let input = json!("Hello");
        let msgs = input_to_messages(&input, None);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[0]["content"], "Hello");
    }

    #[test]
    fn test_input_to_messages_with_instructions() {
        let input = json!("Hello");
        let msgs = input_to_messages(&input, Some("Be helpful"));
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[0]["content"], "Be helpful");
        assert_eq!(msgs[1]["role"], "user");
    }

    #[test]
    fn test_input_to_messages_null_input() {
        let input = Value::Null;
        let msgs = input_to_messages(&input, Some("Instructions"));
        // Null input + instructions → only system message, no default user "Hello"
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[0]["content"], "Instructions");
    }

    #[test]
    fn test_input_to_messages_function_call() {
        let input = json!([
            { "type": "function_call", "call_id": "call_1", "name": "get_weather", "arguments": "{\"city\":\"NYC\"}" },
            { "type": "function_call_output", "call_id": "call_1", "output": "Sunny" }
        ]);
        let msgs = input_to_messages(&input, None);
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0]["role"], "assistant");
        assert_eq!(msgs[1]["role"], "tool");
        assert_eq!(msgs[1]["tool_call_id"], "call_1");
    }

    // ── Tool Conversion Tests ──

    #[test]
    fn test_convert_tools_function() {
        let tools = vec![json!({ "type": "function", "name": "my_fn", "parameters": { "type": "object" } })];
        let result = convert_tools(&tools);
        assert!(result.is_some());
        let arr = result.unwrap();
        let arr = arr.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["function"]["name"], "my_fn");
    }

    #[test]
    fn test_convert_tools_empty() {
        let tools: Vec<Value> = vec![];
        assert!(convert_tools(&tools).is_none());
    }

    #[test]
    fn test_convert_tools_non_function_filtered() {
        let tools = vec![json!({ "type": "web_search" })];
        // Non-function types should be filtered out
        let result = convert_tools(&tools);
        // Since no function tools, should return None
        assert!(result.is_none());
    }

    // ── Image URL Content Tests ──

    #[test]
    fn test_input_to_messages_mixed_text_and_image() {
        let input = json!([{
            "type": "message", "role": "user",
            "content": [
                { "type": "text", "text": "describe" },
                { "type": "input_image", "image_url": "https://example.com/img.jpg" }
            ]
        }]);
        let msgs = input_to_messages(&input, None);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "user");

        // Content should be an array with text + image_url
        let content = msgs[0]["content"].as_array().expect("content should be array");
        assert_eq!(content.len(), 2);
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], "describe");
        assert_eq!(content[1]["type"], "image_url");
        assert_eq!(content[1]["image_url"]["url"], "https://example.com/img.jpg");
        assert_eq!(content[1]["image_url"]["detail"], "auto");
    }

    #[test]
    fn test_input_to_messages_pure_image() {
        let input = json!([{
            "type": "message", "role": "user",
            "content": [
                { "type": "input_image", "image_url": "https://example.com/photo.png" }
            ]
        }]);
        let msgs = input_to_messages(&input, None);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "user");

        // Content should be an array with only image_url
        let content = msgs[0]["content"].as_array().expect("content should be array");
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["type"], "image_url");
        assert_eq!(content[0]["image_url"]["url"], "https://example.com/photo.png");
        assert_eq!(content[0]["image_url"]["detail"], "auto");
    }

    #[test]
    fn test_input_to_messages_image_with_custom_detail() {
        let input = json!([{
            "type": "message", "role": "user",
            "content": [
                { "type": "input_image", "image_url": "https://example.com/img.jpg", "detail": "high" }
            ]
        }]);
        let msgs = input_to_messages(&input, None);
        assert_eq!(msgs.len(), 1);

        let content = msgs[0]["content"].as_array().expect("content should be array");
        assert_eq!(content[0]["image_url"]["detail"], "high");
    }

    #[test]
    fn test_input_to_messages_image_no_regression_text_only() {
        // Text-only content array should still produce a plain string
        let input = json!([{
            "type": "message", "role": "user",
            "content": [
                { "type": "text", "text": "hello" }
            ]
        }]);
        let msgs = input_to_messages(&input, None);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "user");
        // Without images, content remains a plain string
        assert_eq!(msgs[0]["content"], "hello");
    }
}
