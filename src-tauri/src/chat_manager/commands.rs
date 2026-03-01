use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use rusqlite::{params, OptionalExtension};

use crate::api::{api_request, ApiRequest};
use crate::chat_manager::storage::{get_base_prompt, PromptType};
use crate::embedding_model;
use crate::storage_manager::db::open_db;
use crate::utils::{emit_toast, log_error, log_info, log_warn, now_millis};

use super::dynamic_memory::{
    apply_memory_decay, calculate_hot_memory_tokens, context_enrichment_enabled, cosine_similarity,
    dynamic_cold_threshold, dynamic_decay_rate, dynamic_hot_memory_token_budget,
    dynamic_max_entries, dynamic_min_similarity, dynamic_retrieval_limit,
    dynamic_retrieval_strategy, dynamic_window_size, enforce_hot_memory_budget, ensure_pinned_hot,
    generate_memory_id, mark_memories_accessed, normalize_query_text, promote_cold_memories,
    search_cold_memory_indices_by_keyword, select_relevant_memory_indices,
    select_top_cosine_memory_indices, trim_memories_to_max,
};
use super::prompt_engine;
use super::prompts;
use super::prompts::{APP_DYNAMIC_MEMORY_TEMPLATE_ID, APP_DYNAMIC_SUMMARY_TEMPLATE_ID};
use super::request::{
    ensure_assistant_variant, extract_error_message, extract_reasoning, extract_text,
    extract_usage, new_assistant_variant, push_assistant_variant,
};
use super::service::{
    record_failed_usage, record_usage_if_available, resolve_api_key, ChatContext,
};
use crate::usage::tracking::UsageOperationType;

use super::storage::{
    default_character_rules, recent_messages, resolve_provider_credential_for_model, save_session,
};
use super::tooling::{parse_tool_calls, ToolCall, ToolChoice, ToolConfig, ToolDefinition};
use super::types::{
    Character, ChatAddMessageAttachmentArgs, ChatCompletionArgs, ChatContinueArgs,
    ChatRegenerateArgs, ChatTurnResult, ContinueResult, MemoryEmbedding, MemoryRetrievalStrategy,
    Model, Persona, PromptEntryPosition, PromptScope, ProviderCredential, RegenerateResult,
    Session, Settings, StoredMessage, SystemPromptEntry, SystemPromptTemplate,
};
use crate::storage_manager::sessions::{
    messages_upsert_batch, session_conversation_count, session_upsert_meta,
};
use crate::utils::emit_debug;

const FALLBACK_TEMPERATURE: f64 = 0.7;
const FALLBACK_TOP_P: f64 = 1.0;
const FALLBACK_MAX_OUTPUT_TOKENS: u32 = 4096;
const ALLOWED_MEMORY_CATEGORIES: &[&str] = &[
    "character_trait",
    "relationship",
    "plot_event",
    "world_detail",
    "preference",
    "other",
];

fn resolve_persona_id<'a>(session: &'a Session, explicit: Option<&'a str>) -> Option<&'a str> {
    if explicit.is_some() {
        return explicit;
    }
    if session.persona_disabled {
        Some("")
    } else {
        session.persona_id.as_deref()
    }
}

/// Determines if dynamic memory is currently active for this character.
/// Returns true ONLY if BOTH conditions are met:
/// 1. Global dynamic memory setting is enabled in advanced settings
/// 2. Character's memory_type is set to "dynamic"
///
/// If either condition is false, the system falls back to manual memory mode
/// (using session.memories) without modifying the character's memory_type setting.
fn is_dynamic_memory_active(
    settings: &Settings,
    session_character: &super::types::Character,
) -> bool {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|a| a.dynamic_memory.as_ref())
        .map(|dm| dm.enabled)
        .unwrap_or(false)
        && session_character
            .memory_type
            .eq_ignore_ascii_case("dynamic")
}

#[allow(dead_code)]
fn has_image_generation_model(settings: &Settings) -> bool {
    settings.models.iter().any(|m| {
        m.output_scopes
            .iter()
            .any(|s| s.eq_ignore_ascii_case("image"))
    })
}

fn append_image_directive_instructions(
    system_prompt_entries: Vec<SystemPromptEntry>,
    _settings: &Settings,
) -> Vec<SystemPromptEntry> {
    system_prompt_entries
}

fn prompt_entry_to_message(system_role: &str, entry: &SystemPromptEntry) -> Value {
    let role = match entry.role {
        super::types::PromptEntryRole::System => system_role,
        super::types::PromptEntryRole::User => "user",
        super::types::PromptEntryRole::Assistant => "assistant",
    };
    json!({ "role": role, "content": entry.content })
}

fn partition_prompt_entries(
    entries: Vec<SystemPromptEntry>,
) -> (Vec<SystemPromptEntry>, Vec<SystemPromptEntry>) {
    let mut relative = Vec::new();
    let mut in_chat = Vec::new();
    for entry in entries {
        match entry.injection_position {
            PromptEntryPosition::Relative => relative.push(entry),
            PromptEntryPosition::InChat
            | PromptEntryPosition::Conditional
            | PromptEntryPosition::Interval => in_chat.push(entry),
        }
    }
    (relative, in_chat)
}

fn should_insert_in_chat_prompt_entry(entry: &SystemPromptEntry, turn_count: usize) -> bool {
    match entry.injection_position {
        PromptEntryPosition::InChat => true,
        PromptEntryPosition::Conditional => {
            let min_messages = entry.conditional_min_messages.unwrap_or(1) as usize;
            turn_count >= min_messages
        }
        PromptEntryPosition::Interval => {
            let interval = entry.interval_turns.unwrap_or(0) as usize;
            interval > 0 && turn_count > 0 && turn_count % interval == 0
        }
        PromptEntryPosition::Relative => false,
    }
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
    let turn_count = base_len;
    let mut inserts: Vec<(usize, usize, &SystemPromptEntry)> = entries
        .iter()
        .enumerate()
        .filter_map(|(idx, entry)| {
            if !should_insert_in_chat_prompt_entry(entry, turn_count) {
                return None;
            }
            let depth = entry.injection_depth as usize;
            let pos = base_len.saturating_sub(depth);
            Some((pos, idx, entry))
        })
        .collect();
    inserts.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let mut offset = 0usize;
    for (pos, _, entry) in inserts {
        if entry.content.trim().is_empty() {
            continue;
        }
        let insert_at = pos.saturating_add(offset).min(messages.len());
        messages.insert(insert_at, prompt_entry_to_message(system_role, entry));
        offset += 1;
    }
}

fn manual_window_size(settings: &Settings) -> usize {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|a| a.manual_mode_context_window)
        .unwrap_or(50) as usize
}

/// Calculate total tokens used by hot (non-cold) memories
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

/// Extract pinned and unpinned conversation messages separately.
/// Pinned messages are always included but don't count against the window limit.
/// Returns (pinned_messages, recent_unpinned_messages_within_limit)
fn conversation_window_with_pinned(
    messages: &[StoredMessage],
    limit: usize,
) -> (Vec<StoredMessage>, Vec<StoredMessage>) {
    let convo: Vec<StoredMessage> = messages
        .iter()
        .filter(|m| m.role == "user" || m.role == "assistant")
        .cloned()
        .collect();

    let mut pinned = Vec::new();
    let mut unpinned = Vec::new();

    for msg in convo {
        if msg.is_pinned {
            pinned.push(msg);
        } else {
            unpinned.push(msg);
        }
    }

    // Apply sliding window to unpinned messages only
    if unpinned.len() > limit {
        unpinned.drain(0..(unpinned.len() - limit));
    }

    (pinned, unpinned)
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

/// Build an enriched query from the last 2 messages for better memory retrieval.
/// Cases:
/// - [assistant, user] -> assistant.content + user.content (normal chat)
/// - [assistant, assistant] -> prev.content + last.content (chat continue)
/// - [user, user] -> prev.content + last.content (cancelled retry)
/// Falls back to just the latest message if only 1 exists.
fn build_enriched_query(messages: &[StoredMessage]) -> String {
    let convo: Vec<&StoredMessage> = messages
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

fn format_memories_with_ids(session: &Session) -> Vec<String> {
    session
        .memory_embeddings
        .iter()
        .map(|m| format!("[{}] {}", m.id, m.text))
        .collect()
}

fn resolve_temperature(session: &Session, model: &Model, settings: &Settings) -> f64 {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.temperature)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.temperature)
        })
        .or(settings.advanced_model_settings.temperature)
        .unwrap_or(FALLBACK_TEMPERATURE)
}

fn resolve_top_p(session: &Session, model: &Model, settings: &Settings) -> f64 {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.top_p)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.top_p)
        })
        .or(settings.advanced_model_settings.top_p)
        .unwrap_or(FALLBACK_TOP_P)
}

fn resolve_max_tokens(session: &Session, model: &Model, settings: &Settings) -> u32 {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.max_output_tokens)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.max_output_tokens)
        })
        .or(settings.advanced_model_settings.max_output_tokens)
        .unwrap_or(FALLBACK_MAX_OUTPUT_TOKENS)
}

fn resolve_context_length(session: &Session, model: &Model, settings: &Settings) -> Option<u32> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.context_length)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.context_length)
        })
        .or(settings.advanced_model_settings.context_length)
        .filter(|v| *v > 0)
}

fn resolve_frequency_penalty(
    session: &Session,
    model: &Model,
    _settings: &Settings,
) -> Option<f64> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.frequency_penalty)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.frequency_penalty)
        })
}

fn resolve_presence_penalty(session: &Session, model: &Model, _settings: &Settings) -> Option<f64> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.presence_penalty)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.presence_penalty)
        })
}

fn resolve_top_k(session: &Session, model: &Model, _settings: &Settings) -> Option<u32> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.top_k)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.top_k)
        })
}

fn resolve_llama_gpu_layers(session: &Session, model: &Model, settings: &Settings) -> Option<u32> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_gpu_layers)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_gpu_layers)
        })
        .or(settings.advanced_model_settings.llama_gpu_layers)
}

fn resolve_llama_threads(session: &Session, model: &Model, settings: &Settings) -> Option<u32> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_threads)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_threads)
        })
        .or(settings.advanced_model_settings.llama_threads)
}

fn resolve_llama_threads_batch(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<u32> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_threads_batch)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_threads_batch)
        })
        .or(settings.advanced_model_settings.llama_threads_batch)
}

fn resolve_llama_seed(session: &Session, model: &Model, settings: &Settings) -> Option<u32> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_seed)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_seed)
        })
        .or(settings.advanced_model_settings.llama_seed)
}

fn resolve_llama_rope_freq_base(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<f64> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_rope_freq_base)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_rope_freq_base)
        })
        .or(settings.advanced_model_settings.llama_rope_freq_base)
}

fn resolve_llama_rope_freq_scale(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<f64> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_rope_freq_scale)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_rope_freq_scale)
        })
        .or(settings.advanced_model_settings.llama_rope_freq_scale)
}

fn resolve_llama_offload_kqv(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<bool> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_offload_kqv)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_offload_kqv)
        })
        .or(settings.advanced_model_settings.llama_offload_kqv)
}

fn resolve_llama_batch_size(session: &Session, model: &Model, settings: &Settings) -> Option<u32> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_batch_size)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_batch_size)
        })
        .or(settings.advanced_model_settings.llama_batch_size)
        .filter(|v| *v > 0)
}

fn resolve_llama_kv_type(session: &Session, model: &Model, settings: &Settings) -> Option<String> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_kv_type.clone())
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_kv_type.clone())
        })
        .or_else(|| settings.advanced_model_settings.llama_kv_type.clone())
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
}

fn resolve_llama_flash_attention(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<String> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_flash_attention.clone())
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_flash_attention.clone())
        })
        .or_else(|| settings.advanced_model_settings.llama_flash_attention.clone())
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
}

fn build_llama_extra_fields(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<HashMap<String, Value>> {
    let mut extra = HashMap::new();
    if let Some(v) = resolve_llama_gpu_layers(session, model, settings) {
        extra.insert("llamaGpuLayers".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_threads(session, model, settings) {
        extra.insert("llamaThreads".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_threads_batch(session, model, settings) {
        extra.insert("llamaThreadsBatch".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_seed(session, model, settings) {
        extra.insert("llamaSeed".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_rope_freq_base(session, model, settings) {
        extra.insert("llamaRopeFreqBase".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_rope_freq_scale(session, model, settings) {
        extra.insert("llamaRopeFreqScale".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_offload_kqv(session, model, settings) {
        extra.insert("llamaOffloadKqv".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_batch_size(session, model, settings) {
        extra.insert("llamaBatchSize".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_kv_type(session, model, settings) {
        extra.insert("llamaKvType".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_flash_attention(session, model, settings) {
        extra.insert("llamaFlashAttentionPolicy".to_string(), json!(v));
    }

    if extra.is_empty() {
        None
    } else {
        Some(extra)
    }
}

fn build_ollama_extra_fields(
    session: &Session,
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

    let num_ctx = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_num_ctx)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_num_ctx)
        })
        .or(settings.advanced_model_settings.ollama_num_ctx)
        .or(context_length);
    let num_predict = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_num_predict)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_num_predict)
        })
        .or(settings.advanced_model_settings.ollama_num_predict)
        .or(Some(max_tokens));
    let num_keep = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_num_keep)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_num_keep)
        })
        .or(settings.advanced_model_settings.ollama_num_keep);
    let num_batch = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_num_batch)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_num_batch)
        })
        .or(settings.advanced_model_settings.ollama_num_batch);
    let num_gpu = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_num_gpu)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_num_gpu)
        })
        .or(settings.advanced_model_settings.ollama_num_gpu);
    let num_thread = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_num_thread)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_num_thread)
        })
        .or(settings.advanced_model_settings.ollama_num_thread);
    let tfs_z = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_tfs_z)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_tfs_z)
        })
        .or(settings.advanced_model_settings.ollama_tfs_z);
    let typical_p = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_typical_p)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_typical_p)
        })
        .or(settings.advanced_model_settings.ollama_typical_p);
    let min_p = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_min_p)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_min_p)
        })
        .or(settings.advanced_model_settings.ollama_min_p);
    let mirostat = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_mirostat)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_mirostat)
        })
        .or(settings.advanced_model_settings.ollama_mirostat);
    let mirostat_tau = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_mirostat_tau)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_mirostat_tau)
        })
        .or(settings.advanced_model_settings.ollama_mirostat_tau);
    let mirostat_eta = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_mirostat_eta)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_mirostat_eta)
        })
        .or(settings.advanced_model_settings.ollama_mirostat_eta);
    let repeat_penalty = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_repeat_penalty)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_repeat_penalty)
        })
        .or(settings.advanced_model_settings.ollama_repeat_penalty);
    let seed = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_seed)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_seed)
        })
        .or(settings.advanced_model_settings.ollama_seed);
    let stop = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_stop.clone())
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_stop.clone())
        })
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

