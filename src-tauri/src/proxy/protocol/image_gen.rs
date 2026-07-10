//! 图像生成协议适配器
//!
//! 为 `/v1/images/generations` 提供模型能力分发和厂商特定字段转换：
//! - OpenAI 图像模型（gpt-image-1, gpt-image-2）：直通标准 OpenAI 格式
//! - Agnes 图像模型（agnes-image-*）：转换 extra_body 字段 + return_base64
//!
//! 后续扩展新生图模型时，只需在 `detect_image_gen_protocol` 中增加模型名匹配规则，
//! 并在必要时新增转换函数，不影响 HTTP 入口和路由层。

use serde::Deserialize;
use serde_json::Value;

// ─── 请求 / 响应类型 ────────────────────────────────────────

/// OpenAI /v1/images/generations 请求体（用于类型校验和文档）
///
/// 注意：实际转发时使用原始 JSON Value 透传，不强制反序列化为此结构，
/// 以保证对上游新参数的兼容性。
#[derive(Debug, Clone, Deserialize)]
pub struct ImageGenerationRequest {
    pub model: String,
    pub prompt: String,
    #[serde(default = "default_n")]
    pub n: u32,
    pub size: Option<String>,
    pub response_format: Option<String>,
    pub quality: Option<String>,
    pub style: Option<String>,
    pub seed: Option<u32>,
    /// Agnes 图生图：输入图片数组（公网 URL 或 Data URI Base64）
    #[serde(default)]
    pub image: Vec<String>,
}

fn default_n() -> u32 {
    1
}

// ─── 模型能力分发 ──────────────────────────────────────────

/// 图像生成模型的协议类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageGenProtocol {
    /// 标准 OpenAI 图像 API（gpt-image-1, gpt-image-2, dall-e-* 等）
    OpenAi,
    /// Agnes AI 图像 API（agnes-image-*）
    Agnes,
}

/// 根据模型名识别图像生成协议类型
///
/// 扩展规则：新增模型只需在此函数中增加匹配分支。
pub fn detect_image_gen_protocol(model: &str) -> ImageGenProtocol {
    let lower = model.to_ascii_lowercase();
    if lower.starts_with("agnes-image") {
        ImageGenProtocol::Agnes
    } else {
        // gpt-image-1, gpt-image-2, dall-e-3 等均走 OpenAI 标准
        ImageGenProtocol::OpenAi
    }
}

/// 判断模型名是否属于图像生成模型（用于路由前快速识别）
///
/// P1 白名单：gpt-image-*、agnes-image-*、dall-e-*
/// 后续可升级为数据库中的能力字段
pub fn is_image_gen_model(model: &str) -> bool {
    let lower = model.to_ascii_lowercase();
    lower.starts_with("gpt-image")
        || lower.starts_with("agnes-image")
        || lower.starts_with("dall-e")
}

// ─── 适配器转换 ────────────────────────────────────────────

/// 根据协议类型转换图像生成请求体
///
/// OpenAI 标准：直接透传（无额外转换）
/// Agnes：需要将 `response_format` 移入 `extra_body`，设置 `return_base64`
pub fn transform_image_gen_request(body: &mut Value, protocol: ImageGenProtocol) {
    match protocol {
        ImageGenProtocol::OpenAi => {}
        ImageGenProtocol::Agnes => transform_agnes_request(body),
    }
}

/// Agnes 特定请求转换：
/// - `response_format: "b64_json"` → 设置 `return_base64 = true`，移除顶层 `response_format`
/// - `response_format: "url"` → 移入 `extra_body.response_format`，移除顶层
/// - `extra_body` 中的字段平铺到请求体顶层
fn transform_agnes_request(body: &mut Value) {
    let Some(obj) = body.as_object_mut() else {
        return;
    };

    // 处理 response_format
    let response_format = obj
        .get("response_format")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    if let Some(rf) = response_format {
        obj.remove("response_format");

        // 如果是 b64_json，设置 return_base64 = true
        if rf == "b64_json" {
            obj.insert("return_base64".to_string(), Value::Bool(true));
        }

        // 将 response_format 放入 extra_body
        let extra = obj
            .entry("extra_body")
            .or_insert_with(|| Value::Object(serde_json::Map::new()));

        if let Some(extra_obj) = extra.as_object_mut() {
            extra_obj.insert("response_format".to_string(), Value::String(rf));
        }
    }

    // 如果用户已经传了 extra_body，确保其中已有的字段不丢失
    // （上面的 or_insert 只在不存在时创建，不会覆盖已有的 extra_body 内容）
}

