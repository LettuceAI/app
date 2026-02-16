//! Group Chat Manager
//!
//! This module handles group chat functionality including:
//! - Dynamic character selection based on context (via LLM tool calling)
//! - @mention parsing to force specific characters
//! - Building selection prompts with participation stats
//! - Coordinating with the chat_manager for actual response generation
//! - Full dynamic memory system support (decay, hot/cold, summarization, tool updates)

mod selection;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

use rusqlite::OptionalExtension;

use crate::abort_manager::AbortRegistry;
use crate::api::{api_request, ApiRequest};
use crate::models::get_model_pricing;
use crate::usage::add_usage_record;
use crate::usage::tracking::{RequestUsage, UsageFinishReason, UsageOperationType};

use crate::chat_manager::dynamic_memory::{
    apply_memory_decay, calculate_hot_memory_tokens, cosine_similarity,
    effective_group_dynamic_memory_settings, enforce_hot_memory_budget, ensure_pinned_hot,
    generate_memory_id, mark_memories_accessed, normalize_query_text, promote_cold_memories,
    search_cold_memory_indices_by_keyword, select_relevant_memory_indices,
    select_top_cosine_memory_indices, trim_memories_to_max,
};
use crate::chat_manager::prompts::{
    self, APP_DYNAMIC_MEMORY_TEMPLATE_ID, APP_DYNAMIC_SUMMARY_TEMPLATE_ID,
};
use crate::chat_manager::request::{
    extract_error_message, extract_reasoning, extract_text, extract_usage,
};
use crate::chat_manager::service::resolve_api_key;
use crate::chat_manager::storage::{load_personas, load_settings, select_model};
use crate::chat_manager::tooling::{
    parse_tool_calls, ToolCall, ToolChoice, ToolConfig, ToolDefinition,
};
use crate::chat_manager::types::{
    Character, DynamicMemorySettings, MemoryRetrievalStrategy, Model, Persona, PromptEntryPosition,
    PromptEntryRole, ProviderCredential, Settings, SystemPromptEntry,
};
use crate::embedding_model;
use crate::models::calculate_request_cost;
use crate::storage_manager::db::{now_ms, SwappablePool};
use crate::storage_manager::group_sessions::{
    self, group_session_update_memories_internal, GroupMessage, GroupParticipation, GroupSession,
    MemoryEmbedding, UsageSummary,
};
use crate::utils::{log_error, log_info, log_warn, now_millis};

pub use selection::parse_mentions;

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

fn resolve_context_length(model: &Model, settings: &Settings) -> Option<u32> {
    model
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.context_length)
        .or(settings.advanced_model_settings.context_length)
        .filter(|v| *v > 0)
}

fn build_llama_extra_fields(model: &Model, settings: &Settings) -> Option<HashMap<String, Value>> {
    let mut extra = HashMap::new();
    if let Some(v) = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.llama_gpu_layers)
        .or(settings.advanced_model_settings.llama_gpu_layers)
    {
        extra.insert("llamaGpuLayers".to_string(), json!(v));
    }
    if let Some(v) = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.llama_threads)
        .or(settings.advanced_model_settings.llama_threads)
    {
        extra.insert("llamaThreads".to_string(), json!(v));
    }
    if let Some(v) = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.llama_threads_batch)
        .or(settings.advanced_model_settings.llama_threads_batch)
    {
        extra.insert("llamaThreadsBatch".to_string(), json!(v));
    }
    if let Some(v) = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.llama_seed)
        .or(settings.advanced_model_settings.llama_seed)
    {
        extra.insert("llamaSeed".to_string(), json!(v));
    }
    if let Some(v) = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.llama_rope_freq_base)
        .or(settings.advanced_model_settings.llama_rope_freq_base)
    {
        extra.insert("llamaRopeFreqBase".to_string(), json!(v));
    }
    if let Some(v) = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.llama_rope_freq_scale)
        .or(settings.advanced_model_settings.llama_rope_freq_scale)
    {
        extra.insert("llamaRopeFreqScale".to_string(), json!(v));
    }
    if let Some(v) = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.llama_offload_kqv)
        .or(settings.advanced_model_settings.llama_offload_kqv)
    {
        extra.insert("llamaOffloadKqv".to_string(), json!(v));
    }

    if extra.is_empty() {
        None
    } else {
        Some(extra)
    }
}

fn build_ollama_extra_fields(
    model: &Model,
    settings: &Settings,
    context_length: Option<u32>,
    max_tokens: u32,
    temperature: f64,
    top_p: f64,
    top_k: Option<u32>,
    frequency_penalty: Option<f64>,
    presence_penalty: Option<f64>,
) -> Option<HashMap<String, Value>> {
    let mut options = Map::new();

    let num_ctx = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.ollama_num_ctx)
        .or(settings.advanced_model_settings.ollama_num_ctx)
        .or(context_length);
    let num_predict = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.ollama_num_predict)
        .or(settings.advanced_model_settings.ollama_num_predict)
        .or(Some(max_tokens));
    let num_keep = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.ollama_num_keep)
        .or(settings.advanced_model_settings.ollama_num_keep);
    let num_batch = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.ollama_num_batch)
        .or(settings.advanced_model_settings.ollama_num_batch);
    let num_gpu = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.ollama_num_gpu)
        .or(settings.advanced_model_settings.ollama_num_gpu);
    let num_thread = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.ollama_num_thread)
        .or(settings.advanced_model_settings.ollama_num_thread);
    let tfs_z = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.ollama_tfs_z)
        .or(settings.advanced_model_settings.ollama_tfs_z);
    let typical_p = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.ollama_typical_p)
        .or(settings.advanced_model_settings.ollama_typical_p);
    let min_p = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.ollama_min_p)
        .or(settings.advanced_model_settings.ollama_min_p);
    let mirostat = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.ollama_mirostat)
        .or(settings.advanced_model_settings.ollama_mirostat);
    let mirostat_tau = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.ollama_mirostat_tau)
        .or(settings.advanced_model_settings.ollama_mirostat_tau);
    let mirostat_eta = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.ollama_mirostat_eta)
        .or(settings.advanced_model_settings.ollama_mirostat_eta);
    let repeat_penalty = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.ollama_repeat_penalty)
        .or(settings.advanced_model_settings.ollama_repeat_penalty);
    let seed = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.ollama_seed)
        .or(settings.advanced_model_settings.ollama_seed);
    let stop = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.ollama_stop.clone())
        .or(settings.advanced_model_settings.ollama_stop.clone());

    options.insert("temperature".into(), json!(temperature));
    options.insert("top_p".into(), json!(top_p));
    if let Some(v) = top_k {
        options.insert("top_k".into(), json!(v));
    }
    if let Some(v) = frequency_penalty {
        options.insert("frequency_penalty".into(), json!(v));
    }
    if let Some(v) = presence_penalty {
        options.insert("presence_penalty".into(), json!(v));
    }
    if let Some(v) = num_ctx {
        options.insert("num_ctx".into(), json!(v));
    }
    if let Some(v) = num_predict {
        options.insert("num_predict".into(), json!(v));
    }
    if let Some(v) = num_keep {
        options.insert("num_keep".into(), json!(v));
    }
    if let Some(v) = num_batch {
        options.insert("num_batch".into(), json!(v));
    }
    if let Some(v) = num_gpu {
        options.insert("num_gpu".into(), json!(v));
    }
    if let Some(v) = num_thread {
        options.insert("num_thread".into(), json!(v));
    }
    if let Some(v) = tfs_z {
        options.insert("tfs_z".into(), json!(v));
    }
    if let Some(v) = typical_p {
        options.insert("typical_p".into(), json!(v));
    }
    if let Some(v) = min_p {
        options.insert("min_p".into(), json!(v));
    }
    if let Some(v) = mirostat {
        options.insert("mirostat".into(), json!(v));
    }
    if let Some(v) = mirostat_tau {
        options.insert("mirostat_tau".into(), json!(v));
    }
    if let Some(v) = mirostat_eta {
        options.insert("mirostat_eta".into(), json!(v));
    }
    if let Some(v) = repeat_penalty {
        options.insert("repeat_penalty".into(), json!(v));
    }
    if let Some(v) = seed {
        options.insert("seed".into(), json!(v));
    }
    if let Some(v) = stop {
        options.insert("stop".into(), json!(v));
    }

    let mut extra = HashMap::new();
    extra.insert("options".to_string(), Value::Object(options));
    Some(extra)
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
        provider_label: provider_cred.provider_id.clone(),
        operation_type,
        finish_reason: usage_info
            .finish_reason
            .as_ref()
            .and_then(|s| UsageFinishReason::from_str(s)),
        prompt_tokens: usage_info.prompt_tokens,
        completion_tokens: usage_info.completion_tokens,
        total_tokens: usage_info.total_tokens,
        memory_tokens: None,
        summary_tokens: None,
        reasoning_tokens: usage_info.reasoning_tokens,
        image_tokens: usage_info.image_tokens,
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

    // Calculate cost for OpenRouter
    if provider_cred.provider_id.eq_ignore_ascii_case("openrouter") {
        match get_model_pricing(
            app.clone(),
            &provider_cred.provider_id,
            &model.name,
            Some(api_key),
        )
        .await
        {
            Ok(Some(pricing)) => {
                if let Some(cost) = calculate_request_cost(
                    usage_info.prompt_tokens.map(|v| v as u64).unwrap_or(0),
                    usage_info.completion_tokens.map(|v| v as u64).unwrap_or(0),
                    &pricing,
                ) {
                    request_usage.cost = Some(cost.clone());
                    log_info(
                        app,
                        log_scope,
                        format!(
                            "calculated cost for group chat request: ${:.6}",
                            cost.total_cost
                        ),
                    );
                }
            }
            Ok(None) => {
                log_warn(
                    app,
                    log_scope,
                    "no pricing found for model (might be free)".to_string(),
                );
            }
            Err(err) => {
                log_error(app, log_scope, format!("failed to fetch pricing: {}", err));
            }
        }
    }

    if let Err(e) = add_usage_record(app, request_usage) {
        log_error(
            app,
            log_scope,
            format!("failed to record group chat usage: {}", e),
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
        provider_label: provider_cred.provider_id.clone(),
        operation_type: UsageOperationType::GroupChatDecisionMaker,
        finish_reason: usage_info
            .finish_reason
            .as_ref()
            .and_then(|s| UsageFinishReason::from_str(s)),
        prompt_tokens: usage_info.prompt_tokens,
        completion_tokens: usage_info.completion_tokens,
        total_tokens: usage_info.total_tokens,
        memory_tokens: None,
        summary_tokens: None,
        reasoning_tokens: usage_info.reasoning_tokens,
        image_tokens: usage_info.image_tokens,
        cost: None,
        success: true,
        error_message: None,
        metadata: Default::default(),
    };

    // Calculate cost for OpenRouter
    if provider_cred.provider_id.eq_ignore_ascii_case("openrouter") {
        match get_model_pricing(
            app.clone(),
            &provider_cred.provider_id,
            &model.name,
            Some(api_key),
        )
        .await
        {
            Ok(Some(pricing)) => {
                if let Some(cost) = calculate_request_cost(
                    usage_info.prompt_tokens.map(|v| v as u64).unwrap_or(0),
                    usage_info.completion_tokens.map(|v| v as u64).unwrap_or(0),
                    &pricing,
                ) {
                    request_usage.cost = Some(cost.clone());
                    log_info(
                        app,
                        log_scope,
                        format!(
                            "calculated cost for decision maker: ${:.6}",
                            cost.total_cost
                        ),
                    );
                }
            }
            Ok(None) => {}
            Err(_) => {}
        }
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
    if session.memory_tool_events.is_empty() {
        return Ok((0, false));
    }

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
            resolve_group_conversation_index_by_message_id(conn, &session.id, end_id)?
        {
            return Ok((window_end, rev_idx != 0));
        }
    }

    Ok((0, true))
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
                reasoning: None,
                selection_reasoning: None,
                model_id: None,
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
        "createdAt": now_millis().unwrap_or_default(),
    });
    push_group_memory_event(session, event);

    if let Err(save_err) = save_group_session_memories(app, session, pool) {
        log_error(
            app,
            "group_dynamic_memory",
            format!("failed to persist error state: {}", save_err),
        );
    }

    let _ = app.emit(
        "group-dynamic-memory:error",
        json!({ "sessionId": session.id, "error": error, "stage": stage }),
    );
}

