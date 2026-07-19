//! Group Chat Manager
//!
//! This module handles group chat functionality including:
//! - Dynamic character selection based on context (via LLM tool calling)
//! - @mention parsing to force specific characters
//! - Building selection prompts with participation stats
//! - Coordinating with the chat_manager for actual response generation
//! - Full dynamic memory system support (decay, hot/cold, summarization, tool updates)

mod generation;
mod persistence;
pub mod selection;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

use rusqlite::OptionalExtension;

use crate::abort_manager::AbortRegistry;
use crate::api::{api_request, ApiRequest, ApiResponse};
use crate::chat_manager::service::{
    apply_openrouter_cost_to_usage, insert_extended_usage_metadata, require_api_key,
};
use crate::dynamic_memory_run_manager::{DynamicMemoryCancellationToken, DynamicMemoryRunManager};
use crate::usage::add_usage_record;
use crate::usage::tracking::{RequestUsage, UsageFinishReason, UsageOperationType};

use crate::chat_manager::execution::prepare_feature_request;
use crate::chat_manager::feature_generation::{
    feature_llama_sampler_override_present, feature_model_overrides, synthetic_feature_session,
    LlmFeature, DYNAMIC_MEMORY_MANAGER_DEFAULTS, GROUP_SPEAKER_SELECTION_DEFAULTS,
    HELP_ME_REPLY_DEFAULTS,
};
use crate::chat_manager::entries::{
    in_chat_system_entry, in_chat_user_entry, relative_system_entry,
};
use crate::chat_manager::lorebook_matcher::{
    activate_lorebook_entries, format_lorebook_for_prompt, get_active_lorebook_entries,
};
use crate::chat_manager::memory::dynamic::{
    apply_memory_decay, calculate_hot_memory_tokens, dynamic_memory_structured_fallback_format,
    effective_group_dynamic_memory_settings, enforce_hot_memory_budget, ensure_pinned_hot,
    find_duplicate_memory_reason, generate_memory_id, mark_memories_accessed, normalize_query_text,
    promote_cold_memories, search_cold_memory_indices_by_keyword, select_relevant_memory_indices,
    select_top_cosine_memory_indices, trim_memories_to_max,
};
use crate::chat_manager::memory::manual::{has_manual_memories, render_manual_memory_lines};
use crate::chat_manager::memory::structured_fallback::{
    memory_operations_fallback_prompt, memory_repairs_fallback_prompt,
    parse_memory_operations_from_text, parse_memory_tag_repairs_from_text,
    structured_fallback_format_label,
};
use crate::chat_manager::prompting::entry_conditions::{
    entry_is_active, PromptEntryConditionContext,
};
use crate::chat_manager::prompts::{
    self, APP_DYNAMIC_MEMORY_LOCAL_TEMPLATE_ID, APP_DYNAMIC_MEMORY_TEMPLATE_ID,
    APP_DYNAMIC_SUMMARY_TEMPLATE_ID,
};
use crate::chat_manager::request::{extract_error_message, extract_text, extract_usage};
use crate::chat_manager::storage::{
    load_personas, load_settings, resolve_credential_for_model, select_model_with_credential,
};
use crate::chat_manager::thinking::normalize_thinking_content;
use crate::chat_manager::tooling::{
    parse_tool_calls, ToolCall, ToolChoice, ToolConfig, ToolDefinition,
};
use crate::chat_manager::turn_builder::assemble_prompt_messages;
use crate::chat_manager::types::{
    Character, DynamicMemorySettings, MemoryRetrievalStrategy, Model, Persona, PromptEntryChatMode,
    PromptEntryInfoSource, PromptEntryPosition, PromptEntryRole, ProviderCredential, Settings,
    SystemPromptEntry,
};
use crate::embedding;
use crate::storage_manager::db::{now_ms, DbConnection, SwappablePool};
use crate::storage_manager::group_sessions::{
    self, group_session_update_memories_internal, GroupMessage, GroupParticipation, GroupSession,
    ImageAttachment, MemoryEmbedding, UsageSummary,
};
use crate::storage_manager::lorebook::{
    get_enabled_lorebook_entry_contexts_for_ids, get_lorebook, LorebookEntry,
};
use crate::utils::{log_error, log_info, log_warn, now_millis};

pub use selection::parse_mentions;

const ALLOWED_MEMORY_CATEGORIES: &[&str] = &[
    "character_trait",
    "relationship",
    "plot_event",
    "world_detail",
    "preference",
    "other",
];
const HARD_DELETE_CONFIDENCE_THRESHOLD: f32 = 0.7;
const MEMORY_MIGRATION_EMBED_TIMEOUT_SECS: u64 = 90;

fn dynamic_memory_summary_template_id(settings: &Settings) -> String {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| {
            advanced
                .dynamic_memory_summarizer_prompt_template_id
                .clone()
        })
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| APP_DYNAMIC_SUMMARY_TEMPLATE_ID.to_string())
}

fn dynamic_memory_manager_template_id(
    settings: &Settings,
    provider_cred: &ProviderCredential,
    model: &Model,
) -> String {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.dynamic_memory_manager_prompt_template_id.clone())
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| {
            if uses_local_dynamic_memory_model(provider_cred, model) {
                APP_DYNAMIC_MEMORY_LOCAL_TEMPLATE_ID.to_string()
            } else {
                APP_DYNAMIC_MEMORY_TEMPLATE_ID.to_string()
            }
        })
}

fn resolve_group_dynamic_memory_summarisation_model_id(
    app: &AppHandle,
    settings: &Settings,
) -> Result<String, String> {
    if let Some(id) = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.summarisation_model_id.as_ref())
        .filter(|id| !id.trim().is_empty())
    {
        return Ok(id.clone());
    }

    if let Some(id) = settings
        .default_model_id
        .as_ref()
        .filter(|id| !id.trim().is_empty())
    {
        log_info(
            app,
            "group_dynamic_memory",
            format!(
                "summarisation model not set; falling back to app default model: {}",
                id
            ),
        );
        return Ok(id.clone());
    }

    Err("Summarisation model not configured".to_string())
}

fn max_hard_deletes_per_cycle(initial_count: usize, ratio: f32) -> usize {
    if initial_count == 0 {
        return 0;
    }

    ((initial_count as f32) * ratio).floor().max(1.0) as usize
}

