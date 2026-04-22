use rusqlite::{params, OptionalExtension};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::collections::HashMap;
use tauri::Manager;
use uuid;

use super::db::{now_ms, open_db};
use crate::chat_manager::types::{
    AdvancedModelSettings, ImageAttachment, MemoryEmbedding, MessageVariant, Session,
    StoredMessage, UsageSummary,
};
use crate::dynamic_memory_run_manager::DynamicMemoryRunManager;
use crate::embedding;
use crate::utils::{log_error, log_info, log_warn};

const ALLOWED_MEMORY_CATEGORIES: &[&str] = &[
    "character_trait",
    "relationship",
    "plot_event",
    "world_detail",
    "preference",
    "other",
];

fn normalize_memory_category(category: Option<String>) -> Result<Option<String>, String> {
    let normalized = category
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty());

    match normalized {
        Some(value) if !ALLOWED_MEMORY_CATEGORIES.contains(&value.as_str()) => {
            Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                format!(
                    "Invalid memory category '{}'. Allowed values: {}",
                    value,
                    ALLOWED_MEMORY_CATEGORIES.join(", ")
                ),
            ))
        }
        Some(value) => Ok(Some(value)),
        None => Ok(None),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionPreview {
    id: String,
    character_id: String,
    title: String,
    updated_at: i64,
    archived: bool,
    last_message: String,
    message_count: i64,
}

fn session_preview_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<SessionPreview> {
    Ok(SessionPreview {
        id: r.get(0)?,
        character_id: r.get(1)?,
        title: r.get(2)?,
        updated_at: r.get(3)?,
        archived: r.get::<_, i64>(4)? != 0,
        last_message: r.get::<_, String>(5)?,
        message_count: r.get(6)?,
    })
}

fn json_usage_summary(
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    total_tokens: Option<i64>,
) -> Option<JsonValue> {
    let mut usage = JsonMap::new();
    if let Some(v) = prompt_tokens {
        usage.insert("promptTokens".into(), JsonValue::from(v));
    }
    if let Some(v) = completion_tokens {
        usage.insert("completionTokens".into(), JsonValue::from(v));
    }
    if let Some(v) = total_tokens {
        usage.insert("totalTokens".into(), JsonValue::from(v));
    }
    if usage.is_empty() {
        None
    } else {
        Some(JsonValue::Object(usage))
    }
}

fn typed_usage_summary(
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    total_tokens: Option<i64>,
) -> Option<UsageSummary> {
    if prompt_tokens.is_none() && completion_tokens.is_none() && total_tokens.is_none() {
        return None;
    }

    Some(UsageSummary {
        prompt_tokens: prompt_tokens.map(|value| value as u64),
        completion_tokens: completion_tokens.map(|value| value as u64),
        total_tokens: total_tokens.map(|value| value as u64),
        cached_prompt_tokens: None,
        cache_write_tokens: None,
        reasoning_tokens: None,
        image_tokens: None,
        web_search_requests: None,
        api_cost: None,
        response_id: None,
        first_token_ms: None,
        tokens_per_second: None,
        finish_reason: None,
    })
}

fn parse_json_or_default<T>(raw: &str) -> T
where
    T: DeserializeOwned + Default,
{
    serde_json::from_str(raw).unwrap_or_default()
}

fn parse_optional_json_or_default<T>(raw: Option<&str>) -> T
where
    T: DeserializeOwned + Default,
{
    raw.and_then(|value| serde_json::from_str(value).ok())
        .unwrap_or_default()
}

fn resolve_lorebook_ids_override_json(
    conn: &rusqlite::Connection,
    session_id: &str,
    session: &JsonValue,
) -> Result<Option<String>, String> {
    match session.get("lorebookIdsOverride") {
        Some(value) if value.is_null() => Ok(None),
        Some(value) => Ok(Some(
            serde_json::to_string(value).unwrap_or_else(|_| "[]".to_string()),
        )),
        None => conn
            .query_row(
                "SELECT lorebook_ids_override FROM sessions WHERE id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e)),
    }
}

fn resolve_author_note(
    conn: &rusqlite::Connection,
    session_id: &str,
    session: &JsonValue,
) -> Result<Option<String>, String> {
    match session.get("authorNote") {
        Some(value) if value.is_null() => Ok(None),
        Some(value) => Ok(value
            .as_str()
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty())),
        None => conn
            .query_row(
                "SELECT author_note FROM sessions WHERE id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e)),
    }
}

fn build_session_advanced_model_settings(
    temperature: Option<f64>,
    top_p: Option<f64>,
    max_output_tokens: Option<i64>,
    frequency_penalty: Option<f64>,
    presence_penalty: Option<f64>,
    top_k: Option<i64>,
) -> Option<AdvancedModelSettings> {
    if temperature.is_some()
        || top_p.is_some()
        || max_output_tokens.is_some()
        || frequency_penalty.is_some()
        || presence_penalty.is_some()
        || top_k.is_some()
    {
        Some(AdvancedModelSettings {
            temperature,
            top_p,
            max_output_tokens: max_output_tokens.map(|value| value as u32),
            frequency_penalty,
            presence_penalty,
            top_k: top_k.map(|value| value as u32),
            ..AdvancedModelSettings::default()
        })
    } else {
        None
    }
}

fn read_session_meta_typed_internal(
    conn: &rusqlite::Connection,
    id: &str,
) -> Result<Option<Session>, String> {
    let row = conn
        .query_row(
            "SELECT character_id, title, background_image_path, system_prompt, selected_scene_id, persona_id, persona_disabled, voice_autoplay, temperature, top_p, max_output_tokens, frequency_penalty, presence_penalty, top_k, memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events, memory_status, memory_error, archived, created_at, updated_at, prompt_template_id, lorebook_ids_override, memory_progress_step, author_note FROM sessions WHERE id = ?",
            params![id],
            |r| Ok((
                r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, Option<String>>(3)?, r.get::<_, Option<String>>(4)?, r.get::<_, Option<String>>(5)?, r.get::<_, Option<i64>>(6)?, r.get::<_, Option<i64>>(7)?, r.get::<_, Option<f64>>(8)?, r.get::<_, Option<f64>>(9)?, r.get::<_, Option<i64>>(10)?, r.get::<_, Option<f64>>(11)?, r.get::<_, Option<f64>>(12)?, r.get::<_, Option<i64>>(13)?, r.get::<_, String>(14)?, r.get::<_, String>(15)?, r.get::<_, Option<String>>(16)?, r.get::<_, i64>(17)?, r.get::<_, String>(18)?, r.get::<_, Option<String>>(19)?, r.get::<_, Option<String>>(20)?, r.get::<_, i64>(21)?, r.get::<_, i64>(22)?, r.get::<_, i64>(23)?, r.get::<_, Option<String>>(24)?, r.get::<_, Option<String>>(25)?, r.get::<_, Option<i64>>(26)?, r.get::<_, Option<String>>(27)?
            )),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let Some((
        character_id,
        title,
        background_image_path,
        system_prompt,
        selected_scene_id,
        persona_id,
        persona_disabled,
        voice_autoplay,
        temperature,
        top_p,
        max_output_tokens,
        frequency_penalty,
        presence_penalty,
        top_k,
        memories_json,
        memory_embeddings_json,
        memory_summary,
        memory_summary_token_count,
        memory_tool_events_json,
        memory_status,
        memory_error,
        archived,
        created_at,
        updated_at,
        prompt_template_id,
        lorebook_ids_override_json,
        memory_progress_step,
        author_note,
    )) = row
    else {
        return Ok(None);
    };

    Ok(Some(Session {
        id: id.to_string(),
        character_id,
        title,
        background_image_path,
        system_prompt,
        selected_scene_id,
        prompt_template_id,
        lorebook_ids_override: parse_optional_json_or_default(
            lorebook_ids_override_json.as_deref(),
        ),
        author_note,
        persona_id,
        persona_disabled: persona_disabled.unwrap_or(0) != 0,
        voice_autoplay: voice_autoplay.map(|value| value != 0),
        advanced_model_settings: build_session_advanced_model_settings(
            temperature,
            top_p,
            max_output_tokens,
            frequency_penalty,
            presence_penalty,
            top_k,
        ),
        memories: parse_json_or_default(&memories_json),
        memory_embeddings: parse_json_or_default::<Vec<MemoryEmbedding>>(&memory_embeddings_json),
        memory_summary,
        memory_summary_token_count: memory_summary_token_count.max(0) as u32,
        memory_tool_events: parse_json_or_default(&memory_tool_events_json),
        memory_status,
        memory_error,
        memory_progress_step: memory_progress_step.map(|v| v as u32),
        messages: Vec::new(),
        archived: archived != 0,
        created_at: created_at as u64,
        updated_at: updated_at as u64,
    }))
}

fn fetch_messages_page_typed(
    conn: &rusqlite::Connection,
    session_id: &str,
    limit: i64,
    before_created_at: Option<i64>,
    before_id: Option<&str>,
) -> Result<Vec<StoredMessage>, String> {
    let mut sql = String::from(
        "SELECT id, role, content, created_at, scene_edited, prompt_tokens, completion_tokens, total_tokens, selected_variant_id, is_pinned, memory_refs, used_lorebook_entries, attachments, reasoning FROM messages WHERE session_id = ?1",
    );

    let use_before = before_created_at.is_some() && before_id.is_some();
    if use_before {
        sql.push_str(" AND (created_at < ?2 OR (created_at = ?2 AND id < ?3))");
    }
    sql.push_str(" ORDER BY created_at DESC, id DESC LIMIT ");
    sql.push_str(&limit.to_string());

    type RawMessageRow = (
        String,
        String,
        String,
        i64,
        i64,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<String>,
        i64,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    );

    let mut raw_messages: Vec<RawMessageRow> = Vec::new();
    let mut message_ids: Vec<String> = Vec::new();
    {
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if use_before {
            let rows = stmt
                .query_map(
                    params![session_id, before_created_at.unwrap(), before_id.unwrap()],
                    |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?,
                            r.get::<_, i64>(3)?,
                            r.get::<_, i64>(4)?,
                            r.get::<_, Option<i64>>(5)?,
                            r.get::<_, Option<i64>>(6)?,
                            r.get::<_, Option<i64>>(7)?,
                            r.get::<_, Option<String>>(8)?,
                            r.get::<_, i64>(9)?,
                            r.get::<_, Option<String>>(10)?,
                            r.get::<_, Option<String>>(11)?,
                            r.get::<_, Option<String>>(12)?,
                            r.get::<_, Option<String>>(13)?,
                        ))
                    },
                )
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            for row in rows {
                let tuple =
                    row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                message_ids.push(tuple.0.clone());
                raw_messages.push(tuple);
            }
        } else {
            let rows = stmt
                .query_map(params![session_id], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, i64>(3)?,
                        r.get::<_, i64>(4)?,
                        r.get::<_, Option<i64>>(5)?,
                        r.get::<_, Option<i64>>(6)?,
                        r.get::<_, Option<i64>>(7)?,
                        r.get::<_, Option<String>>(8)?,
                        r.get::<_, i64>(9)?,
                        r.get::<_, Option<String>>(10)?,
                        r.get::<_, Option<String>>(11)?,
                        r.get::<_, Option<String>>(12)?,
                        r.get::<_, Option<String>>(13)?,
                    ))
                })
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            for row in rows {
                let tuple =
                    row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                message_ids.push(tuple.0.clone());
                raw_messages.push(tuple);
            }
        }
    }

    let mut variants_by_message: HashMap<String, Vec<MessageVariant>> = HashMap::new();
    if !message_ids.is_empty() {
        let placeholders = message_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let vsql = format!(
            "SELECT message_id, id, content, created_at, prompt_tokens, completion_tokens, total_tokens, reasoning FROM message_variants WHERE message_id IN ({}) ORDER BY created_at ASC",
            placeholders
        );
        let mut vstmt = conn
            .prepare(&vsql)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let vrows = vstmt
            .query_map(rusqlite::params_from_iter(message_ids.iter()), |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, Option<i64>>(4)?,
                    r.get::<_, Option<i64>>(5)?,
                    r.get::<_, Option<i64>>(6)?,
                    r.get::<_, Option<String>>(7)?,
                ))
            })
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        for vr in vrows {
            let (
                message_id,
                id,
                content,
                created_at,
                prompt_tokens,
                completion_tokens,
                total_tokens,
                reasoning,
            ) = vr.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            variants_by_message
                .entry(message_id)
                .or_default()
                .push(MessageVariant {
                    id,
                    content,
                    created_at: created_at as u64,
                    usage: typed_usage_summary(prompt_tokens, completion_tokens, total_tokens),
                    attachments: Vec::new(),
                    reasoning,
                });
        }
    }

    let mut out: Vec<StoredMessage> = Vec::with_capacity(raw_messages.len());
    for (
        id,
        role,
        content,
        created_at,
        scene_edited,
        prompt_tokens,
        completion_tokens,
        total_tokens,
        selected_variant_id,
        is_pinned,
        memory_refs_json,
        used_lorebook_entries_json,
        attachments_json,
        reasoning,
    ) in raw_messages
    {
        out.push(StoredMessage {
            id: id.clone(),
            role,
            content,
            created_at: created_at as u64,
            scene_edited: scene_edited != 0,
            usage: typed_usage_summary(prompt_tokens, completion_tokens, total_tokens),
            variants: variants_by_message.remove(&id).unwrap_or_default(),
            selected_variant_id,
            memory_refs: parse_optional_json_or_default(memory_refs_json.as_deref()),
            used_lorebook_entries: parse_optional_json_or_default(
                used_lorebook_entries_json.as_deref(),
            ),
            is_pinned: is_pinned != 0,
            attachments: parse_optional_json_or_default::<Vec<ImageAttachment>>(
                attachments_json.as_deref(),
            ),
            reasoning,
            model_id: None,
            fallback_from_model_id: None,
        });
    }

    out.reverse();
    Ok(out)
}

