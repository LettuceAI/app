use std::collections::HashMap;

use base64::Engine;
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::{json, Value};
use tauri::Manager;
use url::Url;

use crate::{
    api::{ApiRequest, ApiResponse},
    chat_manager::{
        provider_adapter::ModelInfo,
        types::{ErrorEnvelope, NormalizedEvent, ProviderCredential},
    },
    transport::{self, DEFAULT_REQUEST_TIMEOUT_MS},
};

const DEFAULT_OLLAMA_BASE_URL: &str = "http://127.0.0.1:11434";

pub fn is_ollama_provider(provider_id: Option<&str>) -> bool {
    matches!(provider_id, Some("ollama"))
}

pub async fn list_models(
    app: &tauri::AppHandle,
    credential: &ProviderCredential,
) -> Result<Vec<ModelInfo>, String> {
    let base_url = normalize_base_url(
        credential
            .base_url
            .as_deref()
            .unwrap_or(DEFAULT_OLLAMA_BASE_URL),
    )?;
    let url = format!("{}api/tags", base_url);
    let client = build_http_client(
        Some(&credential_runtime_headers(credential)),
        Some(DEFAULT_REQUEST_TIMEOUT_MS),
        false,
    )?;

    let request = client.get(&url);
    let response = transport::send_with_retries(app, "ollama_list_models", request, 2, None)
        .await
        .map_err(|err| err.to_string())?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|err| crate::utils::err_to_string(module_path!(), line!(), err))?;

    if !status.is_success() {
        return Err(format!("Provider returned error {}: {}", status, text));
    }

    let payload = serde_json::from_str::<Value>(&text)
        .map_err(|err| crate::utils::err_to_string(module_path!(), line!(), err))?;

    Ok(parse_models_list(&payload))
}

pub async fn execute_chat_request(
    app: &tauri::AppHandle,
    req: &ApiRequest,
) -> Result<ApiResponse, String> {
    let stream = req.stream.unwrap_or(false) && req.request_id.is_some();
    let body = req
        .body
        .as_ref()
        .ok_or_else(|| "Ollama request body is required".to_string())?;
    let chat_body = normalize_request_body(body).await?;
    let client = build_http_client(req.headers.as_ref(), req.timeout_ms, stream)?;

    if stream {
        let request_id = req.request_id.clone().unwrap_or_default();
        return execute_streaming_chat_request(app, &client, &chat_body, req, &request_id).await;
    }

    execute_non_streaming_chat_request(
        app,
        &client,
        &req.url,
        &chat_body,
        req.request_id.as_deref(),
    )
    .await
}

fn credential_runtime_headers(credential: &ProviderCredential) -> HashMap<String, String> {
    let mut headers = credential.headers.clone().unwrap_or_default();
    if let Some(api_key) = credential.api_key.as_deref() {
        let trimmed = api_key.trim();
        if !trimmed.is_empty() {
            headers
                .entry("Authorization".to_string())
                .or_insert_with(|| format!("Bearer {}", trimmed));
        }
    }
    headers
}

fn build_http_client(
    headers: Option<&HashMap<String, String>>,
    timeout_ms: Option<u64>,
    stream: bool,
) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder();
    if !stream {
        let timeout = timeout_ms
            .unwrap_or(DEFAULT_REQUEST_TIMEOUT_MS)
            .min(DEFAULT_REQUEST_TIMEOUT_MS);
        builder = builder.timeout(std::time::Duration::from_millis(timeout));
    }

    let header_map = build_header_map(headers)?;
    if !header_map.is_empty() {
        builder = builder.default_headers(header_map);
    }

    builder
        .build()
        .map_err(|err| crate::utils::err_to_string(module_path!(), line!(), err))
}

fn normalize_base_url(raw: &str) -> Result<String, String> {
    let initial = if raw.trim().is_empty() {
        DEFAULT_OLLAMA_BASE_URL
    } else {
        raw.trim()
    };
    let mut url = Url::parse(initial)
        .map_err(|err| crate::utils::err_to_string(module_path!(), line!(), err))?;

    let path = url.path().trim_end_matches('/');
    let normalized_path = path
        .strip_suffix("/api/chat")
        .or_else(|| path.strip_suffix("/api/tags"))
        .or_else(|| path.strip_suffix("/v1"))
        .unwrap_or(path)
        .trim_end_matches('/');

    if normalized_path.is_empty() {
        url.set_path("/");
    } else {
        url.set_path(&format!("{}/", normalized_path));
    }
    url.set_query(None);
    url.set_fragment(None);

    Ok(url.to_string())
}

