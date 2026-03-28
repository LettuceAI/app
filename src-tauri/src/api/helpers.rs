use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;
use std::time::Duration;

use crate::{
    abort_manager::AbortRegistry,
    chat_manager::{
        request as chat_request, sse,
        tooling::parse_tool_calls,
        types::{ErrorEnvelope, NormalizedEvent},
    },
    content_filter::{ContentFilter, StreamFilterContext},
    serde_utils::{
        json_value_to_string, parse_body_to_value, sanitize_header_value, summarize_json,
    },
    transport::{emit_normalized, DEFAULT_REQUEST_TIMEOUT_MS},
    utils::{log_error, log_info, log_warn},
};

use super::ApiRequest;

fn enforce_pure_mode_on_text(
    app: &tauri::AppHandle,
    req: &ApiRequest,
    request_id: Option<&str>,
    text: &str,
) -> Result<(), String> {
    if text.trim().is_empty() {
        return Ok(());
    }

    use tauri::Manager;
    let Some(filter) = app.try_state::<ContentFilter>() else {
        return Ok(());
    };
    if !filter.is_enabled() {
        return Ok(());
    }

    let result = filter.check_text(text);
    if !result.blocked {
        return Ok(());
    }

    log_warn(
        app,
        "api_request",
        format!(
            "[api_request] content blocked by Pure Mode (score={:.1}, terms={:?})",
            result.score, result.matched_terms
        ),
    );

    if let Some(req_id) = request_id {
        let envelope = ErrorEnvelope {
            code: Some("CONTENT_BLOCKED".to_string()),
            message: "Response filtered by Pure Mode".to_string(),
            provider_id: req.provider_id.clone(),
            request_id: Some(req_id.to_string()),
            retryable: Some(true),
            status: None,
        };
        emit_normalized(app, req_id, NormalizedEvent::Error { envelope });
    }

    Err("Response blocked by Pure Mode. Try rephrasing your message.".to_string())
}

pub(crate) fn apply_query_params(
    app: &tauri::AppHandle,
    builder: reqwest::RequestBuilder,
    req: &ApiRequest,
) -> reqwest::RequestBuilder {
    if let Some(query) = &req.query {
        let mut params: Vec<(String, String)> = Vec::new();
        for (key, value) in query.iter() {
            if let Some(string_value) = json_value_to_string(value) {
                params.push((key.clone(), string_value));
            }
        }
        if !params.is_empty() {
            log_info(
                app,
                "api_request",
                format!("[api_request] adding query params: {:?}", params),
            );
            return builder.query(&params);
        }
    }
    builder
}

pub(crate) fn apply_headers(
    app: &tauri::AppHandle,
    builder: reqwest::RequestBuilder,
    req: &ApiRequest,
) -> reqwest::RequestBuilder {
    if let Some(headers) = &req.headers {
        let mut header_map = HeaderMap::new();
        for (key, value) in headers {
            log_info(
                app,
                "api_request",
                format!(
                    "[api_request] adding header: {}={}",
                    key,
                    sanitize_header_value(key, value)
                ),
            );
            if let (Ok(name), Ok(header_value)) = (
                HeaderName::from_bytes(key.as_bytes()),
                HeaderValue::from_str(value),
            ) {
                header_map.insert(name, header_value);
            } else {
                log_warn(
                    app,
                    "api_request",
                    format!("[api_request] invalid header: {}={}", key, value),
                );
            }
        }
        header_map.insert(
            HeaderName::from_bytes(b"HTTP-Referer").unwrap(),
            HeaderValue::from_static("https://github.com/LettuceAI/"),
        );
        header_map.insert(
            HeaderName::from_bytes(b"X-Title").unwrap(),
            HeaderValue::from_static("LettuceAI"),
        );
        log_info(app, "api_request", "All headers set");

        builder.headers(header_map)
    } else {
        let mut header_map = HeaderMap::new();
        header_map.insert(
            HeaderName::from_bytes(b"HTTP-Referer").unwrap(),
            HeaderValue::from_static("https://github.com/LettuceAI/"),
        );
        header_map.insert(
            HeaderName::from_bytes(b"X-Title").unwrap(),
            HeaderValue::from_static("LettuceAI"),
        );
        log_info(app, "api_request", "[api_request] using default headers");
        builder.headers(header_map)
    }
}

