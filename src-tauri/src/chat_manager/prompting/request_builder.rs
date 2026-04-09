use serde_json::{json, Value};
use std::collections::HashMap;

use super::request::provider_base_url;
use crate::chat_manager::provider_adapter::adapter_for;
use crate::chat_manager::tooling::ToolConfig;
use crate::chat_manager::types::ProviderCredential;

pub struct BuiltRequest {
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Value,
    pub stream: bool,
    pub request_id: Option<String>,
}

pub fn provider_streaming_enabled(credential: &ProviderCredential) -> bool {
    credential
        .config
        .as_ref()
        .and_then(|config| config.get("streamingEnabled"))
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

pub fn effective_streaming_enabled(credential: &ProviderCredential, should_stream: bool) -> bool {
    should_stream
        && adapter_for(credential).supports_stream()
        && provider_streaming_enabled(credential)
}

// ---------------------------------------------------------------------------
// Prompt-caching helpers
// ---------------------------------------------------------------------------

fn apply_cache_control(content: &mut Value, cache_control: &Value) {
    if let Some(text) = content.as_str() {
        *content = json!([{
            "type": "text",
            "text": text,
            "cache_control": cache_control
        }]);
    } else if let Some(arr) = content.as_array_mut() {
        if let Some(last) = arr.last_mut().and_then(|item| item.as_object_mut()) {
            if last.get("type").and_then(|t| t.as_str()) == Some("text") {
                last.insert("cache_control".to_string(), cache_control.clone());
            }
        }
    }
}

fn supports_explicit_prompt_caching(credential: &ProviderCredential) -> bool {
    matches!(
        credential.provider_id.as_str(),
        "anthropic" | "custom-anthropic" | "openrouter"
    )
}

fn apply_cache_control_to_system_message(
    body_obj: &mut serde_json::Map<String, Value>,
    cache_control: &Value,
) {
    if let Some(system) = body_obj.get_mut("system") {
        apply_cache_control(system, cache_control);
        return;
    }

    if let Some(messages) = body_obj.get_mut("messages").and_then(|m| m.as_array_mut()) {
        for msg in messages.iter_mut() {
            let role = msg.get("role").and_then(|r| r.as_str());
            if role == Some("system") || role == Some("developer") {
                if let Some(content) = msg.get_mut("content") {
                    apply_cache_control(content, cache_control);
                }
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Main Builder
// ---------------------------------------------------------------------------

/// Build a provider-specific chat API request (endpoint, headers, body).
/// This function accepts messages normalized into OpenAI-style
/// role/content objects and adapts them for each provider.
pub fn build_chat_request(
    credential: &ProviderCredential,
    api_key: &str,
    model_name: &str,
    messages_for_api: &Vec<Value>,
    system_prompt: Option<String>,
    temperature: Option<f64>,
    top_p: Option<f64>,
    max_tokens: u32,
    context_length: Option<u32>,
    should_stream: bool,
    request_id: Option<String>,
    frequency_penalty: Option<f64>,
    presence_penalty: Option<f64>,
    top_k: Option<u32>,
    tool_config: Option<&ToolConfig>,
    reasoning_enabled: bool,
    reasoning_effort: Option<String>,
    reasoning_budget: Option<u32>,
    prompt_caching_enabled: bool,
    extra_body_fields: Option<HashMap<String, Value>>,
) -> BuiltRequest {
    let base_url = provider_base_url(credential);
    let adapter = adapter_for(credential);
    let effective_stream = effective_streaming_enabled(credential, should_stream);
    let url = adapter.build_url(&base_url, model_name, api_key, effective_stream);
    let headers = adapter.headers(api_key, credential.headers.as_ref());
    let mut body = adapter.body(
        model_name,
        messages_for_api,
        system_prompt,
        temperature,
        top_p,
        max_tokens,
        context_length,
        effective_stream,
        frequency_penalty,
        presence_penalty,
        top_k,
        tool_config,
        reasoning_enabled,
        reasoning_effort,
        reasoning_budget,
    );

    if prompt_caching_enabled && supports_explicit_prompt_caching(credential) {
        // Extract TTL safely using the updated camelCase key
        let ttl_val = extra_body_fields
            .as_ref()
            .and_then(|fields| fields.get("promptCachingTtl"))
            .and_then(|val| val.as_str())
            .unwrap_or("5min");

        let cache_control = if ttl_val == "1h" {
            json!({"type": "ephemeral", "ttl": "1h"}) // Anthropic/OpenRouter require the exact string "1h"
        } else {
            json!({"type": "ephemeral"})
        };

        if let Some(body_obj) = body.as_object_mut() {
            // 1. System prompt
            apply_cache_control_to_system_message(body_obj, &cache_control);

            // 2. Tool definitions
            if let Some(tools) = body_obj.get_mut("tools") {
                if let Some(arr) = tools.as_array_mut() {
                    if let Some(last_tool) = arr.last_mut() {
                        if let Some(tool_obj) = last_tool.as_object_mut() {
                            tool_obj.insert("cache_control".to_string(), cache_control.clone());
                        }
                    }
                }
            }

            // 3. Last user message
            if let Some(messages) = body_obj.get_mut("messages").and_then(|m| m.as_array_mut()) {
                for msg in messages.iter_mut().rev() {
                    if msg.get("role").and_then(|r| r.as_str()) == Some("user") {
                        if let Some(content) = msg.get_mut("content") {
                            apply_cache_control(content, &cache_control);
                        }
                        break;
                    }
                }
            }
        }
    }

    if let (Some(extra), Some(map)) = (extra_body_fields, body.as_object_mut()) {
        for (key, mut value) in extra {
            if key == "promptCachingTtl" {
                continue;
            }
            // OpenRouter sticky routing only applies to OpenRouter payloads.
            if prompt_caching_enabled && credential.provider_id == "openrouter" && key == "provider"
            {
                if let Some(provider_obj) = value.as_object_mut() {
                    provider_obj.remove("order");
                    if provider_obj.is_empty() {
                        continue; // Completely drop the "provider" key from the payload
                    }
                }
            }
            map.insert(key, value);
        }
    }

    BuiltRequest {
        url,
        headers,
        body,
        stream: effective_stream,
        request_id: if effective_stream { request_id } else { None },
    }
}

/// Returns the preferred system role keyword for the given provider.
pub fn system_role_for(credential: &ProviderCredential) -> std::borrow::Cow<'static, str> {
    adapter_for(credential).system_role()
}
