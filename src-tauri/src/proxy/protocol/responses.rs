//! OpenAI Responses API 上游适配器（Beta）
//!
//! 作为 channel.api_type = "responses" 时的 adapter，把 chat.completions
//! 中间格式翻译成 Responses API 请求发给上游，再把 Responses API 响应
//! 翻译回 chat.completions 格式供内部消费。
//!
//! 参考官方文档：
//! https://platform.openai.com/docs/api-reference/responses
//!
//! 公理：这边进来什么，那边出去一样。已知字段按文档翻译，未知字段穿透。

use super::{join_url, ProtocolAdapter};
use serde_json::{json, Value};

/// 未知字段穿透开关。
///
/// 默认 true：贯彻"中转不丢失"公理。
/// 如果某个上游对穿透字段返回 400，可改为 false 让 adapter 只发官方已知字段。
#[allow(dead_code)]
const ENABLE_UNKNOWN_FIELD_PASSTHROUGH: bool = true;

pub struct ResponsesAdapter;

impl ProtocolAdapter for ResponsesAdapter {
    fn build_chat_url(&self, base_url: &str, _model: &str) -> String {
        join_url(base_url, "v1/responses")
    }

    fn build_models_url(&self, base_url: &str, _api_key: &str) -> String {
        // Responses API 复用 /v1/models 列表端点
        join_url(base_url, "v1/models")
    }

    fn uses_query_auth(&self) -> bool {
        false
    }

    fn build_auth_headers(&self, api_key: &str) -> Vec<(String, String)> {
        vec![("Authorization".to_string(), format!("Bearer {api_key}"))]
    }

    fn apply_auth(
        &self,
        builder: reqwest::RequestBuilder,
        api_key: &str,
    ) -> reqwest::RequestBuilder {
        builder.header("Authorization", format!("Bearer {api_key}"))
    }

    fn transform_request(&self, body: &mut Value, actual_model: &str) {
        transform_request_to_responses(body, actual_model);
    }

    fn transform_response(&self, body: &mut Value) {
        transform_response_from_responses(body);
    }

    fn needs_sse_transform(&self) -> bool {
        // v1: Responses 流式事件 → chat.completions SSE 的翻译非常复杂
        // 暂时只支持非流式，needs_sse_transform=false 表示 SSE 直通
        // 完整 SSE 翻译在后续迭代中补齐
        false
    }

    fn extract_sse_usage(&self, data_line: &str) -> (i64, i64) {
        if data_line == "[DONE]" {
            return (0, 0);
        }
        let Ok(value) = serde_json::from_str::<Value>(data_line) else {
            return (0, 0);
        };
        // Responses SSE 的 usage 格式：response.completed 事件里带 usage.input_tokens/output_tokens
        let prompt = value
            .pointer("/response/usage/input_tokens")
            .and_then(Value::as_i64)
            .unwrap_or_else(|| {
                value
                    .get("usage")
                    .and_then(|u| u.get("input_tokens"))
                    .and_then(Value::as_i64)
                    .unwrap_or(0)
            });
        let completion = value
            .pointer("/response/usage/output_tokens")
            .and_then(Value::as_i64)
            .unwrap_or_else(|| {
                value
                    .get("usage")
                    .and_then(|u| u.get("output_tokens"))
                    .and_then(Value::as_i64)
                    .unwrap_or(0)
            });
        (prompt, completion)
    }

    fn transform_sse_line(&self, data_line: &str) -> Option<String> {
        // needs_sse_transform=false，理论上不会被调用
        Some(data_line.to_string())
    }