fn fetch_pinned_messages_typed(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> Result<Vec<StoredMessage>, String> {
    let mut stmt = conn
        .prepare("SELECT id FROM messages WHERE session_id = ? AND is_pinned = 1 ORDER BY created_at ASC, id ASC")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map(params![session_id], |r| Ok(r.get::<_, String>(0)?))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut pinned_ids: Vec<String> = Vec::new();
    for row in rows {
        pinned_ids.push(row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?);
    }
    if pinned_ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = pinned_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT id, role, content, created_at, scene_edited, prompt_tokens, completion_tokens, total_tokens, selected_variant_id, is_pinned, memory_refs, used_lorebook_entries, attachments, reasoning FROM messages WHERE session_id = ?1 AND id IN ({}) ORDER BY created_at ASC, id ASC",
        placeholders
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut params_vec: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(1 + pinned_ids.len());
    params_vec.push(&session_id);
    for id in &pinned_ids {
        params_vec.push(id);
    }

    let rows = stmt
        .query_map(params_vec.as_slice(), |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, Option<i64>>(5)?,
                r.get::<_, Option<i64>>(6)?,
                r.get::<_, Option<i64>>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, i64>(9)?,
                r.get::<_, Option<String>>(10)?,
                r.get::<_, Option<String>>(11)?,
                r.get::<_, Option<String>>(12)?,
                r.get::<_, Option<String>>(13)?,
            ))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut raw_messages = Vec::new();
    let mut message_ids = Vec::new();
    for row in rows {
        let tuple = row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        message_ids.push(tuple.0.clone());
        raw_messages.push(tuple);
    }

    let mut variants_by_message: HashMap<String, Vec<MessageVariant>> = HashMap::new();
    if !message_ids.is_empty() {
        let placeholders = message_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let vsql = format!(
            "SELECT message_id, id, content, created_at, prompt_tokens, completion_tokens, total_tokens, reasoning FROM message_variants WHERE message_id IN ({}) ORDER BY created_at ASC",
            placeholders
        );
        let mut vstmt = conn
            .prepare(&vsql)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let rows = vstmt
            .query_map(rusqlite::params_from_iter(message_ids.iter()), |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, Option<i64>>(4)?,
                    r.get::<_, Option<i64>>(5)?,
                    r.get::<_, Option<i64>>(6)?,
                    r.get::<_, Option<String>>(7)?,
                ))
            })
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        for row in rows {
            let (
                message_id,
                id,
                content,
                created_at,
                prompt_tokens,
                completion_tokens,
                total_tokens,
                reasoning,
            ) = row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            variants_by_message
                .entry(message_id)
                .or_default()
                .push(MessageVariant {
                    id,
                    content,
                    created_at: created_at as u64,
                    usage: typed_usage_summary(prompt_tokens, completion_tokens, total_tokens),
                    attachments: Vec::new(),
                    reasoning,
                });
        }
    }

    let mut messages = Vec::with_capacity(raw_messages.len());
    for (
        id,
        role,
        content,
        created_at,
        scene_edited,
        prompt_tokens,
        completion_tokens,
        total_tokens,
        selected_variant_id,
        is_pinned,
        memory_refs_json,
        used_lorebook_entries_json,
        attachments_json,
        reasoning,
    ) in raw_messages
    {
        messages.push(StoredMessage {
            id: id.clone(),
            role,
            content,
            created_at: created_at as u64,
            scene_edited: scene_edited != 0,
            usage: typed_usage_summary(prompt_tokens, completion_tokens, total_tokens),
            variants: variants_by_message.remove(&id).unwrap_or_default(),
            selected_variant_id,
            memory_refs: parse_optional_json_or_default(memory_refs_json.as_deref()),
            used_lorebook_entries: parse_optional_json_or_default(
                used_lorebook_entries_json.as_deref(),
            ),
            is_pinned: is_pinned != 0,
            attachments: parse_optional_json_or_default::<Vec<ImageAttachment>>(
                attachments_json.as_deref(),
            ),
            reasoning,
            model_id: None,
            fallback_from_model_id: None,
        });
    }

    Ok(messages)
}

