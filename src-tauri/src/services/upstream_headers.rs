//! 统一上游请求 Header 注入，从渠道配置的 upstream_headers JSON 中解析
//! 键值对并注入到 reqwest 请求。系统只提供注入方法，不替用户判断注入是否正确。

use crate::error::AppError;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

const SYSTEM_MANAGED_HEADERS: &[&str] = &[
    "host",
    "content-length",
    "transfer-encoding",
    "connection",
];

/// 将渠道 `upstream_headers` JSON（`{"Header-Name": "value", ...}`）注入到请求构建器。
///
/// JSON 必须是对象，且每个值必须是字符串；重复键以配置为准覆盖。
pub(crate) fn apply_upstream_headers(
    builder: reqwest::RequestBuilder,
    upstream_headers: Option<&str>,
) -> Result<reqwest::RequestBuilder, AppError> {
    let Some(raw) = upstream_headers else {
        return Ok(builder);
    };
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(builder);
    }

    let value = serde_json::from_str::<serde_json::Value>(raw).map_err(|e| {
        AppError::Validation(format!("upstream_headers 必须是合法 JSON 对象：{e}"))
    })?;
    let Some(map) = value.as_object() else {
        return Err(AppError::Validation(
            "upstream_headers 必须是 JSON 对象".to_string(),
        ));
    };

    let mut headers = HeaderMap::new();
    for (name, value) in map {
        let Some(value) = value.as_str() else {
            return Err(AppError::Validation(format!(
                "upstream_headers 中的 {name} 必须是字符串"
            )));
        };
        let header_name = HeaderName::from_bytes(name.as_bytes()).map_err(|e| {
            AppError::Validation(format!("upstream_headers 中的 Header 名称无效：{name} ({e})"))
        })?;
        if SYSTEM_MANAGED_HEADERS.contains(&header_name.as_str()) {
            return Err(AppError::Validation(format!(
                "该 Header 由系统管理，不能手动注入：{name}"
            )));
        }
        let header_value = HeaderValue::from_str(value).map_err(|e| {
            AppError::Validation(format!("upstream_headers 中的 {name} 值无效：{e}"))
        })?;
        headers.insert(header_name, header_value);
    }

    Ok(builder.headers(headers))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_builder() -> reqwest::RequestBuilder {
        reqwest::Client::new().post("http://localhost/test")
    }

    #[test]
    fn apply_empty_payload_does_nothing() {
        let builder = new_builder();
        // 验证函数不 panic
        let _ = apply_upstream_headers(builder, None).unwrap();
    }

    #[test]
    fn apply_valid_json_injects_headers() {
        let json = r#"{"x-custom": "abc", "x-test": "123"}"#;
        let request = apply_upstream_headers(new_builder(), Some(json))
            .unwrap()
            .build()
            .expect("build request");
        let headers = request.headers();
        assert_eq!(headers.get("x-custom").unwrap(), "abc");
        assert_eq!(headers.get("x-test").unwrap(), "123");
    }

    #[test]
    fn apply_invalid_json_returns_error() {
        let json = "{not valid}";
        let err = apply_upstream_headers(new_builder(), Some(json)).unwrap_err();
        assert!(err.to_string().contains("upstream_headers 必须是合法 JSON 对象"));
    }

    #[test]
    fn reject_system_managed_headers() {
        let json = r#"{"host": "example.com"}"#;
        let err = apply_upstream_headers(new_builder(), Some(json)).unwrap_err();
        assert!(err.to_string().contains("该 Header 由系统管理"));
    }

    #[test]
    fn injected_headers_override_existing_values() {
        let json = r#"{"x-custom": "new"}"#;
        let request = apply_upstream_headers(new_builder().header("x-custom", "old"), Some(json))
            .unwrap()
            .build()
            .expect("build request");
        let values = request.headers().get_all("x-custom").iter().collect::<Vec<_>>();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0], "new");
    }

    #[test]
    fn apply_empty_string_does_nothing() {
        let request = apply_upstream_headers(new_builder(), Some(""))
            .unwrap()
            .build()
            .expect("build request");
        // 没有任何额外 Header
        assert!(request.headers().is_empty());
    }

    #[test]
    fn apply_empty_json_object_does_nothing() {
        let request = apply_upstream_headers(new_builder(), Some("{}"))
            .unwrap()
            .build()
            .expect("build request");
        assert!(request.headers().is_empty());
    }

    #[test]
    fn reject_non_string_value() {
        let json = r#"{"x-custom": 123}"#;
        let err = apply_upstream_headers(new_builder(), Some(json)).unwrap_err();
        assert!(err.to_string().contains("必须是字符串"));
    }

    #[test]
    fn reject_invalid_header_name() {
        // Header 名称不能包含空格
        let json = r#"{"foo bar": "value"}"#;
        let err = apply_upstream_headers(new_builder(), Some(json)).unwrap_err();
        assert!(err.to_string().contains("Header 名称无效"));
    }
}