fn build_header_map(headers: Option<&HashMap<String, String>>) -> Result<HeaderMap, String> {
    let mut header_map = HeaderMap::new();
    let Some(headers) = headers else {
        return Ok(header_map);
    };

    for (key, value) in headers {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        if key.eq_ignore_ascii_case("authorization") && trimmed.eq_ignore_ascii_case("bearer") {
            continue;
        }

        let name = HeaderName::from_bytes(key.as_bytes())
            .map_err(|err| crate::utils::err_to_string(module_path!(), line!(), err))?;
        let value = HeaderValue::from_str(trimmed)
            .map_err(|err| crate::utils::err_to_string(module_path!(), line!(), err))?;
        header_map.insert(name, value);
    }

    Ok(header_map)
}

async fn normalize_request_body(body: &Value) -> Result<Value, String> {
    let mut object = body
        .as_object()
        .ok_or_else(|| "Ollama request body must be a JSON object".to_string())?
        .clone();

    let model_name = object
        .get("model")
        .and_then(Value::as_str)
        .ok_or_else(|| "Ollama request body is missing model".to_string())?;
    if model_name.trim().is_empty() {
        return Err("Ollama request body has an empty model".to_string());
    }

    let messages = object
        .get("messages")
        .and_then(Value::as_array)
        .ok_or_else(|| "Ollama request body is missing messages".to_string())?;

    let mut tool_call_name_by_id: HashMap<String, String> = HashMap::new();
    let mut converted_messages = Vec::with_capacity(messages.len());
    for message in messages {
        converted_messages.push(convert_message(message, &mut tool_call_name_by_id).await?);
    }
    object.insert("messages".to_string(), Value::Array(converted_messages));

    if !object.contains_key("think") {
        if let Some(reasoning_effort) = object
            .get("reasoning_effort")
            .cloned()
            .or_else(|| object.get("reasoningEffort").cloned())
        {
            object.insert("think".to_string(), reasoning_effort);
        } else if let Some(reasoning_enabled) = object
            .get("reasoning")
            .cloned()
            .or_else(|| object.get("reasoningEnabled").cloned())
        {
            object.insert("think".to_string(), reasoning_enabled);
        }
    }

    Ok(Value::Object(object))
}

async fn convert_message(
    value: &Value,
    tool_call_name_by_id: &mut HashMap<String, String>,
) -> Result<Value, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "Ollama message must be a JSON object".to_string())?;
    let role = object.get("role").and_then(Value::as_str).unwrap_or("user");
    let (content, images) = convert_content(object.get("content")).await?;

    let mut message = serde_json::Map::new();
    for (key, value) in object {
        if key == "content" || key == "images" {
            continue;
        }
        message.insert(key.clone(), value.clone());
    }

    message.insert("role".to_string(), Value::String(role.to_string()));
    message.insert("content".to_string(), Value::String(content));

    if role == "assistant" {
        if let Some(tool_calls) = object.get("tool_calls").and_then(Value::as_array) {
            let normalized = normalize_assistant_tool_calls(tool_calls, tool_call_name_by_id);
            if !normalized.is_empty() {
                message.insert("tool_calls".to_string(), Value::Array(normalized));
            }
        }
    }

    if role == "tool" {
        if let Some(tool_name) = object
            .get("tool_name")
            .and_then(Value::as_str)
            .map(|name| name.to_string())
            .or_else(|| {
                object
                    .get("tool_call_id")
                    .and_then(Value::as_str)
                    .and_then(|id| tool_call_name_by_id.get(id).cloned())
            })
        {
            message.insert("tool_name".to_string(), Value::String(tool_name));
        }
    }

    if !images.is_empty() {
        message.insert(
            "images".to_string(),
            Value::Array(images.into_iter().map(Value::String).collect()),
        );
    }

    Ok(Value::Object(message))
}

