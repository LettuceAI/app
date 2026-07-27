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
        sse::SseDecoder,
        types::{ErrorEnvelope, NormalizedEvent, ProviderCredential},
    },
    infra::abort_manager::AbortRegistry,
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
        app,
        Some(&credential_runtime_headers(credential)),
        Some(DEFAULT_REQUEST_TIMEOUT_MS),
        false,
        credential_allows_invalid_tls(credential),
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
    let client = build_http_client(
        app,
        req.headers.as_ref(),
        req.timeout_ms,
        stream,
        request_allows_invalid_tls(app, req),
    )?;

    if stream {
        let request_id = req.request_id.clone().unwrap_or_default();
        return execute_streaming_chat_request(app, &client, &chat_body, req, &request_id).await;
    }

    execute_non_streaming_chat_request(app, &client, &req.url, &chat_body, req).await
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
    app: &tauri::AppHandle,
    headers: Option<&HashMap<String, String>>,
    timeout_ms: Option<u64>,
    stream: bool,
    allow_invalid_tls: bool,
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
    builder = crate::tls::apply_trusted_certificates(app, builder);
    if allow_invalid_tls {
        builder = builder.danger_accept_invalid_certs(true);
    }

    builder
        .build()
        .map_err(|err| crate::utils::err_to_string(module_path!(), line!(), err))
}

