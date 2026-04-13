use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

use rusqlite::{params, OptionalExtension};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::api::{api_request, ApiRequest, ApiResponse};
use crate::dynamic_memory_run_manager::{DynamicMemoryCancellationToken, DynamicMemoryRunManager};
use crate::embedding;
use crate::storage_manager::db::open_db;
use crate::storage_manager::sessions::session_conversation_count;
use crate::usage::tracking::UsageOperationType;
use crate::utils::{log_error, log_info, log_warn, now_millis};

use super::dynamic::{
    apply_memory_decay, calculate_hot_memory_tokens, dynamic_cold_threshold, dynamic_decay_rate,
    dynamic_hot_memory_token_budget, dynamic_max_entries, dynamic_memory_structured_fallback_format,
    enforce_hot_memory_budget, ensure_pinned_hot, find_duplicate_memory_reason,
    generate_memory_id, normalize_query_text, search_cold_memory_indices_by_keyword,
    select_relevant_memory_indices, select_top_cosine_memory_indices, trim_memories_to_max,
};
use super::structured_fallback::{
    memory_operations_fallback_prompt, memory_repairs_fallback_prompt,
    parse_memory_operations_from_text, parse_memory_tag_repairs_from_text,
    structured_fallback_format_label,
};
use crate::chat_manager::execution::{
    find_model_with_credential, prepare_default_sampling_request,
};
use crate::chat_manager::prompt_engine;
use crate::chat_manager::prompts::{
    self, APP_DYNAMIC_MEMORY_LOCAL_TEMPLATE_ID, APP_DYNAMIC_MEMORY_TEMPLATE_ID,
    APP_DYNAMIC_SUMMARY_TEMPLATE_ID,
};
use crate::chat_manager::request::{extract_error_message, extract_text, extract_usage};
use crate::chat_manager::request_builder;
use crate::chat_manager::service::{record_usage_if_available, require_api_key, ChatContext};
use crate::chat_manager::storage::save_session;
use crate::chat_manager::thinking::normalize_thinking_content;
use crate::chat_manager::tooling::{
    parse_tool_calls, ToolCall, ToolChoice, ToolConfig, ToolDefinition,
};
use crate::chat_manager::types::{
    Character, DynamicMemorySettings, MemoryEmbedding, MemoryRetrievalStrategy, Model, Persona,
    ProviderCredential, Session, Settings, StoredMessage,
};

const ALLOWED_MEMORY_CATEGORIES: &[&str] = &[
    "character_trait",
    "relationship",
    "plot_event",
    "world_detail",
    "preference",
    "other",
];
const HARD_DELETE_CONFIDENCE_THRESHOLD: f32 = 0.7;

fn response_preview(provider_id: &str, value: &Value) -> String {
    if let Some(text) =
        extract_text(value, Some(provider_id)).filter(|text| !text.trim().is_empty())
    {
        text
    } else {
        value.to_string()
    }
}

fn log_text_parse_failure(app: &AppHandle, phase: &str, text: &str, err: &str) {
    log_warn(
        app,
        "dynamic_memory",
        format!(
            "{} parse failed: {} | model response preview: {}",
            phase, err, text
        ),
    );
}

fn log_raw_memory_tool_calls(app: &AppHandle, source: &str, calls: &[ToolCall]) {
    let payload = serde_json::to_string(calls).unwrap_or_else(|_| "<serialize failed>".to_string());
    log_info(
        app,
        "dynamic_memory",
        format!(
            "raw memory tool calls source={} count={} payload={}",
            source,
            calls.len(),
            payload
        ),
    );
}

fn dynamic_memory_debug_capture_enabled(settings: &Settings) -> bool {
    cfg!(debug_assertions)
        || settings
            .advanced_settings
            .as_ref()
            .and_then(|advanced| advanced.developer_mode_enabled)
            .unwrap_or(false)
}

fn push_memory_debug_step(debug_steps: &mut Vec<Value>, enabled: bool, step: Value) {
    if enabled {
        debug_steps.push(step);
    }
}

fn max_hard_deletes_per_cycle(initial_count: usize, ratio: f32) -> usize {
    if initial_count == 0 {
        return 0;
    }

    ((initial_count as f32) * ratio).floor().max(1.0) as usize
}

fn dynamic_memory_run_key(session_id: &str) -> String {
    format!("chat:{}", session_id)
}

fn dynamic_memory_request_id(session_id: &str, phase: &str) -> String {
    format!("dynamic-memory:{}:{}", session_id, phase)
}

fn uses_local_dynamic_memory_model(provider_cred: &ProviderCredential, model: &Model) -> bool {
    crate::llama_cpp::is_llama_cpp(Some(provider_cred.provider_id.as_str()))
        || crate::llama_cpp::is_llama_cpp(Some(model.provider_id.as_str()))
}

fn dynamic_memory_llama_sampler_overwrite_enabled(settings: &Settings) -> bool {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.dynamic_memory_llama_sampler_overwrite_enabled)
        .unwrap_or(true)
}

fn recursive_memory_loops_enabled(dynamic_settings: &DynamicMemorySettings) -> bool {
    dynamic_settings.recursive_memory_loops
}

fn memory_tool_call_payload(provider_id: &str, call: &ToolCall, index: usize) -> Value {
    let is_ollama = crate::ollama::is_ollama_provider(Some(provider_id));
    let arguments = if is_ollama {
        call.arguments.clone()
    } else {
        Value::String(
            call.raw_arguments
                .clone()
                .unwrap_or_else(|| serde_json::to_string(&call.arguments).unwrap_or_default()),
        )
    };

    if is_ollama {
        json!({
            "type": "function",
            "function": {
                "index": index,
                "name": call.name,
                "arguments": arguments
            }
        })
    } else {
        json!({
            "id": call.id,
            "type": "function",
            "function": {
                "name": call.name,
                "arguments": arguments
            }
        })
    }
}

fn memory_tool_result_message(
    provider_id: &str,
    tool_call_id: &str,
    tool_name: Option<&str>,
    result: &Value,
) -> Value {
    let mut message = json!({
        "role": "tool",
        "content": serde_json::to_string(result).unwrap_or_default()
    });

    if let Some(obj) = message.as_object_mut() {
        if crate::ollama::is_ollama_provider(Some(provider_id)) {
            if let Some(name) = tool_name {
                obj.insert("tool_name".to_string(), json!(name));
            }
        } else {
            obj.insert("tool_call_id".to_string(), json!(tool_call_id));
        }
    }

    message
}

async fn request_memory_tool_calls(
    app: &AppHandle,
    provider_cred: &ProviderCredential,
    model: &Model,
    overwrite_llama_sampler_config: bool,
    api_key: &str,
    messages_for_api: &Vec<Value>,
    max_tokens: u32,
    context_length: Option<u32>,
    extra_body_fields: Option<HashMap<String, Value>>,
    tool_config: &ToolConfig,
    fallback_format: crate::chat_manager::types::DynamicMemoryStructuredFallbackFormat,
    fallback_label: &str,
    context: &ChatContext,
    session: &mut Session,
    character: &Character,
    debug_capture_enabled: bool,
    debug_steps: &mut Vec<Value>,
    request_id: Option<&str>,
    cancel_token: Option<&DynamicMemoryCancellationToken>,
) -> Result<(Vec<ToolCall>, &'static str), String> {
    match send_dynamic_memory_request(
        app,
        provider_cred,
        model,
        overwrite_llama_sampler_config,
        api_key,
        messages_for_api,
        max_tokens,
        context_length,
        extra_body_fields.clone(),
        Some(tool_config),
        request_id,
        cancel_token,
    )
    .await
    {
        Ok(api_response) => {
            push_memory_debug_step(
                debug_steps,
                debug_capture_enabled,
                json!({
                    "phase": "memory_tool_request",
                    "requestId": request_id,
                    "providerId": provider_cred.provider_id,
                    "model": model.name,
                    "response": {
                        "ok": api_response.ok,
                        "status": api_response.status,
                        "data": api_response.data().clone(),
                    }
                }),
            );
            let usage = extract_usage(api_response.data());
            record_usage_if_available(
                context,
                &usage,
                session,
                character,
                model,
                provider_cred,
                api_key,
                now_millis().unwrap_or(0),
                UsageOperationType::MemoryManager,
                "memory_manager",
            )
            .await;

            if !api_response.ok {
                let fallback = format!("Provider returned status {}", api_response.status);
                let err_message = extract_error_message(api_response.data()).unwrap_or(fallback);
                log_warn(
                    app,
                    "dynamic_memory",
                    format!(
                        "memory tool request failed; retrying with {} fallback: {}",
                        fallback_label, err_message
                    ),
                );
                if cancel_token.is_some_and(|token| token.is_cancelled()) {
                    return Err("Request was cancelled by user".to_string());
                }
                let mut fallback_messages = messages_for_api.to_vec();
                fallback_messages.push(json!({
                    "role": "user",
                    "content": memory_operations_fallback_prompt(fallback_format)
                }));

                let api_response = send_dynamic_memory_request(
                    app,
                    provider_cred,
                    model,
                    overwrite_llama_sampler_config,
                    api_key,
                    &fallback_messages,
                    max_tokens,
                    context_length,
                    extra_body_fields,
                    None,
                    request_id,
                    cancel_token,
                )
                .await?;

                push_memory_debug_step(
                    debug_steps,
                    debug_capture_enabled,
                    json!({
                        "phase": "memory_tool_fallback_after_http_error",
                        "requestId": request_id,
                        "providerId": provider_cred.provider_id,
                        "model": model.name,
                        "response": {
                            "ok": api_response.ok,
                            "status": api_response.status,
                            "data": api_response.data().clone(),
                        }
                    }),
                );

                let usage = extract_usage(api_response.data());
                record_usage_if_available(
                    context,
                    &usage,
                    session,
                    character,
                    model,
                    provider_cred,
                    api_key,
                    now_millis().unwrap_or(0),
                    UsageOperationType::MemoryManager,
                    "memory_manager_fallback",
                )
                .await;

                if !api_response.ok {
                    let fallback = format!("Provider returned status {}", api_response.status);
                    let err_message =
                        extract_error_message(api_response.data()).unwrap_or(fallback.clone());
                    return Err(if err_message == fallback {
                        err_message
                    } else {
                        format!("{} (status {})", err_message, api_response.status)
                    });
                }

                let text = extract_text(api_response.data(), Some(&provider_cred.provider_id))
                    .ok_or_else(|| {
                        "memory fallback returned neither tool calls nor text output".to_string()
                    })?;
                let calls = parse_memory_operations_from_text(&text, fallback_format).map_err(|err| {
                    log_text_parse_failure(app, "memory fallback", &text, &err);
                    err
                })?;
                Ok((calls, "text_fallback_after_http_error"))
            } else {
                let tool_calls = parse_tool_calls(&provider_cred.provider_id, api_response.data());
                if !tool_calls.is_empty() {
                    Ok((tool_calls, "provider_tool_calls"))
                } else {
                    log_warn(
                        app,
                        "dynamic_memory",
                        format!(
                            "memory tool request returned no tool usage; retrying with {} fallback | response preview: {}",
                            fallback_label,
                            response_preview(&provider_cred.provider_id, api_response.data())
                        ),
                    );
                    if cancel_token.is_some_and(|token| token.is_cancelled()) {
                        return Err("Request was cancelled by user".to_string());
                    }
                    let mut fallback_messages = messages_for_api.to_vec();
                    fallback_messages.push(json!({
                        "role": "user",
                        "content": memory_operations_fallback_prompt(fallback_format)
                    }));
                    let api_response = send_dynamic_memory_request(
                        app,
                        provider_cred,
                        model,
                        overwrite_llama_sampler_config,
                        api_key,
                        &fallback_messages,
                        max_tokens,
                        context_length,
                        extra_body_fields,
                        None,
                        request_id,
                        cancel_token,
                    )
                    .await?;

                    push_memory_debug_step(
                        debug_steps,
                        debug_capture_enabled,
                        json!({
                            "phase": "memory_tool_fallback_after_empty_tool_calls",
                            "requestId": request_id,
                            "providerId": provider_cred.provider_id,
                            "model": model.name,
                            "response": {
                                "ok": api_response.ok,
                                "status": api_response.status,
                                "data": api_response.data().clone(),
                            }
                        }),
                    );

                    let usage = extract_usage(api_response.data());
                    record_usage_if_available(
                        context,
                        &usage,
                        session,
                        character,
                        model,
                        provider_cred,
                        api_key,
                        now_millis().unwrap_or(0),
                        UsageOperationType::MemoryManager,
                        "memory_manager_fallback",
                    )
                    .await;

                    if !api_response.ok {
                        let fallback = format!("Provider returned status {}", api_response.status);
                        let err_message =
                            extract_error_message(api_response.data()).unwrap_or(fallback.clone());
                        return Err(if err_message == fallback {
                            err_message
                        } else {
                            format!("{} (status {})", err_message, api_response.status)
                        });
                    }

                    let text = extract_text(api_response.data(), Some(&provider_cred.provider_id))
                        .ok_or_else(|| {
                            "memory fallback returned neither tool calls nor text output"
                                .to_string()
                        })?;
                    let calls = parse_memory_operations_from_text(&text, fallback_format).map_err(|err| {
                        log_text_parse_failure(app, "memory fallback", &text, &err);
                        err
                    })?;
                    Ok((calls, "text_fallback_after_empty_tool_calls"))
                }
            }
        }
        Err(err) => {
            push_memory_debug_step(
                debug_steps,
                debug_capture_enabled,
                json!({
                    "phase": "memory_tool_request_error",
                    "requestId": request_id,
                    "providerId": provider_cred.provider_id,
                    "model": model.name,
                    "error": err,
                }),
            );
            log_warn(
                app,
                "dynamic_memory",
                format!(
                    "memory tool request errored; retrying with {} fallback: {}",
                    fallback_label, err
                ),
            );
            if cancel_token.is_some_and(|token| token.is_cancelled()) {
                return Err("Request was cancelled by user".to_string());
            }
            let mut fallback_messages = messages_for_api.to_vec();
            fallback_messages.push(json!({
                "role": "user",
                "content": memory_operations_fallback_prompt(fallback_format)
            }));
            let api_response = send_dynamic_memory_request(
                app,
                provider_cred,
                model,
                overwrite_llama_sampler_config,
                api_key,
                &fallback_messages,
                max_tokens,
                context_length,
                extra_body_fields,
                None,
                request_id,
                cancel_token,
            )
            .await?;

            push_memory_debug_step(
                debug_steps,
                debug_capture_enabled,
                json!({
                    "phase": "memory_tool_fallback_after_request_error",
                    "requestId": request_id,
                    "providerId": provider_cred.provider_id,
                    "model": model.name,
                    "response": {
                        "ok": api_response.ok,
                        "status": api_response.status,
                        "data": api_response.data().clone(),
                    }
                }),
            );

            let usage = extract_usage(api_response.data());
            record_usage_if_available(
                context,
                &usage,
                session,
                character,
                model,
                provider_cred,
                api_key,
                now_millis().unwrap_or(0),
                UsageOperationType::MemoryManager,
                "memory_manager_fallback",
            )
            .await;

            if !api_response.ok {
                let fallback = format!("Provider returned status {}", api_response.status);
                let err_message =
                    extract_error_message(api_response.data()).unwrap_or(fallback.clone());
                return Err(if err_message == fallback {
                    err_message
                } else {
                    format!("{} (status {})", err_message, api_response.status)
                });
            }

            let text = extract_text(api_response.data(), Some(&provider_cred.provider_id))
                .ok_or_else(|| {
                    "memory fallback returned neither tool calls nor text output".to_string()
                })?;
            let calls = parse_memory_operations_from_text(&text, fallback_format).map_err(|err| {
                log_text_parse_failure(app, "memory fallback", &text, &err);
                err
            })?;
            Ok((calls, "text_fallback_after_request_error"))
        }
    }
}

