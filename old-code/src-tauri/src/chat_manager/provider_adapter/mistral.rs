use std::collections::HashMap;

use serde_json::{json, Value};

use super::{OpenAIChatRequest, ProviderAdapter};
use crate::chat_manager::tooling::{mistral_tool_choice, openai_tools, ToolConfig};

pub struct MistralAdapter;

impl ProviderAdapter for MistralAdapter {
    fn endpoint(&self, base_url: &str) -> String {
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
        &["X-API-KEY", "x-api-key"]
    }

    fn default_headers_template(&self) -> HashMap<String, String> {
        let mut out = HashMap::new();
        out.insert("X-API-KEY".into(), "<apiKey>".into());
        out.insert("Content-Type".into(), "application/json".into());
        out.insert("Accept".into(), "text/event-stream".into());
        out
    }

    fn headers(
        &self,
        api_key: &str,
        extra: Option<&HashMap<String, String>>,
    ) -> HashMap<String, String> {
        let mut out: HashMap<String, String> = HashMap::new();
        out.insert("X-API-KEY".into(), api_key.to_string());
        out.insert("Content-Type".into(), "application/json".into());
        out.insert("Accept".into(), "text/event-stream".into());
        out.entry("User-Agent".into())
            .or_insert_with(|| "LettuceAI/0.1".into());
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
        _frequency_penalty: Option<f64>,
        _presence_penalty: Option<f64>,
        _top_k: Option<u32>,
        tool_config: Option<&ToolConfig>,
        _reasoning_enabled: bool,
        _reasoning_effort: Option<String>,
        _reasoning_budget: Option<u32>,
    ) -> Value {
        let frequency_penalty = None;
        let presence_penalty = None;

        let (tools, tool_choice) = if let Some(cfg) = tool_config {
            let tools = openai_tools(cfg).unwrap_or_default();
            let choice = if tools.is_empty() {
                None
            } else {
                mistral_tool_choice(cfg.choice.as_ref())
            };
            (tools, choice)
        } else {
            (Vec::new(), None)
        };

        let body = OpenAIChatRequest {
            model: model_name,
            messages: messages_for_api,
            stream: should_stream,
            temperature,
            top_p,
            max_tokens: Some(max_tokens),
            context_length,
            max_completion_tokens: None,
            frequency_penalty,
            presence_penalty,
            reasoning_effort: None,
            reasoning: None,
            tools: if tools.is_empty() { None } else { Some(tools) },
            tool_choice,
        };

        serde_json::to_value(body).unwrap_or_else(|_| json!({}))
    }
}