fn credential_allows_invalid_tls(credential: &ProviderCredential) -> bool {
    credential
        .config
        .as_ref()
        .and_then(|config| config.get("allowInvalidTls"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn request_allows_invalid_tls(app: &tauri::AppHandle, req: &ApiRequest) -> bool {
    crate::tls::allow_invalid_tls_for_request(app, Some("ollama"), &req.url)
}

pub fn normalize_base_url(raw: &str) -> Result<String, String> {
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

pub async fn normalize_request_body(body: &Value) -> Result<Value, String> {
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

pub fn normalize_assistant_tool_calls(
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
    let mut decoder = SseDecoder::new();

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

            saw_done |= emit_normalized_events(
                app,
                request_id,
                &mut decoder,
                &serde_json::to_string(&value)
                    .map_err(|err| crate::utils::err_to_string(module_path!(), line!(), err))?,
            );
            if saw_done {
                raw.push_str("data: [DONE]\n\n");
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
        saw_done |= emit_normalized_events(
            app,
            request_id,
            &mut decoder,
            &serde_json::to_string(&value)
                .map_err(|err| crate::utils::err_to_string(module_path!(), line!(), err))?,
        );
        if saw_done {
            raw.push_str("data: [DONE]\n\n");
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
    req: &ApiRequest,
) -> Result<ApiResponse, String> {
    let request_id = req.request_id.as_deref();
    let request = client.post(url).json(chat_body);
    let mut abort_rx = request_id.map(|req_id| {
        let registry = app.state::<AbortRegistry>();
        registry.register(req_id.to_string())
    });

    let emit_abort = || {
        if let Some(req_id) = request_id {
            let envelope = ErrorEnvelope {
                code: Some("ABORTED".to_string()),
                message: "Request was cancelled by user".to_string(),
                provider_id: req.provider_id.clone(),
                request_id: Some(req_id.to_string()),
                retryable: Some(false),
                status: None,
            };
            transport::emit_normalized(app, req_id, NormalizedEvent::Error { envelope });
        }
    };

    let response = if let Some(abort_rx) = abort_rx.as_mut() {
        tokio::select! {
            _ = abort_rx => {
                unregister_abort(app, request_id);
                emit_abort();
                return Err("Request was cancelled by user".to_string());
            }
            response = transport::send_with_retries(app, "ollama_chat", request, 2, request_id) => {
                match response {
                    Ok(response) => response,
                    Err(err) => {
                        unregister_abort(app, request_id);
                        return Err(err.to_string());
                    }
                }
            }
        }
    } else {
        transport::send_with_retries(app, "ollama_chat", request, 2, request_id)
            .await
            .map_err(|err| err.to_string())?
    };
    let status = response.status();
    let status_code = status.as_u16();
    let text = if let Some(abort_rx) = abort_rx.as_mut() {
        tokio::select! {
            _ = abort_rx => {
                unregister_abort(app, request_id);
                emit_abort();
                return Err("Request was cancelled by user".to_string());
            }
            text = response.text() => {
                text.map_err(|err| crate::utils::err_to_string(module_path!(), line!(), err))?
            }
        }
    } else {
        response
            .text()
            .await
            .map_err(|err| crate::utils::err_to_string(module_path!(), line!(), err))?
    };
    unregister_abort(app, request_id);

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

fn unregister_abort(app: &tauri::AppHandle, request_id: Option<&str>) {
    let Some(request_id) = request_id else {
        return;
    };
    let registry = app.state::<AbortRegistry>();
    registry.unregister(request_id);
}

fn emit_normalized_events(
    app: &tauri::AppHandle,
    request_id: &str,
    decoder: &mut SseDecoder,
    line: &str,
) -> bool {
    let mut saw_done = false;
    for event in decoder.feed(&format!("{line}\n"), Some("ollama")) {
        if matches!(event, NormalizedEvent::Done) {
            saw_done = true;
        }
        transport::emit_normalized(app, request_id, event);
    }
    saw_done
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

#[derive(Debug, Clone, serde::Serialize)]
pub struct OllamaInstalledModel {
    pub name: String,
    pub size: Option<u64>,
    pub modified_at: Option<String>,
    pub digest: Option<String>,
    pub parameter_size: Option<String>,
    pub quantization_level: Option<String>,
    pub family: Option<String>,
}

#[tauri::command]
pub async fn ollama_inventory_list(
    app: tauri::AppHandle,
    credential_id: String,
) -> Result<Vec<OllamaInstalledModel>, String> {
    let credential =
        crate::storage_manager::providers::get_provider_credential(&app, &credential_id)?;
    if !is_ollama_provider(Some(credential.provider_id.as_str())) {
        return Err("Selected provider is not an Ollama provider".to_string());
    }

    let base_url = normalize_base_url(
        credential
            .base_url
            .as_deref()
            .unwrap_or(DEFAULT_OLLAMA_BASE_URL),
    )?;
    let url = format!("{}api/tags", base_url);
    let client = build_http_client(
        &app,
        Some(&credential_runtime_headers(&credential)),
        Some(DEFAULT_REQUEST_TIMEOUT_MS),
        false,
        credential_allows_invalid_tls(&credential),
    )?;

    let request = client.get(&url);
    let response = transport::send_with_retries(&app, "ollama_inventory_list", request, 2, None)
        .await
        .map_err(|err| err.to_string())?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|err| crate::utils::err_to_string(module_path!(), line!(), err))?;
    if !status.is_success() {
        return Err(format!("Ollama returned {}: {}", status, text));
    }

    let payload = serde_json::from_str::<Value>(&text)
        .map_err(|err| crate::utils::err_to_string(module_path!(), line!(), err))?;

    let mut out = Vec::new();
    let Some(list) = payload.get("models").and_then(Value::as_array) else {
        return Ok(out);
    };
    for item in list {
        let Some(name) = item.get("name").and_then(Value::as_str) else {
            continue;
        };
        let details = item.get("details");
        out.push(OllamaInstalledModel {
            name: name.to_string(),
            size: item.get("size").and_then(Value::as_u64),
            modified_at: item
                .get("modified_at")
                .and_then(Value::as_str)
                .map(str::to_string),
            digest: item
                .get("digest")
                .and_then(Value::as_str)
                .map(str::to_string),
            parameter_size: details
                .and_then(|d| d.get("parameter_size"))
                .and_then(Value::as_str)
                .map(str::to_string),
            quantization_level: details
                .and_then(|d| d.get("quantization_level"))
                .and_then(Value::as_str)
                .map(str::to_string),
            family: details
                .and_then(|d| d.get("family"))
                .and_then(Value::as_str)
                .map(str::to_string),
        });
    }
    Ok(out)
}

#[tauri::command]
pub async fn ollama_inventory_delete(
    app: tauri::AppHandle,
    credential_id: String,
    model_name: String,
) -> Result<(), String> {
    let credential =
        crate::storage_manager::providers::get_provider_credential(&app, &credential_id)?;
    if !is_ollama_provider(Some(credential.provider_id.as_str())) {
        return Err("Selected provider is not an Ollama provider".to_string());
    }

    let base_url = normalize_base_url(
        credential
            .base_url
            .as_deref()
            .unwrap_or(DEFAULT_OLLAMA_BASE_URL),
    )?;
    let url = format!("{}api/delete", base_url);
    let client = build_http_client(
        &app,
        Some(&credential_runtime_headers(&credential)),
        Some(DEFAULT_REQUEST_TIMEOUT_MS),
        false,
        credential_allows_invalid_tls(&credential),
    )?;

    let body = json!({ "name": model_name });
    let response = client
        .delete(&url)
        .json(&body)
        .send()
        .await
        .map_err(|err| format!("Ollama delete request failed: {err}"))?;
    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Ollama returned {}: {}", status, text));
    }
    Ok(())
}

#[tauri::command]
pub async fn ollama_pull_model(
    app: tauri::AppHandle,
    credential_id: String,
    model_ref: String,
    metadata: Option<crate::hf_browser::QueueDownloadMetadata>,
) -> Result<String, String> {
    let credential =
        crate::storage_manager::providers::get_provider_credential(&app, &credential_id)?;
    if !is_ollama_provider(Some(credential.provider_id.as_str())) {
        return Err("Selected provider is not an Ollama provider".to_string());
    }
    let trimmed_ref = model_ref.trim();
    if trimmed_ref.is_empty() {
        return Err("Model reference is empty".to_string());
    }

    let queue_id = uuid::Uuid::new_v4().to_string();
    let metadata = metadata.unwrap_or_default();
    let display_name = metadata
        .display_name
        .clone()
        .unwrap_or_else(|| trimmed_ref.to_string());

    let item = crate::hf_browser::QueuedDownload {
        id: queue_id.clone(),
        model_id: trimmed_ref.to_string(),
        filename: display_name.clone(),
        status: "queued".to_string(),
        phase: None,
        downloaded: 0,
        total: 0,
        speed_bytes_per_sec: 0,
        error: None,
        result_path: None,
        create_model_when_finished: metadata.create_model_when_finished,
        mmproj_file: metadata.mmproj_file.clone(),
        mtp_file: metadata.mtp_file,
        mtp_bundled: metadata.mtp_bundled,
        install_id: metadata.install_id.clone(),
        display_name: Some(display_name.clone()),
        context_length: metadata.context_length,
        kv_type: metadata.kv_type.clone(),
        llama_offload_kqv: metadata.llama_offload_kqv,
        llama_gpu_layers: metadata.llama_gpu_layers,
        llama_model_offload_mode: metadata.llama_model_offload_mode.clone(),
        download_role: metadata.download_role.clone(),
        queue_kind: Some("ollama".to_string()),
        asset_root: metadata.asset_root.clone(),
        install_kind: metadata.install_kind.clone(),
        variant: metadata.variant.clone(),
        voice_id: metadata.voice_id.clone(),
        download_url: Some(format!("ollama://{}/{}", credential.id, trimmed_ref)),
        destination_path: metadata.destination_path.clone(),
        expected_size: metadata.expected_size,
        sha256: metadata.sha256.clone(),
        runtime_release: metadata.runtime_release.clone(),
        runtime_asset: metadata.runtime_asset.clone(),
        force_redownload: metadata.force_redownload,
        lora_keywords: metadata.lora_keywords.clone(),
        lora_base_model: metadata.lora_base_model.clone(),
    };

    crate::hf_browser::enqueue_external_item(&app, item).await;

    let model_ref_owned = trimmed_ref.to_string();
    let queue_id_clone = queue_id.clone();
    let app_clone = app.clone();
    tokio::spawn(async move {
        let result =
            run_ollama_pull(&app_clone, &credential, &queue_id_clone, &model_ref_owned).await;
        match result {
            Ok(()) => {
                crate::hf_browser::finish_external_item(&app_clone, &queue_id_clone, true, None)
                    .await;
            }
            Err(err) => {
                if err == "__cancelled__" {
                    crate::hf_browser::cancel_external_item(&app_clone, &queue_id_clone).await;
                } else {
                    crate::hf_browser::finish_external_item(
                        &app_clone,
                        &queue_id_clone,
                        false,
                        Some(err),
                    )
                    .await;
                }
            }
        }
    });

    Ok(queue_id)
}

async fn run_ollama_pull(
    app: &tauri::AppHandle,
    credential: &ProviderCredential,
    queue_id: &str,
    model_ref: &str,
) -> Result<(), String> {
    let base_url = normalize_base_url(
        credential
            .base_url
            .as_deref()
            .unwrap_or(DEFAULT_OLLAMA_BASE_URL),
    )?;
    let url = format!("{}api/pull", base_url);
    let client = build_http_client(
        app,
        Some(&credential_runtime_headers(credential)),
        None,
        true,
        credential_allows_invalid_tls(credential),
    )?;

    let body = json!({ "model": model_ref, "stream": true });
    let response = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|err| format!("Ollama pull request failed: {err}"))?;

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Ollama returned {}: {}", status, text));
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut last_emit = std::time::Instant::now();
    let mut last_completed = 0u64;
    let mut last_total = 0u64;
    let mut last_status_label = String::from("downloading");
    let mut speed_anchor = std::time::Instant::now();
    let mut speed_anchor_bytes = 0u64;
    let mut speed_bytes_per_sec = 0u64;

    loop {
        if crate::hf_browser::external_item_cancel_requested(queue_id).await {
            return Err("__cancelled__".to_string());
        }

        let next = match stream.next().await {
            Some(item) => item,
            None => break,
        };
        let chunk = next.map_err(|err| format!("Ollama pull stream error: {err}"))?;
        let chunk_str = String::from_utf8_lossy(&chunk);
        buffer.push_str(&chunk_str);

        while let Some(idx) = buffer.find('\n') {
            let line = buffer[..idx].trim().to_string();
            buffer = buffer[idx + 1..].to_string();
            if line.is_empty() {
                continue;
            }
            let value: Value = match serde_json::from_str(&line) {
                Ok(value) => value,
                Err(_) => continue,
            };

            if let Some(error_text) = value.get("error").and_then(Value::as_str) {
                return Err(error_text.to_string());
            }

            let status_label = value
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or(last_status_label.as_str())
                .to_string();
            let completed = value
                .get("completed")
                .and_then(Value::as_u64)
                .unwrap_or(last_completed);
            let total = value
                .get("total")
                .and_then(Value::as_u64)
                .unwrap_or(last_total);

            if completed >= speed_anchor_bytes {
                let elapsed = speed_anchor.elapsed().as_millis() as u64;
                if elapsed >= 1_000 {
                    let delta = completed.saturating_sub(speed_anchor_bytes);
                    speed_bytes_per_sec = delta * 1_000 / elapsed.max(1);
                    speed_anchor = std::time::Instant::now();
                    speed_anchor_bytes = completed;
                }
            } else {
                speed_anchor = std::time::Instant::now();
                speed_anchor_bytes = completed;
                speed_bytes_per_sec = 0;
            }

            last_completed = completed;
            last_total = total;
            last_status_label = status_label.clone();

            if status_label.eq_ignore_ascii_case("success") {
                update_pull_progress(app, queue_id, "complete", last_completed, last_total, 0)
                    .await;
                return Ok(());
            }

            let mapped_status = map_ollama_status(&status_label);
            let should_emit =
                last_emit.elapsed().as_millis() > 150 || mapped_status != "downloading";
            if should_emit {
                update_pull_progress(
                    app,
                    queue_id,
                    mapped_status,
                    last_completed,
                    last_total,
                    speed_bytes_per_sec,
                )
                .await;
                last_emit = std::time::Instant::now();
            }
        }
    }

    let trailing = buffer.trim();
    if !trailing.is_empty() {
        if let Ok(value) = serde_json::from_str::<Value>(trailing) {
            if let Some(error_text) = value.get("error").and_then(Value::as_str) {
                return Err(error_text.to_string());
            }
            if value
                .get("status")
                .and_then(Value::as_str)
                .map(|s| s.eq_ignore_ascii_case("success"))
                .unwrap_or(false)
            {
                return Ok(());
            }
        }
    }

    Ok(())
}

async fn update_pull_progress(
    app: &tauri::AppHandle,
    queue_id: &str,
    status_label: &str,
    downloaded: u64,
    total: u64,
    speed: u64,
) {
    crate::hf_browser::update_external_progress(app, queue_id, status_label, downloaded, total)
        .await;
    crate::hf_browser::update_external_speed(app, queue_id, speed).await;
}

fn map_ollama_status(raw: &str) -> &'static str {
    let lower = raw.to_ascii_lowercase();
    if lower.starts_with("pulling") || lower.starts_with("downloading") {
        "downloading"
    } else if lower.contains("verify") || lower.contains("writing") {
        "downloading"
    } else if lower == "success" {
        "complete"
    } else {
        "downloading"
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
                input_modalities: None,
                output_modalities: None,
                supported_endpoints: None,
                input_price: None,
                output_price: None,
            });
        }
    }
    models
}
