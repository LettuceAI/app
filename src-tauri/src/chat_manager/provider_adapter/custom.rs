use std::borrow::Cow;
use std::collections::HashMap;

use serde_json::{json, Value};

use super::{OpenAIChatRequest, ProviderAdapter, ReasoningConfig};
use crate::chat_manager::tooling::{openai_tool_choice, openai_tools, ToolConfig};
use crate::chat_manager::types::ProviderCredential;

pub struct CustomGenericAdapter {
    credential_config: Option<Value>,
}

impl CustomGenericAdapter {
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

    fn tool_choice_mode(&self) -> String {
        self.config_value("toolChoiceMode")
            .unwrap_or_else(|| "auto".to_string())
            .to_lowercase()
    }
}

impl ProviderAdapter for CustomGenericAdapter {
    fn endpoint(&self, base_url: &str) -> String {
        let path = self
            .config_value("chatEndpoint")
            .unwrap_or_else(|| "/chat/completions".to_string());
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
        &["Authorization"]
    }

    fn default_headers_template(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        map.insert("Authorization".to_string(), "Bearer $API_KEY".to_string());
        map
    }

    fn headers(
        &self,
        api_key: &str,
        extra: Option<&HashMap<String, String>>,
    ) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());

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
        // Map messages if necessary
        let mut mapped_messages = if self.merge_same_role_messages() {
            combine_same_role_messages(messages_for_api)
        } else {
            messages_for_api.clone()
        };

        // Handle role mapping if configured
        if let Some(user_role) = self.config_value("userRole") {
            for msg in mapped_messages.iter_mut() {
                if msg["role"] == "user" {
                    msg["role"] = Value::String(user_role.clone());
                }
            }
        }
        if let Some(assistant_role) = self.config_value("assistantRole") {
            for msg in mapped_messages.iter_mut() {
                if msg["role"] == "assistant" {
                    msg["role"] = Value::String(assistant_role.clone());
                }
            }
        }
        if let Some(sys_role) = self.config_value("systemRole") {
            for msg in mapped_messages.iter_mut() {
                if msg["role"] == "system" {
                    msg["role"] = Value::String(sys_role.clone());
                }
            }
        }

        let (tools, tool_choice) = if let Some(cfg) = tool_config {
            let tools = openai_tools(cfg);
            let choice = if tools.is_some() {
                match self.tool_choice_mode().as_str() {
                    "auto" => Some(Value::String("auto".to_string())),
                    "none" => Some(Value::String("none".to_string())),
                    "required" => Some(Value::String("required".to_string())),
                    "omit" => None,
                    // "passthrough" preserves adapter/tool-config driven behavior.
                    "passthrough" => openai_tool_choice(cfg.choice.as_ref()),
                    _ => Some(Value::String("auto".to_string())),
                }
            } else {
                None
            };
            (tools, choice)
        } else {
            (None, None)
        };

        let request = OpenAIChatRequest {
            model: model_name,
            messages: &mapped_messages,
            stream: should_stream,
            temperature,
            top_p,
            max_tokens: Some(max_tokens),
            context_length,
            max_completion_tokens: None,
            frequency_penalty,
            presence_penalty,
            reasoning_effort: None, // Custom usually doesn't strictly support this yet unless generic OAI
            reasoning: if reasoning_enabled {
                Some(ReasoningConfig {
                    effort: reasoning_effort,
                    max_tokens: reasoning_budget,
                })
            } else {
                None
            },
            tools,
            tool_choice,
        };

        let mut val = serde_json::to_value(request).unwrap();

        // Inject chat_template_kwargs if explicitly enabled in provider config
        if self
            .credential_config
            .as_ref()
            .and_then(|v| v.get("sendChatTemplateKwargs"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            if let Some(obj) = val.as_object_mut() {
                obj.insert(
                    "chat_template_kwargs".to_string(),
                    json!({ "enable_thinking": reasoning_enabled }),
                );
            }
        }

        val
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
