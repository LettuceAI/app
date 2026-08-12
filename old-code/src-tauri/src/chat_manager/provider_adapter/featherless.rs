use std::collections::HashMap;

use serde_json::{json, Value};

use super::{OpenAIChatRequest, ProviderAdapter};
use crate::chat_manager::tooling::{openai_tool_choice, openai_tools, ToolConfig};

pub struct FeatherlessAdapter;

impl ProviderAdapter for FeatherlessAdapter {
    fn endpoint(&self, base_url: &str) -> String {
        // Featherless is OpenAI-compatible: /v1/chat/completions
        let trimmed = base_url.trim_end_matches('/');
        if trimmed.ends_with("/v1") {
            format!("{}/chat/completions", trimmed)
        } else {
            format!("{}/v1/chat/completions", trimmed)
        }
    }

    fn system_role(&self) -> std::borrow::Cow<'static, str> {
        // Uses classic system / user / assistant roles
        "system".into()
    }

    fn required_auth_headers(&self) -> &'static [&'static str] {
        &["Authorization"]
    }

    fn default_headers_template(&self) -> HashMap<String, String> {
        let mut out = HashMap::new();
        out.insert("Authorization".into(), "Bearer <apiKey>".into());
        out.insert("Content-Type".into(), "application/json".into());
        out.insert("Accept".into(), "text/event-stream".into());
        // Recommended but not strictly required by Featherless:
        out.insert("HTTP-Referer".into(), "<your app URL>".into());
        out.insert("X-Title".into(), "LettuceAI".into());
        out
    }

    fn headers(
        &self,
        api_key: &str,
        extra: Option<&HashMap<String, String>>,
    ) -> HashMap<String, String> {
        let mut out: HashMap<String, String> = HashMap::new();
        out.insert("Authorization".into(), format!("Bearer {}", api_key));
        out.insert("Content-Type".into(), "application/json".into());
        out.insert("Accept".into(), "text/event-stream".into());
        out.entry("User-Agent".into())
            .or_insert_with(|| "LettuceAI/0.1".into());
        // Default attribution as recommended by Featherless
        out.entry("HTTP-Referer".into())
            .or_insert_with(|| "https://www.lettuceai.app/".into());
        out.entry("X-Title".into())
            .or_insert_with(|| "LettuceAI".into());

        if let Some(extra) = extra {
            for (k, v) in extra.iter() {
                out.insert(k.clone(), v.clone());
            }
        }
        out
    }

    fn body(
        &self,
        model_name: &str,
        messages_for_api: &Vec<Value>,
        _system_prompt: Option<String>,
        temperature: Option<f64>,
        top_p: Option<f64>,
        max_tokens: u32,
        context_length: Option<u32>,
        should_stream: bool,
        frequency_penalty: Option<f64>,
        presence_penalty: Option<f64>,
        _top_k: Option<u32>,
        tool_config: Option<&ToolConfig>,
        reasoning_enabled: bool,
        reasoning_effort: Option<String>,
        reasoning_budget: Option<u32>,
    ) -> Value {
        let (tools, tool_choice) = if let Some(cfg) = tool_config {
            let tools = openai_tools(cfg);
            let choice = if tools.is_some() {
                openai_tool_choice(cfg.choice.as_ref())
            } else {
                None
            };
            (tools, choice)
        } else {
            (None, None)
        };

        let total_tokens = max_tokens + reasoning_budget.unwrap_or(0);

        // Featherless is OpenAI-compatible, so we can reuse the OpenAI-style body.
        let body = OpenAIChatRequest {
            model: model_name,
            messages: messages_for_api,
            stream: should_stream,
            temperature,
            top_p,
            max_tokens: Some(total_tokens),
            context_length,
            max_completion_tokens: None,
            frequency_penalty,
            presence_penalty,
            reasoning_effort: if reasoning_enabled {
                reasoning_effort
            } else {
                None
            },
            reasoning: None,
            tools,
            tool_choice,
        };
        serde_json::to_value(body).unwrap_or_else(|_| json!({}))
    }
}