pub(crate) fn apply_body(
    app: &tauri::AppHandle,
    builder: reqwest::RequestBuilder,
    req: &ApiRequest,
) -> reqwest::RequestBuilder {
    if let Some(body) = req.body.clone() {
        if let Some(text) = body.as_str() {
            log_info(
                app,
                "api_request",
                format!(
                    "[api_request] setting body as text: {}",
                    crate::serde_utils::truncate_for_log(text, 512)
                ),
            );
            builder.body(text.to_owned())
        } else if !body.is_null() {
            log_info(
                app,
                "api_request",
                format!(
                    "[api_request] setting body as JSON: {}",
                    summarize_json(&body)
                ),
            );
            builder.json(&body)
        } else {
            builder
        }
    } else {
        builder
    }
}

pub(crate) async fn handle_streaming_response(
    app: &tauri::AppHandle,
    req: &ApiRequest,
    response: reqwest::Response,
    request_id: String,
    status: reqwest::StatusCode,
    ok: bool,
    url_for_log: &str,
) -> Result<Value, String> {
    let mut collected: Vec<u8> = Vec::new();
    let event_name = format!("api://{}", request_id);
    let mut body_stream = response.bytes_stream();
    log_info(
        app,
        "api_request",
        format!(
            "[api_request] streaming response for {} (provider={:?}, event={})",
            url_for_log, req.provider_id, event_name
        ),
    );

    // Register this request for abort capability
    let mut abort_rx = {
        use tauri::Manager;
        let registry = app.state::<AbortRegistry>();
        registry.register(request_id.clone())
    };

    let mut usage_emitted = false;
    let mut text_emitted = false;
    let mut decoder = sse::SseDecoder::new();
    let mut aborted = false;
    let idle_timeout_ms = req
        .timeout_ms
        .unwrap_or(DEFAULT_REQUEST_TIMEOUT_MS)
        .min(DEFAULT_REQUEST_TIMEOUT_MS);
    let idle_timeout = Duration::from_millis(idle_timeout_ms);

    // Content filter: check if enabled and create streaming context
    let content_filter_active = {
        use tauri::Manager;
        app.try_state::<ContentFilter>()
            .map(|f| f.is_enabled())
            .unwrap_or(false)
    };
    let mut stream_filter_ctx = StreamFilterContext::new();

    loop {
        let next_chunk = async { tokio::time::timeout(idle_timeout, body_stream.next()).await };

        tokio::select! {
            _ = &mut abort_rx => {
                log_warn(
                    app,
                    "api_request",
                    format!("[api_request] request aborted by user: {}", url_for_log),
                );
                aborted = true;
                break;
            }
            chunk_result = next_chunk => {
                match chunk_result {
                    Ok(Some(Ok(chunk))) => {
                        let text = String::from_utf8_lossy(&chunk).to_string();
                        let mut content_blocked = false;
                        for event in decoder.feed(&text, req.provider_id.as_deref()) {
                            if content_filter_active {
                                if let NormalizedEvent::Delta { text: ref delta_text } = event {
                                    use tauri::Manager;
                                    if let Some(filter) = app.try_state::<ContentFilter>() {
                                        let result = filter.check_delta(&mut stream_filter_ctx, delta_text);
                                        if result.blocked {
                                            let envelope = ErrorEnvelope {
                                                code: Some("CONTENT_BLOCKED".to_string()),
                                                message: "Response filtered by Pure Mode".to_string(),
                                                provider_id: req.provider_id.clone(),
                                                request_id: Some(request_id.clone()),
                                                retryable: Some(true),
                                                status: None,
                                            };
                                            emit_normalized(app, &request_id, NormalizedEvent::Error { envelope });
                                            content_blocked = true;
                                            break;
                                        }
                                    }
                                }
                            }
                            match &event {
                                NormalizedEvent::Usage { .. } => { usage_emitted = true; }
                                NormalizedEvent::Delta { .. } => { text_emitted = true; }
                                _ => {}
                            }
                            emit_normalized(app, &request_id, event);
                        }
                        if content_blocked {
                            use tauri::Manager;
                            let registry = app.state::<AbortRegistry>();
                            registry.unregister(&request_id);
                            return Err("Response blocked by Pure Mode. Try rephrasing your message.".to_string());
                        }
                        collected.extend_from_slice(&chunk);
                    }
                    Ok(Some(Err(e))) => {
                        log_error(
                            app,
                            "api_request",
                            format!("[api_request] stream error: {}", e),
                        );
                        use tauri::Manager;
                        let registry = app.state::<AbortRegistry>();
                        registry.unregister(&request_id);
                        return Err(e.to_string());
                    }
                    Ok(None) => {
                        break;
                    }
                    Err(_) => {
                        log_error(
                            app,
                            "api_request",
                            format!(
                                "[api_request] stream idle timeout after {}ms: {}",
                                idle_timeout_ms,
                                url_for_log
                            ),
                        );
                        use tauri::Manager;
                        let registry = app.state::<AbortRegistry>();
                        registry.unregister(&request_id);
                        return Err(format!(
                            "Streaming response timed out after {}ms of inactivity",
                            idle_timeout_ms
                        ));
                    }
                }
            }
        }
    }

    // Unregister the request
    {
        use tauri::Manager;
        let registry = app.state::<AbortRegistry>();
        registry.unregister(&request_id);
    }

    if aborted {
        let envelope = ErrorEnvelope {
            code: Some("ABORTED".to_string()),
            message: "Request was cancelled by user".to_string(),
            provider_id: req.provider_id.clone(),
            request_id: Some(request_id.clone()),
            retryable: Some(false),
            status: None,
        };
        emit_normalized(app, &request_id, NormalizedEvent::Error { envelope });
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Request aborted by user",
        ));
    }

    let text = String::from_utf8_lossy(&collected).to_string();
    log_info(
        app,
        "api_request",
        format!(
            "[api_request] stream completed, total bytes: {}",
            collected.len()
        ),
    );

    let value = parse_body_to_value(&text);

    if !text_emitted && ok {
        if let Some(content) = chat_request::extract_text(&value, req.provider_id.as_deref()) {
            if !content.is_empty() {
                enforce_pure_mode_on_text(app, req, Some(&request_id), &content)?;
                log_info(
                    app,
                    "api_request",
                    "[api_request] non-SSE response detected, emitting content as delta",
                );
                emit_normalized(app, &request_id, NormalizedEvent::Delta { text: content });
            }
        }
    }

    let provider_id = req.provider_id.as_deref().unwrap_or_default();
    let calls = parse_tool_calls(provider_id, &value);
    if !calls.is_empty() {
        emit_normalized(app, &request_id, NormalizedEvent::ToolCall { calls });
    }
    // Emit a final usage event if not already emitted and we can extract it
    if !usage_emitted {
        if let Some(usage) = chat_request::extract_usage(&value) {
            emit_normalized(app, &request_id, NormalizedEvent::Usage { usage });
        }
    }

    // If HTTP status was not OK, emit a normalized error event as well
    if !ok {
        let message = chat_request::extract_error_message(&value)
            .unwrap_or_else(|| format!("HTTP {}", status.as_u16()));
        let envelope = ErrorEnvelope {
            code: None,
            message,
            provider_id: req.provider_id.clone(),
            request_id: Some(request_id.clone()),
            retryable: None,
            status: Some(status.as_u16()),
        };
        emit_normalized(app, &request_id, NormalizedEvent::Error { envelope });
    }

    Ok(value)
}

