use serde_json::{json, Value};

// ═══════════════════════════════════════════════════════════════════
//  Public API: Azure OpenAI -> OpenAI request conversion
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
}