fn emit_dynamic_memory_transition_toast(
    app: &AppHandle,
    toast_id: String,
    title: &str,
    description: String,
) {
    let _ = app.emit(
        "app://toast",
        json!({
            "id": toast_id,
            "variant": "info",
            "title": title,
            "description": description,
        }),
    );
}

async fn prepare_local_dynamic_memory_cycle(
    app: &AppHandle,
    model: &Model,
    session_id: &str,
) -> Result<(), String> {
    emit_dynamic_memory_transition_toast(
        app,
        format!("dynamic-memory:prepare:{session_id}"),
        "Preparing dynamic memory",
        format!(
            "Unloading the active local chat model before loading {}.",
            model.display_name
        ),
    );
    crate::llama_cpp::llamacpp_unload(app.clone())
        .await
        .map_err(|err| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!(
                    "Failed to unload local chat model before dynamic memory for session {}: {}",
                    session_id, err
                ),
            )
        })
}

async fn finish_local_dynamic_memory_cycle(
    app: &AppHandle,
    model: &Model,
    session_id: &str,
) -> Result<(), String> {
    emit_dynamic_memory_transition_toast(
        app,
        format!("dynamic-memory:finish:{session_id}"),
        "Finishing dynamic memory",
        format!(
            "Unloading {} after the memory update completes.",
            model.display_name
        ),
    );
    crate::llama_cpp::llamacpp_unload(app.clone())
        .await
        .map_err(|err| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!(
                    "Failed to unload local dynamic memory model for session {}: {}",
                    session_id, err
                ),
            )
        })
}

fn is_cancelled_request_error(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("aborted")
        || normalized.contains("cancelled")
        || normalized.contains("canceled")
}

fn conversation_window(messages: &[StoredMessage], limit: usize) -> Vec<StoredMessage> {
    let mut convo: Vec<StoredMessage> = messages
        .iter()
        .filter(|m| m.role == "user" || m.role == "assistant")
        .cloned()
        .collect();
    if convo.len() > limit {
        convo.drain(0..(convo.len() - limit));
    }
    convo
}

fn conversation_count(messages: &[StoredMessage]) -> usize {
    messages
        .iter()
        .filter(|m| m.role == "user" || m.role == "assistant")
        .count()
}

