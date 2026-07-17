use super::forwarder;
use super::handlers::ProxyError;
use super::image_router::{build_images_url, ImageEndpoint};
use super::protocol::get_adapter;
use super::server::ProxyState;
use crate::database::{AccessKey, ApiEntry};
use crate::services::api_key_utils::primary_api_key;
use axum::body::Body;
use axum::http::HeaderValue;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use bytes::Bytes;
use serde_json::{json, Value};
use std::time::Instant;

fn content_type_str(content_type: Option<&HeaderValue>) -> &str {
    content_type
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
}

fn multipart_boundary(content_type: &str) -> Option<&str> {
    content_type
        .split(';')
        .map(str::trim)
        .find_map(|part| part.strip_prefix("boundary="))
        .map(|boundary| boundary.trim_matches('"'))
}

fn multipart_parts<'a>(content_type: &str, body: &'a [u8]) -> Vec<(&'a [u8], &'a [u8])> {
    let Some(boundary) = multipart_boundary(content_type) else {
        return Vec::new();
    };
    let marker = format!("--{boundary}");
    let text = String::from_utf8_lossy(body);
    text.split(&marker)
        .filter_map(|part| {
            let part = part.strip_prefix("\r\n").unwrap_or(part);
            let (headers, value) = part.split_once("\r\n\r\n")?;
            let value = value.strip_suffix("\r\n").unwrap_or(value);
            let header_start = headers.as_ptr() as usize - text.as_ptr() as usize;
            let value_start = value.as_ptr() as usize - text.as_ptr() as usize;
            Some((
                &body[header_start..header_start + headers.len()],
                &body[value_start..value_start + value.len()],
            ))
        })
        .collect()
}

fn disposition_param(headers: &[u8], parameter: &str) -> Option<String> {
    let headers = String::from_utf8_lossy(headers);
    let marker = format!("{parameter}=\"");
    let start = headers.find(&marker)? + marker.len();
    let end = headers[start..].find('"')? + start;
    Some(headers[start..end].to_string())
}

pub fn extract_model(content_type: Option<&HeaderValue>, body: &Bytes) -> Option<String> {
    let content_type = content_type_str(content_type);
    if content_type
        .to_ascii_lowercase()
        .starts_with("multipart/form-data")
    {
        return multipart_parts(content_type, body)
            .into_iter()
            .find(|(headers, _)| disposition_param(headers, "name").as_deref() == Some("model"))
            .and_then(|(_, value)| String::from_utf8(value.to_vec()).ok())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
    }

    serde_json::from_slice::<Value>(body)
        .ok()?
        .get("model")?
        .as_str()
        .map(str::to_string)
        .filter(|value| !value.is_empty())
}

fn sanitize_json_value(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                let lower = key.to_ascii_lowercase();
                if lower == "b64_json" || lower == "base64" {
                    *child = json!({"type": "base64", "omitted": true});
                } else if lower == "image"
                    && child
                        .as_str()
                        .is_some_and(|text| text.starts_with("data:image/"))
                {
                    *child = json!({"type": "data_url", "omitted": true});
                } else {
                    sanitize_json_value(child);
                }
            }
        }
        Value::Array(values) => values.iter_mut().for_each(sanitize_json_value),
        _ => {}
    }
}

pub fn sanitize_images_log(content_type: Option<&HeaderValue>, body: &Bytes) -> Value {
    let content_type = content_type_str(content_type);
    if content_type
        .to_ascii_lowercase()
        .starts_with("multipart/form-data")
    {
        let fields = multipart_parts(content_type, body)
            .into_iter()
            .map(|(headers, value)| {
                let name = disposition_param(headers, "name").unwrap_or_default();
                let filename = disposition_param(headers, "filename");
                if let Some(filename) = filename {
                    json!({"name": name, "filename": filename, "size": value.len(), "payload_omitted": true})
                } else {
                    json!({"name": name, "value": String::from_utf8_lossy(value)})
                }
            })
            .collect::<Vec<_>>();
        return json!({"content_type": "multipart/form-data", "fields": fields});
    }

    let mut value = serde_json::from_slice::<Value>(body)
        .unwrap_or_else(|_| json!({"body_size": body.len(), "body_omitted": true}));
    sanitize_json_value(&mut value);
    value
}

fn copy_request_headers(
    mut request: reqwest::RequestBuilder,
    headers: &HeaderMap,
) -> reqwest::RequestBuilder {
    const SKIP: &[&str] = &[
        "authorization",
        "host",
        "content-length",
        "transfer-encoding",
        "connection",
    ];
    for (name, value) in headers {
        if !SKIP.contains(&name.as_str()) {
            request = request.header(name, value);
        }
    }
    request
}

