use std::collections::HashMap;

use serde::Serialize;
use serde_json::{json, Value};

use super::ProviderAdapter;
use crate::chat_manager::tooling::{openai_tools, zai_tool_choice, ToolConfig};

pub struct ZAIAdapter;

#[derive(Serialize)]
struct ZAIChatRequest<'a> {
    model: &'a str,
    messages: &'a Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(rename = "top_p", skip_serializing_if = "Option::is_none")]
    top_p: Option<f64>,
    #[serde(rename = "max_tokens")]
    max_tokens: u32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<Value>,
    thinking: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<String>,
}

impl ProviderAdapter for ZAIAdapter {
    fn endpoint(&self, base_url: &str) -> String {
        // Z.AI is OpenAI-compatible: POST {base}/chat/completions, where base is
        // https://api.z.ai/api/paas/v4 (or /api/coding/paas/v4 for coding-plan keys).
        let trimmed = base_url.trim_end_matches('/');
        if trimmed.ends_with("/chat/completions") {
            trimmed.to_string()
        } else {
            format!("{}/chat/completions", trimmed)
        }
    }

    fn system_role(&self) -> std::borrow::Cow<'static, str> {
        // ZAI uses standard OpenAI-style roles
        "system".into()
    }

    fn required_auth_headers(&self) -> &'static [&'static str] {
        &["Authorization"]
    }

    fn default_headers_template(&self) -> HashMap<String, String> {
        let mut out = HashMap::new();
        out.insert("Authorization".into(), "Bearer <apiKey>".into());
        out.insert("Content-Type".into(), "application/json".into());
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
        _context_length: Option<u32>,
        should_stream: bool,
        _frequency_penalty: Option<f64>,
        _presence_penalty: Option<f64>,
        _top_k: Option<u32>,
        tool_config: Option<&ToolConfig>,
        reasoning_enabled: bool,
        reasoning_effort: Option<String>,
        reasoning_budget: Option<u32>,
    ) -> Value {
        let (tools, tool_choice) = if let Some(cfg) = tool_config {
            let tools = openai_tools(cfg);
            let choice = if tools.is_some() {
                zai_tool_choice(cfg.choice.as_ref())
            } else {
                None
            };
            (tools, choice)
        } else {
            (None, None)
        };

        let total_tokens = max_tokens + reasoning_budget.unwrap_or(0);

        let thinking = json!({
            "type": if reasoning_enabled { "enabled" } else { "disabled" }
        });
        let explicit_reasoning_effort = if reasoning_enabled {
            reasoning_effort
        } else {
            None
        };

        let body = ZAIChatRequest {
            model: model_name,
            messages: messages_for_api,
            temperature,
            top_p,
            max_tokens: total_tokens,
            stream: should_stream,
            tools,
            tool_choice,
            thinking,
            reasoning_effort: explicit_reasoning_effort,
        };

        serde_json::to_value(body).unwrap_or_else(|_| json!({}))
    }
}