fn resolve_conversation_index_by_message_id(
    app: &AppHandle,
    session_id: &str,
    message_id: &str,
) -> Result<Option<usize>, String> {
    let conn = open_db(app)?;

    // Find the message's position in the canonical ordering (created_at ASC, id ASC),
    // restricted to conversation messages.
    let created_at: Option<i64> = conn
        .query_row(
            "SELECT created_at FROM messages WHERE session_id = ?1 AND id = ?2 AND (role = 'user' OR role = 'assistant')",
            params![session_id, message_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let Some(created_at) = created_at else {
        return Ok(None);
    };

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(1) FROM messages
             WHERE session_id = ?1 AND (role = 'user' OR role = 'assistant')
               AND (created_at < ?2 OR (created_at = ?2 AND id <= ?3))",
            params![session_id, created_at, message_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    Ok(Some(count.max(0) as usize))
}

/// Resolve the last valid cursor (windowEnd) from memory tool events by anchoring on message IDs.
/// This self-heals when messages are deleted (counts shrink) or the conversation is rewound.
/// Returns (window_end_index, cursor_rewound).
fn resolve_last_valid_window_end(
    app: &AppHandle,
    session: &Session,
) -> Result<(usize, bool), String> {
    if session.memory_tool_events.is_empty() {
        return Ok((0, false));
    }

    // Walk backwards to find the newest event whose last summarized message still exists.
    for (rev_idx, event) in session.memory_tool_events.iter().rev().enumerate() {
        let end_id = event
            .get("windowMessageIds")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.last())
            .and_then(|v| v.as_str());

        let Some(end_id) = end_id else {
            continue;
        };

        if let Some(window_end) =
            resolve_conversation_index_by_message_id(app, &session.id, end_id)?
        {
            // If we had to skip one or more newer events, the conversation was rewound.
            return Ok((window_end, rev_idx != 0));
        }
    }

    // No event could be anchored; treat as rewind (cursor reset).
    Ok((0, true))
}

fn cancel_dynamic_memory_cycle(
    app: &AppHandle,
    session: &mut Session,
    message: &str,
) -> Result<(), String> {
    session.memory_status = Some("idle".to_string());
    session.memory_error = None;
    session.updated_at = now_millis()?;
    save_session(app, session)?;
    let _ = app.emit(
        "dynamic-memory:cancelled",
        json!({ "sessionId": session.id }),
    );
    Err(message.to_string())
}

fn ensure_dynamic_memory_not_cancelled(
    app: &AppHandle,
    session: &mut Session,
    token: &DynamicMemoryCancellationToken,
) -> Result<(), String> {
    if token.is_cancelled() {
        return cancel_dynamic_memory_cycle(app, session, "Request was cancelled by user");
    }
    Ok(())
}

fn fetch_conversation_messages_range(
    app: &AppHandle,
    session_id: &str,
    start: usize,
    end: usize,
) -> Result<Vec<StoredMessage>, String> {
    if end <= start {
        return Ok(Vec::new());
    }

    let conn = open_db(app)?;
    let limit = (end - start) as i64;
    let offset = start as i64;

    let mut stmt = conn
        .prepare(
            "SELECT id, role, content, created_at, is_pinned
             FROM messages
             WHERE session_id = ?1 AND (role = 'user' OR role = 'assistant')
             ORDER BY created_at ASC, id ASC
             LIMIT ?2 OFFSET ?3",
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let rows = stmt
        .query_map(params![session_id, limit, offset], |r| {
            let created_at: i64 = r.get(3)?;
            let is_pinned: i64 = r.get(4)?;
            Ok(StoredMessage {
                id: r.get(0)?,
                role: r.get(1)?,
                content: r.get(2)?,
                created_at: created_at.max(0) as u64,
                usage: None,
                variants: Vec::new(),
                selected_variant_id: None,
                memory_refs: Vec::new(),
                used_lorebook_entries: Vec::new(),
                is_pinned: is_pinned != 0,
                attachments: Vec::new(),
                reasoning: None,
                model_id: None,
                fallback_from_model_id: None,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?);
    }
    Ok(out)
}

fn format_memories_with_ids(session: &Session) -> Vec<String> {
    session
        .memory_embeddings
        .iter()
        .map(|m| format!("[{}] {}", m.id, m.text))
        .collect()
}

pub(crate) async fn select_relevant_memories(
    app: &AppHandle,
    session: &Session,
    query: &str,
    limit: usize,
    min_similarity: f32,
    strategy: MemoryRetrievalStrategy,
) -> Vec<MemoryEmbedding> {
    if query.is_empty() || session.memory_embeddings.is_empty() {
        return Vec::new();
    }

    let query_embedding = match embedding::compute_embedding(app.clone(), query.to_string()).await {
        Ok(vec) => vec,
        Err(err) => {
            log_warn(
                app,
                "memory_retrieval",
                format!("embedding failed: {}", err),
            );
            return Vec::new();
        }
    };

    if matches!(strategy, MemoryRetrievalStrategy::Cosine) {
        let cosine_indices = select_top_cosine_memory_indices(
            &query_embedding,
            &session.memory_embeddings,
            limit,
            min_similarity,
        );
        if cosine_indices.is_empty() {
            return Vec::new();
        }
        return cosine_indices
            .into_iter()
            .filter_map(|(idx, score)| {
                session.memory_embeddings.get(idx).map(|mem| {
                    let mut cloned = mem.clone();
                    cloned.match_score = Some(score);
                    cloned
                })
            })
            .collect();
    }

    let cosine_limit = (limit.saturating_sub(2)).max(1);
    let cosine_indices = select_relevant_memory_indices(
        &query_embedding,
        &session.memory_embeddings,
        cosine_limit,
        min_similarity,
    );

    let mut selected: HashSet<usize> = HashSet::new();
    let mut results: Vec<MemoryEmbedding> = Vec::new();

    for (idx, score) in &cosine_indices {
        if let Some(mem) = session.memory_embeddings.get(*idx) {
            let mut cloned = mem.clone();
            cloned.match_score = Some(*score);
            results.push(cloned);
            selected.insert(*idx);
        }
    }

    if results.len() < limit {
        if let Some(recent_idx) = session
            .memory_embeddings
            .iter()
            .enumerate()
            .filter(|(i, m)| !m.is_cold && !selected.contains(i))
            .max_by_key(|(_, m)| m.created_at)
            .map(|(i, _)| i)
        {
            if let Some(mem) = session.memory_embeddings.get(recent_idx) {
                results.push(mem.clone());
                selected.insert(recent_idx);
            }
        }
    }

    if results.len() < limit {
        if let Some(freq_idx) = session
            .memory_embeddings
            .iter()
            .enumerate()
            .filter(|(i, m)| !m.is_cold && !selected.contains(i) && m.access_count > 0)
            .max_by_key(|(_, m)| m.access_count)
            .map(|(i, _)| i)
        {
            if let Some(mem) = session.memory_embeddings.get(freq_idx) {
                results.push(mem.clone());
                selected.insert(freq_idx);
            }
        }
    }

    if results.len() < limit {
        let extra_indices = select_relevant_memory_indices(
            &query_embedding,
            &session.memory_embeddings,
            limit,
            min_similarity,
        );
        for (idx, score) in extra_indices {
            if results.len() >= limit {
                break;
            }
            if !selected.contains(&idx) {
                if let Some(mem) = session.memory_embeddings.get(idx) {
                    let mut cloned = mem.clone();
                    cloned.match_score = Some(score);
                    results.push(cloned);
                    selected.insert(idx);
                }
            }
        }
    }

    if results.is_empty() {
        let normalized_query = normalize_query_text(query);
        let cold_indices = search_cold_memory_indices_by_keyword(
            &session.memory_embeddings,
            &normalized_query,
            limit,
        );
        if !cold_indices.is_empty() {
            crate::utils::log_info(
                app,
                "memory_retrieval",
                format!("Found {} memories via keyword search", cold_indices.len()),
            );
        }

        return cold_indices
            .into_iter()
            .filter_map(|idx| session.memory_embeddings.get(idx).cloned())
            .collect();
    }

    results
}

pub async fn retry_dynamic_memory(
    app: AppHandle,
    session_id: String,
    model_id: Option<String>,
    update_default: Option<bool>,
) -> Result<(), String> {
    log_info(
        &app,
        "dynamic_memory",
        format!(
            "retry requested for session {} with model_id={:?} update_default={:?}",
            session_id, model_id, update_default
        ),
    );
    let context = ChatContext::initialize(app.clone())?;
    let mut session = context
        .load_session(&session_id)?
        .ok_or_else(|| "Session not found".to_string())?;

    let character = context.find_character(&session.character_id)?;

    // Run the memory cycle with optional model override
    process_dynamic_memory_cycle_with_model(
        &app,
        &mut session,
        &context.settings,
        &character,
        model_id.as_deref(),
        update_default.unwrap_or(false),
        true, // force = true for retry
    )
    .await
}

pub async fn trigger_dynamic_memory(app: AppHandle, session_id: String) -> Result<(), String> {
    log_info(
        &app,
        "dynamic_memory",
        format!("trigger requested for session {}", session_id),
    );
    let context = ChatContext::initialize(app.clone())?;
    let mut session = context
        .load_session(&session_id)?
        .ok_or_else(|| "Session not found".to_string())?;

    let character = context.find_character(&session.character_id)?;

    // Run the memory cycle with default settings, but force=true
    process_dynamic_memory_cycle_with_model(
        &app,
        &mut session,
        &context.settings,
        &character,
        None,
        false,
        true,
    )
    .await
}

pub fn abort_dynamic_memory(app: AppHandle, session_id: String) -> Result<(), String> {
    let run_key = dynamic_memory_run_key(&session_id);
    let run_manager = app.state::<DynamicMemoryRunManager>().inner().clone();
    let abort_registry = app.state::<crate::abort_manager::AbortRegistry>();
    run_manager.cancel_run(&abort_registry, &run_key)
}

pub(crate) async fn process_dynamic_memory_cycle(
    app: &AppHandle,
    session: &mut Session,
    settings: &Settings,
    character: &Character,
) -> Result<(), String> {
    // Delegate to the version with model override, using None for defaults, and force=false
    process_dynamic_memory_cycle_with_model(app, session, settings, character, None, false, false)
        .await
}

/// Process dynamic memory cycle with optional model override.
/// If `model_id_override` is Some, use that model instead of the configured one.
/// If `update_default_on_success` is true and the cycle succeeds, update the summarisation model in settings.
async fn process_dynamic_memory_cycle_with_model(
    app: &AppHandle,
    session: &mut Session,
    settings: &Settings,
    character: &Character,
    model_id_override: Option<&str>,
    update_default_on_success: bool,
    force: bool,
) -> Result<(), String> {
    log_info(
        app,
        "dynamic_memory",
        format!(
            "starting cycle: session_id={} force={} model_override={} update_default={} embeddings={} events={}",
            session.id,
            force,
            model_id_override.unwrap_or("none"),
            update_default_on_success,
            session.memory_embeddings.len(),
            session.memory_tool_events.len()
        ),
    );
    let Some(advanced) = settings.advanced_settings.as_ref() else {
        log_info(
            app,
            "dynamic_memory",
            "advanced settings missing; skipping dynamic memory",
        );
        return Ok(());
    };
    let Some(dynamic) = advanced.dynamic_memory.as_ref() else {
        log_info(
            app,
            "dynamic_memory",
            "dynamic memory config missing; skipping",
        );
        return Ok(());
    };
    if !dynamic.enabled || !character.memory_type.eq_ignore_ascii_case("dynamic") {
        log_info(
            app,
            "dynamic_memory",
            format!(
                "dynamic memory disabled (global={}, character_type={})",
                dynamic.enabled, character.memory_type
            ),
        );
        return Ok(());
    }

    let window_size = dynamic.summary_message_interval.max(1) as usize;
    let total_messages = session.messages.len();
    let total_convo_at_start = match session_conversation_count(app.clone(), session.id.clone()) {
        Ok(count) => count.max(0) as usize,
        Err(err) => {
            log_warn(
                app,
                "dynamic_memory",
                format!("failed to count conversation messages: {}", err),
            );
            conversation_count(&session.messages)
        }
    };

    // Cursor-based delta summary window:
    // - Normal cycles summarize all new conversation messages since last windowEnd.
    // - If backlog > window_size, include the whole backlog in this run (one-time catch-up),
    //   then future cycles continue at window_size cadence.
    // - Forced cycles (retry/manual trigger/model override) summarize the most recent window_size
    //   messages, even if there are no new messages.
    let (last_window_end, cursor_rewound) = resolve_last_valid_window_end(app, session)?;

    let new_convo = total_convo_at_start.saturating_sub(last_window_end);
    log_info(
        app,
        "dynamic_memory",
        format!(
            "considering dynamic memory: total_convo_at_start={} window_size={} last_window_end={} new_convo={} cursor_rewound={}",
            total_convo_at_start, window_size, last_window_end, new_convo, cursor_rewound
        ),
    );

    // For retry/manual trigger/model override, skip the "enough new messages" gate.
    // Also skip if we detected a rewind; we need to rebuild the summary/memory state.
    if model_id_override.is_none() && !force && !cursor_rewound {
        if total_convo_at_start <= last_window_end {
            log_info(
                app,
                "dynamic_memory",
                format!(
                    "no new messages since last run; skipping (total_convo_at_start={} last_window_end={})",
                    total_convo_at_start, last_window_end
                ),
            );
            return Ok(());
        }

        if new_convo < window_size {
            let next_window_end = last_window_end + window_size;
            log_info(
                app,
                "dynamic_memory",
                format!(
                    "not enough new messages since last run (needed {}, got {}, next_window_end={})",
                    window_size, new_convo, next_window_end
                ),
            );
            return Ok(());
        }
    }

    let mut window_start = if cursor_rewound {
        0
    } else if force || model_id_override.is_some() {
        total_convo_at_start.saturating_sub(window_size)
    } else {
        last_window_end
    };
    let mut window_end = total_convo_at_start;

    let convo_window = match fetch_conversation_messages_range(
        app,
        &session.id,
        window_start,
        window_end,
    ) {
        Ok(msgs) => msgs,
        Err(err) => {
            log_warn(
                app,
                "dynamic_memory",
                format!(
                    "failed to fetch conversation range from DB (start={} end={}): {}; falling back to in-memory window",
                    window_start, window_end, err
                ),
            );
            let fallback = conversation_window(&session.messages, window_size);
            window_end = total_convo_at_start;
            window_start = window_end.saturating_sub(fallback.len());
            fallback
        }
    };

    if convo_window.is_empty() {
        log_warn(
            app,
            "dynamic_memory",
            format!(
                "no messages in computed window; skipping (window_start={} window_end={} total_convo_at_start={})",
                window_start, window_end, total_convo_at_start
            ),
        );
        return Ok(());
    }

    let run_key = dynamic_memory_run_key(&session.id);
    let run_manager = app.state::<DynamicMemoryRunManager>().inner().clone();
    let run_guard = run_manager.start_run(run_key);
    let cancel_token = run_guard.token();

    log_info(
        app,
        "dynamic_memory",
        format!(
            "snapshot taken: window_start={} window_end={} window_count={} window_size={} total_convo_at_start={} total_messages={} non_convo_messages={}",
            window_start,
            window_end,
            convo_window.len(),
            window_size,
            total_convo_at_start,
            total_messages,
            total_messages.saturating_sub(total_convo_at_start),
        ),
    );

    let window_message_ids: Vec<String> = convo_window.iter().map(|m| m.id.clone()).collect();

    // Apply importance decay to all hot, unpinned memories
    let decay_rate = dynamic_decay_rate(settings);
    let cold_threshold = dynamic_cold_threshold(settings);
    let pinned_fixed = ensure_pinned_hot(&mut session.memory_embeddings);
    if pinned_fixed > 0 {
        log_info(
            app,
            "dynamic_memory",
            format!("Restored {} pinned memories to hot", pinned_fixed),
        );
    }

    let (decayed, demoted) =
        apply_memory_decay(&mut session.memory_embeddings, decay_rate, cold_threshold);
    if decayed > 0 || !demoted.is_empty() {
        log_info(
            app,
            "dynamic_memory",
            format!(
                "Memory decay applied: {} memories decayed, {} demoted to cold",
                decayed,
                demoted.len()
            ),
        );
    }

    let summarisation_model_id: String = match model_id_override {
        Some(id) => {
            log_info(
                app,
                "dynamic_memory",
                format!("using override model: {}", id),
            );
            id.to_string()
        }
        None => match advanced.summarisation_model_id.as_ref() {
            Some(id) => id.clone(),
            None => {
                let err = "Summarisation model not configured";
                log_warn(app, "dynamic_memory", err);
                record_dynamic_memory_error(app, session, err, "summary_model");
                return Err(err.to_string());
            }
        },
    };

    let (summary_model, summary_provider) =
        match find_model_with_credential(settings, &summarisation_model_id) {
            Some(found) => found,
            None => {
                let err = "Summarisation model unavailable";
                log_error(app, "dynamic_memory", err);
                record_dynamic_memory_error(app, session, err, "summary_model");
                return Err(err.to_string());
            }
        };

    let api_key = match require_api_key(app, summary_provider, "dynamic_memory") {
        Ok(key) => key,
        Err(err) => {
            record_dynamic_memory_error(app, session, &err, "summary_api_key");
            return Err(err);
        }
    };
    let using_local_dynamic_memory_model =
        uses_local_dynamic_memory_model(summary_provider, summary_model);
    let debug_capture_enabled = dynamic_memory_debug_capture_enabled(settings);
    let mut debug_steps: Vec<Value> = Vec::new();
    if using_local_dynamic_memory_model {
        if let Err(err) = prepare_local_dynamic_memory_cycle(app, summary_model, &session.id).await
        {
            record_dynamic_memory_error(app, session, &err, "prepare_local_model");
            return Err(err);
        }
    }
    // Set processing state
    session.memory_status = Some("processing".to_string());
    session.memory_error = None;
    session.memory_progress_step = Some(1);
    if let Err(e) = save_session(app, session) {
        log_warn(
            app,
            "dynamic_memory",
            format!("failed to save session state: {}", e),
        );
    }

    log_info(
        app,
        "dynamic_memory",
        format!(
            "running summarisation with model={} window_size={} total_convo_at_start={} window_start={} window_end={} window_ids={:?}",
            summary_model.name, window_size, total_convo_at_start, window_start, window_end, window_message_ids
        ),
    );
    let _ = app.emit(
        "dynamic-memory:processing",
        json!({ "sessionId": session.id }),
    );
    let _ = app.emit(
        "dynamic-memory:progress",
        json!({ "sessionId": session.id, "step": 1, "totalSteps": 4, "label": "Summarizing conversation" }),
    );

    ensure_dynamic_memory_not_cancelled(app, session, &cancel_token)?;

    let summary_request_id = dynamic_memory_request_id(&session.id, "summary");
    run_guard.set_active_request_id(Some(summary_request_id.clone()));

    let summary = match summarize_messages(
        app,
        summary_provider,
        summary_model,
        &api_key,
        &convo_window,
        if cursor_rewound {
            None
        } else {
            session.memory_summary.as_deref()
        },
        character,
        session,
        settings,
        None,
        debug_capture_enabled,
        &mut debug_steps,
        Some(&summary_request_id),
        Some(&cancel_token),
    )
    .await
    {
        Ok(s) => s,
        Err(err) => {
            run_guard.set_active_request_id(None);
            if debug_capture_enabled && !debug_steps.is_empty() {
                let event = json!({
                    "id": Uuid::new_v4().to_string(),
                    "windowStart": window_start,
                    "windowEnd": window_end,
                    "windowMessageIds": window_message_ids,
                    "summary": "",
                    "actions": [],
                    "error": err,
                    "status": "error",
                    "stage": "summarization",
                    "debugSteps": debug_steps,
                    "createdAt": now_millis().unwrap_or_default(),
                });
                session.memory_tool_events.push(event);
                if session.memory_tool_events.len() > 50 {
                    let excess = session.memory_tool_events.len() - 50;
                    session.memory_tool_events.drain(0..excess);
                }
                let _ = save_session(app, session);
            }
            if is_cancelled_request_error(&err) {
                if using_local_dynamic_memory_model {
                    let _ =
                        finish_local_dynamic_memory_cycle(app, summary_model, &session.id).await;
                }
                return cancel_dynamic_memory_cycle(app, session, &err);
            }
            if using_local_dynamic_memory_model {
                let _ = finish_local_dynamic_memory_cycle(app, summary_model, &session.id).await;
            }
            record_dynamic_memory_error(app, session, &err, "summarization");
            return Err(err);
        }
    };
    run_guard.set_active_request_id(None);
    log_info(
        app,
        "dynamic_memory",
        format!(
            "summary generated: length={} chars tokens={}",
            summary.len(),
            crate::embedding::tokenizer::count_tokens(app, &summary).unwrap_or(0)
        ),
    );

    log_info(
        app,
        "dynamic_memory",
        format!(
            "summary length={} chars; invoking memory tools",
            summary.len()
        ),
    );
    session.memory_progress_step = Some(2);
    let _ = save_session(app, session);
    let _ = app.emit(
        "dynamic-memory:progress",
        json!({ "sessionId": session.id, "step": 2, "totalSteps": 4, "label": "Analyzing memories" }),
    );
    ensure_dynamic_memory_not_cancelled(app, session, &cancel_token)?;

    let tools_request_id = dynamic_memory_request_id(&session.id, "tools");
    run_guard.set_active_request_id(Some(tools_request_id.clone()));
    let actions = match run_memory_tool_update(
        app,
        summary_provider,
        summary_model,
        &api_key,
        session,
        settings,
        dynamic,
        &summary,
        &convo_window,
        character,
        debug_capture_enabled,
        &mut debug_steps,
        Some(&tools_request_id),
        Some(&cancel_token),
    )
    .await
    {
        Ok(actions) => actions,
        Err(err) => {
            run_guard.set_active_request_id(None);
            if is_cancelled_request_error(&err) {
                if using_local_dynamic_memory_model {
                    let _ =
                        finish_local_dynamic_memory_cycle(app, summary_model, &session.id).await;
                }
                return cancel_dynamic_memory_cycle(app, session, &err);
            }
            log_error(
                app,
                "dynamic_memory",
                format!("memory tool update failed: {}", err),
            );
            if using_local_dynamic_memory_model {
                let _ = finish_local_dynamic_memory_cycle(app, summary_model, &session.id).await;
            }

            let event = json!({
                "id": Uuid::new_v4().to_string(),
                "windowStart": window_start,
                "windowEnd": window_end,
                "windowMessageIds": window_message_ids,
                "summary": summary,
                "actions": [],
                "error": err,
                "status": "error",
                "debugSteps": if debug_capture_enabled { Value::Array(debug_steps.clone()) } else { Value::Null },
                "createdAt": now_millis().unwrap_or_default(),
            });
            session.memory_summary = Some(summary.clone());
            session.memory_summary_token_count =
                crate::embedding::tokenizer::count_tokens(app, &summary).unwrap_or(0);
            session.memory_tool_events.push(event);
            if session.memory_tool_events.len() > 50 {
                let excess = session.memory_tool_events.len() - 50;
                session.memory_tool_events.drain(0..excess);
            }
            session.memory_status = Some("failed".to_string());
            session.memory_error = Some(format!("memory_tools: {}", err));
            session.memory_progress_step = None;
            session.updated_at = now_millis()?;
            if let Err(save_err) = save_session(app, session) {
                record_dynamic_memory_error(app, session, &save_err, "save_session");
                return Ok(());
            }
            let _ = app.emit(
                "dynamic-memory:error",
                json!({ "sessionId": session.id, "error": err, "stage": "memory_tools" }),
            );
            return Ok(());
        }
    };
    run_guard.set_active_request_id(None);

    session.memory_progress_step = Some(3);
    let _ = save_session(app, session);
    let _ = app.emit(
        "dynamic-memory:progress",
        json!({ "sessionId": session.id, "step": 3, "totalSteps": 4, "label": "Applying changes" }),
    );
    ensure_dynamic_memory_not_cancelled(app, session, &cancel_token)?;

    session.memory_summary = Some(summary.clone());
    session.memory_summary_token_count =
        crate::embedding::tokenizer::count_tokens(app, &summary).unwrap_or(0);
    let event = json!({
        "id": Uuid::new_v4().to_string(),
        "windowStart": window_start,
        "windowEnd": window_end,
        "windowMessageIds": window_message_ids,
        "summary": summary,
        "actions": actions,
        "debugSteps": if debug_capture_enabled { Value::Array(debug_steps.clone()) } else { Value::Null },
        "createdAt": now_millis().unwrap_or_default(),
    });
    session.memory_tool_events.push(event);
    if session.memory_tool_events.len() > 50 {
        let excess = session.memory_tool_events.len() - 50;
        session.memory_tool_events.drain(0..excess);
    }

    session.memory_progress_step = Some(4);
    let _ = app.emit(
        "dynamic-memory:progress",
        json!({ "sessionId": session.id, "step": 4, "totalSteps": 4, "label": "Organizing memories" }),
    );
    session.memory_status = Some("idle".to_string());
    session.memory_error = None;
    session.memory_progress_step = None;
    session.updated_at = now_millis()?;
    if let Err(err) = save_session(app, session) {
        if using_local_dynamic_memory_model {
            let _ = finish_local_dynamic_memory_cycle(app, summary_model, &session.id).await;
        }
        record_dynamic_memory_error(app, session, &err, "save_session");
        return Err(err);
    }

    if update_default_on_success && model_id_override.is_some() {
        log_info(
            app,
            "dynamic_memory",
            format!(
                "updating default summarisation model to: {}",
                summarisation_model_id
            ),
        );
        if let Err(err) = update_summarisation_model_setting(app, &summarisation_model_id) {
            log_warn(
                app,
                "dynamic_memory",
                format!("failed to update default model: {}", err),
            );
        }
    }

    let _ = app.emit("dynamic-memory:success", json!({ "sessionId": session.id }));
    log_info(
        app,
        "dynamic_memory",
        format!(
            "dynamic memory cycle complete: events={}, memories={}, embeddings={}, windowEnd={}",
            session.memory_tool_events.len(),
            session.memories.len(),
            session.memory_embeddings.len(),
            window_end
        ),
    );

    if using_local_dynamic_memory_model {
        finish_local_dynamic_memory_cycle(app, summary_model, &session.id).await?;
    }

    Ok(())
}

fn update_summarisation_model_setting(app: &AppHandle, model_id: &str) -> Result<(), String> {
    use crate::storage_manager::settings::{internal_read_settings, settings_set_advanced};

    let settings_json =
        internal_read_settings(app)?.ok_or_else(|| "Settings not found".to_string())?;

    let settings_value: serde_json::Value = serde_json::from_str(&settings_json).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to parse settings: {}", e),
        )
    })?;

    let mut advanced = settings_value
        .get("advancedSettings")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));

    if let Some(obj) = advanced.as_object_mut() {
        obj.insert(
            "summarisationModelId".to_string(),
            serde_json::Value::String(model_id.to_string()),
        );
    }

    let advanced_json = serde_json::to_string(&advanced).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to serialize advanced settings: {}", e),
        )
    })?;

    settings_set_advanced(app.clone(), advanced_json)?;
    Ok(())
}