fn response_from_upstream(
    status: StatusCode,
    headers: &HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    let mut builder = Response::builder().status(status);
    for (name, value) in headers {
        if !matches!(
            name.as_str(),
            "content-length" | "transfer-encoding" | "connection"
        ) {
            builder = builder.header(name, value);
        }
    }
    builder
        .body(Body::from(body))
        .map_err(|error| ProxyError::Internal(format!("Failed to build Images response: {error}")))
}

fn attempt_path_json(attempts: &[Value]) -> String {
    serde_json::to_string(attempts).unwrap_or_else(|_| "[]".to_string())
}

fn log_image_attempt(
    state: &ProxyState,
    access_key: Option<&AccessKey>,
    entry: &ApiEntry,
    requested_model: &str,
    request_log: &Value,
    attempts: &[Value],
    latency_ms: i64,
    status: u16,
    success: bool,
    error: Option<&str>,
) {
    let raw_protocol = json!({
        "event": "images_upstream_request",
        "requested_model": requested_model,
        "resolved_model": entry.model,
        "images_request": request_log,
    });
    let path = attempt_path_json(attempts);
    forwarder::log_usage(
        &state.db,
        &state.app_handle,
        access_key,
        entry,
        requested_model,
        false,
        0,
        0,
        0,
        0,
        latency_ms,
        i32::from(status),
        success,
        error,
        Some(&path),
        None,
        Some(&raw_protocol),
    );
}

fn is_image_endpoint_contract_error(status: u16, message: &str) -> bool {
    if status != 400 && status != 404 {
        return false;
    }
    let lower = message.to_ascii_lowercase();
    lower.contains("not found")
        || lower.contains("unsupported")
        || lower.contains("not supported")
        || lower.contains("only supported on")
        || lower.contains("invalid_request_error")
}

fn is_recoverable_image_failure(
    settings: &crate::database::AppSettings,
    status: u16,
    message: &str,
) -> bool {
    if is_image_endpoint_contract_error(status, message) {
        return false;
    }

    status == 0
        || forwarder::should_disable_entry_for_status(&settings.circuit_disable_codes, status)
        || forwarder::status_matches_rules(&settings.circuit_retry_codes, status)
        || forwarder::should_disable_entry_for_message(
            forwarder::effective_disable_keywords(settings),
            message,
        )
}
async fn apply_image_failure(
    state: &ProxyState,
    entry: &ApiEntry,
    status: u16,
    message: &str,
) -> bool {
    let settings = state.settings.read().await.clone();
    if !is_recoverable_image_failure(&settings, status, message) {
        return false;
    }

    let disable_by_status = status > 0
        && forwarder::should_disable_entry_for_status(&settings.circuit_disable_codes, status);
    let disable_by_keyword = status > 0
        && forwarder::should_disable_entry_for_message(
            forwarder::effective_disable_keywords(&settings),
            message,
        );

    if disable_by_keyword {
        if settings.keyword_freeze_scope == "channel" {
            forwarder::freeze_channel_entries(state, entry).await;
        } else {
            forwarder::cool_down_entry(state, entry).await;
        }
        true
    } else if disable_by_status {
        forwarder::disable_entry(state, entry).await;
        true
    } else {
        forwarder::cool_down_entry(state, entry).await;
        true
    }
}