fn upsert_session_meta_value(app: &tauri::AppHandle, s: &JsonValue) -> Result<(), String> {
    let conn = open_db(app)?;
    let id = s
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "id is required".to_string())?
        .to_string();
    let character_id = s
        .get("characterId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "characterId is required".to_string())?;
    let title = s.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let background_image_path = s
        .get("backgroundImagePath")
        .and_then(|v| v.as_str())
        .map(|x| x.to_string());
    let system_prompt = s
        .get("systemPrompt")
        .and_then(|v| v.as_str())
        .map(|x| x.to_string());
    let selected_scene_id = s
        .get("selectedSceneId")
        .and_then(|v| v.as_str())
        .map(|x| x.to_string());
    let prompt_template_id = s
        .get("promptTemplateId")
        .and_then(|v| v.as_str())
        .map(|x| x.to_string());
    let lorebook_ids_override_json = resolve_lorebook_ids_override_json(&conn, &id, s)?;
    let author_note = resolve_author_note(&conn, &id, s)?;
    let persona_id = s
        .get("personaId")
        .and_then(|v| v.as_str())
        .map(|x| x.to_string());
    let persona_disabled = s
        .get("personaDisabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false) as i64;
    let voice_autoplay = s
        .get("voiceAutoplay")
        .and_then(|v| v.as_bool())
        .map(|value| if value { 1 } else { 0 });
    let archived = s.get("archived").and_then(|v| v.as_bool()).unwrap_or(false) as i64;
    let created_at = s
        .get("createdAt")
        .and_then(|v| v.as_i64())
        .unwrap_or(now_ms() as i64);
    let updated_at = now_ms() as i64;

    let memories_json = match s.get("memories") {
        Some(v) => serde_json::to_string(v).unwrap_or_else(|_| "[]".to_string()),
        None => "[]".to_string(),
    };
    let memory_summary = s
        .get("memorySummary")
        .and_then(|v| v.as_str())
        .map(|value| value.to_string());
    let memory_summary_token_count = s
        .get("memorySummaryTokenCount")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let memory_tool_events_json = match s.get("memoryToolEvents") {
        Some(v) => serde_json::to_string(v).unwrap_or_else(|_| "[]".to_string()),
        None => "[]".to_string(),
    };
    let memory_embeddings_json = match s.get("memoryEmbeddings") {
        Some(v) => serde_json::to_string(v).unwrap_or_else(|_| "[]".to_string()),
        None => "[]".to_string(),
    };
    let memory_status = s
        .get("memoryStatus")
        .and_then(|v| v.as_str())
        .map(|value| value.to_string());
    let memory_error = s
        .get("memoryError")
        .and_then(|v| v.as_str())
        .map(|value| value.to_string());
    let memory_progress_step = s.get("memoryProgressStep").and_then(|v| v.as_i64());

    let adv = s.get("advancedModelSettings");
    let temperature = adv
        .and_then(|v| v.get("temperature"))
        .and_then(|v| v.as_f64());
    let top_p = adv.and_then(|v| v.get("topP")).and_then(|v| v.as_f64());
    let max_output_tokens = adv
        .and_then(|v| v.get("maxOutputTokens"))
        .and_then(|v| v.as_i64());
    let frequency_penalty = adv
        .and_then(|v| v.get("frequencyPenalty"))
        .and_then(|v| v.as_f64());
    let presence_penalty = adv
        .and_then(|v| v.get("presencePenalty"))
        .and_then(|v| v.as_f64());
    let top_k = adv.and_then(|v| v.get("topK")).and_then(|v| v.as_i64());

    conn.execute(
        r#"INSERT INTO sessions (id, character_id, title, background_image_path, system_prompt, selected_scene_id, prompt_template_id, lorebook_ids_override, author_note, persona_id, persona_disabled, voice_autoplay, temperature, top_p, max_output_tokens, frequency_penalty, presence_penalty, top_k, memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events, memory_status, memory_error, memory_progress_step, archived, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
              character_id=excluded.character_id,
              title=excluded.title,
              background_image_path=excluded.background_image_path,
              system_prompt=excluded.system_prompt,
              selected_scene_id=excluded.selected_scene_id,
              prompt_template_id=excluded.prompt_template_id,
              lorebook_ids_override=excluded.lorebook_ids_override,
              author_note=excluded.author_note,
              persona_id=excluded.persona_id,
              persona_disabled=excluded.persona_disabled,
              voice_autoplay=excluded.voice_autoplay,
              temperature=excluded.temperature,
              top_p=excluded.top_p,
              max_output_tokens=excluded.max_output_tokens,
              frequency_penalty=excluded.frequency_penalty,
              presence_penalty=excluded.presence_penalty,
              top_k=excluded.top_k,
              memories=excluded.memories,
              memory_embeddings=excluded.memory_embeddings,
              memory_summary=excluded.memory_summary,
              memory_summary_token_count=excluded.memory_summary_token_count,
              memory_tool_events=excluded.memory_tool_events,
              memory_status=excluded.memory_status,
              memory_error=excluded.memory_error,
              memory_progress_step=excluded.memory_progress_step,
              archived=excluded.archived,
              updated_at=excluded.updated_at"#,
        params![
            &id,
            character_id,
            title,
            background_image_path,
            system_prompt,
            selected_scene_id,
            prompt_template_id,
            lorebook_ids_override_json,
            author_note,
            persona_id,
            persona_disabled,
            voice_autoplay,
            temperature,
            top_p,
            max_output_tokens,
            frequency_penalty,
            presence_penalty,
            top_k,
            &memories_json,
            &memory_embeddings_json,
            memory_summary,
            memory_summary_token_count,
            &memory_tool_events_json,
            memory_status,
            memory_error,
            memory_progress_step,
            archived,
            created_at,
            updated_at
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    Ok(())
}

fn upsert_messages_batch_value(
    app: &tauri::AppHandle,
    session_id: &str,
    msgs: &[JsonValue],
) -> Result<(), String> {
    let mut conn = open_db(app)?;

    log_info(
        app,
        "messages_upsert_batch",
        format!(
            "Upserting {} messages for session {}",
            msgs.len(),
            session_id
        ),
    );

    let now = now_ms() as i64;
    let tx = conn.transaction().map_err(|e| {
        log_error(
            app,
            "messages_upsert_batch",
            format!("Failed to begin transaction: {}", e),
        );
        e.to_string()
    })?;

    for m in msgs {
        let mid = m
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "message.id is required".to_string())?;
        let role = m.get("role").and_then(|v| v.as_str()).unwrap_or("user");
        let content = m.get("content").and_then(|v| v.as_str()).unwrap_or("");
        let mcreated = m.get("createdAt").and_then(|v| v.as_i64()).unwrap_or(now);
        let is_pinned = m.get("isPinned").and_then(|v| v.as_bool()).unwrap_or(false) as i64;
        let scene_edited = m
            .get("sceneEdited")
            .and_then(|v| v.as_bool())
            .unwrap_or(false) as i64;
        let usage = m.get("usage");
        let pt = usage
            .and_then(|u| u.get("promptTokens"))
            .and_then(|v| v.as_i64());
        let ct = usage
            .and_then(|u| u.get("completionTokens"))
            .and_then(|v| v.as_i64());
        let tt = usage
            .and_then(|u| u.get("totalTokens"))
            .and_then(|v| v.as_i64());
        let selected_variant_id = m
            .get("selectedVariantId")
            .and_then(|v| v.as_str())
            .map(|x| x.to_string());
        let memory_refs = m
            .get("memoryRefs")
            .cloned()
            .unwrap_or_else(|| JsonValue::Array(Vec::new()));
        let used_lorebook_entries = m
            .get("usedLorebookEntries")
            .cloned()
            .unwrap_or_else(|| JsonValue::Array(Vec::new()));
        let attachments = m
            .get("attachments")
            .cloned()
            .unwrap_or_else(|| JsonValue::Array(Vec::new()));
        let reasoning = m
            .get("reasoning")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        tx.execute(
            r#"INSERT INTO messages (id, session_id, role, content, created_at, scene_edited, prompt_tokens, completion_tokens, total_tokens, selected_variant_id, is_pinned, memory_refs, used_lorebook_entries, attachments, reasoning)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT(id) DO UPDATE SET
                 session_id=excluded.session_id,
                 role=excluded.role,
                 content=excluded.content,
                 created_at=excluded.created_at,
                 scene_edited=excluded.scene_edited,
                 prompt_tokens=excluded.prompt_tokens,
                 completion_tokens=excluded.completion_tokens,
                 total_tokens=excluded.total_tokens,
                 selected_variant_id=excluded.selected_variant_id,
                 is_pinned=excluded.is_pinned,
                 memory_refs=excluded.memory_refs,
                 used_lorebook_entries=excluded.used_lorebook_entries,
                 attachments=excluded.attachments,
                 reasoning=excluded.reasoning"#,
            params![
                &mid,
                session_id,
                role,
                content,
                mcreated,
                scene_edited,
                pt,
                ct,
                tt,
                selected_variant_id,
                is_pinned,
                memory_refs.to_string(),
                used_lorebook_entries.to_string(),
                attachments.to_string(),
                reasoning
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        if m.get("variants").is_some() {
            tx.execute(
                "DELETE FROM message_variants WHERE message_id = ?",
                params![&mid],
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            if let Some(vars) = m.get("variants").and_then(|v| v.as_array()) {
                for v in vars {
                    let vid = v
                        .get("id")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                    let vcontent = v.get("content").and_then(|x| x.as_str()).unwrap_or("");
                    let vcreated = v.get("createdAt").and_then(|x| x.as_i64()).unwrap_or(now);
                    let u = v.get("usage");
                    let vp = u
                        .and_then(|u| u.get("promptTokens"))
                        .and_then(|v| v.as_i64());
                    let vc = u
                        .and_then(|u| u.get("completionTokens"))
                        .and_then(|v| v.as_i64());
                    let vt = u
                        .and_then(|u| u.get("totalTokens"))
                        .and_then(|v| v.as_i64());
                    let vreasoning = v
                        .get("reasoning")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string());
                    tx.execute(
                        "INSERT INTO message_variants (id, message_id, content, created_at, prompt_tokens, completion_tokens, total_tokens, reasoning) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                        params![vid, &mid, vcontent, vcreated, vp, vc, vt, vreasoning],
                    )
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                }
            }
        }
    }

    tx.execute(
        "UPDATE sessions SET updated_at = ? WHERE id = ?",
        params![now, session_id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

pub fn session_get_meta_internal(
    app: &tauri::AppHandle,
    id: &str,
) -> Result<Option<Session>, String> {
    let conn = open_db(app)?;
    read_session_meta_typed_internal(&conn, id)
}

pub fn messages_list_internal(
    app: &tauri::AppHandle,
    session_id: &str,
    limit: i64,
    before_created_at: Option<i64>,
    before_id: Option<&str>,
) -> Result<Vec<StoredMessage>, String> {
    let conn = open_db(app)?;
    fetch_messages_page_typed(&conn, session_id, limit, before_created_at, before_id)
}

pub fn messages_list_pinned_internal(
    app: &tauri::AppHandle,
    session_id: &str,
) -> Result<Vec<StoredMessage>, String> {
    let conn = open_db(app)?;
    fetch_pinned_messages_typed(&conn, session_id)
}

pub fn session_upsert_meta_internal(
    app: &tauri::AppHandle,
    session: &Session,
) -> Result<(), String> {
    let value = serde_json::to_value(session)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    upsert_session_meta_value(app, &value)
}

pub fn messages_upsert_batch_internal(
    app: &tauri::AppHandle,
    session_id: &str,
    messages: &[StoredMessage],
) -> Result<(), String> {
    let value = serde_json::to_value(messages)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let Some(items) = value.as_array() else {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "messages must serialize to a JSON array",
        ));
    };
    upsert_messages_batch_value(app, session_id, items)
}

fn read_session_meta(conn: &rusqlite::Connection, id: &str) -> Result<Option<JsonValue>, String> {
    let row = conn
        .query_row(
            "SELECT character_id, title, background_image_path, system_prompt, selected_scene_id, persona_id, persona_disabled, voice_autoplay, temperature, top_p, max_output_tokens, frequency_penalty, presence_penalty, top_k, memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events, memory_status, memory_error, archived, created_at, updated_at, prompt_template_id, lorebook_ids_override, memory_progress_step, author_note FROM sessions WHERE id = ?",
            params![id],
            |r| Ok((
                r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, Option<String>>(3)?, r.get::<_, Option<String>>(4)?, r.get::<_, Option<String>>(5)?, r.get::<_, Option<i64>>(6)?, r.get::<_, Option<i64>>(7)?, r.get::<_, Option<f64>>(8)?, r.get::<_, Option<f64>>(9)?, r.get::<_, Option<i64>>(10)?, r.get::<_, Option<f64>>(11)?, r.get::<_, Option<f64>>(12)?, r.get::<_, Option<i64>>(13)?, r.get::<_, String>(14)?, r.get::<_, String>(15)?, r.get::<_, Option<String>>(16)?, r.get::<_, i64>(17)?, r.get::<_, String>(18)?, r.get::<_, Option<String>>(19)?, r.get::<_, Option<String>>(20)?, r.get::<_, i64>(21)?, r.get::<_, i64>(22)?, r.get::<_, i64>(23)?, r.get::<_, Option<String>>(24)?, r.get::<_, Option<String>>(25)?, r.get::<_, Option<i64>>(26)?, r.get::<_, Option<String>>(27)?
            )),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let Some((
        character_id,
        title,
        background_image_path,
        system_prompt,
        selected_scene_id,
        persona_id,
        persona_disabled,
        voice_autoplay,
        temperature,
        top_p,
        max_output_tokens,
        frequency_penalty,
        presence_penalty,
        top_k,
        memories_json,
        memory_embeddings_json,
        memory_summary,
        memory_summary_token_count,
        memory_tool_events_json,
        memory_status,
        memory_error,
        archived,
        created_at,
        updated_at,
        prompt_template_id,
        lorebook_ids_override_json,
        memory_progress_step,
        author_note,
    )) = row
    else {
        return Ok(None);
    };

    let advanced = if temperature.is_some()
        || top_p.is_some()
        || max_output_tokens.is_some()
        || frequency_penalty.is_some()
        || presence_penalty.is_some()
        || top_k.is_some()
    {
        Some(serde_json::json!({
            "temperature": temperature,
            "topP": top_p,
            "maxOutputTokens": max_output_tokens,
            "frequencyPenalty": frequency_penalty,
            "presencePenalty": presence_penalty,
            "topK": top_k,
        }))
    } else {
        None
    };

    let memories: JsonValue =
        serde_json::from_str(&memories_json).unwrap_or_else(|_| JsonValue::Array(vec![]));
    let memory_embeddings: JsonValue =
        serde_json::from_str(&memory_embeddings_json).unwrap_or_else(|_| JsonValue::Array(vec![]));
    let memory_tool_events: JsonValue =
        serde_json::from_str(&memory_tool_events_json).unwrap_or_else(|_| JsonValue::Array(vec![]));

    let session = serde_json::json!({
        "id": id,
        "characterId": character_id,
        "title": title,
        "backgroundImagePath": background_image_path,
        "systemPrompt": system_prompt,
        "selectedSceneId": selected_scene_id,
        "promptTemplateId": prompt_template_id,
        "lorebookIdsOverride": lorebook_ids_override_json.as_deref().and_then(|value| serde_json::from_str::<JsonValue>(value).ok()),
        "authorNote": author_note,
        "personaId": persona_id,
        "personaDisabled": persona_disabled.unwrap_or(0) != 0,
        "voiceAutoplay": voice_autoplay.map(|value| value != 0),
        "advancedModelSettings": advanced,
        "memories": memories,
        "memoryEmbeddings": memory_embeddings,
        "memorySummary": memory_summary.unwrap_or_default(),
        "memorySummaryTokenCount": memory_summary_token_count,
        "memoryToolEvents": memory_tool_events,
        "memoryStatus": memory_status,
        "memoryError": memory_error,
        "memoryProgressStep": memory_progress_step,
        "messages": [],
        "archived": archived != 0,
        "createdAt": created_at,
        "updatedAt": updated_at,
    });
    Ok(Some(session))
}

fn read_session(conn: &rusqlite::Connection, id: &str) -> Result<Option<JsonValue>, String> {
    let row = conn
        .query_row(
            "SELECT character_id, title, background_image_path, system_prompt, selected_scene_id, persona_id, persona_disabled, voice_autoplay, temperature, top_p, max_output_tokens, frequency_penalty, presence_penalty, top_k, memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events, memory_status, memory_error, archived, created_at, updated_at, prompt_template_id, lorebook_ids_override, memory_progress_step, author_note FROM sessions WHERE id = ?",
            params![id],
            |r| Ok((
                r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, Option<String>>(3)?, r.get::<_, Option<String>>(4)?, r.get::<_, Option<String>>(5)?, r.get::<_, Option<i64>>(6)?, r.get::<_, Option<i64>>(7)?, r.get::<_, Option<f64>>(8)?, r.get::<_, Option<f64>>(9)?, r.get::<_, Option<i64>>(10)?, r.get::<_, Option<f64>>(11)?, r.get::<_, Option<f64>>(12)?, r.get::<_, Option<i64>>(13)?, r.get::<_, String>(14)?, r.get::<_, String>(15)?, r.get::<_, Option<String>>(16)?, r.get::<_, i64>(17)?, r.get::<_, String>(18)?, r.get::<_, Option<String>>(19)?, r.get::<_, Option<String>>(20)?, r.get::<_, i64>(21)?, r.get::<_, i64>(22)?, r.get::<_, i64>(23)?, r.get::<_, Option<String>>(24)?, r.get::<_, Option<String>>(25)?, r.get::<_, Option<i64>>(26)?, r.get::<_, Option<String>>(27)?
            )),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let Some((
        character_id,
        title,
        background_image_path,
        system_prompt,
        selected_scene_id,
        persona_id,
        persona_disabled,
        voice_autoplay,
        temperature,
        top_p,
        max_output_tokens,
        frequency_penalty,
        presence_penalty,
        top_k,
        memories_json,
        memory_embeddings_json,
        memory_summary,
        memory_summary_token_count,
        memory_tool_events_json,
        memory_status,
        memory_error,
        archived,
        created_at,
        updated_at,
        prompt_template_id,
        lorebook_ids_override_json,
        memory_progress_step,
        author_note,
    )) = row
    else {
        return Ok(None);
    };

    // messages
    let mut mstmt = conn.prepare("SELECT id, role, content, created_at, scene_edited, prompt_tokens, completion_tokens, total_tokens, selected_variant_id, is_pinned, memory_refs, used_lorebook_entries, attachments, reasoning FROM messages WHERE session_id = ? ORDER BY created_at ASC").map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mrows = mstmt
        .query_map(params![id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, Option<i64>>(5)?,
                r.get::<_, Option<i64>>(6)?,
                r.get::<_, Option<i64>>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, i64>(9)?,
                r.get::<_, Option<String>>(10)?,
                r.get::<_, Option<String>>(11)?,
                r.get::<_, Option<String>>(12)?,
                r.get::<_, Option<String>>(13)?,
            ))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut messages: Vec<JsonValue> = Vec::new();
    for mr in mrows {
        let (
            mid,
            role,
            content,
            mcreated,
            scene_edited,
            p_tokens,
            c_tokens,
            t_tokens,
            selected_variant_id,
            is_pinned,
            memory_refs_json,
            used_lorebook_entries_json,
            attachments_json,
            reasoning,
        ) = mr.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let mut vstmt = conn.prepare("SELECT id, content, created_at, prompt_tokens, completion_tokens, total_tokens, reasoning FROM message_variants WHERE message_id = ? ORDER BY created_at ASC").map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let vrows = vstmt
            .query_map(params![&mid], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                    r.get::<_, Option<i64>>(4)?,
                    r.get::<_, Option<i64>>(5)?,
                    r.get::<_, Option<String>>(6)?,
                ))
            })
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let mut variants: Vec<JsonValue> = Vec::new();
        for vr in vrows {
            let (vid, vcontent, vcreated, vp, vc, vt, vreasoning) =
                vr.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            let mut vobj = JsonMap::new();
            vobj.insert("id".into(), JsonValue::String(vid));
            vobj.insert("content".into(), JsonValue::String(vcontent));
            vobj.insert("createdAt".into(), JsonValue::from(vcreated));
            if let Some(usage) = json_usage_summary(vp, vc, vt) {
                vobj.insert("usage".into(), usage);
            }
            if let Some(r) = vreasoning {
                vobj.insert("reasoning".into(), JsonValue::String(r));
            }
            variants.push(JsonValue::Object(vobj));
        }
        let mut mobj = JsonMap::new();
        mobj.insert("id".into(), JsonValue::String(mid));
        mobj.insert("role".into(), JsonValue::String(role));
        mobj.insert("content".into(), JsonValue::String(content));
        mobj.insert("createdAt".into(), JsonValue::from(mcreated));
        if scene_edited != 0 {
            mobj.insert("sceneEdited".into(), JsonValue::Bool(true));
        }
        if let Some(usage) = json_usage_summary(p_tokens, c_tokens, t_tokens) {
            mobj.insert("usage".into(), usage);
        }
        if !variants.is_empty() {
            mobj.insert("variants".into(), JsonValue::Array(variants));
        }
        if let Some(sel) = selected_variant_id {
            mobj.insert("selectedVariantId".into(), JsonValue::String(sel));
        }
        mobj.insert("isPinned".into(), JsonValue::Bool(is_pinned != 0));
        if let Some(refs_json) = memory_refs_json {
            if let Ok(parsed) = serde_json::from_str::<JsonValue>(&refs_json) {
                mobj.insert("memoryRefs".into(), parsed);
            }
        }
        if let Some(lorebook_json) = used_lorebook_entries_json {
            if let Ok(parsed) = serde_json::from_str::<JsonValue>(&lorebook_json) {
                mobj.insert("usedLorebookEntries".into(), parsed);
            }
        }
        // Parse and insert attachments
        if let Some(att_json) = attachments_json {
            if let Ok(parsed) = serde_json::from_str::<JsonValue>(&att_json) {
                mobj.insert("attachments".into(), parsed);
            }
        }
        // Add reasoning if present
        if let Some(r) = reasoning {
            mobj.insert("reasoning".into(), JsonValue::String(r));
        }
        messages.push(JsonValue::Object(mobj));
    }

    let advanced = if temperature.is_some()
        || top_p.is_some()
        || max_output_tokens.is_some()
        || frequency_penalty.is_some()
        || presence_penalty.is_some()
        || top_k.is_some()
    {
        Some(serde_json::json!({
            "temperature": temperature,
            "topP": top_p,
            "maxOutputTokens": max_output_tokens,
            "frequencyPenalty": frequency_penalty,
            "presencePenalty": presence_penalty,
            "topK": top_k,
        }))
    } else {
        None
    };

    // Parse memories JSON array
    let memories: JsonValue =
        serde_json::from_str(&memories_json).unwrap_or_else(|_| JsonValue::Array(vec![]));
    let memory_embeddings: JsonValue =
        serde_json::from_str(&memory_embeddings_json).unwrap_or_else(|_| JsonValue::Array(vec![]));
    let memory_tool_events: JsonValue =
        serde_json::from_str(&memory_tool_events_json).unwrap_or_else(|_| JsonValue::Array(vec![]));

    let session = serde_json::json!({
        "id": id,
        "characterId": character_id,
        "title": title,
        "backgroundImagePath": background_image_path,
        "systemPrompt": system_prompt,
        "selectedSceneId": selected_scene_id,
        "promptTemplateId": prompt_template_id,
        "lorebookIdsOverride": lorebook_ids_override_json.as_deref().and_then(|value| serde_json::from_str::<JsonValue>(value).ok()),
        "authorNote": author_note,
        "personaId": persona_id,
        "personaDisabled": persona_disabled.unwrap_or(0) != 0,
        "voiceAutoplay": voice_autoplay.map(|value| value != 0),
        "advancedModelSettings": advanced,
        "memories": memories,
        "memoryEmbeddings": memory_embeddings,
        "memorySummary": memory_summary.unwrap_or_default(),
        "memorySummaryTokenCount": memory_summary_token_count,
        "memoryToolEvents": memory_tool_events,
        "memoryStatus": memory_status,
        "memoryError": memory_error,
        "memoryProgressStep": memory_progress_step,
        "messages": messages,
        "archived": archived != 0,
        "createdAt": created_at,
        "updatedAt": updated_at,
    });
    Ok(Some(session))
}

fn reconcile_stale_dynamic_memory_session_state(
    app: &tauri::AppHandle,
    conn: &rusqlite::Connection,
    session_id: &str,
    session: &mut JsonValue,
) -> Result<(), String> {
    let is_processing = session
        .get("memoryStatus")
        .and_then(|value| value.as_str())
        .map(|value| value == "processing")
        .unwrap_or(false);
    if !is_processing {
        return Ok(());
    }

    let run_manager = app.state::<DynamicMemoryRunManager>().inner().clone();
    let run_key = format!("chat:{}", session_id);
    if run_manager.is_run_active(&run_key) {
        return Ok(());
    }

    conn.execute(
        "UPDATE sessions SET memory_status = 'idle', memory_error = NULL, updated_at = ?1 WHERE id = ?2",
        params![now_ms() as i64, session_id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    if let Some(map) = session.as_object_mut() {
        map.insert("memoryStatus".into(), JsonValue::String("idle".into()));
        map.insert("memoryError".into(), JsonValue::Null);
    }

    Ok(())
}

fn fetch_messages_page(
    conn: &rusqlite::Connection,
    session_id: &str,
    limit: i64,
    before_created_at: Option<i64>,
    before_id: Option<&str>,
) -> Result<Vec<JsonValue>, String> {
    let mut sql = String::from(
        "SELECT id, role, content, created_at, scene_edited, prompt_tokens, completion_tokens, total_tokens, selected_variant_id, is_pinned, memory_refs, used_lorebook_entries, attachments, reasoning FROM messages WHERE session_id = ?1",
    );

    let use_before = before_created_at.is_some() && before_id.is_some();
    if use_before {
        sql.push_str(" AND (created_at < ?2 OR (created_at = ?2 AND id < ?3))");
    }
    sql.push_str(" ORDER BY created_at DESC, id DESC LIMIT ");
    sql.push_str(&limit.to_string());

    let mut raw_messages = Vec::new();
    let mut message_ids: Vec<String> = Vec::new();
    {
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if use_before {
            let rows = stmt
                .query_map(
                    params![session_id, before_created_at.unwrap(), before_id.unwrap()],
                    |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?,
                            r.get::<_, i64>(3)?,
                            r.get::<_, i64>(4)?,
                            r.get::<_, Option<i64>>(5)?,
                            r.get::<_, Option<i64>>(6)?,
                            r.get::<_, Option<i64>>(7)?,
                            r.get::<_, Option<String>>(8)?,
                            r.get::<_, i64>(9)?,
                            r.get::<_, Option<String>>(10)?,
                            r.get::<_, Option<String>>(11)?,
                            r.get::<_, Option<String>>(12)?,
                            r.get::<_, Option<String>>(13)?,
                        ))
                    },
                )
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            for row in rows {
                let tuple =
                    row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                message_ids.push(tuple.0.clone());
                raw_messages.push(tuple);
            }
        } else {
            let rows = stmt
                .query_map(params![session_id], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, i64>(3)?,
                        r.get::<_, i64>(4)?,
                        r.get::<_, Option<i64>>(5)?,
                        r.get::<_, Option<i64>>(6)?,
                        r.get::<_, Option<i64>>(7)?,
                        r.get::<_, Option<String>>(8)?,
                        r.get::<_, i64>(9)?,
                        r.get::<_, Option<String>>(10)?,
                        r.get::<_, Option<String>>(11)?,
                        r.get::<_, Option<String>>(12)?,
                        r.get::<_, Option<String>>(13)?,
                    ))
                })
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            for row in rows {
                let tuple =
                    row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                message_ids.push(tuple.0.clone());
                raw_messages.push(tuple);
            }
        }
    }

    let mut variants_by_message: HashMap<String, Vec<JsonValue>> = HashMap::new();
    if !message_ids.is_empty() {
        let placeholders = message_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let vsql = format!(
            "SELECT message_id, id, content, created_at, prompt_tokens, completion_tokens, total_tokens, reasoning FROM message_variants WHERE message_id IN ({}) ORDER BY created_at ASC",
            placeholders
        );
        let mut vstmt = conn
            .prepare(&vsql)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let vrows = vstmt
            .query_map(rusqlite::params_from_iter(message_ids.iter()), |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, Option<i64>>(4)?,
                    r.get::<_, Option<i64>>(5)?,
                    r.get::<_, Option<i64>>(6)?,
                    r.get::<_, Option<String>>(7)?,
                ))
            })
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        for vr in vrows {
            let (message_id, vid, vcontent, vcreated, vp, vc, vt, vreasoning) =
                vr.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            variants_by_message.entry(message_id).or_default().push({
                let mut vobj = JsonMap::new();
                vobj.insert("id".into(), JsonValue::String(vid));
                if let Some(r) = vreasoning {
                    vobj.insert("reasoning".into(), JsonValue::String(r));
                }
                vobj.insert("content".into(), JsonValue::String(vcontent));
                vobj.insert("createdAt".into(), JsonValue::from(vcreated));
                if let Some(usage) = json_usage_summary(vp, vc, vt) {
                    vobj.insert("usage".into(), usage);
                }
                JsonValue::Object(vobj)
            });
        }
    }

    let mut out: Vec<JsonValue> = Vec::with_capacity(raw_messages.len());
    for (
        mid,
        role,
        content,
        mcreated,
        scene_edited,
        p_tokens,
        c_tokens,
        t_tokens,
        selected_variant_id,
        is_pinned,
        memory_refs_json,
        used_lorebook_entries_json,
        attachments_json,
        reasoning,
    ) in raw_messages
    {
        let mut mobj = JsonMap::new();
        mobj.insert("id".into(), JsonValue::String(mid.clone()));
        mobj.insert("role".into(), JsonValue::String(role));
        mobj.insert("content".into(), JsonValue::String(content));
        mobj.insert("createdAt".into(), JsonValue::from(mcreated));
        if scene_edited != 0 {
            mobj.insert("sceneEdited".into(), JsonValue::Bool(true));
        }
        if let Some(usage) = json_usage_summary(p_tokens, c_tokens, t_tokens) {
            mobj.insert("usage".into(), usage);
        }
        if let Some(variants) = variants_by_message.get(&mid) {
            if !variants.is_empty() {
                mobj.insert("variants".into(), JsonValue::Array(variants.clone()));
            }
        }
        if let Some(sel) = selected_variant_id {
            mobj.insert("selectedVariantId".into(), JsonValue::String(sel));
        }
        mobj.insert("isPinned".into(), JsonValue::Bool(is_pinned != 0));
        if let Some(refs_json) = memory_refs_json {
            if let Ok(parsed) = serde_json::from_str::<JsonValue>(&refs_json) {
                mobj.insert("memoryRefs".into(), parsed);
            }
        }
        if let Some(lorebook_json) = used_lorebook_entries_json {
            if let Ok(parsed) = serde_json::from_str::<JsonValue>(&lorebook_json) {
                mobj.insert("usedLorebookEntries".into(), parsed);
            }
        }
        if let Some(att_json) = attachments_json {
            if let Ok(parsed) = serde_json::from_str::<JsonValue>(&att_json) {
                mobj.insert("attachments".into(), parsed);
            }
        }
        if let Some(r) = reasoning {
            mobj.insert("reasoning".into(), JsonValue::String(r));
        }
        out.push(JsonValue::Object(mobj));
    }

    // We fetched DESC for paging; return ASC for rendering.
    out.reverse();
    Ok(out)
}