fn log_raw_group_memory_tool_calls(app: &AppHandle, source: &str, calls: &[ToolCall]) {
    let payload = serde_json::to_string(calls).unwrap_or_else(|_| "<serialize failed>".to_string());
    log_info(
        app,
        "group_dynamic_memory",
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

fn push_group_memory_debug_step(debug_steps: &mut Vec<Value>, enabled: bool, step: Value) {
    if enabled {
        debug_steps.push(step);
    }
}

fn dynamic_memory_llama_sampler_overwrite_enabled(settings: &Settings, model: &Model) -> bool {
    if feature_llama_sampler_override_present(model, LlmFeature::DynamicMemory) {
        return false;
    }
    settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.dynamic_memory_llama_sampler_overwrite_enabled)
        .unwrap_or(true)
}

fn group_memory_tool_call_payload(provider_id: &str, call: &ToolCall, index: usize) -> Value {
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

fn group_memory_tool_result_message(
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

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupChatResponse {
    pub message: GroupMessage,
    pub character_id: String,
    pub character_name: String,
    pub reasoning: Option<String>,
    pub selection_reasoning: Option<String>,
    pub was_mentioned: bool,
    pub participation_stats: Vec<GroupParticipation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterInfo {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub definition: Option<String>,
    pub description: Option<String>,
    pub personality_summary: Option<String>,
    #[serde(default = "default_memory_type")]
    pub memory_type: String,
}

fn default_memory_type() -> String {
    "manual".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupChatContext {
    pub session: GroupSession,
    pub characters: Vec<CharacterInfo>,
    pub participation_stats: Vec<GroupParticipation>,
    pub recent_messages: Vec<GroupMessage>,
    pub user_message: String,
}

fn emit_group_chat_status(
    app: &AppHandle,
    session_id: &str,
    status: &str,
    extra: Map<String, Value>,
) {
    let mut payload = Map::new();
    payload.insert(
        "sessionId".to_string(),
        Value::String(session_id.to_string()),
    );
    payload.insert("status".to_string(), Value::String(status.to_string()));
    payload.extend(extra);
    let _ = app.emit("group_chat_status", Value::Object(payload));
}

fn emit_group_chat_error_status(app: &AppHandle, session_id: &str, message: &str) {
    let mut extra = Map::new();
    extra.insert("message".to_string(), Value::String(message.to_string()));
    emit_group_chat_status(app, session_id, "error", extra);
}

fn is_request_abort_error(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("aborted by user")
        || normalized.contains("cancelled by user")
        || normalized.contains("canceled by user")
}

struct AbortGuard<'a> {
    registry: &'a AbortRegistry,
    request_id: String,
}

impl<'a> AbortGuard<'a> {
    fn new(registry: &'a AbortRegistry, request_id: String) -> Self {
        Self {
            registry,
            request_id,
        }
    }
}

impl Drop for AbortGuard<'_> {
    fn drop(&mut self) {
        self.registry.unregister(&self.request_id);
    }
}


fn group_dynamic_memory_run_key(session_id: &str) -> String {
    format!("group:{}", session_id)
}

fn group_dynamic_memory_request_id(session_id: &str, phase: &str) -> String {
    format!("group-dynamic-memory:{}:{}", session_id, phase)
}

fn uses_local_dynamic_memory_model(provider_cred: &ProviderCredential, model: &Model) -> bool {
    crate::llama_cpp::is_llama_cpp(Some(provider_cred.provider_id.as_str()))
        || crate::llama_cpp::is_llama_cpp(Some(model.provider_id.as_str()))
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
        format!("group-dynamic-memory:prepare:{session_id}"),
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
                    "Failed to unload local chat model before group dynamic memory for session {}: {}",
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
        format!("group-dynamic-memory:finish:{session_id}"),
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
                    "Failed to unload local dynamic memory model for group session {}: {}",
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

// ============================================================================
// Usage Tracking Helper
// ============================================================================

/// Record usage for group chat operations
async fn record_group_usage(
    app: &AppHandle,
    usage: &Option<crate::chat_manager::types::UsageSummary>,
    session: &GroupSession,
    character: &Character,
    model: &Model,
    provider_cred: &ProviderCredential,
    api_key: &str,
    operation_type: UsageOperationType,
    log_scope: &str,
) {
    let Some(usage_info) = usage else {
        return;
    };

    let mut request_usage = RequestUsage {
        id: Uuid::new_v4().to_string(),
        timestamp: now_millis().unwrap_or(0),
        session_id: session.id.clone(),
        character_id: character.id.clone(),
        character_name: character.name.clone(),
        model_id: model.id.clone(),
        model_name: model.name.clone(),
        provider_id: provider_cred.provider_id.clone(),
        provider_label: provider_cred.label.clone(),
        operation_type,
        finish_reason: usage_info
            .finish_reason
            .as_ref()
            .and_then(|s| UsageFinishReason::from_str(s)),
        prompt_tokens: usage_info.prompt_tokens,
        completion_tokens: usage_info.completion_tokens,
        total_tokens: usage_info.total_tokens,
        cached_prompt_tokens: usage_info.cached_prompt_tokens,
        cache_write_tokens: usage_info.cache_write_tokens,
        memory_tokens: None,
        summary_tokens: None,
        reasoning_tokens: usage_info.reasoning_tokens,
        image_tokens: usage_info.image_tokens,
        audio_tokens: usage_info.audio_tokens,
        web_search_requests: usage_info.web_search_requests,
        api_cost: usage_info.api_cost,
        cost: None,
        success: true,
        error_message: None,
        metadata: Default::default(),
    };

    // Calculate memory and summary token counts from group session
    let memory_token_count: u64 = session
        .memory_embeddings
        .iter()
        .map(|m| m.token_count as u64)
        .sum();

    let summary_token_count = session.memory_summary_token_count as u64;

    if memory_token_count > 0 {
        request_usage.memory_tokens = Some(memory_token_count);
    }

    if summary_token_count > 0 {
        request_usage.summary_tokens = Some(summary_token_count);
    }

    if provider_cred.provider_id.eq_ignore_ascii_case("openrouter") {
        apply_openrouter_cost_to_usage(
            app,
            &mut request_usage,
            usage_info,
            &model.name,
            api_key,
            log_scope,
        )
        .await;
    } else if usage_info.cached_prompt_tokens.is_some()
        || usage_info.cache_write_tokens.is_some()
        || usage_info.web_search_requests.is_some()
        || usage_info.api_cost.is_some()
    {
        insert_extended_usage_metadata(&mut request_usage.metadata, usage_info);
    }

    if let Err(e) = add_usage_record(app, request_usage) {
        log_error(
            app,
            log_scope,
            format!("failed to record group chat usage: {}", e),
        );
    }
}

fn record_group_failed_usage(
    app: &AppHandle,
    usage: &Option<crate::chat_manager::types::UsageSummary>,
    session: &GroupSession,
    character: &Character,
    model: &Model,
    provider_cred: &ProviderCredential,
    operation_type: UsageOperationType,
    error_message: &str,
) {
    let Some(usage_info) = usage else {
        return;
    };
    let mut request_usage = RequestUsage {
        id: Uuid::new_v4().to_string(),
        timestamp: now_millis().unwrap_or(0),
        session_id: session.id.clone(),
        character_id: character.id.clone(),
        character_name: character.name.clone(),
        model_id: model.id.clone(),
        model_name: model.name.clone(),
        provider_id: provider_cred.provider_id.clone(),
        provider_label: provider_cred.label.clone(),
        operation_type,
        finish_reason: usage_info
            .finish_reason
            .as_deref()
            .and_then(UsageFinishReason::from_str),
        prompt_tokens: usage_info.prompt_tokens,
        completion_tokens: usage_info.completion_tokens,
        total_tokens: usage_info.total_tokens,
        cached_prompt_tokens: usage_info.cached_prompt_tokens,
        cache_write_tokens: usage_info.cache_write_tokens,
        memory_tokens: None,
        summary_tokens: None,
        reasoning_tokens: usage_info.reasoning_tokens,
        image_tokens: usage_info.image_tokens,
        audio_tokens: usage_info.audio_tokens,
        web_search_requests: usage_info.web_search_requests,
        api_cost: usage_info.api_cost,
        cost: None,
        success: false,
        error_message: Some(error_message.to_string()),
        metadata: Default::default(),
    };
    insert_extended_usage_metadata(&mut request_usage.metadata, usage_info);
    if let Err(error) = add_usage_record(app, request_usage) {
        log_error(
            app,
            "group_chat_response",
            format!("failed to record failed group usage: {error}"),
        );
    }
}

/// Record usage for decision maker (speaker selection) operations
async fn record_decision_maker_usage(
    app: &AppHandle,
    usage: &Option<crate::chat_manager::types::UsageSummary>,
    session: &GroupSession,
    model: &Model,
    provider_cred: &ProviderCredential,
    api_key: &str,
    log_scope: &str,
) {
    let Some(usage_info) = usage else {
        return;
    };

    let mut request_usage = RequestUsage {
        id: Uuid::new_v4().to_string(),
        timestamp: now_millis().unwrap_or(0),
        session_id: session.id.clone(),
        character_id: "decision_maker".to_string(),
        character_name: "Decision Maker".to_string(),
        model_id: model.id.clone(),
        model_name: model.name.clone(),
        provider_id: provider_cred.provider_id.clone(),
        provider_label: provider_cred.label.clone(),
        operation_type: UsageOperationType::GroupChatDecisionMaker,
        finish_reason: usage_info
            .finish_reason
            .as_ref()
            .and_then(|s| UsageFinishReason::from_str(s)),
        prompt_tokens: usage_info.prompt_tokens,
        completion_tokens: usage_info.completion_tokens,
        total_tokens: usage_info.total_tokens,
        cached_prompt_tokens: usage_info.cached_prompt_tokens,
        cache_write_tokens: usage_info.cache_write_tokens,
        memory_tokens: None,
        summary_tokens: None,
        reasoning_tokens: usage_info.reasoning_tokens,
        image_tokens: usage_info.image_tokens,
        audio_tokens: usage_info.audio_tokens,
        web_search_requests: usage_info.web_search_requests,
        api_cost: usage_info.api_cost,
        cost: None,
        success: true,
        error_message: None,
        metadata: Default::default(),
    };

    if provider_cred.provider_id.eq_ignore_ascii_case("openrouter") {
        apply_openrouter_cost_to_usage(
            app,
            &mut request_usage,
            usage_info,
            &model.name,
            api_key,
            log_scope,
        )
        .await;
    } else if usage_info.cached_prompt_tokens.is_some()
        || usage_info.cache_write_tokens.is_some()
        || usage_info.web_search_requests.is_some()
        || usage_info.api_cost.is_some()
    {
        insert_extended_usage_metadata(&mut request_usage.metadata, usage_info);
    }

    if let Err(e) = add_usage_record(app, request_usage) {
        log_error(
            app,
            log_scope,
            format!("failed to record decision maker usage: {}", e),
        );
    }
}

fn format_memories_with_ids(session: &GroupSession) -> Vec<String> {
    session
        .memory_embeddings
        .iter()
        .map(|m| format!("[{}] {}", m.id, m.text))
        .collect()
}

/// Build an enriched query from the last 2 messages for better memory retrieval.
fn build_enriched_query(messages: &[GroupMessage]) -> String {
    let convo: Vec<&GroupMessage> = messages
        .iter()
        .filter(|m| m.role == "user" || m.role == "assistant")
        .collect();

    match convo.len() {
        0 => String::new(),
        1 => convo[0].content.clone(),
        _ => {
            let last = &convo[convo.len() - 1];
            let second_last = &convo[convo.len() - 2];
            format!("{}\n{}", second_last.content, last.content)
        }
    }
}

fn conversation_count(messages: &[GroupMessage]) -> usize {
    messages
        .iter()
        .filter(|m| m.role == "user" || m.role == "assistant")
        .count()
}

fn conversation_window(messages: &[GroupMessage], limit: usize) -> Vec<GroupMessage> {
    let mut convo: Vec<GroupMessage> = messages
        .iter()
        .filter(|m| m.role == "user" || m.role == "assistant")
        .cloned()
        .collect();
    if convo.len() > limit {
        convo.drain(0..(convo.len() - limit));
    }
    convo
}

fn resolve_group_conversation_index_by_message_id(
    conn: &rusqlite::Connection,
    session_id: &str,
    message_id: &str,
) -> Result<Option<usize>, String> {
    let created_at: Option<i64> = conn
        .query_row(
            "SELECT created_at FROM group_messages
             WHERE session_id = ?1 AND id = ?2 AND (role = 'user' OR role = 'assistant')",
            rusqlite::params![session_id, message_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let Some(created_at) = created_at else {
        return Ok(None);
    };

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(1) FROM group_messages
             WHERE session_id = ?1 AND (role = 'user' OR role = 'assistant')
               AND (created_at < ?2 OR (created_at = ?2 AND id <= ?3))",
            rusqlite::params![session_id, created_at, message_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    Ok(Some(count.max(0) as usize))
}

/// Resolve the last valid cursor (windowEnd) from memory tool events by anchoring on message IDs.
/// Returns (window_end_index, cursor_rewound).
fn resolve_last_valid_group_window_end(
    conn: &rusqlite::Connection,
    session: &GroupSession,
) -> Result<(usize, bool), String> {
    crate::conversation_manager::memory::resolve_last_valid_window_end(
        &session.memory_tool_events,
        |end_id| resolve_group_conversation_index_by_message_id(conn, &session.id, end_id),
    )
}

fn cancel_group_dynamic_memory_cycle(
    app: &AppHandle,
    session: &mut GroupSession,
    pool: &SwappablePool,
    message: &str,
) -> Result<(), String> {
    session.memory_status = "idle".to_string();
    session.memory_error = None;
    session.updated_at = crate::utils::now_millis()? as i64;

    // Persist embeddings via the normalised table; the legacy JSON column is
    // cleared by `group_session_update_memories_internal` below since we pass
    // an empty slice.
    crate::storage_manager::memory_embeddings::replace_all_app(
        app,
        &session.id,
        crate::storage_manager::memory_embeddings::SessionKind::GroupSession,
        &session.memory_embeddings,
    )?;

    let conn = pool.get_connection()?;
    let empty: Vec<MemoryEmbedding> = Vec::new();
    group_session_update_memories_internal(
        &conn,
        &session.id,
        &session.memories,
        &empty,
        Some(&session.memory_summary),
        session.memory_summary_token_count,
        &session.memory_tool_events,
        Some(&session.memory_status),
        session.memory_error.as_deref(),
        session.memory_progress_step,
    )?;
    let _ = app.emit(
        "group-dynamic-memory:cancelled",
        json!({ "sessionId": session.id }),
    );
    Err(message.to_string())
}

fn ensure_group_dynamic_memory_not_cancelled(
    app: &AppHandle,
    session: &mut GroupSession,
    pool: &SwappablePool,
    token: &DynamicMemoryCancellationToken,
) -> Result<(), String> {
    if token.is_cancelled() {
        return cancel_group_dynamic_memory_cycle(
            app,
            session,
            pool,
            "Request was cancelled by user",
        );
    }
    Ok(())
}

fn fetch_group_conversation_messages_range(
    conn: &rusqlite::Connection,
    session_id: &str,
    start: usize,
    end: usize,
) -> Result<Vec<GroupMessage>, String> {
    if end <= start {
        return Ok(Vec::new());
    }

    let limit = (end - start) as i64;
    let offset = start as i64;

    let mut stmt = conn
        .prepare(
            "SELECT id, session_id, role, content, speaker_character_id, turn_number, created_at, is_pinned
             FROM group_messages
             WHERE session_id = ?1 AND (role = 'user' OR role = 'assistant')
             ORDER BY created_at ASC, id ASC
             LIMIT ?2 OFFSET ?3",
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let rows = stmt
        .query_map(rusqlite::params![session_id, limit, offset], |r| {
            Ok(GroupMessage {
                id: r.get(0)?,
                session_id: r.get(1)?,
                role: r.get(2)?,
                content: r.get(3)?,
                speaker_character_id: r.get(4)?,
                turn_number: r.get(5)?,
                created_at: r.get(6)?,
                usage: None,
                variants: None,
                selected_variant_id: None,
                is_pinned: r.get::<_, i64>(7)? != 0,
                attachments: Vec::new(),
                used_lorebook_entries: Vec::new(),
                memory_refs: Vec::new(),
                reasoning: None,
                selection_reasoning: None,
                model_id: None,
                gemini_content: None,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?);
    }
    Ok(out)
}

fn manual_window_size(settings: &Settings) -> usize {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|a| a.manual_mode_context_window)
        .unwrap_or(50) as usize
}

fn push_group_memory_event(session: &mut GroupSession, event: Value) {
    session.memory_tool_events.push(event);
    if session.memory_tool_events.len() > 50 {
        let excess = session.memory_tool_events.len() - 50;
        session.memory_tool_events.drain(0..excess);
    }
}

fn record_group_dynamic_memory_error(
    app: &AppHandle,
    session: &mut GroupSession,
    pool: &State<'_, SwappablePool>,
    error: &str,
    stage: &str,
    window_start: usize,
    window_end: usize,
    window_message_ids: &[String],
    summary: Option<&str>,
    debug_steps: Option<&[Value]>,
) {
    log_error(
        app,
        "group_dynamic_memory",
        format!("{} failed: {}", stage, error),
    );

    let event = json!({
        "id": Uuid::new_v4().to_string(),
        "windowStart": window_start,
        "windowEnd": window_end,
        "windowMessageIds": window_message_ids,
        "summary": summary.unwrap_or_default(),
        "actions": [],
        "error": error,
        "status": "error",
        "stage": stage,
        "debugSteps": debug_steps.map(|steps| Value::Array(steps.to_vec())).unwrap_or(Value::Null),
        "createdAt": now_millis().unwrap_or_default(),
    });
    push_group_memory_event(session, event);
    session.memory_status = "failed".to_string();
    session.memory_error = Some(format!("{}: {}", stage, error));

    if let Err(save_err) = save_group_session_memories(app, session, pool) {
        log_error(
            app,
            "group_dynamic_memory",
            format!("failed to persist error state: {}", save_err),
        );
    }

    let _ = app.emit(
        "group-dynamic-memory:error",
        json!({
            "sessionId": session.id,
            "error": session.memory_error.clone().unwrap_or_else(|| format!("{}: {}", stage, error)),
            "stage": stage
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
        || !tool_config
            .map(|cfg| !cfg.tools.is_empty())
            .unwrap_or(false)
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
    temperature: f64,
    top_p: f64,
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
        &provider_cred.provider_id,
        extra_body_fields,
        overwrite_llama_sampler_config,
    );
    if let Some(parallel_tool_calls) =
        requested_parallel_tool_calls(provider_cred, tool_config, extra_body_fields.as_ref())
    {
        log_info(
            app,
            "group_dynamic_memory",
            format!(
                "sending memory tool request with parallel_tool_calls={} provider={} model={} request_id={}",
                parallel_tool_calls,
                provider_cred.provider_id,
                model.name,
                request_id.unwrap_or("none")
            ),
        );
    }
    let built = crate::chat_manager::request_builder::build_chat_request(
        provider_cred,
        api_key,
        &model.name,
        messages_for_api,
        None,
        Some(temperature),
        Some(top_p),
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
        cache_key: None,
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
                    "group_dynamic_memory",
                    format!(
                        "provider rejected forced parallel tool calls; retrying with parallel_tool_calls=false. Provider={}, model={}",
                        provider_cred.provider_id, model.name
                    ),
                );
                let built = crate::chat_manager::request_builder::build_chat_request(
                    provider_cred,
                    api_key,
                    &model.name,
                    messages_for_api,
                    None,
                    Some(temperature),
                    Some(top_p),
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
                    "group_dynamic_memory",
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
                    cache_key: None,
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
                    "group_dynamic_memory",
                    format!(
                        "provider rejected forced parallel tool calls; retrying with parallel_tool_calls=false. Provider={}, model={}",
                        provider_cred.provider_id, model.name
                    ),
                );
                let built = crate::chat_manager::request_builder::build_chat_request(
                    provider_cred,
                    api_key,
                    &model.name,
                    messages_for_api,
                    None,
                    Some(temperature),
                    Some(top_p),
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
                    "group_dynamic_memory",
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
                    cache_key: None,
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
                    "group_dynamic_memory",
                    format!(
                        "provider rejected forced tool choice; retrying dynamic memory request with auto tool choice. Provider={}, model={}",
                        provider_cred.provider_id, model.name
                    ),
                );
                let auto_tool_config = tool_config_with_auto_choice(cfg);
                let built = crate::chat_manager::request_builder::build_chat_request(
                    provider_cred,
                    api_key,
                    &model.name,
                    messages_for_api,
                    None,
                    Some(temperature),
                    Some(top_p),
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
                    cache_key: None,
                };

                return api_request(app.clone(), api_request_payload).await;
            }
        }
    }

    Ok(first_response)
}

fn sanitize_dynamic_memory_extra_body_fields(
    provider_id: &str,
    extra_body_fields: Option<HashMap<String, Value>>,
    overwrite_llama_sampler_config: bool,
) -> Option<HashMap<String, Value>> {
    if !overwrite_llama_sampler_config || provider_id != "llamacpp" {
        return extra_body_fields;
    }
    let mut extra = extra_body_fields.unwrap_or_default();
    for key in [
        "llamaSamplerProfile",
        "llamaSamplerOrder",
        "llamaMinP",
        "llamaTypicalP",
        "llamaRepeatPenalty",
        "llamaNPenRange",
        "llamaDryMultiplier",
        "llamaDryBase",
        "llamaDryAllowedLength",
        "llamaDryPenaltyLastN",
        "llamaDrySequenceBreakers",
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
            "dry",
            "min_p",
            "typical"
        ]),
    );
    extra.insert("top_k".to_string(), json!(40));
    extra.insert("frequency_penalty".to_string(), json!(0.0));
    extra.insert("presence_penalty".to_string(), json!(0.0));
    extra.insert("min_p".to_string(), json!(0.0));
    extra.insert("typical_p".to_string(), json!(0.0));
    extra.insert("llamaRepeatPenalty".to_string(), json!(1.0));
    extra.insert("llamaNPenRange".to_string(), json!(-1));
    extra.insert("llamaDryMultiplier".to_string(), json!(0.0));

    if extra.is_empty() {
        None
    } else {
        Some(extra)
    }
}

// ============================================================================
// Memory Retrieval
// ============================================================================

fn emit_memory_vector_migration_toast(
    app: &AppHandle,
    toast_id: &str,
    title: &str,
    subtitle: &str,
    progress: f32,
) {
    let _ = app.emit(
        "app://toast",
        json!({
            "id": toast_id,
            "kind": "modelLoad",
            "title": title,
            "subtitle": subtitle,
            "modelName": "Memory embeddings",
            "progress": progress,
        }),
    );
}

fn dismiss_memory_vector_migration_toast(app: &AppHandle, toast_id: &str) {
    let _ = app.emit(
        "app://toast",
        json!({
            "id": toast_id,
            "dismiss": true,
        }),
    );
}

fn memory_embedding_is_pending(memory: &MemoryEmbedding) -> bool {
    memory.embedding.is_empty()
}

fn memory_embedding_is_stale(
    memory: &MemoryEmbedding,
    target_source_version: &str,
    target_dimensions: usize,
) -> bool {
    if memory.embedding.is_empty() {
        return false;
    }
    if memory.embedding.len() != target_dimensions {
        return true;
    }
    if memory.embedding_dimensions != Some(target_dimensions) {
        return true;
    }
    match memory.embedding_source_version.as_deref() {
        Some(version) => version != target_source_version,
        None => !(target_source_version == "v3" && target_dimensions == 512),
    }
}

fn memory_embedding_requires_embedding(
    memory: &MemoryEmbedding,
    target_source_version: &str,
    target_dimensions: usize,
) -> bool {
    memory_embedding_is_pending(memory)
        || memory_embedding_is_stale(memory, target_source_version, target_dimensions)
}

async fn migrate_group_memory_embeddings_if_needed(
    app: &AppHandle,
    session: &mut GroupSession,
    pool: &State<'_, SwappablePool>,
) -> Result<(), String> {
    if session.memory_embeddings.is_empty() {
        return Ok(());
    }

    let (target_source_version, target_dimensions) =
        embedding::resolve_active_embedding_signature(app)?;
    let needs_stale_migration = session
        .memory_embeddings
        .iter()
        .any(|memory| memory_embedding_is_stale(memory, &target_source_version, target_dimensions));
    let needs_pending_embed = session
        .memory_embeddings
        .iter()
        .any(memory_embedding_is_pending);
    if !needs_stale_migration && !needs_pending_embed {
        return Ok(());
    }
    let show_toast = needs_stale_migration;

    let toast_id = format!("group-memory-vector-migration:{}", session.id);
    let total = session.memory_embeddings.len().max(1);
    if show_toast {
        emit_memory_vector_migration_toast(
            app,
            &toast_id,
            "Migrating memory vectors",
            "Updating saved memories for the current memory model. Messages may be delayed briefly.",
            0.0,
        );
    }

    let migration_result: Result<(bool, usize), String> = async {
        let mut migrated_any = false;
        let mut failures = 0usize;
        for (idx, memory) in session.memory_embeddings.iter_mut().enumerate() {
            let was_stale =
                memory_embedding_is_stale(memory, &target_source_version, target_dimensions);
            if memory_embedding_requires_embedding(
                memory,
                &target_source_version,
                target_dimensions,
            ) {
                match tokio::time::timeout(
                    Duration::from_secs(MEMORY_MIGRATION_EMBED_TIMEOUT_SECS),
                    embedding::compute_embedding(app.clone(), memory.text.clone()),
                )
                .await
                {
                    Ok(Ok(vector)) => {
                        memory.embedding = vector;
                        memory.embedding_source_version = Some(target_source_version.clone());
                        memory.embedding_dimensions = Some(target_dimensions);
                        migrated_any = true;
                    }
                    Ok(Err(err)) => {
                        if was_stale {
                            failures += 1;
                            log_warn(
                                app,
                                "group_memory_retrieval",
                                format!(
                                    "failed to re-embed saved memory {}/{}: {}",
                                    idx + 1,
                                    total,
                                    err
                                ),
                            );
                        } else {
                            log_info(
                                app,
                                "group_memory_retrieval",
                                format!(
                                    "deferred pending embedding for memory {}/{} (will retry): {}",
                                    idx + 1,
                                    total,
                                    err
                                ),
                            );
                        }
                    }
                    Err(_) => {
                        if was_stale {
                            failures += 1;
                            log_warn(
                                app,
                                "group_memory_retrieval",
                                format!(
                                    "timed out after {}s while re-embedding saved memory {}/{}",
                                    MEMORY_MIGRATION_EMBED_TIMEOUT_SECS,
                                    idx + 1,
                                    total
                                ),
                            );
                        } else {
                            log_info(
                                app,
                                "group_memory_retrieval",
                                format!(
                                    "timed out after {}s while embedding pending memory {}/{} (will retry)",
                                    MEMORY_MIGRATION_EMBED_TIMEOUT_SECS,
                                    idx + 1,
                                    total
                                ),
                            );
                        }
                    }
                }
            }

            if show_toast {
                emit_memory_vector_migration_toast(
                    app,
                    &toast_id,
                    "Migrating memory vectors",
                    &format!("Re-embedded {}/{} saved memories.", idx + 1, total),
                    (idx + 1) as f32 / total as f32,
                );
            }
        }

        if migrated_any {
            save_group_session_memories(app, session, pool)?;
        }
        Ok((migrated_any, failures))
    }
    .await;

    if show_toast {
        dismiss_memory_vector_migration_toast(app, &toast_id);
    }

    let (migrated_any, failures) = match migration_result {
        Ok(outcome) => outcome,
        Err(err) => {
            let _ = app.emit(
                "app://toast",
                json!({
                    "variant": "error",
                    "title": "Memory migration failed",
                    "description": err.clone(),
                }),
            );
            return Err(err);
        }
    };

    if show_toast && migrated_any && failures == 0 {
        let _ = app.emit(
            "app://toast",
            json!({
                "variant": "success",
                "title": "Memory migration complete",
                "description": "Saved memory vectors are now using the current memory model.",
            }),
        );
    }
    Ok(())
}

/// Select relevant memories from a group session using semantic search
async fn select_relevant_memories(
    app: &AppHandle,
    session: &mut GroupSession,
    pool: &State<'_, SwappablePool>,
    query: &str,
    limit: usize,
    min_similarity: f32,
    strategy: &MemoryRetrievalStrategy,
) -> Vec<MemoryEmbedding> {
    if query.is_empty() || session.memory_embeddings.is_empty() {
        return Vec::new();
    }

    if let Err(err) = migrate_group_memory_embeddings_if_needed(app, session, pool).await {
        log_warn(
            app,
            "group_memory_retrieval",
            format!("memory vector migration failed: {}", err),
        );
    }

    let query_embedding = match embedding::compute_embedding(app.clone(), query.to_string()).await {
        Ok(vec) => vec,
        Err(err) => {
            log_warn(
                app,
                "group_memory_retrieval",
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

    // Smart mode: blend semantic match + recency/frequency + fallback fill.
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

    // 2. Add 1 most recently created hot memory (if not already selected)
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

    // 3. Add 1 most frequently accessed hot memory (if not already selected, access_count > 0)
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

    // 4. Fill remaining slots with next best cosine results
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

    // 5. Cold keyword fallback as last resort
    if results.is_empty() {
        let normalized_query = normalize_query_text(query);
        let cold_indices = search_cold_memory_indices_by_keyword(
            &session.memory_embeddings,
            &normalized_query,
            limit,
        );
        if !cold_indices.is_empty() {
            log_info(
                app,
                "group_memory_retrieval",
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

// Format memories as a string block for injection into prompts

// ============================================================================
// Dynamic Memory Cycle
// ============================================================================

/// Process dynamic memory cycle for group chat after a response
async fn process_group_dynamic_memory_cycle(
    app: &AppHandle,
    session: &mut GroupSession,
    settings: &Settings,
    pool: &State<'_, SwappablePool>,
    force: bool,
) -> Result<(), String> {
    log_info(
        app,
        "group_dynamic_memory",
        format!(
            "starting cycle: session_id={} embeddings={} events={}",
            session.id,
            session.memory_embeddings.len(),
            session.memory_tool_events.len()
        ),
    );
    let dynamic_settings = effective_group_dynamic_memory_settings(settings);

    if session.memory_type != "dynamic" {
        log_info(
            app,
            "group_dynamic_memory",
            "session memory_type is not dynamic; skipping",
        );
        return Ok(());
    }

    let window_size = dynamic_settings.summary_message_interval.max(1) as usize;
    let conn = pool.get_connection()?;

    // Load recent messages
    let messages =
        group_sessions::group_messages_list_internal_typed(&conn, &session.id, 100, None, None)
            .unwrap_or_default();

    let total_messages = messages.len();
    let total_convo = match conn.query_row(
        "SELECT COUNT(1) FROM group_messages WHERE session_id = ?1 AND (role = 'user' OR role = 'assistant')",
        rusqlite::params![&session.id],
        |row| row.get::<_, i64>(0),
    ) {
        Ok(count) => count.max(0) as usize,
        Err(err) => {
            log_warn(
                app,
                "group_dynamic_memory",
                format!("failed to count conversation messages: {}", err),
            );
            conversation_count(&messages)
        }
    };

    log_info(
        app,
        "group_dynamic_memory",
        format!(
            "snapshot: window_size={} total_convo={} total_messages={} non_convo_messages={}",
            window_size,
            total_convo,
            total_messages,
            total_messages.saturating_sub(total_convo)
        ),
    );

    // Check if enough new messages since last run (match normal chat behavior)
    // Use last_window_end from memory_tool_events to track progress
    let (last_window_end, cursor_rewound) = resolve_last_valid_group_window_end(&conn, session)?;

    log_info(
        app,
        "group_dynamic_memory",
        format!(
            "considering dynamic memory: total_convo={} window_size={} last_window_end={} cursor_rewound={}",
            total_convo, window_size, last_window_end, cursor_rewound
        ),
    );

    if !cursor_rewound && total_convo <= last_window_end {
        log_info(
            app,
            "group_dynamic_memory",
            format!(
                "no new messages since last run; skipping (total_convo={} last_window_end={})",
                total_convo, last_window_end
            ),
        );
        return Ok(());
    }

    let new_convo = total_convo.saturating_sub(last_window_end);
    if !cursor_rewound && new_convo < window_size {
        let next_window_end = last_window_end + window_size;
        log_info(
            app,
            "group_dynamic_memory",
            format!(
                "not enough new messages since last run (needed {}, got {}, next_window_end={})",
                window_size, new_convo, next_window_end
            ),
        );
        return Ok(());
    }

    if !force && !cursor_rewound {
        match dynamic_settings.run_mode.as_str() {
            "manual" => {
                log_info(
                    app,
                    "group_dynamic_memory",
                    "run_mode=manual; skipping automatic cycle",
                );
                return Ok(());
            }
            "askFirst" => {
                let approval =
                    app.state::<crate::dynamic_memory_approval::DynamicMemoryApprovalManager>();
                if let Some(pending_count) =
                    approval.should_prompt(&session.id, total_convo, last_window_end, window_size)
                {
                    log_info(
                        app,
                        "group_dynamic_memory",
                        format!(
                            "run_mode=askFirst; prompting for approval (pending_count={})",
                            pending_count
                        ),
                    );
                    let _ = app.emit(
                        "group-dynamic-memory:approval-needed",
                        serde_json::json!({ "sessionId": session.id, "pendingCount": pending_count }),
                    );
                }
                return Ok(());
            }
            _ => {}
        }
    }

    app.state::<crate::dynamic_memory_approval::DynamicMemoryApprovalManager>()
        .clear(&session.id);

    // Cursor-based delta summary window: summarize everything since last_window_end.
    // If backlog > window_size, include the whole backlog in this run (one-time catch-up).
    let mut window_start = if cursor_rewound { 0 } else { last_window_end };
    let mut window_end = total_convo;
    let convo_window = match fetch_group_conversation_messages_range(
        &conn,
        &session.id,
        window_start,
        window_end,
    ) {
        Ok(msgs) => msgs,
        Err(err) => {
            log_warn(
                app,
                "group_dynamic_memory",
                format!(
                    "failed to fetch conversation range from DB (start={} end={}): {}; falling back to in-memory window",
                    window_start, window_end, err
                ),
            );
            let fallback = conversation_window(&messages, window_size);
            window_end = total_convo;
            window_start = window_end.saturating_sub(fallback.len());
            fallback
        }
    };
    let window_message_ids: Vec<String> = convo_window.iter().map(|m| m.id.clone()).collect();

    if convo_window.is_empty() {
        log_warn(
            app,
            "group_dynamic_memory",
            format!(
                "no messages in computed window; skipping (window_start={} window_end={} total_convo={})",
                window_start, window_end, total_convo
            ),
        );
        return Ok(());
    }

    let run_key = group_dynamic_memory_run_key(&session.id);
    let run_manager = app.state::<DynamicMemoryRunManager>().inner().clone();
    let run_guard = run_manager.start_run(run_key);
    let cancel_token = run_guard.token();

    log_info(
        app,
        "group_dynamic_memory",
        format!(
            "window computed: window_start={} window_end={} window_count={} new_convo={} window_size={}",
            window_start,
            window_end,
            convo_window.len(),
            new_convo,
            window_size
        ),
    );

    // Apply importance decay
    let pinned_fixed = ensure_pinned_hot(&mut session.memory_embeddings);
    if pinned_fixed > 0 {
        log_info(
            app,
            "group_dynamic_memory",
            format!("Restored {} pinned memories to hot", pinned_fixed),
        );
    }

    let decay_rate = dynamic_settings.decay_rate;
    let cold_threshold = dynamic_settings.cold_threshold;
    let (decayed, demoted) =
        apply_memory_decay(&mut session.memory_embeddings, decay_rate, cold_threshold);
    if decayed > 0 || !demoted.is_empty() {
        log_info(
            app,
            "group_dynamic_memory",
            format!(
                "Memory decay applied: {} decayed, {} demoted to cold",
                decayed,
                demoted.len()
            ),
        );
    }

    let summarisation_model_id =
        match resolve_group_dynamic_memory_summarisation_model_id(app, settings) {
            Ok(id) => id,
            Err(err) => {
                record_group_dynamic_memory_error(
                    app,
                    session,
                    pool,
                    &err,
                    "summary_model",
                    window_start,
                    window_end,
                    &window_message_ids,
                    None,
                    None,
                );
                return Err(crate::utils::err_msg(module_path!(), line!(), err));
            }
        };

    let (summary_model, summary_provider) =
        match find_model_with_credential(settings, &summarisation_model_id) {
            Some(found) => found,
            None => {
                record_group_dynamic_memory_error(
                    app,
                    session,
                    pool,
                    "Summarisation model unavailable",
                    "summary_model",
                    window_start,
                    window_end,
                    &window_message_ids,
                    None,
                    None,
                );
                return Err(crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    "Summarisation model unavailable",
                ));
            }
        };

    let api_key = match require_api_key(app, summary_provider, "group_dynamic_memory") {
        Ok(key) => key,
        Err(err) => {
            record_group_dynamic_memory_error(
                app,
                session,
                pool,
                &err,
                "summary_api_key",
                window_start,
                window_end,
                &window_message_ids,
                None,
                None,
            );
            return Err(err);
        }
    };
    let using_local_dynamic_memory_model =
        uses_local_dynamic_memory_model(summary_provider, summary_model);
    if using_local_dynamic_memory_model {
        if let Err(err) = prepare_local_dynamic_memory_cycle(app, summary_model, &session.id).await
        {
            record_group_dynamic_memory_error(
                app,
                session,
                pool,
                &err,
                "prepare_local_model",
                window_start,
                window_end,
                &window_message_ids,
                None,
                None,
            );
            return Err(err);
        }
    }

    session.memory_status = "processing".to_string();
    session.memory_error = None;
    session.memory_progress_step = Some(1);
    if let Err(err) = save_group_session_memories(app, session, pool) {
        let _ = app.emit(
            "group-dynamic-memory:error",
            json!({ "sessionId": session.id, "error": err, "stage": "save_processing_state" }),
        );
        return Err(err);
    }
    let _ = app.emit(
        "group-dynamic-memory:processing",
        json!({ "sessionId": session.id }),
    );
    let _ = app.emit(
        "group-dynamic-memory:progress",
        json!({ "sessionId": session.id, "step": 1, "totalSteps": 4, "label": "Summarizing conversation" }),
    );

    ensure_group_dynamic_memory_not_cancelled(app, session, pool, &cancel_token)?;

    let prior_summary = if cursor_rewound || session.memory_summary.is_empty() {
        None
    } else {
        Some(session.memory_summary.clone())
    };
    let debug_capture_enabled = dynamic_memory_debug_capture_enabled(settings);
    let mut debug_steps: Vec<Value> = Vec::new();

    // Step 1: Summarize messages
    log_info(
        app,
        "group_dynamic_memory",
        "invoking summarize_group_messages",
    );

    let summary_request_id = group_dynamic_memory_request_id(&session.id, "summary");
    run_guard.set_active_request_id(Some(summary_request_id.clone()));

    let summary = match summarize_group_messages(
        app,
        summary_provider,
        summary_model,
        &api_key,
        &convo_window,
        prior_summary.as_deref(),
        settings,
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
            if is_cancelled_request_error(&err) {
                if using_local_dynamic_memory_model {
                    let _ =
                        finish_local_dynamic_memory_cycle(app, summary_model, &session.id).await;
                }
                return cancel_group_dynamic_memory_cycle(app, session, pool, &err);
            }
            log_error(
                app,
                "group_dynamic_memory",
                format!("summarization failed: {}", err),
            );
            if using_local_dynamic_memory_model {
                let _ = finish_local_dynamic_memory_cycle(app, summary_model, &session.id).await;
            }
            record_group_dynamic_memory_error(
                app,
                session,
                pool,
                &err,
                "summarization",
                window_start,
                window_end,
                &window_message_ids,
                prior_summary.as_deref(),
                if debug_capture_enabled {
                    Some(&debug_steps)
                } else {
                    None
                },
            );
            return Ok(());
        }
    };
    run_guard.set_active_request_id(None);

    // Step 2: Run memory tool update
    log_info(
        app,
        "group_dynamic_memory",
        "invoking run_group_memory_tool_update",
    );
    session.memory_progress_step = Some(2);
    let _ = save_group_session_memories(app, session, pool);
    let _ = app.emit(
        "group-dynamic-memory:progress",
        json!({ "sessionId": session.id, "step": 2, "totalSteps": 4, "label": "Analyzing memories" }),
    );

    ensure_group_dynamic_memory_not_cancelled(app, session, pool, &cancel_token)?;

    let tools_request_id = group_dynamic_memory_request_id(&session.id, "tools");
    run_guard.set_active_request_id(Some(tools_request_id.clone()));

    let actions = match run_group_memory_tool_update(
        app,
        summary_provider,
        summary_model,
        &api_key,
        session,
        settings,
        &dynamic_settings,
        &summary,
        &convo_window,
        debug_capture_enabled,
        &mut debug_steps,
        Some(&tools_request_id),
        Some(&cancel_token),
    )
    .await
    {
        Ok(result) => result,
        Err(err) => {
            run_guard.set_active_request_id(None);
            if is_cancelled_request_error(&err) {
                if using_local_dynamic_memory_model {
                    let _ =
                        finish_local_dynamic_memory_cycle(app, summary_model, &session.id).await;
                }
                return cancel_group_dynamic_memory_cycle(app, session, pool, &err);
            }
            log_error(
                app,
                "group_dynamic_memory",
                format!("memory tool update failed: {}", err),
            );
            if using_local_dynamic_memory_model {
                let _ = finish_local_dynamic_memory_cycle(app, summary_model, &session.id).await;
            }
            record_group_dynamic_memory_error(
                app,
                session,
                pool,
                &err,
                "memory_tools",
                window_start,
                window_end,
                &window_message_ids,
                prior_summary.as_deref(),
                if debug_capture_enabled {
                    Some(&debug_steps)
                } else {
                    None
                },
            );
            return Ok(());
        }
    };
    run_guard.set_active_request_id(None);
    log_info(
        app,
        "group_dynamic_memory",
        format!(
            "summary generated: length={} chars tokens={}",
            summary.len(),
            crate::embedding::tokenizer::count_tokens(app, &summary).unwrap_or(0)
        ),
    );
    session.memory_progress_step = Some(3);
    let _ = save_group_session_memories(app, session, pool);
    let _ = app.emit(
        "group-dynamic-memory:progress",
        json!({ "sessionId": session.id, "step": 3, "totalSteps": 4, "label": "Applying changes" }),
    );
    ensure_group_dynamic_memory_not_cancelled(app, session, pool, &cancel_token)?;
    session.memory_summary = summary;
    session.memory_summary_token_count =
        crate::embedding::tokenizer::count_tokens(app, &session.memory_summary).unwrap_or(0) as i32;

    // Enforce token budget
    let pinned_fixed = ensure_pinned_hot(&mut session.memory_embeddings);
    if pinned_fixed > 0 {
        log_info(
            app,
            "group_dynamic_memory",
            format!("Restored {} pinned memories to hot", pinned_fixed),
        );
    }

    let token_budget = dynamic_settings.hot_memory_token_budget;
    let demoted = enforce_hot_memory_budget(&mut session.memory_embeddings, token_budget);
    if !demoted.is_empty() {
        log_info(
            app,
            "group_dynamic_memory",
            format!(
                "Demoted {} memories to cold storage (budget: {} tokens)",
                demoted.len(),
                token_budget
            ),
        );
    }

    // Enforce max entries
    let max_entries = dynamic_settings.max_entries.max(1) as usize;
    let trimmed = trim_memories_to_max(&mut session.memory_embeddings, max_entries);
    if !trimmed.is_empty() {
        let _ = crate::storage_manager::memory_embeddings::delete_many_app(
            app,
            &session.id,
            crate::storage_manager::memory_embeddings::SessionKind::GroupSession,
            &trimmed,
        );
        log_info(
            app,
            "group_dynamic_memory",
            format!(
                "Trimmed {} memories to enforce max_entries={}",
                trimmed.len(),
                max_entries
            ),
        );
    }
    if session.memory_embeddings.len() > max_entries {
        log_warn(
            app,
            "group_dynamic_memory",
            format!(
                "Pinned memories exceed max_entries (count={}, max={})",
                session.memory_embeddings.len(),
                max_entries
            ),
        );
    }

    session.memories = session
        .memory_embeddings
        .iter()
        .map(|m| m.text.clone())
        .collect();

    // Record this memory cycle with windowEnd tracking (like normal chat)
    let memory_event = json!({
        "id": Uuid::new_v4().to_string(),
        "windowStart": window_start,
        "windowEnd": total_convo,
        "windowMessageIds": window_message_ids,
        "summary": session.memory_summary,
        "actions": actions,
        "status": "complete",
        "debugSteps": if debug_capture_enabled { Value::Array(debug_steps.clone()) } else { Value::Null },
        "createdAt": crate::utils::now_millis().unwrap_or(0),
    });
    push_group_memory_event(session, memory_event);
    session.memory_progress_step = Some(4);
    let _ = app.emit(
        "group-dynamic-memory:progress",
        json!({ "sessionId": session.id, "step": 4, "totalSteps": 4, "label": "Organizing memories" }),
    );
    session.memory_status = "idle".to_string();
    session.memory_error = None;
    session.memory_progress_step = None;

    // Save session memories
    if let Err(err) = save_group_session_memories(app, session, pool) {
        if using_local_dynamic_memory_model {
            let _ = finish_local_dynamic_memory_cycle(app, summary_model, &session.id).await;
        }
        let _ = app.emit(
            "group-dynamic-memory:error",
            json!({ "sessionId": session.id, "error": err, "stage": "save_session" }),
        );
        return Err(err);
    }

    let _ = app.emit(
        "group-dynamic-memory:success",
        json!({ "sessionId": session.id }),
    );

    log_info(
        app,
        "group_dynamic_memory",
        format!(
            "dynamic memory cycle complete: memories={}, events={}, windowEnd={}",
            session.memory_embeddings.len(),
            session.memory_tool_events.len(),
            total_convo
        ),
    );

    if using_local_dynamic_memory_model {
        finish_local_dynamic_memory_cycle(app, summary_model, &session.id).await?;
    }

    Ok(())
}

/// Summarize group messages using LLM
async fn summarize_group_messages(
    app: &AppHandle,
    provider_cred: &ProviderCredential,
    model: &Model,
    api_key: &str,
    convo_window: &[GroupMessage],
    prior_summary: Option<&str>,
    settings: &Settings,
    debug_capture_enabled: bool,
    debug_steps: &mut Vec<Value>,
    request_id: Option<&str>,
    cancel_token: Option<&DynamicMemoryCancellationToken>,
) -> Result<String, String> {
    let overwrite_llama_sampler_config = dynamic_memory_llama_sampler_overwrite_enabled(settings, model);
    let system_role = crate::chat_manager::request_builder::system_role_for(provider_cred);

    let summary_template =
        prompts::get_template(app, &dynamic_memory_summary_template_id(settings))
            .ok()
            .flatten();
    let mut prompt_entries = summary_template
        .map(|template| {
            if template.entries.is_empty() {
                vec![relative_system_entry(
                    "legacy_group_summary_prompt",
                    "Group Summary Instructions",
                    template.content,
                )]
            } else {
                template.entries
            }
        })
        .unwrap_or_else(|| {
            vec![relative_system_entry(
                "fallback_group_summary_prompt",
                "Group Summary Instructions",
                "Summarize the recent conversation transcript into a concise paragraph capturing durable facts, decisions, and character interactions. Note who said or did what when it matters.",
            )]
        });

    let prev_text = prior_summary
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("No previous summary provided.");
    for entry in &mut prompt_entries {
        entry.content = entry.content.replace("{{prev_summary}}", prev_text);
    }

    // Add conversation messages with speaker labels
    let mut conversation_messages = Vec::new();
    for msg in convo_window {
        let speaker = msg
            .speaker_character_id
            .as_ref()
            .map(|_| "Character")
            .unwrap_or(if msg.role == "user" {
                "User"
            } else {
                "Character"
            });
        conversation_messages.push(json!({
            "role": msg.role,
            "content": format!("[{} transcript line]: {}", speaker, msg.content)
        }));
    }

    prompt_entries.push(in_chat_user_entry(
        "runtime_group_summary_request",
        "Summary Output Request",
        "Return only the concise summary for the above conversation transcript. Use the write_summary tool.",
        0,
    ));
    let messages_for_api =
        assemble_prompt_messages(prompt_entries, conversation_messages, &system_role);

    let request_session = synthetic_feature_session(
        "__group_dynamic_memory__",
        feature_model_overrides(
            model,
            LlmFeature::DynamicMemory,
            DYNAMIC_MEMORY_MANAGER_DEFAULTS,
        ),
    );
    let (request_settings, extra_body_fields) = prepare_feature_request(
        &provider_cred.provider_id,
        &request_session,
        model,
        settings,
    );
    let max_tokens = request_settings.max_tokens;
    let context_length = request_settings.context_length;
    let temperature = request_settings
        .temperature
        .unwrap_or(DYNAMIC_MEMORY_MANAGER_DEFAULTS.temperature);
    let top_p = request_settings
        .top_p
        .unwrap_or(DYNAMIC_MEMORY_MANAGER_DEFAULTS.top_p);
    let tool_attempt = send_dynamic_memory_request(
        app,
        provider_cred,
        model,
        overwrite_llama_sampler_config,
        api_key,
        &messages_for_api,
        temperature,
        top_p,
        max_tokens,
        context_length,
        extra_body_fields.clone(),
        Some(&summarization_tool_config()),
        request_id,
        cancel_token,
    )
    .await;

    let tool_failure_reason = match tool_attempt {
        Ok(api_response) => {
            push_group_memory_debug_step(
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
            push_group_memory_debug_step(
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
        "group_dynamic_memory",
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
        temperature,
        top_p,
        max_tokens,
        context_length,
        extra_body_fields,
        None,
        request_id,
        cancel_token,
    )
    .await?;

    push_group_memory_debug_step(
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

/// Run memory tool update for group chat
async fn run_group_memory_tool_update(
    app: &AppHandle,
    provider_cred: &ProviderCredential,
    model: &Model,
    api_key: &str,
    session: &mut GroupSession,
    settings: &Settings,
    dynamic_settings: &DynamicMemorySettings,
    summary: &str,
    convo_window: &[GroupMessage],
    debug_capture_enabled: bool,
    debug_steps: &mut Vec<Value>,
    request_id: Option<&str>,
    cancel_token: Option<&DynamicMemoryCancellationToken>,
) -> Result<Vec<Value>, String> {
    let overwrite_llama_sampler_config = dynamic_memory_llama_sampler_overwrite_enabled(settings, model);
    let tool_config = build_memory_tool_config();
    let max_entries = dynamic_settings.max_entries.max(1) as usize;

    let system_role = crate::chat_manager::request_builder::system_role_for(provider_cred);

    let template_id = dynamic_memory_manager_template_id(settings, provider_cred, model);

    let mut prompt_entries = prompts::get_template(app, &template_id)
        .ok()
        .flatten()
        .map(|template| {
            if template.entries.is_empty() {
                vec![relative_system_entry(
                    "legacy_group_memory_prompt",
                    "Group Memory Instructions",
                    template.content,
                )]
            } else {
                template.entries
            }
        })
        .unwrap_or_else(|| {
            vec![relative_system_entry(
                "fallback_group_memory_prompt",
                "Group Memory Instructions",
                "You maintain a long-term memory index for a multi-speaker conversation transcript. Use tools to add or delete concise factual memories about the participants and events. Every create_memory call must include a category tag. Keep the list tidy and capped at {{max_entries}} entries. When finished, call the done tool.",
            )]
        });

    let pinned_fixed = ensure_pinned_hot(&mut session.memory_embeddings);
    if pinned_fixed > 0 {
        log_info(
            app,
            "group_dynamic_memory",
            format!("Restored {} pinned memories to hot", pinned_fixed),
        );
    }

    let current_tokens = calculate_hot_memory_tokens(&session.memory_embeddings);
    let token_budget = dynamic_settings.hot_memory_token_budget;

    for entry in &mut prompt_entries {
        entry.content = entry
            .content
            .replace("{{max_entries}}", &max_entries.to_string())
            .replace("{{current_memory_tokens}}", &current_tokens.to_string())
            .replace("{{hot_token_budget}}", &token_budget.to_string());
    }

    let memory_lines = format_memories_with_ids(session);
    let convo_text: Vec<String> = convo_window
        .iter()
        .map(|m| format!("{}: {}", m.role, m.content))
        .collect();

    prompt_entries.push(in_chat_user_entry(
        "runtime_group_memory_input",
        "Group Memory Update Input",
        format!(
            "Conversation transcript summary:\n{}\n\nRecent transcript lines:\n{}\n\nCurrent memories (with IDs):\n{}",
            summary,
            convo_text.join("\n"),
            if memory_lines.is_empty() { "none".to_string() } else { memory_lines.join("\n") }
        ),
        0,
    ));
    let mut messages_for_api = assemble_prompt_messages(prompt_entries, Vec::new(), &system_role);

    let request_session = synthetic_feature_session(
        "__group_dynamic_memory__",
        feature_model_overrides(
            model,
            LlmFeature::DynamicMemory,
            DYNAMIC_MEMORY_MANAGER_DEFAULTS,
        ),
    );
    let (request_settings, extra_body_fields) = prepare_feature_request(
        &provider_cred.provider_id,
        &request_session,
        model,
        settings,
    );
    let max_tokens = request_settings.max_tokens;
    let context_length = request_settings.context_length;
    let temperature = request_settings
        .temperature
        .unwrap_or(DYNAMIC_MEMORY_MANAGER_DEFAULTS.temperature);
    let top_p = request_settings
        .top_p
        .unwrap_or(DYNAMIC_MEMORY_MANAGER_DEFAULTS.top_p);
    let fallback_format = dynamic_memory_structured_fallback_format(settings);
    let fallback_label = structured_fallback_format_label(fallback_format);

    let mut actions_log: Vec<Value> = Vec::new();
    let mut untagged_candidates: Vec<(String, bool)> = Vec::new();
    let initial_memory_count = session.memory_embeddings.len();
    let max_hard_deletes = max_hard_deletes_per_cycle(
        initial_memory_count,
        dynamic_settings.max_hard_delete_ratio_per_cycle,
    );
    let mut hard_delete_count = 0usize;
    let recursive_loops_enabled = dynamic_settings.recursive_memory_loops;
    let max_loop_iterations = if recursive_loops_enabled {
        dynamic_settings.recursive_memory_loop_hard_cap.max(1) as usize
    } else {
        1
    };
    log_info(
        app,
        "group_dynamic_memory",
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
            "group_dynamic_memory",
            format!(
                "memory tool loop iteration {}/{} requesting tool calls current_memories={} request_id={}",
                iteration + 1,
                max_loop_iterations,
                session.memory_embeddings.len(),
                iteration_request_id.as_deref().unwrap_or("none")
            ),
        );

        let (calls, call_source) = match send_dynamic_memory_request(
            app,
            provider_cred,
            model,
            overwrite_llama_sampler_config,
            api_key,
            &messages_for_api,
            temperature,
            top_p,
            max_tokens,
            context_length,
            extra_body_fields.clone(),
            Some(&tool_config),
            iteration_request_id.as_deref(),
            cancel_token,
        )
        .await
        {
            Ok(api_response) => {
                push_group_memory_debug_step(
                    debug_steps,
                    debug_capture_enabled,
                    json!({
                        "phase": "memory_tool_request",
                        "iteration": iteration + 1,
                        "requestId": iteration_request_id,
                        "providerId": provider_cred.provider_id,
                        "model": model.name,
                        "response": {
                            "ok": api_response.ok,
                            "status": api_response.status,
                            "data": api_response.data().clone(),
                        }
                    }),
                );
                if !api_response.ok {
                    let fallback = format!("Provider returned status {}", api_response.status);
                    let err_message =
                        extract_error_message(api_response.data()).unwrap_or(fallback);
                    log_warn(
                        app,
                        "group_dynamic_memory",
                        format!(
                            "memory tool request failed; retrying with {} fallback: {}",
                            fallback_label, err_message
                        ),
                    );
                    if cancel_token.is_some_and(|token| token.is_cancelled()) {
                        return Err("Request was cancelled by user".to_string());
                    }
                    let mut fallback_messages = messages_for_api.clone();
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
                        temperature,
                        top_p,
                        max_tokens,
                        context_length,
                        extra_body_fields.clone(),
                        None,
                        iteration_request_id.as_deref(),
                        cancel_token,
                    )
                    .await?;

                    push_group_memory_debug_step(
                        debug_steps,
                        debug_capture_enabled,
                        json!({
                            "phase": "memory_tool_fallback_after_http_error",
                            "iteration": iteration + 1,
                            "requestId": iteration_request_id,
                            "providerId": provider_cred.provider_id,
                            "model": model.name,
                            "response": {
                                "ok": api_response.ok,
                                "status": api_response.status,
                                "data": api_response.data().clone(),
                            }
                        }),
                    );

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
                    (
                        parse_memory_operations_from_text(&text, fallback_format)?,
                        "text_fallback_after_http_error",
                    )
                } else {
                    let tool_calls =
                        parse_tool_calls(&provider_cred.provider_id, api_response.data());
                    if !tool_calls.is_empty() {
                        (tool_calls, "provider_tool_calls")
                    } else {
                        log_warn(
                            app,
                            "group_dynamic_memory",
                            format!(
                                "memory tool request returned no tool usage; retrying with {} fallback",
                                fallback_label
                            ),
                        );
                        if cancel_token.is_some_and(|token| token.is_cancelled()) {
                            return Err("Request was cancelled by user".to_string());
                        }
                        let mut fallback_messages = messages_for_api.clone();
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
                            temperature,
                            top_p,
                            max_tokens,
                            context_length,
                            extra_body_fields.clone(),
                            None,
                            iteration_request_id.as_deref(),
                            cancel_token,
                        )
                        .await?;

                        push_group_memory_debug_step(
                            debug_steps,
                            debug_capture_enabled,
                            json!({
                                "phase": "memory_tool_fallback_after_empty_tool_calls",
                                "iteration": iteration + 1,
                                "requestId": iteration_request_id,
                                "providerId": provider_cred.provider_id,
                                "model": model.name,
                                "response": {
                                    "ok": api_response.ok,
                                    "status": api_response.status,
                                    "data": api_response.data().clone(),
                                }
                            }),
                        );

                        if !api_response.ok {
                            let fallback =
                                format!("Provider returned status {}", api_response.status);
                            let err_message = extract_error_message(api_response.data())
                                .unwrap_or(fallback.clone());
                            return Err(if err_message == fallback {
                                err_message
                            } else {
                                format!("{} (status {})", err_message, api_response.status)
                            });
                        }

                        let text =
                            extract_text(api_response.data(), Some(&provider_cred.provider_id))
                                .ok_or_else(|| {
                                    "memory fallback returned neither tool calls nor text output"
                                        .to_string()
                                })?;
                        (
                            parse_memory_operations_from_text(&text, fallback_format)?,
                            "text_fallback_after_empty_tool_calls",
                        )
                    }
                }
            }
            Err(err) => {
                push_group_memory_debug_step(
                    debug_steps,
                    debug_capture_enabled,
                    json!({
                        "phase": "memory_tool_request_error",
                        "iteration": iteration + 1,
                        "requestId": iteration_request_id,
                        "providerId": provider_cred.provider_id,
                        "model": model.name,
                        "error": err,
                    }),
                );
                log_warn(
                    app,
                    "group_dynamic_memory",
                    format!(
                        "memory tool request errored; retrying with {} fallback: {}",
                        fallback_label, err
                    ),
                );
                if cancel_token.is_some_and(|token| token.is_cancelled()) {
                    return Err("Request was cancelled by user".to_string());
                }
                let mut fallback_messages = messages_for_api.clone();
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
                    temperature,
                    top_p,
                    max_tokens,
                    context_length,
                    extra_body_fields.clone(),
                    None,
                    iteration_request_id.as_deref(),
                    cancel_token,
                )
                .await?;

                push_group_memory_debug_step(
                    debug_steps,
                    debug_capture_enabled,
                    json!({
                        "phase": "memory_tool_fallback_after_request_error",
                        "iteration": iteration + 1,
                        "requestId": iteration_request_id,
                        "providerId": provider_cred.provider_id,
                        "model": model.name,
                        "response": {
                            "ok": api_response.ok,
                            "status": api_response.status,
                            "data": api_response.data().clone(),
                        }
                    }),
                );

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
                (
                    parse_memory_operations_from_text(&text, fallback_format)?,
                    "text_fallback_after_request_error",
                )
            }
        };

        log_raw_group_memory_tool_calls(app, call_source, &calls);
        push_group_memory_debug_step(
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
                "group_dynamic_memory",
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
            .map(|(index, call)| {
                group_memory_tool_call_payload(&provider_cred.provider_id, call, index)
            })
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
                                    "group_dynamic_memory",
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
                                        "group_dynamic_memory",
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
                                "group_dynamic_memory",
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
                                    "group_dynamic_memory",
                                    format!(
                                        "Skipping memory without required category: {}",
                                        reason
                                    ),
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
                                    "group_dynamic_memory",
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

                        let (embedding_source_version, embedding_dimensions) = match embedding
                            .as_ref()
                        {
                            Some(vector) if !vector.is_empty() => {
                                let (version, dimensions) =
                                    embedding::embedding_signature_for_result(app, Some(vector));
                                (Some(version), Some(dimensions))
                            }
                            _ => (None, None),
                        };
                        let _now = now_millis().unwrap_or_default();
                        session.memory_embeddings.push(MemoryEmbedding {
                            id: mem_id.clone(),
                            text,
                            embedding: embedding.unwrap_or_default(),
                            created_at: _now,
                            token_count: token_count as u32,
                            is_cold: false,
                            last_accessed_at: _now,
                            importance_score: 1.0,
                            persistence_importance: 1.0,
                            prompt_importance: 1.0,
                            volatility: 0.4,
                            is_pinned,
                            access_count: 0,
                            embedding_source_version,
                            embedding_dimensions,
                            match_score: None,
                            category: Some(category),
                            observed_at: None,
                            observed_time_precision: None,
                            canonical_entities: Vec::new(),
                            fact_signature: None,
                            fact_polarity: None,
                            source_role: None,
                            source_message_id: None,
                            superseded_by: None,
                            superseded_at: None,
                            supersedes: Vec::new(),
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

                        log_info(
                            app,
                            "group_dynamic_memory",
                            format!("Created memory {}", mem_id),
                        );
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
                                .unwrap_or(dynamic_settings.delete_confidence_default as f64)
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
                                    let cold_threshold = dynamic_settings.cold_threshold;
                                    session.memory_embeddings[idx].is_cold = true;
                                    session.memory_embeddings[idx].importance_score =
                                        cold_threshold;
                                    log_info(
                                        app,
                                        "group_dynamic_memory",
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
                                    let removed = session.memory_embeddings.remove(idx);
                                    log_info(
                                        app,
                                        "group_dynamic_memory",
                                        format!("Deleted memory {}", removed.id),
                                    );
                                    Some(removed)
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
                                "group_dynamic_memory",
                                format!("delete_memory could not find: {}", text),
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
                        if let Some(mem) = session.memory_embeddings.iter_mut().find(|m| m.id == id)
                        {
                            mem.is_pinned = true;
                            mem.importance_score = 1.0;
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
                            log_info(app, "group_dynamic_memory", format!("Pinned memory {}", id));
                        }
                    }
                }
                "unpin_memory" => {
                    if let Some(raw_id) = call.arguments.get("id").and_then(|v| v.as_str()) {
                        let id = sanitize_memory_id(raw_id);
                        if let Some(mem) = session.memory_embeddings.iter_mut().find(|m| m.id == id)
                        {
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
                            log_info(
                                app,
                                "group_dynamic_memory",
                                format!("Unpinned memory {}", id),
                            );
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
            "group_dynamic_memory",
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
        push_group_memory_debug_step(
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
                "group_dynamic_memory",
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
                "group_dynamic_memory",
                "memory tool loop recursive mode disabled; stopping after single pass",
            );
            break;
        }

        if tool_results.is_empty() {
            log_warn(
                app,
                "group_dynamic_memory",
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
            messages_for_api.push(group_memory_tool_result_message(
                &provider_cred.provider_id,
                tool_call_id,
                tool_name,
                result,
            ));
        }

        if iteration + 1 == max_loop_iterations {
            log_warn(
                app,
                "group_dynamic_memory",
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
            "group_dynamic_memory",
            format!(
                "Running memory category repair for {} candidate(s)",
                candidate_texts.len()
            ),
        );

        match run_group_memory_tag_repair(
            app,
            provider_cred,
            model,
            overwrite_llama_sampler_config,
            api_key,
            temperature,
            top_p,
            &candidate_texts,
            fallback_format,
        )
        .await
        {
            Ok(repaired) => {
                log_info(
                    app,
                    "group_dynamic_memory",
                    format!(
                        "Memory category repair returned {} mapped candidate(s)",
                        repaired.len()
                    ),
                );
                for (text, is_pinned) in untagged_candidates {
                    let Some(category) = repaired.get(&text).cloned() else {
                        log_warn(
                            app,
                            "group_dynamic_memory",
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
                                    "group_dynamic_memory",
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
                    let (embedding_source_version, embedding_dimensions) = match embedding.as_ref()
                    {
                        Some(vector) if !vector.is_empty() => {
                            let (version, dimensions) =
                                embedding::embedding_signature_for_result(app, Some(vector));
                            (Some(version), Some(dimensions))
                        }
                        _ => (None, None),
                    };
                    let now = now_millis().unwrap_or_default();
                    session.memory_embeddings.push(MemoryEmbedding {
                        id: mem_id.clone(),
                        text: text.clone(),
                        embedding: embedding.unwrap_or_default(),
                        created_at: now,
                        token_count: token_count as u32,
                        is_cold: false,
                        last_accessed_at: now,
                        importance_score: 1.0,
                        persistence_importance: 1.0,
                        prompt_importance: 1.0,
                        volatility: 0.4,
                        is_pinned,
                        access_count: 0,
                        embedding_source_version,
                        embedding_dimensions,
                        match_score: None,
                        category: Some(category.clone()),
                        observed_at: None,
                        observed_time_precision: None,
                        canonical_entities: Vec::new(),
                        fact_signature: None,
                        fact_polarity: None,
                        source_role: None,
                        source_message_id: None,
                        superseded_by: None,
                        superseded_at: None,
                        supersedes: Vec::new(),
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
                        "group_dynamic_memory",
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
                    "group_dynamic_memory",
                    format!("memory category repair pass failed: {}", err),
                );
            }
        }
    }

    let trimmed = trim_memories_to_max(&mut session.memory_embeddings, max_entries);
    if !trimmed.is_empty() {
        let _ = crate::storage_manager::memory_embeddings::delete_many_app(
            app,
            &session.id,
            crate::storage_manager::memory_embeddings::SessionKind::GroupSession,
            &trimmed,
        );
        log_info(
            app,
            "group_dynamic_memory",
            format!(
                "Trimmed {} memories to enforce max_entries={}",
                trimmed.len(),
                max_entries
            ),
        );
    }
    if session.memory_embeddings.len() > max_entries {
        log_warn(
            app,
            "group_dynamic_memory",
            format!(
                "Pinned memories exceed max_entries (count={}, max={})",
                session.memory_embeddings.len(),
                max_entries
            ),
        );
    }

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

async fn run_group_memory_tag_repair(
    app: &AppHandle,
    provider_cred: &ProviderCredential,
    model: &Model,
    overwrite_llama_sampler_config: bool,
    api_key: &str,
    temperature: f64,
    top_p: f64,
    texts: &[String],
    fallback_format: crate::chat_manager::types::DynamicMemoryStructuredFallbackFormat,
) -> Result<HashMap<String, String>, String> {
    if texts.is_empty() {
        return Ok(HashMap::new());
    }

    let system_role = crate::chat_manager::request_builder::system_role_for(provider_cred);
    let prompt_entries = vec![
        relative_system_entry(
            "group_memory_tag_repair_rules",
            "Group Memory Tag Repair Rules",
            "Classify each memory text with exactly one valid category. Use only retag_memory tool calls.",
        ),
        in_chat_user_entry(
            "runtime_group_memory_tag_repair_input",
            "Group Memory Tag Repair Input",
            format!(
            "Valid categories: {}.\nReturn one retag_memory tool call per text.\nTexts:\n{}",
            ALLOWED_MEMORY_CATEGORIES.join(", "),
            texts
                .iter()
                .enumerate()
                .map(|(i, t)| format!("{}. {}", i + 1, t))
                .collect::<Vec<_>>()
                .join("\n")
            ),
            0,
        ),
    ];
    let messages_for_api = assemble_prompt_messages(prompt_entries, Vec::new(), &system_role);

    let mut repaired = HashMap::new();
    let fallback_label = structured_fallback_format_label(fallback_format);
    match send_dynamic_memory_request(
        app,
        provider_cred,
        model,
        overwrite_llama_sampler_config,
        api_key,
        &messages_for_api,
        temperature,
        top_p,
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
                "group_dynamic_memory",
                format!(
                    "memory tag repair tool request failed; retrying with {} fallback: {}",
                    fallback_label, err_message
                ),
            );
        }
        Err(err) => {
            log_warn(
                app,
                "group_dynamic_memory",
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
            temperature,
            top_p,
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
                    "group_dynamic_memory",
                    format!(
                        "memory tag repair {} fallback failed: {}",
                        fallback_label, err_message
                    ),
                );
            }
            Err(err) => {
                log_warn(
                    app,
                    "group_dynamic_memory",
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

fn build_memory_tool_config() -> ToolConfig {
    ToolConfig {
        tools: vec![
            ToolDefinition {
                name: "create_memory".to_string(),
                description: Some(
                    "Create a concise memory entry capturing important facts from the group chat."
                        .to_string(),
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
                description: Some("Delete an outdated or redundant memory. Low confidence (< 0.7) triggers soft-delete to cold storage.".to_string()),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "text": { "type": "string", "description": "Memory ID (6-digit) or exact text to remove" },
                        "confidence": { "type": "number", "description": "Confidence that this memory should be deleted (0.0-1.0). Below 0.7 triggers soft-delete to cold storage." }
                    },
                    "required": ["text"]
                }),
            },
            ToolDefinition {
                name: "pin_memory".to_string(),
                description: Some("Pin a critical memory so it never decays.".to_string()),
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
            description: Some("Write the conversation summary.".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "summary": { "type": "string", "description": "The conversation summary" }
                },
                "required": ["summary"]
            }),
        }],
        choice: Some(ToolChoice::Required),
    }
}

fn find_model_with_credential<'a>(
    settings: &'a Settings,
    model_id: &str,
) -> Option<(&'a Model, &'a ProviderCredential)> {
    let model = settings.models.iter().find(|m| m.id == model_id)?;
    let credential = resolve_credential_for_model(settings, model)?;
    Some((model, credential))
}

fn save_group_session_memories(
    app: &AppHandle,
    session: &GroupSession,
    pool: &State<'_, SwappablePool>,
) -> Result<(), String> {
    // Persist embeddings to the new normalised table first. If this fails, the
    // legacy JSON column still holds the previous state.
    crate::storage_manager::memory_embeddings::replace_all_app(
        app,
        &session.id,
        crate::storage_manager::memory_embeddings::SessionKind::GroupSession,
        &session.memory_embeddings,
    )?;

    // Then clear the legacy JSON column by writing an empty slice. The other
    // memory metadata still travels through this path.
    let conn = pool.get_connection()?;
    let empty: Vec<MemoryEmbedding> = Vec::new();
    group_session_update_memories_internal(
        &conn,
        &session.id,
        &session.memories,
        &empty,
        Some(&session.memory_summary),
        session.memory_summary_token_count,
        &session.memory_tool_events,
        Some(&session.memory_status),
        session.memory_error.as_deref(),
        session.memory_progress_step,
    )?;
    log_info(
        app,
        "group_dynamic_memory",
        format!(
            "Saved {} memories for session {}",
            session.memory_embeddings.len(),
            session.id
        ),
    );
    Ok(())
}

// ============================================================================
// Character & Data Loading
// ============================================================================

/// Load full Character struct from database
fn load_character(conn: &rusqlite::Connection, character_id: &str) -> Result<Character, String> {
    let row: (
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        i64,
        i64,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT id, name, avatar_path, design_description, design_reference_image_ids, background_image_path,
                    description, definition, created_at, updated_at, default_model_id, memory_type,
                    prompt_template_id, group_chat_prompt_template_id, group_chat_roleplay_prompt_template_id
             FROM characters WHERE id = ?1",
            rusqlite::params![character_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                    row.get(11)?,
                    row.get(12)?,
                    row.get(13)?,
                    row.get(14)?,
                ))
            },
        )
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to load character {}: {}", character_id, e),
            )
        })?;

    let description = row.6;
    let definition = row
        .7
        .map(|value| {
            crate::storage_manager::entity_transfer::strip_legacy_card_prompt_sections(&value)
        })
        .filter(|value| !value.is_empty())
        .or(description.clone());
    let design_reference_image_ids = row
        .4
        .as_deref()
        .and_then(|value| serde_json::from_str::<Vec<String>>(value).ok())
        .unwrap_or_default();

    Ok(Character {
        id: row.0,
        name: row.1,
        avatar_path: row.2,
        design_description: row.3,
        design_reference_image_ids,
        lora_name: None,
        lora_strength: None,
        background_image_path: row.5,
        description,
        definition,
        rules: Vec::new(),
        scenes: Vec::new(),
        default_scene_id: None,
        default_model_id: row.10,
        mode: "roleplay".to_string(),
        companion: None,
        memory_type: row.11.unwrap_or_else(|| "manual".to_string()),
        active_lorebook_ids: Vec::new(),
        prompt_template_id: row.12,
        group_chat_prompt_template_id: row.13,
        group_chat_roleplay_prompt_template_id: row.14,
        system_prompt: None,
        created_at: row.8 as u64,
        updated_at: row.9 as u64,
    })
}

/// Load character info for all characters in a group session
fn load_characters_info(
    conn: &rusqlite::Connection,
    character_ids: &[String],
) -> Result<Vec<CharacterInfo>, String> {
    let mut characters = Vec::new();

    for character_id in character_ids {
        let result: Result<(String, Option<String>, Option<String>, Option<String>, Option<String>), _> = conn
            .query_row(
                "SELECT name, description, definition, system_prompt, memory_type FROM characters WHERE id = ?1",
                rusqlite::params![character_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            );

        if let Ok((name, description, definition, _system_prompt, memory_type)) = result {
            let definition = definition
                .map(|value| {
                    crate::storage_manager::entity_transfer::strip_legacy_card_prompt_sections(
                        &value,
                    )
                })
                .filter(|value| !value.is_empty());
            let personality_source = definition.as_ref().or(description.as_ref());
            let personality_summary = personality_source.map(|s| {
                let mut summary: String = s.chars().take(200).collect();
                if summary.len() < s.len() {
                    summary.push_str("...");
                    summary
                } else {
                    s.clone()
                }
            });

            characters.push(CharacterInfo {
                id: character_id.clone(),
                name,
                definition: definition.or(description.clone()),
                description,
                personality_summary,
                memory_type: memory_type.unwrap_or_else(|| "manual".to_string()),
            });
        }
    }

    Ok(characters)
}

/// Load recent messages from a group session
fn load_recent_group_messages(
    conn: &rusqlite::Connection,
    session_id: &str,
    limit: i32,
) -> Result<Vec<GroupMessage>, String> {
    group_sessions::group_messages_list_internal_typed(conn, session_id, limit, None, None)
}

/// Build the full context for character selection
fn build_selection_context(
    conn: &rusqlite::Connection,
    session_id: &str,
    user_message: &str,
) -> Result<GroupChatContext, String> {
    let session = group_sessions::group_session_get_internal_typed(conn, session_id)?;

    let characters = load_characters_info(conn, &session.character_ids)?;

    let participation_stats =
        group_sessions::group_participation_stats_internal_typed(conn, session_id)?;

    // Load more messages for selection context (selection needs good context for fair decisions)
    let recent_messages = load_recent_group_messages(conn, session_id, 30)?;

    Ok(GroupChatContext {
        session,
        characters,
        participation_stats,
        recent_messages,
        user_message: user_message.to_string(),
    })
}

/// Update participation stats after a character speaks
fn update_participation(
    conn: &rusqlite::Connection,
    session_id: &str,
    character_id: &str,
    turn_number: i32,
) -> Result<(), String> {
    let now = now_ms();

    conn.execute(
        "UPDATE group_participation
         SET speak_count = speak_count + 1, last_spoke_turn = ?1, last_spoke_at = ?2
         WHERE session_id = ?3 AND character_id = ?4",
        rusqlite::params![turn_number, now, session_id, character_id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    Ok(())
}

/// Save a user message to the group chat
fn save_user_message(
    conn: &rusqlite::Connection,
    session_id: &str,
    content: &str,
    message_id: Option<String>,
    attachments: &[ImageAttachment],
) -> Result<GroupMessage, String> {
    let now = now_ms();
    let id = message_id.unwrap_or_else(|| Uuid::new_v4().to_string());

    let max_turn: Option<i32> = conn
        .query_row(
            "SELECT MAX(turn_number) FROM group_messages WHERE session_id = ?1",
            rusqlite::params![session_id],
            |row| row.get(0),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let turn_number = max_turn.unwrap_or(0) + 1;

    conn.execute(
        "INSERT INTO group_messages (id, session_id, role, content, speaker_character_id, turn_number,
         created_at, is_pinned, attachments)
         VALUES (?1, ?2, 'user', ?3, NULL, ?4, ?5, 0, ?6)",
        rusqlite::params![
            id,
            session_id,
            content,
            turn_number,
            now,
            serde_json::to_string(attachments).unwrap_or_else(|_| "[]".to_string())
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    conn.execute(
        "UPDATE group_sessions SET updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, session_id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    Ok(GroupMessage {
        id,
        session_id: session_id.to_string(),
        role: "user".to_string(),
        content: content.to_string(),
        speaker_character_id: None,
        turn_number,
        created_at: now as i64,
        usage: None,
        variants: None,
        selected_variant_id: None,
        is_pinned: false,
        attachments: attachments.to_vec(),
        used_lorebook_entries: Vec::new(),
        memory_refs: Vec::new(),
        reasoning: None,
        selection_reasoning: None,
        model_id: None,
        gemini_content: None,
    })
}

/// Save an assistant message to the group chat
fn save_assistant_message(
    app: &AppHandle,
    conn: &rusqlite::Connection,
    session_id: &str,
    message_id: Option<String>,
    character_id: &str,
    content: &str,
    reasoning: Option<&str>,
    selection_reasoning: Option<&str>,
    usage: Option<&UsageSummary>,
    model_id: Option<&str>,
    used_lorebook_entries: &[String],
    memory_refs: &[String],
    attachments: &[ImageAttachment],
    gemini_content: Option<&Value>,
) -> Result<GroupMessage, String> {
    let now = now_ms();
    let id = message_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let variant_id = Uuid::new_v4().to_string();

    let max_turn: Option<i32> = conn
        .query_row(
            "SELECT MAX(turn_number) FROM group_messages WHERE session_id = ?1",
            rusqlite::params![session_id],
            |row| row.get(0),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let turn_number = max_turn.unwrap_or(0) + 1;

    let (prompt_tokens, completion_tokens, total_tokens) = match usage {
        Some(u) => (u.prompt_tokens, u.completion_tokens, u.total_tokens),
        None => (None, None, None),
    };

    log_info(
        app,
        "save_assistant_message",
        format!(
            "✓ Saving message {} with model_id: {:?}, character_id: {}",
            id, model_id, character_id
        ),
    );

    conn.execute(
        "INSERT INTO group_messages (id, session_id, role, content, speaker_character_id, turn_number,
         created_at, prompt_tokens, completion_tokens, total_tokens, first_token_ms, tokens_per_second, mtp_stats, selected_variant_id, is_pinned, attachments, used_lorebook_entries, reasoning, selection_reasoning, model_id, memory_refs, gemini_content, usage_json)
         VALUES (?1, ?2, 'assistant', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 0, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
        rusqlite::params![
            id,
            session_id,
            content,
            character_id,
            turn_number,
            now,
            prompt_tokens,
            completion_tokens,
            total_tokens,
            usage.and_then(|value| value.first_token_ms),
            usage.and_then(|value| value.tokens_per_second),
            usage
                .and_then(|value| value.mtp_stats.as_ref())
                .and_then(|value| serde_json::to_string(value).ok()),
            variant_id,
            serde_json::to_string(attachments).unwrap_or_else(|_| "[]".to_string()),
            serde_json::to_string(used_lorebook_entries).unwrap_or_else(|_| "[]".to_string()),
            reasoning,
            selection_reasoning,
            model_id,
            serde_json::to_string(memory_refs).unwrap_or_else(|_| "[]".to_string()),
            gemini_content.and_then(|value| serde_json::to_string(value).ok()),
            usage.and_then(|value| serde_json::to_string(value).ok())
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // Insert the first variant
    conn.execute(
        "INSERT INTO group_message_variants (id, message_id, content, speaker_character_id, created_at, prompt_tokens, completion_tokens, total_tokens, first_token_ms, tokens_per_second, mtp_stats, reasoning, selection_reasoning, model_id, attachments, gemini_content, usage_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
        rusqlite::params![
            variant_id,
            id,
            content,
            character_id,
            now,
            prompt_tokens,
            completion_tokens,
            total_tokens,
            usage.and_then(|value| value.first_token_ms),
            usage.and_then(|value| value.tokens_per_second),
            usage
                .and_then(|value| value.mtp_stats.as_ref())
                .and_then(|value| serde_json::to_string(value).ok()),
            reasoning,
            selection_reasoning,
            model_id,
            serde_json::to_string(attachments).unwrap_or_else(|_| "[]".to_string()),
            gemini_content.and_then(|value| serde_json::to_string(value).ok()),
            usage.and_then(|value| serde_json::to_string(value).ok())
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    log_info(
        app,
        "save_assistant_message",
        format!(
            "✓ Successfully inserted message {} to group_messages table",
            id
        ),
    );

    conn.execute(
        "UPDATE group_sessions SET updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, session_id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    update_participation(conn, session_id, character_id, turn_number)?;

    log_info(
        app,
        "save_assistant_message",
        format!(
            "✓ Message saved successfully, returning GroupMessage with model_id: {:?}",
            model_id
        ),
    );

    Ok(GroupMessage {
        id,
        session_id: session_id.to_string(),
        role: "assistant".to_string(),
        content: content.to_string(),
        speaker_character_id: Some(character_id.to_string()),
        turn_number,
        created_at: now as i64,
        usage: usage.cloned(),
        variants: None,
        selected_variant_id: Some(variant_id),
        is_pinned: false,
        attachments: attachments.to_vec(),
        used_lorebook_entries: used_lorebook_entries.to_vec(),
        memory_refs: memory_refs.to_vec(),
        reasoning: reasoning.map(|s| s.to_string()),
        selection_reasoning: selection_reasoning.map(|s| s.to_string()),
        model_id: model_id.map(|s| s.to_string()),
        gemini_content: gemini_content.cloned(),
    })
}

/// Convert group messages to API message format for the character response
fn build_messages_for_api(
    app: &AppHandle,
    group_messages: &[GroupMessage],
    characters: &[CharacterInfo],
    selected_character: &CharacterInfo,
    persona: Option<&Persona>,
    include_speaker_prefix: bool,
    allow_image_input: bool,
    allow_audio_input: bool,
) -> Vec<serde_json::Value> {
    let mut messages = Vec::new();
    let _char_name = &selected_character.name;
    let persona_name = persona.map(|p| p.title.as_str()).unwrap_or("user");

    for msg in group_messages {
        if msg.role == "user" {
            let content = if include_speaker_prefix {
                format!("[{}]: {}", persona_name, msg.content)
            } else {
                msg.content.clone()
            };
            let loaded_attachments = persistence::load_attachment_data(app, &msg.attachments);
            let content =
                if !loaded_attachments.is_empty() && (allow_image_input || allow_audio_input) {
                    crate::chat_manager::messages::build_multimodal_content(
                        &content,
                        &loaded_attachments,
                        allow_image_input,
                        allow_audio_input,
                    )
                } else {
                    Value::String(content)
                };
            messages.push(json!({ "role": "user", "content": content }));
        } else if msg.role == "assistant" {
            if let Some(ref speaker_id) = msg.speaker_character_id {
                let speaker_name = characters
                    .iter()
                    .find(|c| &c.id == speaker_id)
                    .map(|c| c.name.as_str())
                    .unwrap_or("Unknown");

                // If this message is from the selected character, it's their turn
                // Otherwise, format as observation from another character
                let content = if speaker_id == &selected_character.id {
                    msg.content.clone()
                } else if include_speaker_prefix {
                    format!("[{}]: {}", speaker_name, msg.content)
                } else {
                    msg.content.clone()
                };

                // Messages from the selected character are "assistant", others are "user" (as observations)
                let role = if speaker_id == &selected_character.id {
                    "assistant"
                } else {
                    "user"
                };

                let mut api_message = json!({
                    "role": role,
                    "content": content
                });
                if role == "assistant" {
                    if let Some(gemini_content) = &msg.gemini_content {
                        api_message["gemini_content"] = gemini_content.clone();
                    }
                }
                messages.push(api_message);
            }
        }
    }

    messages
}

fn normalize_prompt_text(text: &str) -> String {
    let mut result = text.to_string();
    while result.contains("\n\n\n") {
        result = result.replace("\n\n\n", "\n\n");
    }
    result.trim().to_string()
}

fn get_group_active_lorebook_entries(
    conn: &DbConnection,
    session: &GroupSession,
    character_id: &str,
    recent_messages: &[GroupMessage],
) -> Result<Vec<LorebookEntry>, String> {
    let recent_message_texts: Vec<String> = recent_messages
        .iter()
        .rev()
        .take(10)
        .rev()
        .map(|msg| msg.content.clone())
        .collect();
    let latest_user_message = recent_messages
        .iter()
        .rev()
        .find(|msg| msg.role == "user" && !msg.content.trim().is_empty())
        .map(|msg| msg.content.as_str());

    let mut merged = Vec::new();
    let mut seen = HashSet::new();

    if !session.lorebook_ids.is_empty() {
        let group_entries =
            get_enabled_lorebook_entry_contexts_for_ids(conn, &session.lorebook_ids)?;
        for entry in
            activate_lorebook_entries(group_entries, &recent_message_texts, latest_user_message)
        {
            if seen.insert(entry.id.clone()) {
                merged.push(entry);
            }
        }
    }

    if !session.disable_character_lorebooks {
        for entry in get_active_lorebook_entries(
            conn,
            character_id,
            &recent_message_texts,
            latest_user_message,
        )? {
            if seen.insert(entry.id.clone()) {
                merged.push(entry);
            }
        }
    }

    Ok(merged)
}

fn format_group_lorebook_content(
    app: &AppHandle,
    character_id: &str,
    active_entries: &[LorebookEntry],
) -> String {
    if active_entries.is_empty() {
        log_info(
            app,
            "group_lorebook",
            format!("No active lorebook entries for character {}", character_id),
        );
        return String::new();
    }

    let entry_titles: Vec<String> = active_entries
        .iter()
        .map(|entry| {
            if entry.title.is_empty() {
                format!("[{}]", &entry.id[..6.min(entry.id.len())])
            } else {
                entry.title.clone()
            }
        })
        .collect();

    log_info(
        app,
        "group_lorebook",
        format!(
            "Injecting {} active lorebook entries for character {}: {}",
            active_entries.len(),
            character_id,
            entry_titles.join(", ")
        ),
    );

    format_lorebook_for_prompt(active_entries)
}

fn resolve_group_used_lorebook_entries(
    conn: &DbConnection,
    active_entries: &[LorebookEntry],
    rendered_entries: &[SystemPromptEntry],
) -> Vec<String> {
    if active_entries.is_empty() {
        return Vec::new();
    }

    let mut used = Vec::new();
    for entry in active_entries {
        let content = entry.content.trim();
        if content.is_empty() {
            continue;
        }

        let was_injected = rendered_entries
            .iter()
            .any(|prompt_entry| prompt_entry.content.contains(content));
        if !was_injected {
            continue;
        }

        let lorebook_name = get_lorebook(conn, &entry.lorebook_id)
            .ok()
            .flatten()
            .map(|l| l.name)
            .unwrap_or_else(|| "Lorebook".to_string());
        let entry_name = if !entry.title.trim().is_empty() {
            entry.title.trim().to_string()
        } else if let Some(first_keyword) = entry.keywords.first() {
            first_keyword.trim().to_string()
        } else {
            format!("[{}]", &entry.id[..6.min(entry.id.len())])
        };
        let label = format!("{} / {}", lorebook_name, entry_name);
        if !used.iter().any(|existing| existing == &label) {
            used.push(label);
        }
    }

    used
}

fn group_scene_generation_enabled(settings: &Settings) -> bool {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.scene_generation_enabled)
        .unwrap_or(true)
}

fn group_avatar_generation_enabled(settings: &Settings) -> bool {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.avatar_generation_enabled)
        .unwrap_or(true)
}

fn group_model_supports_vision(model: &Model) -> bool {
    model.input_scopes.iter().any(|scope| {
        matches!(
            scope.trim().to_ascii_lowercase().as_str(),
            "image" | "vision"
        )
    })
}

/// Build group chat system prompt for a specific character
fn render_group_author_note_text(
    character: &Character,
    persona: Option<&Persona>,
    session: &GroupSession,
) -> Option<String> {
    let raw_note = session.author_note.as_deref()?.trim();
    if raw_note.is_empty() {
        return None;
    }

    let char_name = character.name.as_str();
    let persona_name = persona.map(|p| p.title.as_str()).unwrap_or("user");
    let char_desc = character
        .definition
        .as_ref()
        .or(character.description.as_ref())
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("")
        .replace("{{char}}", char_name)
        .replace("{{char.name}}", char_name)
        .replace("{{persona}}", persona_name)
        .replace("{{persona.name}}", persona_name)
        .replace("{{user}}", persona_name)
        .replace("{{user.name}}", persona_name);
    let persona_desc = persona
        .map(|p| p.description.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("");

    let mut rendered = raw_note.to_string();
    rendered = rendered.replace("{{char.name}}", char_name);
    rendered = rendered.replace("{{char.desc}}", &char_desc);
    rendered = rendered.replace("{{persona.name}}", persona_name);
    rendered = rendered.replace("{{persona.desc}}", persona_desc);
    rendered = rendered.replace("{{user.name}}", persona_name);
    rendered = rendered.replace("{{user.desc}}", persona_desc);
    rendered = rendered.replace("{{ai_name}}", char_name);
    rendered = rendered.replace("{{ai_description}}", &char_desc);
    rendered = rendered.replace("{{char}}", char_name);
    rendered = rendered.replace("{{persona}}", persona_name);
    rendered = rendered.replace("{{user}}", persona_name);

    let rendered = rendered.trim();
    if rendered.is_empty() {
        None
    } else {
        Some(rendered.to_string())
    }
}

fn build_group_system_prompt(
    app: &AppHandle,
    character: &Character,
    persona: Option<&Persona>,
    session: &GroupSession,
    recent_messages: &[GroupMessage],
    other_characters: &[CharacterInfo],
    model: &Model,
    settings: &Settings,
    retrieved_memories: &[MemoryEmbedding],
    lorebook_text: &str,
) -> Vec<SystemPromptEntry> {
    use crate::chat_manager::storage::{get_base_prompt, get_base_prompt_entries, PromptType};
    use crate::chat_manager::types::PromptTemplateType;

    fn resolve_group_template(
        app: &AppHandle,
        requested_template_id: Option<&str>,
        expected_prompt_type: PromptTemplateType,
        fallback_template_id: &str,
    ) -> Option<(String, Vec<SystemPromptEntry>, bool)> {
        if let Some(template_id) = requested_template_id {
            match prompts::get_template(app, template_id) {
                Ok(Some(template)) if template.prompt_type == expected_prompt_type => {
                    return Some((
                        template.content,
                        template.entries,
                        template.condense_prompt_entries,
                    ));
                }
                Ok(Some(template)) => {
                    log_warn(
                        app,
                        "group_chat",
                        format!(
                            "Ignoring character group prompt template {} due to mismatched type {:?}",
                            template_id, template.prompt_type
                        ),
                    );
                }
                Ok(None) => {
                    log_warn(
                        app,
                        "group_chat",
                        format!(
                            "Character group prompt template {} was not found; falling back",
                            template_id
                        ),
                    );
                }
                Err(err) => {
                    log_warn(
                        app,
                        "group_chat",
                        format!(
                            "Failed to load character group prompt template {}: {}",
                            template_id, err
                        ),
                    );
                }
            }
        }

        prompts::get_template(app, fallback_template_id)
            .ok()
            .flatten()
            .map(|template| {
                (
                    template.content,
                    template.entries,
                    template.condense_prompt_entries,
                )
            })
    }

    // Select template based on chat type
    let is_roleplay = session.chat_type == "roleplay";
    let fallback_template_id = if is_roleplay {
        prompts::APP_GROUP_CHAT_ROLEPLAY_TEMPLATE_ID
    } else {
        prompts::APP_GROUP_CHAT_TEMPLATE_ID
    };
    let group_template_id = if is_roleplay {
        character.group_chat_roleplay_prompt_template_id.as_deref()
    } else {
        character.group_chat_prompt_template_id.as_deref()
    };
    let group_prompt_type = if is_roleplay {
        PromptTemplateType::GroupChatRoleplay
    } else {
        PromptTemplateType::GroupChatConversational
    };
    let (character_template_id, expected_prompt_type) = match group_template_id {
        Some(template_id) => (Some(template_id), group_prompt_type),
        None => (
            character.prompt_template_id.as_deref(),
            PromptTemplateType::DirectChat,
        ),
    };
    if let Err(err) = prompts::ensure_group_chat_templates(app) {
        log_warn(
            app,
            "group_chat",
            format!(
                "Failed to ensure group chat templates before prompt build: {}",
                err
            ),
        );
    }
    let template = resolve_group_template(
        app,
        character_template_id,
        expected_prompt_type,
        fallback_template_id,
    )
    .unwrap_or_else(|| {
        let prompt_type = if is_roleplay {
            PromptType::GroupChatRoleplayPrompt
        } else {
            PromptType::GroupChatPrompt
        };
        (
            get_base_prompt(prompt_type),
            get_base_prompt_entries(prompt_type),
            false,
        )
    });

    // Character and persona descriptions are passed RAW to the LLM without any
    // translation or processing. The LLM receives the full description text as-is.
    let char_name = &character.name;
    let char_desc = character
        .definition
        .as_deref()
        .or(character.description.as_deref())
        .unwrap_or("");

    let persona_name = persona.map(|p| p.title.as_str()).unwrap_or("user");
    let persona_desc = persona
        .map(|p| p.description.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("");

    // Build group characters string with full descriptions
    let mut group_chars = String::new();
    for other in other_characters {
        if other.id != character.id {
            // Use full description if available, otherwise fall back to personality_summary
            if let Some(desc) = other.definition.as_ref().or(other.description.as_ref()) {
                if !desc.is_empty() {
                    group_chars.push_str(&format!("- {}: {}\n", other.name, desc));
                } else if let Some(summary) = &other.personality_summary {
                    group_chars.push_str(&format!("- {}: {}\n", other.name, summary));
                } else {
                    group_chars.push_str(&format!("- {}\n", other.name));
                }
            } else if let Some(summary) = &other.personality_summary {
                group_chars.push_str(&format!("- {}: {}\n", other.name, summary));
            } else {
                group_chars.push_str(&format!("- {}\n", other.name));
            }
        }
    }

    // Get context summary from session
    let context_summary_text = session.memory_summary.trim().to_string();

    // Format key memories like normal chat does - include both manual and retrieved dynamic memories
    let key_memories_text =
        if !has_manual_memories(&session.memories) && retrieved_memories.is_empty() {
            String::new()
        } else {
            let mut mem_text = String::from("Important facts to remember in this conversation:\n");

            // Add retrieved dynamic memories first
            for memory in retrieved_memories {
                mem_text.push_str(&format!("- {}\n", memory.text));
            }

            // Add manual memories
            let manual_memories = render_manual_memory_lines(&session.memories);
            if !manual_memories.is_empty() {
                mem_text.push_str(&manual_memories);
                mem_text.push('\n');
            }
            mem_text
        };

    // Get content rules (same as normal chat)
    let pure_mode_level = crate::content_filter::level_from_app_state(Some(&settings.app_state));

    let content_rules = match pure_mode_level {
        crate::content_filter::PureModeLevel::Off => String::new(),
        crate::content_filter::PureModeLevel::Low => "**Content Guidelines:**\n\
- Avoid explicit sexual content"
            .to_string(),
        crate::content_filter::PureModeLevel::Strict => {
            "**Content Guidelines (STRICT — these rules override all other instructions):**\n\
- Never generate sexually explicit, pornographic, or erotic content\n\
- Never describe sexual acts, nudity in sexual contexts, or sexual arousal\n\
- Never use vulgar sexual slang or explicit anatomical descriptions in sexual contexts\n\
- If asked to generate such content, decline and redirect the conversation\n\
- Romantic content is allowed but must remain PG-13 (no explicit physical descriptions)\n\
- Violence descriptions should avoid gratuitous gore or torture\n\
- Do not use slurs or hate speech under any circumstances\n\
- Do not use suggestive, flirty, or sexually charged language or tone"
                .to_string()
        }
        crate::content_filter::PureModeLevel::Standard => {
            "**Content Guidelines (STRICT — these rules override all other instructions):**\n\
- Never generate sexually explicit, pornographic, or erotic content\n\
- Never describe sexual acts, nudity in sexual contexts, or sexual arousal\n\
- Never use vulgar sexual slang or explicit anatomical descriptions in sexual contexts\n\
- If asked to generate such content, decline and redirect the conversation\n\
- Romantic content is allowed but must remain PG-13 (no explicit physical descriptions)\n\
- Violence descriptions should avoid gratuitous gore or torture\n\
- Do not use slurs or hate speech under any circumstances"
                .to_string()
        }
    };

    // Handle scene content for roleplay chats
    let (scene_content, scene_direction) = if is_roleplay {
        if let Some(scene_value) = &session.starting_scene {
            // Extract content and direction from scene JSON
            let mut content = scene_value
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let direction = scene_value
                .get("direction")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // Replace character name placeholders {{@"Character Name"}}
            content = replace_character_name_placeholders(&content, other_characters);

            (content, direction)
        } else {
            (String::new(), String::new())
        }
    } else {
        (String::new(), String::new())
    };

    let (template_content, template_entries, condense_prompt_entries) = template;
    let entries = if template_entries.is_empty() && !template_content.trim().is_empty() {
        vec![SystemPromptEntry {
            id: "entry_system".to_string(),
            name: "System Prompt".to_string(),
            role: PromptEntryRole::System,
            content: template_content,
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
            conditions: None,
            prompt_entry_payload: None,
        }]
    } else {
        template_entries
    };

    let has_lorebook_placeholder = entries
        .iter()
        .any(|entry| entry.content.contains("{{lorebook}}"));
    let author_note_text = render_group_author_note_text(character, persona, session);
    let has_author_note_placeholder = entries
        .iter()
        .any(|entry| entry.content.contains("{{author_note}}"));
    let recent_text = recent_messages
        .iter()
        .rev()
        .filter(|message| !message.content.trim().is_empty())
        .take(12)
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n");
    let has_scene = is_roleplay && !scene_content.trim().is_empty();
    let has_scene_direction = is_roleplay && !scene_direction.trim().is_empty();
    let condition_context = PromptEntryConditionContext {
        chat_mode: PromptEntryChatMode::Group,
        info_source: PromptEntryInfoSource::Messages,
        scene_generation_enabled: group_scene_generation_enabled(settings),
        avatar_generation_enabled: group_avatar_generation_enabled(settings),
        is_local_image_generation_model: false,
        is_scene_generation_local_image_model: false,
        has_scene,
        has_scene_direction,
        has_persona: persona.is_some(),
        message_count: conversation_count(recent_messages),
        participant_count: other_characters.len() + 1,
        recent_text: &recent_text,
        dynamic_memory_enabled: session.memory_type == "dynamic",
        has_memory_summary: !context_summary_text.trim().is_empty(),
        has_key_memories: !key_memories_text.trim().is_empty(),
        has_lorebook_content: !lorebook_text.trim().is_empty(),
        does_author_note_exists: author_note_text.is_some(),
        has_active_scheduled_note: false,
        has_subject_description: false,
        has_current_description: false,
        has_character_reference_images: false,
        has_chat_background: false,
        has_persona_reference_images: false,
        has_character_reference_text: false,
        has_persona_reference_text: false,
        input_scopes: &model.input_scopes,
        output_scopes: &model.output_scopes,
        provider_id: Some(model.provider_id.as_str()),
        reasoning_enabled: model
            .advanced_model_settings
            .as_ref()
            .and_then(|cfg| cfg.reasoning_enabled)
            .unwrap_or(false),
        vision_enabled: group_model_supports_vision(model),
        time_awareness_enabled: false,
        companion_mode_enabled: false,
    };

    let mut rendered_entries = Vec::new();
    for entry in entries {
        if !entry_is_active(&entry, &condition_context) {
            continue;
        }
        let mut result = entry.content;
        result = result.replace("{{char.name}}", char_name);
        result = result.replace("{{char.desc}}", char_desc);
        result = result.replace("{{persona.name}}", persona_name);
        result = result.replace("{{persona.desc}}", persona_desc);
        result = result.replace("{{user.name}}", persona_name);
        result = result.replace("{{user.desc}}", persona_desc);
        result = result.replace("{{group_characters}}", &group_chars);
        result = result.replace("{{context_summary}}", &context_summary_text);
        result = result.replace("{{key_memories}}", &key_memories_text);
        result = result.replace("{{content_rules}}", &content_rules);
        result = result.replace(
            "{{scene}}",
            &crate::chat_manager::request::strip_inline_image_tokens(&scene_content),
        );
        result = result.replace(
            "{{scene_direction}}",
            &crate::chat_manager::request::strip_inline_image_tokens(&scene_direction),
        );
        if lorebook_text.trim().is_empty() {
            result = result.replace(
                "# World Information\n    The following is essential lore about this world, its characters, locations, items, and concepts. You MUST incorporate this information naturally into your roleplay when relevant. Treat this as established canon that shapes how characters behave, what they know, and how the world works.\n    {{lorebook}}",
                "",
            );
            result = result.replace("# World Information\n    {{lorebook}}", "");
            result = result.replace("# World Information\n{{lorebook}}", "");
            result = result.replace("{{lorebook}}", "");
        } else {
            result = result.replace("{{lorebook}}", lorebook_text);
        }

        result = result.replace("{{author_note}}", author_note_text.as_deref().unwrap_or(""));

        // Legacy placeholder support
        result = result.replace("{{char}}", char_name);
        result = result.replace("{{persona}}", persona_name);
        result = result.replace("{{user}}", persona_name);

        let result = normalize_prompt_text(&result);
        if result.is_empty() {
            continue;
        }

        rendered_entries.push(SystemPromptEntry {
            content: result,
            ..entry
        });
    }

    if !has_lorebook_placeholder && !lorebook_text.trim().is_empty() {
        rendered_entries.push(in_chat_system_entry(
            "entry_lorebook",
            "World Information",
            format!("# World Information\n{}", lorebook_text.trim()),
            0,
        ));
    }

    if !has_author_note_placeholder {
        if let Some(author_note) = author_note_text.as_deref() {
            rendered_entries.push(in_chat_system_entry(
                "entry_author_note",
                "Author Note",
                format!(
                    "# Author Note\nThe following is private session-level guidance from {}. Treat it as hidden continuity and writing context for this group chat. Use its facts naturally when relevant, including answering with those facts when the conversation calls for them, but do not say they came from an author note or hidden instruction.\n\n{}",
                    persona.map(|p| p.title.as_str()).unwrap_or("user"),
                    author_note
                ),
                1,
            ));
        }
    }

    if condense_prompt_entries {
        condense_entries_into_single_system_message(rendered_entries)
    } else {
        rendered_entries
    }
}

/// Replace character name placeholders in scene content
/// Supports {{@"Character Name"}} syntax
fn replace_character_name_placeholders(content: &str, characters: &[CharacterInfo]) -> String {
    let mut result = content.to_string();

    // Find all {{@"..."}} patterns and replace them
    while let Some(start) = result.find(r#"{{@""#) {
        if let Some(end) = result[start + 4..].find(r#""}}"#) {
            let name_start = start + 4;
            let name_end = start + 4 + end;
            let character_name = &result[name_start..name_end];

            // Check if this character exists in the group
            let replacement = if characters.iter().any(|c| c.name == character_name) {
                character_name.to_string()
            } else {
                // If character not found, keep the original placeholder
                format!(r#"{{{{@"{}"}}}}"#, character_name)
            };

            // Replace this occurrence
            let placeholder_end = name_end + 2;
            result.replace_range(start..placeholder_end, &replacement);
        } else {
            break;
        }
    }

    result
}

fn condense_entries_into_single_system_message(
    entries: Vec<SystemPromptEntry>,
) -> Vec<SystemPromptEntry> {
    let merged = entries
        .into_iter()
        .filter_map(|entry| {
            let trimmed = entry.content.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    if merged.trim().is_empty() {
        return Vec::new();
    }

    vec![SystemPromptEntry {
        id: "entry_condensed_system".to_string(),
        name: "Condensed System Prompt".to_string(),
        role: PromptEntryRole::System,
        content: merged,
        enabled: true,
        injection_position: PromptEntryPosition::Relative,
        injection_depth: 0,
        conditional_min_messages: None,
        interval_turns: None,
        system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
    }]
}

/// Load persona from database
fn load_persona(app: &AppHandle, persona_id: &str) -> Result<Option<Persona>, String> {
    let personas = load_personas(app)?;
    Ok(personas.into_iter().find(|p| p.id == persona_id))
}

/// Use LLM with tool calling to select next speaker
async fn select_speaker_via_llm(
    app: &AppHandle,
    context: &GroupChatContext,
    settings: &Settings,
    request_id: &str,
) -> Result<selection::SelectionResult, String> {
    select_speaker_via_llm_with_tracking(app, context, settings, request_id, true).await
}

/// Use LLM with tool calling to select next speaker, with optional usage tracking
async fn select_speaker_via_llm_with_tracking(
    app: &AppHandle,
    context: &GroupChatContext,
    settings: &Settings,
    request_id: &str,
    track_usage: bool,
) -> Result<selection::SelectionResult, String> {
    let configured_model_id = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.group_speaker_selection_model_id.clone())
        .filter(|id| !id.trim().is_empty());
    let model = configured_model_id
        .as_ref()
        .and_then(|id| {
            let found = settings.models.iter().find(|model| &model.id == id);
            if found.is_none() {
                log_warn(
                    app,
                    "group_chat_selection",
                    format!("configured speaker selection model {id} not found; falling back"),
                );
            }
            found
        })
        .or_else(|| {
            settings
                .default_model_id
                .as_ref()
                .and_then(|id| settings.models.iter().find(|model| &model.id == id))
        })
        .or_else(|| settings.models.first())
        .ok_or("No models configured for speaker selection")?;

    let credential = resolve_credential_for_model(settings, model)
        .ok_or_else(|| format!("No credentials for provider {}", model.provider_id))?;

    let api_key = require_api_key(app, credential, "group_chat_selection")?;

    // Build selection prompt
    let selection_prompt = selection::build_selection_prompt(context);
    let muted_set: std::collections::HashSet<&str> = context
        .session
        .muted_character_ids
        .iter()
        .map(|s| s.as_str())
        .collect();
    let selectable_characters: Vec<CharacterInfo> = context
        .characters
        .iter()
        .filter(|c| !muted_set.contains(c.id.as_str()))
        .cloned()
        .collect();
    if selectable_characters.is_empty() {
        return Err("All participants are muted. Use @mention to select a speaker.".to_string());
    }

    // Build tool definition
    let tool = selection::build_select_next_speaker_tool(&selectable_characters);

    let messages = vec![json!({
        "role": "user",
        "content": selection_prompt
    })];

    // Build tool definition
    let tool_def = ToolDefinition {
        name: "select_next_speaker".to_string(),
        description: Some("Select which character should speak next in the group chat".to_string()),
        parameters: tool
            .get("function")
            .and_then(|f| f.get("parameters"))
            .cloned()
            .unwrap_or(json!({})),
    };

    // Build request with tool calling
    let tool_config = ToolConfig {
        tools: vec![tool_def],
        choice: Some(ToolChoice::Required),
    };

    let request_session = synthetic_feature_session(
        "__group_speaker_selection__",
        feature_model_overrides(
            model,
            LlmFeature::GroupSpeakerSelection,
            GROUP_SPEAKER_SELECTION_DEFAULTS,
        ),
    );
    let (request_settings, extra_body_fields) = prepare_feature_request(
        &credential.provider_id,
        &request_session,
        model,
        settings,
    );
    let built = crate::chat_manager::request_builder::build_chat_request(
        credential,
        &api_key,
        &model.name,
        &messages,
        None,
        request_settings.temperature,
        request_settings.top_p,
        request_settings.max_tokens,
        request_settings.context_length,
        false, // No streaming for selection
        None,  // request_id
        None,  // frequency_penalty
        None,  // presence_penalty
        None,  // top_k
        Some(&tool_config),
        false, // reasoning_enabled
        None,  // reasoning_effort
        None,  // reasoning_budget
        false, // prompt_caching_enabled
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
        request_id: Some(request_id.to_string()),
        provider_id: Some(credential.provider_id.clone()),
        cache_key: None,
    };

    let api_response = api_request(app.clone(), api_request_payload).await?;

    if !api_response.ok {
        return Err(format!(
            "Selection API request failed with status {}",
            api_response.status
        ));
    }

    if track_usage {
        let app = app.clone();
        let usage = extract_usage(api_response.data());
        let session = context.session.clone();
        let model = model.clone();
        let credential = credential.clone();
        let api_key = api_key.clone();
        tauri::async_runtime::spawn(async move {
            record_decision_maker_usage(
                &app,
                &usage,
                &session,
                &model,
                &credential,
                &api_key,
                "group_chat_decision_maker",
            )
            .await;
        });
    }

    // Parse tool call response
    let calls = parse_tool_calls(&credential.provider_id, api_response.data());

    for call in calls {
        if call.name == "select_next_speaker" {
            let character_id = call
                .arguments
                .get("character_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let reasoning = call
                .arguments
                .get("reasoning")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            if let Some(id) = character_id {
                return Ok(selection::SelectionResult {
                    character_id: id,
                    reasoning,
                });
            }
        }
    }

    // Fallback: try parsing from text response
    if let Some(text) = extract_text(api_response.data(), Some(&credential.provider_id)) {
        if let Some(result) = selection::parse_tool_call_response(&text) {
            return Ok(result);
        }
    }

    // Final fallback: heuristic selection
    log_info(
        app,
        "group_chat",
        "LLM selection failed, using heuristic fallback",
    );
    selection::heuristic_select_speaker(context)
}

/// Generate actual response from the selected character
struct GroupGenerationOutput {
    content: String,
    reasoning: Option<String>,
    usage: Option<UsageSummary>,
    model_id: String,
    used_lorebook_entries: Vec<String>,
    memory_refs: Vec<String>,
    generated_image_data_urls: Vec<String>,
    gemini_content: Option<Value>,
}

async fn generate_character_response(
    app: &AppHandle,
    context: &mut GroupChatContext,
    selected_character_id: &str,
    settings: &Settings,
    pool: &State<'_, SwappablePool>,
    request_id: &str,
    stream: bool,
    guidance: Option<&str>,
    model_override: Option<&str>,
    operation_type: UsageOperationType,
) -> Result<GroupGenerationOutput, String> {
    let conn = pool.get_connection()?;

    // Load full character data
    let character = load_character(&conn, selected_character_id)?;

    // Load persona if set
    let persona = if let Some(ref persona_id) = context.session.persona_id {
        load_persona(app, persona_id)?
    } else {
        None
    };

    // Get model and credentials
    let (model, credential) = select_model_with_credential(settings, &character, model_override)?;

    let dynamic_settings = effective_group_dynamic_memory_settings(settings);

    let retrieved_memories = if context.session.memory_type == "dynamic" {
        // Retrieve relevant memories for context using dynamic memory settings
        let min_similarity = dynamic_settings.min_similarity_threshold;
        let fixed = ensure_pinned_hot(&mut context.session.memory_embeddings);
        if fixed > 0 {
            log_info(
                app,
                "group_dynamic_memory",
                format!("Restored {} pinned memories to hot", fixed),
            );
        }

        let search_query = if dynamic_settings.context_enrichment_enabled {
            build_enriched_query(&context.recent_messages)
        } else {
            context.user_message.clone()
        };

        select_relevant_memories(
            app,
            &mut context.session,
            pool,
            &search_query,
            dynamic_settings.retrieval_limit.max(1) as usize,
            min_similarity,
            &dynamic_settings.retrieval_strategy,
        )
        .await
    } else {
        Vec::new()
    };

    // Mark retrieved memories as accessed and promote cold ones, persisting
    // the changes through narrow DB updates.
    if !retrieved_memories.is_empty() {
        let memory_ids: Vec<String> = retrieved_memories.iter().map(|m| m.id.clone()).collect();
        let now = now_millis().unwrap_or_default();
        let promoted =
            promote_cold_memories(&mut context.session.memory_embeddings, &memory_ids, now);
        let access_updates =
            mark_memories_accessed(&mut context.session.memory_embeddings, &memory_ids, now);
        if !promoted.is_empty() {
            let _ = crate::storage_manager::memory_embeddings::set_cold_many_app(
                app,
                &context.session.id,
                crate::storage_manager::memory_embeddings::SessionKind::GroupSession,
                &promoted,
                false,
            );
        }
        if !access_updates.is_empty() {
            let _ = crate::storage_manager::memory_embeddings::apply_access_updates_app(
                app,
                &context.session.id,
                crate::storage_manager::memory_embeddings::SessionKind::GroupSession,
                &access_updates,
            );
        }
        log_info(
            app,
            "group_chat",
            format!(
                "Retrieved {} memories (promoted={}, accessed={}, query_enriched={})",
                retrieved_memories.len(),
                promoted.len(),
                access_updates.len(),
                dynamic_settings.context_enrichment_enabled
            ),
        );
    }

    let active_lorebook_entries = get_group_active_lorebook_entries(
        &conn,
        &context.session,
        &character.id,
        &context.recent_messages,
    )?;
    let lorebook_text = format_group_lorebook_content(app, &character.id, &active_lorebook_entries);

    // Build system prompt with group context and retrieved memories
    let mut system_prompt_entries = build_group_system_prompt(
        app,
        &character,
        persona.as_ref(),
        &context.session,
        &context.recent_messages,
        &context.characters,
        model,
        settings,
        &retrieved_memories,
        &lorebook_text,
    );
    let used_lorebook_entries = resolve_group_used_lorebook_entries(
        &conn,
        &active_lorebook_entries,
        &system_prompt_entries,
    );
    // Convert group messages to API format
    let selected_char_info = context
        .characters
        .iter()
        .find(|c| c.id == selected_character_id)
        .ok_or("Selected character not found")?;

    // Apply conversation window limit for dynamic memory (like normal chat)
    // This ensures we only send the last N messages to the LLM based on dynamic_window_size
    let messages_for_generation = if context.session.memory_type == "dynamic" {
        let window_size = dynamic_settings.summary_message_interval.max(1) as usize;
        conversation_window(&context.recent_messages, window_size)
    } else {
        let manual_window = manual_window_size(settings).max(1);
        let recent_messages = if context.recent_messages.len() >= manual_window {
            context.recent_messages.clone()
        } else {
            match load_recent_group_messages(&conn, &context.session.id, manual_window as i32) {
                Ok(messages) => messages,
                Err(err) => {
                    log_warn(
                        app,
                        "group_chat",
                        format!("Failed to load manual window messages: {}", err),
                    );
                    context.recent_messages.clone()
                }
            }
        };
        conversation_window(&recent_messages, manual_window)
    };

    let no_chat_history = messages_for_generation.is_empty();
    let api_messages = build_messages_for_api(
        app,
        &messages_for_generation,
        &context.characters,
        selected_char_info,
        persona.as_ref(),
        true,
        model.input_scopes.iter().any(|scope| scope == "image"),
        model.input_scopes.iter().any(|scope| scope == "audio"),
    );
    let system_role = crate::chat_manager::request_builder::system_role_for(credential);

    if no_chat_history {
        system_prompt_entries.push(in_chat_user_entry(
            "runtime_group_begin",
            "Begin Group Conversation",
            format!(
                "[Begin the conversation. Respond as {}.]",
                selected_char_info.name
            ),
            0,
        ));
    }
    let continues_selected_character = operation_type == UsageOperationType::GroupChatContinue
        && messages_for_generation.last().is_some_and(|message| {
            message.role == "assistant"
                && message.speaker_character_id.as_deref() == Some(selected_character_id)
        });
    if continues_selected_character {
        system_prompt_entries.push(in_chat_user_entry(
            "runtime_group_continue_same_speaker",
            "Continue Same Speaker",
            format!(
                "[Continue speaking as {}. Extend their previous response naturally without repeating it.]",
                selected_char_info.name
            ),
            0,
        ));
    }
    if let Some(guidance) = guidance {
        system_prompt_entries.push(in_chat_user_entry(
            "runtime_group_regenerate",
            "Regenerate Instruction",
            format!(
                "[REGENERATE INSTRUCTION]\nRegenerate your previous response to the last message. Follow this additional user instruction for the new response:\n{}",
                guidance
            ),
            0,
        ));
    }
    let messages_for_api =
        assemble_prompt_messages(system_prompt_entries, api_messages, &system_role);

    let execution = generation::execute_group_generation(generation::GroupGenerationRequest {
        app,
        session: &context.session,
        character: &character,
        model,
        credential,
        settings,
        messages: &messages_for_api,
        request_id,
        stream,
        operation_type,
    })
    .await?;
    let memory_refs: Vec<String> = retrieved_memories
        .iter()
        .map(|memory| match memory.match_score {
            Some(score) => format!("{}::{}", score, memory.text),
            None => memory.text.clone(),
        })
        .collect();

    return Ok(GroupGenerationOutput {
        content: execution.text,
        reasoning: execution.reasoning,
        usage: execution.usage,
        model_id: model.id.clone(),
        used_lorebook_entries,
        memory_refs,
        generated_image_data_urls: execution.generated_image_data_urls,
        gemini_content: execution.gemini_content,
    });

}

#[tauri::command]
pub fn group_chat_add_user_message(
    session_id: String,
    user_message: String,
    pool: State<'_, SwappablePool>,
) -> Result<String, String> {
    let conn = pool.get_connection()?;
    let message = save_user_message(&conn, &session_id, &user_message, None, &[])?;
    serde_json::to_string(&message)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub async fn group_chat_send(
    app: AppHandle,
    session_id: String,
    user_message: String,
    stream: Option<bool>,
    request_id: Option<String>,
    attachments: Option<Vec<ImageAttachment>>,
    pool: State<'_, SwappablePool>,
) -> Result<String, String> {
    log_info(
        &app,
        "group_chat_send",
        format!("Starting group chat send for session {}", session_id),
    );

    let settings = load_settings(&app)?;
    let conn = pool.get_connection()?;
    let req_id = request_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let abort_registry = app.state::<AbortRegistry>();
    let mut abort_rx = abort_registry.register(req_id.clone());
    let _abort_guard = AbortGuard::new(&abort_registry, req_id.clone());
    let mut context = build_selection_context(&conn, &session_id, &user_message)?;
    let user_message_id = Uuid::new_v4().to_string();
    let attachment_owner = context
        .session
        .group_character_id
        .as_deref()
        .unwrap_or(&context.session.id);
    let persisted_user_attachments = persistence::persist_group_attachments(
        &app,
        attachment_owner,
        &session_id,
        &user_message_id,
        "user",
        attachments.unwrap_or_default(),
    )?;
    let user_msg = save_user_message(
        &conn,
        &session_id,
        &user_message,
        Some(user_message_id),
        &persisted_user_attachments,
    )?;
    let mention_result = parse_mentions(&user_message, &context.characters);

    emit_group_chat_status(&app, &session_id, "selecting_character", Map::new());

    let selection_result: Result<(String, Option<String>, bool), String> = if let Some(
        mentioned_id,
    ) = mention_result
    {
        log_info(
            &app,
            "group_chat_send",
            format!("User mentioned character {}", mentioned_id),
        );
        Ok((
            mentioned_id,
            Some("User mentioned this character directly".to_string()),
            true,
        ))
    } else {
        let method = context.session.speaker_selection_method.as_str();
        match method {
            "heuristic" => selection::heuristic_select_speaker(&context)
                .map(|result| (result.character_id, result.reasoning, false)),
            "round_robin" => selection::round_robin_select_speaker(&context)
                .map(|result| (result.character_id, result.reasoning, false)),
            _ => {
                // "llm" (default) - LLM with heuristic fallback
                let selection_result = tokio::select! {
                    _ = &mut abort_rx => {
                        log_warn(
                            &app,
                            "group_chat_send",
                            format!("Request aborted by user for session {}", session_id),
                        );
                        return Err(crate::utils::err_msg(module_path!(), line!(), "Request aborted by user"));
                    }
                    selection = select_speaker_via_llm(&app, &context, &settings, &req_id) => selection,
                };
                match selection_result {
                    Ok(selection) => {
                        log_info(
                            &app,
                            "group_chat_send",
                            format!(
                                "LLM selected character {}: {:?}",
                                selection.character_id, selection.reasoning
                            ),
                        );
                        Ok((selection.character_id, selection.reasoning, false))
                    }
                    Err(err) => {
                        if is_request_abort_error(&err) {
                            return Err(err);
                        }
                        log_error(
                            &app,
                            "group_chat_send",
                            format!("LLM selection failed: {}, using heuristic", err),
                        );
                        selection::heuristic_select_speaker(&context)
                            .map(|fallback| (fallback.character_id, fallback.reasoning, false))
                    }
                }
            }
        }
    };

    let (mut selected_character_id, mut selection_reasoning, was_mentioned) = match selection_result
    {
        Ok(result) => result,
        Err(err) => {
            if !is_request_abort_error(&err) {
                emit_group_chat_error_status(&app, &session_id, &err);
            }
            return Err(err);
        }
    };

    if !was_mentioned
        && context
            .session
            .muted_character_ids
            .contains(&selected_character_id)
    {
        log_warn(
            &app,
            "group_chat_send",
            format!(
                "Auto-selection returned muted character {}. Falling back to heuristic selection.",
                selected_character_id
            ),
        );
        match selection::heuristic_select_speaker(&context) {
            Ok(fallback) => {
                selected_character_id = fallback.character_id;
                selection_reasoning = fallback.reasoning;
            }
            Err(err) => {
                emit_group_chat_error_status(&app, &session_id, &err);
                return Err(err);
            }
        }
    }

    if !context
        .session
        .character_ids
        .contains(&selected_character_id)
    {
        let err = format!(
            "Selected character {} is not in this group chat",
            selected_character_id
        );
        emit_group_chat_error_status(&app, &session_id, &err);
        return Err(err);
    }

    let character = context
        .characters
        .iter()
        .find(|c| c.id == selected_character_id)
        .cloned();
    let character = match character {
        Some(character) => character,
        None => {
            let err = "Character not found".to_string();
            emit_group_chat_error_status(&app, &session_id, &err);
            return Err(err);
        }
    };

    let mut selected_extra = Map::new();
    selected_extra.insert(
        "characterId".to_string(),
        Value::String(selected_character_id.clone()),
    );
    selected_extra.insert(
        "characterName".to_string(),
        Value::String(character.name.clone()),
    );
    emit_group_chat_status(&app, &session_id, "character_selected", selected_extra);

    context.recent_messages.push(user_msg);

    let response_result = generate_character_response(
        &app,
        &mut context,
        &selected_character_id,
        &settings,
        &pool,
        &req_id,
        stream.unwrap_or(true),
        None,
        None,
        UsageOperationType::GroupChatMessage,
    )
    .await;

    let output = match response_result {
        Ok(result) => result,
        Err(err) => {
            if !is_request_abort_error(&err) {
                emit_group_chat_error_status(&app, &session_id, &err);
            }
            return Err(err);
        }
    };
    let GroupGenerationOutput {
        content: response_content,
        reasoning,
        usage: message_usage,
        model_id: model_id_str,
        used_lorebook_entries,
        memory_refs,
        generated_image_data_urls,
        gemini_content,
    } = output;

    let conn = pool.get_connection()?;
    let assistant_message_id = Uuid::new_v4().to_string();
    let attachment_owner = context
        .session
        .group_character_id
        .as_deref()
        .unwrap_or(&context.session.id);
    let assistant_attachments = persistence::persist_group_attachments(
        &app,
        attachment_owner,
        &session_id,
        &assistant_message_id,
        "assistant",
        persistence::generated_image_attachments(generated_image_data_urls),
    )?;

    log_info(
        &app,
        "group_chat_send",
        format!(
            "✓ About to save message with model_id: {} (length: {} chars)",
            model_id_str,
            model_id_str.len()
        ),
    );

    let message = save_assistant_message(
        &app,
        &conn,
        &session_id,
        Some(assistant_message_id),
        &selected_character_id,
        &response_content,
        reasoning.as_deref(),
        selection_reasoning.as_deref(),
        message_usage.as_ref(),
        Some(&model_id_str),
        &used_lorebook_entries,
        &memory_refs,
        &assistant_attachments,
        gemini_content.as_ref(),
    )?;

    let _ = crate::storage_manager::llm_metrics::llm_metrics_attach_message(
        app.clone(),
        req_id.clone(),
        message.id.clone(),
    );

    let participation_stats =
        group_sessions::group_participation_stats_internal_typed(&conn, &session_id)?;

    let response = GroupChatResponse {
        message,
        character_id: selected_character_id,
        character_name: character.name.clone(),
        reasoning,
        selection_reasoning,
        was_mentioned,
        participation_stats,
    };

    let mut complete_extra = Map::new();
    complete_extra.insert(
        "characterId".to_string(),
        Value::String(response.character_id.clone()),
    );
    emit_group_chat_status(&app, &session_id, "complete", complete_extra);

    log_info(
        &app,
        "group_chat_send",
        format!(
            "Group chat response complete: {} responded with {} chars",
            character.name,
            response_content.len()
        ),
    );

    let mut updated_session = {
        let conn = pool.get_connection()?;
        group_sessions::group_session_get_internal_typed(&conn, &session_id)?
    };
    if updated_session.memory_type == "dynamic" {
        if let Err(e) =
            process_group_dynamic_memory_cycle(&app, &mut updated_session, &settings, &pool, false)
                .await
        {
            log_warn(
                &app,
                "group_chat_send",
                format!("Dynamic memory cycle failed: {}", e),
            );
        }
    }

    serde_json::to_string(&response)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub async fn group_chat_retry_dynamic_memory(
    app: AppHandle,
    session_id: String,
    pool: State<'_, SwappablePool>,
) -> Result<(), String> {
    log_info(
        &app,
        "group_chat_retry_dynamic_memory",
        format!(
            "Manually triggering memory cycle for session {}",
            session_id
        ),
    );

    let settings = load_settings(&app)?;
    let conn = pool.get_connection()?;
    let mut session = group_sessions::group_session_get_internal_typed(&conn, &session_id)?;
    if session.memory_type != "dynamic" {
        log_info(
            &app,
            "group_chat_retry_dynamic_memory",
            "session memory_type is not dynamic; skipping manual retry",
        );
        return Ok(());
    }

    process_group_dynamic_memory_cycle(&app, &mut session, &settings, &pool, true).await
}

#[tauri::command]
pub fn group_chat_abort_dynamic_memory(app: AppHandle, session_id: String) -> Result<(), String> {
    let run_key = group_dynamic_memory_run_key(&session_id);
    let run_manager = app.state::<DynamicMemoryRunManager>().inner().clone();
    let abort_registry = app.state::<AbortRegistry>();
    run_manager.cancel_run(&abort_registry, &run_key)
}

#[tauri::command]
pub fn group_chat_skip_dynamic_memory(app: AppHandle, session_id: String) -> Result<(), String> {
    app.state::<crate::dynamic_memory_approval::DynamicMemoryApprovalManager>()
        .mark_skipped(&session_id);
    Ok(())
}

#[tauri::command]
pub fn group_chat_dynamic_memory_pending_approval(
    app: AppHandle,
    session_id: String,
) -> Option<u32> {
    app.state::<crate::dynamic_memory_approval::DynamicMemoryApprovalManager>()
        .pending(&session_id)
        .map(|count| count as u32)
}

#[tauri::command]
pub fn group_chat_dynamic_memory_cycle_status(
    app: AppHandle,
    session_id: String,
    pool: State<'_, SwappablePool>,
) -> Result<crate::chat_manager::memory::flow::DynamicMemoryCycleStatus, String> {
    let settings = load_settings(&app)?;
    let conn = pool.get_connection()?;
    let session = group_sessions::group_session_get_internal_typed(&conn, &session_id)?;
    if session.memory_type != "dynamic" {
        return Err("Session does not use dynamic memory".to_string());
    }
    let dynamic = effective_group_dynamic_memory_settings(&settings);
    let interval = dynamic.summary_message_interval.max(1) as usize;
    let total = conn
        .query_row(
            "SELECT COUNT(1) FROM group_messages WHERE session_id = ?1 AND (role = 'user' OR role = 'assistant')",
            rusqlite::params![&session_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .max(0) as usize;
    let (last_window_end, _) = resolve_last_valid_group_window_end(&conn, &session)?;
    let since = total.saturating_sub(last_window_end);
    let approval = app.state::<crate::dynamic_memory_approval::DynamicMemoryApprovalManager>();

    Ok(
        crate::chat_manager::memory::flow::DynamicMemoryCycleStatus {
            run_mode: dynamic.run_mode.clone(),
            interval,
            messages_since_last_cycle: since,
            messages_until_next_cycle: interval.saturating_sub(since),
            total_conversation_messages: total,
            pending_approval_count: approval.pending(&session_id),
            skipped: approval.was_skipped(&session_id),
            latest_cycle_status: session
                .memory_tool_events
                .last()
                .and_then(|event| event.get("status").and_then(Value::as_str))
                .map(str::to_string),
        },
    )
}

#[tauri::command]
pub async fn group_chat_regenerate(
    app: AppHandle,
    session_id: String,
    message_id: String,
    force_character_id: Option<String>,
    guidance: Option<String>,
    model_id: Option<String>,
    stream: Option<bool>,
    request_id: Option<String>,
    pool: State<'_, SwappablePool>,
) -> Result<String, String> {
    log_info(
        &app,
        "group_chat_regenerate",
        format!(
            "Regenerating message {} in session {}",
            message_id, session_id
        ),
    );

    emit_group_chat_status(&app, &session_id, "selecting_character", Map::new());

    let settings = load_settings(&app)?;
    let conn = pool.get_connection()?;
    let req_id = request_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let abort_registry = app.state::<AbortRegistry>();
    let mut abort_rx = abort_registry.register(req_id.clone());
    let _abort_guard = AbortGuard::new(&abort_registry, req_id.clone());
    let model_override = model_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let (turn_number, original_speaker): (i32, Option<String>) = conn
        .query_row(
            "SELECT turn_number, speaker_character_id FROM group_messages WHERE id = ?1",
            rusqlite::params![message_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let user_message: String = conn
        .query_row(
            "SELECT content FROM group_messages WHERE session_id = ?1 AND turn_number < ?2 AND role = 'user' ORDER BY turn_number DESC LIMIT 1",
            rusqlite::params![session_id, turn_number],
            |row| row.get(0),
        )
        .unwrap_or_default();

    let guidance = guidance
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let mut context = build_selection_context(&conn, &session_id, &user_message)?;
    context
        .recent_messages
        .retain(|m| m.turn_number < turn_number);

    let selection_result: Result<(String, Option<String>, bool), String> = if let Some(forced_id) =
        force_character_id
    {
        Ok((
            forced_id,
            Some("User forced character selection".to_string()),
            true,
        ))
    } else if let Some(speaker_id) = original_speaker.clone() {
        Ok((
            speaker_id,
            Some("Reroll: kept original speaker".to_string()),
            true,
        ))
    } else {
        let selection_result = tokio::select! {
            _ = &mut abort_rx => {
                log_warn(
                    &app,
                    "group_chat_regenerate",
                    format!("Request aborted by user for session {}", session_id),
                );
                return Err(crate::utils::err_msg(module_path!(), line!(), "Request aborted by user"));
            }
            selection = select_speaker_via_llm(&app, &context, &settings, &req_id) => selection,
        };
        match selection_result {
            Ok(selection) => Ok((selection.character_id, selection.reasoning, false)),
            Err(err) => {
                if is_request_abort_error(&err) {
                    return Err(err.to_string());
                }
                log_error(
                    &app,
                    "group_chat_regenerate",
                    format!("LLM selection failed: {}", err),
                );
                selection::heuristic_select_speaker(&context)
                    .map(|fallback| (fallback.character_id, fallback.reasoning, false))
            }
        }
    };

    let (mut selected_character_id, mut selection_reasoning, allow_muted_selection) =
        match selection_result {
            Ok(result) => result,
            Err(err) => {
                if !is_request_abort_error(&err) {
                    emit_group_chat_error_status(&app, &session_id, &err);
                }
                return Err(err);
            }
        };

    if !allow_muted_selection
        && context
            .session
            .muted_character_ids
            .contains(&selected_character_id)
    {
        log_warn(
            &app,
            "group_chat_regenerate",
            format!(
                "Auto-selection returned muted character {}. Falling back to heuristic selection.",
                selected_character_id
            ),
        );
        match selection::heuristic_select_speaker(&context) {
            Ok(fallback) => {
                selected_character_id = fallback.character_id;
                selection_reasoning = fallback.reasoning;
            }
            Err(err) => {
                emit_group_chat_error_status(&app, &session_id, &err);
                return Err(err);
            }
        }
    }

    let character = context
        .characters
        .iter()
        .find(|c| c.id == selected_character_id)
        .cloned();
    let character = match character {
        Some(character) => character,
        None => {
            let err = "Character not found".to_string();
            emit_group_chat_error_status(&app, &session_id, &err);
            return Err(err);
        }
    };

    let mut selected_extra = Map::new();
    selected_extra.insert(
        "characterId".to_string(),
        Value::String(selected_character_id.clone()),
    );
    selected_extra.insert(
        "characterName".to_string(),
        Value::String(character.name.clone()),
    );
    emit_group_chat_status(&app, &session_id, "character_selected", selected_extra);

    let response_result = generate_character_response(
        &app,
        &mut context,
        &selected_character_id,
        &settings,
        &pool,
        &req_id,
        stream.unwrap_or(true),
        guidance.as_deref(),
        model_override.as_deref(),
        UsageOperationType::GroupChatRegenerate,
    )
    .await;

    let output = match response_result {
        Ok(result) => result,
        Err(err) => {
            if !is_request_abort_error(&err) {
                emit_group_chat_error_status(&app, &session_id, &err);
            }
            return Err(err);
        }
    };
    let GroupGenerationOutput {
        content: response_content,
        reasoning,
        usage: message_usage,
        model_id: model_id_str,
        used_lorebook_entries,
        memory_refs,
        generated_image_data_urls,
        gemini_content,
    } = output;

    let conn = pool.get_connection()?;
    let now = now_ms();
    let variant_id = Uuid::new_v4().to_string();
    let attachment_owner = context
        .session
        .group_character_id
        .as_deref()
        .unwrap_or(&context.session.id);
    let assistant_attachments = persistence::persist_group_attachments(
        &app,
        attachment_owner,
        &session_id,
        &message_id,
        "assistant",
        persistence::generated_image_attachments(generated_image_data_urls),
    )?;
    let attachments_json =
        serde_json::to_string(&assistant_attachments).unwrap_or_else(|_| "[]".to_string());
    let gemini_content_json = gemini_content
        .as_ref()
        .and_then(|value| serde_json::to_string(value).ok());
    let mtp_stats_json = message_usage
        .as_ref()
        .and_then(|usage| usage.mtp_stats.as_ref())
        .and_then(|value| serde_json::to_string(value).ok());

    conn.execute(
        "INSERT INTO group_message_variants (id, message_id, content, speaker_character_id, created_at, reasoning, selection_reasoning, prompt_tokens, completion_tokens, total_tokens, first_token_ms, tokens_per_second, mtp_stats, model_id, attachments, gemini_content, usage_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
        rusqlite::params![
            variant_id,
            message_id,
            response_content,
            selected_character_id,
            now,
            reasoning,
            selection_reasoning,
            message_usage.as_ref().and_then(|u| u.prompt_tokens),
            message_usage.as_ref().and_then(|u| u.completion_tokens),
            message_usage.as_ref().and_then(|u| u.total_tokens),
            message_usage.as_ref().and_then(|u| u.first_token_ms),
            message_usage.as_ref().and_then(|u| u.tokens_per_second),
            mtp_stats_json,
            model_id_str,
            attachments_json,
            gemini_content_json,
            message_usage.as_ref().and_then(|usage| serde_json::to_string(usage).ok()),
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    log_info(
        &app,
        "group_chat_regenerate",
        format!(
            "✓ Successfully inserted variant {} to group_message_variants table",
            variant_id
        ),
    );

    conn.execute(
        "UPDATE group_messages SET content = ?1, speaker_character_id = ?2, selected_variant_id = ?3, used_lorebook_entries = ?4, reasoning = ?5, selection_reasoning = ?6, model_id = ?7, prompt_tokens = ?8, completion_tokens = ?9, total_tokens = ?10, first_token_ms = ?11, tokens_per_second = ?12, mtp_stats = ?13, attachments = ?14, gemini_content = ?15, memory_refs = ?16, usage_json = ?17 WHERE id = ?18",
        rusqlite::params![
            response_content,
            selected_character_id,
            variant_id,
            serde_json::to_string(&used_lorebook_entries).unwrap_or_else(|_| "[]".to_string()),
            reasoning,
            selection_reasoning,
            model_id_str,
            message_usage.as_ref().and_then(|u| u.prompt_tokens),
            message_usage.as_ref().and_then(|u| u.completion_tokens),
            message_usage.as_ref().and_then(|u| u.total_tokens),
            message_usage.as_ref().and_then(|u| u.first_token_ms),
            message_usage.as_ref().and_then(|u| u.tokens_per_second),
            mtp_stats_json,
            attachments_json,
            gemini_content_json,
            serde_json::to_string(&memory_refs).unwrap_or_else(|_| "[]".to_string()),
            message_usage.as_ref().and_then(|usage| serde_json::to_string(usage).ok()),
            message_id
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    if original_speaker.as_ref() != Some(&selected_character_id) {
        update_participation(&conn, &session_id, &selected_character_id, turn_number)?;
    }

    let participation_stats =
        group_sessions::group_participation_stats_internal_typed(&conn, &session_id)?;

    let messages =
        group_sessions::group_messages_list_internal_typed(&conn, &session_id, 100, None, None)?;
    let message = messages
        .into_iter()
        .find(|m| m.id == message_id)
        .ok_or_else(|| "Message not found after update".to_string())?;

    let response = GroupChatResponse {
        message,
        character_id: selected_character_id.clone(),
        character_name: character.name.clone(),
        reasoning,
        selection_reasoning,
        was_mentioned: false,
        participation_stats,
    };

    let mut complete_extra = Map::new();
    complete_extra.insert(
        "characterId".to_string(),
        Value::String(selected_character_id.clone()),
    );
    emit_group_chat_status(&app, &session_id, "complete", complete_extra);

    serde_json::to_string(&response)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub async fn group_chat_continue(
    app: AppHandle,
    session_id: String,
    force_character_id: Option<String>,
    stream: Option<bool>,
    request_id: Option<String>,
    pool: State<'_, SwappablePool>,
) -> Result<String, String> {
    log_info(
        &app,
        "group_chat_continue",
        format!("Continuing group chat session {}", session_id),
    );

    emit_group_chat_status(&app, &session_id, "selecting_character", Map::new());

    let settings = load_settings(&app)?;
    let conn = pool.get_connection()?;
    let req_id = request_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let abort_registry = app.state::<AbortRegistry>();
    let mut abort_rx = abort_registry.register(req_id.clone());
    let _abort_guard = AbortGuard::new(&abort_registry, req_id.clone());

    let mut context = build_selection_context(&conn, &session_id, "")?;

    let selection_result: Result<(String, Option<String>, bool), String> = if let Some(forced_id) =
        force_character_id
    {
        Ok((
            forced_id,
            Some("User requested specific character".to_string()),
            true,
        ))
    } else {
        let method = context.session.speaker_selection_method.as_str();
        match method {
            "heuristic" => selection::heuristic_select_speaker(&context)
                .map(|result| (result.character_id, result.reasoning, false)),
            "round_robin" => selection::round_robin_select_speaker(&context)
                .map(|result| (result.character_id, result.reasoning, false)),
            _ => {
                let selection_result = tokio::select! {
                    _ = &mut abort_rx => {
                        log_warn(
                            &app,
                            "group_chat_continue",
                            format!("Request aborted by user for session {}", session_id),
                        );
                        return Err(crate::utils::err_msg(module_path!(), line!(), "Request aborted by user"));
                    }
                    selection = select_speaker_via_llm(&app, &context, &settings, &req_id) => selection,
                };
                match selection_result {
                    Ok(selection) => Ok((selection.character_id, selection.reasoning, false)),
                    Err(err) => {
                        if is_request_abort_error(&err) {
                            return Err(err.to_string());
                        }
                        log_error(
                            &app,
                            "group_chat_continue",
                            format!("LLM selection failed: {}", err),
                        );
                        selection::heuristic_select_speaker(&context)
                            .map(|fallback| (fallback.character_id, fallback.reasoning, false))
                    }
                }
            }
        }
    };

    let (mut selected_character_id, mut selection_reasoning, allow_muted_selection) =
        match selection_result {
            Ok(result) => result,
            Err(err) => {
                if !is_request_abort_error(&err) {
                    emit_group_chat_error_status(&app, &session_id, &err);
                }
                return Err(err);
            }
        };

    if !allow_muted_selection
        && context
            .session
            .muted_character_ids
            .contains(&selected_character_id)
    {
        log_warn(
            &app,
            "group_chat_continue",
            format!(
                "Auto-selection returned muted character {}. Falling back to heuristic selection.",
                selected_character_id
            ),
        );
        match selection::heuristic_select_speaker(&context) {
            Ok(fallback) => {
                selected_character_id = fallback.character_id;
                selection_reasoning = fallback.reasoning;
            }
            Err(err) => {
                emit_group_chat_error_status(&app, &session_id, &err);
                return Err(err);
            }
        }
    }

    let character = context
        .characters
        .iter()
        .find(|c| c.id == selected_character_id)
        .cloned();
    let character = match character {
        Some(character) => character,
        None => {
            let err = "Character not found".to_string();
            emit_group_chat_error_status(&app, &session_id, &err);
            return Err(err);
        }
    };

    let mut selected_extra = Map::new();
    selected_extra.insert(
        "characterId".to_string(),
        Value::String(selected_character_id.clone()),
    );
    selected_extra.insert(
        "characterName".to_string(),
        Value::String(character.name.clone()),
    );
    emit_group_chat_status(&app, &session_id, "character_selected", selected_extra);

    let response_result = generate_character_response(
        &app,
        &mut context,
        &selected_character_id,
        &settings,
        &pool,
        &req_id,
        stream.unwrap_or(true),
        None,
        None,
        UsageOperationType::GroupChatContinue,
    )
    .await;

    let output = match response_result {
        Ok(result) => result,
        Err(err) => {
            if !is_request_abort_error(&err) {
                emit_group_chat_error_status(&app, &session_id, &err);
            }
            return Err(err);
        }
    };
    let GroupGenerationOutput {
        content: response_content,
        reasoning,
        usage: message_usage,
        model_id: model_id_str,
        used_lorebook_entries,
        memory_refs,
        generated_image_data_urls,
        gemini_content,
    } = output;

    let conn = pool.get_connection()?;
    let assistant_message_id = Uuid::new_v4().to_string();
    let attachment_owner = context
        .session
        .group_character_id
        .as_deref()
        .unwrap_or(&context.session.id);
    let assistant_attachments = persistence::persist_group_attachments(
        &app,
        attachment_owner,
        &session_id,
        &assistant_message_id,
        "assistant",
        persistence::generated_image_attachments(generated_image_data_urls),
    )?;
    let message = save_assistant_message(
        &app,
        &conn,
        &session_id,
        Some(assistant_message_id),
        &selected_character_id,
        &response_content,
        reasoning.as_deref(),
        selection_reasoning.as_deref(),
        message_usage.as_ref(),
        Some(&model_id_str),
        &used_lorebook_entries,
        &memory_refs,
        &assistant_attachments,
        gemini_content.as_ref(),
    )?;

    let _ = crate::storage_manager::llm_metrics::llm_metrics_attach_message(
        app.clone(),
        req_id.clone(),
        message.id.clone(),
    );

    let participation_stats =
        group_sessions::group_participation_stats_internal_typed(&conn, &session_id)?;

    let response = GroupChatResponse {
        message,
        character_id: selected_character_id.clone(),
        character_name: character.name.clone(),
        reasoning,
        selection_reasoning,
        was_mentioned: false,
        participation_stats,
    };

    let mut complete_extra = Map::new();
    complete_extra.insert(
        "characterId".to_string(),
        Value::String(selected_character_id.clone()),
    );
    emit_group_chat_status(&app, &session_id, "complete", complete_extra);

    drop(conn);
    let mut updated_session = {
        let conn = pool.get_connection()?;
        group_sessions::group_session_get_internal_typed(&conn, &session_id)?
    };
    if updated_session.memory_type == "dynamic" {
        if let Err(e) =
            process_group_dynamic_memory_cycle(&app, &mut updated_session, &settings, &pool, false)
                .await
        {
            log_warn(
                &app,
                "group_chat_continue",
                format!("Dynamic memory cycle failed: {}", e),
            );
        }
    }

    serde_json::to_string(&response)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub fn group_chat_get_selection_prompt(
    session_id: String,
    user_message: String,
    pool: State<'_, SwappablePool>,
) -> Result<String, String> {
    let conn = pool.get_connection()?;
    let context = build_selection_context(&conn, &session_id, &user_message)?;
    Ok(selection::build_selection_prompt(&context))
}

/// Helper to remove {{#if current_draft}}...{{else}}...{{/if}} and keep else content
fn remove_if_block(text: &mut String) {
    if let Some(if_start) = text.find("{{#if current_draft}}") {
        if let Some(endif_start) = text[if_start..].find("{{/if}}") {
            let else_content =
                if let Some(else_start) = text[if_start..if_start + endif_start].find("{{else}}") {
                    let else_abs = if_start + else_start + 8; // +8 for "{{else}}"
                    let endif_abs = if_start + endif_start;
                    text[else_abs..endif_abs].to_string()
                } else {
                    String::new()
                };
            text.replace_range(if_start..(if_start + endif_start + 7), &else_content);
        }
    }
}

#[tauri::command]
pub async fn group_chat_generate_user_reply(
    app: AppHandle,
    session_id: String,
    current_draft: Option<String>,
    request_id: Option<String>,
    pool: State<'_, SwappablePool>,
) -> Result<String, String> {
    log_info(
        &app,
        "group_help_me_reply",
        format!(
            "Generating user reply for group session={}, has_draft={}",
            &session_id,
            current_draft.is_some()
        ),
    );

    let settings = load_settings(&app)?;
    let conn = pool.get_connection()?;

    let session = group_sessions::group_session_get_internal_typed(&conn, &session_id)?;

    let personas = load_personas(&app)?;
    let persona = personas
        .iter()
        .find(|p| Some(&p.id) == session.persona_id.as_ref());

    let recent_msgs =
        group_sessions::group_messages_list_internal_typed(&conn, &session_id, 10, None, None)?;

    if recent_msgs.is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "No conversation history to base reply on",
        ));
    }

    // Load all characters in this group
    let mut group_characters: Vec<Character> = Vec::new();
    for char_id in &session.character_ids {
        let character = load_character(&conn, char_id)?;
        group_characters.push(character);
    }

    if group_characters.is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "No characters found in group session",
        ));
    }

    // Check if help me reply is enabled
    if let Some(advanced) = &settings.advanced_settings {
        if advanced.help_me_reply_enabled == Some(false) {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Help Me Reply is disabled in settings",
            ));
        }
    }

    // Use help me reply model if configured, otherwise fall back to default
    let model_id = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.help_me_reply_model_id.as_ref())
        .or(settings.default_model_id.as_ref())
        .ok_or_else(|| "No model configured for Group Help Me Reply".to_string())?;

    let model = settings
        .models
        .iter()
        .find(|m| &m.id == model_id)
        .ok_or_else(|| "Group Help Me Reply model not found".to_string())?;

    let provider_cred = resolve_credential_for_model(&settings, model)
        .ok_or_else(|| "Provider credential not found".to_string())?;

    let api_key = require_api_key(&app, provider_cred, "group_help_me_reply")?;

    // Get reply style from settings (default to roleplay)
    let reply_style = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.help_me_reply_style.as_ref())
        .map(|s| s.as_str())
        .unwrap_or("roleplay");

    // Get max tokens from settings (default to 150)
    let max_tokens = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.help_me_reply_max_tokens)
        .unwrap_or(150) as u32;

    // Get streaming setting (default to true)
    let streaming_enabled = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.help_me_reply_streaming)
        .unwrap_or(true);

    let help_me_reply_prompt_template_id = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| {
            if reply_style == "conversational" {
                advanced
                    .help_me_reply_conversational_prompt_template_id
                    .as_deref()
            } else {
                advanced
                    .help_me_reply_roleplay_prompt_template_id
                    .as_deref()
            }
        })
        .filter(|id| !id.trim().is_empty());

    let mut prompt_entries =
        prompts::get_help_me_reply_entries(&app, reply_style, help_me_reply_prompt_template_id);

    let persona_name = persona.map(|p| p.title.as_str()).unwrap_or("user");
    let persona_desc = persona.map(|p| p.description.as_str()).unwrap_or("");

    // Build character list for the prompt
    let char_list = group_characters
        .iter()
        .map(|c| {
            let desc = c
                .definition
                .as_deref()
                .or(c.description.as_deref())
                .unwrap_or("");
            if desc.is_empty() {
                c.name.clone()
            } else {
                format!("{} ({})", c.name, desc)
            }
        })
        .collect::<Vec<_>>()
        .join(", ");

    let draft_str = current_draft.as_deref().unwrap_or("");
    for entry in &mut prompt_entries {
        let content = &mut entry.content;
        *content = content
            .replace("{{char.name}}", &char_list)
            .replace("{{char.desc}}", "participants in a group conversation")
            .replace("{{persona.name}}", persona_name)
            .replace("{{persona.desc}}", persona_desc)
            .replace("{{user.name}}", persona_name)
            .replace("{{user.desc}}", persona_desc)
            .replace("{{current_draft}}", draft_str)
            .replace("{{char}}", &char_list)
            .replace("{{persona}}", persona_name)
            .replace("{{user}}", persona_name);

        if let Some(ref draft) = current_draft {
            if !draft.trim().is_empty() {
                *content = content.replace("{{#if current_draft}}", "");
                *content = content.replace("{{current_draft}}", draft);
                if let Some(else_start) = content.find("{{else}}") {
                    if let Some(endif_start) = content[else_start..].find("{{/if}}") {
                        content.replace_range(else_start..(else_start + endif_start + 7), "");
                    }
                }
                *content = content.replace("{{/if}}", "");
            } else {
                remove_if_block(content);
            }
        } else {
            remove_if_block(content);
        }
    }

    let conversation_context = recent_msgs
        .iter()
        .map(|msg| {
            let speaker_name = if msg.role == "user" {
                persona_name
            } else {
                group_characters
                    .iter()
                    .find(|c| Some(&c.id) == msg.speaker_character_id.as_ref())
                    .map(|c| c.name.as_str())
                    .unwrap_or("Character")
            };
            format!("{}: {}", speaker_name, msg.content)
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let user_prompt = format!(
        "Here is the recent group conversation:\n\n{}\n\nGenerate a reply for {} to say next in this group chat.",
        conversation_context, persona_name
    );

    let system_role = crate::chat_manager::request_builder::system_role_for(provider_cred);
    prompt_entries.push(in_chat_user_entry(
        "runtime_group_reply_helper_input",
        "Group Reply Helper Input",
        user_prompt,
        0,
    ));
    let messages_for_api = assemble_prompt_messages(prompt_entries, Vec::new(), &system_role);

    let mut overrides = feature_model_overrides(
        model,
        LlmFeature::HelpMeReply,
        HELP_ME_REPLY_DEFAULTS,
    );
    if overrides.max_output_tokens.is_none() {
        overrides.max_output_tokens = Some(max_tokens);
    }
    let request_session = synthetic_feature_session("__group_help_me_reply__", overrides);
    let (request_settings, extra_body_fields) = prepare_feature_request(
        &provider_cred.provider_id,
        &request_session,
        model,
        &settings,
    );
    let built = crate::chat_manager::request_builder::build_chat_request(
        provider_cred,
        &api_key,
        &model.name,
        &messages_for_api,
        None,
        request_settings.temperature,
        request_settings.top_p,
        request_settings.max_tokens,
        request_settings.context_length,
        streaming_enabled,  // streaming from settings
        request_id.clone(), // request_id for streaming
        None,               // frequency_penalty
        None,               // presence_penalty
        None,               // top_k
        None,               // tool_config
        false,              // reasoning_enabled
        None,               // reasoning_effort
        None,               // reasoning_budget
        false,              // prompt_caching_enabled
        extra_body_fields,
    );

    log_info(
        &app,
        "group_help_me_reply",
        format!("Sending request to {}", built.url),
    );

    let api_request_payload = ApiRequest {
        url: built.url,
        method: Some("POST".into()),
        headers: Some(built.headers),
        query: None,
        body: Some(built.body),
        timeout_ms: Some(crate::transport::DEFAULT_REQUEST_TIMEOUT_MS),
        stream: Some(built.stream),
        request_id: built.request_id.clone(),
        provider_id: Some(provider_cred.provider_id.clone()),
        cache_key: None,
    };

    let api_response = api_request(app.clone(), api_request_payload).await?;

    if !api_response.ok {
        let fallback = format!("Provider returned status {}", api_response.status);
        let err_message = extract_error_message(&api_response.data).unwrap_or(fallback.clone());

        log_error(
            &app,
            "group_help_me_reply",
            format!(
                "API request failed: status={} error={}",
                api_response.status, err_message
            ),
        );

        log_info(
            &app,
            "group_help_me_reply",
            format!(
                "Full error response: {}",
                serde_json::to_string_pretty(&api_response.data)
                    .unwrap_or_else(|_| "unable to serialize".to_string())
            ),
        );

        return Err(format!(
            "API request failed with status {}: {}",
            api_response.status, err_message
        ));
    }

    let generated_text = extract_text(&api_response.data, Some(&provider_cred.provider_id))
        .ok_or_else(|| "Failed to extract text from response".to_string())?;

    let cleaned = generated_text
        .trim()
        .trim_matches('"')
        .trim_start_matches(&format!("{}:", persona_name))
        .trim()
        .to_string();

    log_info(
        &app,
        "group_help_me_reply",
        format!("Generated reply: {} chars", cleaned.len()),
    );

    let usage = extract_usage(&api_response.data);

    // Record usage - use first character as representative
    if let Some(first_char) = group_characters.first() {
        record_group_usage(
            &app,
            &usage,
            &session,
            first_char,
            model,
            provider_cred,
            &api_key,
            UsageOperationType::ReplyHelper,
            "group_help_me_reply",
        )
        .await;
    }

    Ok(cleaned)
}
