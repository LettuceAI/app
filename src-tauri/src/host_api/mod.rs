use std::convert::Infallible;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use futures_util::stream;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::Listener;
use tauri::Manager;
use tokio::sync::mpsc;
use tokio::sync::{oneshot, Mutex};
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;

use crate::api::{api_request, ApiRequest};
use crate::chat_manager::execution::{build_provider_extra_fields, RequestSettings};
use crate::chat_manager::persistence::storage::resolve_credential_for_model;
use crate::chat_manager::request::{extract_error_message, extract_text, extract_usage};
use crate::chat_manager::request_builder::build_chat_request;
use crate::chat_manager::types::{Model, Session, Settings, UsageSummary};
use crate::storage_manager::settings::internal_read_settings;
use crate::utils::{log_error, log_info, log_warn};

#[derive(Default)]
pub struct HostApiManager {
    runtime: Arc<Mutex<Option<HostApiRuntime>>>,
}

struct HostApiRuntime {
    bind_address: String,
    port: u16,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

#[derive(Clone)]
struct HostApiState {
    app: tauri::AppHandle,
    token: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostApiStatus {
    pub running: bool,
    pub bind_address: Option<String>,
    pub port: Option<u16>,
    pub base_url: Option<String>,
}

#[derive(Clone)]
struct ExposedModel {
    api_id: String,
    internal_model_id: String,
    label: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChatCompletionRequest {
    model: String,
    messages: Vec<Value>,
    #[serde(default)]
    stream: Option<bool>,
    #[serde(default)]
    stream_options: Option<OpenAIStreamOptions>,
    #[serde(default)]
    temperature: Option<f64>,
    #[serde(default)]
    top_p: Option<f64>,
    #[serde(default)]
    max_tokens: Option<u32>,
    #[serde(default)]
    max_completion_tokens: Option<u32>,
    #[serde(default)]
    frequency_penalty: Option<f64>,
    #[serde(default)]
    presence_penalty: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct OpenAIStreamOptions {
    #[serde(default)]
    include_usage: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NormalizedEnvelope {
    #[serde(rename = "requestId")]
    request_id: String,
    #[serde(rename = "type")]
    event_type: String,
    data: Value,
}

#[derive(Debug)]
struct HostApiError {
    status: StatusCode,
    message: String,
    error_type: &'static str,
}

impl HostApiError {
    fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
            error_type: "authentication_error",
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
            error_type: "invalid_request_error",
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
            error_type: "server_error",
        }
    }
}

impl IntoResponse for HostApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({
                "error": {
                    "message": self.message,
                    "type": self.error_type,
                }
            })),
        )
            .into_response()
    }
}

fn build_status(bind_address: Option<String>, port: Option<u16>, running: bool) -> HostApiStatus {
    let display_address = bind_address.as_deref().map(|addr| {
        if addr == "0.0.0.0" || addr == "::" {
            crate::utils::get_local_ip().unwrap_or_else(|_| addr.to_string())
        } else {
            addr.to_string()
        }
    });
    let base_url = match (display_address.as_deref(), port) {
        (Some(addr), Some(port)) => Some(format!("http://{}:{}", addr, port)),
        _ => None,
    };
    HostApiStatus {
        running,
        bind_address,
        port,
        base_url,
    }
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?.trim();
    let token = value.strip_prefix("Bearer ")?;
    Some(token.trim().to_string())
}

fn require_auth(headers: &HeaderMap, expected_token: &str) -> Result<(), HostApiError> {
    let Some(token) = extract_bearer_token(headers) else {
        return Err(HostApiError::unauthorized("Missing bearer token."));
    };
    if token != expected_token {
        return Err(HostApiError::unauthorized("Invalid bearer token."));
    }
    Ok(())
}

fn parse_settings(app: &tauri::AppHandle) -> Result<Settings, HostApiError> {
    let raw = internal_read_settings(app)
        .map_err(HostApiError::internal)?
        .ok_or_else(|| HostApiError::internal("Settings are not available."))?;
    serde_json::from_str::<Settings>(&raw)
        .map_err(|err| HostApiError::internal(format!("Failed to parse settings: {}", err)))
}