#[tauri::command]
pub fn sessions_list_ids(app: tauri::AppHandle) -> Result<String, String> {
    let conn = open_db(&app)?;
    let mut stmt = conn
        .prepare("SELECT id FROM sessions ORDER BY created_at ASC")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |r| Ok(r.get::<_, String>(0)?))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut ids: Vec<String> = Vec::new();
    for r in rows {
        ids.push(r.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?);
    }
    Ok(serde_json::to_string(&ids)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?)
}

/// List session previews without loading full message history.
///
/// This is intentionally designed to avoid `session_get` for every session when
/// rendering chat lists and history.
#[tauri::command]
pub fn sessions_list_previews(
    app: tauri::AppHandle,
    character_id: Option<String>,
    limit: Option<i64>,
) -> Result<String, String> {
    let conn = open_db(&app)?;

    let mut sql = String::from(
        r#"
        SELECT
          s.id,
          s.character_id,
          s.title,
          s.updated_at,
          s.archived,
          COALESCE(
            (
              SELECT substr(m.content, 1, 400)
              FROM messages m
              WHERE m.session_id = s.id
              ORDER BY m.created_at DESC
              LIMIT 1
            ),
            ''
          ) AS last_message,
          (
            SELECT COUNT(1)
            FROM messages m
            WHERE m.session_id = s.id
          ) AS message_count
        FROM sessions s
        WHERE (?1 IS NULL OR s.character_id = ?1)
        ORDER BY s.updated_at DESC
        "#,
    );
    if limit.is_some() {
        sql.push_str(" LIMIT ?2");
    }

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut previews: Vec<SessionPreview> = Vec::new();
    if limit.is_some() {
        let rows = stmt
            .query_map(params![character_id, limit], session_preview_from_row)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        for row in rows {
            previews
                .push(row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?);
        }
    } else {
        let rows = stmt
            .query_map(params![character_id], session_preview_from_row)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        for row in rows {
            previews
                .push(row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?);
        }
    }

    Ok(serde_json::to_string(&previews)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?)
}

