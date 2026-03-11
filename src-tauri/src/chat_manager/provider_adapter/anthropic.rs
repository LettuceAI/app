use std::collections::HashMap;

use serde::Serialize;
use serde_json::{json, Value};

use super::ProviderAdapter;
use crate::chat_manager::tooling::{anthropic_tool_choice, anthropic_tools, ToolConfig};

pub struct AnthropicAdapter;

#[derive(Serialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    kind: &'static str,
    text: String,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: Vec<AnthropicContent>,
}

#[derive(Serialize)]
struct AnthropicMessagesRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    temperature: f64,
    #[serde(rename = "top_p")]
    top_p: f64,
    #[serde(rename = "max_tokens")]
    max_tokens: u32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(rename = "top_k", skip_serializing_if = "Option::is_none")]
    top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Value>>,
    #[serde(rename = "tool_choice", skip_serializing_if = "Option::is_none")]
    tool_choice: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<AnthropicThinking>,
}

#[derive(Serialize)]
struct AnthropicThinking {
    #[serde(rename = "type")]
    kind: &'static str,
    budget_tokens: u32,
}

impl ProviderAdapter for AnthropicAdapter {
    fn endpoint(&self, base_url: &str) -> String {
        let trimmed = base_url.trim_end_matches('/');
        if trimmed.ends_with("/v1") {
            format!("{}/messages", trimmed)
        } else {
            format!("{}/v1/messages", trimmed)
        }
    }

    fn system_role(&self) -> std::borrow::Cow<'static, str> {
        "system".into()
    }

    fn supports_stream(&self) -> bool {
        true
    }

    fn required_auth_headers(&self) -> &'static [&'static str] {
        &["x-api-key", "X-API-Key"]
    }

    fn default_headers_template(&self) -> HashMap<String, String> {
        let mut out = HashMap::new();
        out.insert("x-api-key".into(), "<apiKey>".into());
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
        out.insert("x-api-key".into(), api_key.to_string());
        out.insert("Content-Type".into(), "application/json".into());
        out.insert("Accept".into(), "text/event-stream".into());
        out.insert("anthropic-version".into(), "2023-06-01".into());
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
        system_prompt: Option<String>,
        temperature: f64,
        top_p: f64,
        max_tokens: u32,
        _context_length: Option<u32>,
        should_stream: bool,
        _frequency_penalty: Option<f64>,
        _presence_penalty: Option<f64>,
        top_k: Option<u32>,
        tool_config: Option<&ToolConfig>,
        reasoning_enabled: bool,
        _reasoning_effort: Option<String>,
        reasoning_budget: Option<u32>,
    ) -> Value {
        let mut msgs: Vec<AnthropicMessage> = Vec::new();
        let mut system_parts: Vec<String> = Vec::new();

        if let Some(s) = system_prompt {
            if !s.is_empty() {
                system_parts.push(s);
            }
        }

        for msg in messages_for_api {
            let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
            let content_text = msg
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if role == "system" || role == "developer" {
                if !content_text.is_empty() {
                    system_parts.push(content_text);
                }
                continue;
            }

            if content_text.is_empty() {
                continue;
            }
            let mapped_role = match role {
                "assistant" => "assistant",
                _ => "user",
            }
            .to_string();
            msgs.push(AnthropicMessage {
                role: mapped_role,
                content: vec![AnthropicContent {
                    kind: "text",
                    text: content_text,
                }],
            });
        }

        let combined_system = if system_parts.is_empty() {
            None
        } else {
            Some(system_parts.join("\n\n"))
        };

        let thinking = if reasoning_enabled {
            reasoning_budget.map(|budget| AnthropicThinking {
                kind: "enabled",
                budget_tokens: budget,
            })
        } else {
            None
        };

        // If thinking is enabled, max_tokens must be greater than budget_tokens
        let total_max_tokens = if let Some(ref t) = thinking {
            max_tokens + t.budget_tokens
        } else {
            max_tokens
        };

        let tools = tool_config.and_then(anthropic_tools);
        let tool_choice = tool_config.and_then(|cfg| anthropic_tool_choice(cfg.choice.as_ref()));

        let body = AnthropicMessagesRequest {
            model: model_name.to_string(),
            messages: msgs,

            temperature: if thinking.is_some() { 1.0 } else { temperature },
            top_p,
            max_tokens: total_max_tokens,
            stream: should_stream,
            system: combined_system,
            top_k,
            tools,
            tool_choice,
            thinking,
        };
        serde_json::to_value(body).unwrap_or_else(|_| json!({}))
    }
    fn list_models_endpoint(&self, base_url: &str) -> String {
        let trimmed = base_url.trim_end_matches('/');
        if trimmed.ends_with("/v1") {
            format!("{}/models", trimmed)
        } else {
            format!("{}/v1/models", trimmed)
        }
    }

    fn parse_models_list(
        &self,
        response: Value,
    ) -> Vec<crate::chat_manager::provider_adapter::ModelInfo> {
        let mut models = Vec::new();
        if let Some(data) = response.get("data").and_then(|d| d.as_array()) {
            for item in data {
                if let Some(id) = item.get("id").and_then(|id| id.as_str()) {
                    models.push(crate::chat_manager::provider_adapter::ModelInfo {
                        id: id.to_string(),
                        display_name: item
                            .get("display_name") // Anthropic uses display_name
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string()),
                        description: None,
                        context_length: None, // Anthropic doesn't explicitly send context_length in this list usually
                        input_price: None,
                        output_price: None,
                    });
                }
            }
        }
        models
    }
}
