use std::borrow::Cow;
use std::collections::HashMap;

use serde::Serialize;
use serde_json::{json, Value};

use super::ProviderAdapter;
use crate::chat_manager::tooling::{anthropic_tool_choice, anthropic_tools, ToolConfig};
use crate::chat_manager::types::ProviderCredential;

pub struct CustomAnthropicAdapter {
    credential_config: Option<Value>,
}

impl CustomAnthropicAdapter {
    pub fn new(credential: &ProviderCredential) -> Self {
        Self {
            credential_config: credential.config.clone(),
        }
    }

    fn config_value(&self, key: &str) -> Option<String> {
        self.credential_config
            .as_ref()
            .and_then(|v| v.get(key))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    fn merge_same_role_messages(&self) -> bool {
        self.credential_config
            .as_ref()
            .and_then(|v| v.get("mergeSameRoleMessages"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    }

    fn auth_mode(&self) -> String {
        self.config_value("authMode")
            .unwrap_or_else(|| "header".to_string())
            .to_lowercase()
    }

    fn auth_header_name(&self) -> String {
        self.config_value("authHeaderName")
            .unwrap_or_else(|| "x-api-key".to_string())
    }

    fn auth_query_param_name(&self) -> String {
        self.config_value("authQueryParamName")
            .unwrap_or_else(|| "api_key".to_string())
    }

    fn append_query_auth(&self, url: String, api_key: &str) -> String {
        if self.auth_mode() != "query" || api_key.trim().is_empty() {
            return url;
        }
        let name = self.auth_query_param_name();
        let separator = if url.contains('?') { '&' } else { '?' };
        format!("{}{}{}={}", url, separator, name, api_key)
    }
}

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

impl ProviderAdapter for CustomAnthropicAdapter {
    fn endpoint(&self, base_url: &str) -> String {
        let path = self
            .config_value("chatEndpoint")
            .unwrap_or_else(|| "/v1/messages".to_string());
        format!("{}{}", base_url.trim_end_matches('/'), path)
    }

    fn system_role(&self) -> Cow<'static, str> {
        self.config_value("systemRole")
            .map(|s| Cow::Owned(s))
            .unwrap_or(Cow::Borrowed("system"))
    }

    fn build_url(
        &self,
        base_url: &str,
        _model_name: &str,
        api_key: &str,
        _should_stream: bool,
    ) -> String {
        self.append_query_auth(self.endpoint(base_url), api_key)
    }

    fn list_models_endpoint(&self, base_url: &str) -> String {
        let endpoint = self
            .config_value("modelsEndpoint")
            .unwrap_or_else(|| "/v1/models".to_string());
        if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
            endpoint
        } else if endpoint.starts_with('/') {
            format!("{}{}", base_url.trim_end_matches('/'), endpoint)
        } else {
            format!("{}/{}", base_url.trim_end_matches('/'), endpoint)
        }
    }

    fn supports_stream(&self) -> bool {
        self.credential_config
            .as_ref()
            .and_then(|v| v.get("supportsStream"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    }

    fn required_auth_headers(&self) -> &'static [&'static str] {
        &["x-api-key"]
    }

    fn default_headers_template(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        map.insert("x-api-key".to_string(), "$API_KEY".to_string());
        map
    }

    fn headers(
        &self,
        api_key: &str,
        extra: Option<&HashMap<String, String>>,
    ) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("Accept".to_string(), "text/event-stream".to_string());
        headers.insert("anthropic-version".to_string(), "2023-06-01".to_string());

        match self.auth_mode().as_str() {
            "none" => {}
            "header" => {
                let key = self.auth_header_name();
                if !api_key.trim().is_empty() {
                    headers.insert(key, api_key.to_string());
                }
            }
            "query" => {}
            _ => {
                if !api_key.trim().is_empty() {
                    headers.insert("Authorization".to_string(), format!("Bearer {}", api_key));
                }
            }
        }

        if let Some(extra_headers) = extra {
            for (k, v) in extra_headers {
                headers.insert(k.clone(), v.clone());
            }
        }
        headers
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
        // Get custom role mappings
        let user_role = self
            .config_value("userRole")
            .unwrap_or_else(|| "user".to_string());
        let assistant_role = self
            .config_value("assistantRole")
            .unwrap_or_else(|| "assistant".to_string());

        let source_messages = if self.merge_same_role_messages() {
            combine_same_role_messages(messages_for_api)
        } else {
            messages_for_api.clone()
        };

        let mut msgs: Vec<AnthropicMessage> = Vec::new();
        let mut system_parts: Vec<String> = Vec::new();

        if let Some(s) = system_prompt {
            if !s.is_empty() {
                system_parts.push(s);
            }
        }

        for msg in &source_messages {
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

            // Map to custom roles
            let mapped_role = if role == "assistant" {
                assistant_role.clone()
            } else {
                user_role.clone()
            };

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
}

fn combine_same_role_messages(messages: &[Value]) -> Vec<Value> {
    let mut combined: Vec<Value> = Vec::new();

    for msg in messages {
        let role = msg.get("role").and_then(|v| v.as_str());
        let content = msg.get("content").and_then(|v| v.as_str());

        if role.is_none() || content.is_none() {
            combined.push(msg.clone());
            continue;
        }

        let mut merged = false;
        if let Some(last) = combined.last_mut() {
            let last_role = last.get("role").and_then(|v| v.as_str());
            if last_role == role {
                if let Value::Object(map) = last {
                    if let Some(Value::String(existing)) = map.get_mut("content") {
                        if !existing.is_empty() {
                            existing.push_str("\n\n");
                        }
                        existing.push_str(content.unwrap());
                        merged = true;
                    }
                }
            }
        }

        if !merged {
            combined.push(msg.clone());
        }
    }

    combined
}
