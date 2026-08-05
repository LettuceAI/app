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

        let messages = normalize_system_messages(messages_for_api);

        let mut body = json!({
            "model": model_name,
            "messages": messages,
            "stream": should_stream,
        });

        if let Some(map) = body.as_object_mut() {
            if let Some(tools) = tools {
                map.insert("tools".to_string(), Value::Array(tools));
            }
            if let Some(choice) = tool_choice {
                map.insert("tool_choice".to_string(), choice);
            }
            // Only send `think` when reasoning is actually enabled. Ollama rejects
            // the field outright for models without a thinking template ("... does
            // not support thinking", 400), so a stale effort value must never leak
            // through when the reasoning toggle is off.
            if reasoning_enabled {
                if let Some(effort) = reasoning_effort {
                    map.insert("think".to_string(), Value::String(effort));
                } else {
                    map.insert("think".to_string(), Value::Bool(true));
                }
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
                        input_modalities: None,
                        output_modalities: None,
                        supported_endpoints: None,
                        input_price: None,
                        output_price: None,
                    });
                }
            }
        }
        models
    }
}

/// Reconcile Lettuce's message layout with the strict `system`-placement rules
/// baked into many Ollama chat templates.
///
/// Qwen-style templates (and a good number of llama.cpp ones) enforce that at
/// most one `system` message exists and that it is the very first message. Any
/// other `system` message trips a `raise_exception('System message must be at
/// the beginning.')` guard, which Ollama surfaces as an opaque
/// `400 Unable to generate parser for this template`. Lettuce legitimately
/// injects depth-positioned system entries mid-conversation (author's notes,
/// interval reminders, lorebook entries), so without normalization those
/// requests fail outright.
///
/// We therefore fold the leading run of `system` messages into a single opening
/// block and demote every later `system` message to `user`, preserving its
/// content at the same position.
fn normalize_system_messages(messages: &[Value]) -> Vec<Value> {
    fn is_system(msg: &Value) -> bool {
        msg.get("role").and_then(Value::as_str) == Some("system")
    }

    let leading_system = messages.iter().take_while(|m| is_system(m)).count();

    let mut out: Vec<Value> = Vec::with_capacity(messages.len());

    match leading_system {
        // Leave a single (or absent) leading system block untouched so we don't
        // reshape well-formed content — this is the common case.
        0 | 1 => {
            if leading_system == 1 {
                out.push(messages[0].clone());
            }
        }
        // Multiple leading system messages already violate the "exactly one"
        // rule, so merge their text into one opening block.
        _ => {
            let mut merged = String::new();
            for msg in &messages[..leading_system] {
                let content = msg.get("content").and_then(Value::as_str).unwrap_or("");
                if content.is_empty() {
                    continue;
                }
                if !merged.is_empty() {
                    merged.push_str("\n\n");
                }
                merged.push_str(content);
            }
            let mut opening = messages[0].clone();
            if let Some(obj) = opening.as_object_mut() {
                obj.insert("content".to_string(), Value::String(merged));
            }
            out.push(opening);
        }
    }

    for msg in &messages[leading_system..] {
        if is_system(msg) {
            let mut demoted = msg.clone();
            if let Some(obj) = demoted.as_object_mut() {
                obj.insert("role".to_string(), Value::String("user".to_string()));
            }
            out.push(demoted);
        } else {
            out.push(msg.clone());
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::normalize_system_messages;
    use serde_json::json;

    fn role(msg: &serde_json::Value) -> &str {
        msg.get("role").and_then(|r| r.as_str()).unwrap_or("")
    }

    #[test]
    fn leaves_leading_system_untouched() {
        let input = vec![
            json!({"role": "system", "content": "main"}),
            json!({"role": "user", "content": "hi"}),
        ];
        let out = normalize_system_messages(&input);
        assert_eq!(out, input);
    }

    #[test]
    fn demotes_mid_conversation_system_to_user() {
        let input = vec![
            json!({"role": "system", "content": "main"}),
            json!({"role": "user", "content": "hi"}),
            json!({"role": "system", "content": "author note"}),
            json!({"role": "assistant", "content": "ok"}),
        ];
        let out = normalize_system_messages(&input);
        assert_eq!(role(&out[0]), "system");
        assert_eq!(role(&out[1]), "user");
        assert_eq!(role(&out[2]), "user");
        assert_eq!(out[2]["content"], json!("author note"));
        assert_eq!(role(&out[3]), "assistant");
    }

    #[test]
    fn demotes_system_when_no_leading_system() {
        let input = vec![
            json!({"role": "user", "content": "hi"}),
            json!({"role": "system", "content": "be nice"}),
            json!({"role": "user", "content": "hello again"}),
        ];
        let out = normalize_system_messages(&input);
        assert_eq!(role(&out[0]), "user");
        assert_eq!(role(&out[1]), "user");
        assert_eq!(out[1]["content"], json!("be nice"));
        assert_eq!(role(&out[2]), "user");
    }

    #[test]
    fn merges_multiple_leading_system_messages() {
        let input = vec![
            json!({"role": "system", "content": "first"}),
            json!({"role": "system", "content": "second"}),
            json!({"role": "user", "content": "hi"}),
        ];
        let out = normalize_system_messages(&input);
        assert_eq!(out.len(), 2);
        assert_eq!(role(&out[0]), "system");
        assert_eq!(out[0]["content"], json!("first\n\nsecond"));
        assert_eq!(role(&out[1]), "user");
    }
}