fn normalize_assistant_tool_calls(
    tool_calls: &[Value],
    tool_call_name_by_id: &mut HashMap<String, String>,
) -> Vec<Value> {
    tool_calls
        .iter()
        .enumerate()
        .filter_map(|(index, tool_call)| {
            let function = tool_call.get("function").unwrap_or(tool_call);
            let name = function.get("name").and_then(Value::as_str)?.to_string();

            if let Some(id) = tool_call.get("id").and_then(Value::as_str) {
                if !id.is_empty() {
                    tool_call_name_by_id.insert(id.to_string(), name.clone());
                }
            }

            let arguments = match function.get("arguments") {
                Some(Value::String(raw)) => serde_json::from_str::<Value>(raw)
                    .unwrap_or_else(|_| Value::String(raw.clone())),
                Some(other) => other.clone(),
                None => Value::Object(Default::default()),
            };

            Some(json!({
                "type": "function",
                "function": {
                    "index": function
                        .get("index")
                        .and_then(Value::as_u64)
                        .unwrap_or(index as u64),
                    "name": name,
                    "arguments": arguments
                }
            }))
        })
        .collect()
}

async fn convert_content(value: Option<&Value>) -> Result<(String, Vec<String>), String> {
    let Some(value) = value else {
        return Ok((String::new(), Vec::new()));
    };

    match value {
        Value::String(text) => Ok((text.clone(), Vec::new())),
        Value::Array(parts) => {
            let mut text = String::new();
            let mut images = Vec::new();

            for part in parts {
                let Some(object) = part.as_object() else {
                    continue;
                };
                match object.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        if let Some(part_text) = object.get("text").and_then(Value::as_str) {
                            text.push_str(part_text);
                        }
                    }
                    Some("image_url") => {
                        let Some(url) = object
                            .get("image_url")
                            .and_then(Value::as_object)
                            .and_then(|image| image.get("url"))
                            .and_then(Value::as_str)
                        else {
                            continue;
                        };
                        images.push(image_url_to_base64(url).await?);
                    }
                    _ => {}
                }
            }

            Ok((text, images))
        }
        other => Ok((other.to_string(), Vec::new())),
    }
}

async fn image_url_to_base64(url: &str) -> Result<String, String> {
    if let Some((_, data)) = crate::chat_manager::provider_adapter::parse_data_url(url) {
        return Ok(data);
    }

    if url.starts_with("http://") || url.starts_with("https://") {
        let bytes = reqwest::get(url)
            .await
            .map_err(|err| crate::utils::err_to_string(module_path!(), line!(), err))?
            .bytes()
            .await
            .map_err(|err| crate::utils::err_to_string(module_path!(), line!(), err))?;
        return Ok(base64::engine::general_purpose::STANDARD.encode(bytes));
    }

    Ok(url.to_string())
}

