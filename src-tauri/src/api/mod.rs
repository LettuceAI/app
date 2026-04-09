use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::abort_manager::AbortRegistry;
use crate::chat_manager::types::{ErrorEnvelope, NormalizedEvent};
use crate::llama_cpp;
use crate::serde_utils::truncate_for_log;
use crate::transport;
use crate::transport::emit_normalized;
use crate::utils::{log_error, log_info};

mod helpers;
use helpers::{
    apply_body, apply_headers, apply_query_params, handle_non_streaming_response,
    handle_streaming_response,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiRequest {
    pub url: String,
    pub method: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub query: Option<HashMap<String, Value>>,
    pub body: Option<Value>,
    pub timeout_ms: Option<u64>,
    pub stream: Option<bool>,
    pub request_id: Option<String>,
    pub provider_id: Option<String>,
}

#[derive(Serialize)]
pub struct ApiResponse {
    pub status: u16,
    pub ok: bool,
    pub headers: HashMap<String, String>,
    pub data: Value,
}

impl ApiResponse {
    pub fn data(&self) -> &Value {
        &self.data
    }
}

#[tauri::command]
pub async fn api_request(app: tauri::AppHandle, req: ApiRequest) -> Result<ApiResponse, String> {
    log_info(&app, "api_request", "started");

    if llama_cpp::is_llama_cpp(req.provider_id.as_deref()) {
        return llama_cpp::handle_local_request(app, req).await;
    }
    if crate::ollama::is_ollama_provider(req.provider_id.as_deref()) {
        return crate::ollama::execute_chat_request(&app, &req).await;
    }

    if req.provider_id.as_deref() == Some("openrouter") {
        if let Some(api_key) = req
            .headers
            .as_ref()
            .and_then(|headers| headers.get("Authorization"))
            .and_then(|value| value.strip_prefix("Bearer "))
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            crate::pricing_cache::kick_openrouter_deferred_refreshes(
                app.clone(),
                api_key.to_string(),
            );
        }
    }

    let method_str = req.method.clone().unwrap_or_else(|| "POST".to_string());
    let url_for_log = req.url.clone();
    let method = match Method::from_bytes(method_str.as_bytes()) {
        Ok(m) => m,
        Err(e) => {
            log_error(
                &app,
                "api_request",
                format!("[api_request] invalid method: {}", method_str),
            );
            return Err(e.to_string());
        }
    };

    let header_preview = req.headers.as_ref().map(|headers| {
        headers
            .iter()
            .map(|(key, value)| format!("{}={}", key, truncate_for_log(value, 64)))
            .collect::<Vec<_>>()
    });
    let query_keys = req
        .query
        .as_ref()
        .map(|query| query.keys().cloned().collect::<Vec<String>>());
    let body_preview = req.body.as_ref().map(crate::serde_utils::summarize_json);

    let stream = req.stream.unwrap_or(false);
    let request_id = req.request_id.clone();

    let client = match transport::build_client(req.timeout_ms, stream) {
        Ok(c) => c,
        Err(e) => {
            log_error(&app, "api_request", format!("client build error: {}", e));
            return Err(e.to_string());
        }
    };

    let mut request_builder = client.request(method.clone(), &req.url);

    log_info(
        &app,
        "api_request",
        format!("[api_request] method={} url={}", method_str, url_for_log),
    );

    request_builder = apply_query_params(&app, request_builder, &req);
    request_builder = apply_headers(&app, request_builder, &req);
    request_builder = apply_body(&app, request_builder, &req);

    log_info(
        &app,
        "api_request",
        format!(
            "[api_request] method={} full_url={} stream={} request_id={:?} timeout_ms={:?}",
            method_str, url_for_log, stream, request_id, req.timeout_ms
        ),
    );

    if let Some(headers) = &header_preview {
        if !headers.is_empty() {
            log_info(
                &app,
                "api_request",
                format!("[api_request] headers: {}", headers.join(", ")),
            );
        }
    } else {
        log_info(&app, "api_request", "[api_request] headers: <default>");
    }

    if let Some(keys) = &query_keys {
        if !keys.is_empty() {
            log_info(
                &app,
                "api_request",
                format!("[api_request] query params: {:?}", keys),
            );
        }
    }

    if let Some(body) = &body_preview {
        log_info(
            &app,
            "api_request",
            format!("[api_request] body preview: {}", body),
        );
    }

    let mut abort_rx = if !stream {
        request_id.as_ref().map(|req_id| {
            use tauri::Manager;
            let registry = app.state::<AbortRegistry>();
            registry.register(req_id.clone())
        })
    } else {
        None
    };

    let emit_abort = || {
        if let Some(req_id) = request_id.as_ref() {
            let envelope = ErrorEnvelope {
                code: Some("ABORTED".to_string()),
                message: "Request was cancelled by user".to_string(),
                provider_id: req.provider_id.clone(),
                request_id: Some(req_id.clone()),
                retryable: Some(false),
                status: None,
            };
            emit_normalized(&app, req_id, NormalizedEvent::Error { envelope });
        }
    };

    log_info(&app, "api_request", "[api_request] sending request...");
    let response = if let Some(abort_rx) = abort_rx.as_mut() {
        tokio::select! {
            _ = abort_rx => {
                if let Some(req_id) = request_id.as_ref() {
                    use tauri::Manager;
                    let registry = app.state::<AbortRegistry>();
                    registry.unregister(req_id);
                }
                emit_abort();
                return Err("Request was cancelled by user".to_string());
            }
            response = transport::send_with_retries(
                &app,
                "api_request",
                request_builder,
                2,
                request_id.as_deref(),
            ) => {
                match response {
                    Ok(resp) => {
                        log_info(
                            &app,
                            "api_request",
                            "[api_request] request sent successfully",
                        );
                        resp
                    }
                    Err(err) => {
                        if let Some(req_id) = request_id.as_ref() {
                            use tauri::Manager;
                            let registry = app.state::<AbortRegistry>();
                            registry.unregister(req_id);
                        }
                        log_info(
                            &app,
                            "api_request",
                            format!("[api_request] request error for {}: {}", url_for_log, err),
                        );
                        return Err(err.to_string());
                    }
                }
            }
        }
    } else {
        match transport::send_with_retries(
            &app,
            "api_request",
            request_builder,
            2,
            request_id.as_deref(),
        )
        .await
        {
            Ok(resp) => {
                log_info(
                    &app,
                    "api_request",
                    "[api_request] request sent successfully",
                );
                resp
            }
            Err(err) => {
                log_info(
                    &app,
                    "api_request",
                    format!("[api_request] request error for {}: {}", url_for_log, err),
                );
                return Err(err.to_string());
            }
        }
    };
    let status = response.status();
    let ok = status.is_success();

    log_info(
        &app,
        "api_request",
        format!("[api_request] response status: {} ok: {}", status, ok),
    );

    let mut headers = HashMap::new();

    for (key, value) in response.headers().iter() {
        if let Ok(text) = value.to_str() {
            log_info(
                &app,
                "api_request",
                format!(
                    "[api_request] response header: {}={}",
                    key,
                    truncate_for_log(text, 512)
                ),
            );
            headers.insert(key.to_string(), text.to_string());
        }
    }

    let data = if stream && request_id.is_some() {
        handle_streaming_response(
            &app,
            &req,
            response,
            request_id.clone().unwrap(),
            status,
            ok,
            &url_for_log,
        )
        .await?
    } else {
        let result = if let Some(abort_rx) = abort_rx.as_mut() {
            tokio::select! {
                _ = abort_rx => {
                    if let Some(req_id) = request_id.as_ref() {
                        use tauri::Manager;
                        let registry = app.state::<AbortRegistry>();
                        registry.unregister(req_id);
                    }
                    emit_abort();
                    return Err("Request was cancelled by user".to_string());
                }
                result = handle_non_streaming_response(&app, &req, response, request_id.clone(), status, ok) => result,
            }
        } else {
            handle_non_streaming_response(&app, &req, response, request_id.clone(), status, ok)
                .await
        };

        if let Some(req_id) = request_id.as_ref() {
            use tauri::Manager;
            let registry = app.state::<AbortRegistry>();
            registry.unregister(req_id);
        }

        result?
    };

    log_info(
        &app,
        "api_request",
        format!(
            "[api_request] completed {} {} status={} ok={} stream={} request_id={:?}",
            method_str, url_for_log, status, ok, stream, request_id
        ),
    );

    Ok(ApiResponse {
        status: status.as_u16(),
        ok,
        headers,
        data,
    })
}

#[tauri::command]
pub async fn abort_request(app: tauri::AppHandle, request_id: String) -> Result<(), String> {
    use tauri::Manager;

    log_info(
        &app,
        "abort_request",
        format!(
            "[abort_request] attempting to abort request_id={}",
            request_id
        ),
    );

    let registry = app.state::<AbortRegistry>();
    registry.abort(&request_id)?;

    log_info(
        &app,
        "abort_request",
        format!(
            "[abort_request] successfully aborted request_id={}",
            request_id
        ),
    );

    Ok(())
}