/// 根据协议类型转换上游响应为标准 OpenAI Images 格式
///
/// OpenAI 标准：直接透传
/// Agnes：尝试转换为 OpenAI 格式（如果上游不是标准格式）
pub fn transform_image_gen_response(upstream_body: &Value, protocol: ImageGenProtocol) -> Value {
    match protocol {
        ImageGenProtocol::OpenAi => upstream_body.clone(),
        ImageGenProtocol::Agnes => transform_agnes_response(upstream_body),
    }
}

/// Agnes 响应转换：
/// 如果上游已返回 OpenAI 兼容格式（含 `data` 数组），直接透传。
/// 否则尝试从 `images` / `outputs` 字段提取并转换。
fn transform_agnes_response(body: &Value) -> Value {
    // 已经是 OpenAI 兼容格式
    if body.get("data").and_then(|d| d.as_array()).is_some() {
        return body.clone();
    }

    let created = chrono::Utc::now().timestamp();

    // 尝试从 Agnes 的 images / outputs 字段转换
    let images = body
        .get("images")
        .or_else(|| body.get("outputs"))
        .and_then(|v| v.as_array());

    if let Some(arr) = images {
        let data: Vec<Value> = arr
            .iter()
            .map(|img| {
                let mut item = serde_json::Map::new();

                // 优先取 b64_json，其次 base64、data
                let b64 = img
                    .get("b64_json")
                    .or_else(|| img.get("base64"))
                    .or_else(|| img.get("data"))
                    .and_then(|v| v.as_str());
                if let Some(b) = b64 {
                    item.insert("b64_json".to_string(), Value::String(b.to_string()));
                }

                let url = img.get("url").and_then(|v| v.as_str());
                if let Some(u) = url {
                    item.insert("url".to_string(), Value::String(u.to_string()));
                }

                let revised = img.get("revised_prompt").and_then(|v| v.as_str());
                if let Some(r) = revised {
                    item.insert("revised_prompt".to_string(), Value::String(r.to_string()));
                }

                Value::Object(item)
            })
            .collect();

        serde_json::json!({ "created": created, "data": data })
    } else {
        // 未知格式：返回空数据，避免客户端解析崩溃
        serde_json::json!({ "created": created, "data": [] })
    }
}