pub async fn forward_images(
    state: &ProxyState,
    entries: &[ApiEntry],
    endpoint: ImageEndpoint,
    body: Bytes,
    headers: &HeaderMap,
    requested_model: &str,
    access_key: Option<&AccessKey>,
) -> Result<Response, ProxyError> {
    let request_log = sanitize_images_log(headers.get("content-type"), &body);
    let mut attempts = Vec::new();
    let mut last_transport_error: Option<String> = None;
    let last_index = entries.len().saturating_sub(1);

    for (index, entry) in entries.iter().enumerate() {
        let start = Instant::now();
        let channel = state.db.get_channel(&entry.channel_id)?;
        let url = build_images_url(&channel.base_url, endpoint);
        let adapter = get_adapter(&channel.api_type);
        let mut request = adapter.apply_auth(
            state.http_client.post(url),
            primary_api_key(&channel.api_key),
        );
        request = copy_request_headers(request, headers);
        request = crate::services::upstream_headers::apply_upstream_headers(
            request,
            channel.upstream_headers.as_deref(),
        )?;

        let response = match request.body(body.clone()).send().await {
            Ok(response) => response,
            Err(error) => {
                let message = format!("Images upstream transport error: {error}");
                attempts.push(json!({
                    "entry_id": entry.id,
                    "channel": entry.channel_name,
                    "model": entry.model,
                    "status_code": 502,
                    "success": false,
                    "error": message,
                }));
                log_image_attempt(
                    state,
                    access_key,
                    entry,
                    requested_model,
                    &request_log,
                    &attempts,
                    start.elapsed().as_millis() as i64,
                    502,
                    false,
                    Some(&message),
                );
                apply_image_failure(state, entry, 0, &message).await;
                last_transport_error = Some(message);
                continue;
            }
        };

        let status = response.status();
        let response_headers = response.headers().clone();
        let response_body = response.bytes().await.unwrap_or_default();
        let error_message = if status.is_success() {
            None
        } else {
            Some(String::from_utf8_lossy(&response_body).into_owned())
        };
        attempts.push(json!({
            "entry_id": entry.id,
            "channel": entry.channel_name,
            "model": entry.model,
            "status_code": status.as_u16(),
            "success": status.is_success(),
            "error": error_message,
        }));
        log_image_attempt(
            state,
            access_key,
            entry,
            requested_model,
            &request_log,
            &attempts,
            start.elapsed().as_millis() as i64,
            status.as_u16(),
            status.is_success(),
            error_message.as_deref(),
        );

        if status.is_success() {
            forwarder::record_circuit_success(state, &entry.id).await;
            return response_from_upstream(status, &response_headers, response_body);
        }

        let retry = apply_image_failure(
            state,
            entry,
            status.as_u16(),
            error_message.as_deref().unwrap_or_default(),
        )
        .await;
        if !retry {
            return response_from_upstream(status, &response_headers, response_body);
        }

        last_transport_error = None;
        if index == last_index {
            return response_from_upstream(status, &response_headers, response_body);
        }
    }

    Err(last_transport_error
        .map(ProxyError::Internal)
        .unwrap_or(ProxyError::AllProvidersFailed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_model_from_json_without_rewriting_body() {
        let original = Bytes::from_static(br#"{ "prompt" : "cat", "model" : "image-x", "n": 2 }"#);
        let snapshot = original.clone();
        let content_type = HeaderValue::from_static("application/json");
        assert_eq!(
            extract_model(Some(&content_type), &original).as_deref(),
            Some("image-x")
        );
        assert_eq!(original, snapshot);
    }

    #[test]
    fn extracts_model_from_multipart_without_rewriting_file_bytes() {
        let boundary = "----api-switch-boundary";
        let raw = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\nimage-x\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"image\"; filename=\"cat.png\"\r\nContent-Type: image/png\r\n\r\nPNG_BYTES_123\0\x01\r\n--{boundary}--\r\n"
        );
        let original = Bytes::from(raw.into_bytes());
        let snapshot = original.clone();
        let content_type =
            HeaderValue::from_str(&format!("multipart/form-data; boundary={boundary}")).unwrap();
        assert_eq!(
            extract_model(Some(&content_type), &original).as_deref(),
            Some("image-x")
        );
        assert_eq!(original, snapshot);
    }

    fn plan_image_attempts(
        settings: &crate::database::AppSettings,
        upstream: &[(u16, &str)],
    ) -> Vec<u16> {
        let mut attempts = Vec::new();
        for (status, message) in upstream {
            attempts.push(*status);
            if (200..300).contains(status)
                || !is_recoverable_image_failure(settings, *status, message)
            {
                break;
            }
        }
        attempts
    }
    #[test]
    fn retry_policy_stops_on_unknown_errors_and_retries_chat_classified_failures() {
        let mut settings = crate::database::AppSettings::default();
        settings.circuit_disable_codes = "401,403".to_string();
        settings.circuit_retry_codes = "429,500-503".to_string();
        settings.disable_keywords = "quota exhausted".to_string();

        assert_eq!(
            plan_image_attempts(&settings, &[(400, "bad request"), (201, "ok")]),
            vec![400]
        );
        assert_eq!(
            plan_image_attempts(&settings, &[(429, "rate limited"), (201, "ok")]),
            vec![429, 201]
        );
        assert_eq!(
            plan_image_attempts(&settings, &[(502, "bad gateway"), (503, "still down")]),
            vec![502, 503]
        );
    }
    #[test]
    fn image_failure_classification_matches_chat_status_and_keyword_rules() {
        let mut settings = crate::database::AppSettings::default();
        settings.circuit_disable_codes = "401,403".to_string();
        settings.circuit_retry_codes = "429,500-503".to_string();
        settings.disable_keywords = "quota exhausted".to_string();

        assert!(is_recoverable_image_failure(&settings, 401, "unauthorized"));
        assert!(is_recoverable_image_failure(&settings, 429, "rate limited"));
        assert!(is_recoverable_image_failure(&settings, 502, "bad gateway"));
        assert!(is_recoverable_image_failure(
            &settings,
            400,
            "quota exhausted"
        ));
        assert!(is_recoverable_image_failure(&settings, 0, "transport"));
        assert!(!is_recoverable_image_failure(&settings, 400, "bad request"));
        assert!(!is_recoverable_image_failure(
            &settings,
            404,
            "OpenAIException - {\"detail\":\"Not Found\"}"
        ));
        assert!(!is_recoverable_image_failure(
            &settings,
            400,
            "model gpt-image-2 is only supported on /v1/images/generations and /v1/images/edits"
        ));
        assert!(!is_recoverable_image_failure(&settings, 404, "not found"));
    }
    #[test]
    fn image_log_keeps_multipart_metadata_but_not_file_bytes() {
        let boundary = "----quoted-boundary";
        let raw = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\nimage-x\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"image\"; filename=\"cat.png\"\r\nContent-Type: image/png\r\n\r\nSECRET_PNG_BYTES\r\n--{boundary}--\r\n"
        );
        let content_type = HeaderValue::from_str(&format!(
            "multipart/form-data; charset=utf-8; boundary=\"{boundary}\""
        ))
        .unwrap();
        let body = Bytes::from(raw.into_bytes());

        assert_eq!(
            extract_model(Some(&content_type), &body).as_deref(),
            Some("image-x")
        );
        let log = sanitize_images_log(Some(&content_type), &body).to_string();
        assert!(log.contains("cat.png"));
        assert!(log.contains("payload_omitted"));
        assert!(!log.contains("SECRET_PNG_BYTES"));
    }

    #[test]
    fn response_preserves_status_end_to_end_headers_and_binary_body() {
        let mut headers = HeaderMap::new();
        headers.insert("content-type", HeaderValue::from_static("image/png"));
        headers.insert("x-upstream-id", HeaderValue::from_static("request-1"));
        headers.insert("connection", HeaderValue::from_static("close"));
        let body = Bytes::from_static(b"\x89PNG\r\n\x1a\n");

        let response = response_from_upstream(StatusCode::IM_A_TEAPOT, &headers, body.clone())
            .expect("response should build");
        assert_eq!(response.status(), StatusCode::IM_A_TEAPOT);
        assert_eq!(response.headers()["content-type"], "image/png");
        assert_eq!(response.headers()["x-upstream-id"], "request-1");
        assert!(!response.headers().contains_key("connection"));

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let actual = runtime
            .block_on(axum::body::to_bytes(response.into_body(), usize::MAX))
            .unwrap();
        assert_eq!(actual, body);
    }

    #[test]
    fn response_preserves_empty_text_and_html_bodies_without_json_wrapping() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        for (content_type, bytes) in [
            ("text/plain", Bytes::new()),
            ("text/html", Bytes::from_static(b"<h1>upstream failed</h1>")),
        ] {
            let mut headers = HeaderMap::new();
            headers.insert("content-type", HeaderValue::from_str(content_type).unwrap());
            let response =
                response_from_upstream(StatusCode::BAD_GATEWAY, &headers, bytes.clone()).unwrap();
            let actual = runtime
                .block_on(axum::body::to_bytes(response.into_body(), usize::MAX))
                .unwrap();
            assert_eq!(actual, bytes);
        }
    }

    #[test]
    fn image_log_keeps_metadata_but_not_image_payloads() {
        let content_type = HeaderValue::from_static("application/json");
        let body = Bytes::from_static(br#"{"model":"image-x","prompt":"cat","b64_json":"SECRET_IMAGE","image":"data:image/png;base64,SECRET_DATA","url":"https://example.com/cat.png"}"#);
        let log = sanitize_images_log(Some(&content_type), &body).to_string();
        assert!(log.contains("image-x"));
        assert!(log.contains("https://example.com/cat.png"));
        assert!(!log.contains("SECRET_IMAGE"));
        assert!(!log.contains("SECRET_DATA"));
    }
}
