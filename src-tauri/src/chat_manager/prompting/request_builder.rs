use serde_json::{json, Value};
use std::collections::HashMap;

use super::request::provider_base_url;
use crate::chat_manager::provider_adapter::adapter_for;
use crate::chat_manager::tooling::ToolConfig;
use crate::chat_manager::types::ProviderCredential;
use crate::providers::config::supported_extra_body_keys_for_provider;

pub struct BuiltRequest {
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Value,
    pub stream: bool,
    pub request_id: Option<String>,
}

fn should_force_local_parallel_tool_calls(
    credential: &ProviderCredential,
    tool_config: Option<&ToolConfig>,
) -> bool {
    credential.provider_id == "llamacpp"
        && tool_config
            .map(|config| !config.tools.is_empty())
            .unwrap_or(false)
}

fn strip_provider_incompatible_extra_fields(
    credential: &ProviderCredential,
    extra_body_fields: &mut HashMap<String, Value>,
) {
    let supported_keys = supported_extra_body_keys_for_provider(&credential.provider_id);
    extra_body_fields.retain(|key, _| supported_keys.contains(&key.as_str()));
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

pub fn effective_streaming_enabled_with_override(
    credential: &ProviderCredential,
    should_stream: bool,
    llama_streaming_enabled: Option<bool>,
) -> bool {
    let stream_allowed = if credential.provider_id.eq_ignore_ascii_case("llamacpp") {
        llama_streaming_enabled.unwrap_or(true)
    } else {
        true
    };

    effective_streaming_enabled(credential, should_stream) && stream_allowed
}

fn sanitize_outbound_messages(messages_for_api: &[Value]) -> Vec<Value> {
    messages_for_api
        .iter()
        .map(sanitize_outbound_message)
        .collect()
}

fn sanitize_outbound_message(message: &Value) -> Value {
    let Some(message_obj) = message.as_object() else {
        return message.clone();
    };

    let role = message_obj.get("role").cloned().unwrap_or(Value::Null);
    let content = message_obj.get("content").cloned().unwrap_or(Value::Null);
    let mut sanitized = serde_json::Map::from_iter([
        ("role".to_string(), role.clone()),
        ("content".to_string(), content),
    ]);

    if let Some(name) = message_obj.get("name").cloned() {
        sanitized.insert("name".to_string(), name);
    }
    if let Some(tool_calls) = message_obj.get("tool_calls").cloned() {
        sanitized.insert("tool_calls".to_string(), tool_calls);
    }
    if let Some(tool_call_id) = message_obj.get("tool_call_id").cloned() {
        sanitized.insert("tool_call_id".to_string(), tool_call_id);
    }

    Value::Object(sanitized)
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

fn supports_openai_prompt_cache_retention(credential: &ProviderCredential) -> bool {
    credential.provider_id == "openai"
}

fn supports_gemini_explicit_prompt_caching(credential: &ProviderCredential) -> bool {
    // explicit cachedContents only — excludes express (it uses implicit caching)
    crate::gemini_cache::is_gemini_provider(Some(&credential.provider_id))
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
    let sanitized_messages_for_api = sanitize_outbound_messages(messages_for_api);
    let llama_streaming_enabled = extra_body_fields
        .as_ref()
        .and_then(|fields| fields.get("llamaStreamingEnabled"))
        .and_then(Value::as_bool);
    let effective_stream = effective_streaming_enabled_with_override(
        credential,
        should_stream,
        llama_streaming_enabled,
    ) && !adapter.disables_streaming_for_model(model_name);
    let url = adapter.build_url(&base_url, model_name, api_key, effective_stream);
    let headers = adapter.headers(api_key, credential.headers.as_ref());
    let mut body = adapter.body(
        model_name,
        &sanitized_messages_for_api,
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

    let mut extra_body_fields = extra_body_fields.unwrap_or_default();
    strip_provider_incompatible_extra_fields(credential, &mut extra_body_fields);
    if should_force_local_parallel_tool_calls(credential, tool_config)
        && !extra_body_fields.contains_key("parallel_tool_calls")
    {
        extra_body_fields.insert("parallel_tool_calls".to_string(), json!(true));
    }

    if prompt_caching_enabled && supports_explicit_prompt_caching(credential) {
        // Extract TTL safely using the updated camelCase key
        let ttl_val = extra_body_fields
            .get("promptCachingTtl")
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

    if prompt_caching_enabled && supports_openai_prompt_cache_retention(credential) {
        let retention = match extra_body_fields
            .get("promptCachingTtl")
            .and_then(|val| val.as_str())
            .unwrap_or("in_memory")
        {
            "24h" => Some("24h"),
            "in_memory" | "5min" | "1h" => Some("in_memory"),
            _ => None,
        };

        if let (Some(body_obj), Some(retention)) = (body.as_object_mut(), retention) {
            body_obj.insert(
                "prompt_cache_retention".to_string(),
                Value::String(retention.to_string()),
            );
        }
    }

    if prompt_caching_enabled && supports_gemini_explicit_prompt_caching(credential) {
        let ttl = match extra_body_fields
            .get("promptCachingTtl")
            .and_then(|val| val.as_str())
            .unwrap_or("1h")
        {
            "5min" | "300s" => "300s",
            _ => "3600s",
        };

        if let Some(body_obj) = body.as_object_mut() {
            body_obj.insert(
                "_lettucePromptCachingEnabled".to_string(),
                Value::Bool(true),
            );
            body_obj.insert(
                "_lettucePromptCachingTtl".to_string(),
                Value::String(ttl.to_string()),
            );
        }
    }

    if let Some(map) = body.as_object_mut() {
        for (key, mut value) in extra_body_fields {
            if key == "promptCachingTtl" || key == "llamaStreamingEnabled" {
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
        request_id,
    }
}

/// Returns the preferred system role keyword for the given provider.
pub fn system_role_for(credential: &ProviderCredential) -> std::borrow::Cow<'static, str> {
    adapter_for(credential).system_role()
}