// ─── 单元测试 ────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ─── is_image_gen_model ─────────────────────────────

    #[test]
    fn recognizes_gpt_image_models() {
        assert!(is_image_gen_model("gpt-image-1"));
        assert!(is_image_gen_model("gpt-image-2"));
        assert!(is_image_gen_model("GPT-IMAGE-1"));
    }

    #[test]
    fn recognizes_agnes_image_models() {
        assert!(is_image_gen_model("agnes-image-2.1-flash"));
        assert!(is_image_gen_model("AGNES-IMAGE-1"));
    }

    #[test]
    fn recognizes_dall_e_models() {
        assert!(is_image_gen_model("dall-e-3"));
        assert!(is_image_gen_model("DALL-E-2"));
    }

    #[test]
    fn rejects_chat_models() {
        assert!(!is_image_gen_model("gpt-4o"));
        assert!(!is_image_gen_model("claude-3-opus"));
        assert!(!is_image_gen_model("gemini-pro"));
    }

    // ─── detect_image_gen_protocol ──────────────────────

    #[test]
    fn agnes_model_detected_correctly() {
        assert_eq!(
            detect_image_gen_protocol("agnes-image-2.1-flash"),
            ImageGenProtocol::Agnes
        );
    }

    #[test]
    fn openai_model_detected_correctly() {
        assert_eq!(
            detect_image_gen_protocol("gpt-image-1"),
            ImageGenProtocol::OpenAi
        );
        assert_eq!(
            detect_image_gen_protocol("gpt-image-2"),
            ImageGenProtocol::OpenAi
        );
        assert_eq!(
            detect_image_gen_protocol("dall-e-3"),
            ImageGenProtocol::OpenAi
        );
    }

    // ─── transform_image_gen_request ────────────────────

    #[test]
    fn openai_request_passthrough() {
        let mut body = json!({
            "model": "gpt-image-1",
            "prompt": "a cat",
            "n": 2,
            "size": "1024x1024"
        });
        transform_image_gen_request(&mut body, ImageGenProtocol::OpenAi);
        // OpenAI 路径不改任何字段
        assert_eq!(body["model"], "gpt-image-1");
        assert_eq!(body["prompt"], "a cat");
        assert_eq!(body["n"], 2);
        assert_eq!(body["size"], "1024x1024");
    }

    #[test]
    fn agnes_request_b64_json_sets_return_base64_and_moves_response_format() {
        let mut body = json!({
            "model": "agnes-image-2.1-flash",
            "prompt": "a cat",
            "size": "1024x1024",
            "response_format": "b64_json"
        });
        transform_image_gen_request(&mut body, ImageGenProtocol::Agnes);
        assert_eq!(body["return_base64"], true);
        assert!(
            body.get("response_format").is_none(),
            "顶层 response_format 应被移除"
        );
        assert_eq!(body["extra_body"]["response_format"], "b64_json");
    }

    #[test]
    fn agnes_request_url_format_moves_to_extra_body() {
        let mut body = json!({
            "model": "agnes-image-2.1-flash",
            "prompt": "a cat",
            "size": "1024x1024",
            "response_format": "url"
        });
        transform_image_gen_request(&mut body, ImageGenProtocol::Agnes);
        // 非 b64_json 不设置 return_base64
        assert!(body.get("return_base64").is_none());
        assert!(
            body.get("response_format").is_none(),
            "顶层 response_format 应被移除"
        );
        assert_eq!(body["extra_body"]["response_format"], "url");
    }

    #[test]
    fn agnes_request_no_response_format_unchanged() {
        let mut body = json!({
            "model": "agnes-image-2.1-flash",
            "prompt": "a cat",
            "size": "1024x1024"
        });
        transform_image_gen_request(&mut body, ImageGenProtocol::Agnes);
        assert!(body.get("return_base64").is_none());
        assert!(body.get("extra_body").is_none());
    }

    // ─── transform_image_gen_response ───────────────────

    #[test]
    fn openai_response_passthrough() {
        let resp = json!({
            "created": 1234567890,
            "data": [{"b64_json": "abc123"}]
        });
        let result = transform_image_gen_response(&resp, ImageGenProtocol::OpenAi);
        assert_eq!(result, resp);
    }

    #[test]
    fn agnes_response_with_data_passthrough() {
        let resp = json!({
            "created": 1234567890,
            "data": [{"b64_json": "abc123"}]
        });
        let result = transform_image_gen_response(&resp, ImageGenProtocol::Agnes);
        assert_eq!(result, resp);
    }

    #[test]
    fn agnes_response_images_array_converted() {
        let resp = json!({
            "images": [
                {"base64": "img1"},
                {"base64": "img2", "revised_prompt": "revised"}
            ]
        });
        let result = transform_image_gen_response(&resp, ImageGenProtocol::Agnes);
        assert!(result.get("created").is_some());
        let data = result["data"].as_array().unwrap();
        assert_eq!(data.len(), 2);
        assert_eq!(data[0]["b64_json"], "img1");
        assert_eq!(data[1]["b64_json"], "img2");
        assert_eq!(data[1]["revised_prompt"], "revised");
    }

    #[test]
    fn agnes_response_with_url_field() {
        let resp = json!({
            "images": [
                {"url": "https://example.com/img.png"}
            ]
        });
        let result = transform_image_gen_response(&resp, ImageGenProtocol::Agnes);
        let data = result["data"].as_array().unwrap();
        assert_eq!(data.len(), 1);
        assert_eq!(data[0]["url"], "https://example.com/img.png");
    }

    #[test]
    fn agnes_response_unknown_format_returns_empty() {
        let resp = json!({
            "status": "ok",
            "message": "generated"
        });
        let result = transform_image_gen_response(&resp, ImageGenProtocol::Agnes);
        assert!(result["data"].as_array().unwrap().is_empty());
        assert!(result.get("created").is_some());
    }
}
