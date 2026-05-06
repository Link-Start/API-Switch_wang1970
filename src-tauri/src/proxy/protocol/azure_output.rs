use serde_json::{json, Value};

// ═══════════════════════════════════════════════════════════════════
//  Public API: Azure OpenAI <-> OpenAI format conversion
// ═══════════════════════════════════════════════════════════════════

/// Convert Azure OpenAI request format to standard OpenAI format.
///
/// Azure and OpenAI request formats are identical except Azure puts the
/// deployment name (model) in the URL path and ignores `model` in the body.
/// This function takes an Azure request and returns an OpenAI-compatible request,
/// ensuring the `model` field is set from the deployment name if not present.
///
/// - model: set from deployment name if missing in body
/// - messages, max_tokens, temperature, etc.: passthrough as-is
/// - tools, tool_choice, stream, stop: passthrough as-is
pub fn azure_to_openai_request(azure: &Value, deployment: &str) -> Value {
    let mut openai = azure.clone();

    // Azure puts model in URL path, not body. Ensure model field exists for OpenAI.
    if openai.get("model").is_none() {
        openai["model"] = json!(deployment);
    }

    openai
}

/// Convert OpenAI response format to Azure OpenAI response format.
///
/// Azure uses the exact same response format as OpenAI, so this is a passthrough.
///
/// - id, object, choices, usage: all identical
/// - finish_reason: same values ("stop", "length", "tool_calls")
pub fn openai_to_azure_response(openai: &Value) -> Value {
    // Azure response format is identical to OpenAI — no conversion needed.
    openai.clone()
}

/// Transform an error into Azure OpenAI error format.
///
/// Azure uses the same error format as OpenAI:
/// ```json
/// {
///   "error": {
///     "code": "...",
///     "message": "...",
///     "type": "..."
///   }
/// }
/// ```
pub fn transform_azure_error(status: u16, message: &str) -> Value {
    let error_type = match status {
        400 => "invalid_request_error",
        401 => "authentication_error",
        403 => "permission_error",
        404 => "not_found_error",
        429 => "rate_limit_error",
        500..=599 => "server_error",
        _ => "server_error",
    };

    json!({
        "error": {
            "code": status,
            "message": message,
            "type": error_type
        }
    })
}

// ═══════════════════════════════════════════════════════════════════
//  AzureSSETransformer: SSE passthrough (Azure SSE = OpenAI SSE)
// ═══════════════════════════════════════════════════════════════════

/// Transforms Azure OpenAI streaming SSE chunks.
///
/// Azure's SSE format is identical to OpenAI's, so this is a passthrough.
/// The transformer exists for structural parity with other protocols
/// (Claude, Gemini) that need actual transformation.
pub struct AzureSSETransformer {
    #[allow(dead_code)]
    message_id: String,
    #[allow(dead_code)]
    model: String,
}

impl AzureSSETransformer {
    pub fn new(message_id: String, model: String) -> Self {
        Self { message_id, model }
    }

    /// Passthrough: Azure SSE chunks are already in OpenAI format.
    /// Returns the chunk unchanged wrapped in a vector.
    pub fn transform_chunk(&self, azure_chunk: &str) -> Vec<String> {
        // Azure SSE is OpenAI-compatible — no transformation needed.
        vec![azure_chunk.to_string()]
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ─── azure_to_openai_request tests ────────────────────────────

    #[test]
    fn basic_azure_to_openai() {
        let azure = json!({
            "messages": [
                {"role": "user", "content": "Hello"}
            ],
            "max_tokens": 100,
            "temperature": 0.7
        });

        let openai = azure_to_openai_request(&azure, "my-gpt4-deployment");

        // model should be injected from deployment name
        assert_eq!(openai["model"], "my-gpt4-deployment");
        assert_eq!(openai["messages"][0]["role"], "user");
        assert_eq!(openai["messages"][0]["content"], "Hello");
        assert_eq!(openai["max_tokens"], 100);
        assert_eq!(openai["temperature"], 0.7);
    }

    #[test]
    fn azure_to_openai_preserves_existing_model() {
        let azure = json!({
            "model": "gpt-4",
            "messages": [
                {"role": "user", "content": "Hi"}
            ]
        });

        let openai = azure_to_openai_request(&azure, "my-deployment");

        // Existing model in body should be kept (not overwritten)
        assert_eq!(openai["model"], "gpt-4");
    }

    #[test]
    fn azure_to_openai_passthrough_all_fields() {
        let azure = json!({
            "messages": [{"role": "user", "content": "test"}],
            "max_tokens": 512,
            "temperature": 0.9,
            "top_p": 0.95,
            "stream": true,
            "stop": ["END"],
            "tools": [{"type": "function", "function": {"name": "search"}}],
            "tool_choice": "auto"
        });

        let openai = azure_to_openai_request(&azure, "deployment-1");

        assert_eq!(openai["max_tokens"], 512);
        assert_eq!(openai["temperature"], 0.9);
        assert_eq!(openai["top_p"], 0.95);
        assert_eq!(openai["stream"], true);
        assert_eq!(openai["stop"], json!(["END"]));
        assert_eq!(openai["tools"][0]["function"]["name"], "search");
        assert_eq!(openai["tool_choice"], "auto");
    }

    #[test]
    fn azure_to_openai_empty_body() {
        let azure = json!({});
        let openai = azure_to_openai_request(&azure, "gpt-4-deploy");
        assert_eq!(openai["model"], "gpt-4-deploy");
    }

    // ─── openai_to_azure_response tests ───────────────────────────

    #[test]
    fn basic_openai_to_azure() {
        let openai = json!({
            "id": "chatcmpl-abc123",
            "object": "chat.completion",
            "model": "gpt-4o",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "Hello! How can I help?"
                    },
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 8,
                "total_tokens": 18
            }
        });