fn resolve_reasoning_enabled(session: &Session, model: &Model, _settings: &Settings) -> bool {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.reasoning_enabled)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.reasoning_enabled)
        })
        .unwrap_or(false)
}

fn resolve_reasoning_effort(
    session: &Session,
    model: &Model,
    _settings: &Settings,
) -> Option<String> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.reasoning_effort.clone())
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.reasoning_effort.clone())
        })
}

fn resolve_reasoning_budget(
    session: &Session,
    model: &Model,
    _settings: &Settings,
    reasoning_effort: Option<&str>,
) -> Option<u32> {
    // First check for explicit budget
    let explicit_budget = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.reasoning_budget_tokens)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.reasoning_budget_tokens)
        });

    if explicit_budget.is_some() {
        return explicit_budget;
    }

    // Default budget based on effort level
    reasoning_effort.map(|effort| match effort {
        "low" => 2048,
        "medium" => 8192,
        "high" => 16384,
        _ => 4096, // default fallback
    })
}

async fn select_relevant_memories(
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

    let query_embedding =
        match embedding_model::compute_embedding(app.clone(), query.to_string()).await {
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

use super::types::ImageAttachment;
use crate::storage_manager::media::storage_save_session_attachment;

fn persist_attachments(
    app: &AppHandle,
    character_id: &str,
    session_id: &str,
    message_id: &str,
    role: &str,
    attachments: Vec<ImageAttachment>,
) -> Result<Vec<ImageAttachment>, String> {
    let mut persisted = Vec::new();

    for attachment in attachments {
        if attachment.storage_path.is_some() && attachment.data.is_empty() {
            persisted.push(attachment);
            continue;
        }

        if attachment.data.is_empty() {
            continue;
        }

        let storage_path = storage_save_session_attachment(
            app.clone(),
            character_id.to_string(),
            session_id.to_string(),
            message_id.to_string(),
            attachment.id.clone(),
            role.to_string(),
            attachment.data.clone(),
        )?;

        persisted.push(ImageAttachment {
            id: attachment.id,
            data: String::new(),
            mime_type: attachment.mime_type,
            filename: attachment.filename,
            width: attachment.width,
            height: attachment.height,
            storage_path: Some(storage_path),
        });
    }

    Ok(persisted)
}

use crate::storage_manager::media::storage_load_session_attachment;

fn load_attachment_data(app: &AppHandle, message: &StoredMessage) -> StoredMessage {
    let mut loaded_message = message.clone();

    loaded_message.attachments = message
        .attachments
        .iter()
        .map(|attachment| {
            if !attachment.data.is_empty() {
                return attachment.clone();
            }

            let storage_path = match &attachment.storage_path {
                Some(path) => path,
                None => return attachment.clone(),
            };

            match storage_load_session_attachment(app.clone(), storage_path.clone()) {
                Ok(data) => ImageAttachment {
                    id: attachment.id.clone(),
                    data,
                    mime_type: attachment.mime_type.clone(),
                    filename: attachment.filename.clone(),
                    width: attachment.width,
                    height: attachment.height,
                    storage_path: attachment.storage_path.clone(),
                },
                Err(_) => attachment.clone(),
            }
        })
        .collect();

    loaded_message
}

fn role_swap_enabled(flag: Option<bool>) -> bool {
    flag.unwrap_or(false)
}

fn swap_role_for_api(role: &str) -> &str {
    match role {
        "user" => "assistant",
        "assistant" => "user",
        _ => role,
    }
}

fn maybe_swap_message_for_api(message: &StoredMessage, swap_places: bool) -> StoredMessage {
    if !swap_places {
        return message.clone();
    }
    let mut swapped = message.clone();
    swapped.role = swap_role_for_api(message.role.as_str()).to_string();
    swapped
}

fn swapped_prompt_entities(
    character: &Character,
    persona: Option<&Persona>,
) -> (Character, Option<Persona>) {
    let Some(persona) = persona else {
        return (character.clone(), None);
    };

    let mut swapped_character = character.clone();
    swapped_character.name = persona.title.clone();
    swapped_character.definition = Some(persona.description.clone());
    swapped_character.description = Some(persona.description.clone());

    let mut swapped_persona = persona.clone();
    swapped_persona.title = character.name.clone();
    swapped_persona.description = character
        .definition
        .clone()
        .or(character.description.clone())
        .unwrap_or_default();

    (swapped_character, Some(swapped_persona))
}

#[tauri::command]
pub async fn chat_completion(
    app: AppHandle,
    args: ChatCompletionArgs,
) -> Result<ChatTurnResult, String> {
    let ChatCompletionArgs {
        session_id,
        character_id,
        user_message,
        persona_id,
        swap_places,
        stream,
        request_id,
        attachments,
    } = args;
    let swap_places = role_swap_enabled(swap_places);

    log_info(
        &app,
        "chat_completion",
        format!(
            "start session={} character={} stream={:?} request_id={:?}",
            &session_id, &character_id, stream, request_id
        ),
    );

    let context = ChatContext::initialize(app.clone())?;
    let settings = &context.settings;

    emit_debug(
        &app,
        "loading_character",
        json!({ "characterId": character_id.clone() }),
    );

    let character = match context.find_character(&character_id) {
        Ok(found) => found,
        Err(err) => {
            log_error(
                &app,
                "chat_completion",
                format!("character {} not found", &character_id),
            );
            return Err(err);
        }
    };

    let mut session = match context.load_session(&session_id)? {
        Some(s) => s,
        None => {
            log_error(
                &app,
                "chat_completion",
                format!("session {} not found", &session_id),
            );
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Session not found",
            ));
        }
    };

    let effective_persona_id = resolve_persona_id(&session, persona_id.as_deref());
    let persona = context.choose_persona(effective_persona_id);

    emit_debug(
        &app,
        "session_loaded",
        json!({
            "sessionId": session.id,
            "messageCount": session.messages.len(),
            "updatedAt": session.updated_at,
        }),
    );

    if session.character_id != character.id {
        session.character_id = character.id.clone();
    }

    let dynamic_memory_enabled = is_dynamic_memory_active(settings, &character);
    let dynamic_window = dynamic_window_size(settings);
    if dynamic_memory_enabled {
        let _ = prompts::ensure_dynamic_memory_templates(&app);
    }

    let (model, provider_cred) = context.select_model(&character)?;

    log_info(
        &app,
        "chat_completion",
        format!(
            "selected provider={} model={} credential={}",
            provider_cred.provider_id.as_str(),
            model.name.as_str(),
            provider_cred.id.as_str()
        ),
    );

    emit_debug(
        &app,
        "model_selected",
        json!({
            "providerId": provider_cred.provider_id,
            "model": model.name,
            "credentialId": provider_cred.id,
        }),
    );

    let now = now_millis()?;

    let user_msg_id = uuid::Uuid::new_v4().to_string();

    let persisted_attachments = persist_attachments(
        &app,
        &character_id,
        &session_id,
        &user_msg_id,
        "user",
        attachments,
    )?;

    let user_msg = StoredMessage {
        id: user_msg_id,
        role: "user".into(),
        content: user_message.clone(),
        created_at: now,
        usage: None,
        variants: Vec::new(),
        selected_variant_id: None,
        memory_refs: Vec::new(),
        used_lorebook_entries: Vec::new(),
        is_pinned: false,
        attachments: persisted_attachments,
        reasoning: None,
        model_id: None,
        fallback_from_model_id: None,
    };
    session.messages.push(user_msg.clone());
    session.updated_at = now;
    save_session(&app, &session)?;

    emit_debug(
        &app,
        "session_saved",
        json!({
            "stage": "after_user_message",
            "sessionId": session.id,
            "messageCount": session.messages.len(),
            "updatedAt": session.updated_at,
        }),
    );

    let prompt_entries = if swap_places {
        let (prompt_character, prompt_persona) = swapped_prompt_entities(&character, persona);
        append_image_directive_instructions(
            context.build_system_prompt(
                &prompt_character,
                model,
                prompt_persona.as_ref(),
                &session,
            ),
            settings,
        )
    } else {
        append_image_directive_instructions(
            context.build_system_prompt(&character, model, persona, &session),
            settings,
        )
    };
    let used_lorebook_entries = super::prompt_engine::resolve_used_lorebook_entries(
        &app,
        &character.id,
        &session,
        &prompt_entries,
    );
    let (relative_entries, in_chat_entries) = partition_prompt_entries(prompt_entries);

    // Determine message window: use conversation_window for dynamic memory (limited context),
    // or recent_messages for manual memory (includes all recent non-scene messages)
    // For dynamic memory with pinned messages: pinned messages are always included but don't count in the limit
    let (pinned_msgs, recent_msgs) = if dynamic_memory_enabled {
        let (pinned, unpinned) = conversation_window_with_pinned(&session.messages, dynamic_window);
        (pinned, unpinned)
    } else {
        (
            Vec::new(),
            recent_messages(&session, manual_window_size(settings)),
        )
    };

    // Retrieve top-k relevant memories for this turn.
    // - Dynamic memory: use semantic search over memory embeddings
    // - Manual memory: memories are injected via system prompt (see below)
    let relevant_memories = if dynamic_memory_enabled && !session.memory_embeddings.is_empty() {
        let fixed = ensure_pinned_hot(&mut session.memory_embeddings);
        if fixed > 0 {
            log_info(
                &app,
                "dynamic_memory",
                format!("Restored {} pinned memories to hot", fixed),
            );
        }

        // Build search query - use enriched query (last 2 msgs) if enabled, else just user message
        let search_query = if context_enrichment_enabled(settings) {
            build_enriched_query(&session.messages)
        } else {
            user_message.clone()
        };

        crate::utils::log_info(
            &app,
            "memory_retrieval",
            format!(
                "Search query ({} chars, enriched={})",
                search_query.len(),
                context_enrichment_enabled(settings)
            ),
        );

        select_relevant_memories(
            &app,
            &session,
            &search_query,
            dynamic_retrieval_limit(settings),
            dynamic_min_similarity(settings),
            dynamic_retrieval_strategy(settings),
        )
        .await
    } else {
        Vec::new()
    };

    // Update access tracking for retrieved memories
    if !relevant_memories.is_empty() {
        let memory_ids: Vec<String> = relevant_memories.iter().map(|m| m.id.clone()).collect();
        // Promote any cold memories that were recalled via keyword search
        let now = now_millis().unwrap_or_default();
        let promoted = promote_cold_memories(&mut session.memory_embeddings, &memory_ids, now);
        let accessed = mark_memories_accessed(&mut session.memory_embeddings, &memory_ids, now);
        if promoted > 0 {
            log_info(
                &app,
                "dynamic_memory",
                format!("Promoted {} cold memories to hot", promoted),
            );
        }
        if accessed > 0 {
            log_info(
                &app,
                "dynamic_memory",
                format!("Marked {} memories as accessed", accessed),
            );
        }
    }

    let system_role = super::request_builder::system_role_for(provider_cred);
    let mut messages_for_api = Vec::new();
    for entry in &relative_entries {
        crate::chat_manager::messages::push_prompt_entry_message(
            &mut messages_for_api,
            &system_role,
            entry,
        );
    }
    if swap_places {
        let persona_title = persona
            .map(|p| p.title.clone())
            .unwrap_or_else(|| "the user persona".to_string());
        crate::chat_manager::messages::push_system_message(
            &mut messages_for_api,
            &system_role,
            Some(format!(
                "Swap places mode is active for this turn. The human is speaking as character '{}' and you must respond as persona '{}'. Keep the response in first person as '{}'.",
                character.name, persona_title, persona_title
            )),
        );
    }

    // Inject memory context when available
    // - Dynamic memory: inject semantically relevant memories as context
    // - Manual memory: session.memories are already included in system prompt
    //   (see build_system_prompt in prompt_engine.rs)
    let memory_block = if dynamic_memory_enabled {
        if relevant_memories.is_empty() {
            None
        } else {
            Some(
                relevant_memories
                    .iter()
                    .map(|m| format!("- {}", m.text))
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
        }
    } else if !session.memories.is_empty() {
        Some(
            session
                .memories
                .iter()
                .map(|m| format!("- {}", m))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    } else {
        None
    };
    if let Some(block) = memory_block {
        crate::chat_manager::messages::push_system_message(
            &mut messages_for_api,
            &system_role,
            Some(format!("Relevant memories:\n{}", block)),
        );
    }

    let char_name = if swap_places {
        persona.map(|p| p.title.as_str()).unwrap_or("User")
    } else {
        character.name.as_str()
    };
    let persona_name = if swap_places {
        character.name.as_str()
    } else {
        persona.map(|p| p.title.as_str()).unwrap_or("")
    };
    let allow_image_input = model
        .input_scopes
        .iter()
        .any(|scope| scope.eq_ignore_ascii_case("image"));

    let mut chat_messages = Vec::new();

    // Include pinned messages first (if dynamic memory is enabled)
    // Pinned messages are always included but don't count against the sliding window limit
    for msg in &pinned_msgs {
        let msg_with_data = load_attachment_data(&app, msg);
        let msg_with_data = maybe_swap_message_for_api(&msg_with_data, swap_places);
        crate::chat_manager::messages::push_user_or_assistant_message_with_context(
            &mut chat_messages,
            &msg_with_data,
            char_name,
            persona_name,
            allow_image_input,
        );
    }

    for msg in &recent_msgs {
        let msg_with_data = load_attachment_data(&app, msg);
        let msg_with_data = maybe_swap_message_for_api(&msg_with_data, swap_places);
        crate::chat_manager::messages::push_user_or_assistant_message_with_context(
            &mut chat_messages,
            &msg_with_data,
            char_name,
            persona_name,
            allow_image_input,
        );
    }

    insert_in_chat_prompt_entries(&mut chat_messages, &system_role, &in_chat_entries);
    messages_for_api.extend(chat_messages);

    crate::chat_manager::messages::sanitize_placeholders_in_api_messages(
        &mut messages_for_api,
        char_name,
        persona_name,
    );

    let should_stream = stream.unwrap_or(true);
    let request_id = if should_stream {
        request_id.or_else(|| Some(Uuid::new_v4().to_string()))
    } else {
        None
    };

    let explicit_fallback_candidate = character
        .fallback_model_id
        .as_ref()
        .filter(|fallback_id| *fallback_id != &model.id)
        .and_then(|fallback_id| find_model_and_credential(settings, fallback_id));

    let app_default_fallback_candidate = settings
        .default_model_id
        .as_ref()
        .filter(|default_id| *default_id != &model.id)
        .and_then(|default_id| find_model_and_credential(settings, default_id));

    let mut attempts: Vec<(&Model, &ProviderCredential, bool)> =
        vec![(model, provider_cred, false)];
    if let Some((fallback_model, fallback_cred)) = explicit_fallback_candidate {
        attempts.push((fallback_model, fallback_cred, true));
    } else if character
        .fallback_model_id
        .as_ref()
        .is_some_and(|id| id != &model.id)
    {
        log_warn(
            &app,
            "chat_completion",
            format!(
                "configured character fallback model id {} could not be resolved",
                character.fallback_model_id.as_deref().unwrap_or("")
            ),
        );
        if let Some((fallback_model, fallback_cred)) = app_default_fallback_candidate {
            log_info(
                &app,
                "chat_completion",
                format!(
                    "using app default model {} as fallback candidate",
                    fallback_model.name
                ),
            );
            attempts.push((fallback_model, fallback_cred, true));
        }
    }

    let mut selected_model = model;
    let mut selected_provider_cred = provider_cred;
    let mut selected_api_key = String::new();
    let mut fallback_from_model_id: Option<String> = None;
    let mut successful_response = None;
    let mut last_error = "request failed".to_string();
    let mut fallback_toast_shown = false;

    for (idx, (attempt_model, attempt_provider_cred, is_fallback_attempt)) in
        attempts.iter().enumerate()
    {
        let has_next_attempt = idx + 1 < attempts.len();

        let attempt_api_key = match resolve_api_key(&app, attempt_provider_cred, "chat_completion")
        {
            Ok(key) => key,
            Err(err) => {
                log_error(
                    &app,
                    "chat_completion",
                    format!(
                        "failed to resolve API key for model={} provider={}: {}",
                        attempt_model.name, attempt_provider_cred.provider_id, err
                    ),
                );
                last_error = err;
                if has_next_attempt {
                    emit_fallback_retry_toast(&app, &mut fallback_toast_shown);
                    continue;
                }
                return Err(last_error);
            }
        };

        let temperature = resolve_temperature(&session, attempt_model, &settings);
        let top_p = resolve_top_p(&session, attempt_model, &settings);
        let max_tokens = resolve_max_tokens(&session, attempt_model, &settings);
        let context_length = resolve_context_length(&session, attempt_model, &settings);
        let frequency_penalty = resolve_frequency_penalty(&session, attempt_model, &settings);
        let presence_penalty = resolve_presence_penalty(&session, attempt_model, &settings);
        let top_k = resolve_top_k(&session, attempt_model, &settings);
        let reasoning_enabled = resolve_reasoning_enabled(&session, attempt_model, &settings);
        let reasoning_effort = resolve_reasoning_effort(&session, attempt_model, &settings);
        let reasoning_budget = resolve_reasoning_budget(
            &session,
            attempt_model,
            &settings,
            reasoning_effort.as_deref(),
        );
        let extra_body_fields = if attempt_provider_cred.provider_id == "llamacpp" {
            build_llama_extra_fields(&session, attempt_model, &settings)
        } else if attempt_provider_cred.provider_id == "ollama" {
            build_ollama_extra_fields(
                &session,
                attempt_model,
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

        log_info(
            &app,
            "chat_completion",
            format!(
                "reasoning settings: enabled={} effort={:?} budget={:?} model_adv={:?}",
                reasoning_enabled,
                reasoning_effort,
                reasoning_budget,
                attempt_model
                    .advanced_model_settings
                    .as_ref()
                    .map(|a| a.reasoning_enabled)
            ),
        );

        let built = super::request_builder::build_chat_request(
            attempt_provider_cred,
            &attempt_api_key,
            &attempt_model.name,
            &messages_for_api,
            None,
            temperature,
            top_p,
            max_tokens,
            context_length,
            should_stream,
            request_id.clone(),
            frequency_penalty,
            presence_penalty,
            top_k,
            None,
            reasoning_enabled,
            reasoning_effort,
            reasoning_budget,
            extra_body_fields,
        );

        log_info(
            &app,
            "chat_completion",
            format!(
                "request prepared endpoint={} stream={} request_id={:?} model={} fallback_attempt={}",
                built.url.as_str(),
                should_stream,
                &request_id,
                attempt_model.name,
                is_fallback_attempt
            ),
        );

        log_info(
            &app,
            "chat_completion",
            format!(
                "request body: reasoning_effort={:?}, reasoning_budget={:?}, max_tokens={:?}, reasoning_enabled={}",
                built.body.get("reasoning_effort"),
                built.body.get("reasoning").and_then(|r| r.get("max_tokens")),
                built.body.get("max_completion_tokens").or(built.body.get("max_tokens")),
                reasoning_enabled
            ),
        );

        emit_debug(
            &app,
            "sending_request",
            json!({
                "providerId": attempt_provider_cred.provider_id,
                "model": attempt_model.name,
                "stream": should_stream,
                "requestId": request_id,
                "endpoint": built.url,
                "reasoning": built.body.get("reasoning"),
                "reasoning_effort": built.body.get("reasoning_effort"),
                "max_completion_tokens": built.body.get("max_completion_tokens"),
                "fallbackAttempt": is_fallback_attempt,
            }),
        );

        let api_request_payload = ApiRequest {
            url: built.url,
            method: Some("POST".into()),
            headers: Some(built.headers),
            query: None,
            body: Some(built.body),
            timeout_ms: Some(900_000),
            stream: Some(built.stream),
            request_id: built.request_id.clone(),
            provider_id: Some(attempt_provider_cred.provider_id.clone()),
        };

        let api_response = match api_request(app.clone(), api_request_payload).await {
            Ok(resp) => resp,
            Err(err) => {
                log_error(
                    &app,
                    "chat_completion",
                    format!(
                        "api_request failed model={} provider={} err={}",
                        attempt_model.name, attempt_provider_cred.provider_id, err
                    ),
                );
                last_error = err;
                if has_next_attempt {
                    emit_fallback_retry_toast(&app, &mut fallback_toast_shown);
                    continue;
                }
                return Err(last_error);
            }
        };

        emit_debug(
            &app,
            "response",
            json!({
                "status": api_response.status,
                "ok": api_response.ok,
                "model": attempt_model.name,
            }),
        );

        if !api_response.ok {
            let fallback = format!("Provider returned status {}", api_response.status);
            let err_message =
                extract_error_message(api_response.data()).unwrap_or(fallback.clone());

            let failed_usage = extract_usage(api_response.data());
            if let Some(ref usage) = failed_usage {
                log_info(
                    &app,
                    "chat_completion",
                    format!(
                        "usage from failed request: prompt={:?} completion={:?} total={:?} reasoning={:?}",
                        usage.prompt_tokens,
                        usage.completion_tokens,
                        usage.total_tokens,
                        usage.reasoning_tokens
                    ),
                );
                emit_debug(
                    &app,
                    "failed_request_usage",
                    json!({
                        "promptTokens": usage.prompt_tokens,
                        "completionTokens": usage.completion_tokens,
                        "totalTokens": usage.total_tokens,
                        "reasoningTokens": usage.reasoning_tokens,
                    }),
                );
                if !has_next_attempt {
                    record_failed_usage(
                        &app,
                        &failed_usage,
                        &session,
                        &character,
                        attempt_model,
                        attempt_provider_cred,
                        UsageOperationType::Chat,
                        &err_message,
                        "chat_completion",
                    );
                }
            }

            emit_debug(
                &app,
                "provider_error",
                json!({
                    "status": api_response.status,
                    "message": err_message,
                    "usage": failed_usage,
                    "model": attempt_model.name,
                }),
            );

            let combined_error = if err_message == fallback {
                err_message
            } else {
                format!("{} (status {})", err_message, api_response.status)
            };
            log_error(
                &app,
                "chat_completion",
                format!("provider error: {}", &combined_error),
            );
            last_error = combined_error;

            if has_next_attempt {
                emit_fallback_retry_toast(&app, &mut fallback_toast_shown);
                continue;
            }
            return Err(last_error);
        }

        selected_model = attempt_model;
        selected_provider_cred = attempt_provider_cred;
        selected_api_key = attempt_api_key;
        fallback_from_model_id = if *is_fallback_attempt {
            Some(model.id.clone())
        } else {
            None
        };
        successful_response = Some(api_response);
        break;
    }

    let api_response = match successful_response {
        Some(resp) => resp,
        None => return Err(last_error),
    };

    // Extract assistant text and any image outputs.
    // Some multimodal models stream image data URLs via SSE; we must not treat those as text.
    let images_from_sse = match api_response.data() {
        Value::String(s) if s.contains("data:") => {
            super::sse::accumulate_image_data_urls_from_sse(s)
        }
        _ => Vec::new(),
    };

    let text =
        extract_text(api_response.data(), Some(&selected_model.provider_id)).unwrap_or_default();
    let usage = extract_usage(api_response.data());
    let reasoning = extract_reasoning(api_response.data(), Some(&selected_model.provider_id));

    if text.trim().is_empty() && images_from_sse.is_empty() {
        let preview =
            serde_json::to_string(api_response.data()).unwrap_or_else(|_| "<non-json>".into());

        // Enhanced debug info for diagnosing model-specific parsing issues
        let raw_len = match api_response.data() {
            Value::String(s) => s.len(),
            _ => 0,
        };
        let has_sse_marker = match api_response.data() {
            Value::String(s) => s.contains("data:"),
            _ => false,
        };

        let has_reasoning = reasoning.as_ref().map_or(false, |r| !r.trim().is_empty());
        let reasoning_len = reasoning.as_ref().map_or(0, |r| r.len());
        let error_detail = if has_reasoning {
            "Model completed reasoning but generated no response text. This may indicate the model ran out of tokens or encountered an issue during generation."
        } else {
            "Empty response from provider"
        };

        log_error(
            &app,
            "chat_completion",
            format!(
                "empty response from provider: has_reasoning={}, reasoning_len={}, raw_len={}, has_sse_marker={}, preview_start={}",
                has_reasoning,
                reasoning_len,
                raw_len,
                has_sse_marker,
                preview.chars().take(500).collect::<String>()
            ),
        );
        emit_debug(
            &app,
            "empty_response",
            json!({
                "preview": preview,
                "hasReasoning": has_reasoning,
                "reasoningLen": reasoning_len,
                "rawLen": raw_len,
                "hasSseMarker": has_sse_marker
            }),
        );
        return Err(error_detail.to_string());
    }

    // Post-generation content filter check
    if let Some(filter) = app.try_state::<crate::content_filter::ContentFilter>() {
        if filter.is_enabled() {
            let result = filter.check_text(&text);
            if result.blocked {
                log_warn(
                    &app,
                    "chat_completion",
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

    emit_debug(
        &app,
        "assistant_reply",
        json!({
            "length": text.len(),
        }),
    );

    let assistant_created_at = now_millis()?;
    let variant = new_assistant_variant(text.clone(), usage.clone(), assistant_created_at);
    let variant_id = variant.id.clone();

    let assistant_message_id = Uuid::new_v4().to_string();

    let mut assistant_generated_attachments: Vec<ImageAttachment> = Vec::new();
    for data_url in images_from_sse {
        // Best-effort mime type inference from data URL header; fallback to PNG.
        let mime_type = data_url
            .split_once(";base64,")
            .and_then(|(prefix, _)| prefix.strip_prefix("data:"))
            .unwrap_or("image/png")
            .to_string();

        assistant_generated_attachments.push(ImageAttachment {
            id: Uuid::new_v4().to_string(),
            data: data_url,
            mime_type,
            filename: None,
            width: None,
            height: None,
            storage_path: None,
        });
    }

    let persisted_assistant_attachments = persist_attachments(
        &app,
        &character_id,
        &session_id,
        &assistant_message_id,
        "assistant",
        assistant_generated_attachments,
    )?;

    let assistant_message = StoredMessage {
        id: assistant_message_id,
        role: "assistant".into(),
        content: text.clone(),
        created_at: assistant_created_at,
        usage: usage.clone(),
        variants: vec![variant],
        selected_variant_id: Some(variant_id),
        memory_refs: if dynamic_memory_enabled {
            relevant_memories
                .iter()
                .map(|m| {
                    if let Some(score) = m.match_score {
                        format!("{}::{}", score, m.text)
                    } else {
                        m.text.clone()
                    }
                })
                .collect()
        } else {
            Vec::new()
        },
        used_lorebook_entries,
        is_pinned: false,
        attachments: persisted_assistant_attachments,
        reasoning,
        model_id: Some(selected_model.id.clone()),
        fallback_from_model_id: fallback_from_model_id.clone(),
    };

    session.messages.push(assistant_message.clone());
    session.updated_at = now_millis()?;
    save_session(&app, &session)?;

    log_info(
        &app,
        "chat_completion",
        format!(
            "assistant response saved message_id={} length={} total_messages={}",
            assistant_message.id.as_str(),
            assistant_message.content.len(),
            session.messages.len()
        ),
    );

    emit_debug(
        &app,
        "session_saved",
        json!({
            "stage": "after_assistant_message",
            "sessionId": session.id,
            "messageCount": session.messages.len(),
            "updatedAt": session.updated_at,
        }),
    );

    record_usage_if_available(
        &context,
        &usage,
        &session,
        &character,
        selected_model,
        selected_provider_cred,
        &selected_api_key,
        assistant_created_at,
        UsageOperationType::Chat,
        "chat_completion",
    )
    .await;

    if dynamic_memory_enabled {
        if let Err(err) =
            process_dynamic_memory_cycle(&app, &mut session, settings, &character).await
        {
            log_error(
                &app,
                "chat_completion",
                format!("dynamic memory cycle failed: {}", err),
            );
        }
    }

    Ok(ChatTurnResult {
        session_id: session.id,
        session_updated_at: session.updated_at,
        request_id,
        user_message: user_msg,
        assistant_message,
        usage,
    })
}

#[tauri::command]
pub async fn chat_regenerate(
    app: AppHandle,
    args: ChatRegenerateArgs,
) -> Result<RegenerateResult, String> {
    let ChatRegenerateArgs {
        session_id,
        message_id,
        swap_places,
        stream,
        request_id,
    } = args;
    let swap_places = role_swap_enabled(swap_places);

    let context = ChatContext::initialize(app.clone())?;
    let settings = &context.settings;

    log_info(
        &app,
        "chat_regenerate",
        format!(
            "start session={} message={} stream={:?} request_id={:?}",
            &session_id, &message_id, stream, request_id
        ),
    );

    let mut session = match context.load_session(&session_id)? {
        Some(s) => s,
        None => {
            log_error(
                &app,
                "chat_regenerate",
                format!("session {} not found", &session_id),
            );
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Session not found",
            ));
        }
    };

    emit_debug(
        &app,
        "regenerate_start",
        json!({
            "sessionId": session.id,
            "messageId": message_id,
            "messageCount": session.messages.len(),
        }),
    );

    if session.messages.is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "No messages available for regeneration",
        ));
    }

    let target_index = session
        .messages
        .iter()
        .position(|msg| msg.id == message_id)
        .ok_or_else(|| "Assistant message not found".to_string())?;

    if target_index + 1 != session.messages.len() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Can only regenerate the latest assistant response",
        ));
    }

    if target_index == 0 {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Assistant message has no preceding user prompt",
        ));
    }

    let preceding_index = target_index - 1;
    let preceding_message = &session.messages[preceding_index];
    if preceding_message.role != "user"
        && preceding_message.role != "assistant"
        && preceding_message.role != "scene"
    {
        return Err(
            "Expected preceding user, assistant, or scene message before assistant response".into(),
        );
    }

    if session.messages[target_index].role != "assistant"
        && session.messages[target_index].role != "scene"
    {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Selected message is not an assistant or scene response",
        ));
    }

    let character = match context.find_character(&session.character_id) {
        Ok(found) => found,
        Err(err) => {
            log_error(
                &app,
                "chat_regenerate",
                format!("character {} not found", &session.character_id),
            );
            return Err(err);
        }
    };

    let persona = context.choose_persona(resolve_persona_id(&session, None));

    let (model, provider_cred) = context.select_model(&character)?;

    log_info(
        &app,
        "chat_regenerate",
        format!(
            "selected provider={} model={} credential={}",
            provider_cred.provider_id.as_str(),
            model.name.as_str(),
            provider_cred.id.as_str()
        ),
    );

    emit_debug(
        &app,
        "regenerate_model_selected",
        json!({
            "providerId": provider_cred.provider_id,
            "model": model.name,
            "credentialId": provider_cred.id,
        }),
    );

    let dynamic_memory_enabled = is_dynamic_memory_active(settings, &character);
    let dynamic_window = dynamic_window_size(settings);

    let relevant_memories = if dynamic_memory_enabled && !session.memory_embeddings.is_empty() {
        let fixed = ensure_pinned_hot(&mut session.memory_embeddings);
        if fixed > 0 {
            log_info(
                &app,
                "dynamic_memory",
                format!("Restored {} pinned memories to hot", fixed),
            );
        }

        // Build search query - use enriched query (last 2 msgs up to target) if enabled
        let messages_up_to: Vec<StoredMessage> = session
            .messages
            .iter()
            .take(target_index + 1) // Include the message being regenerated
            .cloned()
            .collect();
        let search_query = if context_enrichment_enabled(&context.settings) {
            build_enriched_query(&messages_up_to)
        } else {
            messages_up_to
                .iter()
                .rev()
                .find(|m| m.role == "user")
                .map(|m| m.content.clone())
                .unwrap_or_default()
        };
        select_relevant_memories(
            &app,
            &session,
            &search_query,
            dynamic_retrieval_limit(&context.settings),
            dynamic_min_similarity(&context.settings),
            dynamic_retrieval_strategy(&context.settings),
        )
        .await
    } else {
        Vec::new()
    };

    // Update access tracking for retrieved memories
    if !relevant_memories.is_empty() {
        let memory_ids: Vec<String> = relevant_memories.iter().map(|m| m.id.clone()).collect();
        let now = now_millis().unwrap_or_default();
        let promoted = promote_cold_memories(&mut session.memory_embeddings, &memory_ids, now);
        let accessed = mark_memories_accessed(&mut session.memory_embeddings, &memory_ids, now);
        if promoted > 0 {
            log_info(
                &app,
                "dynamic_memory",
                format!("Promoted {} cold memories to hot", promoted),
            );
        }
        if accessed > 0 {
            log_info(
                &app,
                "dynamic_memory",
                format!("Marked {} memories as accessed", accessed),
            );
        }
    }

    let prompt_entries = if swap_places {
        let (prompt_character, prompt_persona) = swapped_prompt_entities(&character, persona);
        append_image_directive_instructions(
            context.build_system_prompt(
                &prompt_character,
                model,
                prompt_persona.as_ref(),
                &session,
            ),
            settings,
        )
    } else {
        append_image_directive_instructions(
            context.build_system_prompt(&character, model, persona, &session),
            settings,
        )
    };
    let used_lorebook_entries = super::prompt_engine::resolve_used_lorebook_entries(
        &app,
        &character.id,
        &session,
        &prompt_entries,
    );
    let (relative_entries, in_chat_entries) = partition_prompt_entries(prompt_entries);

    let system_role = super::request_builder::system_role_for(provider_cred);
    let messages_for_api = {
        let mut out = Vec::new();
        for entry in &relative_entries {
            crate::chat_manager::messages::push_prompt_entry_message(&mut out, &system_role, entry);
        }
        if swap_places {
            let persona_title = persona
                .map(|p| p.title.clone())
                .unwrap_or_else(|| "the user persona".to_string());
            crate::chat_manager::messages::push_system_message(
                &mut out,
                &system_role,
                Some(format!(
                    "Swap places mode is active for this turn. The human is speaking as character '{}' and you must respond as persona '{}'. Keep the response in first person as '{}'.",
                    character.name, persona_title, persona_title
                )),
            );
        }

        let char_name = if swap_places {
            persona.map(|p| p.title.as_str()).unwrap_or("User")
        } else {
            character.name.as_str()
        };
        let persona_name = if swap_places {
            character.name.as_str()
        } else {
            persona.map(|p| p.title.as_str()).unwrap_or("")
        };
        let allow_image_input = model
            .input_scopes
            .iter()
            .any(|scope| scope.eq_ignore_ascii_case("image"));

        let messages_before_target: Vec<StoredMessage> = session
            .messages
            .iter()
            .enumerate()
            .filter(|(idx, _)| *idx < target_index)
            .map(|(_, msg)| msg.clone())
            .collect();

        let mut chat_messages = Vec::new();
        if dynamic_memory_enabled {
            let (pinned_msgs, recent_msgs) =
                conversation_window_with_pinned(&messages_before_target, dynamic_window);

            for msg in &pinned_msgs {
                let msg_with_data = load_attachment_data(&app, msg);
                let msg_with_data = maybe_swap_message_for_api(&msg_with_data, swap_places);
                crate::chat_manager::messages::push_user_or_assistant_message_with_context(
                    &mut chat_messages,
                    &msg_with_data,
                    char_name,
                    persona_name,
                    allow_image_input,
                );
            }

            for msg in &recent_msgs {
                let msg_with_data = load_attachment_data(&app, msg);
                let msg_with_data = maybe_swap_message_for_api(&msg_with_data, swap_places);
                crate::chat_manager::messages::push_user_or_assistant_message_with_context(
                    &mut chat_messages,
                    &msg_with_data,
                    char_name,
                    persona_name,
                    allow_image_input,
                );
            }
        } else {
            let start_index = target_index.saturating_sub(manual_window_size(settings));
            for (idx, msg) in session.messages.iter().enumerate() {
                if idx < start_index {
                    continue;
                }
                if idx > target_index {
                    break;
                }
                if idx == target_index {
                    continue;
                }
                let msg_with_data = load_attachment_data(&app, msg);
                let msg_with_data = maybe_swap_message_for_api(&msg_with_data, swap_places);
                crate::chat_manager::messages::push_user_or_assistant_message_with_context(
                    &mut chat_messages,
                    &msg_with_data,
                    char_name,
                    persona_name,
                    allow_image_input,
                );
            }
        }

        insert_in_chat_prompt_entries(&mut chat_messages, &system_role, &in_chat_entries);
        out.extend(chat_messages);

        crate::chat_manager::messages::sanitize_placeholders_in_api_messages(
            &mut out,
            char_name,
            persona_name,
        );
        out
    };

    let should_stream = stream.unwrap_or(true);
    let request_id = if should_stream {
        request_id.or_else(|| Some(Uuid::new_v4().to_string()))
    } else {
        None
    };

    let attempts = build_model_attempts(
        &app,
        settings,
        &character,
        model,
        provider_cred,
        "chat_regenerate",
    );

    let mut selected_model = model;
    let mut selected_provider_cred = provider_cred;
    let mut selected_api_key = String::new();
    let mut fallback_from_model_id: Option<String> = None;
    let mut successful_response = None;
    let mut last_error = "request failed".to_string();
    let mut fallback_toast_shown = false;

    {
        let message = session
            .messages
            .get_mut(target_index)
            .ok_or_else(|| "Assistant message not accessible".to_string())?;
        ensure_assistant_variant(message);
    }

    for (idx, (attempt_model, attempt_provider_cred, is_fallback_attempt)) in
        attempts.iter().enumerate()
    {
        let has_next_attempt = idx + 1 < attempts.len();

        let attempt_api_key = match resolve_api_key(&app, attempt_provider_cred, "chat_regenerate")
        {
            Ok(key) => key,
            Err(err) => {
                last_error = err;
                if has_next_attempt {
                    emit_fallback_retry_toast(&app, &mut fallback_toast_shown);
                    continue;
                }
                return Err(last_error);
            }
        };

        let temperature = resolve_temperature(&session, attempt_model, &settings);
        let top_p = resolve_top_p(&session, attempt_model, &settings);
        let max_tokens = resolve_max_tokens(&session, attempt_model, &settings);
        let context_length = resolve_context_length(&session, attempt_model, &settings);
        let frequency_penalty = resolve_frequency_penalty(&session, attempt_model, &settings);
        let presence_penalty = resolve_presence_penalty(&session, attempt_model, &settings);
        let top_k = resolve_top_k(&session, attempt_model, &settings);
        let reasoning_enabled = resolve_reasoning_enabled(&session, attempt_model, &settings);
        let reasoning_effort = resolve_reasoning_effort(&session, attempt_model, &settings);
        let reasoning_budget = resolve_reasoning_budget(
            &session,
            attempt_model,
            &settings,
            reasoning_effort.as_deref(),
        );
        let extra_body_fields = if attempt_provider_cred.provider_id == "llamacpp" {
            build_llama_extra_fields(&session, attempt_model, &settings)
        } else if attempt_provider_cred.provider_id == "ollama" {
            build_ollama_extra_fields(
                &session,
                attempt_model,
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

        let built = super::request_builder::build_chat_request(
            attempt_provider_cred,
            &attempt_api_key,
            &attempt_model.name,
            &messages_for_api,
            None,
            temperature,
            top_p,
            max_tokens,
            context_length,
            should_stream,
            request_id.clone(),
            frequency_penalty,
            presence_penalty,
            top_k,
            None,
            reasoning_enabled,
            reasoning_effort,
            reasoning_budget,
            extra_body_fields,
        );

        emit_debug(
            &app,
            "regenerate_request",
            json!({
                "sessionId": session.id,
                "messageId": message_id,
                "requestId": request_id,
                "endpoint": built.url,
                "model": attempt_model.name,
                "fallbackAttempt": is_fallback_attempt,
            }),
        );

        let api_request_payload = ApiRequest {
            url: built.url,
            method: Some("POST".into()),
            headers: Some(built.headers),
            query: None,
            body: Some(built.body),
            timeout_ms: Some(900_000),
            stream: Some(built.stream),
            request_id: built.request_id.clone(),
            provider_id: Some(attempt_provider_cred.provider_id.clone()),
        };

        let api_response = match api_request(app.clone(), api_request_payload).await {
            Ok(resp) => resp,
            Err(err) => {
                last_error = err;
                if has_next_attempt {
                    emit_fallback_retry_toast(&app, &mut fallback_toast_shown);
                    continue;
                }
                return Err(last_error);
            }
        };

        emit_debug(
            &app,
            "regenerate_response",
            json!({
                "status": api_response.status,
                "ok": api_response.ok,
                "model": attempt_model.name,
            }),
        );

        if !api_response.ok {
            let fallback = format!("Provider returned status {}", api_response.status);
            let err_message =
                extract_error_message(api_response.data()).unwrap_or(fallback.clone());
            let failed_usage = extract_usage(api_response.data());
            emit_debug(
                &app,
                "regenerate_provider_error",
                json!({
                    "status": api_response.status,
                    "message": err_message,
                    "usage": failed_usage,
                    "model": attempt_model.name,
                }),
            );
            if !has_next_attempt {
                record_failed_usage(
                    &app,
                    &failed_usage,
                    &session,
                    &character,
                    attempt_model,
                    attempt_provider_cred,
                    UsageOperationType::Regenerate,
                    &err_message,
                    "chat_regenerate",
                );
            }
            last_error = if err_message == fallback {
                err_message
            } else {
                format!("{} (status {})", err_message, api_response.status)
            };
            if has_next_attempt {
                emit_fallback_retry_toast(&app, &mut fallback_toast_shown);
                continue;
            }
            return Err(last_error);
        }

        selected_model = attempt_model;
        selected_provider_cred = attempt_provider_cred;
        selected_api_key = attempt_api_key;
        fallback_from_model_id = if *is_fallback_attempt {
            Some(model.id.clone())
        } else {
            None
        };
        successful_response = Some(api_response);
        break;
    }

    let api_response = match successful_response {
        Some(resp) => resp,
        None => return Err(last_error),
    };

    let images_from_sse = match api_response.data() {
        Value::String(s) if s.contains("data:") => {
            super::sse::accumulate_image_data_urls_from_sse(s)
        }
        _ => Vec::new(),
    };

    let text = extract_text(
        api_response.data(),
        Some(&selected_provider_cred.provider_id),
    )
    .unwrap_or_default();
    let usage = extract_usage(api_response.data());
    let reasoning = extract_reasoning(
        api_response.data(),
        Some(&selected_provider_cred.provider_id),
    );

    if text.trim().is_empty() && images_from_sse.is_empty() {
        let preview =
            serde_json::to_string(api_response.data()).unwrap_or_else(|_| "<non-json>".into());

        let has_reasoning = reasoning.as_ref().map_or(false, |r| !r.trim().is_empty());
        let error_detail = if has_reasoning {
            "Model completed reasoning but generated no response text. This may indicate the model ran out of tokens or encountered an issue during generation."
        } else {
            "Empty response from provider"
        };

        emit_debug(
            &app,
            "regenerate_empty_response",
            json!({ "preview": preview, "hasReasoning": has_reasoning }),
        );
        return Err(error_detail.to_string());
    }

    // Post-generation content filter check
    if let Some(filter) = app.try_state::<crate::content_filter::ContentFilter>() {
        if filter.is_enabled() {
            let result = filter.check_text(&text);
            if result.blocked {
                log_warn(
                    &app,
                    "chat_regenerate",
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

    let created_at = now_millis()?;
    let new_variant = new_assistant_variant(text.clone(), usage.clone(), created_at);

    let mut assistant_generated_attachments: Vec<ImageAttachment> = Vec::new();
    for data_url in images_from_sse {
        let mime_type = data_url
            .split_once(";base64,")
            .and_then(|(prefix, _)| prefix.strip_prefix("data:"))
            .unwrap_or("image/png")
            .to_string();
        assistant_generated_attachments.push(ImageAttachment {
            id: Uuid::new_v4().to_string(),
            data: data_url,
            mime_type,
            filename: None,
            width: None,
            height: None,
            storage_path: None,
        });
    }

    let persisted_assistant_attachments = persist_attachments(
        &app,
        &character.id,
        &session.id,
        &message_id,
        "assistant",
        assistant_generated_attachments,
    )?;

    let assistant_clone = {
        let assistant_message = session
            .messages
            .get_mut(target_index)
            .ok_or_else(|| "Assistant message not accessible".to_string())?;

        assistant_message.content = text.clone();
        assistant_message.usage = usage.clone();
        assistant_message.reasoning = reasoning.clone();
        assistant_message.model_id = Some(selected_model.id.clone());
        assistant_message.fallback_from_model_id = fallback_from_model_id.clone();
        push_assistant_variant(assistant_message, new_variant);

        if dynamic_memory_enabled {
            assistant_message.memory_refs = relevant_memories
                .iter()
                .map(|m| {
                    if let Some(score) = m.match_score {
                        format!("{}::{}", score, m.text)
                    } else {
                        m.text.clone()
                    }
                })
                .collect();
        }
        assistant_message.used_lorebook_entries = used_lorebook_entries.clone();
        if !persisted_assistant_attachments.is_empty() {
            assistant_message.attachments = persisted_assistant_attachments;
        }
        assistant_message.clone()
    };

    session.updated_at = now_millis()?;
    save_session(&app, &session)?;

    emit_debug(
        &app,
        "regenerate_saved",
        json!({
            "sessionId": session.id,
            "messageId": message_id,
            "variantId": assistant_clone
                .selected_variant_id
                .clone()
                .unwrap_or_default(),
            "variantCount": assistant_clone.variants.len(),
        }),
    );

    log_info(
        &app,
        "chat_regenerate",
        format!(
            "completed messageId={} variants={} request_id={:?}",
            assistant_clone.id.as_str(),
            assistant_clone.variants.len(),
            &request_id
        ),
    );

    record_usage_if_available(
        &context,
        &usage,
        &session,
        &character,
        selected_model,
        selected_provider_cred,
        &selected_api_key,
        created_at,
        UsageOperationType::Regenerate,
        "chat_regenerate",
    )
    .await;

    Ok(RegenerateResult {
        session_id: session.id,
        session_updated_at: session.updated_at,
        request_id,
        assistant_message: assistant_clone,
    })
}

#[tauri::command]
pub async fn chat_continue(
    app: AppHandle,
    args: ChatContinueArgs,
) -> Result<ContinueResult, String> {
    let ChatContinueArgs {
        session_id,
        character_id,
        persona_id,
        swap_places,
        stream,
        request_id,
    } = args;
    let swap_places = role_swap_enabled(swap_places);

    let context = ChatContext::initialize(app.clone())?;
    let settings = &context.settings;

    log_info(
        &app,
        "chat_continue",
        format!(
            "start session={} character={} stream={:?} request_id={:?}",
            &session_id, &character_id, stream, request_id
        ),
    );

    let mut session = match context.load_session(&session_id)? {
        Some(s) => s,
        None => {
            log_error(
                &app,
                "chat_continue",
                format!("session {} not found", &session_id),
            );
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Session not found",
            ));
        }
    };

    emit_debug(
        &app,
        "continue_start",
        json!({
            "sessionId": session.id,
            "characterId": character_id,
            "messageCount": session.messages.len(),
        }),
    );

    let stored_total_messages = session.messages.len();
    let stored_convo_messages = conversation_count(&session.messages);
    log_info(
        &app,
        "chat_continue",
        format!(
            "stored message counts before continue total={} convo={} (no [CONTINUE] prompt persisted)",
            stored_total_messages, stored_convo_messages
        ),
    );

    let character = match context.find_character(&character_id) {
        Ok(found) => found,
        Err(err) => {
            log_error(
                &app,
                "chat_continue",
                format!("character {} not found", &character_id),
            );
            return Err(err);
        }
    };

    let effective_persona_id = resolve_persona_id(&session, persona_id.as_deref());
    let persona = context.choose_persona(effective_persona_id);

    let (model, provider_cred) = context.select_model(&character)?;

    log_info(
        &app,
        "chat_continue",
        format!(
            "selected provider={} model={} credential={}",
            provider_cred.provider_id.as_str(),
            model.name.as_str(),
            provider_cred.id.as_str()
        ),
    );

    emit_debug(
        &app,
        "continue_model_selected",
        json!({
            "providerId": provider_cred.provider_id,
            "model": model.name,
            "credentialId": provider_cred.id,
        }),
    );

    let dynamic_memory_enabled = is_dynamic_memory_active(settings, &character);
    let dynamic_window = dynamic_window_size(settings);

    let relevant_memories = if dynamic_memory_enabled && !session.memory_embeddings.is_empty() {
        let fixed = ensure_pinned_hot(&mut session.memory_embeddings);
        if fixed > 0 {
            log_info(
                &app,
                "dynamic_memory",
                format!("Restored {} pinned memories to hot", fixed),
            );
        }

        // Build search query - use enriched query (last 2 msgs) if enabled
        let search_query = if context_enrichment_enabled(&context.settings) {
            build_enriched_query(&session.messages)
        } else {
            session
                .messages
                .iter()
                .rev()
                .find(|m| m.role == "user")
                .map(|m| m.content.clone())
                .unwrap_or_default()
        };
        select_relevant_memories(
            &app,
            &session,
            &search_query,
            dynamic_retrieval_limit(&context.settings),
            dynamic_min_similarity(&context.settings),
            dynamic_retrieval_strategy(&context.settings),
        )
        .await
    } else {
        Vec::new()
    };

    // Update access tracking for retrieved memories
    if !relevant_memories.is_empty() {
        let memory_ids: Vec<String> = relevant_memories.iter().map(|m| m.id.clone()).collect();
        let now = now_millis().unwrap_or_default();
        let promoted = promote_cold_memories(&mut session.memory_embeddings, &memory_ids, now);
        let accessed = mark_memories_accessed(&mut session.memory_embeddings, &memory_ids, now);
        if promoted > 0 {
            log_info(
                &app,
                "dynamic_memory",
                format!("Promoted {} cold memories to hot", promoted),
            );
        }
        if accessed > 0 {
            log_info(
                &app,
                "dynamic_memory",
                format!("Marked {} memories as accessed", accessed),
            );
        }
    }

    let prompt_entries = if swap_places {
        let (prompt_character, prompt_persona) = swapped_prompt_entities(&character, persona);
        append_image_directive_instructions(
            context.build_system_prompt(
                &prompt_character,
                model,
                prompt_persona.as_ref(),
                &session,
            ),
            settings,
        )
    } else {
        append_image_directive_instructions(
            context.build_system_prompt(&character, model, persona, &session),
            settings,
        )
    };
    let used_lorebook_entries = super::prompt_engine::resolve_used_lorebook_entries(
        &app,
        &character.id,
        &session,
        &prompt_entries,
    );
    let (relative_entries, in_chat_entries) = partition_prompt_entries(prompt_entries);

    let (pinned_msgs, recent_msgs) = if dynamic_memory_enabled {
        let (pinned, unpinned) = conversation_window_with_pinned(&session.messages, dynamic_window);
        (pinned, unpinned)
    } else {
        (
            Vec::new(),
            recent_messages(&session, manual_window_size(settings)),
        )
    };

    let system_role = super::request_builder::system_role_for(provider_cred);
    let mut messages_for_api = Vec::new();
    for entry in &relative_entries {
        crate::chat_manager::messages::push_prompt_entry_message(
            &mut messages_for_api,
            &system_role,
            entry,
        );
    }
    if swap_places {
        let persona_title = persona
            .map(|p| p.title.clone())
            .unwrap_or_else(|| "the user persona".to_string());
        crate::chat_manager::messages::push_system_message(
            &mut messages_for_api,
            &system_role,
            Some(format!(
                "Swap places mode is active for this turn. The human is speaking as character '{}' and you must respond as persona '{}'. Keep the response in first person as '{}'.",
                character.name, persona_title, persona_title
            )),
        );
    }

    let char_name = if swap_places {
        persona.map(|p| p.title.as_str()).unwrap_or("User")
    } else {
        character.name.as_str()
    };
    let persona_name = if swap_places {
        character.name.as_str()
    } else {
        persona.map(|p| p.title.as_str()).unwrap_or("")
    };
    let allow_image_input = model
        .input_scopes
        .iter()
        .any(|scope| scope.eq_ignore_ascii_case("image"));

    let mut chat_messages = Vec::new();
    for msg in &pinned_msgs {
        let msg_with_data = load_attachment_data(&app, msg);
        let msg_with_data = maybe_swap_message_for_api(&msg_with_data, swap_places);
        crate::chat_manager::messages::push_user_or_assistant_message_with_context(
            &mut chat_messages,
            &msg_with_data,
            char_name,
            persona_name,
            allow_image_input,
        );
    }

    for msg in &recent_msgs {
        let msg_with_data = load_attachment_data(&app, msg);
        let msg_with_data = maybe_swap_message_for_api(&msg_with_data, swap_places);
        crate::chat_manager::messages::push_user_or_assistant_message_with_context(
            &mut chat_messages,
            &msg_with_data,
            char_name,
            persona_name,
            allow_image_input,
        );
    }
    insert_in_chat_prompt_entries(&mut chat_messages, &system_role, &in_chat_entries);
    messages_for_api.extend(chat_messages);
    crate::chat_manager::messages::sanitize_placeholders_in_api_messages(
        &mut messages_for_api,
        char_name,
        persona_name,
    );

    messages_for_api.push(json!({
        "role": "user",
        "content": "[CONTINUE] You were in the middle of a response. Continue writing from exactly where you left off. Do NOT restart, regenerate, or rewrite what you already said. Simply pick up the narrative thread and continue the scene forward with new content."
    }));

    let should_stream = stream.unwrap_or(true);
    let request_id = if should_stream {
        request_id.or_else(|| Some(Uuid::new_v4().to_string()))
    } else {
        None
    };
    let attempts = build_model_attempts(
        &app,
        settings,
        &character,
        model,
        provider_cred,
        "chat_continue",
    );

    let mut selected_model = model;
    let mut selected_provider_cred = provider_cred;
    let mut selected_api_key = String::new();
    let mut fallback_from_model_id: Option<String> = None;
    let mut successful_response = None;
    let mut last_error = "request failed".to_string();
    let mut fallback_toast_shown = false;

    for (idx, (attempt_model, attempt_provider_cred, is_fallback_attempt)) in
        attempts.iter().enumerate()
    {
        let has_next_attempt = idx + 1 < attempts.len();

        let attempt_api_key = match resolve_api_key(&app, attempt_provider_cred, "chat_continue") {
            Ok(key) => key,
            Err(err) => {
                last_error = err;
                if has_next_attempt {
                    emit_fallback_retry_toast(&app, &mut fallback_toast_shown);
                    continue;
                }
                return Err(last_error);
            }
        };

        let temperature = resolve_temperature(&session, attempt_model, &settings);
        let top_p = resolve_top_p(&session, attempt_model, &settings);
        let max_tokens = resolve_max_tokens(&session, attempt_model, &settings);
        let context_length = resolve_context_length(&session, attempt_model, &settings);
        let frequency_penalty = resolve_frequency_penalty(&session, attempt_model, &settings);
        let presence_penalty = resolve_presence_penalty(&session, attempt_model, &settings);
        let top_k = resolve_top_k(&session, attempt_model, &settings);
        let reasoning_enabled = resolve_reasoning_enabled(&session, attempt_model, &settings);
        let reasoning_effort = resolve_reasoning_effort(&session, attempt_model, &settings);
        let reasoning_budget = resolve_reasoning_budget(
            &session,
            attempt_model,
            &settings,
            reasoning_effort.as_deref(),
        );
        let extra_body_fields = if attempt_provider_cred.provider_id == "llamacpp" {
            build_llama_extra_fields(&session, attempt_model, &settings)
        } else if attempt_provider_cred.provider_id == "ollama" {
            build_ollama_extra_fields(
                &session,
                attempt_model,
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

        let built = super::request_builder::build_chat_request(
            attempt_provider_cred,
            &attempt_api_key,
            &attempt_model.name,
            &messages_for_api,
            None,
            temperature,
            top_p,
            max_tokens,
            context_length,
            should_stream,
            request_id.clone(),
            frequency_penalty,
            presence_penalty,
            top_k,
            None,
            reasoning_enabled,
            reasoning_effort,
            reasoning_budget,
            extra_body_fields,
        );

        emit_debug(
            &app,
            "continue_request",
            json!({
                "providerId": attempt_provider_cred.provider_id,
                "model": attempt_model.name,
                "stream": should_stream,
                "requestId": request_id,
                "endpoint": built.url,
                "fallbackAttempt": is_fallback_attempt,
            }),
        );

        let api_request_payload = ApiRequest {
            url: built.url,
            method: Some("POST".into()),
            headers: Some(built.headers),
            query: None,
            body: Some(built.body),
            timeout_ms: Some(900_000),
            stream: Some(built.stream),
            request_id: built.request_id.clone(),
            provider_id: Some(attempt_provider_cred.provider_id.clone()),
        };

        let api_response = match api_request(app.clone(), api_request_payload).await {
            Ok(resp) => resp,
            Err(err) => {
                last_error = err;
                if has_next_attempt {
                    emit_fallback_retry_toast(&app, &mut fallback_toast_shown);
                    continue;
                }
                return Err(last_error);
            }
        };

        emit_debug(
            &app,
            "continue_response",
            json!({
                "status": api_response.status,
                "ok": api_response.ok,
                "model": attempt_model.name,
            }),
        );

        if !api_response.ok {
            let fallback = format!("Provider returned status {}", api_response.status);
            let err_message =
                extract_error_message(api_response.data()).unwrap_or(fallback.clone());
            let failed_usage = extract_usage(api_response.data());
            emit_debug(
                &app,
                "continue_provider_error",
                json!({
                    "status": api_response.status,
                    "message": err_message,
                    "usage": failed_usage,
                    "model": attempt_model.name,
                }),
            );
            if !has_next_attempt {
                record_failed_usage(
                    &app,
                    &failed_usage,
                    &session,
                    &character,
                    attempt_model,
                    attempt_provider_cred,
                    UsageOperationType::Continue,
                    &err_message,
                    "chat_continue",
                );
            }
            last_error = if err_message == fallback {
                err_message
            } else {
                format!("{} (status {})", err_message, api_response.status)
            };
            if has_next_attempt {
                emit_fallback_retry_toast(&app, &mut fallback_toast_shown);
                continue;
            }
            return Err(last_error);
        }

        selected_model = attempt_model;
        selected_provider_cred = attempt_provider_cred;
        selected_api_key = attempt_api_key;
        fallback_from_model_id = if *is_fallback_attempt {
            Some(model.id.clone())
        } else {
            None
        };
        successful_response = Some(api_response);
        break;
    }

    let api_response = match successful_response {
        Some(resp) => resp,
        None => return Err(last_error),
    };

    let images_from_sse = match api_response.data() {
        Value::String(s) if s.contains("data:") => {
            super::sse::accumulate_image_data_urls_from_sse(s)
        }
        _ => Vec::new(),
    };

    let text = extract_text(
        api_response.data(),
        Some(&selected_provider_cred.provider_id),
    )
    .unwrap_or_default();
    let usage = extract_usage(api_response.data());
    let reasoning = extract_reasoning(
        api_response.data(),
        Some(&selected_provider_cred.provider_id),
    );

    if text.trim().is_empty() && images_from_sse.is_empty() {
        let preview =
            serde_json::to_string(api_response.data()).unwrap_or_else(|_| "<non-json>".into());

        let has_reasoning = reasoning.as_ref().map_or(false, |r| !r.trim().is_empty());
        let error_detail = if has_reasoning {
            "Model completed reasoning but generated no response text. This may indicate the model ran out of tokens or encountered an issue during generation."
        } else {
            "Empty response from provider"
        };

        log_warn(
            &app,
            "chat_continue",
            format!(
                "empty response from provider, has_reasoning={}, preview={}",
                has_reasoning, &preview
            ),
        );
        emit_debug(
            &app,
            "continue_empty_response",
            json!({ "preview": preview, "hasReasoning": has_reasoning }),
        );
        return Err(error_detail.to_string());
    }

    // Post-generation content filter check
    if let Some(filter) = app.try_state::<crate::content_filter::ContentFilter>() {
        if filter.is_enabled() {
            let result = filter.check_text(&text);
            if result.blocked {
                log_warn(
                    &app,
                    "chat_continue",
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

    emit_debug(
        &app,
        "continue_assistant_reply",
        json!({
            "length": text.len(),
        }),
    );

    let assistant_created_at = now_millis()?;
    let variant = new_assistant_variant(text.clone(), usage.clone(), assistant_created_at);
    let variant_id = variant.id.clone();

    let assistant_message_id = Uuid::new_v4().to_string();

    let mut assistant_generated_attachments: Vec<ImageAttachment> = Vec::new();
    for data_url in images_from_sse {
        let mime_type = data_url
            .split_once(";base64,")
            .and_then(|(prefix, _)| prefix.strip_prefix("data:"))
            .unwrap_or("image/png")
            .to_string();
        assistant_generated_attachments.push(ImageAttachment {
            id: Uuid::new_v4().to_string(),
            data: data_url,
            mime_type,
            filename: None,
            width: None,
            height: None,
            storage_path: None,
        });
    }

    let persisted_assistant_attachments = persist_attachments(
        &app,
        &character_id,
        &session_id,
        &assistant_message_id,
        "assistant",
        assistant_generated_attachments,
    )?;

    let assistant_message = StoredMessage {
        id: assistant_message_id,
        role: "assistant".into(),
        content: text.clone(),
        created_at: assistant_created_at,
        usage: usage.clone(),
        variants: vec![variant],
        selected_variant_id: Some(variant_id),
        memory_refs: if dynamic_memory_enabled {
            relevant_memories
                .iter()
                .map(|m| {
                    if let Some(score) = m.match_score {
                        format!("{}::{}", score, m.text)
                    } else {
                        m.text.clone()
                    }
                })
                .collect()
        } else {
            Vec::new()
        },
        used_lorebook_entries,
        is_pinned: false,
        attachments: persisted_assistant_attachments,
        reasoning,
        model_id: Some(selected_model.id.clone()),
        fallback_from_model_id: fallback_from_model_id.clone(),
    };

    session.messages.push(assistant_message.clone());
    session.updated_at = now_millis()?;
    save_session(&app, &session)?;

    emit_debug(
        &app,
        "continue_session_saved",
        json!({
            "sessionId": session.id,
            "messageCount": session.messages.len(),
            "updatedAt": session.updated_at,
        }),
    );

    log_info(
        &app,
        "chat_continue",
        format!(
            "assistant continuation saved message_id={} total_messages={} convo_messages={} request_id={:?}",
            assistant_message.id.as_str(),
            session.messages.len(),
            conversation_count(&session.messages),
            &request_id
        ),
    );

    record_usage_if_available(
        &context,
        &usage,
        &session,
        &character,
        selected_model,
        selected_provider_cred,
        &selected_api_key,
        assistant_created_at,
        UsageOperationType::Continue,
        "chat_continue",
    )
    .await;

    if dynamic_memory_enabled {
        if let Err(err) =
            process_dynamic_memory_cycle(&app, &mut session, settings, &character).await
        {
            log_error(
                &app,
                "chat_continue",
                format!("dynamic memory cycle failed: {}", err),
            );
        }
    }

    Ok(ContinueResult {
        session_id: session.id,
        session_updated_at: session.updated_at,
        request_id,
        assistant_message,
    })
}

#[tauri::command]
pub fn get_default_character_rules(pure_mode_level: String) -> Vec<String> {
    default_character_rules(&pure_mode_level)
}

#[tauri::command]
pub fn get_default_system_prompt_template() -> String {
    get_base_prompt(PromptType::SystemPrompt)
}

// ==================== Prompt Template Commands ====================

#[tauri::command]
pub fn list_prompt_templates(app: AppHandle) -> Result<Vec<SystemPromptTemplate>, String> {
    prompts::load_templates(&app)
}

#[tauri::command]
pub fn create_prompt_template(
    app: AppHandle,
    name: String,
    scope: PromptScope,
    target_ids: Vec<String>,
    content: String,
    entries: Option<Vec<SystemPromptEntry>>,
    condense_prompt_entries: Option<bool>,
) -> Result<SystemPromptTemplate, String> {
    prompts::create_template(
        &app,
        name,
        scope,
        target_ids,
        content,
        entries,
        condense_prompt_entries,
    )
}

#[tauri::command]
pub fn update_prompt_template(
    app: AppHandle,
    id: String,
    name: Option<String>,
    scope: Option<PromptScope>,
    target_ids: Option<Vec<String>>,
    content: Option<String>,
    entries: Option<Vec<SystemPromptEntry>>,
    condense_prompt_entries: Option<bool>,
) -> Result<SystemPromptTemplate, String> {
    prompts::update_template(
        &app,
        id,
        name,
        scope,
        target_ids,
        content,
        entries,
        condense_prompt_entries,
    )
}

#[tauri::command]
pub fn delete_prompt_template(app: AppHandle, id: String) -> Result<(), String> {
    prompts::delete_template(&app, id)
}

#[tauri::command]
pub fn get_prompt_template(
    app: AppHandle,
    id: String,
) -> Result<Option<SystemPromptTemplate>, String> {
    prompts::get_template(&app, &id)
}

#[tauri::command]
pub fn get_app_default_template_id() -> String {
    prompts::APP_DEFAULT_TEMPLATE_ID.to_string()
}

#[tauri::command]
pub fn is_app_default_template(id: String) -> bool {
    prompts::is_app_default_template(&id)
}

#[tauri::command]
pub fn reset_app_default_template(app: AppHandle) -> Result<SystemPromptTemplate, String> {
    prompts::reset_app_default_template(&app)
}

#[tauri::command]
pub fn reset_dynamic_summary_template(app: AppHandle) -> Result<SystemPromptTemplate, String> {
    prompts::reset_dynamic_summary_template(&app)
}

#[tauri::command]
pub fn reset_dynamic_memory_template(app: AppHandle) -> Result<SystemPromptTemplate, String> {
    prompts::reset_dynamic_memory_template(&app)
}

#[tauri::command]
pub fn reset_help_me_reply_template(app: AppHandle) -> Result<SystemPromptTemplate, String> {
    prompts::reset_help_me_reply_template(&app)
}

#[tauri::command]
pub fn reset_help_me_reply_conversational_template(
    app: AppHandle,
) -> Result<SystemPromptTemplate, String> {
    prompts::reset_help_me_reply_conversational_template(&app)
}

#[tauri::command]
pub fn get_required_template_variables(template_id: String) -> Vec<String> {
    prompts::get_required_variables(&template_id)
}

#[tauri::command]
pub fn validate_template_variables(
    template_id: String,
    content: String,
    entries: Option<Vec<SystemPromptEntry>>,
) -> Result<(), String> {
    let validation_text = if let Some(entries) = entries {
        if entries.is_empty() {
            content
        } else {
            entries
                .iter()
                .map(|entry| entry.content.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        }
    } else {
        content
    };
    prompts::validate_required_variables(&template_id, &validation_text)
        .map_err(|missing| format!("Missing required variables: {}", missing.join(", ")))
}

// Deprecated: get_applicable_prompts_for_* commands removed in favor of global list on client

// ==================== Prompt Preview Command ====================

#[tauri::command]
pub fn render_prompt_preview(
    app: AppHandle,
    content: String,
    character_id: String,
    session_id: Option<String>,
    persona_id: Option<String>,
) -> Result<String, String> {
    let context = super::service::ChatContext::initialize(app.clone())?;
    let settings = &context.settings;

    let character = context.find_character(&character_id)?;

    // Load session if provided, otherwise synthesize a minimal one
    let session: Session = if let Some(sid) = session_id.as_ref() {
        context
            .load_session(sid)
            .and_then(|opt| opt.ok_or_else(|| "Session not found".to_string()))?
    } else {
        // Minimal ephemeral session for preview
        let now = now_millis()?;
        Session {
            id: "preview".to_string(),
            character_id: character.id.clone(),
            title: "Preview".to_string(),
            system_prompt: None,
            selected_scene_id: None,
            prompt_template_id: None,
            persona_id: None,
            persona_disabled: false,
            voice_autoplay: None,
            advanced_model_settings: None,
            messages: vec![],
            archived: false,
            created_at: now,
            updated_at: now,
            memory_status: None,
            memory_error: None,
            memories: vec![
                "Memory 1 (Preview): The user prefers direct communication.".to_string(),
                "Memory 2 (Preview): We met in the tavern last night.".to_string(),
            ],
            memory_embeddings: vec![],
            memory_summary: Some("This is a placeholder for the context summary that will be generated by the AI based on your conversation history.".to_string()),
            memory_summary_token_count: 0,
            memory_tool_events: vec![],
        }
    };

    let effective_persona_id = resolve_persona_id(&session, persona_id.as_deref());
    let persona = context.choose_persona(effective_persona_id);

    let rendered =
        prompt_engine::render_with_context(&app, &content, &character, persona, &session, settings);
    Ok(rendered)
}

#[tauri::command]
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

#[tauri::command]
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

async fn process_dynamic_memory_cycle(
    app: &AppHandle,
    session: &mut Session,
    settings: &Settings,
    character: &super::types::Character,
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
    character: &super::types::Character,
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
        match find_model_and_credential(settings, &summarisation_model_id) {
            Some(found) => found,
            None => {
                let err = "Summarisation model unavailable";
                log_error(app, "dynamic_memory", err);
                record_dynamic_memory_error(app, session, err, "summary_model");
                return Err(err.to_string());
            }
        };

    let api_key = match resolve_api_key(app, summary_provider, "dynamic_memory") {
        Ok(key) => key,
        Err(err) => {
            record_dynamic_memory_error(app, session, &err, "summary_api_key");
            return Err(err);
        }
    };
    // Set processing state
    session.memory_status = Some("processing".to_string());
    session.memory_error = None;
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
    )
    .await
    {
        Ok(s) => s,
        Err(err) => {
            record_dynamic_memory_error(app, session, &err, "summarization");
            return Err(err);
        }
    };
    log_info(
        app,
        "dynamic_memory",
        format!(
            "summary generated: length={} chars tokens={}",
            summary.len(),
            crate::tokenizer::count_tokens(app, &summary).unwrap_or(0)
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
    let actions = match run_memory_tool_update(
        app,
        summary_provider,
        summary_model,
        &api_key,
        session,
        settings,
        &summary,
        &convo_window,
        character,
    )
    .await
    {
        Ok(actions) => actions,
        Err(err) => {
            log_error(
                app,
                "dynamic_memory",
                format!("memory tool update failed: {}", err),
            );

            let event = json!({
                "id": Uuid::new_v4().to_string(),
                "windowStart": window_start,
                "windowEnd": window_end,
                "windowMessageIds": window_message_ids,
                "summary": summary,
                "actions": [],
                "error": err,
                "status": "error",
                "createdAt": now_millis().unwrap_or_default(),
            });
            session.memory_summary = Some(summary.clone());
            session.memory_summary_token_count =
                crate::tokenizer::count_tokens(app, &summary).unwrap_or(0);
            session.memory_tool_events.push(event);
            if session.memory_tool_events.len() > 50 {
                let excess = session.memory_tool_events.len() - 50;
                session.memory_tool_events.drain(0..excess);
            }
            session.memory_status = Some("failed".to_string());
            session.memory_error = Some(err.clone());
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

    session.memory_summary = Some(summary.clone());
    session.memory_summary_token_count = crate::tokenizer::count_tokens(app, &summary).unwrap_or(0);
    let event = json!({
        "id": Uuid::new_v4().to_string(),
        "windowStart": window_start,
        "windowEnd": window_end,
        "windowMessageIds": window_message_ids,
        "summary": summary,
        "actions": actions,
        "createdAt": now_millis().unwrap_or_default(),
    });
    session.memory_tool_events.push(event);
    if session.memory_tool_events.len() > 50 {
        let excess = session.memory_tool_events.len() - 50;
        session.memory_tool_events.drain(0..excess);
    }

    session.memory_status = Some("idle".to_string());
    session.memory_error = None;
    session.updated_at = now_millis()?;
    if let Err(err) = save_session(app, session) {
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
    log_error(
        app,
        "dynamic_memory",
        format!("{} failed: {}", stage, error),
    );

    session.memory_status = Some("failed".to_string());
    session.memory_error = Some(error.to_string());
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
            "error": error,
            "stage": stage,
        }),
    );
}

async fn run_memory_tool_update(
    app: &AppHandle,
    provider_cred: &ProviderCredential,
    model: &Model,
    api_key: &str,
    session: &mut Session,
    settings: &Settings,
    summary: &str,
    convo_window: &[StoredMessage],
    character: &super::types::Character,
) -> Result<Vec<Value>, String> {
    let tool_config = build_memory_tool_config();
    let max_entries = dynamic_max_entries(settings);

    let mut messages_for_api = Vec::new();
    let system_role = super::request_builder::system_role_for(provider_cred);

    let base_template = prompts::get_template(app, APP_DYNAMIC_MEMORY_TEMPLATE_ID)
        .ok()
        .flatten()
        .map(|t| t.content)
        .unwrap_or_else(|| {
            "You maintain long-term memories for this chat. Use tools to add or delete concise factual memories. Every create_memory call must include a category tag. Keep the list tidy and capped at {{max_entries}} entries. Prefer deleting by ID when removing items. When finished, call the done tool.".to_string()
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
            "Conversation summary:\n{}\n\nRecent messages:\n{}\n\nCurrent memories (with IDs):\n{}",
            summary,
            convo_window.iter().map(|m| format!("{}: {}", m.role, m.content)).collect::<Vec<_>>().join("\n"),
            if memory_lines.is_empty() { "none".to_string() } else { memory_lines.join("\n") }
        )
    }));

    let context_length = resolve_context_length(session, model, settings);
    let max_tokens = resolve_max_tokens(session, model, settings);
    let extra_body_fields = if provider_cred.provider_id == "llamacpp" {
        build_llama_extra_fields(session, model, settings)
    } else if provider_cred.provider_id == "ollama" {
        build_ollama_extra_fields(
            session,
            model,
            settings,
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
    let built = super::request_builder::build_chat_request(
        provider_cred,
        api_key,
        &model.name,
        &messages_for_api,
        None,
        0.2,
        1.0,
        max_tokens, // Dynamic max tokens
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
        return Err(if err_message == fallback {
            err_message
        } else {
            format!("{} (status {})", err_message, api_response.status)
        });
    }

    let usage = extract_usage(api_response.data());
    let context = ChatContext::initialize(app.clone())?;
    record_usage_if_available(
        &context,
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

    let calls = parse_tool_calls(&provider_cred.provider_id, api_response.data());
    if calls.is_empty() {
        log_warn(
            app,
            "dynamic_memory",
            "memory tool call returned no tool usage",
        );
        return Ok(Vec::new());
    }

    let mut actions_log: Vec<Value> = Vec::new();
    let mut untagged_candidates: Vec<(String, bool)> = Vec::new();
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
                                    "dynamic_memory",
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
                                "dynamic_memory",
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
                            untagged_candidates.push((text, is_pinned));
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
                    actions_log.push(json!({
                        "name": "create_memory",
                        "arguments": call.arguments,
                        "memoryId": mem_id,
                        "timestamp": now_millis().unwrap_or_default(),
                        "updatedMemories": format_memories_with_ids(session),
                    }));
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
                                let cold_threshold = dynamic_cold_threshold(settings);
                                session.memory_embeddings[idx].is_cold = true;
                                session.memory_embeddings[idx].importance_score = cold_threshold;
                                log_info(
                                    app,
                                    "dynamic_memory",
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
                                session.memory_embeddings.remove(idx);
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
                            "dynamic_memory",
                            format!("delete_memory could not find target: {}", text),
                        );
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
                        log_info(app, "dynamic_memory", format!("Pinned memory {}", id));
                    } else {
                        log_warn(
                            app,
                            "dynamic_memory",
                            format!("pin_memory could not find: {}", id),
                        );
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
                        log_info(app, "dynamic_memory", format!("Unpinned memory {}", id));
                    } else {
                        log_warn(
                            app,
                            "dynamic_memory",
                            format!("unpin_memory could not find: {}", id),
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

    if !untagged_candidates.is_empty() {
        let mut seen = HashSet::new();
        let candidate_texts: Vec<String> = untagged_candidates
            .iter()
            .map(|(text, _)| text.clone())
            .filter(|text| seen.insert(text.clone()))
            .collect();

        match run_memory_tag_repair(app, provider_cred, model, api_key, &candidate_texts).await {
            Ok(repaired) => {
                for (text, is_pinned) in untagged_candidates {
                    let Some(category) = repaired.get(&text).cloned() else {
                        continue;
                    };

                    let mem_id = generate_memory_id();
                    let embedding =
                        match embedding_model::compute_embedding(app.clone(), text.clone()).await {
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
                    if let Some(ref new_emb) = embedding {
                        let is_duplicate = session.memory_embeddings.iter().any(|existing| {
                            !existing.embedding.is_empty()
                                && cosine_similarity(new_emb, &existing.embedding) > 0.85
                        });
                        if is_duplicate {
                            actions_log.push(json!({
                                "name": "create_memory",
                                "repaired": true,
                                "text": text,
                                "skipped": true,
                                "reason": "duplicate (cosine > 0.85)",
                                "timestamp": now_millis().unwrap_or_default(),
                            }));
                            continue;
                        }
                    }
                    let token_count = crate::tokenizer::count_tokens(app, &text).unwrap_or(0);
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
                        "text": text,
                        "category": category,
                        "memoryId": mem_id,
                        "timestamp": now_millis().unwrap_or_default(),
                        "updatedMemories": format_memories_with_ids(session),
                    }));
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
    api_key: &str,
    texts: &[String],
) -> Result<HashMap<String, String>, String> {
    if texts.is_empty() {
        return Ok(HashMap::new());
    }

    let mut messages_for_api = Vec::new();
    let system_role = super::request_builder::system_role_for(provider_cred);
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

    let built = super::request_builder::build_chat_request(
        provider_cred,
        api_key,
        &model.name,
        &messages_for_api,
        None,
        0.0,
        1.0,
        512,
        None,
        false,
        None,
        None,
        None,
        None,
        Some(&build_memory_tag_repair_tool_config()),
        false,
        None,
        None,
        None,
    );

    let api_request_payload = ApiRequest {
        url: built.url,
        method: Some("POST".into()),
        headers: Some(built.headers),
        query: None,
        body: Some(built.body),
        timeout_ms: Some(30_000),
        stream: Some(false),
        request_id: built.request_id.clone(),
        provider_id: Some(provider_cred.provider_id.clone()),
    };

    let api_response = api_request(app.clone(), api_request_payload).await?;
    if !api_response.ok {
        let fallback = format!("Provider returned status {}", api_response.status);
        let err_message = extract_error_message(api_response.data()).unwrap_or(fallback.clone());
        return Err(if err_message == fallback {
            err_message
        } else {
            format!("{} (status {})", err_message, api_response.status)
        });
    }

    let mut repaired = HashMap::new();
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
    character: &super::types::Character,
    session: &Session,
    settings: &Settings,
    persona: Option<&super::types::Persona>,
) -> Result<String, String> {
    let mut messages_for_api = Vec::new();
    let system_role = super::request_builder::system_role_for(provider_cred);

    let summary_template = prompts::get_template(app, APP_DYNAMIC_SUMMARY_TEMPLATE_ID)
        .ok()
        .flatten()
        .map(|t| t.content)
        .unwrap_or_else(|| {
            "Summarize the recent conversation window into a concise paragraph capturing key facts and decisions. Avoid adding new information.".to_string()
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

    let context_length = resolve_context_length(session, model, settings);
    let max_tokens = resolve_max_tokens(session, model, settings);
    let extra_body_fields = if provider_cred.provider_id == "llamacpp" {
        build_llama_extra_fields(session, model, settings)
    } else if provider_cred.provider_id == "ollama" {
        build_ollama_extra_fields(
            session,
            model,
            settings,
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
    let built = super::request_builder::build_chat_request(
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

    let usage = extract_usage(api_response.data());
    let context = ChatContext::initialize(app.clone())?;
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

    if !api_response.ok {
        let fallback = format!("Provider returned status {}", api_response.status);
        let err_message = extract_error_message(api_response.data()).unwrap_or(fallback.clone());
        return Err(if err_message == fallback {
            err_message
        } else {
            format!("{} (status {})", err_message, api_response.status)
        });
    }

    let calls = parse_tool_calls(&provider_cred.provider_id, api_response.data());
    for call in calls.iter() {
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

    if let Some(text) = extract_text(api_response.data(), Some(&provider_cred.provider_id))
        .filter(|s| !s.is_empty())
    {
        return Ok(text);
    }

    if calls.is_empty() {
        let legacy_hint = if payload_contains_function_call(api_response.data()) {
            " (response uses legacy function_call format)"
        } else {
            ""
        };
        return Err(format!(
            "Failed to summarize recent messages: model returned no tool call and no text{}. Provider={}, model={}",
            legacy_hint, provider_cred.provider_id, model.name
        ));
    }

    let tool_names = calls
        .iter()
        .map(|c| c.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!(
        "Failed to summarize recent messages: expected write_summary tool call, got {}. Provider={}, model={}",
        tool_names, provider_cred.provider_id, model.name
    ))
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

fn find_model_and_credential<'a>(
    settings: &'a Settings,
    model_id: &str,
) -> Option<(&'a Model, &'a ProviderCredential)> {
    let model = settings.models.iter().find(|m| m.id == model_id)?;
    let provider_cred = resolve_provider_credential_for_model(settings, model)?;
    Some((model, provider_cred))
}

fn build_model_attempts<'a>(
    app: &AppHandle,
    settings: &'a Settings,
    character: &Character,
    primary_model: &'a Model,
    primary_provider_cred: &'a ProviderCredential,
    log_scope: &str,
) -> Vec<(&'a Model, &'a ProviderCredential, bool)> {
    let explicit_fallback_candidate = character
        .fallback_model_id
        .as_ref()
        .filter(|fallback_id| *fallback_id != &primary_model.id)
        .and_then(|fallback_id| find_model_and_credential(settings, fallback_id));

    let app_default_fallback_candidate = settings
        .default_model_id
        .as_ref()
        .filter(|default_id| *default_id != &primary_model.id)
        .and_then(|default_id| find_model_and_credential(settings, default_id));

    let mut attempts: Vec<(&Model, &ProviderCredential, bool)> =
        vec![(primary_model, primary_provider_cred, false)];
    if let Some((fallback_model, fallback_cred)) = explicit_fallback_candidate {
        attempts.push((fallback_model, fallback_cred, true));
    } else if character
        .fallback_model_id
        .as_ref()
        .is_some_and(|id| id != &primary_model.id)
    {
        log_warn(
            app,
            log_scope,
            format!(
                "configured character fallback model id {} could not be resolved",
                character.fallback_model_id.as_deref().unwrap_or("")
            ),
        );
        if let Some((fallback_model, fallback_cred)) = app_default_fallback_candidate {
            log_info(
                app,
                log_scope,
                format!(
                    "using app default model {} as fallback candidate",
                    fallback_model.name
                ),
            );
            attempts.push((fallback_model, fallback_cred, true));
        }
    }

    attempts
}

fn emit_fallback_retry_toast(app: &AppHandle, shown: &mut bool) {
    if *shown {
        return;
    }
    emit_toast(
        app,
        "warning",
        "Primary model failed",
        Some("Retrying with fallback model.".to_string()),
    );
    *shown = true;
}

#[tauri::command]
pub async fn chat_add_message_attachment(
    app: AppHandle,
    args: ChatAddMessageAttachmentArgs,
) -> Result<StoredMessage, String> {
    let ChatAddMessageAttachmentArgs {
        session_id,
        character_id,
        message_id,
        role,
        attachment_id,
        base64_data,
        mime_type,
        filename,
        width,
        height,
    } = args;

    if base64_data.trim().is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "base64Data cannot be empty",
        ));
    }

    let mut session = super::storage::load_session(&app, &session_id)?
        .ok_or_else(|| "Session not found".to_string())?;

    let target_index = session
        .messages
        .iter()
        .position(|m| m.id == message_id)
        .ok_or_else(|| "Message not found in loaded session window".to_string())?;

    let storage_path = crate::storage_manager::media::storage_save_session_attachment(
        app.clone(),
        character_id,
        session_id.clone(),
        message_id.clone(),
        attachment_id.clone(),
        role,
        base64_data,
    )?;

    let new_attachment = super::types::ImageAttachment {
        id: attachment_id,
        data: String::new(),
        mime_type,
        filename,
        width,
        height,
        storage_path: Some(storage_path),
    };

    let updated_message = {
        let target = &mut session.messages[target_index];
        if let Some(existing) = target
            .attachments
            .iter_mut()
            .find(|att| att.id == new_attachment.id)
        {
            *existing = new_attachment;
        } else {
            target.attachments.push(new_attachment);
        }
        target.clone()
    };

    session.updated_at = now_millis()?;

    // Persist meta + the updated message (even if it's not the last message).
    let mut meta = session.clone();
    meta.messages = Vec::new();
    let meta_json = serde_json::to_string(&meta)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    session_upsert_meta(app.clone(), meta_json)?;

    let payload = serde_json::to_string(&vec![updated_message.clone()])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    messages_upsert_batch(app.clone(), session_id, payload)?;

    Ok(updated_message)
}

#[tauri::command]
pub async fn search_messages(
    app: AppHandle,
    session_id: String,
    query: String,
) -> Result<Vec<super::types::MessageSearchResult>, String> {
    let context = ChatContext::initialize(app.clone())?;

    let session = match context.load_session(&session_id)? {
        Some(s) => s,
        None => {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Session not found",
            ))
        }
    };

    let query_lower = query.to_lowercase();
    let results: Vec<super::types::MessageSearchResult> = session
        .messages
        .iter()
        .filter(|msg| {
            msg.content.to_lowercase().contains(&query_lower)
                && (msg.role == "user" || msg.role == "assistant")
        })
        .map(|msg| super::types::MessageSearchResult {
            message_id: msg.id.clone(),
            content: msg.content.clone(),
            created_at: msg.created_at,
            role: msg.role.clone(),
        })
        .collect();

    Ok(results)
}

#[tauri::command]
pub async fn chat_generate_user_reply(
    app: AppHandle,
    session_id: String,
    current_draft: Option<String>,
    request_id: Option<String>,
    swap_places: Option<bool>,
) -> Result<String, String> {
    log_info(
        &app,
        "help_me_reply",
        format!(
            "Generating user reply for session={}, has_draft={}",
            &session_id,
            current_draft.is_some()
        ),
    );

    let swap_places = role_swap_enabled(swap_places);
    let context = ChatContext::initialize(app.clone())?;
    let settings = &context.settings;

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

    let session = match context.load_session(&session_id)? {
        Some(s) => s,
        None => {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Session not found",
            ))
        }
    };

    let character = context.find_character(&session.character_id)?;
    let persona = context.choose_persona(resolve_persona_id(&session, None));
    let (prompt_character, prompt_persona) = if swap_places {
        swapped_prompt_entities(&character, persona)
    } else {
        (character.clone(), persona.cloned())
    };

    let recent_msgs = recent_messages(&session, 10);

    if recent_msgs.is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "No conversation history to base reply on",
        ));
    }

    // Use help me reply model if configured, otherwise fall back to default
    let model_id = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.help_me_reply_model_id.as_ref())
        .or(settings.default_model_id.as_ref())
        .ok_or_else(|| "No model configured for Help Me Reply".to_string())?;

    let model = settings
        .models
        .iter()
        .find(|m| &m.id == model_id)
        .ok_or_else(|| "Help Me Reply model not found".to_string())?;

    let provider_cred = resolve_provider_credential_for_model(settings, model)
        .ok_or_else(|| "Provider credential not found".to_string())?;

    let api_key = resolve_api_key(&app, provider_cred, "help_me_reply")?;

    // Get reply style from settings (default to roleplay)
    let reply_style = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.help_me_reply_style.as_ref())
        .map(|s| s.as_str())
        .unwrap_or("roleplay");

    let base_prompt = prompts::get_help_me_reply_prompt(&app, reply_style);

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

    let char_name = &prompt_character.name;
    let char_desc = prompt_character
        .definition
        .as_deref()
        .or(prompt_character.description.as_deref())
        .unwrap_or("");
    let persona_name = prompt_persona
        .as_ref()
        .map(|p| p.title.as_str())
        .unwrap_or("User");
    let persona_desc = prompt_persona
        .as_ref()
        .map(|p| p.description.as_str())
        .unwrap_or("");

    let mut system_prompt = base_prompt;
    system_prompt = system_prompt.replace("{{char.name}}", char_name);
    system_prompt = system_prompt.replace("{{char.desc}}", char_desc);
    system_prompt = system_prompt.replace("{{persona.name}}", persona_name);
    system_prompt = system_prompt.replace("{{persona.desc}}", persona_desc);
    system_prompt = system_prompt.replace("{{user.name}}", persona_name);
    system_prompt = system_prompt.replace("{{user.desc}}", persona_desc);
    let draft_str = current_draft.as_deref().unwrap_or("");
    system_prompt = system_prompt.replace("{{current_draft}}", draft_str);
    // Legacy placeholders
    system_prompt = system_prompt.replace("{{char}}", char_name);
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

    let effective_user_name = if swap_places { char_name } else { persona_name };
    let effective_assistant_name = if swap_places { persona_name } else { char_name };

    let conversation_context = recent_msgs
        .iter()
        .map(|msg| {
            let effective_role = if swap_places {
                swap_role_for_api(msg.role.as_str())
            } else {
                msg.role.as_str()
            };
            let role_label = if effective_role == "user" {
                effective_user_name
            } else {
                effective_assistant_name
            };
            format!("{}: {}", role_label, msg.content)
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let user_prompt = format!(
        "Here is the recent conversation:\n\n{}\n\nGenerate a reply for {} to say next.",
        conversation_context, effective_user_name
    );

    let messages_for_api: Vec<Value> = vec![
        json!({ "role": "system", "content": system_prompt }),
        json!({ "role": "user", "content": user_prompt }),
    ];

    let context_length = resolve_context_length(&session, model, settings);
    let extra_body_fields = if provider_cred.provider_id == "llamacpp" {
        build_llama_extra_fields(&session, model, settings)
    } else if provider_cred.provider_id == "ollama" {
        build_ollama_extra_fields(
            &session,
            model,
            settings,
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
    let built = super::request_builder::build_chat_request(
        provider_cred,
        &api_key,
        &model.name,
        &messages_for_api,
        None,       // system_prompt already in messages
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
        "help_me_reply",
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
        return Err(format!(
            "API request failed with status {}",
            api_response.status
        ));
    }

    let generated_text = extract_text(&api_response.data, Some(&provider_cred.provider_id))
        .ok_or_else(|| "Failed to extract text from response".to_string())?;

    let cleaned = generated_text
        .trim()
        .trim_matches('"')
        .trim_start_matches(&format!("{}:", effective_user_name))
        .trim()
        .to_string();

    log_info(
        &app,
        "help_me_reply",
        format!("Generated reply: {} chars", cleaned.len()),
    );

    let usage = super::sse::usage_from_value(&api_response.data);
    super::service::record_usage_if_available(
        &context,
        &usage,
        &session,
        &prompt_character,
        &model,
        &provider_cred,
        &api_key,
        now_millis().unwrap_or(0),
        UsageOperationType::ReplyHelper,
        "help_me_reply",
    )
    .await;

    Ok(cleaned)
}

/// Helper to remove {{#if current_draft}}...{{else}}...{{/if}} and keep else content
fn remove_if_block(prompt: &mut String) {
    if let Some(if_start) = prompt.find("{{#if current_draft}}") {
        if let Some(else_pos) = prompt.find("{{else}}") {
            prompt.replace_range(if_start..(else_pos + 8), "");
        }
    }
    *prompt = prompt.replace("{{/if}}", "");
}
