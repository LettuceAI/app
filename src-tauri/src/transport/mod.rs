use serde_json::{json, Value};
use std::time::Duration;
use tauri::Emitter;
use tokio::time::sleep;

use crate::chat_manager::types::NormalizedEvent;
use crate::error::AppError;
use crate::utils::log_warn;

pub const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 30 * 60 * 1000;

pub fn build_client(timeout_ms: Option<u64>, stream: bool) -> Result<reqwest::Client, AppError> {
    let mut builder = reqwest::Client::builder();
    let normalized_timeout_ms = timeout_ms
        .unwrap_or(DEFAULT_REQUEST_TIMEOUT_MS)
        .min(DEFAULT_REQUEST_TIMEOUT_MS);
    if let Some(ms) = (!stream).then_some(normalized_timeout_ms) {
        let timeout = Duration::from_millis(ms);
        builder = builder.timeout(timeout);
    }
    builder.build().map_err(AppError::from)
}

pub fn emit_normalized(app: &tauri::AppHandle, request_id: &str, event: NormalizedEvent) {
    let channel = format!("api-normalized://{}", request_id);
    let payload = match &event {
        NormalizedEvent::Delta { text } => json!({
            "requestId": request_id,
            "type": "delta",
            "data": { "text": text },
        }),
        NormalizedEvent::Reasoning { text } => json!({
            "requestId": request_id,
            "type": "reasoning",
            "data": { "text": text },
        }),
        NormalizedEvent::Usage { usage } => json!({
            "requestId": request_id,
            "type": "usage",
            "data": usage,
        }),
        NormalizedEvent::Done => json!({
            "requestId": request_id,
            "type": "done",
            "data": Value::Null,
        }),
        NormalizedEvent::ToolCall { calls } => json!({
            "requestId": request_id,
            "type": "toolCall",
            "data": calls,
        }),
        NormalizedEvent::Error { envelope } => json!({
            "requestId": request_id,
            "type": "error",
            "data": envelope,
        }),
    };
    let _ = app.emit(&channel, payload);
}

#[allow(dead_code)]
pub fn emit_raw(app: &tauri::AppHandle, event_name: &str, chunk: &str) {
    let _ = app.emit(event_name, chunk.to_string());
}

#[allow(dead_code)]
pub async fn send_request(builder: reqwest::RequestBuilder) -> Result<reqwest::Response, AppError> {
    builder.send().await.map_err(AppError::from)
}

pub async fn send_with_retries(
    app: &tauri::AppHandle,
    scope: &str,
    builder: reqwest::RequestBuilder,
    max_retries: u32,
    request_id: Option<&str>,
) -> Result<reqwest::Response, AppError> {
    let base = match builder.try_clone() {
        Some(b) => b,
        None => return builder.send().await.map_err(AppError::from),
    };
    let mut attempt: u32 = 0;
    loop {
        let attempt_builder = base
            .try_clone()
            .expect("reqwest::RequestBuilder should be clonable for retries");
        let result = attempt_builder.send().await;
        match result {
            Ok(resp) => {
                let status = resp.status();
                let is_rate_limited = status.as_u16() == 429;
                let should_retry_status = status.is_server_error();
                let rate_limit_attempts: u32 = 3;
                let allowed_retries = if is_rate_limited {
                    rate_limit_attempts.saturating_sub(1)
                } else {
                    max_retries
                };

                if (is_rate_limited || should_retry_status) && attempt < allowed_retries {
                    attempt += 1;

                    // Honor Retry-After header for 429s when present; otherwise use exponential backoff.
                    let retry_after_ms = if is_rate_limited {
                        resp.headers()
                            .get("retry-after")
                            .and_then(|h| h.to_str().ok())
                            .and_then(|s| s.parse::<u64>().ok())
                            .map(|secs| secs * 1000)
                    } else {
                        None
                    };
                    let delay = retry_after_ms.unwrap_or_else(|| backoff_delay_ms(attempt));

                    log_warn(
                        app,
                        scope,
                        format!(
                            "{} {} - retrying in {}ms (attempt {}/{})",
                            status,
                            if is_rate_limited {
                                "rate limited"
                            } else {
                                "server error"
                            },
                            delay,
                            attempt,
                            allowed_retries
                        ),
                    );
                    if let Some(request_id) = request_id {
                        crate::utils::emit_warn_event(
                            app,
                            "transport_retry",
                            json!({
                                "requestId": request_id,
                                "scope": scope,
                                "attempt": attempt,
                                "maxRetries": allowed_retries,
                                "status": status.as_u16(),
                                "reason": if is_rate_limited { "rate_limited" } else { "server_error" },
                                "delayMs": delay,
                            }),
                        );
                    }
                    sleep(Duration::from_millis(delay)).await;
                } else {
                    return Ok(resp);
                }
            }
            Err(err) => {
                if (err.is_timeout() || err.is_request()) && attempt < max_retries {
                    attempt += 1;
                    let delay = backoff_delay_ms(attempt);
                    log_warn(
                        app,
                        scope,
                        format!(
                            "request error '{}' - retrying in {}ms (attempt {}/{})",
                            err, delay, attempt, max_retries
                        ),
                    );
                    if let Some(request_id) = request_id {
                        crate::utils::emit_warn_event(
                            app,
                            "transport_retry",
                            json!({
                                "requestId": request_id,
                                "scope": scope,
                                "attempt": attempt,
                                "maxRetries": max_retries,
                                "reason": if err.is_timeout() { "timeout" } else { "request_error" },
                                "error": err.to_string(),
                                "delayMs": delay,
                            }),
                        );
                    }
                    sleep(Duration::from_millis(delay)).await;
                } else {
                    return Err(AppError::from(err));
                }
            }
        }
    }
}

fn backoff_delay_ms(attempt: u32) -> u64 {
    // 200ms, 400ms, 800ms (cap at 1.6s)
    200u64 * (1u64 << (attempt.saturating_sub(1).min(3)))
}