async fn execute_streaming_chat_request(
    app: &tauri::AppHandle,
    client: &reqwest::Client,
    chat_body: &Value,
    req: &ApiRequest,
    request_id: &str,
) -> Result<ApiResponse, String> {
    let request = client.post(&req.url).json(chat_body);
    let response =
        transport::send_with_retries(app, "ollama_chat_stream", request, 2, Some(request_id))
            .await
            .map_err(|err| err.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return error_response_from_http(status.as_u16(), response).await;
    }

    let mut stream = response.bytes_stream();
    let registry = app.state::<crate::abort_manager::AbortRegistry>();
    let mut abort_rx = registry.register(request_id.to_string());
    let mut raw = String::new();
    let mut saw_done = false;
    let mut buffer = String::new();

    loop {
        let next_item = tokio::select! {
            _ = &mut abort_rx => {
                registry.unregister(request_id);
                let envelope = ErrorEnvelope {
                    code: Some("ABORTED".to_string()),
                    message: "Request was cancelled by user".to_string(),
                    provider_id: req.provider_id.clone(),
                    request_id: Some(request_id.to_string()),
                    retryable: Some(false),
                    status: None,
                };
                transport::emit_normalized(app, request_id, NormalizedEvent::Error { envelope });
                return Err("Request was cancelled by user".to_string());
            }
            item = stream.next() => item,
        };

        let Some(item) = next_item else {
            break;
        };

        let chunk = match item {
            Ok(chunk) => chunk,
            Err(_) => {
                registry.unregister(request_id);
                let envelope = ErrorEnvelope {
                    code: Some("OLLAMA_STREAM_DECODE".to_string()),
                    message: "Failed to decode Ollama streaming response".to_string(),
                    provider_id: req.provider_id.clone(),
                    request_id: Some(request_id.to_string()),
                    retryable: Some(false),
                    status: Some(500),
                };
                transport::emit_normalized(app, request_id, NormalizedEvent::Error { envelope });
                return Err("Failed to decode Ollama streaming response".to_string());
            }
        };

        let chunk = String::from_utf8(chunk.to_vec())
            .map_err(|err| crate::utils::err_to_string(module_path!(), line!(), err))?;
        buffer.push_str(&chunk);

        while let Some(newline_index) = buffer.find('\n') {
            let line = buffer[..newline_index].trim().to_string();
            buffer = buffer[newline_index + 1..].to_string();

            if line.is_empty() {
                continue;
            }

            let mut value = serde_json::from_str::<Value>(&line)
                .map_err(|err| crate::utils::err_to_string(module_path!(), line!(), err))?;
            inject_missing_tool_call_ids(&mut value);

            raw.push_str("data: ");
            raw.push_str(&line);
            raw.push_str("\n\n");

            emit_normalized_events(app, request_id, &value);

            if value.get("done").and_then(Value::as_bool).unwrap_or(false) {
                transport::emit_normalized(app, request_id, NormalizedEvent::Done);
                raw.push_str("data: [DONE]\n\n");
                saw_done = true;
            }
        }
    }

    let trailing = buffer.trim();
    if !trailing.is_empty() {
        let mut value = serde_json::from_str::<Value>(trailing)
            .map_err(|err| crate::utils::err_to_string(module_path!(), line!(), err))?;
        inject_missing_tool_call_ids(&mut value);
        raw.push_str("data: ");
        raw.push_str(trailing);
        raw.push_str("\n\n");
        emit_normalized_events(app, request_id, &value);
        if value.get("done").and_then(Value::as_bool).unwrap_or(false) {
            transport::emit_normalized(app, request_id, NormalizedEvent::Done);
            raw.push_str("data: [DONE]\n\n");
            saw_done = true;
        }
    }

    registry.unregister(request_id);
    if !saw_done {
        transport::emit_normalized(app, request_id, NormalizedEvent::Done);
        raw.push_str("data: [DONE]\n\n");
    }

    Ok(ApiResponse {
        status: 200,
        ok: true,
        headers: HashMap::new(),
        data: Value::String(raw),
    })
}

async fn execute_non_streaming_chat_request(
    app: &tauri::AppHandle,
    client: &reqwest::Client,
    url: &str,
    chat_body: &Value,
    request_id: Option<&str>,
) -> Result<ApiResponse, String> {
    let request = client.post(url).json(chat_body);
    let response = transport::send_with_retries(app, "ollama_chat", request, 2, request_id)
        .await
        .map_err(|err| err.to_string())?;
    let status = response.status();
    let status_code = status.as_u16();
    let text = response
        .text()
        .await
        .map_err(|err| crate::utils::err_to_string(module_path!(), line!(), err))?;

    if !status.is_success() {
        let payload =
            serde_json::from_str::<Value>(&text).unwrap_or_else(|_| json!({ "error": text }));
        return Ok(ApiResponse {
            status: status_code,
            ok: false,
            headers: HashMap::new(),
            data: payload,
        });
    }

    let mut payload = serde_json::from_str::<Value>(&text)
        .map_err(|err| crate::utils::err_to_string(module_path!(), line!(), err))?;
    inject_missing_tool_call_ids(&mut payload);

    Ok(ApiResponse {
        status: status_code,
        ok: true,
        headers: HashMap::new(),
        data: payload,
    })
}

fn emit_normalized_events(app: &tauri::AppHandle, request_id: &str, value: &Value) {
    let calls = crate::chat_manager::tooling::parse_tool_calls("ollama", value);
    if !calls.is_empty() {
        transport::emit_normalized(app, request_id, NormalizedEvent::ToolCall { calls });
    }

    if let Some(text) = crate::chat_manager::request::extract_text(value, Some("ollama")) {
        if !text.is_empty() {
            transport::emit_normalized(app, request_id, NormalizedEvent::Delta { text });
        }
    }

    if let Some(text) = crate::chat_manager::request::extract_reasoning(value, Some("ollama")) {
        if !text.is_empty() {
            transport::emit_normalized(app, request_id, NormalizedEvent::Reasoning { text });
        }
    }

    if value.get("done").and_then(Value::as_bool).unwrap_or(false) {
        if let Some(usage) = crate::chat_manager::request::extract_usage(value) {
            transport::emit_normalized(app, request_id, NormalizedEvent::Usage { usage });
        }
    }
}