pub(crate) async fn handle_non_streaming_response(
    app: &tauri::AppHandle,
    req: &ApiRequest,
    response: reqwest::Response,
    request_id: Option<String>,
    status: reqwest::StatusCode,
    ok: bool,
) -> Result<Value, String> {
    match response.bytes().await {
        Ok(bytes) => {
            log_info(
                app,
                "api_request",
                format!("[api_request] response body bytes: {}", bytes.len()),
            );
            let text = String::from_utf8_lossy(&bytes).to_string();
            log_info(
                app,
                "api_request",
                format!(
                    "[api_request] response body preview: {}",
                    crate::serde_utils::truncate_for_log(&text, 512)
                ),
            );
            let value = parse_body_to_value(&text);
            let filter_candidate = chat_request::extract_text(&value, req.provider_id.as_deref())
                .or_else(|| value.as_str().map(|s| s.to_string()));
            if ok {
                if let Some(candidate_text) = filter_candidate.as_deref() {
                    enforce_pure_mode_on_text(app, req, request_id.as_deref(), candidate_text)?;
                }
            }
            if let Some(req_id) = &request_id {
                let calls =
                    parse_tool_calls(req.provider_id.as_deref().unwrap_or_default(), &value);
                if !calls.is_empty() {
                    emit_normalized(app, req_id, NormalizedEvent::ToolCall { calls });
                }
            }
            if !ok {
                if let Some(req_id) = &request_id {
                    let message = chat_request::extract_error_message(&value)
                        .unwrap_or_else(|| format!("HTTP {}", status.as_u16()));
                    let envelope = ErrorEnvelope {
                        code: None,
                        message,
                        provider_id: req.provider_id.clone(),
                        request_id: Some(req_id.clone()),
                        retryable: None,
                        status: Some(status.as_u16()),
                    };
                    emit_normalized(app, req_id, NormalizedEvent::Error { envelope });
                }
            }
            Ok(value)
        }
        Err(e) => {
            log_error(
                app,
                "api_request",
                format!("[api_request] error reading response body: {}", e),
            );
            Err(e.to_string())
        }
    }
}