// ============================================================================
// Memory Retrieval
// ============================================================================

/// Select relevant memories from a group session using semantic search
async fn select_relevant_memories(
    app: &AppHandle,
    session: &GroupSession,
    query: &str,
    limit: usize,
    min_similarity: f32,
    strategy: &MemoryRetrievalStrategy,
) -> Vec<MemoryEmbedding> {
    if query.is_empty() || session.memory_embeddings.is_empty() {
        return Vec::new();
    }

    let query_embedding =
        match embedding_model::compute_embedding(app.clone(), query.to_string()).await {
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
            .filter_map(|(idx, _score)| session.memory_embeddings.get(idx).cloned())
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

    for (idx, _score) in &cosine_indices {
        if let Some(mem) = session.memory_embeddings.get(*idx) {
            results.push(mem.clone());
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
        for (idx, _score) in extra_indices {
            if results.len() >= limit {
                break;
            }
            if !selected.contains(&idx) {
                if let Some(mem) = session.memory_embeddings.get(idx) {
                    results.push(mem.clone());
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

/// Format memories as a string block for injection into prompts

// ============================================================================
// Dynamic Memory Cycle
// ============================================================================

/// Process dynamic memory cycle for group chat after a response
async fn process_group_dynamic_memory_cycle(
    app: &AppHandle,
    session: &mut GroupSession,
    settings: &Settings,
    pool: &State<'_, SwappablePool>,
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

    if !dynamic_settings.enabled {
        log_info(
            app,
            "group_dynamic_memory",
            "dynamic memory disabled globally; skipping",
        );
        return Ok(());
    }

    let window_size = dynamic_settings.summary_message_interval.max(1) as usize;
    let conn = pool.get_connection()?;

    // Load recent messages
    let messages_json =
        group_sessions::group_messages_list_internal(&conn, &session.id, 100, None, None)?;
    let messages: Vec<GroupMessage> = serde_json::from_str(&messages_json).unwrap_or_default();

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
    let (last_window_end, cursor_rewound) = resolve_last_valid_group_window_end(&*conn, session)?;

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

    // Cursor-based delta summary window: summarize everything since last_window_end.
    // If backlog > window_size, include the whole backlog in this run (one-time catch-up).
    let mut window_start = if cursor_rewound { 0 } else { last_window_end };
    let mut window_end = total_convo;
    let convo_window = match fetch_group_conversation_messages_range(
        &*conn,
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

    // Get summarisation model
    let Some(advanced) = settings.advanced_settings.as_ref() else {
        record_group_dynamic_memory_error(
            app,
            session,
            pool,
            "Advanced settings missing",
            "settings",
            window_start,
            window_end,
            &window_message_ids,
            None,
        );
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Advanced settings missing",
        ));
    };

    let summarisation_model_id = match advanced.summarisation_model_id.as_ref() {
        Some(id) => id.clone(),
        None => {
            record_group_dynamic_memory_error(
                app,
                session,
                pool,
                "Summarisation model not configured",
                "summary_model",
                window_start,
                window_end,
                &window_message_ids,
                None,
            );
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Summarisation model not configured",
            ));
        }
    };

    let (summary_model, summary_provider) =
        match find_model_and_credential(settings, &summarisation_model_id) {
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
                );
                return Err(crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    "Summarisation model unavailable",
                ));
            }
        };

    let api_key = match resolve_api_key(app, summary_provider, "group_dynamic_memory") {
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
            );
            return Err(err);
        }
    };

    let _ = app.emit(
        "group-dynamic-memory:processing",
        json!({ "sessionId": session.id }),
    );

    let prior_summary = if cursor_rewound || session.memory_summary.is_empty() {
        None
    } else {
        Some(session.memory_summary.clone())
    };

    // Step 1: Summarize messages
    log_info(
        app,
        "group_dynamic_memory",
        "invoking summarize_group_messages",
    );

    let summary = match summarize_group_messages(
        app,
        summary_provider,
        summary_model,
        &api_key,
        &convo_window,
        prior_summary.as_deref(),
        settings,
    )
    .await
    {
        Ok(s) => s,
        Err(err) => {
            log_error(
                app,
                "group_dynamic_memory",
                format!("summarization failed: {}", err),
            );
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
            );
            return Ok(());
        }
    };

    // Step 2: Run memory tool update
    log_info(
        app,
        "group_dynamic_memory",
        "invoking run_group_memory_tool_update",
    );

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
    )
    .await
    {
        Ok(result) => result,
        Err(err) => {
            log_error(
                app,
                "group_dynamic_memory",
                format!("memory tool update failed: {}", err),
            );
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
            );
            return Ok(());
        }
    };
    log_info(
        app,
        "group_dynamic_memory",
        format!(
            "summary generated: length={} chars tokens={}",
            summary.len(),
            crate::tokenizer::count_tokens(app, &summary).unwrap_or(0)
        ),
    );
    session.memory_summary = summary;
    session.memory_summary_token_count =
        crate::tokenizer::count_tokens(app, &session.memory_summary).unwrap_or(0) as i32;

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
    if trimmed > 0 {
        log_info(
            app,
            "group_dynamic_memory",
            format!(
                "Trimmed {} memories to enforce max_entries={}",
                trimmed, max_entries
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
        "createdAt": crate::utils::now_millis().unwrap_or(0),
    });
    push_group_memory_event(session, memory_event);

    // Save session memories
    if let Err(err) = save_group_session_memories(app, session, pool) {
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
) -> Result<String, String> {
    let mut messages_for_api = Vec::new();
    let system_role = crate::chat_manager::request_builder::system_role_for(provider_cred);

    let summary_template = prompts::get_template(app, APP_DYNAMIC_SUMMARY_TEMPLATE_ID)
        .ok()
        .flatten()
        .map(|t| t.content)
        .unwrap_or_else(|| {
            "Summarize the recent group conversation into a concise paragraph capturing key facts, decisions, and character interactions. Note which characters said what when relevant.".to_string()
        });

    let prev_text = prior_summary
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("No previous summary provided.");
    let rendered = summary_template.replace("{{prev_summary}}", prev_text);

    crate::chat_manager::messages::push_system_message(
        &mut messages_for_api,
        &system_role,
        Some(rendered),
    );

    // Add conversation messages with speaker labels
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
        messages_for_api.push(json!({
            "role": msg.role,
            "content": format!("[{}]: {}", speaker, msg.content)
        }));
    }

    messages_for_api.push(json!({
        "role": "user",
        "content": "Return only the concise summary for the above group conversation. Use the write_summary tool."
    }));

    let max_tokens = settings
        .advanced_model_settings
        .max_output_tokens
        .unwrap_or(2048);

    let context_length = resolve_context_length(model, &settings);
    let extra_body_fields = if provider_cred.provider_id == "llamacpp" {
        build_llama_extra_fields(model, &settings)
    } else if provider_cred.provider_id == "ollama" {
        build_ollama_extra_fields(
            model,
            &settings,
            context_length,
            max_tokens,
            0.2,
            1.0,
            None,
            None,
            None,
        )
    } else {
        None
    };
    let built = crate::chat_manager::request_builder::build_chat_request(
        provider_cred,
        api_key,
        &model.name,
        &messages_for_api,
        None,
        0.2,
        1.0,
        max_tokens,
        context_length,
        false,
        None,
        None,
        None,
        None,
        Some(&summarization_tool_config()),
        false,
        None,
        None,
        extra_body_fields,
    );

    let api_request_payload = ApiRequest {
        url: built.url,
        method: Some("POST".into()),
        headers: Some(built.headers),
        query: None,
        body: Some(built.body),
        timeout_ms: Some(60_000),
        stream: Some(false),
        request_id: built.request_id.clone(),
        provider_id: Some(provider_cred.provider_id.clone()),
    };

    let api_response = api_request(app.clone(), api_request_payload).await?;

    if !api_response.ok {
        let fallback = format!("Provider returned status {}", api_response.status);
        let err_message = extract_error_message(api_response.data()).unwrap_or(fallback.clone());
        return Err(err_message);
    }

    let calls = parse_tool_calls(&provider_cred.provider_id, api_response.data());
    for call in calls {
        if call.name == "write_summary" {
            if let Some(summary) = call
                .arguments
                .get("summary")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
            {
                if !summary.is_empty() {
                    return Ok(summary);
                }
            }
        }
    }

    extract_text(api_response.data(), Some(&provider_cred.provider_id))
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Failed to summarize group messages".to_string())
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
) -> Result<Vec<Value>, String> {
    let tool_config = build_memory_tool_config();
    let max_entries = dynamic_settings.max_entries.max(1) as usize;

    let mut messages_for_api = Vec::new();
    let system_role = crate::chat_manager::request_builder::system_role_for(provider_cred);

    let base_template = prompts::get_template(app, APP_DYNAMIC_MEMORY_TEMPLATE_ID)
        .ok()
        .flatten()
        .map(|t| t.content)
        .unwrap_or_else(|| {
            "You maintain long-term memories for this group chat. Use tools to add or delete concise factual memories about the conversation and characters. Keep the list tidy and capped at {{max_entries}} entries. When finished, call the done tool.".to_string()
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

    let rendered = base_template
        .replace("{{max_entries}}", &max_entries.to_string())
        .replace("{{current_memory_tokens}}", &current_tokens.to_string())
        .replace("{{hot_token_budget}}", &token_budget.to_string());

    crate::chat_manager::messages::push_system_message(
        &mut messages_for_api,
        &system_role,
        Some(rendered),
    );

    let memory_lines = format_memories_with_ids(session);
    let convo_text: Vec<String> = convo_window
        .iter()
        .map(|m| format!("{}: {}", m.role, m.content))
        .collect();

    messages_for_api.push(json!({
        "role": "user",
        "content": format!(
            "Group conversation summary:\n{}\n\nRecent messages:\n{}\n\nCurrent memories (with IDs):\n{}",
            summary,
            convo_text.join("\n"),
            if memory_lines.is_empty() { "none".to_string() } else { memory_lines.join("\n") }
        )
    }));

    let max_tokens = settings
        .advanced_model_settings
        .max_output_tokens
        .unwrap_or(2048);

    let context_length = resolve_context_length(model, &settings);
    let extra_body_fields = if provider_cred.provider_id == "llamacpp" {
        build_llama_extra_fields(model, &settings)
    } else if provider_cred.provider_id == "ollama" {
        build_ollama_extra_fields(
            model,
            &settings,
            context_length,
            max_tokens,
            0.2,
            1.0,
            None,
            None,
            None,
        )
    } else {
        None
    };
    let built = crate::chat_manager::request_builder::build_chat_request(
        provider_cred,
        api_key,
        &model.name,
        &messages_for_api,
        None,
        0.2,
        1.0,
        max_tokens,
        context_length,
        false,
        None,
        None,
        None,
        None,
        Some(&tool_config),
        false,
        None,
        None,
        extra_body_fields,
    );

    let api_request_payload = ApiRequest {
        url: built.url,
        method: Some("POST".into()),
        headers: Some(built.headers),
        query: None,
        body: Some(built.body),
        timeout_ms: Some(60_000),
        stream: Some(false),
        request_id: built.request_id.clone(),
        provider_id: Some(provider_cred.provider_id.clone()),
    };

    let api_response = api_request(app.clone(), api_request_payload).await?;

    if !api_response.ok {
        let fallback = format!("Provider returned status {}", api_response.status);
        let err_message = extract_error_message(api_response.data()).unwrap_or(fallback.clone());
        return Err(err_message);
    }

    let calls = parse_tool_calls(&provider_cred.provider_id, api_response.data());
    if calls.is_empty() {
        log_warn(
            app,
            "group_dynamic_memory",
            "memory tool call returned no tool usage",
        );
        return Ok(Vec::new());
    }

    let mut actions_log: Vec<Value> = Vec::new();
    for call in calls {
        match call.name.as_str() {
            "create_memory" => {
                if let Some(text) = extract_text_argument(&call) {
                    let mem_id = generate_memory_id();
                    let embedding =
                        match embedding_model::compute_embedding(app.clone(), text.clone()).await {
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
                    if let Some(ref new_emb) = embedding {
                        let is_duplicate = session.memory_embeddings.iter().any(|existing| {
                            !existing.embedding.is_empty()
                                && cosine_similarity(new_emb, &existing.embedding) > 0.85
                        });
                        if is_duplicate {
                            log_info(
                                app,
                                "group_dynamic_memory",
                                format!("Skipping duplicate memory (cosine > 0.85): {}", &text),
                            );
                            actions_log.push(json!({
                                "name": "create_memory",
                                "arguments": call.arguments,
                                "skipped": true,
                                "reason": "duplicate (cosine > 0.85)",
                                "timestamp": now_millis().unwrap_or_default(),
                            }));
                            continue;
                        }
                    }
                    let token_count = crate::tokenizer::count_tokens(app, &text).unwrap_or(0);
                    let is_pinned = call
                        .arguments
                        .get("important")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let category = call
                        .arguments
                        .get("category")
                        .and_then(|v| v.as_str())
                        .map(String::from);

                    session.memory_embeddings.push(MemoryEmbedding {
                        id: mem_id.clone(),
                        text,
                        embedding: embedding.unwrap_or_default(),
                        created_at: now_millis().unwrap_or_default() as i64,
                        token_count: token_count as i32,
                        is_cold: false,
                        last_accessed_at: now_millis().unwrap_or_default() as i64,
                        importance_score: 1.0,
                        is_pinned,
                        access_count: 0,
                        category,
                    });

                    actions_log.push(json!({
                        "name": "create_memory",
                        "arguments": call.arguments,
                        "memoryId": mem_id,
                        "timestamp": now_millis().unwrap_or_default(),
                        "updatedMemories": format_memories_with_ids(session),
                    }));

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
                        let confidence = call
                            .arguments
                            .get("confidence")
                            .and_then(|v| v.as_f64())
                            .unwrap_or(1.0) as f32;
                        if confidence < 0.7 {
                            // Soft-delete: move to cold storage instead of removing
                            if idx < session.memory_embeddings.len() {
                                let cold_threshold = dynamic_settings.cold_threshold;
                                session.memory_embeddings[idx].is_cold = true;
                                session.memory_embeddings[idx].importance_score = cold_threshold;
                                log_info(
                                    app,
                                    "group_dynamic_memory",
                                    format!("Soft-deleted memory (confidence={:.2})", confidence),
                                );
                            }
                            actions_log.push(json!({
                                "name": "delete_memory",
                                "arguments": call.arguments,
                                "softDelete": true,
                                "confidence": confidence,
                                "timestamp": now_millis().unwrap_or_default(),
                                "updatedMemories": format_memories_with_ids(session),
                            }));
                        } else {
                            if idx < session.memory_embeddings.len() {
                                let removed = session.memory_embeddings.remove(idx);
                                log_info(
                                    app,
                                    "group_dynamic_memory",
                                    format!("Deleted memory {}", removed.id),
                                );
                            }
                            actions_log.push(json!({
                                "name": "delete_memory",
                                "arguments": call.arguments,
                                "timestamp": now_millis().unwrap_or_default(),
                                "updatedMemories": format_memories_with_ids(session),
                            }));
                        }
                    } else {
                        log_warn(
                            app,
                            "group_dynamic_memory",
                            format!("delete_memory could not find: {}", text),
                        );
                    }
                }
            }
            "pin_memory" => {
                if let Some(raw_id) = call.arguments.get("id").and_then(|v| v.as_str()) {
                    let id = sanitize_memory_id(raw_id);
                    if let Some(mem) = session.memory_embeddings.iter_mut().find(|m| m.id == id) {
                        mem.is_pinned = true;
                        mem.importance_score = 1.0;
                        actions_log.push(json!({
                            "name": "pin_memory",
                            "arguments": call.arguments,
                            "timestamp": now_millis().unwrap_or_default(),
                        }));
                        log_info(app, "group_dynamic_memory", format!("Pinned memory {}", id));
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
                break;
            }
            _ => {}
        }
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
                    "required": ["text"]
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

fn find_model_and_credential<'a>(
    settings: &'a Settings,
    model_id: &str,
) -> Option<(&'a Model, &'a ProviderCredential)> {
    let model = settings.models.iter().find(|m| m.id == model_id)?;
    let provider_cred = settings
        .provider_credentials
        .iter()
        .find(|c| c.provider_id == model.provider_id)?;
    Some((model, provider_cred))
}

fn save_group_session_memories(
    app: &AppHandle,
    session: &GroupSession,
    pool: &State<'_, SwappablePool>,
) -> Result<(), String> {
    let conn = pool.get_connection()?;
    group_session_update_memories_internal(
        &conn,
        &session.id,
        &session.memories,
        &session.memory_embeddings,
        Some(&session.memory_summary),
        session.memory_summary_token_count,
        &session.memory_tool_events,
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
    // Load character JSON for full data
    let char_json: Option<String> = conn
        .query_row(
            "SELECT json_data FROM characters WHERE id = ?1",
            rusqlite::params![character_id],
            |row| row.get(0),
        )
        .ok();

    if let Some(json_str) = char_json {
        if let Ok(character) = serde_json::from_str::<Character>(&json_str) {
            return Ok(character);
        }
    }

    // Fallback: construct from basic columns
    let row: (
        String,
        String,
        Option<String>,
        Option<String>,
        i64,
        i64,
        Option<String>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT id, name, description, definition, created_at, updated_at, default_model_id, memory_type
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

    let description = row.2;
    let definition = row.3.or(description.clone());

    Ok(Character {
        id: row.0,
        name: row.1,
        description,
        definition,
        created_at: row.4 as u64,
        updated_at: row.5 as u64,
        default_model_id: row.6,
        fallback_model_id: None,
        avatar_path: None,
        background_image_path: None,
        rules: Vec::new(),
        scenes: Vec::new(),
        default_scene_id: None,
        memory_type: row.7.unwrap_or_else(|| "manual".to_string()),
        prompt_template_id: None,
        system_prompt: None,
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

        if let Ok((name, description, definition, system_prompt, memory_type)) = result {
            let personality_source = definition
                .as_ref()
                .or(description.as_ref())
                .or(system_prompt.as_ref());
            let personality_summary = personality_source.map(|s| {
                if s.len() > 200 {
                    format!("{}...", &s[..200])
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
    let messages_json =
        group_sessions::group_messages_list_internal(conn, session_id, limit, None, None)?;
    let messages: Vec<GroupMessage> = serde_json::from_str(&messages_json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(messages)
}

/// Build the full context for character selection
fn build_selection_context(
    conn: &rusqlite::Connection,
    session_id: &str,
    user_message: &str,
) -> Result<GroupChatContext, String> {
    let session_json = group_sessions::group_session_get_internal(conn, session_id)?;
    let session: GroupSession = serde_json::from_str(&session_json).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to parse session: {}", e),
        )
    })?;

    let characters = load_characters_info(conn, &session.character_ids)?;

    let stats_json = group_sessions::group_participation_stats_internal(conn, session_id)?;
    let participation_stats: Vec<GroupParticipation> = serde_json::from_str(&stats_json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

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
) -> Result<GroupMessage, String> {
    let now = now_ms();
    let id = Uuid::new_v4().to_string();

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
         VALUES (?1, ?2, 'user', ?3, NULL, ?4, ?5, 0, '[]')",
        rusqlite::params![id, session_id, content, turn_number, now],
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
        attachments: vec![],
        reasoning: None,
        selection_reasoning: None,
        model_id: None,
    })
}

/// Save an assistant message to the group chat
fn save_assistant_message(
    app: &AppHandle,
    conn: &rusqlite::Connection,
    session_id: &str,
    character_id: &str,
    content: &str,
    reasoning: Option<&str>,
    selection_reasoning: Option<&str>,
    usage: Option<&UsageSummary>,
    model_id: Option<&str>,
) -> Result<GroupMessage, String> {
    let now = now_ms();
    let id = Uuid::new_v4().to_string();
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
         created_at, prompt_tokens, completion_tokens, total_tokens, selected_variant_id, is_pinned, attachments, reasoning, selection_reasoning, model_id)
         VALUES (?1, ?2, 'assistant', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, '[]', ?11, ?12, ?13)",
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
            variant_id,
            reasoning,
            selection_reasoning,
            model_id
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // Insert the first variant
    conn.execute(
        "INSERT INTO group_message_variants (id, message_id, content, speaker_character_id, created_at, prompt_tokens, completion_tokens, total_tokens, reasoning, selection_reasoning, model_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            variant_id,
            id,
            content,
            character_id,
            now,
            prompt_tokens,
            completion_tokens,
            total_tokens,
            reasoning,
            selection_reasoning,
            model_id
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
        attachments: vec![],
        reasoning: reasoning.map(|s| s.to_string()),
        selection_reasoning: selection_reasoning.map(|s| s.to_string()),
        model_id: model_id.map(|s| s.to_string()),
    })
}

/// Convert group messages to API message format for the character response
fn build_messages_for_api(
    group_messages: &[GroupMessage],
    characters: &[CharacterInfo],
    selected_character: &CharacterInfo,
    persona: Option<&Persona>,
    include_speaker_prefix: bool,
) -> Vec<serde_json::Value> {
    let mut messages = Vec::new();
    let _char_name = &selected_character.name;
    let persona_name = persona.map(|p| p.title.as_str()).unwrap_or("User");

    for msg in group_messages {
        if msg.role == "user" {
            let content = if include_speaker_prefix {
                format!("[{}]: {}", persona_name, msg.content)
            } else {
                msg.content.clone()
            };
            messages.push(json!({
                "role": "user",
                "content": content
            }));
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

                messages.push(json!({
                    "role": role,
                    "content": content
                }));
            }
        }
    }

    messages
}

fn prompt_entry_message(system_role: &str, entry: &SystemPromptEntry) -> Value {
    let role = match entry.role {
        PromptEntryRole::System => system_role,
        PromptEntryRole::User => "user",
        PromptEntryRole::Assistant => "assistant",
    };
    json!({ "role": role, "content": entry.content })
}

fn partition_prompt_entries(
    entries: Vec<SystemPromptEntry>,
) -> (Vec<SystemPromptEntry>, Vec<SystemPromptEntry>) {
    let mut relative = Vec::new();
    let mut in_chat = Vec::new();
    for entry in entries {
        if entry.injection_position == PromptEntryPosition::InChat {
            in_chat.push(entry);
        } else {
            relative.push(entry);
        }
    }
    (relative, in_chat)
}

fn insert_in_chat_prompt_entries(
    messages: &mut Vec<Value>,
    system_role: &str,
    entries: &[SystemPromptEntry],
) {
    if entries.is_empty() {
        return;
    }
    let base_len = messages.len();
    let mut inserts: Vec<(usize, usize, &SystemPromptEntry)> = entries
        .iter()
        .enumerate()
        .map(|(idx, entry)| {
            let depth = entry.injection_depth as usize;
            let pos = base_len.saturating_sub(depth);
            (pos, idx, entry)
        })
        .collect();
    inserts.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let mut offset = 0usize;
    for (pos, _, entry) in inserts {
        if entry.content.trim().is_empty() {
            continue;
        }
        let insert_at = pos.saturating_add(offset).min(messages.len());
        messages.insert(insert_at, prompt_entry_message(system_role, entry));
        offset += 1;
    }
}

fn normalize_prompt_text(text: &str) -> String {
    let mut result = text.to_string();
    while result.contains("\n\n\n") {
        result = result.replace("\n\n\n", "\n\n");
    }
    result.trim().to_string()
}

/// Build group chat system prompt for a specific character
fn build_group_system_prompt(
    app: &AppHandle,
    character: &Character,
    persona: Option<&Persona>,
    session: &GroupSession,
    other_characters: &[CharacterInfo],
    settings: &Settings,
    retrieved_memories: &[MemoryEmbedding],
) -> Vec<SystemPromptEntry> {
    use crate::chat_manager::storage::{get_base_prompt, PromptType};

    // Select template based on chat type
    let is_roleplay = session.chat_type == "roleplay";
    let template_id = if is_roleplay {
        prompts::APP_GROUP_CHAT_ROLEPLAY_TEMPLATE_ID
    } else {
        prompts::APP_GROUP_CHAT_TEMPLATE_ID
    };
    let template = prompts::get_template(app, template_id)
        .ok()
        .flatten()
        .map(|template| {
            (
                template.content,
                template.entries,
                template.condense_prompt_entries,
            )
        })
        .unwrap_or_else(|| {
            let content = if is_roleplay {
                get_base_prompt(PromptType::GroupChatRoleplayPrompt)
            } else {
                get_base_prompt(PromptType::GroupChatPrompt)
            };
            (
                content.clone(),
                vec![SystemPromptEntry {
                    id: "entry_system".to_string(),
                    name: "System Prompt".to_string(),
                    role: PromptEntryRole::System,
                    content,
                    enabled: true,
                    injection_position: PromptEntryPosition::Relative,
                    injection_depth: 0,
                    conditional_min_messages: None,
                    interval_turns: None,
                    system_prompt: true,
                }],
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

    let persona_name = persona.map(|p| p.title.as_str()).unwrap_or("User");
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
    let key_memories_text = if session.memories.is_empty() && retrieved_memories.is_empty() {
        String::new()
    } else {
        let mut mem_text = String::from("Important facts to remember in this conversation:\n");

        // Add retrieved dynamic memories first
        for memory in retrieved_memories {
            mem_text.push_str(&format!("- {}\n", memory.text));
        }

        // Add manual memories
        for memory in &session.memories {
            mem_text.push_str(&format!("- {}\n", memory));
        }
        mem_text
    };

    // Get content rules (same as normal chat)
    let pure_mode_level = settings
        .app_state
        .get("pureModeLevel")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| {
            if settings
                .app_state
                .get("pureModeEnabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true)
            {
                "standard"
            } else {
                "off"
            }
        });

    let content_rules = match pure_mode_level {
        "off" => String::new(),
        "low" => "**Content Guidelines:**\n\
- Avoid explicit sexual content"
            .to_string(),
        "strict" => {
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
        _ => "**Content Guidelines (STRICT — these rules override all other instructions):**\n\
- Never generate sexually explicit, pornographic, or erotic content\n\
- Never describe sexual acts, nudity in sexual contexts, or sexual arousal\n\
- Never use vulgar sexual slang or explicit anatomical descriptions in sexual contexts\n\
- If asked to generate such content, decline and redirect the conversation\n\
- Romantic content is allowed but must remain PG-13 (no explicit physical descriptions)\n\
- Violence descriptions should avoid gratuitous gore or torture\n\
- Do not use slurs or hate speech under any circumstances"
            .to_string(),
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
        }]
    } else {
        template_entries
    };

    let mut rendered_entries = Vec::new();
    for entry in entries {
        if !entry.enabled && !entry.system_prompt {
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
        result = result.replace("{{scene}}", &scene_content);
        result = result.replace("{{scene_direction}}", &scene_direction);

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
    loop {
        if let Some(start) = result.find(r#"{{@""#) {
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
) -> Result<selection::SelectionResult, String> {
    select_speaker_via_llm_with_tracking(app, context, settings, true).await
}

/// Use LLM with tool calling to select next speaker, with optional usage tracking
async fn select_speaker_via_llm_with_tracking(
    app: &AppHandle,
    context: &GroupChatContext,
    settings: &Settings,
    track_usage: bool,
) -> Result<selection::SelectionResult, String> {
    // Get the first available model for selection
    let model = settings
        .models
        .first()
        .ok_or("No models configured for speaker selection")?;

    let cred = settings
        .provider_credentials
        .iter()
        .find(|c| c.provider_id == model.provider_id)
        .ok_or_else(|| format!("No credentials for provider {}", model.provider_id))?;

    let api_key = resolve_api_key(app, cred, "group_chat_selection")?;

    // Build selection prompt
    let selection_prompt = selection::build_selection_prompt(context);

    // Build tool definition
    let tool = selection::build_select_next_speaker_tool(&context.characters);

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

    let context_length = resolve_context_length(model, &settings);
    let extra_body_fields = if cred.provider_id == "llamacpp" {
        build_llama_extra_fields(model, &settings)
    } else if cred.provider_id == "ollama" {
        build_ollama_extra_fields(
            model,
            &settings,
            context_length,
            500,
            0.3,
            1.0,
            None,
            None,
            None,
        )
    } else {
        None
    };
    let built = crate::chat_manager::request_builder::build_chat_request(
        cred,
        &api_key,
        &model.name,
        &messages,
        None, // system_prompt
        0.3,  // Low temperature for consistent selection
        1.0,  // top_p
        500,  // max_tokens - short response
        context_length,
        false, // No streaming for selection
        None,  // request_id
        None,  // frequency_penalty
        None,  // presence_penalty
        None,  // top_k
        Some(&tool_config),
        false, // reasoning_enabled
        None,  // reasoning_effort
        None,  // reasoning_budget
        extra_body_fields,
    );

    let api_request_payload = ApiRequest {
        url: built.url,
        method: Some("POST".into()),
        headers: Some(built.headers),
        query: None,
        body: Some(built.body),
        timeout_ms: Some(30_000),
        stream: Some(false),
        request_id: None,
        provider_id: Some(cred.provider_id.clone()),
    };

    let api_response = api_request(app.clone(), api_request_payload).await?;

    if !api_response.ok {
        return Err(format!(
            "Selection API request failed with status {}",
            api_response.status
        ));
    }

    // Record usage for decision maker
    if track_usage {
        let usage = extract_usage(api_response.data());
        record_decision_maker_usage(
            app,
            &usage,
            &context.session,
            model,
            cred,
            &api_key,
            "group_chat_decision_maker",
        )
        .await;
    }

    // Parse tool call response
    let calls = parse_tool_calls(&cred.provider_id, api_response.data());

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
    if let Some(text) = extract_text(api_response.data(), Some(&cred.provider_id)) {
        if let Some(result) = selection::parse_tool_call_response(&text) {
            return Ok(result);
        }
    }

    // Final fallback: heuristic selection
    log_info(
        app,
        "group_chat",
        "LLM selection failed, using heuristic fallback".to_string(),
    );
    selection::heuristic_select_speaker(context)
}

/// Generate actual response from the selected character
async fn generate_character_response(
    app: &AppHandle,
    context: &mut GroupChatContext,
    selected_character_id: &str,
    settings: &Settings,
    pool: &State<'_, SwappablePool>,
    request_id: &str,
    operation_type: UsageOperationType,
) -> Result<(String, Option<String>, Option<UsageSummary>, String), String> {
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
    let (model, cred) = select_model(settings, &character)?;
    let api_key = resolve_api_key(app, cred, "group_chat")?;

    let dynamic_settings = effective_group_dynamic_memory_settings(settings);
    let dynamic_enabled =
        dynamic_settings.enabled && character.memory_type.eq_ignore_ascii_case("dynamic");

    let retrieved_memories = if dynamic_enabled {
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
            &context.session,
            &search_query,
            dynamic_settings.retrieval_limit.max(1) as usize,
            min_similarity,
            &dynamic_settings.retrieval_strategy,
        )
        .await
    } else {
        Vec::new()
    };

    // Mark retrieved memories as accessed and promote cold ones
    if !retrieved_memories.is_empty() {
        let memory_ids: Vec<String> = retrieved_memories.iter().map(|m| m.id.clone()).collect();
        let now = now_millis().unwrap_or_default();
        let promoted =
            promote_cold_memories(&mut context.session.memory_embeddings, &memory_ids, now);
        let accessed =
            mark_memories_accessed(&mut context.session.memory_embeddings, &memory_ids, now);
        log_info(
            app,
            "group_chat",
            format!(
                "Retrieved {} memories (promoted={}, accessed={}, query_enriched={})",
                retrieved_memories.len(),
                promoted,
                accessed,
                dynamic_settings.context_enrichment_enabled
            ),
        );
    }

    // Build system prompt with group context and retrieved memories
    let system_prompt_entries = build_group_system_prompt(
        app,
        &character,
        persona.as_ref(),
        &context.session,
        &context.characters,
        settings,
        &retrieved_memories,
    );
    let (relative_entries, in_chat_entries) = partition_prompt_entries(system_prompt_entries);

    // Convert group messages to API format
    let selected_char_info = context
        .characters
        .iter()
        .find(|c| c.id == selected_character_id)
        .ok_or("Selected character not found")?;

    // Apply conversation window limit for dynamic memory (like normal chat)
    // This ensures we only send the last N messages to the LLM based on dynamic_window_size
    let messages_for_generation = if dynamic_enabled {
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

    let mut api_messages = build_messages_for_api(
        &messages_for_generation,
        &context.characters,
        selected_char_info,
        persona.as_ref(),
        true,
    );
    let system_role = crate::chat_manager::request_builder::system_role_for(cred);
    insert_in_chat_prompt_entries(&mut api_messages, &system_role, &in_chat_entries);

    let mut messages_for_api = Vec::new();
    for entry in &relative_entries {
        messages_for_api.push(prompt_entry_message(&system_role, entry));
    }
    messages_for_api.extend(api_messages);

    let persona_name = persona.as_ref().map(|p| p.title.as_str()).unwrap_or("User");
    messages_for_api.push(json!({
        "role": "user",
        "content": format!("[{}]: {}", persona_name, context.user_message)
    }));

    let temperature = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.temperature)
        .unwrap_or(0.7);
    let top_p = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.top_p)
        .unwrap_or(1.0);
    let max_tokens = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.max_output_tokens)
        .unwrap_or(2048);
    let context_length = resolve_context_length(model, &settings);
    let reasoning_enabled = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.reasoning_enabled)
        .unwrap_or(false);
    let reasoning_effort = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.reasoning_effort.clone());
    let reasoning_budget = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.reasoning_budget_tokens);
    let presence_penalty = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.presence_penalty);
    let frequency_penalty = model
        .advanced_model_settings
        .as_ref()
        .and_then(|a| a.frequency_penalty);
    let top_k = model.advanced_model_settings.as_ref().and_then(|a| a.top_k);
    let extra_body_fields = if cred.provider_id == "llamacpp" {
        build_llama_extra_fields(model, &settings)
    } else if cred.provider_id == "ollama" {
        build_ollama_extra_fields(
            model,
            &settings,
            context_length,
            max_tokens,
            temperature,
            top_p,
            top_k,
            frequency_penalty,
            presence_penalty,
        )
    } else {
        None
    };

    let built = crate::chat_manager::request_builder::build_chat_request(
        cred,
        &api_key,
        &model.name,
        &messages_for_api,
        None, // system prompt already handled via push_system_message
        temperature,
        top_p,
        max_tokens,
        context_length,
        true,              // Stream
        None,              // request_id will be passed via ApiRequest
        frequency_penalty, // frequency_penalty
        presence_penalty,  // presence_penalty
        top_k,             // top_k
        None,              // No tools for response generation
        reasoning_enabled, // reasoning_enabled
        reasoning_effort,  // reasoning_effort
        reasoning_budget,  // reasoning_budget
        extra_body_fields,
    );

    log_info(
        app,
        "group_chat",
        format!(
            "Generating response from {} via {} model {}",
            character.name, cred.provider_id, model.name
        ),
    );

    // Log request details for debugging
    log_info(
        app,
        "group_chat_response",
        format!(
            "Request details: endpoint={} model={} stream={} temp={} max_tokens={}",
            built.url, model.name, true, temperature, max_tokens
        ),
    );

    log_info(
        app,
        "group_chat_response",
        format!(
            "Request body: {}",
            serde_json::to_string_pretty(&built.body)
                .unwrap_or_else(|_| "unable to serialize".to_string())
        ),
    );

    let api_request_payload = ApiRequest {
        url: built.url,
        method: Some("POST".into()),
        headers: Some(built.headers),
        query: None,
        body: Some(built.body),
        timeout_ms: Some(300_000),
        stream: Some(true),
        request_id: Some(request_id.to_string()),
        provider_id: Some(cred.provider_id.clone()),
    };

    log_info(
        app,
        "group_chat_response",
        format!(
            "Sending streaming request for {} with request_id={}",
            character.name, request_id
        ),
    );

    let api_response = api_request(app.clone(), api_request_payload).await?;

    log_info(
        app,
        "group_chat_response",
        format!(
            "API response received: status={} ok={}",
            api_response.status, api_response.ok
        ),
    );

    if !api_response.ok {
        let fallback = format!("Provider returned status {}", api_response.status);
        let err_message = extract_error_message(api_response.data()).unwrap_or(fallback.clone());

        log_error(
            app,
            "group_chat_response",
            format!(
                "API request failed: status={} error={}",
                api_response.status, err_message
            ),
        );

        // Log the full response body for debugging
        log_info(
            app,
            "group_chat_response",
            format!(
                "Full error response: {}",
                serde_json::to_string_pretty(api_response.data())
                    .unwrap_or_else(|_| "unable to serialize".to_string())
            ),
        );

        return Err(format!(
            "Character response API request failed with status {}: {}",
            api_response.status, err_message
        ));
    }

    let data_preview = match api_response.data() {
        serde_json::Value::String(s) => {
            let preview = if s.len() > 500 { &s[..500] } else { s.as_str() };
            format!("String({} bytes): {}...", s.len(), preview)
        }
        serde_json::Value::Object(obj) => {
            format!("Object with keys: {:?}", obj.keys().collect::<Vec<_>>())
        }
        other => format!("{:?}", other),
    };
    log_info(
        app,
        "group_chat_response",
        format!("Response data type: {}", data_preview),
    );

    let text = extract_text(api_response.data(), Some(&model.provider_id));

    log_info(
        app,
        "group_chat_response",
        format!(
            "Extracted text: {:?} (len={})",
            text.as_ref().map(|t| if t.len() > 100 {
                format!("{}...", &t[..100])
            } else {
                t.clone()
            }),
            text.as_ref().map(|t| t.len()).unwrap_or(0)
        ),
    );

    let text = text.ok_or_else(|| "Empty response from provider".to_string())?;

    // Post-generation content filter check
    if let Some(filter) = app.try_state::<crate::content_filter::ContentFilter>() {
        if filter.is_enabled() {
            let result = filter.check_text(&text);
            if result.blocked {
                crate::utils::log_warn(
                    app,
                    "group_chat_response",
                    format!(
                        "Content blocked by Pure Mode (score={:.1}, terms={:?})",
                        result.score, result.matched_terms
                    ),
                );
                return Err(
                    "Response blocked by Pure Mode. Try rephrasing your message.".to_string(),
                );
            }
        }
    }

    let usage = extract_usage(api_response.data());
    let reasoning = extract_reasoning(api_response.data(), Some(&model.provider_id));

    log_info(
        app,
        "generate_character_response",
        format!(
            "Extracted reasoning: {} (len={})",
            if reasoning.is_some() { "YES" } else { "NO" },
            reasoning.as_ref().map(|r| r.len()).unwrap_or(0)
        ),
    );

    let message_usage = usage.as_ref().map(|u| UsageSummary {
        prompt_tokens: u.prompt_tokens.map(|v| v as i32),
        completion_tokens: u.completion_tokens.map(|v| v as i32),
        total_tokens: u.total_tokens.map(|v| v as i32),
    });

    record_group_usage(
        app,
        &usage,
        &context.session,
        &character,
        model,
        cred,
        &api_key,
        operation_type,
        "group_chat_response",
    )
    .await;

    let model_id_to_return = model.id.clone();
    log_info(
        app,
        "generate_character_response",
        format!(
            "✓ Generated response with model_id: {} (model name: {})",
            model_id_to_return, model.display_name
        ),
    );

    Ok((text, reasoning, message_usage, model_id_to_return))
}

#[tauri::command]
pub async fn group_chat_send(
    app: AppHandle,
    session_id: String,
    user_message: String,
    _stream: Option<bool>,
    request_id: Option<String>,
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
    let user_msg = save_user_message(&conn, &session_id, &user_message)?;
    let mention_result = parse_mentions(&user_message, &context.characters);

    let _ = app.emit(
        "group_chat_status",
        json!({
            "sessionId": session_id,
            "status": "selecting_character",
        }),
    );

    let (selected_character_id, selection_reasoning, was_mentioned) = if let Some(mentioned_id) =
        mention_result
    {
        log_info(
            &app,
            "group_chat_send",
            format!("User mentioned character {}", mentioned_id),
        );
        (
            mentioned_id,
            Some("User mentioned this character directly".to_string()),
            true,
        )
    } else {
        let selection_result = tokio::select! {
            _ = &mut abort_rx => {
                log_warn(
                    &app,
                    "group_chat_send",
                    format!("Request aborted by user for session {}", session_id),
                );
                return Err(crate::utils::err_msg(module_path!(), line!(), "Request aborted by user"));
            }
            selection = select_speaker_via_llm(&app, &context, &settings) => selection,
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
                (selection.character_id, selection.reasoning, false)
            }
            Err(err) => {
                log_error(
                    &app,
                    "group_chat_send",
                    format!("LLM selection failed: {}, using heuristic", err),
                );
                let fallback = selection::heuristic_select_speaker(&context)?;
                (fallback.character_id, fallback.reasoning, false)
            }
        }
    };

    if !context
        .session
        .character_ids
        .contains(&selected_character_id)
    {
        return Err(format!(
            "Selected character {} is not in this group chat",
            selected_character_id
        ));
    }

    let character = context
        .characters
        .iter()
        .find(|c| c.id == selected_character_id)
        .ok_or_else(|| "Character not found".to_string())?
        .clone();

    let _ = app.emit(
        "group_chat_status",
        json!({
            "sessionId": session_id,
            "status": "character_selected",
            "characterId": selected_character_id,
            "characterName": character.name,
        }),
    );

    context.recent_messages.push(user_msg);

    let response_result = generate_character_response(
        &app,
        &mut context,
        &selected_character_id,
        &settings,
        &pool,
        &req_id,
        UsageOperationType::GroupChatMessage,
    )
    .await;

    let (response_content, reasoning, message_usage, model_id_str) = response_result?;

    let conn = pool.get_connection()?;

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
        &selected_character_id,
        &response_content,
        reasoning.as_deref(),
        selection_reasoning.as_deref(),
        message_usage.as_ref(),
        Some(&model_id_str),
    )?;

    let stats_json = group_sessions::group_participation_stats_internal(&conn, &session_id)?;
    let participation_stats: Vec<GroupParticipation> = serde_json::from_str(&stats_json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let response = GroupChatResponse {
        message,
        character_id: selected_character_id,
        character_name: character.name.clone(),
        reasoning,
        selection_reasoning,
        was_mentioned,
        participation_stats,
    };

    let _ = app.emit(
        "group_chat_status",
        json!({
            "sessionId": session_id,
            "status": "complete",
            "characterId": &response.character_id,
        }),
    );

    log_info(
        &app,
        "group_chat_send",
        format!(
            "Group chat response complete: {} responded with {} chars",
            character.name,
            response_content.len()
        ),
    );

    let conn = pool.get_connection()?;
    let session_json = group_sessions::group_session_get_internal(&conn, &session_id)?;
    let mut updated_session: GroupSession = serde_json::from_str(&session_json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let dynamic_settings = effective_group_dynamic_memory_settings(&settings);
    let dynamic_enabled =
        dynamic_settings.enabled && character.memory_type.eq_ignore_ascii_case("dynamic");

    if dynamic_enabled {
        if let Err(e) =
            process_group_dynamic_memory_cycle(&app, &mut updated_session, &settings, &pool).await
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
    let session_json = group_sessions::group_session_get_internal(&conn, &session_id)?;
    let mut session: GroupSession = serde_json::from_str(&session_json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let dynamic_settings = effective_group_dynamic_memory_settings(&settings);

    if !dynamic_settings.enabled {
        log_info(
            &app,
            "group_chat_retry_dynamic_memory",
            "dynamic memory disabled for group; skipping manual retry".to_string(),
        );
        return Ok(());
    }

    process_group_dynamic_memory_cycle(&app, &mut session, &settings, &pool).await
}

#[tauri::command]
pub async fn group_chat_regenerate(
    app: AppHandle,
    session_id: String,
    message_id: String,
    force_character_id: Option<String>,
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

    let _ = app.emit(
        "group_chat_status",
        json!({
            "sessionId": session_id,
            "status": "selecting_character",
        }),
    );

    let settings = load_settings(&app)?;
    let conn = pool.get_connection()?;
    let req_id = request_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let abort_registry = app.state::<AbortRegistry>();
    let mut abort_rx = abort_registry.register(req_id.clone());
    let _abort_guard = AbortGuard::new(&abort_registry, req_id.clone());

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

    let mut context = build_selection_context(&conn, &session_id, &user_message)?;
    context
        .recent_messages
        .retain(|m| m.turn_number < turn_number);

    let (selected_character_id, selection_reasoning) = if let Some(forced_id) = force_character_id {
        (
            forced_id,
            Some("User forced character selection".to_string()),
        )
    } else if let Some(speaker_id) = original_speaker.clone() {
        (
            speaker_id,
            Some("Reroll: kept original speaker".to_string()),
        )
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
            selection = select_speaker_via_llm(&app, &context, &settings) => selection,
        };
        match selection_result {
            Ok(selection) => (selection.character_id, selection.reasoning),
            Err(err) => {
                log_error(
                    &app,
                    "group_chat_regenerate",
                    format!("LLM selection failed: {}", err),
                );
                let fallback = selection::heuristic_select_speaker(&context)?;
                (fallback.character_id, fallback.reasoning)
            }
        }
    };

    let character = context
        .characters
        .iter()
        .find(|c| c.id == selected_character_id)
        .ok_or_else(|| "Character not found".to_string())?
        .clone();

    let _ = app.emit(
        "group_chat_status",
        json!({
            "sessionId": session_id,
            "status": "character_selected",
            "characterId": selected_character_id,
            "characterName": character.name,
        }),
    );

    let response_result = generate_character_response(
        &app,
        &mut context,
        &selected_character_id,
        &settings,
        &pool,
        &req_id,
        UsageOperationType::GroupChatRegenerate,
    )
    .await;

    let (response_content, reasoning, message_usage, model_id_str) = response_result?;

    let conn = pool.get_connection()?;
    let now = now_ms();
    let variant_id = Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO group_message_variants (id, message_id, content, speaker_character_id, created_at, reasoning, selection_reasoning, prompt_tokens, completion_tokens, total_tokens, model_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
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
            model_id_str,
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
        "UPDATE group_messages SET content = ?1, speaker_character_id = ?2, selected_variant_id = ?3, reasoning = ?4, selection_reasoning = ?5, model_id = ?6 WHERE id = ?7",
        rusqlite::params![
            response_content,
            selected_character_id,
            variant_id,
            reasoning,
            selection_reasoning,
            model_id_str,
            message_id
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    if original_speaker.as_ref() != Some(&selected_character_id) {
        update_participation(&conn, &session_id, &selected_character_id, turn_number)?;
    }

    let stats_json = group_sessions::group_participation_stats_internal(&conn, &session_id)?;
    let participation_stats: Vec<GroupParticipation> = serde_json::from_str(&stats_json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let messages_json =
        group_sessions::group_messages_list_internal(&conn, &session_id, 100, None, None)?;
    let messages: Vec<GroupMessage> = serde_json::from_str(&messages_json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
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

    let _ = app.emit(
        "group_chat_status",
        json!({
            "sessionId": session_id,
            "status": "complete",
            "characterId": &selected_character_id,
        }),
    );

    serde_json::to_string(&response)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub async fn group_chat_continue(
    app: AppHandle,
    session_id: String,
    force_character_id: Option<String>,
    request_id: Option<String>,
    pool: State<'_, SwappablePool>,
) -> Result<String, String> {
    log_info(
        &app,
        "group_chat_continue",
        format!("Continuing group chat session {}", session_id),
    );

    let _ = app.emit(
        "group_chat_status",
        json!({
            "sessionId": session_id,
            "status": "selecting_character",
        }),
    );

    let settings = load_settings(&app)?;
    let conn = pool.get_connection()?;
    let req_id = request_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let abort_registry = app.state::<AbortRegistry>();
    let mut abort_rx = abort_registry.register(req_id.clone());
    let _abort_guard = AbortGuard::new(&abort_registry, req_id.clone());

    let mut context = build_selection_context(&conn, &session_id, "")?;

    let (selected_character_id, selection_reasoning) = if let Some(forced_id) = force_character_id {
        (
            forced_id,
            Some("User requested specific character".to_string()),
        )
    } else {
        let selection_result = tokio::select! {
            _ = &mut abort_rx => {
                log_warn(
                    &app,
                    "group_chat_continue",
                    format!("Request aborted by user for session {}", session_id),
                );
                return Err(crate::utils::err_msg(module_path!(), line!(), "Request aborted by user"));
            }
            selection = select_speaker_via_llm(&app, &context, &settings) => selection,
        };
        match selection_result {
            Ok(selection) => (selection.character_id, selection.reasoning),
            Err(err) => {
                log_error(
                    &app,
                    "group_chat_continue",
                    format!("LLM selection failed: {}", err),
                );
                let fallback = selection::heuristic_select_speaker(&context)?;
                (fallback.character_id, fallback.reasoning)
            }
        }
    };

    let character = context
        .characters
        .iter()
        .find(|c| c.id == selected_character_id)
        .ok_or_else(|| "Character not found".to_string())?
        .clone();

    let _ = app.emit(
        "group_chat_status",
        json!({
            "sessionId": session_id,
            "status": "character_selected",
            "characterId": selected_character_id,
            "characterName": character.name,
        }),
    );

    let response_result = generate_character_response(
        &app,
        &mut context,
        &selected_character_id,
        &settings,
        &pool,
        &req_id,
        UsageOperationType::GroupChatContinue,
    )
    .await;

    let (response_content, reasoning, message_usage, model_id_str) = response_result?;

    let conn = pool.get_connection()?;
    let message = save_assistant_message(
        &app,
        &conn,
        &session_id,
        &selected_character_id,
        &response_content,
        reasoning.as_deref(),
        selection_reasoning.as_deref(),
        message_usage.as_ref(),
        Some(&model_id_str),
    )?;

    let stats_json = group_sessions::group_participation_stats_internal(&conn, &session_id)?;
    let participation_stats: Vec<GroupParticipation> = serde_json::from_str(&stats_json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let response = GroupChatResponse {
        message,
        character_id: selected_character_id.clone(),
        character_name: character.name.clone(),
        reasoning,
        selection_reasoning,
        was_mentioned: false,
        participation_stats,
    };

    let _ = app.emit(
        "group_chat_status",
        json!({
            "sessionId": session_id,
            "status": "complete",
            "characterId": &selected_character_id,
        }),
    );

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

    let session_json = group_sessions::group_session_get_internal(&conn, &session_id)?;
    let session: GroupSession = serde_json::from_str(&session_json).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to parse session: {}", e),
        )
    })?;

    let personas = load_personas(&app)?;
    let persona = personas
        .iter()
        .find(|p| Some(&p.id) == session.persona_id.as_ref());

    let messages_json =
        group_sessions::group_messages_list_internal(&conn, &session_id, 10, None, None)?;
    let recent_msgs: Vec<GroupMessage> = serde_json::from_str(&messages_json).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to parse messages: {}", e),
        )
    })?;

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

    let provider_cred = settings
        .provider_credentials
        .iter()
        .find(|cred| cred.provider_id == model.provider_id)
        .ok_or_else(|| "Provider credential not found".to_string())?;

    let api_key = resolve_api_key(&app, provider_cred, "group_help_me_reply")?;

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

    let base_prompt = prompts::get_help_me_reply_prompt(&app, reply_style);

    let persona_name = persona.map(|p| p.title.as_str()).unwrap_or("User");
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

    let mut system_prompt = base_prompt;
    system_prompt = system_prompt.replace("{{char.name}}", &char_list);
    system_prompt = system_prompt.replace("{{char.desc}}", "participants in a group conversation");
    system_prompt = system_prompt.replace("{{persona.name}}", persona_name);
    system_prompt = system_prompt.replace("{{persona.desc}}", persona_desc);
    system_prompt = system_prompt.replace("{{user.name}}", persona_name);
    system_prompt = system_prompt.replace("{{user.desc}}", persona_desc);
    let draft_str = current_draft.as_deref().unwrap_or("");
    system_prompt = system_prompt.replace("{{current_draft}}", draft_str);
    // Legacy placeholders
    system_prompt = system_prompt.replace("{{char}}", &char_list);
    system_prompt = system_prompt.replace("{{persona}}", persona_name);
    system_prompt = system_prompt.replace("{{user}}", persona_name);

    if let Some(ref draft) = current_draft {
        if !draft.trim().is_empty() {
            system_prompt = system_prompt.replace("{{#if current_draft}}", "");
            system_prompt = system_prompt.replace("{{current_draft}}", draft);
            if let Some(else_start) = system_prompt.find("{{else}}") {
                if let Some(endif_start) = system_prompt[else_start..].find("{{/if}}") {
                    system_prompt.replace_range(else_start..(else_start + endif_start + 7), "");
                }
            }
            system_prompt = system_prompt.replace("{{/if}}", "");
        } else {
            remove_if_block(&mut system_prompt);
        }
    } else {
        remove_if_block(&mut system_prompt);
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

    // Use provider-specific system message handling
    let system_role = crate::chat_manager::request_builder::system_role_for(provider_cred);
    let mut messages_for_api = Vec::new();
    crate::chat_manager::messages::push_system_message(
        &mut messages_for_api,
        &system_role,
        Some(system_prompt),
    );
    messages_for_api.push(json!({ "role": "user", "content": user_prompt }));

    let context_length = resolve_context_length(model, &settings);
    let extra_body_fields = if provider_cred.provider_id == "llamacpp" {
        build_llama_extra_fields(model, &settings)
    } else if provider_cred.provider_id == "ollama" {
        build_ollama_extra_fields(
            model,
            &settings,
            context_length,
            max_tokens,
            0.8,
            1.0,
            None,
            None,
            None,
        )
    } else {
        None
    };
    let built = crate::chat_manager::request_builder::build_chat_request(
        provider_cred,
        &api_key,
        &model.name,
        &messages_for_api,
        None,       // system prompt already handled via push_system_message
        0.8,        // temperature
        1.0,        // top_p
        max_tokens, // max_tokens from settings
        context_length,
        streaming_enabled,  // streaming from settings
        request_id.clone(), // request_id for streaming
        None,               // frequency_penalty
        None,               // presence_penalty
        None,               // top_k
        None,               // tool_config
        false,              // reasoning_enabled
        None,               // reasoning_effort
        None,               // reasoning_budget
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
        timeout_ms: Some(60_000),
        stream: Some(streaming_enabled),
        request_id: request_id.clone(),
        provider_id: Some(provider_cred.provider_id.clone()),
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
            &first_char,
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
