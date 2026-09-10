use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use serde_json::Value;

use crate::{
    LegacyBackupConversionNotice, LegacyBackupConversionNoticeKind, LegacyBackupDocumentKind,
    LegacyBackupPricingPlan,
};

const SESSION_LIMIT: usize = 10_000;
const MESSAGE_LIMIT: usize = 200_000;
const VARIANT_LIMIT: usize = 400_000;
const TEXT_LIMIT: usize = 1_000_000;
const JSON_LIMIT: usize = 16 * 1024 * 1024;

#[derive(Debug)]
pub struct LegacyBackupDirectSessionPlan {
    pub sessions: Vec<LegacyBackupDirectSession>,
    pub notices: Vec<LegacyBackupConversionNotice>,
    pub source: LegacyBackupPricingPlan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyBackupDirectSession {
    pub source_id: String,
    pub character_source_id: String,
    pub title: String,
    pub parent_session_source_id: Option<String>,
    pub branched_from_message_source_id: Option<String>,
    pub root_session_source_id: String,
    pub background_image_locator: Option<String>,
    pub deprecated_system_prompt: Option<String>,
    pub mode: String,
    pub selected_scene_source_id: Option<String>,
    pub author_note: Option<String>,
    pub persona_source_id: Option<String>,
    pub persona_disabled: bool,
    pub voice_autoplay: Option<bool>,
    pub prompt_source_id: Option<String>,
    pub lorebook_source_ids_override: Option<Vec<String>>,
    pub generation_settings: LegacyBackupSessionGenerationSettings,
    pub companion_state_json: Option<String>,
    pub memories_json: String,
    pub memory_embeddings_json: String,
    pub memory_summary: Option<String>,
    pub memory_summary_token_count: u64,
    pub memory_tool_events_json: String,
    pub memory_status: Option<String>,
    pub memory_error: Option<String>,
    pub memory_progress_step: Option<u64>,
    pub archived: bool,
    pub created_at: u64,
    pub updated_at: u64,
    pub messages: Vec<LegacyBackupDirectMessage>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyBackupSessionGenerationSettings {
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub max_output_tokens: Option<u64>,
    pub frequency_penalty: Option<f64>,
    pub presence_penalty: Option<f64>,
    pub top_k: Option<u64>,
    pub advanced_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyBackupDirectMessage {
    pub source_id: String,
    pub ordinal: u64,
    pub role: String,
    pub content: String,
    pub created_at: u64,
    pub effective_at: Option<u64>,
    pub visible_in_chat: bool,
    pub scene_edited: bool,
    pub usage: LegacyBackupMessageUsage,
    pub model_source_id: Option<String>,
    pub selected_variant_source_id: Option<String>,
    pub pinned: bool,
    pub memory_refs_json: String,
    pub used_lorebook_entries_json: String,
    pub attachments_json: String,
    pub reasoning: Option<String>,
    pub parent_message_source_id: Option<String>,
    pub variants: Vec<LegacyBackupDirectMessageVariant>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyBackupDirectMessageVariant {
    pub source_id: String,
    pub ordinal: u64,
    pub content: String,
    pub created_at: u64,
    pub usage: LegacyBackupMessageUsage,
    pub reasoning: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyBackupMessageUsage {
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub first_token_ms: Option<u64>,
    pub tokens_per_second: Option<f64>,
    pub mtp_stats_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LegacyBackupSessionError {
    #[error("legacy backup direct session document is malformed")]
    Malformed { field: String },
    #[error("legacy backup direct session document exceeds its record limit")]
    LimitExceeded,
    #[error("legacy backup direct session graph contains an orphaned link")]
    Orphan { field: String },
}

#[derive(Deserialize)]
struct SessionRow {
    id: String,
    character_id: String,
    title: String,
    parent_session_id: Option<String>,
    branched_from_message_id: Option<String>,
    root_session_id: Option<String>,
    background_image_path: Option<String>,
    system_prompt: Option<String>,
    mode: String,
    selected_scene_id: Option<String>,
    author_note: Option<String>,
    persona_id: Option<String>,
    persona_disabled: bool,
    voice_autoplay: Option<bool>,
    prompt_template_id: Option<String>,
    lorebook_ids_override: Option<String>,
    temperature: Option<f64>,
    top_p: Option<f64>,
    max_output_tokens: Option<i64>,
    frequency_penalty: Option<f64>,
    presence_penalty: Option<f64>,
    top_k: Option<i64>,
    advanced_model_settings: Option<String>,
    companion_state: Option<String>,
    memories: String,
    memory_embeddings: String,
    memory_summary: Option<String>,
    memory_summary_token_count: i64,
    memory_tool_events: String,
    memory_status: Option<String>,
    memory_error: Option<String>,
    memory_progress_step: Option<i64>,
    archived: bool,
    created_at: i64,
    updated_at: i64,
    #[serde(default)]
    messages: Vec<MessageRow>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct MessageRow {
    id: String,
    role: String,
    content: String,
    created_at: i64,
    effective_at: Option<i64>,
    visible_in_chat: bool,
    scene_edited: bool,
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    total_tokens: Option<i64>,
    first_token_ms: Option<i64>,
    tokens_per_second: Option<f64>,
    mtp_stats: Option<String>,
    model_id: Option<String>,
    selected_variant_id: Option<String>,
    is_pinned: bool,
    memory_refs: String,
    used_lorebook_entries: String,
    attachments: String,
    reasoning: Option<String>,
    parent_message_id: Option<String>,
    #[serde(default)]
    variants: Vec<VariantRow>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct VariantRow {
    id: String,
    content: String,
    created_at: i64,
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    total_tokens: Option<i64>,
    first_token_ms: Option<i64>,
    tokens_per_second: Option<f64>,
    mtp_stats: Option<String>,
    reasoning: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

pub fn plan_legacy_backup_direct_sessions(
    source: LegacyBackupPricingPlan,
) -> Result<LegacyBackupDirectSessionPlan, LegacyBackupSessionError> {
    let document = source
        .source
        .source
        .source
        .authored
        .configuration
        .source
        .documents
        .iter()
        .find(|document| document.kind == LegacyBackupDocumentKind::Sessions);
    let mut notices = source.notices.clone();
    let sessions = match document {
        Some(document) => {
            let rows: Vec<SessionRow> =
                serde_json::from_slice(&document.bytes).map_err(|_| malformed("$"))?;
            map_sessions(rows, &source, &mut notices)?
        }
        None => {
            notices.push(notice(LegacyBackupConversionNoticeKind::Absent, "$"));
            Vec::new()
        }
    };
    notices.sort();
    notices.dedup();
    Ok(LegacyBackupDirectSessionPlan {
        sessions,
        notices,
        source,
    })
}

fn map_sessions(
    rows: Vec<SessionRow>,
    source: &LegacyBackupPricingPlan,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<LegacyBackupDirectSession>, LegacyBackupSessionError> {
    if rows.len() > SESSION_LIMIT {
        return Err(LegacyBackupSessionError::LimitExceeded);
    }
    let authored = &source.source.source.source.authored;
    let character_ids = authored
        .characters
        .iter()
        .map(|value| value.id.to_string())
        .collect::<BTreeSet<_>>();
    let persona_ids = authored
        .personas
        .personas
        .iter()
        .map(|value| value.id.to_string())
        .collect::<BTreeSet<_>>();
    let prompt_ids = authored
        .configuration
        .prompts
        .prompts
        .iter()
        .map(|value| value.source_id.clone())
        .collect::<BTreeSet<_>>();
    let lorebook_ids = authored
        .lorebooks
        .lorebooks
        .iter()
        .map(|value| value.id.to_string())
        .collect::<BTreeSet<_>>();
    let model_ids = authored
        .configuration
        .provider_models
        .model_profiles
        .iter()
        .map(|value| value.id.to_string())
        .collect::<BTreeSet<_>>();
    let mut session_ids = BTreeSet::new();
    let mut message_ids = BTreeSet::new();
    let mut variant_ids = BTreeSet::new();
    let mut message_count = 0_usize;
    let mut variant_count = 0_usize;
    let mut sessions = Vec::with_capacity(rows.len());
    for (session_index, row) in rows.into_iter().enumerate() {
        let path = format!("[{session_index}]");
        report_extra(&path, &row.extra, notices);
        validate_identifier(&row.id, &format!("{path}.id"))?;
        if !session_ids.insert(row.id.clone()) {
            return Err(malformed(format!("{path}.id")));
        }
        if !contains_case_insensitive(&character_ids, &row.character_id) {
            return Err(orphan(format!("{path}.character_id")));
        }
        if row
            .persona_id
            .as_ref()
            .is_some_and(|id| !contains_case_insensitive(&persona_ids, id))
        {
            return Err(orphan(format!("{path}.persona_id")));
        }
        if row
            .prompt_template_id
            .as_ref()
            .is_some_and(|id| !prompt_ids.contains(id))
        {
            return Err(orphan(format!("{path}.prompt_template_id")));
        }
        let lorebook_override = optional_string_array(
            row.lorebook_ids_override.as_deref(),
            &format!("{path}.lorebook_ids_override"),
        )?;
        if lorebook_override.as_ref().is_some_and(|ids| {
            ids.iter()
                .any(|id| !contains_case_insensitive(&lorebook_ids, id))
        }) {
            return Err(orphan(format!("{path}.lorebook_ids_override")));
        }
        validate_session_settings(&row, &path)?;
        let created_at = timestamp(row.created_at, &format!("{path}.created_at"))?;
        let updated_at = timestamp(row.updated_at, &format!("{path}.updated_at"))?;
        if created_at > updated_at {
            return Err(malformed(format!("{path}.updated_at")));
        }
        for (field, raw, expected) in [
            (
                "advanced_model_settings",
                row.advanced_model_settings.as_deref(),
                JsonShape::Object,
            ),
            (
                "companion_state",
                row.companion_state.as_deref(),
                JsonShape::Object,
            ),
            ("memories", Some(row.memories.as_str()), JsonShape::Array),
            (
                "memory_embeddings",
                Some(row.memory_embeddings.as_str()),
                JsonShape::Array,
            ),
            (
                "memory_tool_events",
                Some(row.memory_tool_events.as_str()),
                JsonShape::Array,
            ),
        ] {
            if let Some(raw) = raw {
                validate_json(raw, expected, &format!("{path}.{field}"))?;
            }
        }
        let messages = map_messages(
            row.messages,
            &path,
            &model_ids,
            &mut message_ids,
            &mut variant_ids,
            notices,
        )?;
        message_count = message_count
            .checked_add(messages.len())
            .ok_or(LegacyBackupSessionError::LimitExceeded)?;
        variant_count = messages.iter().try_fold(variant_count, |count, message| {
            count
                .checked_add(message.variants.len())
                .ok_or(LegacyBackupSessionError::LimitExceeded)
        })?;
        if message_count > MESSAGE_LIMIT || variant_count > VARIANT_LIMIT {
            return Err(LegacyBackupSessionError::LimitExceeded);
        }
        let root_session_source_id = row
            .root_session_id
            .clone()
            .unwrap_or_else(|| row.id.clone());
        sessions.push(LegacyBackupDirectSession {
            source_id: row.id,
            character_source_id: row.character_id,
            title: row.title,
            parent_session_source_id: row.parent_session_id,
            branched_from_message_source_id: row.branched_from_message_id,
            root_session_source_id,
            background_image_locator: row.background_image_path,
            deprecated_system_prompt: row.system_prompt,
            mode: row.mode,
            selected_scene_source_id: row.selected_scene_id,
            author_note: row.author_note,
            persona_source_id: row.persona_id,
            persona_disabled: row.persona_disabled,
            voice_autoplay: row.voice_autoplay,
            prompt_source_id: row.prompt_template_id,
            lorebook_source_ids_override: lorebook_override,
            generation_settings: LegacyBackupSessionGenerationSettings {
                temperature: row.temperature,
                top_p: row.top_p,
                max_output_tokens: optional_count(
                    row.max_output_tokens,
                    &format!("{path}.max_output_tokens"),
                )?,
                frequency_penalty: row.frequency_penalty,
                presence_penalty: row.presence_penalty,
                top_k: optional_count(row.top_k, &format!("{path}.top_k"))?,
                advanced_json: row.advanced_model_settings,
            },
            companion_state_json: row.companion_state,
            memories_json: row.memories,
            memory_embeddings_json: row.memory_embeddings,
            memory_summary: row.memory_summary,
            memory_summary_token_count: count(
                row.memory_summary_token_count,
                &format!("{path}.memory_summary_token_count"),
            )?,
            memory_tool_events_json: row.memory_tool_events,
            memory_status: row.memory_status,
            memory_error: row.memory_error,
            memory_progress_step: optional_count(
                row.memory_progress_step,
                &format!("{path}.memory_progress_step"),
            )?,
            archived: row.archived,
            created_at,
            updated_at,
            messages,
        });
    }
    validate_session_graph(&sessions)?;
    validate_authored_session_links(&sessions, authored)?;
    if !sessions.is_empty() {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Lossy,
            "[].currentGenerationAttempts",
        ));
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Lossy,
            "[].currentProtectedSnapshots",
        ));
        if sessions
            .iter()
            .any(|session| session.parent_session_source_id.is_some())
        {
            notices.push(notice(
                LegacyBackupConversionNoticeKind::Lossy,
                "[].currentSelectedBranch",
            ));
        }
    }
    Ok(sessions)
}

fn map_messages(
    rows: Vec<MessageRow>,
    session_path: &str,
    model_ids: &BTreeSet<String>,
    all_message_ids: &mut BTreeSet<String>,
    all_variant_ids: &mut BTreeSet<String>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<LegacyBackupDirectMessage>, LegacyBackupSessionError> {
    let local_ids = rows
        .iter()
        .map(|row| row.id.clone())
        .collect::<BTreeSet<_>>();
    if local_ids.len() != rows.len() {
        return Err(malformed(format!("{session_path}.messages[].id")));
    }
    let mut messages = Vec::with_capacity(rows.len());
    let mut tied_timestamps = BTreeSet::new();
    for (index, row) in rows.into_iter().enumerate() {
        let path = format!("{session_path}.messages[{index}]");
        report_extra(&path, &row.extra, notices);
        validate_identifier(&row.id, &format!("{path}.id"))?;
        if !all_message_ids.insert(row.id.clone()) {
            return Err(malformed(format!("{path}.id")));
        }
        if !matches!(row.role.as_str(), "user" | "assistant" | "system" | "scene") {
            return Err(malformed(format!("{path}.role")));
        }
        if row
            .parent_message_id
            .as_ref()
            .is_some_and(|id| !local_ids.contains(id))
        {
            return Err(orphan(format!("{path}.parent_message_id")));
        }
        if row
            .model_id
            .as_ref()
            .is_some_and(|id| !contains_case_insensitive(model_ids, id))
        {
            return Err(orphan(format!("{path}.model_id")));
        }
        validate_text(&row.content, &format!("{path}.content"))?;
        for (field, raw) in [
            ("memory_refs", row.memory_refs.as_str()),
            ("used_lorebook_entries", row.used_lorebook_entries.as_str()),
            ("attachments", row.attachments.as_str()),
        ] {
            validate_json(raw, JsonShape::Array, &format!("{path}.{field}"))?;
        }
        if let Some(raw) = &row.mtp_stats {
            validate_mtp_stats(raw, &format!("{path}.mtp_stats"))?;
        }
        let created_at = timestamp(row.created_at, &format!("{path}.created_at"))?;
        if !tied_timestamps.insert(created_at) {
            notices.push(notice(
                LegacyBackupConversionNoticeKind::Lossy,
                &format!("{session_path}.messages[].created_at_order"),
            ));
        }
        let variants = map_variants(row.variants, &path, all_variant_ids, notices)?;
        if row
            .selected_variant_id
            .as_ref()
            .is_some_and(|id| !variants.iter().any(|variant| variant.source_id == *id))
        {
            return Err(orphan(format!("{path}.selected_variant_id")));
        }
        messages.push(LegacyBackupDirectMessage {
            source_id: row.id,
            ordinal: u64::try_from(index).map_err(|_| LegacyBackupSessionError::LimitExceeded)?,
            role: row.role,
            content: row.content,
            created_at,
            effective_at: row
                .effective_at
                .map(|value| timestamp(value, &format!("{path}.effective_at")))
                .transpose()?,
            visible_in_chat: row.visible_in_chat,
            scene_edited: row.scene_edited,
            usage: usage(
                row.prompt_tokens,
                row.completion_tokens,
                row.total_tokens,
                row.first_token_ms,
                row.tokens_per_second,
                row.mtp_stats,
                &path,
            )?,
            model_source_id: row.model_id,
            selected_variant_source_id: row.selected_variant_id,
            pinned: row.is_pinned,
            memory_refs_json: row.memory_refs,
            used_lorebook_entries_json: row.used_lorebook_entries,
            attachments_json: row.attachments,
            reasoning: row.reasoning,
            parent_message_source_id: row.parent_message_id,
            variants,
        });
    }
    validate_message_cycles(&messages, session_path)?;
    Ok(messages)
}

fn map_variants(
    rows: Vec<VariantRow>,
    message_path: &str,
    all_variant_ids: &mut BTreeSet<String>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<LegacyBackupDirectMessageVariant>, LegacyBackupSessionError> {
    if rows.len() > 1 {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Lossy,
            &format!("{message_path}.variants[].source_order"),
        ));
    }
    rows.into_iter()
        .enumerate()
        .map(|(index, row)| {
            let path = format!("{message_path}.variants[{index}]");
            report_extra(&path, &row.extra, notices);
            validate_identifier(&row.id, &format!("{path}.id"))?;
            validate_text(&row.content, &format!("{path}.content"))?;
            if !all_variant_ids.insert(row.id.clone()) {
                return Err(malformed(format!("{path}.id")));
            }
            if let Some(raw) = &row.mtp_stats {
                validate_mtp_stats(raw, &format!("{path}.mtp_stats"))?;
            }
            Ok(LegacyBackupDirectMessageVariant {
                source_id: row.id,
                ordinal: u64::try_from(index)
                    .map_err(|_| LegacyBackupSessionError::LimitExceeded)?,
                content: row.content,
                created_at: timestamp(row.created_at, &format!("{path}.created_at"))?,
                usage: usage(
                    row.prompt_tokens,
                    row.completion_tokens,
                    row.total_tokens,
                    row.first_token_ms,
                    row.tokens_per_second,
                    row.mtp_stats,
                    &path,
                )?,
                reasoning: row.reasoning,
            })
        })
        .collect()
}

fn usage(
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    total_tokens: Option<i64>,
    first_token_ms: Option<i64>,
    tokens_per_second: Option<f64>,
    mtp_stats_json: Option<String>,
    path: &str,
) -> Result<LegacyBackupMessageUsage, LegacyBackupSessionError> {
    if tokens_per_second.is_some_and(|value| !value.is_finite() || value < 0.0) {
        return Err(malformed(format!("{path}.tokens_per_second")));
    }
    Ok(LegacyBackupMessageUsage {
        prompt_tokens: optional_count(prompt_tokens, &format!("{path}.prompt_tokens"))?,
        completion_tokens: optional_count(completion_tokens, &format!("{path}.completion_tokens"))?,
        total_tokens: optional_count(total_tokens, &format!("{path}.total_tokens"))?,
        first_token_ms: optional_count(first_token_ms, &format!("{path}.first_token_ms"))?,
        tokens_per_second,
        mtp_stats_json,
    })
}

fn validate_session_graph(
    sessions: &[LegacyBackupDirectSession],
) -> Result<(), LegacyBackupSessionError> {
    let by_id = sessions
        .iter()
        .map(|session| (session.source_id.as_str(), session))
        .collect::<BTreeMap<_, _>>();
    for session in sessions {
        let root = by_id
            .get(session.root_session_source_id.as_str())
            .ok_or_else(|| orphan("[].root_session_id"))?;
        if root.parent_session_source_id.is_some()
            || root.root_session_source_id != root.source_id
            || root.character_source_id != session.character_source_id
        {
            return Err(orphan("[].root_session_id"));
        }
        if let Some(parent_id) = &session.parent_session_source_id {
            let parent = by_id
                .get(parent_id.as_str())
                .ok_or_else(|| orphan("[].parent_session_id"))?;
            if parent.root_session_source_id != session.root_session_source_id
                || parent.character_source_id != session.character_source_id
            {
                return Err(orphan("[].parent_session_id"));
            }
        } else if session.source_id != session.root_session_source_id {
            return Err(orphan("[].root_session_id"));
        }
        if session
            .branched_from_message_source_id
            .as_ref()
            .is_some_and(|id| {
                !session
                    .messages
                    .iter()
                    .any(|message| message.source_id == *id)
            })
        {
            return Err(orphan("[].branched_from_message_id"));
        }
        let mut cursor = session.parent_session_source_id.as_deref();
        let mut visited = BTreeSet::new();
        while let Some(id) = cursor {
            if !visited.insert(id) || id == session.source_id {
                return Err(malformed("[].parent_session_id"));
            }
            cursor = by_id
                .get(id)
                .ok_or_else(|| orphan("[].parent_session_id"))?
                .parent_session_source_id
                .as_deref();
        }
    }
    Ok(())
}

fn validate_authored_session_links(
    sessions: &[LegacyBackupDirectSession],
    authored: &crate::LegacyBackupAuthoredPlan,
) -> Result<(), LegacyBackupSessionError> {
    for session in sessions {
        let character = authored
            .characters
            .iter()
            .find(|candidate| {
                candidate
                    .id
                    .to_string()
                    .eq_ignore_ascii_case(&session.character_source_id)
            })
            .ok_or_else(|| orphan("[].character_id"))?;
        if session.selected_scene_source_id.as_ref().is_some_and(|id| {
            !character
                .scenes
                .iter()
                .any(|scene| scene.id.to_string().eq_ignore_ascii_case(id))
        }) {
            return Err(orphan("[].selected_scene_id"));
        }
    }
    Ok(())
}

fn validate_session_settings(row: &SessionRow, path: &str) -> Result<(), LegacyBackupSessionError> {
    validate_identifier(&row.character_id, &format!("{path}.character_id"))?;
    validate_text(&row.title, &format!("{path}.title"))?;
    if row.mode.trim().is_empty() || row.mode.chars().any(char::is_control) {
        return Err(malformed(format!("{path}.mode")));
    }
    for (field, value, range) in [
        ("temperature", row.temperature, Some((0.0, 2.0))),
        ("top_p", row.top_p, Some((0.0, 1.0))),
        (
            "frequency_penalty",
            row.frequency_penalty,
            Some((-2.0, 2.0)),
        ),
        ("presence_penalty", row.presence_penalty, Some((-2.0, 2.0))),
    ] {
        if value.is_some_and(|value| {
            !value.is_finite()
                || range.is_some_and(|(minimum, maximum)| !(minimum..=maximum).contains(&value))
        }) {
            return Err(malformed(format!("{path}.{field}")));
        }
    }
    if row.max_output_tokens.is_some_and(|value| value <= 0)
        || row.top_k.is_some_and(|value| !(1..=500).contains(&value))
        || row.memory_summary_token_count < 0
        || row.memory_progress_step.is_some_and(|value| value < 0)
    {
        return Err(malformed(format!("{path}.generation_or_memory_settings")));
    }
    Ok(())
}

fn validate_message_cycles(
    messages: &[LegacyBackupDirectMessage],
    path: &str,
) -> Result<(), LegacyBackupSessionError> {
    let parents = messages
        .iter()
        .map(|message| {
            (
                message.source_id.as_str(),
                message.parent_message_source_id.as_deref(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for message in messages {
        let mut cursor = message.parent_message_source_id.as_deref();
        let mut visited = BTreeSet::new();
        while let Some(id) = cursor {
            if id == message.source_id || !visited.insert(id) {
                return Err(malformed(format!("{path}.messages[].parent_message_id")));
            }
            cursor = parents.get(id).copied().flatten();
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum JsonShape {
    Array,
    Object,
}

fn validate_json(raw: &str, shape: JsonShape, field: &str) -> Result<(), LegacyBackupSessionError> {
    if raw.len() > JSON_LIMIT {
        return Err(LegacyBackupSessionError::LimitExceeded);
    }
    let value: Value = serde_json::from_str(raw).map_err(|_| malformed(field))?;
    if !matches!(
        (shape, value),
        (JsonShape::Array, Value::Array(_)) | (JsonShape::Object, Value::Object(_))
    ) {
        return Err(malformed(field));
    }
    Ok(())
}

fn validate_mtp_stats(raw: &str, field: &str) -> Result<(), LegacyBackupSessionError> {
    validate_json(raw, JsonShape::Object, field)?;
    let value: Value = serde_json::from_str(raw).map_err(|_| malformed(field))?;
    let object = value.as_object().ok_or_else(|| malformed(field))?;
    for key in [
        "draftTokens",
        "finalDraftTokens",
        "adaptationCount",
        "rounds",
        "drafted",
        "accepted",
    ] {
        if object
            .get(key)
            .is_some_and(|value| !value.is_null() && value.as_u64().is_none())
        {
            return Err(malformed(format!("{field}.{key}")));
        }
    }
    for key in ["tokensPerRound", "draftAcceptance"] {
        if object.get(key).is_some_and(|value| {
            !value.is_null() && !value.as_f64().is_some_and(|value| value >= 0.0)
        }) {
            return Err(malformed(format!("{field}.{key}")));
        }
    }
    Ok(())
}

fn optional_string_array(
    raw: Option<&str>,
    field: &str,
) -> Result<Option<Vec<String>>, LegacyBackupSessionError> {
    raw.map(|raw| {
        if raw.len() > JSON_LIMIT {
            return Err(LegacyBackupSessionError::LimitExceeded);
        }
        let values: Vec<String> = serde_json::from_str(raw).map_err(|_| malformed(field))?;
        if values.iter().any(|value| value.trim().is_empty())
            || values.iter().collect::<BTreeSet<_>>().len() != values.len()
        {
            return Err(malformed(field));
        }
        Ok(values)
    })
    .transpose()
}

fn contains_case_insensitive(values: &BTreeSet<String>, expected: &str) -> bool {
    values
        .iter()
        .any(|value| value.eq_ignore_ascii_case(expected))
}

fn validate_identifier(value: &str, field: &str) -> Result<(), LegacyBackupSessionError> {
    if value.trim().is_empty()
        || value.trim() != value
        || value.chars().count() > 1_024
        || value.chars().any(char::is_control)
    {
        Err(malformed(field))
    } else {
        Ok(())
    }
}

fn validate_text(value: &str, field: &str) -> Result<(), LegacyBackupSessionError> {
    if value.chars().count() > TEXT_LIMIT || value.contains('\0') {
        Err(malformed(field))
    } else {
        Ok(())
    }
}

fn timestamp(value: i64, field: &str) -> Result<u64, LegacyBackupSessionError> {
    u64::try_from(value).map_err(|_| malformed(field))
}

fn count(value: i64, field: &str) -> Result<u64, LegacyBackupSessionError> {
    u64::try_from(value).map_err(|_| malformed(field))
}

fn optional_count(
    value: Option<i64>,
    field: &str,
) -> Result<Option<u64>, LegacyBackupSessionError> {
    value.map(|value| count(value, field)).transpose()
}

fn report_extra(
    path: &str,
    extra: &BTreeMap<String, Value>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) {
    for field in extra.keys() {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Unsupported,
            &format!("{path}.{field}"),
        ));
    }
}

fn notice(kind: LegacyBackupConversionNoticeKind, field: &str) -> LegacyBackupConversionNotice {
    LegacyBackupConversionNotice {
        kind,
        document: LegacyBackupDocumentKind::Sessions,
        field: field.to_owned(),
    }
}

fn malformed(field: impl Into<String>) -> LegacyBackupSessionError {
    LegacyBackupSessionError::Malformed {
        field: field.into(),
    }
}

fn orphan(field: impl Into<String>) -> LegacyBackupSessionError {
    LegacyBackupSessionError::Orphan {
        field: field.into(),
    }
}

#[cfg(test)]
mod tests {
    use lettuce_types::ContentHash;
    use serde_json::{Value, json};
    use zeroize::Zeroizing;

    use super::*;
    use crate::{
        LegacyBackupDocument, LegacyBackupInventory, plan_legacy_backup_asr,
        plan_legacy_backup_authored, plan_legacy_backup_authored_media,
        plan_legacy_backup_configuration, plan_legacy_backup_pricing, plan_legacy_backup_usage,
    };

    fn id(value: u128) -> String {
        uuid::Uuid::from_u128(value).to_string()
    }

    fn document(kind: LegacyBackupDocumentKind, value: Value) -> LegacyBackupDocument {
        LegacyBackupDocument {
            kind,
            bytes: Zeroizing::new(serde_json::to_vec(&value).expect("fixture document")),
        }
    }

    fn source(session_rows: Value, character_id: &str) -> LegacyBackupPricingPlan {
        let inventory = LegacyBackupInventory {
            version: 1,
            created_at: 1,
            app_version: "legacy".into(),
            source_hash: ContentHash::parse("77".repeat(32)).expect("source hash"),
            documents: vec![
                document(
                    LegacyBackupDocumentKind::Characters,
                    json!([{
                        "id": character_id,
                        "name": "Mira",
                        "created_at": 1,
                        "updated_at": 1
                    }]),
                ),
                document(LegacyBackupDocumentKind::Sessions, session_rows),
            ],
            media: Vec::new(),
        };
        let configuration =
            plan_legacy_backup_configuration(inventory).expect("configuration plan");
        let authored = plan_legacy_backup_authored(configuration).expect("authored plan");
        let media = plan_legacy_backup_authored_media(authored).expect("media plan");
        let asr = plan_legacy_backup_asr(media).expect("ASR plan");
        let usage = plan_legacy_backup_usage(asr).expect("usage plan");
        plan_legacy_backup_pricing(usage).expect("pricing plan")
    }

    fn message(message_id: &str, parent_id: Option<&str>, variant_id: Option<&str>) -> Value {
        json!({
            "id": message_id,
            "role": "assistant",
            "content": "Hello",
            "created_at": 10,
            "effective_at": 11,
            "visible_in_chat": true,
            "scene_edited": false,
            "prompt_tokens": 4,
            "completion_tokens": 2,
            "total_tokens": 6,
            "first_token_ms": 20,
            "tokens_per_second": 12.5,
            "mtp_stats": "{\"draftTokens\":2}",
            "model_id": null,
            "selected_variant_id": variant_id,
            "is_pinned": true,
            "memory_refs": "[\"memory-1\"]",
            "used_lorebook_entries": "[]",
            "attachments": "[{\"id\":\"image-1\",\"storagePath\":\"photo.png\"}]",
            "reasoning": "Private reasoning",
            "parent_message_id": parent_id,
            "variants": variant_id.map(|variant_id| vec![json!({
                "id": variant_id,
                "content": "Alternative",
                "created_at": 12,
                "prompt_tokens": 4,
                "completion_tokens": 3,
                "total_tokens": 7,
                "first_token_ms": 22,
                "tokens_per_second": 10.0,
                "mtp_stats": null,
                "reasoning": "Alternative reasoning"
            })]).unwrap_or_default()
        })
    }

    fn session(
        session_id: &str,
        character_id: &str,
        parent_id: Option<&str>,
        root_id: &str,
        branched_message_id: Option<&str>,
        messages: Vec<Value>,
    ) -> Value {
        json!({
            "id": session_id,
            "character_id": character_id,
            "title": "Conversation",
            "parent_session_id": parent_id,
            "branched_from_message_id": branched_message_id,
            "root_session_id": root_id,
            "background_image_path": null,
            "system_prompt": "Legacy prompt",
            "mode": "roleplay",
            "selected_scene_id": null,
            "author_note": "Keep the tone calm",
            "persona_id": null,
            "persona_disabled": false,
            "voice_autoplay": true,
            "prompt_template_id": null,
            "lorebook_ids_override": "[]",
            "temperature": 0.8,
            "top_p": 0.9,
            "max_output_tokens": 512,
            "frequency_penalty": 0.0,
            "presence_penalty": 0.0,
            "top_k": 40,
            "advanced_model_settings": "{\"version\":1}",
            "companion_state": "{\"version\":1}",
            "memories": "[]",
            "memory_embeddings": "[]",
            "memory_summary": "Summary",
            "memory_summary_token_count": 3,
            "memory_tool_events": "[]",
            "memory_status": "complete",
            "memory_error": null,
            "memory_progress_step": 2,
            "archived": false,
            "created_at": 1,
            "updated_at": 20,
            "messages": messages
        })
    }

    #[test]
    fn direct_sessions_preserve_branch_messages_variants_and_runtime_payloads() {
        let character = id(1);
        let root = id(2);
        let branch = id(3);
        let root_message = id(4);
        let branch_message = id(5);
        let variant = id(6);
        let rows = json!([
            session(
                &root,
                &character,
                None,
                &root,
                None,
                vec![message(&root_message, None, Some(&variant))]
            ),
            session(
                &branch,
                &character,
                Some(&root),
                &root,
                Some(&branch_message),
                vec![message(&branch_message, None, None)]
            )
        ]);
        let plan = plan_legacy_backup_direct_sessions(source(rows, &character))
            .expect("direct session plan");
        assert_eq!(plan.sessions.len(), 2);
        assert_eq!(plan.sessions[1].root_session_source_id, root);
        assert_eq!(
            plan.sessions[1].parent_session_source_id.as_deref(),
            Some(root.as_str())
        );
        let root_message = &plan.sessions[0].messages[0];
        assert_eq!(root_message.ordinal, 0);
        assert_eq!(
            root_message.selected_variant_source_id.as_deref(),
            Some(variant.as_str())
        );
        assert_eq!(root_message.variants[0].content, "Alternative");
        assert_eq!(root_message.usage.prompt_tokens, Some(4));
        assert_eq!(
            root_message.attachments_json,
            "[{\"id\":\"image-1\",\"storagePath\":\"photo.png\"}]"
        );
        assert!(plan.notices.iter().any(|notice| {
            notice.kind == LegacyBackupConversionNoticeKind::Lossy
                && notice.field == "[].currentGenerationAttempts"
        }));
    }

    #[test]
    fn direct_sessions_reject_orphaned_branches_and_selected_variants() {
        let character = id(10);
        let root = id(11);
        let message_id = id(12);
        let orphan_parent = id(13);
        let rows = json!([session(
            &root,
            &character,
            Some(&orphan_parent),
            &root,
            None,
            vec![message(&message_id, None, None)]
        )]);
        assert!(matches!(
            plan_legacy_backup_direct_sessions(source(rows, &character)),
            Err(LegacyBackupSessionError::Orphan { ref field })
                if field == "[].root_session_id" || field == "[].parent_session_id"
        ));

        let missing_variant = id(14);
        let rows = json!([session(
            &root,
            &character,
            None,
            &root,
            None,
            vec![message(&message_id, None, Some(&missing_variant))]
        )]);
        let mut value = rows;
        value[0]["messages"][0]["variants"] = json!([]);
        assert!(matches!(
            plan_legacy_backup_direct_sessions(source(value, &character)),
            Err(LegacyBackupSessionError::Orphan { ref field })
                if field == "[0].messages[0].selected_variant_id"
        ));
    }

    #[test]
    fn direct_sessions_reject_negative_metrics_and_message_cycles() {
        let character = id(20);
        let root = id(21);
        let first = id(22);
        let second = id(23);
        let mut negative = session(
            &root,
            &character,
            None,
            &root,
            None,
            vec![message(&first, None, None)],
        );
        negative["messages"][0]["prompt_tokens"] = json!(-1);
        assert!(matches!(
            plan_legacy_backup_direct_sessions(source(json!([negative]), &character)),
            Err(LegacyBackupSessionError::Malformed { ref field })
                if field == "[0].messages[0].prompt_tokens"
        ));

        let mut invalid_mtp = session(
            &root,
            &character,
            None,
            &root,
            None,
            vec![message(&first, None, None)],
        );
        invalid_mtp["messages"][0]["mtp_stats"] = "{\"draftTokens\":-1}".into();
        assert!(matches!(
            plan_legacy_backup_direct_sessions(source(json!([invalid_mtp]), &character)),
            Err(LegacyBackupSessionError::Malformed { ref field })
                if field == "[0].messages[0].mtp_stats.draftTokens"
        ));

        let cycle = session(
            &root,
            &character,
            None,
            &root,
            None,
            vec![
                message(&first, Some(&second), None),
                message(&second, Some(&first), None),
            ],
        );
        assert!(matches!(
            plan_legacy_backup_direct_sessions(source(json!([cycle]), &character)),
            Err(LegacyBackupSessionError::Malformed { ref field })
                if field == "[0].messages[].parent_message_id"
        ));
    }
}