fn load_exposed_models(settings: &Settings) -> Vec<ExposedModel> {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.host_api.as_ref())
        .map(|host_api| {
            host_api
                .exposed_models
                .iter()
                .filter(|item| item.enabled)
                .map(|item| ExposedModel {
                    api_id: item.id.clone(),
                    internal_model_id: item.model_id.clone(),
                    label: item.label.clone(),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn resolve_exposed_model<'a>(
    settings: &'a Settings,
    exposed_id: &str,
) -> Result<(&'a Model, ExposedModel), HostApiError> {
    let exposed = load_exposed_models(settings)
        .into_iter()
        .find(|item| item.api_id == exposed_id)
        .ok_or_else(|| {
            HostApiError::not_found(format!(
                "Model '{}' is not exposed by this host.",
                exposed_id
            ))
        })?;

    let model = settings
        .models
        .iter()
        .find(|item| item.id == exposed.internal_model_id)
        .ok_or_else(|| {
            HostApiError::not_found(format!(
                "Host model '{}' is no longer available.",
                exposed.api_id
            ))
        })?;

    Ok((model, exposed))
}

fn build_gateway_session() -> Session {
    Session {
        id: "host-api".to_string(),
        character_id: "host-api".to_string(),
        title: "Host API".to_string(),
        background_image_path: None,
        system_prompt: None,
        selected_scene_id: None,
        prompt_template_id: None,
        lorebook_ids_override: None,
        author_note: None,
        persona_id: None,
        persona_disabled: false,
        voice_autoplay: None,
        advanced_model_settings: None,
        memories: Vec::new(),
        memory_embeddings: Vec::new(),
        memory_summary: None,
        memory_summary_token_count: 0,
        memory_tool_events: Vec::new(),
        memory_status: None,
        memory_error: None,
        memory_progress_step: None,
        messages: Vec::new(),
        archived: false,
        created_at: 0,
        updated_at: 0,
    }
}

async fn health_handler(
    State(_state): State<HostApiState>,
    _headers: HeaderMap,
) -> Result<Json<Value>, HostApiError> {
    Ok(Json(json!({
        "object": "status",
        "status": "ok",
        "service": "lettuce-host",
    })))
}

async fn root_handler(State(_state): State<HostApiState>) -> Json<Value> {
    Json(json!({
        "service": "lettuce-host",
        "status": "ok",
        "auth": {
            "type": "bearer",
            "requiredFor": ["/v1/models", "/v1/chat/completions"]
        },
        "endpoints": ["/health", "/v1/models", "/v1/chat/completions"]
    }))
}

async fn list_models_handler(
    State(state): State<HostApiState>,
    headers: HeaderMap,
) -> Result<Json<Value>, HostApiError> {
    require_auth(&headers, &state.token)?;
    let settings = parse_settings(&state.app)?;
    let data = load_exposed_models(&settings)
        .into_iter()
        .filter_map(|item| {
            let model = settings
                .models
                .iter()
                .find(|model| model.id == item.internal_model_id)?;
            Some(json!({
                "id": item.api_id,
                "object": "model",
                "created": (model.created_at / 1000) as i64,
                "owned_by": "lettuce-host",
                "name": item.label.unwrap_or_else(|| model.display_name.clone()),
            }))
        })
        .collect::<Vec<_>>();

    Ok(Json(json!({
        "object": "list",
        "data": data,
    })))
}

async fn chat_completions_handler(
    State(state): State<HostApiState>,
    headers: HeaderMap,
    Json(payload): Json<OpenAIChatCompletionRequest>,
) -> Result<Response, HostApiError> {
    require_auth(&headers, &state.token)?;

    if payload.stream.unwrap_or(false) {
        return execute_streaming_chat_completion(state, payload).await;
    }

    let settings = parse_settings(&state.app)?;
    let (model, exposed_model) = resolve_exposed_model(&settings, &payload.model)?;
    let credential = resolve_credential_for_model(&settings, model)
        .ok_or_else(|| HostApiError::not_found("Provider credential for model is unavailable."))?;
    let session = build_gateway_session();
    let mut request_settings = RequestSettings::resolve(&session, model, &settings);

    if let Some(value) = payload.temperature {
        request_settings.temperature = Some(value);
    }
    if let Some(value) = payload.top_p {
        request_settings.top_p = Some(value);
    }
    if let Some(value) = payload.max_completion_tokens.or(payload.max_tokens) {
        request_settings.max_tokens = value;
    }
    if let Some(value) = payload.frequency_penalty {
        request_settings.frequency_penalty = Some(value);
    }
    if let Some(value) = payload.presence_penalty {
        request_settings.presence_penalty = Some(value);
    }

    let extra_body_fields = build_provider_extra_fields(
        &model.provider_id,
        &session,
        model,
        &settings,
        &request_settings,
    );

    let built_request = build_chat_request(
        credential,
        credential.api_key.as_deref().unwrap_or(""),
        &model.name,
        &payload.messages,
        None,
        request_settings.temperature,
        request_settings.top_p,
        request_settings.max_tokens,
        request_settings.context_length,
        false,
        None,
        request_settings.frequency_penalty,
        request_settings.presence_penalty,
        request_settings.top_k,
        None,
        request_settings.reasoning_enabled,
        request_settings.reasoning_effort.clone(),
        request_settings.reasoning_budget,
        request_settings.prompt_caching_enabled.unwrap_or(false),
        extra_body_fields,
    );

    let response = api_request(
        state.app.clone(),
        ApiRequest {
            url: built_request.url,
            method: Some("POST".to_string()),
            headers: Some(built_request.headers),
            query: None,
            body: Some(built_request.body),
            timeout_ms: None,
            stream: Some(false),
            request_id: None,
            provider_id: Some(model.provider_id.clone()),
        },
    )
    .await
    .map_err(HostApiError::internal)?;

    if !response.ok {
        let message = extract_error_message(response.data())
            .unwrap_or_else(|| format!("Upstream provider returned status {}.", response.status));
        return Err(HostApiError {
            status: StatusCode::from_u16(response.status).unwrap_or(StatusCode::BAD_GATEWAY),
            message,
            error_type: "invalid_request_error",
        });
    }

    let content = extract_text(response.data(), Some(&model.provider_id)).unwrap_or_default();
    let usage = extract_usage(response.data());

    let response_json = json!({
        "id": format!("chatcmpl-{}", Uuid::new_v4()),
        "object": "chat.completion",
        "created": Utc::now().timestamp(),
        "model": exposed_model.api_id,
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": content,
            },
            "finish_reason": "stop",
        }],
        "usage": usage.as_ref().map(build_usage_json),
    });

    Ok(Json(response_json).into_response())
}

