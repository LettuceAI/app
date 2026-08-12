use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::api::{api_request, ApiRequest};
use crate::chat_manager::execution::{build_provider_extra_fields, RequestSettings};
use crate::chat_manager::request::{
    extract_error_message, extract_gemini_content, extract_reasoning, extract_text, extract_usage,
};
use crate::chat_manager::request_builder::build_chat_request;
use crate::chat_manager::service::require_api_key;
use crate::chat_manager::take_aborted_request;
use crate::chat_manager::tooling::ToolConfig;
use crate::chat_manager::types::{Model, ProviderCredential, Session, Settings, UsageSummary};
use crate::utils::{emit_error_event, emit_info, log_error, log_warn, now_millis};

pub(crate) struct ConversationExecutionInput<'a> {
    pub app: &'a AppHandle,
    pub session_id: &'a str,
    pub request_session: &'a Session,
    pub settings: &'a Settings,
    pub model: &'a Model,
    pub credential: &'a ProviderCredential,
    pub messages: &'a Vec<Value>,
    pub stream: bool,
    pub request_id: Option<String>,
    pub operation: &'a str,
    pub log_scope: &'a str,
    pub tool_config: Option<&'a ToolConfig>,
}

#[derive(Debug)]
pub(crate) struct ConversationExecutionOutput {
    pub text: String,
    pub reasoning: Option<String>,
    pub gemini_content: Option<Value>,
    pub usage: Option<UsageSummary>,
    pub generated_image_data_urls: Vec<String>,
    pub api_key: String,
}

#[derive(Debug)]
pub(crate) struct ConversationExecutionFailure {
    pub message: String,
    pub usage: Option<UsageSummary>,
}

impl std::fmt::Display for ConversationExecutionFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ConversationExecutionFailure {}

impl ConversationExecutionFailure {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            usage: None,
        }
    }
}

pub(crate) async fn execute_generation(
    input: ConversationExecutionInput<'_>,
) -> Result<ConversationExecutionOutput, ConversationExecutionFailure> {
    let api_key = require_api_key(input.app, input.credential, input.log_scope)
        .map_err(ConversationExecutionFailure::new)?;
    let request_settings =
        RequestSettings::resolve(input.request_session, input.model, input.settings);
    let extra_body_fields = build_provider_extra_fields(
        &input.credential.provider_id,
        input.request_session,
        input.model,
        input.settings,
        &request_settings,
    );
    let built = build_chat_request(
        input.credential,
        &api_key,
        &input.model.name,
        input.messages,
        None,
        request_settings.temperature,
        request_settings.top_p,
        request_settings.max_tokens,
        request_settings.context_length,
        input.stream,
        input.request_id.clone(),
        request_settings.frequency_penalty,
        request_settings.presence_penalty,
        request_settings.top_k,
        input.tool_config,
        request_settings.reasoning_enabled,
        request_settings.reasoning_effort.clone(),
        request_settings.reasoning_budget,
        request_settings.prompt_caching_enabled.unwrap_or(false),
        extra_body_fields,
    );

    let request_started_at = now_millis().unwrap_or_default();
    emit_info(
        input.app,
        "sending_request",
        json!({
            "operation": input.operation,
            "sessionId": input.session_id,
            "providerId": input.credential.provider_id,
            "model": input.model.name,
            "stream": input.stream,
            "requestId": input.request_id,
            "endpoint": built.url,
            "requestStartedAt": request_started_at,
            "requestBody": &built.body,
            "requestSettings": {
                "temperature": request_settings.temperature,
                "topP": request_settings.top_p,
                "maxTokens": request_settings.max_tokens,
                "contextLength": request_settings.context_length,
                "frequencyPenalty": request_settings.frequency_penalty,
                "presencePenalty": request_settings.presence_penalty,
                "topK": request_settings.top_k,
                "reasoningEnabled": request_settings.reasoning_enabled,
                "reasoningEffort": request_settings.reasoning_effort,
                "reasoningBudget": request_settings.reasoning_budget,
            },
        }),
    );

    let response = api_request(
        input.app.clone(),
        ApiRequest {
            url: built.url,
            method: Some("POST".into()),
            headers: Some(built.headers),
            query: None,
            body: Some(built.body),
            timeout_ms: Some(crate::transport::DEFAULT_REQUEST_TIMEOUT_MS),
            stream: Some(built.stream),
            request_id: built.request_id,
            provider_id: Some(input.credential.provider_id.clone()),
            cache_key: Some(input.session_id.to_string()),
        },
    )
    .await
    .map_err(|message| {
        log_error(input.app, input.log_scope, &message);
        ConversationExecutionFailure::new(message)
    })?;

    emit_info(
        input.app,
        "response",
        json!({
            "operation": input.operation,
            "sessionId": input.session_id,
            "requestId": input.request_id,
            "status": response.status,
            "ok": response.ok,
            "model": input.model.name,
            "elapsedMs": now_millis().unwrap_or_default().saturating_sub(request_started_at),
        }),
    );

    if !response.ok {
        let fallback = format!("Provider returned status {}", response.status);
        let message = extract_error_message(response.data()).unwrap_or(fallback.clone());
        let usage = extract_usage(response.data());
        emit_error_event(
            input.app,
            "provider_error",
            json!({
                "operation": input.operation,
                "sessionId": input.session_id,
                "requestId": input.request_id,
                "status": response.status,
                "message": message,
                "usage": usage,
                "model": input.model.name,
            }),
        );
        return Err(ConversationExecutionFailure {
            message: if message == fallback {
                message
            } else {
                format!("{} (status {})", message, response.status)
            },
            usage,
        });
    }

    if take_aborted_request(input.app, input.request_id.as_deref()) {
        return Err(ConversationExecutionFailure::new("Request aborted by user"));
    }

    let generated_image_data_urls = match response.data() {
        Value::String(value) if value.contains("data:") => {
            crate::chat_manager::sse::accumulate_image_data_urls_from_sse(value)
        }
        value => crate::chat_manager::sse::image_data_urls_from_response(value),
    };
    let text =
        extract_text(response.data(), Some(&input.credential.provider_id)).unwrap_or_default();
    let reasoning = extract_reasoning(response.data(), Some(&input.credential.provider_id));
    let usage = extract_usage(response.data());
    let gemini_content = crate::chat_manager::provider_adapter::is_gemini_format_provider(
        &input.credential.provider_id,
    )
    .then(|| extract_gemini_content(response.data()))
    .flatten();

    if text.trim().is_empty() && generated_image_data_urls.is_empty() {
        let message = if reasoning
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty())
        {
            "Model completed reasoning but generated no response text. This may indicate the model ran out of tokens or encountered an issue during generation."
        } else {
            "Empty response from provider"
        };
        return Err(ConversationExecutionFailure::new(message));
    }

    if let Some(filter) = input
        .app
        .try_state::<crate::content_filter::ContentFilter>()
    {
        if filter.is_enabled() {
            let result = filter.check_text(&text);
            if result.blocked {
                log_warn(
                    input.app,
                    input.log_scope,
                    format!(
                        "Content blocked by Pure Mode (score={:.1}, terms={:?})",
                        result.score, result.matched_terms
                    ),
                );
                return Err(ConversationExecutionFailure::new(
                    "Response blocked by Pure Mode. Try rephrasing your message.",
                ));
            }
        }
    }

    Ok(ConversationExecutionOutput {
        text,
        reasoning,
        gemini_content,
        usage,
        generated_image_data_urls,
        api_key,
    })
}
