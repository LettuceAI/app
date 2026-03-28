use std::collections::HashMap;

use serde_json::Value;

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
    extra_body_fields: Option<HashMap<String, Value>>,
) -> BuiltRequest {
    let base_url = provider_base_url(credential);

    let adapter = adapter_for(credential);
    let effective_stream = effective_streaming_enabled(credential, should_stream);
    let url = adapter.build_url(&base_url, model_name, api_key, effective_stream);
    let headers = adapter.headers(api_key, credential.headers.as_ref());

    let body = adapter.body(
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
    let mut body = body;
    if let (Some(extra), Some(map)) = (extra_body_fields, body.as_object_mut()) {
        for (key, value) in extra {
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
    let adapter = adapter_for(credential);
    adapter.system_role()
}