fn build_stream_chunk(
    stream_id: &str,
    model_id: &str,
    created: i64,
    content: Option<&str>,
    finish_reason: Option<&str>,
) -> Value {
    let mut delta = serde_json::Map::new();
    if let Some(content) = content {
        delta.insert("content".to_string(), Value::String(content.to_string()));
    }

    json!({
        "id": stream_id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model_id,
        "choices": [{
            "index": 0,
            "delta": Value::Object(delta),
            "finish_reason": finish_reason,
        }],
    })
}

fn build_usage_json(usage: &UsageSummary) -> Value {
    let mut value = serde_json::Map::new();
    value.insert(
        "prompt_tokens".to_string(),
        Value::from(usage.prompt_tokens.unwrap_or(0)),
    );
    value.insert(
        "completion_tokens".to_string(),
        Value::from(usage.completion_tokens.unwrap_or(0)),
    );
    value.insert(
        "total_tokens".to_string(),
        Value::from(usage.total_tokens.unwrap_or(0)),
    );
    if let Some(tokens) = usage.reasoning_tokens {
        value.insert("reasoning_tokens".to_string(), Value::from(tokens));
    }
    if let Some(tokens) = usage.image_tokens {
        value.insert("image_tokens".to_string(), Value::from(tokens));
    }
    if let Some(tokens) = usage.cached_prompt_tokens {
        value.insert("cached_prompt_tokens".to_string(), Value::from(tokens));
    }
    if let Some(tokens) = usage.cache_write_tokens {
        value.insert("cache_write_tokens".to_string(), Value::from(tokens));
    }
    if let Some(count) = usage.web_search_requests {
        value.insert("web_search_requests".to_string(), Value::from(count));
    }
    if let Some(cost) = usage.api_cost {
        value.insert("cost".to_string(), json!(cost));
    }
    Value::Object(value)
}