async fn error_response_from_http(
    status: u16,
    response: reqwest::Response,
) -> Result<ApiResponse, String> {
    let text = response
        .text()
        .await
        .map_err(|err| crate::utils::err_to_string(module_path!(), line!(), err))?;
    let payload = serde_json::from_str::<Value>(&text).unwrap_or_else(|_| json!({ "error": text }));
    Ok(ApiResponse {
        status,
        ok: false,
        headers: HashMap::new(),
        data: payload,
    })
}

fn inject_missing_tool_call_ids(value: &mut Value) {
    let Some(tool_calls) = value
        .get_mut("message")
        .and_then(Value::as_object_mut)
        .and_then(|message| message.get_mut("tool_calls"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };

    for (index, tool_call) in tool_calls.iter_mut().enumerate() {
        let Some(object) = tool_call.as_object_mut() else {
            continue;
        };
        object
            .entry("id".to_string())
            .or_insert_with(|| Value::String(format!("call_{}", index + 1)));
    }
}

fn parse_models_list(payload: &Value) -> Vec<ModelInfo> {
    let mut models = Vec::new();
    if let Some(list) = payload.get("models").and_then(Value::as_array) {
        for item in list {
            let Some(name) = item.get("name").and_then(Value::as_str) else {
                continue;
            };

            let description = item
                .get("details")
                .and_then(|details| details.get("parameter_size"))
                .and_then(Value::as_str)
                .map(|size| format!("{} parameters", size))
                .or_else(|| {
                    item.get("size")
                        .and_then(Value::as_u64)
                        .map(|size| format!("{} bytes", size))
                });

            models.push(ModelInfo {
                id: name.to_string(),
                display_name: Some(name.to_string()),
                description,
                context_length: None,
                input_price: None,
                output_price: None,
            });
        }
    }
    models
}

#[cfg(test)]
mod tests {
    use super::{normalize_assistant_tool_calls, normalize_base_url, normalize_request_body};
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn strips_api_chat_suffix() {
        let url = normalize_base_url("http://127.0.0.1:11434/api/chat").expect("url");
        assert_eq!(url, "http://127.0.0.1:11434/");
    }

    #[test]
    fn strips_v1_suffix() {
        let url = normalize_base_url("http://127.0.0.1:11434/v1").expect("url");
        assert_eq!(url, "http://127.0.0.1:11434/");
    }

    #[test]
    fn normalizes_openai_style_tool_calls_for_ollama() {
        let input = vec![json!({
            "id": "call_weather",
            "type": "function",
            "function": {
                "name": "get_weather",
                "arguments": "{\"city\":\"Istanbul\"}"
            }
        })];
        let mut map = HashMap::new();

        let normalized = normalize_assistant_tool_calls(&input, &mut map);

        assert_eq!(
            map.get("call_weather").map(String::as_str),
            Some("get_weather")
        );
        assert_eq!(
            normalized,
            vec![json!({
                "type": "function",
                "function": {
                    "index": 0,
                    "name": "get_weather",
                    "arguments": { "city": "Istanbul" }
                }
            })]
        );
    }

    #[tokio::test]
    async fn infers_tool_name_from_tool_call_id_for_tool_messages() {
        let body = json!({
            "model": "qwen3",
            "messages": [
                {
                    "role": "assistant",
                    "tool_calls": [{
                        "id": "call_1",
                        "function": {
                            "name": "get_weather",
                            "arguments": { "city": "Istanbul" }
                        }
                    }]
                },
                {
                    "role": "tool",
                    "tool_call_id": "call_1",
                    "content": "{\"temperature\":\"20C\"}"
                }
            ]
        });

        let normalized = normalize_request_body(&body)
            .await
            .expect("normalized body");
        let messages = normalized
            .get("messages")
            .and_then(|value| value.as_array())
            .expect("messages array");

        assert_eq!(
            messages[1]
                .get("tool_name")
                .and_then(|value| value.as_str()),
            Some("get_weather")
        );
    }
}