        let azure = openai_to_azure_response(&openai);

        // Azure format is identical — passthrough
        assert_eq!(azure["id"], "chatcmpl-abc123");
        assert_eq!(azure["object"], "chat.completion");
        assert_eq!(azure["model"], "gpt-4o");
        assert_eq!(azure["choices"][0]["message"]["content"], "Hello! How can I help?");
        assert_eq!(azure["choices"][0]["finish_reason"], "stop");
        assert_eq!(azure["usage"]["prompt_tokens"], 10);
        assert_eq!(azure["usage"]["completion_tokens"], 8);
        assert_eq!(azure["usage"]["total_tokens"], 18);
    }

    #[test]
    fn openai_to_azure_with_tool_calls() {
        let openai = json!({
            "id": "chatcmpl-tool123",
            "object": "chat.completion",
            "model": "gpt-4o",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "Let me check.",
                        "tool_calls": [
                            {
                                "id": "call_abc",
                                "type": "function",
                                "function": {
                                    "name": "get_weather",
                                    "arguments": "{\"city\": \"Tokyo\"}"
                                }
                            }
                        ]
                    },
                    "finish_reason": "tool_calls"
                }
            ],
            "usage": {
                "prompt_tokens": 20,
                "completion_tokens": 15,
                "total_tokens": 35
            }
        });

        let azure = openai_to_azure_response(&openai);

        assert_eq!(azure["choices"][0]["finish_reason"], "tool_calls");
        assert_eq!(
            azure["choices"][0]["message"]["tool_calls"][0]["id"],
            "call_abc"
        );
        assert_eq!(
            azure["choices"][0]["message"]["tool_calls"][0]["function"]["name"],
            "get_weather"
        );
    }

    #[test]
    fn openai_to_azure_max_tokens() {
        let openai = json!({
            "id": "chatcmpl-length",
            "model": "gpt-4",
            "choices": [
                {
                    "message": {
                        "role": "assistant",
                        "content": "This is a truncated response..."
                    },
                    "finish_reason": "length"
                }
            ],
            "usage": {"prompt_tokens": 100, "completion_tokens": 4096}
        });

        let azure = openai_to_azure_response(&openai);
        assert_eq!(azure["choices"][0]["finish_reason"], "length");
        assert_eq!(azure["usage"]["completion_tokens"], 4096);
    }

    // ─── transform_azure_error tests ──────────────────────────────

    #[test]
    fn azure_error_400() {
        let error = transform_azure_error(400, "Invalid request body");
        assert_eq!(error["error"]["type"], "invalid_request_error");
        assert_eq!(error["error"]["message"], "Invalid request body");
        assert_eq!(error["error"]["code"], 400);
    }

    #[test]
    fn azure_error_401() {
        let error = transform_azure_error(401, "Access denied due to invalid key");
        assert_eq!(error["error"]["type"], "authentication_error");
        assert_eq!(error["error"]["message"], "Access denied due to invalid key");
        assert_eq!(error["error"]["code"], 401);
    }

    #[test]
    fn azure_error_404() {
        let error = transform_azure_error(404, "Deployment not found");
        assert_eq!(error["error"]["type"], "not_found_error");
        assert_eq!(error["error"]["code"], 404);
    }

    #[test]
    fn azure_error_429() {
        let error = transform_azure_error(429, "Rate limit exceeded");
        assert_eq!(error["error"]["type"], "rate_limit_error");
        assert_eq!(error["error"]["code"], 429);
    }

    #[test]
    fn azure_error_500() {
        let error = transform_azure_error(500, "Internal server error");
        assert_eq!(error["error"]["type"], "server_error");
        assert_eq!(error["error"]["code"], 500);
    }

    #[test]
    fn azure_error_unknown_status() {
        let error = transform_azure_error(418, "I'm a teapot");
        // Unknown status falls through to server_error
        assert_eq!(error["error"]["type"], "server_error");
        assert_eq!(error["error"]["code"], 418);
    }

    // ─── AzureSSETransformer tests ────────────────────────────────

    #[test]
    fn sse_passthrough_basic() {
        let transformer =
            AzureSSETransformer::new("chatcmpl-test".to_string(), "gpt-4o".to_string());

        let chunk =
            r#"{"id":"chatcmpl-abc","object":"chat.completion.chunk","choices":[{"delta":{"role":"assistant"}}]}"#;
        let events = transformer.transform_chunk(chunk);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], chunk);
    }

    #[test]
    fn sse_passthrough_content_delta() {
        let transformer =
            AzureSSETransformer::new("chatcmpl-test".to_string(), "gpt-4o".to_string());

        let chunk =
            r#"{"id":"chatcmpl-abc","choices":[{"delta":{"content":"Hello world"},"finish_reason":null}]}"#;
        let events = transformer.transform_chunk(chunk);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], chunk);
    }

    #[test]
    fn sse_passthrough_finish() {
        let transformer =
            AzureSSETransformer::new("chatcmpl-test".to_string(), "gpt-4o".to_string());

        let chunk =
            r#"{"id":"chatcmpl-abc","choices":[{"delta":{},"finish_reason":"stop"}]}"#;
        let events = transformer.transform_chunk(chunk);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], chunk);
    }

    #[test]
    fn sse_passthrough_tool_call_chunk() {
        let transformer =
            AzureSSETransformer::new("chatcmpl-test".to_string(), "gpt-4o".to_string());

        let chunk = r#"{"id":"chatcmpl-abc","choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_123","type":"function","function":{"name":"search","arguments":""}}]},"finish_reason":null}]}"#;
        let events = transformer.transform_chunk(chunk);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], chunk);
    }
}