fn build_stream_usage_chunk(
    stream_id: &str,
    model_id: &str,
    created: i64,
    usage: &UsageSummary,
) -> Value {
    json!({
        "id": stream_id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model_id,
        "choices": [],
        "usage": build_usage_json(usage),
    })
}

fn emit_stream_success(
    tx: &mpsc::UnboundedSender<Result<Event, Infallible>>,
    stream_id: &str,
    model_id: &str,
    created: i64,
    usage: Option<&UsageSummary>,
    include_usage: bool,
) {
    let final_chunk = build_stream_chunk(stream_id, model_id, created, None, Some("stop"));
    let _ = tx.send(Ok(Event::default().data(final_chunk.to_string())));
    if include_usage {
        if let Some(usage) = usage {
            let usage_chunk = build_stream_usage_chunk(stream_id, model_id, created, usage);
            let _ = tx.send(Ok(Event::default().data(usage_chunk.to_string())));
        }
    }
    let _ = tx.send(Ok(Event::default().data("[DONE]")));
}

async fn execute_streaming_chat_completion(
    state: HostApiState,
    payload: OpenAIChatCompletionRequest,
) -> Result<Response, HostApiError> {
    let settings = parse_settings(&state.app)?;
    let (model, exposed_model) = resolve_exposed_model(&settings, &payload.model)?;
    let credential = resolve_credential_for_model(&settings, model)
        .ok_or_else(|| HostApiError::not_found("Provider credential for model is unavailable."))?;
    let session = build_gateway_session();
    let mut request_settings = RequestSettings::resolve(&session, model, &settings);

    if let Some(value) = payload.temperature {
        request_settings.temperature = Some(value);
    }
    if let Some(value) = payload.top_p {
        request_settings.top_p = Some(value);
    }
    if let Some(value) = payload.max_completion_tokens.or(payload.max_tokens) {
        request_settings.max_tokens = value;
    }
    if let Some(value) = payload.frequency_penalty {
        request_settings.frequency_penalty = Some(value);
    }
    if let Some(value) = payload.presence_penalty {
        request_settings.presence_penalty = Some(value);
    }

    let extra_body_fields = build_provider_extra_fields(
        &model.provider_id,
        &session,
        model,
        &settings,
        &request_settings,
    );

    let built_request = build_chat_request(
        credential,
        credential.api_key.as_deref().unwrap_or(""),
        &model.name,
        &payload.messages,
        None,
        request_settings.temperature,
        request_settings.top_p,
        request_settings.max_tokens,
        request_settings.context_length,
        true,
        None,
        request_settings.frequency_penalty,
        request_settings.presence_penalty,
        request_settings.top_k,
        None,
        request_settings.reasoning_enabled,
        request_settings.reasoning_effort.clone(),
        request_settings.reasoning_budget,
        request_settings.prompt_caching_enabled.unwrap_or(false),
        extra_body_fields,
    );

    let request_id = Uuid::new_v4().to_string();
    let event_name = format!("api-normalized://{}", request_id);
    let stream_id = format!("chatcmpl-{}", Uuid::new_v4());
    let include_usage = payload
        .stream_options
        .as_ref()
        .and_then(|options| options.include_usage)
        .unwrap_or(false);
    let response_model_id = exposed_model.api_id.clone();
    let created = Utc::now().timestamp();
    let (tx, rx) = mpsc::unbounded_channel::<Result<Event, Infallible>>();
    let listener_tx = tx.clone();
    let error_emitted = Arc::new(AtomicBool::new(false));
    let error_flag = error_emitted.clone();
    let final_emitted = Arc::new(AtomicBool::new(false));
    let final_flag = final_emitted.clone();
    let usage_summary = Arc::new(std::sync::Mutex::new(None::<UsageSummary>));
    let listener_usage_summary = usage_summary.clone();
    let app = state.app.clone();
    let listener_stream_id = stream_id.clone();
    let listener_model_id = response_model_id.clone();

    let listener_id = app.listen_any(event_name.clone(), move |event| {
        let payload_str = event.payload();
        let Ok(envelope) = serde_json::from_str::<NormalizedEnvelope>(payload_str) else {
            return;
        };
        let _request_id_seen = envelope.request_id;
        match envelope.event_type.as_str() {
            "delta" => {
                if let Some(text) = envelope.data.get("text").and_then(Value::as_str) {
                    let chunk = build_stream_chunk(
                        &listener_stream_id,
                        &response_model_id,
                        created,
                        Some(text),
                        None,
                    );
                    let _ = listener_tx.send(Ok(Event::default().data(chunk.to_string())));
                }
            }
            "usage" => {
                if let Ok(usage) = serde_json::from_value::<UsageSummary>(envelope.data) {
                    if let Ok(mut slot) = listener_usage_summary.lock() {
                        *slot = Some(usage);
                    }
                }
            }
            "done" => {
                if !final_flag.swap(true, Ordering::SeqCst) {
                    let usage = listener_usage_summary
                        .lock()
                        .ok()
                        .and_then(|slot| slot.clone());
                    emit_stream_success(
                        &listener_tx,
                        &listener_stream_id,
                        &listener_model_id,
                        created,
                        usage.as_ref(),
                        include_usage,
                    );
                }
            }
            "error" => {
                error_flag.store(true, Ordering::SeqCst);
                if !final_flag.swap(true, Ordering::SeqCst) {
                    let message = envelope
                        .data
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("Streaming request failed.");
                    let _ = listener_tx.send(Ok(Event::default().data(
                        json!({
                            "error": {
                                "message": message,
                                "type": "server_error",
                            }
                        })
                        .to_string(),
                    )));
                    let _ = listener_tx.send(Ok(Event::default().data("[DONE]")));
                }
            }
            _ => {}
        }
    });

    let app_for_task = state.app.clone();
    let model_provider_id = model.provider_id.clone();
    let task_model_name = payload.model.clone();
    let task_stream_id = stream_id.clone();
    let task_final_emitted = final_emitted.clone();
    let task_tx = tx.clone();
    let task_usage_summary = usage_summary.clone();
    tauri::async_runtime::spawn(async move {
        let result = api_request(
            app_for_task.clone(),
            ApiRequest {
                url: built_request.url,
                method: Some("POST".to_string()),
                headers: Some(built_request.headers),
                query: None,
                body: Some(built_request.body),
                timeout_ms: None,
                stream: Some(true),
                request_id: Some(request_id.clone()),
                provider_id: Some(model_provider_id),
            },
        )
        .await;

        app_for_task.unlisten(listener_id);

        match result {
            Ok(response) => {
                if let Some(usage) = extract_usage(response.data()) {
                    if let Ok(mut slot) = task_usage_summary.lock() {
                        *slot = Some(usage);
                    }
                }
                if !error_emitted.load(Ordering::SeqCst)
                    && !task_final_emitted.swap(true, Ordering::SeqCst)
                {
                    let usage = task_usage_summary.lock().ok().and_then(|slot| slot.clone());
                    emit_stream_success(
                        &task_tx,
                        &task_stream_id,
                        &task_model_name,
                        created,
                        usage.as_ref(),
                        include_usage,
                    );
                }
            }
            Err(error) => {
                if !error_emitted.swap(true, Ordering::SeqCst)
                    && !task_final_emitted.swap(true, Ordering::SeqCst)
                {
                    let _ = task_tx.send(Ok(Event::default().data(
                        json!({
                            "error": {
                                "message": error,
                                "type": "server_error",
                            }
                        })
                        .to_string(),
                    )));
                    let _ = task_tx.send(Ok(Event::default().data("[DONE]")));
                }
            }
        }
    });

    let event_stream = stream::unfold(rx, |mut rx| async {
        rx.recv().await.map(|event| (event, rx))
    });

    Ok(Sse::new(event_stream)
        .keep_alive(KeepAlive::default())
        .into_response())
}