#[tauri::command]
pub fn session_get(app: tauri::AppHandle, id: String) -> Result<Option<String>, String> {
    let conn = open_db(&app)?;
    let v = read_session(&conn, &id)?
        .map(|mut value| {
            reconcile_stale_dynamic_memory_session_state(&app, &conn, &id, &mut value)?;
            Ok::<_, String>(value)
        })
        .transpose()?;
    Ok(match v {
        Some(json) => Some(
            serde_json::to_string(&json)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
        ),
        None => None,
    })
}

#[tauri::command]
pub fn session_get_meta(app: tauri::AppHandle, id: String) -> Result<Option<String>, String> {
    let conn = open_db(&app)?;
    let v = read_session_meta(&conn, &id)?
        .map(|mut value| {
            reconcile_stale_dynamic_memory_session_state(&app, &conn, &id, &mut value)?;
            Ok::<_, String>(value)
        })
        .transpose()?;
    Ok(match v {
        Some(json) => Some(
            serde_json::to_string(&json)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
        ),
        None => None,
    })
}

#[tauri::command]
pub fn session_message_count(app: tauri::AppHandle, session_id: String) -> Result<i64, String> {
    let conn = open_db(&app)?;
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(1) FROM messages WHERE session_id = ?",
            params![session_id],
            |r| r.get(0),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(count)
}

pub fn session_conversation_count(
    app: tauri::AppHandle,
    session_id: String,
) -> Result<i64, String> {
    let conn = open_db(&app)?;
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(1) FROM messages WHERE session_id = ? AND (role = 'user' OR role = 'assistant')",
            params![session_id],
            |r| r.get(0),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(count)
}

pub fn session_get_meta_typed<T>(app: &tauri::AppHandle, id: &str) -> Result<Option<T>, String>
where
    T: serde::de::DeserializeOwned,
{
    let conn = open_db(app)?;
    read_session_meta(&conn, id)?
        .map(|mut value| {
            reconcile_stale_dynamic_memory_session_state(app, &conn, id, &mut value)?;
            Ok::<_, String>(value)
        })
        .transpose()?
        .map(|value| {
            serde_json::from_value(value)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
        })
        .transpose()
}

pub fn messages_list_typed<T>(
    app: &tauri::AppHandle,
    session_id: &str,
    limit: i64,
    before_created_at: Option<i64>,
    before_id: Option<&str>,
) -> Result<Vec<T>, String>
where
    T: serde::de::DeserializeOwned,
{
    let conn = open_db(app)?;
    serde_json::from_value(JsonValue::Array(fetch_messages_page(
        &conn,
        session_id,
        limit,
        before_created_at,
        before_id,
    )?))
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

pub fn messages_list_pinned_typed<T>(
    app: &tauri::AppHandle,
    session_id: &str,
) -> Result<Vec<T>, String>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_value(
        serde_json::to_value(messages_list_pinned_internal(app, session_id)?)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

pub fn session_upsert_meta_typed<T>(app: &tauri::AppHandle, session: &T) -> Result<(), String>
where
    T: serde::Serialize,
{
    let value = serde_json::to_value(session)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    upsert_session_meta_value(app, &value)
}

pub fn messages_upsert_batch_typed<T>(
    app: &tauri::AppHandle,
    session_id: &str,
    messages: &[T],
) -> Result<(), String>
where
    T: serde::Serialize,
{
    let value = serde_json::to_value(messages)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let Some(items) = value.as_array() else {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "messages must serialize to a JSON array",
        ));
    };
    upsert_messages_batch_value(app, session_id, items)
}

#[tauri::command]
pub fn messages_list(
    app: tauri::AppHandle,
    session_id: String,
    limit: i64,
    before_created_at: Option<i64>,
    before_id: Option<String>,
) -> Result<String, String> {
    let conn = open_db(&app)?;
    let messages = fetch_messages_page(
        &conn,
        &session_id,
        limit.max(0).min(500),
        before_created_at,
        before_id.as_deref(),
    )?;
    Ok(serde_json::to_string(&messages)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?)
}

#[tauri::command]
pub fn messages_list_pinned(app: tauri::AppHandle, session_id: String) -> Result<String, String> {
    let conn = open_db(&app)?;
    let mut stmt = conn
        .prepare("SELECT id FROM messages WHERE session_id = ? AND is_pinned = 1 ORDER BY created_at ASC, id ASC")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map(params![&session_id], |r| Ok(r.get::<_, String>(0)?))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut pinned_ids: Vec<String> = Vec::new();
    for row in rows {
        pinned_ids.push(row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?);
    }
    if pinned_ids.is_empty() {
        return Ok("[]".to_string());
    }

    let placeholders = pinned_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT id, role, content, created_at, scene_edited, prompt_tokens, completion_tokens, total_tokens, selected_variant_id, is_pinned, memory_refs, used_lorebook_entries, attachments, reasoning FROM messages WHERE session_id = ?1 AND id IN ({}) ORDER BY created_at ASC, id ASC",
        placeholders
    );
    let mut mstmt = conn
        .prepare(&sql)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut params_vec: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(1 + pinned_ids.len());
    params_vec.push(&session_id);
    for id in pinned_ids.iter() {
        params_vec.push(id);
    }

    let mrows = mstmt
        .query_map(params_vec.as_slice(), |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, Option<i64>>(5)?,
                r.get::<_, Option<i64>>(6)?,
                r.get::<_, Option<i64>>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, i64>(9)?,
                r.get::<_, Option<String>>(10)?,
                r.get::<_, Option<String>>(11)?,
                r.get::<_, Option<String>>(12)?,
                r.get::<_, Option<String>>(13)?,
            ))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut raw_messages = Vec::new();
    let mut message_ids: Vec<String> = Vec::new();
    for row in mrows {
        let tuple = row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        message_ids.push(tuple.0.clone());
        raw_messages.push(tuple);
    }

    let mut variants_by_message: HashMap<String, Vec<JsonValue>> = HashMap::new();
    if !message_ids.is_empty() {
        let placeholders = message_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let vsql = format!(
            "SELECT message_id, id, content, created_at, prompt_tokens, completion_tokens, total_tokens, reasoning FROM message_variants WHERE message_id IN ({}) ORDER BY created_at ASC",
            placeholders
        );
        let mut vstmt = conn
            .prepare(&vsql)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let vrows = vstmt
            .query_map(rusqlite::params_from_iter(message_ids.iter()), |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, Option<i64>>(4)?,
                    r.get::<_, Option<i64>>(5)?,
                    r.get::<_, Option<i64>>(6)?,
                    r.get::<_, Option<String>>(7)?,
                ))
            })
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        for vr in vrows {
            let (message_id, vid, vcontent, vcreated, vp, vc, vt, vreasoning) =
                vr.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            variants_by_message.entry(message_id).or_default().push({
                let mut vobj = JsonMap::new();
                vobj.insert("id".into(), JsonValue::String(vid));
                if let Some(r) = vreasoning {
                    vobj.insert("reasoning".into(), JsonValue::String(r));
                }
                vobj.insert("content".into(), JsonValue::String(vcontent));
                vobj.insert("createdAt".into(), JsonValue::from(vcreated));
                if let Some(usage) = json_usage_summary(vp, vc, vt) {
                    vobj.insert("usage".into(), usage);
                }
                JsonValue::Object(vobj)
            });
        }
    }

    let mut out: Vec<JsonValue> = Vec::with_capacity(raw_messages.len());
    for (
        mid,
        role,
        content,
        mcreated,
        scene_edited,
        p_tokens,
        c_tokens,
        t_tokens,
        selected_variant_id,
        is_pinned,
        memory_refs_json,
        used_lorebook_entries_json,
        attachments_json,
        reasoning,
    ) in raw_messages
    {
        let mut mobj = JsonMap::new();
        mobj.insert("id".into(), JsonValue::String(mid.clone()));
        mobj.insert("role".into(), JsonValue::String(role));
        mobj.insert("content".into(), JsonValue::String(content));
        mobj.insert("createdAt".into(), JsonValue::from(mcreated));
        if scene_edited != 0 {
            mobj.insert("sceneEdited".into(), JsonValue::Bool(true));
        }
        if let Some(usage) = json_usage_summary(p_tokens, c_tokens, t_tokens) {
            mobj.insert("usage".into(), usage);
        }
        if let Some(variants) = variants_by_message.get(&mid) {
            if !variants.is_empty() {
                mobj.insert("variants".into(), JsonValue::Array(variants.clone()));
            }
        }
        if let Some(sel) = selected_variant_id {
            mobj.insert("selectedVariantId".into(), JsonValue::String(sel));
        }
        mobj.insert("isPinned".into(), JsonValue::Bool(is_pinned != 0));
        if let Some(refs_json) = memory_refs_json {
            if let Ok(parsed) = serde_json::from_str::<JsonValue>(&refs_json) {
                mobj.insert("memoryRefs".into(), parsed);
            }
        }
        if let Some(lorebook_json) = used_lorebook_entries_json {
            if let Ok(parsed) = serde_json::from_str::<JsonValue>(&lorebook_json) {
                mobj.insert("usedLorebookEntries".into(), parsed);
            }
        }
        if let Some(att_json) = attachments_json {
            if let Ok(parsed) = serde_json::from_str::<JsonValue>(&att_json) {
                mobj.insert("attachments".into(), parsed);
            }
        }
        if let Some(r) = reasoning {
            mobj.insert("reasoning".into(), JsonValue::String(r));
        }
        out.push(JsonValue::Object(mobj));
    }

    Ok(serde_json::to_string(&out)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?)
}