fn sanitize_memory_id(id: &str) -> String {
    id.trim()
        .trim_matches(|c| {
            c == '#'
                || c == '*'
                || c == '"'
                || c == '\''
                || c == '['
                || c == ']'
                || c == '('
                || c == ')'
        })
        .to_string()
}

fn record_dynamic_memory_error(app: &AppHandle, session: &mut Session, error: &str, stage: &str) {
    let formatted_error = format!("{}: {}", stage, error);
    log_error(
        app,
        "dynamic_memory",
        format!("{} failed: {}", stage, error),
    );

    session.memory_status = Some("failed".to_string());
    session.memory_error = Some(formatted_error.clone());
    session.memory_progress_step = None;
    session.updated_at = now_millis().unwrap_or(session.updated_at);

    if let Err(save_err) = save_session(app, session) {
        log_error(
            app,
            "dynamic_memory",
            format!("failed to persist error state: {}", save_err),
        );
    }

    let _ = app.emit(
        "dynamic-memory:error",
        json!({
            "sessionId": session.id,
            "error": formatted_error,
            "stage": stage,
        }),
    );
}

fn normalize_llm_output_text(raw: &str) -> String {
    let trimmed = raw.trim();
    let without_fences = if trimmed.starts_with("```") {
        let mut lines = trimmed.lines();
        let _ = lines.next();
        let mut body: Vec<&str> = lines.collect();
        if body
            .last()
            .map(|line| line.trim() == "```")
            .unwrap_or(false)
        {
            body.pop();
        }
        body.join("\n").trim().to_string()
    } else {
        trimmed.to_string()
    };

    normalize_thinking_content(Some(&without_fences), None).content
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn validate_summary_text(summary: &str) -> Result<String, String> {
    let normalized = collapse_whitespace(&normalize_llm_output_text(summary));
    if normalized.is_empty() {
        return Err("summary was empty".to_string());
    }
    if normalized.len() > 6_000 {
        return Err("summary was implausibly long".to_string());
    }

    let lower = normalized.to_ascii_lowercase();
    let refusal_prefixes = [
        "i'm sorry",
        "i am sorry",
        "sorry,",
        "sorry but",
        "i can't help",
        "i cannot help",
        "i can't assist",
        "i cannot assist",
        "i can't provide",
        "i cannot provide",
        "i'm unable to",
        "i am unable to",
        "cannot comply",
    ];
    if refusal_prefixes
        .iter()
        .any(|prefix| lower.starts_with(prefix))
    {
        return Err("summary looked like a refusal".to_string());
    }
    if lower.contains("write_summary") || lower.contains("create_memory(") {
        return Err("summary leaked tool syntax".to_string());
    }

    Ok(normalized)
}

fn validate_memory_text(memory: &str) -> Result<String, String> {
    let normalized = collapse_whitespace(&normalize_llm_output_text(memory));
    if normalized.is_empty() {
        return Err("memory was empty".to_string());
    }
    if normalized.len() > 280 {
        return Err("memory was too long".to_string());
    }

    let lower = normalized.to_ascii_lowercase();
    let refusal_markers = [
        "i'm sorry",
        "i am sorry",
        "i can't",
        "i cannot",
        "i'm unable",
        "i am unable",
        "cannot comply",
        "i won't help",
    ];
    if refusal_markers
        .iter()
        .any(|marker| lower.starts_with(marker) || lower.contains(marker))
    {
        return Err("memory looked like a refusal".to_string());
    }

    let meta_markers = [
        "as an ai",
        "as a language model",
        "assistant:",
        "user:",
        "system:",
        "content policy",
        "safety policy",
        "cannot assist with",
        "here's a summary",
        "write_summary",
        "create_memory(",
        "\"operations\"",
        "\"items\"",
    ];
    if meta_markers.iter().any(|marker| lower.contains(marker)) {
        return Err("memory looked like meta output".to_string());
    }

    Ok(normalized)
}

fn guess_memory_category(text: &str) -> String {
    let lower = text.to_ascii_lowercase();

    if [
        "prefer",
        "preference",
        "likes",
        "dislikes",
        "favorite",
        "boundary",
        "request",
        "wants",
        "doesn't want",
        "does not want",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        return "preference".to_string();
    }

    if [
        "friend",
        "ally",
        "enemy",
        "trust",
        "relationship",
        "bond",
        "dating",
        "married",
        "siblings",
        "partners",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        return "relationship".to_string();
    }

    if [
        "city", "town", "kingdom", "forest", "artifact", "magic", "rule", "world", "location",
        "village",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        return "world_detail".to_string();
    }

    if [
        "decided",
        "chose",
        "agreed",
        "arrived",
        "left",
        "found",
        "discovered",
        "promised",
        "killed",
        "saved",
        "escaped",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        return "plot_event".to_string();
    }

    if [
        "afraid",
        "fear",
        "goal",
        "trait",
        "personality",
        "backstory",
        "secret",
        "revealed",
        "believes",
        "hates",
        "loves",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        return "character_trait".to_string();
    }

    "other".to_string()
}

fn tool_choice_requires_auto(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("tool choice must be auto")
        || lower.contains("tool_choice must be auto")
        || (lower.contains("tool choice") && lower.contains("auto"))
}

fn requested_parallel_tool_calls(
    provider_cred: &ProviderCredential,
    tool_config: Option<&ToolConfig>,
    extra_body_fields: Option<&HashMap<String, Value>>,
) -> Option<bool> {
    if provider_cred.provider_id != "llamacpp"
        || !tool_config.map(|cfg| !cfg.tools.is_empty()).unwrap_or(false)
    {
        return None;
    }
    Some(
        extra_body_fields
            .and_then(|extra| extra.get("parallel_tool_calls"))
            .and_then(|value| value.as_bool())
            .unwrap_or(true),
    )
}

fn parallel_tool_calls_requires_disable(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    (lower.contains("parallel_tool_calls") || lower.contains("parallel tool calls"))
        && (lower.contains("unsupported")
            || lower.contains("unknown")
            || lower.contains("invalid")
            || lower.contains("unexpected")
            || lower.contains("must be"))
}

fn tool_extra_fields_with_parallel_disabled(
    extra_body_fields: Option<HashMap<String, Value>>,
) -> Option<HashMap<String, Value>> {
    let mut extra = extra_body_fields.unwrap_or_default();
    extra.insert("parallel_tool_calls".to_string(), json!(false));
    if extra.is_empty() {
        None
    } else {
        Some(extra)
    }
}

fn tool_config_with_auto_choice(tool_config: &ToolConfig) -> ToolConfig {
    let mut cloned = tool_config.clone();
    cloned.choice = Some(ToolChoice::Auto);
    cloned
}

async fn send_dynamic_memory_request(
    app: &AppHandle,
    provider_cred: &ProviderCredential,
    model: &Model,
    overwrite_llama_sampler_config: bool,
    api_key: &str,
    messages_for_api: &Vec<Value>,
    max_tokens: u32,
    context_length: Option<u32>,
    extra_body_fields: Option<HashMap<String, Value>>,
    tool_config: Option<&ToolConfig>,
    request_id: Option<&str>,
    cancel_token: Option<&DynamicMemoryCancellationToken>,
) -> Result<ApiResponse, String> {
    if cancel_token.is_some_and(|token| token.is_cancelled()) {
        return Err("Request was cancelled by user".to_string());
    }
    let extra_body_fields = sanitize_dynamic_memory_extra_body_fields(
        extra_body_fields,
        overwrite_llama_sampler_config,
    );
    if let Some(parallel_tool_calls) =
        requested_parallel_tool_calls(provider_cred, tool_config, extra_body_fields.as_ref())
    {
        log_info(
            app,
            "dynamic_memory",
            format!(
                "sending memory tool request with parallel_tool_calls={} provider={} model={} request_id={}",
                parallel_tool_calls,
                provider_cred.provider_id,
                model.name,
                request_id.unwrap_or("none")
            ),
        );
    }
    let built = request_builder::build_chat_request(
        provider_cred,
        api_key,
        &model.name,
        messages_for_api,
        None,
        Some(0.4),
        Some(1.0),
        max_tokens,
        context_length,
        false,
        request_id.map(|id| id.to_string()),
        None,
        None,
        None,
        tool_config,
        false,
        None,
        None,
        false,
        extra_body_fields.clone(),
    );

    let api_request_payload = ApiRequest {
        url: built.url,
        method: Some("POST".into()),
        headers: Some(built.headers),
        query: None,
        body: Some(built.body),
        timeout_ms: Some(crate::transport::DEFAULT_REQUEST_TIMEOUT_MS),
        stream: Some(false),
        request_id: built.request_id.clone(),
        provider_id: Some(provider_cred.provider_id.clone()),
    };

    let first_response = match api_request(app.clone(), api_request_payload).await {
        Ok(response) => response,
        Err(err) => {
            if tool_config.is_some()
                && provider_cred.provider_id == "llamacpp"
                && parallel_tool_calls_requires_disable(&err)
            {
                if cancel_token.is_some_and(|token| token.is_cancelled()) {
                    return Err("Request was cancelled by user".to_string());
                }
                log_warn(
                    app,
                    "dynamic_memory",
                    format!(
                        "provider rejected forced parallel tool calls; retrying with parallel_tool_calls=false. Provider={}, model={}",
                        provider_cred.provider_id, model.name
                    ),
                );
                let built = request_builder::build_chat_request(
                    provider_cred,
                    api_key,
                    &model.name,
                    messages_for_api,
                    None,
                    Some(0.4),
                    Some(1.0),
                    max_tokens,
                    context_length,
                    false,
                    request_id.map(|id| id.to_string()),
                    None,
                    None,
                    None,
                    tool_config,
                    false,
                    None,
                    None,
                    false,
                    tool_extra_fields_with_parallel_disabled(extra_body_fields.clone()),
                );
                log_info(
                    app,
                    "dynamic_memory",
                    format!(
                        "retrying memory tool request with parallel_tool_calls=false provider={} model={} request_id={}",
                        provider_cred.provider_id,
                        model.name,
                        request_id.unwrap_or("none")
                    ),
                );

                let api_request_payload = ApiRequest {
                    url: built.url,
                    method: Some("POST".into()),
                    headers: Some(built.headers),
                    query: None,
                    body: Some(built.body),
                    timeout_ms: Some(crate::transport::DEFAULT_REQUEST_TIMEOUT_MS),
                    stream: Some(false),
                    request_id: built.request_id.clone(),
                    provider_id: Some(provider_cred.provider_id.clone()),
                };

                api_request(app.clone(), api_request_payload).await?
            } else {
                return Err(err);
            }
        }
    };

    if cancel_token.is_some_and(|token| token.is_cancelled()) {
        return Err("Request was cancelled by user".to_string());
    }

    if !first_response.ok {
        let fallback = format!("Provider returned status {}", first_response.status);
        let err_message = extract_error_message(first_response.data()).unwrap_or(fallback);

        if let Some(cfg) = tool_config {
            if provider_cred.provider_id == "llamacpp"
                && parallel_tool_calls_requires_disable(&err_message)
            {
                if cancel_token.is_some_and(|token| token.is_cancelled()) {
                    return Err("Request was cancelled by user".to_string());
                }
                log_warn(
                    app,
                    "dynamic_memory",
                    format!(
                        "provider rejected forced parallel tool calls; retrying with parallel_tool_calls=false. Provider={}, model={}",
                        provider_cred.provider_id, model.name
                    ),
                );
                let built = request_builder::build_chat_request(
                    provider_cred,
                    api_key,
                    &model.name,
                    messages_for_api,
                    None,
                    Some(0.4),
                    Some(1.0),
                    max_tokens,
                    context_length,
                    false,
                    request_id.map(|id| id.to_string()),
                    None,
                    None,
                    None,
                    tool_config,
                    false,
                    None,
                    None,
                    false,
                    tool_extra_fields_with_parallel_disabled(extra_body_fields.clone()),
                );
                log_info(
                    app,
                    "dynamic_memory",
                    format!(
                        "retrying memory tool request with parallel_tool_calls=false after HTTP error provider={} model={} request_id={}",
                        provider_cred.provider_id,
                        model.name,
                        request_id.unwrap_or("none")
                    ),
                );

                let api_request_payload = ApiRequest {
                    url: built.url,
                    method: Some("POST".into()),
                    headers: Some(built.headers),
                    query: None,
                    body: Some(built.body),
                    timeout_ms: Some(crate::transport::DEFAULT_REQUEST_TIMEOUT_MS),
                    stream: Some(false),
                    request_id: built.request_id.clone(),
                    provider_id: Some(provider_cred.provider_id.clone()),
                };

                return api_request(app.clone(), api_request_payload).await;
            }
            if !matches!(cfg.choice, Some(ToolChoice::Auto))
                && tool_choice_requires_auto(&err_message)
            {
                if cancel_token.is_some_and(|token| token.is_cancelled()) {
                    return Err("Request was cancelled by user".to_string());
                }
                log_warn(
                    app,
                    "dynamic_memory",
                    format!(
                        "provider rejected forced tool choice; retrying dynamic memory request with auto tool choice. Provider={}, model={}",
                        provider_cred.provider_id, model.name
                    ),
                );
                let auto_tool_config = tool_config_with_auto_choice(cfg);
                let built = request_builder::build_chat_request(
                    provider_cred,
                    api_key,
                    &model.name,
                    messages_for_api,
                    None,
                    Some(0.2),
                    Some(1.0),
                    max_tokens,
                    context_length,
                    false,
                    request_id.map(|id| id.to_string()),
                    None,
                    None,
                    None,
                    Some(&auto_tool_config),
                    false,
                    None,
                    None,
                    false,
                    extra_body_fields,
                );

                let api_request_payload = ApiRequest {
                    url: built.url,
                    method: Some("POST".into()),
                    headers: Some(built.headers),
                    query: None,
                    body: Some(built.body),
                    timeout_ms: Some(crate::transport::DEFAULT_REQUEST_TIMEOUT_MS),
                    stream: Some(false),
                    request_id: built.request_id.clone(),
                    provider_id: Some(provider_cred.provider_id.clone()),
                };

                return api_request(app.clone(), api_request_payload).await;
            }
        }
    }

    Ok(first_response)
}

fn sanitize_dynamic_memory_extra_body_fields(
    extra_body_fields: Option<HashMap<String, Value>>,
    overwrite_llama_sampler_config: bool,
) -> Option<HashMap<String, Value>> {
    if !overwrite_llama_sampler_config {
        return extra_body_fields;
    }
    let mut extra = extra_body_fields.unwrap_or_default();
    for key in [
        "llamaSamplerProfile",
        "llamaSamplerOrder",
        "llamaMinP",
        "llamaTypicalP",
        "llamaDisableSamplerProfileDefaults",
        "top_k",
        "frequency_penalty",
        "presence_penalty",
        "min_p",
        "typical_p",
    ] {
        extra.remove(key);
    }
    extra.insert(
        "llamaDisableSamplerProfileDefaults".to_string(),
        json!(true),
    );
    extra.insert(
        "llamaSamplerOrder".to_string(),
        json!([
            "penalties",
            "grammar",
            "top_k",
            "top_p",
            "temp",
            "min_p",
            "typical"
        ]),
    );
    extra.insert("top_k".to_string(), json!(40));
    extra.insert("frequency_penalty".to_string(), json!(0.0));
    extra.insert("presence_penalty".to_string(), json!(0.0));
    extra.insert("min_p".to_string(), json!(0.0));
    extra.insert("typical_p".to_string(), json!(0.0));

    if extra.is_empty() {
        None
    } else {
        Some(extra)
    }
}

async fn run_memory_tool_update(
    app: &AppHandle,
    provider_cred: &ProviderCredential,
    model: &Model,
    api_key: &str,
    session: &mut Session,
    settings: &Settings,
    dynamic_settings: &DynamicMemorySettings,
    summary: &str,
    convo_window: &[StoredMessage],
    character: &Character,
    debug_capture_enabled: bool,
    debug_steps: &mut Vec<Value>,
    request_id: Option<&str>,
    cancel_token: Option<&DynamicMemoryCancellationToken>,
) -> Result<Vec<Value>, String> {
    let overwrite_llama_sampler_config = dynamic_memory_llama_sampler_overwrite_enabled(settings);
    let tool_config = build_memory_tool_config();
    let max_entries = dynamic_max_entries(settings);

    let mut messages_for_api = Vec::new();
    let system_role = request_builder::system_role_for(provider_cred);

    let template_id = if uses_local_dynamic_memory_model(provider_cred, model) {
        APP_DYNAMIC_MEMORY_LOCAL_TEMPLATE_ID
    } else {
        APP_DYNAMIC_MEMORY_TEMPLATE_ID
    };

    let base_template = prompts::get_template(app, template_id)
        .ok()
        .flatten()
        .map(|t| t.content)
        .unwrap_or_else(|| {
            "You maintain a long-term memory index for a conversation transcript. Use tools to add or delete concise factual memories. Every create_memory call must include a category tag. Keep the list tidy and capped at {{max_entries}} entries. Prefer deleting by ID when removing items. When finished, call the done tool.".to_string()
        });

    let pinned_fixed = ensure_pinned_hot(&mut session.memory_embeddings);
    if pinned_fixed > 0 {
        log_info(
            app,
            "dynamic_memory",
            format!("Restored {} pinned memories to hot", pinned_fixed),
        );
    }

    let current_tokens = calculate_hot_memory_tokens(&session.memory_embeddings);
    let token_budget = dynamic_hot_memory_token_budget(settings);

    let rendered =
        prompt_engine::render_with_context(app, &base_template, character, None, session, settings)
            .replace("{{max_entries}}", &max_entries.to_string())
            .replace("{{current_memory_tokens}}", &current_tokens.to_string())
            .replace("{{hot_token_budget}}", &token_budget.to_string());

    crate::chat_manager::messages::push_system_message(
        &mut messages_for_api,
        &system_role,
        Some(rendered),
    );
    let memory_lines = format_memories_with_ids(session);
    messages_for_api.push(json!({
        "role": "user",
        "content": format!(
            "Conversation transcript summary:\n{}\n\nRecent transcript lines:\n{}\n\nCurrent memories (with IDs):\n{}",
            summary,
            convo_window.iter().map(|m| format!("{}: {}", m.role, m.content)).collect::<Vec<_>>().join("\n"),
            if memory_lines.is_empty() { "none".to_string() } else { memory_lines.join("\n") }
        )
    }));

    let (request_settings, extra_body_fields) = prepare_default_sampling_request(
        &provider_cred.provider_id,
        session,
        model,
        settings,
        0.2,
        1.0,
        None,
        None,
        None,
    );
    let context = ChatContext::initialize(app.clone())?;
    let fallback_format = dynamic_memory_structured_fallback_format(settings);
    let fallback_label = structured_fallback_format_label(fallback_format);

    let mut actions_log: Vec<Value> = Vec::new();
    let mut untagged_candidates: Vec<(String, bool)> = Vec::new();
    let initial_memory_count = session.memory_embeddings.len();
    let delete_confidence_default = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.dynamic_memory.as_ref())
        .map(|dynamic| dynamic.delete_confidence_default)
        .unwrap_or(0.5);
    let max_hard_delete_ratio = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.dynamic_memory.as_ref())
        .map(|dynamic| dynamic.max_hard_delete_ratio_per_cycle)
        .unwrap_or(0.5);
    let max_hard_deletes = max_hard_deletes_per_cycle(initial_memory_count, max_hard_delete_ratio);
    let mut hard_delete_count = 0usize;
    let recursive_loops_enabled = recursive_memory_loops_enabled(dynamic_settings);
    let max_loop_iterations = if recursive_loops_enabled {
        dynamic_settings.recursive_memory_loop_hard_cap.max(1) as usize
    } else {
        1
    };
    log_info(
        app,
        "dynamic_memory",
        format!(
            "memory tool loop configured recursive={} hard_cap={} initial_memories={} provider={} model={}",
            recursive_loops_enabled,
            max_loop_iterations,
            session.memory_embeddings.len(),
            provider_cred.provider_id,
            model.name
        ),
    );

    for iteration in 0..max_loop_iterations {
        if cancel_token.is_some_and(|token| token.is_cancelled()) {
            return Err("Request was cancelled by user".to_string());
        }

        let iteration_request_id = request_id.map(|id| {
            if recursive_loops_enabled {
                format!("{}:loop-{}", id, iteration + 1)
            } else {
                id.to_string()
            }
        });
        log_info(
            app,
            "dynamic_memory",
            format!(
                "memory tool loop iteration {}/{} requesting tool calls current_memories={} request_id={}",
                iteration + 1,
                max_loop_iterations,
                session.memory_embeddings.len(),
                iteration_request_id.as_deref().unwrap_or("none")
            ),
        );

        let (calls, call_source) = request_memory_tool_calls(
            app,
            provider_cred,
            model,
            overwrite_llama_sampler_config,
            api_key,
            &messages_for_api,
            request_settings.max_tokens,
            request_settings.context_length,
            extra_body_fields.clone(),
            &tool_config,
            fallback_format,
            fallback_label,
            &context,
            session,
            character,
            debug_capture_enabled,
            debug_steps,
            iteration_request_id.as_deref(),
            cancel_token,
        )
        .await?;

        log_raw_memory_tool_calls(app, call_source, &calls);
        push_memory_debug_step(
            debug_steps,
            debug_capture_enabled,
            json!({
                "phase": "memory_tool_iteration_calls",
                "iteration": iteration + 1,
                "requestId": iteration_request_id,
                "source": call_source,
                "calls": calls,
            }),
        );

        if calls.is_empty() {
            log_warn(
                app,
                "dynamic_memory",
                format!(
                    "memory tool loop iteration {} returned no tool calls; stopping",
                    iteration + 1
                ),
            );
            break;
        }

        let tool_calls_json: Vec<Value> = calls
            .iter()
            .enumerate()
            .map(|(index, call)| memory_tool_call_payload(&provider_cred.provider_id, call, index))
            .collect();
        let mut tool_results: Vec<Value> = Vec::new();
        let mut saw_done = false;

        for call in calls {
        match call.name.as_str() {
            "create_memory" => {
                if let Some(raw_text) = extract_text_argument(&call) {
                    let text = match validate_memory_text(&raw_text) {
                        Ok(text) => text,
                        Err(reason) => {
                            log_warn(
                                app,
                                "dynamic_memory",
                                format!("Skipping invalid memory text: {}", reason),
                            );
                            actions_log.push(json!({
                                "name": "create_memory",
                                "arguments": call.arguments,
                                "skipped": true,
                                "reason": reason,
                                "timestamp": now_millis().unwrap_or_default(),
                            }));
                            tool_results.push(json!({
                                "status": "skipped",
                                "name": "create_memory",
                                "reason": reason,
                                "arguments": call.arguments,
                            }));
                            continue;
                        }
                    };
                    let mem_id = generate_memory_id();
                    let embedding =
                        match embedding::compute_embedding(app.clone(), text.clone()).await {
                            Ok(vec) => Some(vec),
                            Err(err) => {
                                log_error(
                                    app,
                                    "dynamic_memory",
                                    format!("failed to embed memory: {}", err),
                                );
                                None
                            }
                        };
                    if let Some(reason) = find_duplicate_memory_reason(
                        &text,
                        embedding.as_deref(),
                        &session.memory_embeddings,
                    ) {
                        log_info(
                            app,
                            "dynamic_memory",
                            format!("Skipping duplicate memory ({}): {}", reason, &text),
                        );
                        actions_log.push(json!({
                            "name": "create_memory",
                            "arguments": call.arguments,
                            "skipped": true,
                            "reason": reason,
                            "timestamp": now_millis().unwrap_or_default(),
                        }));
                        tool_results.push(json!({
                            "status": "skipped",
                            "name": "create_memory",
                            "reason": reason,
                            "arguments": call.arguments,
                        }));
                        continue;
                    }
                    let token_count =
                        crate::embedding::tokenizer::count_tokens(app, &text).unwrap_or(0);
                    // Check if memory should be pinned
                    let is_pinned = call
                        .arguments
                        .get("important")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let category = match extract_required_memory_category(&call) {
                        Ok(category) => category,
                        Err(reason) => {
                            log_warn(
                                app,
                                "dynamic_memory",
                                format!("Skipping memory without required category: {}", reason),
                            );
                            actions_log.push(json!({
                                "name": "create_memory",
                                "arguments": call.arguments,
                                "skipped": true,
                                "reason": reason,
                                "timestamp": now_millis().unwrap_or_default(),
                            }));
                            log_info(
                                app,
                                "dynamic_memory",
                                format!(
                                    "Queued memory for category repair: text=\"{}\" pinned={}",
                                    text, is_pinned
                                ),
                            );
                            untagged_candidates.push((text, is_pinned));
                            tool_results.push(json!({
                                "status": "skipped",
                                "name": "create_memory",
                                "reason": reason,
                                "repairQueued": true,
                                "arguments": call.arguments,
                            }));
                            continue;
                        }
                    };
                    session.memory_embeddings.push(MemoryEmbedding {
                        id: mem_id.clone(),
                        text,
                        embedding: embedding.unwrap_or_default(),
                        created_at: now_millis().unwrap_or_default(),
                        token_count,
                        is_cold: false,
                        last_accessed_at: now_millis().unwrap_or_default(),
                        importance_score: 1.0,
                        is_pinned,
                        access_count: 0,
                        match_score: None,
                        category: Some(category),
                    });
                    let action = json!({
                        "name": "create_memory",
                        "arguments": call.arguments,
                        "memoryId": mem_id,
                        "timestamp": now_millis().unwrap_or_default(),
                        "updatedMemories": format_memories_with_ids(session),
                    });
                    tool_results.push(json!({
                        "status": "created",
                        "name": "create_memory",
                        "memoryId": action.get("memoryId").cloned().unwrap_or(Value::Null),
                        "updatedMemories": action.get("updatedMemories").cloned().unwrap_or(Value::Null),
                    }));
                    actions_log.push(action);
                }
            }
            "delete_memory" => {
                if let Some(text) = call.arguments.get("text").and_then(|v| v.as_str()) {
                    let sanitized = sanitize_memory_id(text);
                    let target_idx =
                        if sanitized.len() == 6 && sanitized.chars().all(char::is_numeric) {
                            session
                                .memory_embeddings
                                .iter()
                                .position(|m| m.id == sanitized)
                        } else {
                            session
                                .memory_embeddings
                                .iter()
                                .position(|m| m.text == text)
                        };
                    if let Some(idx) = target_idx {
                        let target_memory = session.memory_embeddings.get(idx).cloned();
                        let confidence = call
                            .arguments
                            .get("confidence")
                            .and_then(|v| v.as_f64())
                            .unwrap_or(delete_confidence_default as f64)
                            as f32;
                        let confidence_defaulted = call
                            .arguments
                            .get("confidence")
                            .and_then(|v| v.as_f64())
                            .is_none();
                        let force_soft_delete = confidence >= HARD_DELETE_CONFIDENCE_THRESHOLD
                            && hard_delete_count >= max_hard_deletes;
                        if confidence < HARD_DELETE_CONFIDENCE_THRESHOLD || force_soft_delete {
                            // Soft-delete: move to cold storage instead of removing
                            if idx < session.memory_embeddings.len() {
                                let cold_threshold = dynamic_cold_threshold(settings);
                                session.memory_embeddings[idx].is_cold = true;
                                session.memory_embeddings[idx].importance_score = cold_threshold;
                                log_info(
                                    app,
                                    "dynamic_memory",
                                    if force_soft_delete {
                                        format!(
                                            "Soft-deleted memory due to hard-delete safeguard (hard_deletes={}/{}, confidence={:.2})",
                                            hard_delete_count,
                                            max_hard_deletes,
                                            confidence
                                        )
                                    } else {
                                        format!(
                                            "Soft-deleted memory (confidence={:.2}, defaulted={})",
                                            confidence, confidence_defaulted
                                        )
                                    },
                                );
                            }
                            actions_log.push(json!({
                                "name": "delete_memory",
                                "arguments": call.arguments,
                                "deletedText": target_memory.as_ref().map(|m| m.text.clone()),
                                "deletedMemoryId": target_memory.as_ref().map(|m| m.id.clone()),
                                "memorySnapshot": target_memory,
                                "softDelete": true,
                                "reason": if force_soft_delete {
                                    "hard_delete_limit_reached"
                                } else {
                                    "low_confidence"
                                },
                                "confidence": confidence,
                                "confidenceDefaulted": confidence_defaulted,
                                "hardDeleteCount": hard_delete_count,
                                "hardDeleteLimit": max_hard_deletes,
                                "timestamp": now_millis().unwrap_or_default(),
                                "updatedMemories": format_memories_with_ids(session),
                            }));
                            tool_results.push(json!({
                                "status": "soft_deleted",
                                "name": "delete_memory",
                                "deletedMemoryId": target_memory.as_ref().map(|m| m.id.clone()),
                                "deletedText": target_memory.as_ref().map(|m| m.text.clone()),
                                "updatedMemories": format_memories_with_ids(session),
                            }));
                        } else {
                            let removed_memory = if idx < session.memory_embeddings.len() {
                                Some(session.memory_embeddings.remove(idx))
                            } else {
                                None
                            };
                            hard_delete_count += 1;
                            actions_log.push(json!({
                                "name": "delete_memory",
                                "arguments": call.arguments,
                                "deletedText": removed_memory.as_ref().map(|m| m.text.clone()),
                                "deletedMemoryId": removed_memory.as_ref().map(|m| m.id.clone()),
                                "memorySnapshot": removed_memory,
                                "confidence": confidence,
                                "confidenceDefaulted": confidence_defaulted,
                                "hardDeleteCount": hard_delete_count,
                                "hardDeleteLimit": max_hard_deletes,
                                "timestamp": now_millis().unwrap_or_default(),
                                "updatedMemories": format_memories_with_ids(session),
                            }));
                            tool_results.push(json!({
                                "status": "deleted",
                                "name": "delete_memory",
                                "deletedMemoryId": removed_memory.as_ref().map(|m| m.id.clone()),
                                "deletedText": removed_memory.as_ref().map(|m| m.text.clone()),
                                "updatedMemories": format_memories_with_ids(session),
                            }));
                        }
                    } else {
                        log_warn(
                            app,
                            "dynamic_memory",
                            format!("delete_memory could not find target: {}", text),
                        );
                        tool_results.push(json!({
                            "status": "skipped",
                            "name": "delete_memory",
                            "reason": "target_not_found",
                            "arguments": call.arguments,
                        }));
                    }
                }
            }
            "pin_memory" => {
                if let Some(raw_id) = call.arguments.get("id").and_then(|v| v.as_str()) {
                    let id = sanitize_memory_id(raw_id);
                    if let Some(mem) = session.memory_embeddings.iter_mut().find(|m| m.id == id) {
                        mem.is_pinned = true;
                        mem.importance_score = 1.0; // Reset score when pinned
                        actions_log.push(json!({
                            "name": "pin_memory",
                            "arguments": call.arguments,
                            "timestamp": now_millis().unwrap_or_default(),
                        }));
                        tool_results.push(json!({
                            "status": "pinned",
                            "name": "pin_memory",
                            "memoryId": id,
                        }));
                        log_info(app, "dynamic_memory", format!("Pinned memory {}", id));
                    } else {
                        log_warn(
                            app,
                            "dynamic_memory",
                            format!("pin_memory could not find: {}", id),
                        );
                        tool_results.push(json!({
                            "status": "skipped",
                            "name": "pin_memory",
                            "reason": "target_not_found",
                            "arguments": call.arguments,
                        }));
                    }
                }
            }
            "unpin_memory" => {
                if let Some(raw_id) = call.arguments.get("id").and_then(|v| v.as_str()) {
                    let id = sanitize_memory_id(raw_id);
                    if let Some(mem) = session.memory_embeddings.iter_mut().find(|m| m.id == id) {
                        mem.is_pinned = false;
                        actions_log.push(json!({
                            "name": "unpin_memory",
                            "arguments": call.arguments,
                            "timestamp": now_millis().unwrap_or_default(),
                        }));
                        tool_results.push(json!({
                            "status": "unpinned",
                            "name": "unpin_memory",
                            "memoryId": id,
                        }));
                        log_info(app, "dynamic_memory", format!("Unpinned memory {}", id));
                    } else {
                        log_warn(
                            app,
                            "dynamic_memory",
                            format!("unpin_memory could not find: {}", id),
                        );
                        tool_results.push(json!({
                            "status": "skipped",
                            "name": "unpin_memory",
                            "reason": "target_not_found",
                            "arguments": call.arguments,
                        }));
                    }
                }
            }
            "done" => {
                actions_log.push(json!({
                    "name": "done",
                    "arguments": call.arguments,
                    "timestamp": now_millis().unwrap_or_default(),
                }));
                saw_done = true;
                break;
            }
            _ => {
                tool_results.push(json!({
                    "status": "skipped",
                    "name": call.name,
                    "reason": "unsupported_tool",
                    "arguments": call.arguments,
                }));
            }
        }
    }

        let skipped_results = tool_results
            .iter()
            .filter(|result| result.get("status").and_then(Value::as_str) == Some("skipped"))
            .count();
        log_info(
            app,
            "dynamic_memory",
            format!(
                "memory tool loop iteration {}/{} applied calls={} tool_results={} skipped_results={} memories_now={} saw_done={}",
                iteration + 1,
                max_loop_iterations,
                tool_calls_json.len(),
                tool_results.len(),
                skipped_results,
                session.memory_embeddings.len(),
                saw_done
            ),
        );
        push_memory_debug_step(
            debug_steps,
            debug_capture_enabled,
            json!({
                "phase": "memory_tool_iteration_applied",
                "iteration": iteration + 1,
                "toolCalls": tool_calls_json,
                "toolResults": tool_results,
                "actions": actions_log,
                "memoriesNow": session.memory_embeddings.len(),
                "sawDone": saw_done,
            }),
        );

        if saw_done {
            log_info(
                app,
                "dynamic_memory",
                format!(
                    "memory tool loop iteration {}/{} received done; stopping recursive loop",
                    iteration + 1,
                    max_loop_iterations
                ),
            );
            break;
        }

        if !recursive_loops_enabled {
            log_info(
                app,
                "dynamic_memory",
                "memory tool loop recursive mode disabled; stopping after single pass",
            );
            break;
        }

        if tool_results.is_empty() {
            log_warn(
                app,
                "dynamic_memory",
                format!(
                    "memory tool loop iteration {} produced no executable tool results; stopping",
                    iteration + 1
                ),
            );
            break;
        }

        messages_for_api.push(json!({
            "role": "assistant",
            "content": Value::Null,
            "tool_calls": tool_calls_json,
        }));
        for (call, result) in tool_calls_json.iter().zip(tool_results.iter()) {
            let tool_call_id = call.get("id").and_then(Value::as_str).unwrap_or_default();
            let tool_name = call
                .get("function")
                .and_then(|value| value.get("name"))
                .and_then(Value::as_str);
            messages_for_api.push(memory_tool_result_message(
                &provider_cred.provider_id,
                tool_call_id,
                tool_name,
                result,
            ));
        }

        if iteration + 1 == max_loop_iterations {
            log_warn(
                app,
                "dynamic_memory",
                format!(
                    "recursive memory loops reached hard cap of {}; stopping without done",
                    max_loop_iterations
                ),
            );
        }
    }

    if !untagged_candidates.is_empty() {
        let mut seen = HashSet::new();
        let candidate_texts: Vec<String> = untagged_candidates
            .iter()
            .map(|(text, _)| text.clone())
            .filter(|text| seen.insert(text.clone()))
            .collect();

        log_info(
            app,
            "dynamic_memory",
            format!(
                "Running memory category repair for {} candidate(s)",
                candidate_texts.len()
            ),
        );

        match run_memory_tag_repair(
            app,
            provider_cred,
            model,
            overwrite_llama_sampler_config,
            api_key,
            &candidate_texts,
            fallback_format,
        )
        .await
        {
            Ok(repaired) => {
                log_info(
                    app,
                    "dynamic_memory",
                    format!(
                        "Memory category repair returned {} mapped candidate(s)",
                        repaired.len()
                    ),
                );
                for (text, is_pinned) in untagged_candidates {
                    let Some(category) = repaired.get(&text).cloned() else {
                        log_warn(
                            app,
                            "dynamic_memory",
                            format!(
                                "Memory category repair returned no category for text=\"{}\"",
                                text
                            ),
                        );
                        continue;
                    };

                    let text = match validate_memory_text(&text) {
                        Ok(text) => text,
                        Err(reason) => {
                            actions_log.push(json!({
                                "name": "create_memory",
                                "repaired": true,
                                "arguments": {
                                    "text": text,
                                    "category": category,
                                    "important": is_pinned,
                                },
                                "skipped": true,
                                "reason": reason,
                                "timestamp": now_millis().unwrap_or_default(),
                            }));
                            continue;
                        }
                    };

                    let mem_id = generate_memory_id();
                    let embedding =
                        match embedding::compute_embedding(app.clone(), text.clone()).await {
                            Ok(vec) => Some(vec),
                            Err(err) => {
                                log_error(
                                    app,
                                    "dynamic_memory",
                                    format!("failed to embed repaired memory: {}", err),
                                );
                                None
                            }
                        };
                    if let Some(reason) = find_duplicate_memory_reason(
                        &text,
                        embedding.as_deref(),
                        &session.memory_embeddings,
                    ) {
                        actions_log.push(json!({
                            "name": "create_memory",
                            "repaired": true,
                            "arguments": {
                                "text": text,
                                "category": category,
                                "important": is_pinned,
                            },
                            "skipped": true,
                            "reason": reason,
                            "timestamp": now_millis().unwrap_or_default(),
                        }));
                        continue;
                    }
                    let token_count =
                        crate::embedding::tokenizer::count_tokens(app, &text).unwrap_or(0);
                    session.memory_embeddings.push(MemoryEmbedding {
                        id: mem_id.clone(),
                        text: text.clone(),
                        embedding: embedding.unwrap_or_default(),
                        created_at: now_millis().unwrap_or_default(),
                        token_count,
                        is_cold: false,
                        last_accessed_at: now_millis().unwrap_or_default(),
                        importance_score: 1.0,
                        is_pinned,
                        access_count: 0,
                        match_score: None,
                        category: Some(category.clone()),
                    });
                    actions_log.push(json!({
                        "name": "create_memory",
                        "repaired": true,
                        "arguments": {
                            "text": text,
                            "category": category,
                            "important": is_pinned,
                        },
                        "memoryId": mem_id,
                        "timestamp": now_millis().unwrap_or_default(),
                        "updatedMemories": format_memories_with_ids(session),
                    }));
                    log_info(
                        app,
                        "dynamic_memory",
                        format!(
                            "Created repaired memory {} category={} pinned={}",
                            mem_id, category, is_pinned
                        ),
                    );
                }
            }
            Err(err) => {
                log_warn(
                    app,
                    "dynamic_memory",
                    format!("memory category repair pass failed: {}", err),
                );
            }
        }
    }

    let trimmed = trim_memories_to_max(&mut session.memory_embeddings, max_entries);
    if trimmed > 0 {
        log_info(
            app,
            "dynamic_memory",
            format!(
                "Trimmed {} memories to enforce max_entries={}",
                trimmed, max_entries
            ),
        );
    }
    if session.memory_embeddings.len() > max_entries {
        log_warn(
            app,
            "dynamic_memory",
            format!(
                "Pinned memories exceed max_entries (count={}, max={})",
                session.memory_embeddings.len(),
                max_entries
            ),
        );
    }

    // Enforce token budget - demote oldest memories to cold storage if over budget
    let token_budget = dynamic_hot_memory_token_budget(settings);
    let demoted = enforce_hot_memory_budget(&mut session.memory_embeddings, token_budget);
    if !demoted.is_empty() {
        log_info(
            app,
            "dynamic_memory",
            format!(
                "Demoted {} memories to cold storage (budget: {} tokens)",
                demoted.len(),
                token_budget
            ),
        );
    }

    session.memories = session
        .memory_embeddings
        .iter()
        .map(|m| m.text.clone())
        .collect();

    session.updated_at = now_millis()?;
    save_session(app, session)?;
    Ok(actions_log)
}

