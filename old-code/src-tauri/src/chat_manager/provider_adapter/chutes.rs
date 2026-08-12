use std::collections::HashMap;

use serde_json::Value;

use super::{deepseek::DeepSeekAdapter, ProviderAdapter};
use crate::chat_manager::tooling::ToolConfig;

/// Chutes provides OpenAI-compatible endpoints (e.g. /v1/chat/completions).
///
/// This adapter intentionally reuses our OpenAI-style request/headers logic.
pub struct ChutesAdapter;

impl ProviderAdapter for ChutesAdapter {
    fn endpoint(&self, base_url: &str) -> String {
        let normalized = normalize_chutes_base_url(base_url);
        DeepSeekAdapter.endpoint(&normalized)
    }

    fn system_role(&self) -> std::borrow::Cow<'static, str> {
        // vLLM / SGLang deployments generally expect classic OpenAI roles.
        "system".into()
    }

    fn required_auth_headers(&self) -> &'static [&'static str] {
        // NOTE: keep explicit to remain resilient even if DeepSeekAdapter changes.
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
        DeepSeekAdapter.body(
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
        )
    }
}

fn normalize_chutes_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return "https://llm.chutes.ai".to_string();
    }

    trimmed
        .replace("://api.chutes.ai", "://llm.chutes.ai")
        .replace("://www.api.chutes.ai", "://llm.chutes.ai")
}
