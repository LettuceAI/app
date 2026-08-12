use std::collections::HashMap;

use serde_json::{json, Value};

use super::{deepseek::DeepSeekAdapter, ProviderAdapter};
use crate::chat_manager::tooling::ToolConfig;

pub struct NanoGPTAdapter;

impl ProviderAdapter for NanoGPTAdapter {
    fn endpoint(&self, base_url: &str) -> String {
        // NanoGPT usually uses BASE = https://nano-gpt.com/api/v1
        let trimmed = base_url.trim_end_matches('/');
        if trimmed.ends_with("/v1") {
            format!("{}/chat/completions", trimmed)
        } else {
            format!("{}/v1/chat/completions", trimmed)
        }
    }

    fn system_role(&self) -> std::borrow::Cow<'static, str> {
        "system".into()
    }

    fn required_auth_headers(&self) -> &'static [&'static str] {
        &["Authorization"]
    }

    fn default_headers_template(&self) -> HashMap<String, String> {
        DeepSeekAdapter.default_headers_template()
    }

    fn headers(
        &self,
        api_key: &str,
        extra: Option<&HashMap<String, String>>,
    ) -> HashMap<String, String> {
        DeepSeekAdapter.headers(api_key, extra)
    }

    fn body(
        &self,
        model_name: &str,
        messages_for_api: &Vec<Value>,
        system_prompt: Option<String>,
        temperature: Option<f64>,
        top_p: Option<f64>,
        max_tokens: u32,
        context_length: Option<u32>,
        should_stream: bool,
        frequency_penalty: Option<f64>,
        presence_penalty: Option<f64>,
        top_k: Option<u32>,
        tool_config: Option<&ToolConfig>,
        reasoning_enabled: bool,
        reasoning_effort: Option<String>,
        reasoning_budget: Option<u32>,
    ) -> Value {
        let mut body = DeepSeekAdapter.body(
            model_name,
            messages_for_api,
            system_prompt,
            temperature,
            top_p,
            max_tokens,
            context_length,
            should_stream,
            frequency_penalty,
            presence_penalty,
            top_k,
            tool_config,
            reasoning_enabled,
            reasoning_effort,
            reasoning_budget,
        );
        if should_stream {
            if let Some(object) = body.as_object_mut() {
                object.insert("stream_options".into(), json!({ "include_usage": true }));
            }
        }
        body
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body_for(should_stream: bool) -> Value {
        NanoGPTAdapter.body(
            "nvidia/nemotron-3-ultra-550b-a55b",
            &vec![json!({ "role": "user", "content": "hi" })],
            None,
            None,
            None,
            2048,
            None,
            should_stream,
            None,
            None,
            None,
            None,
            false,
            None,
            None,
        )
    }

    #[test]
    fn requests_usage_on_streamed_completions() {
        assert_eq!(
            body_for(true).get("stream_options"),
            Some(&json!({ "include_usage": true }))
        );
    }

    #[test]
    fn omits_stream_options_when_not_streaming() {
        assert_eq!(body_for(false).get("stream_options"), None);
    }
}