fn extract_text_argument(call: &ToolCall) -> Option<String> {
    if let Some(text) = call
        .arguments
        .get("text")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
    {
        return Some(text);
    }
    call.raw_arguments.clone()
}

fn extract_required_memory_category(call: &ToolCall) -> Result<String, String> {
    let category = call
        .arguments
        .get("category")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "missing required category".to_string())?;

    if !ALLOWED_MEMORY_CATEGORIES.contains(&category.as_str()) {
        return Err(format!(
            "invalid category '{}'; expected one of: {}",
            category,
            ALLOWED_MEMORY_CATEGORIES.join(", ")
        ));
    }

    Ok(category)
}

fn build_memory_tag_repair_tool_config() -> ToolConfig {
    ToolConfig {
        tools: vec![ToolDefinition {
            name: "retag_memory".to_string(),
            description: Some("Assign a valid category for each memory text.".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "Original memory text to categorize" },
                    "category": {
                        "type": "string",
                        "enum": ["character_trait", "relationship", "plot_event", "world_detail", "preference", "other"],
                        "description": "Category tag for the memory"
                    }
                },
                "required": ["text", "category"]
            }),
        }],
        choice: Some(ToolChoice::Any),
    }
}

async fn run_memory_tag_repair(
    app: &AppHandle,
    provider_cred: &ProviderCredential,
    model: &Model,
    overwrite_llama_sampler_config: bool,
    api_key: &str,
    texts: &[String],
    fallback_format: crate::chat_manager::types::DynamicMemoryStructuredFallbackFormat,
) -> Result<HashMap<String, String>, String> {
    if texts.is_empty() {
        return Ok(HashMap::new());
    }

    let mut messages_for_api = Vec::new();
    let system_role = request_builder::system_role_for(provider_cred);
    crate::chat_manager::messages::push_system_message(
        &mut messages_for_api,
        &system_role,
        Some(
            "Classify each memory text with exactly one valid category. Use only retag_memory tool calls."
                .to_string(),
        ),
    );
    messages_for_api.push(json!({
        "role": "user",
        "content": format!(
            "Valid categories: {}.\nReturn one retag_memory tool call per text.\nTexts:\n{}",
            ALLOWED_MEMORY_CATEGORIES.join(", "),
            texts
                .iter()
                .enumerate()
                .map(|(i, t)| format!("{}. {}", i + 1, t))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }));

    let mut repaired = HashMap::new();
    let fallback_label = structured_fallback_format_label(fallback_format);
    match send_dynamic_memory_request(
        app,
        provider_cred,
        model,
        overwrite_llama_sampler_config,
        api_key,
        &messages_for_api,
        512,
        None,
        None,
        Some(&build_memory_tag_repair_tool_config()),
        None,
        None,
    )
    .await
    {
        Ok(api_response) if api_response.ok => {
            for call in parse_tool_calls(&provider_cred.provider_id, api_response.data()) {
                if call.name != "retag_memory" {
                    continue;
                }
                let Some(text) = call
                    .arguments
                    .get("text")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                else {
                    continue;
                };
                if let Ok(category) = extract_required_memory_category(&call) {
                    repaired.insert(text, category);
                }
            }
        }
        Ok(api_response) => {
            let fallback = format!("Provider returned status {}", api_response.status);
            let err_message = extract_error_message(api_response.data()).unwrap_or(fallback);
            log_warn(
                app,
                "dynamic_memory",
                format!(
                    "memory tag repair tool request failed; retrying with {} fallback: {}",
                    fallback_label, err_message
                ),
            );
        }
        Err(err) => {
            log_warn(
                app,
                "dynamic_memory",
                format!(
                    "memory tag repair tool request errored; retrying with {} fallback: {}",
                    fallback_label, err
                ),
            );
        }
    }

    if repaired.is_empty() {
        let mut fallback_messages = messages_for_api.clone();
        fallback_messages.push(json!({
            "role": "user",
            "content": memory_repairs_fallback_prompt(fallback_format)
        }));
        match send_dynamic_memory_request(
            app,
            provider_cred,
            model,
            overwrite_llama_sampler_config,
            api_key,
            &fallback_messages,
            512,
            None,
            None,
            None,
            None,
            None,
        )
        .await
        {
            Ok(api_response) if api_response.ok => {
                if let Some(text) =
                    extract_text(api_response.data(), Some(&provider_cred.provider_id))
                {
                    if let Ok(parsed) = parse_memory_tag_repairs_from_text(
                        &text,
                        ALLOWED_MEMORY_CATEGORIES,
                        fallback_format,
                    ) {
                        repaired.extend(parsed);
                    }
                }
            }
            Ok(api_response) => {
                let fallback = format!("Provider returned status {}", api_response.status);
                let err_message = extract_error_message(api_response.data()).unwrap_or(fallback);
                log_warn(
                    app,
                    "dynamic_memory",
                    format!(
                        "memory tag repair {} fallback failed: {}",
                        fallback_label, err_message
                    ),
                );
            }
            Err(err) => {
                log_warn(
                    app,
                    "dynamic_memory",
                    format!(
                        "memory tag repair {} fallback errored: {}",
                        fallback_label, err
                    ),
                );
            }
        }
    }

    if repaired.is_empty() {
        for text in texts {
            repaired.insert(text.clone(), guess_memory_category(text));
        }
    }

    Ok(repaired)
}

fn build_memory_tool_config() -> ToolConfig {
    ToolConfig {
        tools: vec![
            ToolDefinition {
                name: "create_memory".to_string(),
                description: Some(
                    "Create a concise memory entry capturing important facts.".to_string(),
                ),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "text": { "type": "string", "description": "Concise memory to store" },
                        "important": { "type": "boolean", "description": "If true, memory will be pinned (never decays)" },
                        "category": {
                            "type": "string",
                            "enum": ["character_trait", "relationship", "plot_event", "world_detail", "preference", "other"],
                            "description": "Category of this memory for organization"
                        }
                    },
                    "required": ["text", "category"]
                }),
            },
            ToolDefinition {
                name: "delete_memory".to_string(),
                description: Some(
                    "Delete an outdated or redundant memory. Low confidence (< 0.7) triggers soft-delete to cold storage.".to_string(),
                ),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "text": { "type": "string", "description": "Memory ID (preferred) or exact text to remove" },
                        "confidence": { "type": "number", "description": "Confidence that this memory should be deleted (0.0-1.0). Below 0.7 triggers soft-delete to cold storage." }
                    },
                    "required": ["text"]
                }),
            },
            ToolDefinition {
                name: "pin_memory".to_string(),
                description: Some(
                    "Pin a critical memory so it never decays. Use for character-defining facts."
                        .to_string(),
                ),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "description": "6-digit memory ID to pin" }
                    },
                    "required": ["id"]
                }),
            },
            ToolDefinition {
                name: "unpin_memory".to_string(),
                description: Some("Unpin a memory, allowing it to decay normally.".to_string()),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "description": "6-digit memory ID to unpin" }
                    },
                    "required": ["id"]
                }),
            },
            ToolDefinition {
                name: "done".to_string(),
                description: Some(
                    "Call this when you have finished adding or deleting memories.".to_string(),
                ),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "summary": { "type": "string", "description": "Optional short note of changes made" }
                    },
                    "required": []
                }),
            },
        ],
        choice: Some(ToolChoice::Any),
    }
}

