use std::collections::HashMap;

use serde_json::{json, Value};

use super::{OpenAIChatRequest, ProviderAdapter};
use crate::chat_manager::tooling::{openai_tool_choice, openai_tools, ToolConfig};

pub struct QwenAdapter;

impl ProviderAdapter for QwenAdapter {
    fn endpoint(&self, base_url: &str) -> String {
        // Qwen uses OpenAI-style endpoints
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
        let mut out = HashMap::new();
        out.insert("Authorization".into(), "Bearer <apiKey>".into());
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
        out.insert("Authorization".into(), format!("Bearer {}", api_key));
        out.insert("Content-Type".into(), "application/json".into());
        out.insert("Accept".into(), "text/event-stream".into());

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

        // Qwen uses OpenAI-compatible body
        let explicit_reasoning_effort = if reasoning_enabled {
            reasoning_effort
        } else {
            None
        };

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
            reasoning_effort: explicit_reasoning_effort,
            reasoning: None,
            tools,
            tool_choice,
        };
        let mut value = serde_json::to_value(body).unwrap_or_else(|_| json!({}));
        if reasoning_enabled {
            if let Some(map) = value.as_object_mut() {
                map.insert("enable_thinking".to_string(), json!(true));
                if let Some(budget) = reasoning_budget {
                    map.insert("thinking_budget".to_string(), json!(budget));
                }
            }
        }
        value
    }
}