fn build_router(state: HostApiState) -> Router {
    Router::new()
        .route("/", get(root_handler))
        .route("/health", get(health_handler))
        .route("/v1/models", get(list_models_handler))
        .route("/v1/chat/completions", post(chat_completions_handler))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_headers(Any)
                .allow_methods(Any),
        )
        .with_state(state)
}

async fn stop_runtime(manager: &HostApiManager) {
    let mut guard = manager.runtime.lock().await;
    if let Some(mut runtime) = guard.take() {
        if let Some(tx) = runtime.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
    drop(guard);
    // Give axum time to release the socket after graceful shutdown signal
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
}

async fn start_runtime(app: tauri::AppHandle) -> Result<HostApiStatus, String> {
    let manager = app.state::<HostApiManager>();
    let settings = parse_settings(&app).map_err(|err| err.message)?;
    let host_api = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.host_api.as_ref())
        .ok_or_else(|| "Host API settings are not configured.".to_string())?;

    if !host_api.enabled {
        return Err("Host API is disabled in settings.".to_string());
    }
    if host_api.token.trim().is_empty() {
        return Err("Host API token is empty.".to_string());
    }

    let bind_ip: IpAddr = host_api
        .bind_address
        .parse()
        .map_err(|err| format!("Invalid bind address: {}", err))?;
    let socket = SocketAddr::new(bind_ip, host_api.port);

    stop_runtime(&manager).await;

    let listener = tokio::net::TcpListener::bind(socket)
        .await
        .map_err(|err| format!("Failed to bind host API server: {}", err))?;
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let state = HostApiState {
        app: app.clone(),
        token: host_api.token.clone(),
    };
    let router = build_router(state);
    let server_bind_address = host_api.bind_address.clone();
    let server_port = host_api.port;
    let app_for_task = app.clone();

    tauri::async_runtime::spawn(async move {
        let serve_result = axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await;

        if let Err(err) = serve_result {
            log_error(
                &app_for_task,
                "host_api",
                format!("Host API server exited with error: {}", err),
            );
        } else {
            log_info(&app_for_task, "host_api", "Host API server stopped");
        }
    });

    {
        let mut guard = manager.runtime.lock().await;
        *guard = Some(HostApiRuntime {
            bind_address: server_bind_address.clone(),
            port: server_port,
            shutdown_tx: Some(shutdown_tx),
        });
    }

    log_info(
        &app,
        "host_api",
        format!(
            "Host API server listening on http://{}:{}",
            server_bind_address, server_port
        ),
    );

    Ok(build_status(
        Some(server_bind_address),
        Some(server_port),
        true,
    ))
}