fn summarization_tool_config() -> ToolConfig {
    ToolConfig {
        tools: vec![ToolDefinition {
            name: "write_summary".to_string(),
            description: Some(
                "Return a concise summary of the provided conversation window.".to_string(),
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "summary": { "type": "string", "description": "Concise summary text" }
                },
                "required": ["summary"]
            }),
        }],
        choice: Some(ToolChoice::Required),
    }
}

async fn summarize_messages(
    app: &AppHandle,
    provider_cred: &ProviderCredential,
    model: &Model,
    api_key: &str,
    convo_window: &[StoredMessage],
    prior_summary: Option<&str>,
    character: &Character,
    session: &Session,
    settings: &Settings,
    persona: Option<&Persona>,
    debug_capture_enabled: bool,
    debug_steps: &mut Vec<Value>,
    request_id: Option<&str>,
    cancel_token: Option<&DynamicMemoryCancellationToken>,
) -> Result<String, String> {
    let overwrite_llama_sampler_config = dynamic_memory_llama_sampler_overwrite_enabled(settings);
    let mut messages_for_api = Vec::new();
    let system_role = request_builder::system_role_for(provider_cred);

    let summary_template = prompts::get_template(app, APP_DYNAMIC_SUMMARY_TEMPLATE_ID)
        .ok()
        .flatten()
        .map(|t| t.content)
        .unwrap_or_else(|| {
            "Summarize the recent conversation transcript into a concise paragraph capturing durable facts and decisions. Avoid adding new information.".to_string()
        });

    let mut rendered = prompt_engine::render_with_context(
        app,
        &summary_template,
        character,
        persona,
        session,
        settings,
    );
    let prev_text = prior_summary
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("No previous summary provided.");
    rendered = rendered.replace("{{prev_summary}}", prev_text);
    crate::chat_manager::messages::push_system_message(
        &mut messages_for_api,
        &system_role,
        Some(rendered),
    );
    for msg in convo_window {
        messages_for_api.push(json!({
            "role": msg.role,
            "content": msg.content
        }));
    }

    messages_for_api.push(json!({
        "role": "user",
        "content": "Return only the concise summary for the above conversation window. Use the write_summary tool."
    }));

    let (request_settings, extra_body_fields) = prepare_default_sampling_request(
        &provider_cred.provider_id,
        session,
        model,
        settings,
        0.2,
        1.0,
        None,
        None,
        None,
    );
    let context = ChatContext::initialize(app.clone())?;
    let tool_attempt = send_dynamic_memory_request(
        app,
        provider_cred,
        model,
        overwrite_llama_sampler_config,
        api_key,
        &messages_for_api,
        request_settings.max_tokens,
        request_settings.context_length,
        extra_body_fields.clone(),
        Some(&summarization_tool_config()),
        request_id,
        cancel_token,
    )
    .await;

    let tool_failure_reason = match tool_attempt {
        Ok(api_response) => {
            push_memory_debug_step(
                debug_steps,
                debug_capture_enabled,
                json!({
                    "phase": "summary_tool_attempt",
                    "requestId": request_id,
                    "providerId": provider_cred.provider_id,
                    "model": model.name,
                    "response": {
                        "ok": api_response.ok,
                        "status": api_response.status,
                        "data": api_response.data().clone(),
                    }
                }),
            );
            let usage = extract_usage(api_response.data());
            record_usage_if_available(
                &context,
                &usage,
                session,
                character,
                model,
                provider_cred,
                api_key,
                now_millis().unwrap_or(0),
                UsageOperationType::Summary,
                "dynamic_summary",
            )
            .await;

            if api_response.ok {
                let calls = parse_tool_calls(&provider_cred.provider_id, api_response.data());
                for call in calls.iter() {
                    if call.name != "write_summary" {
                        continue;
                    }
                    if let Some(summary) = call.arguments.get("summary").and_then(|v| v.as_str()) {
                        if let Ok(validated) = validate_summary_text(summary) {
                            return Ok(validated);
                        }
                    }
                }

                if let Some(text) =
                    extract_text(api_response.data(), Some(&provider_cred.provider_id))
                        .filter(|s| !s.is_empty())
                {
                    if let Ok(validated) = validate_summary_text(&text) {
                        return Ok(validated);
                    }
                }

                if calls.is_empty() {
                    let legacy_hint = if payload_contains_function_call(api_response.data()) {
                        " (response uses legacy function_call format)"
                    } else {
                        ""
                    };
                    log_warn(
                        app,
                        "dynamic_memory",
                        format!(
                            "summary tool response preview: {}",
                            response_preview(&provider_cred.provider_id, api_response.data())
                        ),
                    );
                    format!(
                        "model returned no tool call and no valid text{}. Provider={}, model={}",
                        legacy_hint, provider_cred.provider_id, model.name
                    )
                } else {
                    let tool_names = calls
                        .iter()
                        .map(|c| c.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!(
                        "expected write_summary tool call or valid text, got {}. Provider={}, model={}",
                        tool_names, provider_cred.provider_id, model.name
                    )
                }
            } else {
                let fallback = format!("Provider returned status {}", api_response.status);
                let err_message =
                    extract_error_message(api_response.data()).unwrap_or(fallback.clone());
                if err_message == fallback {
                    err_message
                } else {
                    format!("{} (status {})", err_message, api_response.status)
                }
            }
        }
        Err(err) => {
            push_memory_debug_step(
                debug_steps,
                debug_capture_enabled,
                json!({
                    "phase": "summary_tool_attempt_error",
                    "requestId": request_id,
                    "providerId": provider_cred.provider_id,
                    "model": model.name,
                    "error": err,
                }),
            );
            if is_cancelled_request_error(&err) {
                return Err(err);
            }
            err
        }
    };

    log_warn(
        app,
        "dynamic_memory",
        format!(
            "summary tool request failed or was invalid; retrying with plain-text fallback: {}",
            tool_failure_reason
        ),
    );

    if cancel_token.is_some_and(|token| token.is_cancelled()) {
        return Err("Request was cancelled by user".to_string());
    }

    let mut fallback_messages = messages_for_api.clone();
    fallback_messages.push(json!({
        "role": "user",
        "content": "Return only the final merged summary as plain text. No tools, no JSON, no markdown, no commentary."
    }));

    let api_response = send_dynamic_memory_request(
        app,
        provider_cred,
        model,
        overwrite_llama_sampler_config,
        api_key,
        &fallback_messages,
        request_settings.max_tokens,
        request_settings.context_length,
        extra_body_fields,
        None,
        request_id,
        cancel_token,
    )
    .await?;

    push_memory_debug_step(
        debug_steps,
        debug_capture_enabled,
        json!({
            "phase": "summary_text_fallback",
            "requestId": request_id,
            "providerId": provider_cred.provider_id,
            "model": model.name,
            "response": {
                "ok": api_response.ok,
                "status": api_response.status,
                "data": api_response.data().clone(),
            }
        }),
    );

    let usage = extract_usage(api_response.data());
    record_usage_if_available(
        &context,
        &usage,
        session,
        character,
        model,
        provider_cred,
        api_key,
        now_millis().unwrap_or(0),
        UsageOperationType::Summary,
        "dynamic_summary_fallback",
    )
    .await;

    if !api_response.ok {
        let fallback = format!("Provider returned status {}", api_response.status);
        let err_message = extract_error_message(api_response.data()).unwrap_or(fallback.clone());
        return Err(if err_message == fallback {
            format!(
                "summary fallback failed after tool attempt '{}': {}",
                tool_failure_reason, err_message
            )
        } else {
            format!(
                "summary fallback failed after tool attempt '{}': {} (status {})",
                tool_failure_reason, err_message, api_response.status
            )
        });
    }

    let text =
        extract_text(api_response.data(), Some(&provider_cred.provider_id)).ok_or_else(|| {
            format!(
                "summary fallback returned no text after tool attempt '{}'",
                tool_failure_reason
            )
        })?;
    validate_summary_text(&text)
}

fn payload_contains_function_call(value: &Value) -> bool {
    match value {
        Value::Object(map) => {
            if map.contains_key("function_call") || map.contains_key("functionCall") {
                return true;
            }
            map.values().any(payload_contains_function_call)
        }
        Value::Array(items) => items.iter().any(payload_contains_function_call),
        _ => false,
    }
}