#[tauri::command]
pub fn session_upsert_meta(app: tauri::AppHandle, session_json: String) -> Result<(), String> {
    let conn = open_db(&app)?;
    let s: JsonValue = serde_json::from_str(&session_json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let id = s
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "id is required".to_string())?
        .to_string();
    let character_id = s
        .get("characterId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "characterId is required".to_string())?;
    let title = s.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let background_image_path = s
        .get("backgroundImagePath")
        .and_then(|v| v.as_str())
        .map(|x| x.to_string());
    let system_prompt = s
        .get("systemPrompt")
        .and_then(|v| v.as_str())
        .map(|x| x.to_string());
    let selected_scene_id = s
        .get("selectedSceneId")
        .and_then(|v| v.as_str())
        .map(|x| x.to_string());
    let prompt_template_id = s
        .get("promptTemplateId")
        .and_then(|v| v.as_str())
        .map(|x| x.to_string());
    let lorebook_ids_override_json = resolve_lorebook_ids_override_json(&conn, &id, &s)?;
    let author_note = resolve_author_note(&conn, &id, &s)?;
    let persona_id = s
        .get("personaId")
        .and_then(|v| v.as_str())
        .map(|x| x.to_string());
    let persona_disabled = s
        .get("personaDisabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false) as i64;
    let voice_autoplay = s
        .get("voiceAutoplay")
        .and_then(|v| v.as_bool())
        .map(|value| if value { 1 } else { 0 });
    let archived = s.get("archived").and_then(|v| v.as_bool()).unwrap_or(false) as i64;
    let created_at = s
        .get("createdAt")
        .and_then(|v| v.as_i64())
        .unwrap_or(now_ms() as i64);
    let updated_at = now_ms() as i64;

    let memories_json = match s.get("memories") {
        Some(v) => serde_json::to_string(v).unwrap_or_else(|_| "[]".to_string()),
        None => "[]".to_string(),
    };
    let memory_summary = s
        .get("memorySummary")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let memory_summary_token_count = s
        .get("memorySummaryTokenCount")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let memory_tool_events_json = match s.get("memoryToolEvents") {
        Some(v) => serde_json::to_string(v).unwrap_or_else(|_| "[]".to_string()),
        None => "[]".to_string(),
    };
    let memory_embeddings_json = match s.get("memoryEmbeddings") {
        Some(v) => serde_json::to_string(v).unwrap_or_else(|_| "[]".to_string()),
        None => "[]".to_string(),
    };
    let memory_status = s
        .get("memoryStatus")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let memory_error = s
        .get("memoryError")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let memory_progress_step = s.get("memoryProgressStep").and_then(|v| v.as_i64());

    let adv = s.get("advancedModelSettings");
    let temperature = adv
        .and_then(|v| v.get("temperature"))
        .and_then(|v| v.as_f64());
    let top_p = adv.and_then(|v| v.get("topP")).and_then(|v| v.as_f64());
    let max_output_tokens = adv
        .and_then(|v| v.get("maxOutputTokens"))
        .and_then(|v| v.as_i64());
    let frequency_penalty = adv
        .and_then(|v| v.get("frequencyPenalty"))
        .and_then(|v| v.as_f64());
    let presence_penalty = adv
        .and_then(|v| v.get("presencePenalty"))
        .and_then(|v| v.as_f64());
    let top_k = adv.and_then(|v| v.get("topK")).and_then(|v| v.as_i64());

    conn.execute(
        r#"INSERT INTO sessions (id, character_id, title, background_image_path, system_prompt, selected_scene_id, prompt_template_id, lorebook_ids_override, author_note, persona_id, persona_disabled, voice_autoplay, temperature, top_p, max_output_tokens, frequency_penalty, presence_penalty, top_k, memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events, memory_status, memory_error, memory_progress_step, archived, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
              character_id=excluded.character_id,
              title=excluded.title,
              background_image_path=excluded.background_image_path,
              system_prompt=excluded.system_prompt,
              selected_scene_id=excluded.selected_scene_id,
              prompt_template_id=excluded.prompt_template_id,
              lorebook_ids_override=excluded.lorebook_ids_override,
              author_note=excluded.author_note,
              persona_id=excluded.persona_id,
              persona_disabled=excluded.persona_disabled,
              voice_autoplay=excluded.voice_autoplay,
              temperature=excluded.temperature,
              top_p=excluded.top_p,
              max_output_tokens=excluded.max_output_tokens,
              frequency_penalty=excluded.frequency_penalty,
              presence_penalty=excluded.presence_penalty,
              top_k=excluded.top_k,
              memories=excluded.memories,
              memory_embeddings=excluded.memory_embeddings,
              memory_summary=excluded.memory_summary,
              memory_summary_token_count=excluded.memory_summary_token_count,
              memory_tool_events=excluded.memory_tool_events,
              memory_status=excluded.memory_status,
              memory_error=excluded.memory_error,
              memory_progress_step=excluded.memory_progress_step,
              archived=excluded.archived,
              updated_at=excluded.updated_at"#,
        params![
            &id,
            character_id,
            title,
            background_image_path,
            system_prompt,
            selected_scene_id,
            prompt_template_id,
            lorebook_ids_override_json,
            author_note,
            persona_id,
            persona_disabled,
            voice_autoplay,
            temperature,
            top_p,
            max_output_tokens,
            frequency_penalty,
            presence_penalty,
            top_k,
            &memories_json,
            &memory_embeddings_json,
            memory_summary,
            memory_summary_token_count,
            &memory_tool_events_json,
            memory_status,
            memory_error,
            memory_progress_step,
            archived,
            created_at,
            updated_at
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    Ok(())
}

#[tauri::command]
pub fn messages_upsert_batch(
    app: tauri::AppHandle,
    session_id: String,
    messages_json: String,
) -> Result<(), String> {
    let mut conn = open_db(&app)?;
    let v: JsonValue = serde_json::from_str(&messages_json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let Some(msgs) = v.as_array() else {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "messages_json must be a JSON array",
        ));
    };

    log_info(
        &app,
        "messages_upsert_batch",
        format!(
            "Upserting {} messages for session {}",
            msgs.len(),
            session_id
        ),
    );

    let now = now_ms() as i64;
    let tx = conn.transaction().map_err(|e| {
        log_error(
            &app,
            "messages_upsert_batch",
            format!("Failed to begin transaction: {}", e),
        );
        e.to_string()
    })?;

    for m in msgs {
        let mid = m
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "message.id is required".to_string())?;
        let role = m.get("role").and_then(|v| v.as_str()).unwrap_or("user");
        let content = m.get("content").and_then(|v| v.as_str()).unwrap_or("");
        let mcreated = m.get("createdAt").and_then(|v| v.as_i64()).unwrap_or(now);
        let is_pinned = m.get("isPinned").and_then(|v| v.as_bool()).unwrap_or(false) as i64;
        let scene_edited = m
            .get("sceneEdited")
            .and_then(|v| v.as_bool())
            .unwrap_or(false) as i64;
        let usage = m.get("usage");
        let pt = usage
            .and_then(|u| u.get("promptTokens"))
            .and_then(|v| v.as_i64());
        let ct = usage
            .and_then(|u| u.get("completionTokens"))
            .and_then(|v| v.as_i64());
        let tt = usage
            .and_then(|u| u.get("totalTokens"))
            .and_then(|v| v.as_i64());
        let selected_variant_id = m
            .get("selectedVariantId")
            .and_then(|v| v.as_str())
            .map(|x| x.to_string());
        let memory_refs = m
            .get("memoryRefs")
            .cloned()
            .unwrap_or_else(|| JsonValue::Array(Vec::new()));
        let used_lorebook_entries = m
            .get("usedLorebookEntries")
            .cloned()
            .unwrap_or_else(|| JsonValue::Array(Vec::new()));
        let attachments = m
            .get("attachments")
            .cloned()
            .unwrap_or_else(|| JsonValue::Array(Vec::new()));
        let reasoning = m
            .get("reasoning")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        tx.execute(
            r#"INSERT INTO messages (id, session_id, role, content, created_at, scene_edited, prompt_tokens, completion_tokens, total_tokens, selected_variant_id, is_pinned, memory_refs, used_lorebook_entries, attachments, reasoning)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT(id) DO UPDATE SET
                 session_id=excluded.session_id,
                 role=excluded.role,
                 content=excluded.content,
                 created_at=excluded.created_at,
                 scene_edited=excluded.scene_edited,
                 prompt_tokens=excluded.prompt_tokens,
                 completion_tokens=excluded.completion_tokens,
                 total_tokens=excluded.total_tokens,
                 selected_variant_id=excluded.selected_variant_id,
                 is_pinned=excluded.is_pinned,
                 memory_refs=excluded.memory_refs,
                 used_lorebook_entries=excluded.used_lorebook_entries,
                 attachments=excluded.attachments,
                 reasoning=excluded.reasoning"#,
            params![
                &mid,
                &session_id,
                role,
                content,
                mcreated,
                scene_edited,
                pt,
                ct,
                tt,
                selected_variant_id,
                is_pinned,
                memory_refs.to_string(),
                used_lorebook_entries.to_string(),
                attachments.to_string(),
                reasoning
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        if m.get("variants").is_some() {
            tx.execute(
                "DELETE FROM message_variants WHERE message_id = ?",
                params![&mid],
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            if let Some(vars) = m.get("variants").and_then(|v| v.as_array()) {
                for v in vars {
                    let vid = v
                        .get("id")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                    let vcontent = v.get("content").and_then(|x| x.as_str()).unwrap_or("");
                    let vcreated = v.get("createdAt").and_then(|x| x.as_i64()).unwrap_or(now);
                    let u = v.get("usage");
                    let vp = u
                        .and_then(|u| u.get("promptTokens"))
                        .and_then(|v| v.as_i64());
                    let vc = u
                        .and_then(|u| u.get("completionTokens"))
                        .and_then(|v| v.as_i64());
                    let vt = u
                        .and_then(|u| u.get("totalTokens"))
                        .and_then(|v| v.as_i64());
                    let vreasoning = v
                        .get("reasoning")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string());
                    tx.execute(
                        "INSERT INTO message_variants (id, message_id, content, created_at, prompt_tokens, completion_tokens, total_tokens, reasoning) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                        params![vid, &mid, vcontent, vcreated, vp, vc, vt, vreasoning],
                    )
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                }
            }
        }
    }

    tx.execute(
        "UPDATE sessions SET updated_at = ? WHERE id = ?",
        params![now, &session_id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub fn message_delete(
    app: tauri::AppHandle,
    session_id: String,
    message_id: String,
) -> Result<(), String> {
    log_info(
        &app,
        "message_delete",
        format!(
            "Deleting message {} from session {}",
            message_id, session_id
        ),
    );
    let conn = open_db(&app)?;
    let now = now_ms() as i64;
    conn.execute(
        "DELETE FROM messages WHERE id = ? AND session_id = ?",
        params![&message_id, &session_id],
    )
    .map_err(|e| {
        log_error(
            &app,
            "message_delete",
            format!("Failed to delete message: {}", e),
        );
        e.to_string()
    })?;
    conn.execute(
        "UPDATE sessions SET updated_at = ? WHERE id = ?",
        params![now, &session_id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

#[tauri::command]
pub fn messages_delete_after(
    app: tauri::AppHandle,
    session_id: String,
    message_id: String,
) -> Result<(), String> {
    let mut conn = open_db(&app)?;
    let now = now_ms() as i64;
    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let ids: Vec<String> = {
        let mut stmt = tx
            .prepare("SELECT id FROM messages WHERE session_id = ? ORDER BY created_at ASC, id ASC")
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let rows = stmt
            .query_map(params![&session_id], |r| Ok(r.get::<_, String>(0)?))
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let mut ids: Vec<String> = Vec::new();
        for row in rows {
            ids.push(row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?);
        }
        ids
    };

    let Some(pos) = ids.iter().position(|id| id == &message_id) else {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Message not found in session",
        ));
    };

    let to_delete = &ids[(pos + 1)..];
    log_info(
        &app,
        "messages_delete_after",
        format!(
            "Rewinding session {} after message {} (deleting {} messages)",
            session_id,
            message_id,
            to_delete.len()
        ),
    );
    for id in to_delete {
        tx.execute(
            "DELETE FROM messages WHERE id = ? AND session_id = ?",
            params![id, &session_id],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    tx.execute(
        "UPDATE sessions SET updated_at = ? WHERE id = ?",
        params![now, &session_id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

#[tauri::command]
pub fn session_upsert(app: tauri::AppHandle, session_json: String) -> Result<(), String> {
    let mut conn = open_db(&app)?;
    let s: JsonValue = serde_json::from_str(&session_json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let id = s
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "id is required".to_string())?
        .to_string();
    let character_id = s
        .get("characterId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "characterId is required".to_string())?;
    let title = s.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let background_image_path = s
        .get("backgroundImagePath")
        .and_then(|v| v.as_str())
        .map(|x| x.to_string());
    let system_prompt = s
        .get("systemPrompt")
        .and_then(|v| v.as_str())
        .map(|x| x.to_string());
    let selected_scene_id = s
        .get("selectedSceneId")
        .and_then(|v| v.as_str())
        .map(|x| x.to_string());
    let prompt_template_id = s
        .get("promptTemplateId")
        .and_then(|v| v.as_str())
        .map(|x| x.to_string());
    let lorebook_ids_override_json = resolve_lorebook_ids_override_json(&conn, &id, &s)?;
    let author_note = resolve_author_note(&conn, &id, &s)?;
    let persona_id = s
        .get("personaId")
        .and_then(|v| v.as_str())
        .map(|x| x.to_string());
    let persona_disabled = s
        .get("personaDisabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false) as i64;
    let voice_autoplay = s
        .get("voiceAutoplay")
        .and_then(|v| v.as_bool())
        .map(|value| if value { 1 } else { 0 });
    let archived = s.get("archived").and_then(|v| v.as_bool()).unwrap_or(false) as i64;
    let created_at = s
        .get("createdAt")
        .and_then(|v| v.as_i64())
        .unwrap_or(now_ms() as i64);
    let updated_at = now_ms() as i64;

    // Handle memories - serialize to JSON string
    let memories_json = match s.get("memories") {
        Some(v) => serde_json::to_string(v).unwrap_or_else(|_| "[]".to_string()),
        None => "[]".to_string(),
    };
    /*let memory_embeddings_json = match s.get("memoryEmbeddings") {
        Some(v) => serde_json::to_string(v).unwrap_or_else(|_| "[]".to_string()),
        None => "[]".to_string(),
    };*/
    let memory_summary = s
        .get("memorySummary")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Debug log
    log_info(
        &app,
        "session_upsert",
        format!(
            "Saving session {}. Memory summary present: {}, value: {:?}",
            id,
            memory_summary.is_some(),
            memory_summary
        ),
    );
    let memory_summary_token_count = s
        .get("memorySummaryTokenCount")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let memory_tool_events_json = match s.get("memoryToolEvents") {
        Some(v) => serde_json::to_string(v).unwrap_or_else(|_| "[]".to_string()),
        None => "[]".to_string(),
    };
    let memory_embeddings_json = match s.get("memoryEmbeddings") {
        Some(v) => serde_json::to_string(v).unwrap_or_else(|_| "[]".to_string()),
        None => "[]".to_string(),
    };

    let adv = s.get("advancedModelSettings");
    let temperature = adv
        .and_then(|v| v.get("temperature"))
        .and_then(|v| v.as_f64());
    let top_p = adv.and_then(|v| v.get("topP")).and_then(|v| v.as_f64());
    let max_output_tokens = adv
        .and_then(|v| v.get("maxOutputTokens"))
        .and_then(|v| v.as_i64());
    let frequency_penalty = adv
        .and_then(|v| v.get("frequencyPenalty"))
        .and_then(|v| v.as_f64());
    let presence_penalty = adv
        .and_then(|v| v.get("presencePenalty"))
        .and_then(|v| v.as_f64());
    let top_k = adv.and_then(|v| v.get("topK")).and_then(|v| v.as_i64());

    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    tx.execute(
        r#"INSERT INTO sessions (id, character_id, title, background_image_path, system_prompt, selected_scene_id, prompt_template_id, lorebook_ids_override, author_note, persona_id, persona_disabled, voice_autoplay, temperature, top_p, max_output_tokens, frequency_penalty, presence_penalty, top_k, memories, memory_embeddings, memory_summary, memory_summary_token_count, memory_tool_events, archived, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
              character_id=excluded.character_id,
              title=excluded.title,
              background_image_path=excluded.background_image_path,
              system_prompt=excluded.system_prompt,
              selected_scene_id=excluded.selected_scene_id,
              prompt_template_id=excluded.prompt_template_id,
              lorebook_ids_override=excluded.lorebook_ids_override,
              author_note=excluded.author_note,
              persona_id=excluded.persona_id,
              persona_disabled=excluded.persona_disabled,
              voice_autoplay=excluded.voice_autoplay,
              temperature=excluded.temperature,
              top_p=excluded.top_p,
              max_output_tokens=excluded.max_output_tokens,
              frequency_penalty=excluded.frequency_penalty,
              presence_penalty=excluded.presence_penalty,
              top_k=excluded.top_k,
              memories=excluded.memories,
              memory_embeddings=excluded.memory_embeddings,
              memory_summary=excluded.memory_summary,
              memory_summary_token_count=excluded.memory_summary_token_count,
              memory_tool_events=excluded.memory_tool_events,
              archived=excluded.archived,
              updated_at=excluded.updated_at"#,
        params![&id, character_id, title, background_image_path, system_prompt, selected_scene_id, prompt_template_id, lorebook_ids_override_json, author_note, persona_id, persona_disabled, voice_autoplay, temperature, top_p, max_output_tokens, frequency_penalty, presence_penalty, top_k, &memories_json, &memory_embeddings_json, memory_summary, memory_summary_token_count, &memory_tool_events_json, archived, created_at, updated_at],
    ).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    if let Some(msgs) = s.get("messages").and_then(|v| v.as_array()) {
        for m in msgs {
            let mid = m
                .get("id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let role = m.get("role").and_then(|v| v.as_str()).unwrap_or("user");
            let content = m.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let mcreated = m
                .get("createdAt")
                .and_then(|v| v.as_i64())
                .unwrap_or(updated_at);
            let is_pinned = m.get("isPinned").and_then(|v| v.as_bool()).unwrap_or(false) as i64;
            let scene_edited = m
                .get("sceneEdited")
                .and_then(|v| v.as_bool())
                .unwrap_or(false) as i64;
            let usage = m.get("usage");
            let pt = usage
                .and_then(|u| u.get("promptTokens"))
                .and_then(|v| v.as_i64());
            let ct = usage
                .and_then(|u| u.get("completionTokens"))
                .and_then(|v| v.as_i64());
            let tt = usage
                .and_then(|u| u.get("totalTokens"))
                .and_then(|v| v.as_i64());
            let selected_variant_id = m
                .get("selectedVariantId")
                .and_then(|v| v.as_str())
                .map(|x| x.to_string());
            let memory_refs = m
                .get("memoryRefs")
                .cloned()
                .unwrap_or_else(|| JsonValue::Array(Vec::new()));
            let used_lorebook_entries = m
                .get("usedLorebookEntries")
                .cloned()
                .unwrap_or_else(|| JsonValue::Array(Vec::new()));
            let attachments = m
                .get("attachments")
                .cloned()
                .unwrap_or_else(|| JsonValue::Array(Vec::new()));
            let reasoning = m
                .get("reasoning")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            tx.execute(
                r#"INSERT INTO messages (id, session_id, role, content, created_at, scene_edited, prompt_tokens, completion_tokens, total_tokens, selected_variant_id, is_pinned, memory_refs, used_lorebook_entries, attachments, reasoning)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                   ON CONFLICT(id) DO UPDATE SET
                     session_id=excluded.session_id,
                     role=excluded.role,
                     content=excluded.content,
                     created_at=excluded.created_at,
                     scene_edited=excluded.scene_edited,
                     prompt_tokens=excluded.prompt_tokens,
                     completion_tokens=excluded.completion_tokens,
                     total_tokens=excluded.total_tokens,
                     selected_variant_id=excluded.selected_variant_id,
                     is_pinned=excluded.is_pinned,
                     memory_refs=excluded.memory_refs,
                     used_lorebook_entries=excluded.used_lorebook_entries,
                     attachments=excluded.attachments,
                     reasoning=excluded.reasoning"#,
                params![
                    &mid,
                    &id,
                    role,
                    content,
                    mcreated,
                    scene_edited,
                    pt,
                    ct,
                    tt,
                    selected_variant_id,
                    is_pinned,
                    memory_refs.to_string(),
                    used_lorebook_entries.to_string(),
                    attachments.to_string(),
                    reasoning
                ],
            ).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

            if m.get("variants").is_some() {
                tx.execute(
                    "DELETE FROM message_variants WHERE message_id = ?",
                    params![&mid],
                )
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                if let Some(vars) = m.get("variants").and_then(|v| v.as_array()) {
                    for v in vars {
                        let vid = v
                            .get("id")
                            .and_then(|x| x.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                        let vcontent = v.get("content").and_then(|x| x.as_str()).unwrap_or("");
                        let vcreated = v
                            .get("createdAt")
                            .and_then(|x| x.as_i64())
                            .unwrap_or(updated_at);
                        let u = v.get("usage");
                        let vp = u
                            .and_then(|u| u.get("promptTokens"))
                            .and_then(|v| v.as_i64());
                        let vc = u
                            .and_then(|u| u.get("completionTokens"))
                            .and_then(|v| v.as_i64());
                        let vt = u
                            .and_then(|u| u.get("totalTokens"))
                            .and_then(|v| v.as_i64());
                        let vreasoning = v
                            .get("reasoning")
                            .and_then(|x| x.as_str())
                            .map(|s| s.to_string());
                        tx.execute(
                            "INSERT INTO message_variants (id, message_id, content, created_at, prompt_tokens, completion_tokens, total_tokens, reasoning) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                            params![vid, &mid, vcontent, vcreated, vp, vc, vt, vreasoning],
                        )
                        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                    }
                }
            }
        }
    }
    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub fn session_delete(app: tauri::AppHandle, id: String) -> Result<(), String> {
    log_info(&app, "session_delete", format!("Deleting session {}", id));
    let conn = open_db(&app)?;
    conn.execute("DELETE FROM sessions WHERE id = ?", params![id])
        .map_err(|e| {
            log_error(
                &app,
                "session_delete",
                format!("Failed to delete session: {}", e),
            );
            e.to_string()
        })?;
    Ok(())
}

#[tauri::command]
pub fn session_archive(app: tauri::AppHandle, id: String, archived: bool) -> Result<(), String> {
    let conn = open_db(&app)?;
    let now = now_ms() as i64;
    conn.execute(
        "UPDATE sessions SET archived = ?, updated_at = ? WHERE id = ?",
        params![if archived { 1 } else { 0 }, now, id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

#[tauri::command]
pub fn session_update_title(
    app: tauri::AppHandle,
    id: String,
    title: String,
) -> Result<(), String> {
    let conn = open_db(&app)?;
    let now = now_ms() as i64;
    conn.execute(
        "UPDATE sessions SET title = ?, updated_at = ? WHERE id = ?",
        params![title, now, id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

#[tauri::command]
pub fn session_update_author_note(
    app: tauri::AppHandle,
    id: String,
    author_note: Option<String>,
) -> Result<(), String> {
    let conn = open_db(&app)?;
    let now = now_ms() as i64;
    let author_note = author_note
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    conn.execute(
        "UPDATE sessions SET author_note = ?, updated_at = ? WHERE id = ?",
        params![author_note, now, id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

#[tauri::command]
pub fn message_toggle_pin(
    app: tauri::AppHandle,
    session_id: String,
    message_id: String,
) -> Result<Option<String>, String> {
    let conn = open_db(&app)?;
    let current: Option<i64> = conn
        .query_row(
            "SELECT is_pinned FROM messages WHERE id = ?",
            params![&message_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    if let Some(is_pinned) = current {
        conn.execute(
            "UPDATE messages SET is_pinned = ? WHERE id = ?",
            params![if is_pinned == 0 { 1 } else { 0 }, &message_id],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if let Some(json) = read_session(&conn, &session_id)? {
            return Ok(Some(serde_json::to_string(&json).map_err(|e| {
                crate::utils::err_to_string(module_path!(), line!(), e)
            })?));
        }
        Ok(None)
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub fn message_toggle_pin_state(
    app: tauri::AppHandle,
    session_id: String,
    message_id: String,
) -> Result<Option<bool>, String> {
    let conn = open_db(&app)?;
    let now = now_ms() as i64;
    let current: Option<i64> = conn
        .query_row(
            "SELECT is_pinned FROM messages WHERE id = ? AND session_id = ?",
            params![&message_id, &session_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    if let Some(is_pinned) = current {
        let next = if is_pinned == 0 { 1 } else { 0 };
        conn.execute(
            "UPDATE messages SET is_pinned = ? WHERE id = ? AND session_id = ?",
            params![next, &message_id, &session_id],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        conn.execute(
            "UPDATE sessions SET updated_at = ? WHERE id = ?",
            params![now, &session_id],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        Ok(Some(next != 0))
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub async fn session_add_memory(
    app: tauri::AppHandle,
    session_id: String,
    memory: String,
    memory_category: Option<String>,
) -> Result<Option<String>, String> {
    log_info(
        &app,
        "session_add_memory",
        format!("Adding memory to session {}", session_id),
    );
    let conn = open_db(&app)?;

    // Read current memories
    let (current_memories_json, current_embeddings_json): (String, String) = conn
        .query_row(
            "SELECT memories, memory_embeddings FROM sessions WHERE id = ?",
            params![&session_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .unwrap_or_else(|| ("[]".to_string(), "[]".to_string()));

    let mut memories: Vec<String> =
        serde_json::from_str(&current_memories_json).unwrap_or_else(|_| vec![]);
    let mut memory_embeddings: Vec<JsonValue> =
        serde_json::from_str(&current_embeddings_json).unwrap_or_else(|_| vec![]);

    // Add new memory (clone so we can still use `memory` for the embedding)
    memories.push(memory.clone());

    // Compute embedding (best-effort)
    let embedding = match embedding::compute_embedding(app.clone(), memory.clone()).await {
        Ok(vec) => vec,
        Err(err) => {
            log_warn(
                &app,
                "session_add_memory",
                format!("embedding failed: {}", err),
            );
            Vec::new()
        }
    };

    // Count tokens (best-effort)
    let token_count = crate::embedding::tokenizer::count_tokens(&app, &memory).unwrap_or(0);
    let normalized_category = normalize_memory_category(memory_category)?;

    memory_embeddings.push(serde_json::json!({
        "id": uuid::Uuid::new_v4().to_string(),
        "text": memory.clone(),
        "embedding": embedding,
        "createdAt": now_ms() as i64,
        "tokenCount": token_count,
        "category": normalized_category,
    }));

    // Save back
    let new_memories_json = serde_json::to_string(&memories)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let new_embeddings_json = serde_json::to_string(&memory_embeddings)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let now = now_ms() as i64;

    conn.execute(
        "UPDATE sessions SET memories = ?, memory_embeddings = ?, updated_at = ? WHERE id = ?",
        params![new_memories_json, new_embeddings_json, now, &session_id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    if let Some(json) = read_session_meta(&conn, &session_id)? {
        return Ok(Some(serde_json::to_string(&json).map_err(|e| {
            crate::utils::err_to_string(module_path!(), line!(), e)
        })?));
    }
    Ok(None)
}

#[tauri::command]
pub fn session_remove_memory(
    app: tauri::AppHandle,
    session_id: String,
    memory_index: usize,
) -> Result<Option<String>, String> {
    let conn = open_db(&app)?;

    // Read current memories
    let (current_memories_json, current_embeddings_json): (String, String) = conn
        .query_row(
            "SELECT memories, memory_embeddings FROM sessions WHERE id = ?",
            params![&session_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .unwrap_or_else(|| ("[]".to_string(), "[]".to_string()));

    let mut memories: Vec<String> =
        serde_json::from_str(&current_memories_json).unwrap_or_else(|_| vec![]);
    let mut memory_embeddings: Vec<JsonValue> =
        serde_json::from_str(&current_embeddings_json).unwrap_or_else(|_| vec![]);

    // Remove memory at index
    if memory_index < memories.len() {
        memories.remove(memory_index);

        if memory_index < memory_embeddings.len() {
            memory_embeddings.remove(memory_index);
        }

        // Save back
        let new_memories_json = serde_json::to_string(&memories)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let new_embeddings_json = serde_json::to_string(&memory_embeddings)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let now = now_ms() as i64;

        conn.execute(
            "UPDATE sessions SET memories = ?, memory_embeddings = ?, updated_at = ? WHERE id = ?",
            params![new_memories_json, new_embeddings_json, now, &session_id],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    if let Some(json) = read_session_meta(&conn, &session_id)? {
        return Ok(Some(serde_json::to_string(&json).map_err(|e| {
            crate::utils::err_to_string(module_path!(), line!(), e)
        })?));
    }
    Ok(None)
}

#[tauri::command]
pub async fn session_update_memory(
    app: tauri::AppHandle,
    session_id: String,
    memory_index: usize,
    new_memory: String,
    new_category: Option<String>,
) -> Result<Option<String>, String> {
    let conn = open_db(&app)?;

    // Read current memories
    let (current_memories_json, current_embeddings_json): (String, String) = conn
        .query_row(
            "SELECT memories, memory_embeddings FROM sessions WHERE id = ?",
            params![&session_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .unwrap_or_else(|| ("[]".to_string(), "[]".to_string()));

    let mut memories: Vec<String> =
        serde_json::from_str(&current_memories_json).unwrap_or_else(|_| vec![]);
    let mut memory_embeddings: Vec<JsonValue> =
        serde_json::from_str(&current_embeddings_json).unwrap_or_else(|_| vec![]);

    // Update memory at index
    if memory_index < memories.len() {
        memories[memory_index] = new_memory.clone();
        let normalized_category = normalize_memory_category(new_category)?;

        // Recompute embedding
        let embedding = match embedding::compute_embedding(app.clone(), new_memory.clone()).await {
            Ok(vec) => vec,
            Err(err) => {
                log_error(
                    &app,
                    "session_update_memory",
                    format!("embedding failed: {}", err),
                );
                Vec::new()
            }
        };

        if memory_index < memory_embeddings.len() {
            if let Some(obj) = memory_embeddings
                .get_mut(memory_index)
                .and_then(|v| v.as_object_mut())
            {
                obj.insert(
                    "text".into(),
                    JsonValue::String(memories[memory_index].clone()),
                );
                obj.insert(
                    "embedding".into(),
                    JsonValue::Array(embedding.iter().map(|f| JsonValue::from(*f)).collect()),
                );
                match normalized_category.as_ref() {
                    Some(category) => {
                        obj.insert("category".into(), JsonValue::String(category.clone()));
                    }
                    None => {
                        obj.remove("category");
                    }
                }
            }
        } else {
            memory_embeddings.push(serde_json::json!({
                "id": uuid::Uuid::new_v4().to_string(),
                "text": memories[memory_index].clone(),
                "embedding": embedding,
                "createdAt": now_ms() as i64,
                "category": normalized_category,
            }));
        }

        // Save back
        let new_memories_json = serde_json::to_string(&memories)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let new_embeddings_json = serde_json::to_string(&memory_embeddings)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let now = now_ms() as i64;

        conn.execute(
            "UPDATE sessions SET memories = ?, memory_embeddings = ?, updated_at = ? WHERE id = ?",
            params![new_memories_json, new_embeddings_json, now, &session_id],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    if let Some(json) = read_session_meta(&conn, &session_id)? {
        return Ok(Some(serde_json::to_string(&json).map_err(|e| {
            crate::utils::err_to_string(module_path!(), line!(), e)
        })?));
    }
    Ok(None)
}

#[tauri::command]
pub fn session_toggle_memory_pin(
    app: tauri::AppHandle,
    session_id: String,
    memory_index: usize,
) -> Result<Option<String>, String> {
    let conn = open_db(&app)?;

    // Read current memory embeddings
    let current_embeddings_json: String = conn
        .query_row(
            "SELECT memory_embeddings FROM sessions WHERE id = ?",
            params![&session_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .unwrap_or_else(|| "[]".to_string());

    let mut memory_embeddings: Vec<JsonValue> =
        serde_json::from_str(&current_embeddings_json).unwrap_or_else(|_| vec![]);

    let now = now_ms() as i64;

    // Toggle pin status at index
    if memory_index < memory_embeddings.len() {
        if let Some(obj) = memory_embeddings
            .get_mut(memory_index)
            .and_then(|v| v.as_object_mut())
        {
            let current_pinned = obj
                .get("isPinned")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let next_pinned = !current_pinned;
            obj.insert("isPinned".into(), JsonValue::Bool(next_pinned));
            if next_pinned {
                obj.insert("isCold".into(), JsonValue::Bool(false));
                obj.insert("importanceScore".into(), JsonValue::from(1.0));
                obj.insert("lastAccessedAt".into(), JsonValue::from(now));
            }
        }

        // Save back
        let new_embeddings_json = serde_json::to_string(&memory_embeddings)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        conn.execute(
            "UPDATE sessions SET memory_embeddings = ?, updated_at = ? WHERE id = ?",
            params![new_embeddings_json, now, &session_id],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    if let Some(json) = read_session_meta(&conn, &session_id)? {
        return Ok(Some(serde_json::to_string(&json).map_err(|e| {
            crate::utils::err_to_string(module_path!(), line!(), e)
        })?));
    }
    Ok(None)
}

#[tauri::command]
pub fn session_set_memory_cold_state(
    app: tauri::AppHandle,
    session_id: String,
    memory_index: usize,
    is_cold: bool,
) -> Result<Option<String>, String> {
    let conn = open_db(&app)?;

    // Read current memories + embeddings so we can keep alignment.
    let (current_memories_json, current_embeddings_json): (String, String) = conn
        .query_row(
            "SELECT memories, memory_embeddings FROM sessions WHERE id = ?",
            params![&session_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .unwrap_or_else(|| ("[]".to_string(), "[]".to_string()));

    let memories: Vec<String> =
        serde_json::from_str(&current_memories_json).unwrap_or_else(|_| vec![]);
    let mut memory_embeddings: Vec<JsonValue> =
        serde_json::from_str(&current_embeddings_json).unwrap_or_else(|_| vec![]);

    if memory_index >= memories.len() {
        if let Some(json) = read_session_meta(&conn, &session_id)? {
            return Ok(Some(serde_json::to_string(&json).map_err(|e| {
                crate::utils::err_to_string(module_path!(), line!(), e)
            })?));
        }
        return Ok(None);
    }

    let now = now_ms() as i64;

    // Ensure embeddings vector is long enough; fill missing entries with placeholders.
    while memory_embeddings.len() <= memory_index {
        let idx = memory_embeddings.len();
        let text = memories.get(idx).cloned().unwrap_or_default();
        memory_embeddings.push(serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "text": text,
            "embedding": [],
            "createdAt": now,
            "tokenCount": 0,
        }));
    }

    if let Some(obj) = memory_embeddings
        .get_mut(memory_index)
        .and_then(|v| v.as_object_mut())
    {
        let is_pinned = obj
            .get("isPinned")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if is_pinned && is_cold {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Pinned memories cannot be moved to cold storage",
            ));
        }

        obj.insert("isCold".into(), JsonValue::Bool(is_cold));
        if is_cold {
            obj.insert("importanceScore".into(), JsonValue::from(0.0));
        } else {
            obj.insert("importanceScore".into(), JsonValue::from(1.0));
            obj.insert("lastAccessedAt".into(), JsonValue::from(now));
        }
    }

    let new_embeddings_json = serde_json::to_string(&memory_embeddings)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    conn.execute(
        "UPDATE sessions SET memory_embeddings = ?, updated_at = ? WHERE id = ?",
        params![new_embeddings_json, now, &session_id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    if let Some(json) = read_session_meta(&conn, &session_id)? {
        return Ok(Some(serde_json::to_string(&json).map_err(|e| {
            crate::utils::err_to_string(module_path!(), line!(), e)
        })?));
    }
    Ok(None)
}
