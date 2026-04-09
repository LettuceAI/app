use std::collections::HashMap;

use serde_json::{json, Value};

use super::ProviderAdapter;
use crate::chat_manager::tooling::{openai_tool_choice, openai_tools, ToolConfig};

pub struct OllamaAdapter;

impl ProviderAdapter for OllamaAdapter {
    fn endpoint(&self, base_url: &str) -> String {
        let trimmed = base_url.trim_end_matches('/');
        if trimmed.ends_with("/v1") {
            format!("{}/api/chat", trimmed.trim_end_matches("/v1"))
        } else {
            format!("{}/api/chat", trimmed)
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
        out.insert("Accept".into(), "application/json".into());
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
        out.insert("Accept".into(), "application/json".into());
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

        let mut body = json!({
            "model": model_name,
            "messages": messages_for_api,
            "stream": should_stream,
        });

        if let Some(map) = body.as_object_mut() {
            if let Some(tools) = tools {
                map.insert("tools".to_string(), Value::Array(tools));
            }
            if let Some(choice) = tool_choice {
                map.insert("tool_choice".to_string(), choice);
            }
            if let Some(effort) = reasoning_effort {
                map.insert("think".to_string(), Value::String(effort));
            } else if reasoning_enabled {
                map.insert("think".to_string(), Value::Bool(true));
            }
        }

        let _ = (
            temperature,
            top_p,
            max_tokens,
            context_length,
            frequency_penalty,
            presence_penalty,
            reasoning_budget,
        );

        body
    }
    fn list_models_endpoint(&self, base_url: &str) -> String {
        let mut base = base_url.trim_end_matches('/').to_string();
        if base.ends_with("/v1") {
            base = base
                .trim_end_matches("/v1")
                .trim_end_matches('/')
                .to_string();
        }
        format!("{}/api/tags", base)
    }

    fn parse_models_list(
        &self,
        response: Value,
    ) -> Vec<crate::chat_manager::provider_adapter::ModelInfo> {
        let mut models = Vec::new();
        if let Some(list) = response.get("models").and_then(|d| d.as_array()) {
            for item in list {
                if let Some(name) = item.get("name").and_then(|n| n.as_str()) {
                    models.push(crate::chat_manager::provider_adapter::ModelInfo {
                        id: name.to_string(),
                        display_name: Some(name.to_string()),
                        description: item
                            .get("details")
                            .and_then(|d| d.get("parameter_size"))
                            .and_then(|s| s.as_str())
                            .map(|s| format!("{} parameters", s)),
                        context_length: None,
                        input_price: None,
                        output_price: None,
                    });
                }
            }
        }
        models
    }
}