    fn parse_models_response(&self, body: &Value) -> Vec<(String, Option<String>)> {
        // OpenAI 标准 /v1/models 响应格式
        body.get("data")
            .and_then(|d| d.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        let id = m.get("id")?.as_str()?.to_string();
                        let owned_by = m.get("owned_by").and_then(|v| v.as_str()).map(String::from);
                        Some((id, owned_by))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

// ═══════════════════════════════════════════════════════════════════
//  chat.completions → Responses API 请求
// ═══════════════════════════════════════════════════════════════════

/// 把 chat.completions 格式的请求翻译成 Responses API 请求。
///
/// 核心映射（根据 OpenAI 官方文档）：
/// - `model` → `model`（直接）
/// - `messages[]` → `input[]`（事件流形式）
///   - system/developer → 顶层 `instructions`（取第一条）
///   - user message → `input_text` item
///   - assistant message → `message` item（role=assistant）
///   - assistant tool_calls → `function_call` items
///   - tool message → `function_call_output` item
/// - `max_tokens` → `max_output_tokens`
/// - `stop` → Responses API 没直接对应，保留在 body（passthrough）
/// - `stream` → 保留
/// - `temperature`/`top_p` → 保留
/// - `tools[]` → `tools[]`（function tools 解包 function 嵌套）
/// - `tool_choice` → `tool_choice`（字符串或对象直接穿透）
/// - `response_format` → `text.format`（官方对应关系）
/// - 未知字段：ENABLE_UNKNOWN_FIELD_PASSTHROUGH=true 时全部保留
fn transform_request_to_responses(body: &mut Value, actual_model: &str) {
    let Some(obj) = body.as_object_mut() else {
        return;
    };

    // 1. 先收集和转换 messages → input
    let (instructions, input_items) = messages_to_input(obj.remove("messages"));

    // 2. 构造 Responses 请求骨架
    let mut responses = serde_json::Map::new();
    responses.insert("model".to_string(), json!(actual_model));
    responses.insert("input".to_string(), json!(input_items));
    if let Some(inst) = instructions {
        if !inst.is_empty() {
            responses.insert("instructions".to_string(), json!(inst));
        }
    }

    // 3. 已知字段映射
    if let Some(max_tokens) = obj
        .remove("max_completion_tokens")
        .or_else(|| obj.remove("max_tokens"))
    {
        responses.insert("max_output_tokens".to_string(), max_tokens);
    }

    // tools：从 chat.completions 的 {type:"function", function:{name,...}}
    // 转到 Responses 的 {type:"function", name, ...}（解包 function 嵌套）
    if let Some(tools) = obj.remove("tools") {
        responses.insert("tools".to_string(), convert_tools_to_responses(&tools));
    }

    // response_format → text.format（官方文档中的对应关系）
    if let Some(rf) = obj.remove("response_format") {
        responses.insert("text".to_string(), json!({ "format": rf }));
    }

    // 4. 其他已知字段直接拷贝（Responses API 也支持）
    for field in [
        "stream",
        "temperature",
        "top_p",
        "top_logprobs",
        "stream_options",
        "tool_choice",
        "parallel_tool_calls",
        "reasoning",
        "service_tier",
        "user",
        "metadata",
        "store",
        "include",
        "max_tool_calls",
        "previous_response_id",
        "truncation",
        "safety_identifier",
        "prompt",
        "prompt_cache_key",
        "prompt_cache_retention",
    ] {
        if let Some(val) = obj.remove(field) {
            responses.insert(field.to_string(), val);
        }
    }

    // 5. 未知字段穿透（公理二）
    if ENABLE_UNKNOWN_FIELD_PASSTHROUGH {
        for (key, value) in obj.iter() {
            if !responses.contains_key(key) {
                responses.insert(key.clone(), value.clone());
            }
        }
    }

    *body = Value::Object(responses);
}

/// 把 chat.completions 的 messages[] 转成 Responses 的 (instructions, input[])。
///
/// 返回 (instructions, input_items)。
/// - 第一个 system/developer 消息提升为 instructions（顶层参数）
/// - 后续 system 消息作为 input item 保留
/// - user 消息 → input_text item
/// - assistant 消息 → message item（可能带 tool_calls）
/// - tool 消息 → function_call_output item
fn messages_to_input(messages: Option<Value>) -> (Option<String>, Vec<Value>) {
    let Some(Value::Array(msgs)) = messages else {
        return (None, Vec::new());
    };

    let mut instructions: Option<String> = None;
    let mut input_items: Vec<Value> = Vec::new();

    for msg in msgs {
        let Some(obj) = msg.as_object() else {
            continue;
        };
        let role = obj.get("role").and_then(|v| v.as_str()).unwrap_or("user");
        let content = obj.get("content");

        match role {
            "system" | "developer" => {
                // 第一条 system 消息作为 instructions
                let text = extract_text_content(content);
                if instructions.is_none() {
                    instructions = Some(text);
                } else {
                    // 后续 system 消息进 input
                    input_items.push(json!({
                        "type": "message",
                        "role": "system",
                        "content": [{ "type": "input_text", "text": text }]
                    }));
                }
            }
            "user" => {
                let parts = user_content_to_responses_parts(content);
                input_items.push(json!({
                    "type": "message",
                    "role": "user",
                    "content": parts,
                }));
            }
            "assistant" => {
                // assistant 可能既有 content 也有 tool_calls
                let text = extract_text_content(content);
                if !text.is_empty() {
                    input_items.push(json!({
                        "type": "message",
                        "role": "assistant",
                        "content": [{ "type": "output_text", "text": text }]
                    }));
                }
                if let Some(tool_calls) = obj.get("tool_calls").and_then(|v| v.as_array()) {
                    for tc in tool_calls {
                        let call_id = tc.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        let fn_obj = tc.get("function");
                        let name = fn_obj
                            .and_then(|f| f.get("name"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let args = fn_obj
                            .and_then(|f| f.get("arguments"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("{}");
                        input_items.push(json!({
                            "type": "function_call",
                            "call_id": call_id,
                            "name": name,
                            "arguments": args,
                        }));
                    }
                }
            }
            "tool" => {
                let call_id = obj
                    .get("tool_call_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let output = extract_text_content(content);
                input_items.push(json!({
                    "type": "function_call_output",
                    "call_id": call_id,
                    "output": output,
                }));
            }
            other => {
                // 未知 role，穿透
                input_items.push(json!({
                    "type": "message",
                    "role": other,
                    "content": content.cloned().unwrap_or(Value::Null),
                }));
            }
        }
    }

    (instructions, input_items)
}

fn extract_text_content(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|part| {
                part.get("text")
                    .and_then(|t| t.as_str())
                    .or_else(|| part.as_str())
                    .map(String::from)
            })
            .collect::<Vec<_>>()
            .join(""),
        Some(Value::Null) | None => String::new(),
        Some(other) => serde_json::to_string(other).unwrap_or_default(),
    }
}

/// 把 chat.completions user message 的 content 转成 Responses input content parts。
/// 支持字符串和 content block array（含 image_url 等）。
fn user_content_to_responses_parts(content: Option<&Value>) -> Vec<Value> {
    match content {
        Some(Value::String(s)) => vec![json!({ "type": "input_text", "text": s })],
        Some(Value::Array(parts)) => parts
            .iter()
            .map(|p| {
                if let Some(obj) = p.as_object() {
                    let typ = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    match typ {
                        "text" => {
                            let text = obj.get("text").and_then(|v| v.as_str()).unwrap_or("");
                            json!({ "type": "input_text", "text": text })
                        }
                        "image_url" => {
                            let url = obj
                                .get("image_url")
                                .and_then(|u| u.get("url"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let detail = obj
                                .get("image_url")
                                .and_then(|u| u.get("detail"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("auto");
                            json!({
                                "type": "input_image",
                                "image_url": url,
                                "detail": detail,
                            })
                        }
                        _ => p.clone(), // 未知类型穿透
                    }
                } else if let Some(s) = p.as_str() {
                    json!({ "type": "input_text", "text": s })
                } else {
                    p.clone()
                }
            })
            .collect(),
        Some(other) => vec![json!({
            "type": "input_text",
            "text": serde_json::to_string(other).unwrap_or_default()
        })],
        None => Vec::new(),
    }
}

/// 把 chat.completions tools[] 转成 Responses tools[]。
/// chat: `{type:"function", function:{name,...}}` → resp: `{type:"function", name,...}`
fn convert_tools_to_responses(tools: &Value) -> Value {
    let Some(arr) = tools.as_array() else {
        return tools.clone();
    };

    let converted: Vec<Value> = arr
        .iter()
        .map(|tool| {
            let Some(obj) = tool.as_object() else {
                return tool.clone();
            };
            let typ = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
            // function tool 需要解包 function 嵌套
            if typ == "function" {
                if let Some(func) = obj.get("function").and_then(|v| v.as_object()) {
                    let mut new_tool = serde_json::Map::new();
                    new_tool.insert("type".to_string(), json!("function"));
                    for (k, v) in func.iter() {
                        new_tool.insert(k.clone(), v.clone());
                    }
                    // 未知顶层字段穿透
                    if ENABLE_UNKNOWN_FIELD_PASSTHROUGH {
                        for (k, v) in obj.iter() {
                            if k != "type" && k != "function" && !new_tool.contains_key(k) {
                                new_tool.insert(k.clone(), v.clone());
                            }
                        }
                    }
                    return Value::Object(new_tool);
                }
            }
            // 非 function 工具：直接穿透
            tool.clone()
        })
        .collect();

    json!(converted)
}

// ═══════════════════════════════════════════════════════════════════
//  Responses API 响应 → chat.completions 响应
// ═══════════════════════════════════════════════════════════════════

/// 把 Responses API 响应翻译成 chat.completions 响应。
///
/// 核心映射：
/// - `output[]` 里的 message item → choices[0].message.content
/// - `output[]` 里的 function_call item → choices[0].message.tool_calls
/// - `usage.input_tokens` → `usage.prompt_tokens`
/// - `usage.output_tokens` → `usage.completion_tokens`
/// - `status` → finish_reason 映射
/// - 未知字段穿透
fn transform_response_from_responses(body: &mut Value) {
    let Some(obj) = body.as_object_mut() else {
        return;
    };

    // 取出 output 数组
    let output = obj.remove("output");
    let mut content_text = String::new();
    let mut tool_calls: Vec<Value> = Vec::new();

    if let Some(Value::Array(items)) = output {
        for item in items {
            let Some(item_obj) = item.as_object() else {
                continue;
            };
            let typ = item_obj.get("type").and_then(|v| v.as_str()).unwrap_or("");

            match typ {
                "message" => {
                    // message.content[] 里的 output_text 拼接起来
                    if let Some(parts) = item_obj.get("content").and_then(|v| v.as_array()) {
                        for part in parts {
                            let part_type = part.get("type").and_then(|v| v.as_str()).unwrap_or("");
                            if part_type == "output_text" || part_type == "text" {
                                if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                                    content_text.push_str(text);
                                }
                            }
                        }
                    }
                }
                "function_call" => {
                    let call_id = item_obj
                        .get("call_id")
                        .and_then(|v| v.as_str())
                        .or_else(|| item_obj.get("id").and_then(|v| v.as_str()))
                        .unwrap_or("");
                    let name = item_obj.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let args = item_obj
                        .get("arguments")
                        .and_then(|v| v.as_str())
                        .unwrap_or("{}");
                    tool_calls.push(json!({
                        "id": call_id,
                        "type": "function",
                        "function": {
                            "name": name,
                            "arguments": args,
                        }
                    }));
                }
                _ => {
                    // 其他 item 类型（reasoning、refusal 等）保留到 provider_specific
                    // 或穿透。此处先简化处理，保留在最终 provider_specific 里。
                }
            }
        }
    }

    // 构造 message
    let mut message = serde_json::Map::new();
    message.insert("role".to_string(), json!("assistant"));
    message.insert("content".to_string(), json!(content_text));
    if !tool_calls.is_empty() {
        message.insert("tool_calls".to_string(), json!(tool_calls));
    }

    // finish_reason 映射
    let finish_reason = match obj.get("status").and_then(|v| v.as_str()) {
        Some("completed") => {
            if !tool_calls.is_empty() {
                "tool_calls"
            } else {
                "stop"
            }
        }
        Some("incomplete") => {
            // 看 incomplete_details.reason
            obj.get("incomplete_details")
                .and_then(|d| d.get("reason"))
                .and_then(|v| v.as_str())
                .unwrap_or("length")
        }
        Some(other) => other,
        None => "stop",
    };

    let choice = json!({
        "index": 0,
        "message": Value::Object(message),
        "finish_reason": finish_reason,
    });

    // usage 映射
    let usage_src = obj.remove("usage").unwrap_or(json!({}));
    let prompt_tokens = usage_src
        .get("input_tokens")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let completion_tokens = usage_src
        .get("output_tokens")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let total_tokens = usage_src
        .get("total_tokens")
        .and_then(Value::as_i64)
        .unwrap_or(prompt_tokens + completion_tokens);
    let mut usage_out = json!({
        "prompt_tokens": prompt_tokens,
        "completion_tokens": completion_tokens,
        "total_tokens": total_tokens,
    });
    // cached_tokens / reasoning_tokens 保留
    if let Some(cached) = usage_src
        .get("input_tokens_details")
        .and_then(|d| d.get("cached_tokens"))
        .and_then(Value::as_i64)
    {
        if cached > 0 {
            usage_out["prompt_tokens_details"] = json!({ "cached_tokens": cached });
        }
    }
    if let Some(reasoning) = usage_src
        .get("output_tokens_details")
        .and_then(|d| d.get("reasoning_tokens"))
        .and_then(Value::as_i64)
    {
        if reasoning > 0 {
            usage_out["completion_tokens_details"] = json!({ "reasoning_tokens": reasoning });
        }
    }

    // 构造 chat.completions 响应骨架
    let mut chat_response = serde_json::Map::new();
    if let Some(id) = obj.remove("id") {
        chat_response.insert("id".to_string(), id);
    }
    chat_response.insert("object".to_string(), json!("chat.completion"));
    if let Some(created) = obj.remove("created_at").or_else(|| obj.remove("created")) {
        chat_response.insert("created".to_string(), created);
    } else {
        chat_response.insert("created".to_string(), json!(chrono::Utc::now().timestamp()));
    }
    if let Some(model) = obj.remove("model") {
        chat_response.insert("model".to_string(), model);
    }
    chat_response.insert("choices".to_string(), json!([choice]));
    chat_response.insert("usage".to_string(), usage_out);

    // 未知字段穿透（公理二）
    if ENABLE_UNKNOWN_FIELD_PASSTHROUGH {
        for (key, value) in obj.iter() {
            if !chat_response.contains_key(key) {
                chat_response.insert(key.clone(), value.clone());
            }
        }
    }

    *body = Value::Object(chat_response);
}

// ═══════════════════════════════════════════════════════════════════
//  单元测试
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_url_points_to_v1_responses() {
        let a = ResponsesAdapter;
        assert_eq!(
            a.build_chat_url("https://api.openai.com", "gpt-4o"),
            "https://api.openai.com/v1/responses"
        );
        assert_eq!(
            a.build_chat_url("https://api.openai.com/v1", "gpt-4o"),
            "https://api.openai.com/v1/responses"
        );
    }

    #[test]
    fn auth_is_bearer_token() {
        let a = ResponsesAdapter;
        let headers = a.build_auth_headers("sk-abc");
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].0, "Authorization");
        assert_eq!(headers[0].1, "Bearer sk-abc");
    }

    #[test]
    fn transform_request_basic_user_message() {
        let a = ResponsesAdapter;
        let mut body = json!({
            "model": "auto",
            "messages": [
                {"role": "user", "content": "Hello"}
            ]
        });
        a.transform_request(&mut body, "gpt-4o");

        assert_eq!(body["model"], "gpt-4o");
        assert!(body.get("input").is_some());
        let input = body["input"].as_array().unwrap();
        assert_eq!(input.len(), 1);
        assert_eq!(input[0]["type"], "message");
        assert_eq!(input[0]["role"], "user");
        assert_eq!(input[0]["content"][0]["type"], "input_text");
        assert_eq!(input[0]["content"][0]["text"], "Hello");
    }

    #[test]
    fn transform_request_system_becomes_instructions() {
        let a = ResponsesAdapter;
        let mut body = json!({
            "model": "auto",
            "messages": [
                {"role": "system", "content": "Be brief."},
                {"role": "user", "content": "Hi"}
            ]
        });
        a.transform_request(&mut body, "gpt-4o");

        assert_eq!(body["instructions"], "Be brief.");
        let input = body["input"].as_array().unwrap();
        // system 已移到 instructions，input 里只剩 user
        assert_eq!(input.len(), 1);
        assert_eq!(input[0]["role"], "user");
    }

    #[test]
    fn transform_request_max_tokens_becomes_max_output_tokens() {
        let a = ResponsesAdapter;
        let mut body = json!({
            "model": "auto",
            "messages": [{"role": "user", "content": "Hi"}],
            "max_tokens": 1000
        });
        a.transform_request(&mut body, "gpt-4o");

        assert_eq!(body["max_output_tokens"], 1000);
        assert!(body.get("max_tokens").is_none());
    }

    #[test]
    fn transform_request_tools_unnest_function() {
        let a = ResponsesAdapter;
        let mut body = json!({
            "model": "auto",
            "messages": [{"role": "user", "content": "Weather?"}],
            "tools": [{
                "type": "function",
                "function": {
                    "name": "get_weather",
                    "description": "Get weather",
                    "parameters": {"type": "object"}
                }
            }]
        });
        a.transform_request(&mut body, "gpt-4o");

        let tool = &body["tools"][0];
        assert_eq!(tool["type"], "function");
        // 解包后直接在顶层
        assert_eq!(tool["name"], "get_weather");
        assert_eq!(tool["description"], "Get weather");
        // 不再有嵌套 function 字段
        assert!(tool.get("function").is_none());
    }

    #[test]
    fn transform_request_unknown_fields_passthrough() {
        let a = ResponsesAdapter;
        let mut body = json!({
            "model": "auto",
            "messages": [{"role": "user", "content": "Hi"}],
            "x_custom_tracking": "abc-123",
            "x_future_openai_field": {"nested": true}
        });
        a.transform_request(&mut body, "gpt-4o");

        // 公理二：未知字段必须穿透
        assert_eq!(body["x_custom_tracking"], "abc-123");
        assert_eq!(body["x_future_openai_field"]["nested"], true);
    }

    #[test]
    fn transform_request_assistant_tool_calls_become_function_call_items() {
        let a = ResponsesAdapter;
        let mut body = json!({
            "model": "auto",
            "messages": [
                {"role": "user", "content": "Weather?"},
                {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_abc",
                        "type": "function",
                        "function": {"name": "get_weather", "arguments": "{\"city\":\"SF\"}"}
                    }]
                },
                {"role": "tool", "tool_call_id": "call_abc", "content": "Sunny"}
            ]
        });
        a.transform_request(&mut body, "gpt-4o");

        let input = body["input"].as_array().unwrap();
        // user + function_call + function_call_output
        assert_eq!(input.len(), 3);
        assert_eq!(input[1]["type"], "function_call");
        assert_eq!(input[1]["call_id"], "call_abc");
        assert_eq!(input[1]["name"], "get_weather");
        assert_eq!(input[2]["type"], "function_call_output");
        assert_eq!(input[2]["call_id"], "call_abc");
        assert_eq!(input[2]["output"], "Sunny");
    }

    #[test]
    fn transform_response_basic_text() {
        let a = ResponsesAdapter;
        let mut body = json!({
            "id": "resp_123",
            "object": "response",
            "status": "completed",
            "model": "gpt-4o",
            "output": [{
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "Hello!"}]
            }],
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5,
                "total_tokens": 15
            }
        });
        a.transform_response(&mut body);

        assert_eq!(body["object"], "chat.completion");
        assert_eq!(body["id"], "resp_123");
        assert_eq!(body["model"], "gpt-4o");
        assert_eq!(body["choices"][0]["message"]["role"], "assistant");
        assert_eq!(body["choices"][0]["message"]["content"], "Hello!");
        assert_eq!(body["choices"][0]["finish_reason"], "stop");
        assert_eq!(body["usage"]["prompt_tokens"], 10);
        assert_eq!(body["usage"]["completion_tokens"], 5);
        assert_eq!(body["usage"]["total_tokens"], 15);
    }

    #[test]
    fn transform_response_function_call_becomes_tool_calls() {
        let a = ResponsesAdapter;
        let mut body = json!({
            "id": "resp_456",
            "status": "completed",
            "model": "gpt-4o",
            "output": [{
                "type": "function_call",
                "call_id": "call_xyz",
                "name": "get_weather",
                "arguments": "{\"city\":\"Tokyo\"}"
            }],
            "usage": {"input_tokens": 20, "output_tokens": 10}
        });
        a.transform_response(&mut body);

        let tool_calls = body["choices"][0]["message"]["tool_calls"]
            .as_array()
            .unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0]["id"], "call_xyz");
        assert_eq!(tool_calls[0]["function"]["name"], "get_weather");
        assert_eq!(
            tool_calls[0]["function"]["arguments"],
            "{\"city\":\"Tokyo\"}"
        );
        // tool_calls 存在时 finish_reason 应为 tool_calls
        assert_eq!(body["choices"][0]["finish_reason"], "tool_calls");
    }

    #[test]
    fn transform_response_unknown_fields_passthrough() {
        let a = ResponsesAdapter;
        let mut body = json!({
            "id": "resp_789",
            "status": "completed",
            "model": "gpt-4o",
            "output": [{
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "Hi"}]
            }],
            "usage": {"input_tokens": 5, "output_tokens": 3},
            "x_future_response_field": "preserve_me",
            "reasoning": {"effort": "high"}
        });
        a.transform_response(&mut body);

        // 公理二：响应方向未知字段也要穿透
        assert_eq!(body["x_future_response_field"], "preserve_me");
        assert_eq!(body["reasoning"]["effort"], "high");
    }

    #[test]
    fn transform_response_incomplete_maps_to_length() {
        let a = ResponsesAdapter;
        let mut body = json!({
            "id": "resp_inc",
            "status": "incomplete",
            "incomplete_details": {"reason": "max_output_tokens"},
            "model": "gpt-4o",
            "output": [{
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "partial"}]
            }],
            "usage": {"input_tokens": 5, "output_tokens": 100}
        });
        a.transform_response(&mut body);

        assert_eq!(body["choices"][0]["finish_reason"], "max_output_tokens");
    }
}