pub async fn maybe_start_from_settings(app: &tauri::AppHandle) {
    let settings = match parse_settings(app) {
        Ok(settings) => settings,
        Err(err) => {
            log_warn(
                app,
                "host_api",
                format!("Skipping host API startup: {}", err.message),
            );
            return;
        }
    };

    let should_start = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.host_api.as_ref())
        .map(|host_api| host_api.enabled && !host_api.token.trim().is_empty())
        .unwrap_or(false);

    if !should_start {
        return;
    }

    if let Err(err) = start_runtime(app.clone()).await {
        log_error(
            app,
            "host_api",
            format!("Failed to start host API during bootstrap: {}", err),
        );
    }
}

#[tauri::command]
pub async fn host_api_get_status(app: tauri::AppHandle) -> Result<HostApiStatus, String> {
    let manager = app.state::<HostApiManager>();
    let guard = manager.runtime.lock().await;
    if let Some(runtime) = guard.as_ref() {
        return Ok(build_status(
            Some(runtime.bind_address.clone()),
            Some(runtime.port),
            true,
        ));
    }

    let settings = parse_settings(&app).map_err(|err| err.message)?;
    let configured = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.host_api.as_ref())
        .map(|host_api| (host_api.bind_address.clone(), host_api.port))
        .unwrap_or_else(|| ("0.0.0.0".to_string(), 3333));

    Ok(build_status(Some(configured.0), Some(configured.1), false))
}

#[tauri::command]
pub async fn host_api_start(app: tauri::AppHandle) -> Result<HostApiStatus, String> {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = app;
        return Err("Host API is only available on desktop.".to_string());
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        start_runtime(app).await
    }
}

#[tauri::command]
pub async fn host_api_stop(app: tauri::AppHandle) -> Result<HostApiStatus, String> {
    let manager = app.state::<HostApiManager>();
    stop_runtime(&manager).await;
    Ok(build_status(None, None, false))
}
