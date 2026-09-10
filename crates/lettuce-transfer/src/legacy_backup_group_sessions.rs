use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use serde_json::Value;

use crate::{
    LegacyBackupConversionNotice, LegacyBackupConversionNoticeKind, LegacyBackupDirectSessionPlan,
    LegacyBackupDocumentKind,
};

const SESSION_LIMIT: usize = 10_000;
const PARTICIPATION_LIMIT: usize = 100_000;
const MESSAGE_LIMIT: usize = 200_000;
const VARIANT_LIMIT: usize = 400_000;
const TEXT_LIMIT: usize = 1_000_000;
const JSON_LIMIT: usize = 16 * 1024 * 1024;

#[derive(Debug)]
pub struct LegacyBackupGroupSessionPlan {
    pub sessions: Vec<LegacyBackupGroupSession>,
    pub notices: Vec<LegacyBackupConversionNotice>,
    pub source: LegacyBackupDirectSessionPlan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyBackupGroupSession {
    pub source_id: String,
    pub group_source_id: Option<String>,
    pub name: String,
    pub member_source_ids: Vec<String>,
    pub muted_member_source_ids: Vec<String>,
    pub persona_source_id: Option<String>,
    pub parent_session_source_id: Option<String>,
    pub branched_from_message_source_id: Option<String>,
    pub root_session_source_id: String,
    pub chat_mode: String,
    pub speaker_selection: String,
    pub memory_policy: String,
    pub character_model_overrides: BTreeMap<String, String>,
    pub group_conversation_prompt_source_id: Option<String>,
    pub group_roleplay_prompt_source_id: Option<String>,
    pub starting_scene_json: Option<String>,
    pub background_image_locator: Option<String>,
    pub lorebook_source_ids: Vec<String>,
    pub disable_character_lorebooks: bool,
    pub author_note: Option<String>,
    pub config_overrides_json: String,
    pub memories_json: String,
    pub memory_embeddings_json: String,
    pub memory_summary: String,
    pub memory_summary_token_count: u64,
    pub memory_tool_events_json: String,
    pub memory_status: Option<String>,
    pub memory_error: Option<String>,
    pub memory_progress_step: Option<u64>,
    pub archived: bool,
    pub created_at: u64,
    pub updated_at: u64,
    pub participation: Vec<LegacyBackupGroupParticipation>,
    pub messages: Vec<LegacyBackupGroupMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyBackupGroupParticipation {
    pub source_id: String,
    pub ordinal: u64,
    pub character_source_id: String,
    pub speak_count: u64,
    pub last_spoke_turn: Option<u64>,
    pub last_spoke_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyBackupGroupMessage {
    pub source_id: String,
    pub ordinal: u64,
    pub role: String,
    pub content: String,
    pub speaker_character_source_id: Option<String>,
    pub turn_number: u64,
    pub created_at: u64,
    pub usage: LegacyBackupGroupMessageUsage,
    pub selected_variant_source_id: Option<String>,
    pub pinned: bool,
    pub attachments_json: String,
    pub used_lorebook_entries_json: String,
    pub memory_refs_json: String,
    pub reasoning: Option<String>,
    pub selection_reasoning: Option<String>,
    pub model_source_id: Option<String>,
    pub gemini_content_json: Option<String>,
    pub usage_json: Option<String>,
    pub parent_message_source_id: Option<String>,
    pub variants: Vec<LegacyBackupGroupMessageVariant>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyBackupGroupMessageVariant {
    pub source_id: String,
    pub ordinal: u64,
    pub content: String,
    pub speaker_character_source_id: Option<String>,
    pub created_at: u64,
    pub usage: LegacyBackupGroupMessageUsage,
    pub reasoning: Option<String>,
    pub selection_reasoning: Option<String>,
    pub model_source_id: Option<String>,
    pub attachments_json: String,
    pub gemini_content_json: Option<String>,
    pub usage_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyBackupGroupMessageUsage {
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub first_token_ms: Option<u64>,
    pub tokens_per_second: Option<f64>,
    pub mtp_stats_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LegacyBackupGroupSessionError {
    #[error("legacy backup group session document is malformed")]
    Malformed { field: String },
    #[error("legacy backup group session document exceeds its record limit")]
    LimitExceeded,
    #[error("legacy backup group session graph contains an orphaned link")]
    Orphan { field: String },
}

#[derive(Deserialize)]
struct SessionRow {
    id: String,
    group_character_id: Option<String>,
    name: String,
    character_ids: String,
    muted_character_ids: String,
    persona_id: Option<String>,
    created_at: i64,
    updated_at: i64,
    archived: bool,
    chat_type: String,
    starting_scene: Option<String>,
    background_image_path: Option<String>,
    lorebook_ids: String,
    disable_character_lorebooks: bool,
    author_note: Option<String>,
    memories: String,
    memory_embeddings: String,
    memory_summary: String,
    memory_summary_token_count: i64,
    memory_tool_events: String,
    memory_status: Option<String>,
    memory_error: Option<String>,
    memory_progress_step: Option<i64>,
    speaker_selection_method: Option<String>,
    memory_type: Option<String>,
    config_overrides: String,
    parent_session_id: Option<String>,
    branched_from_message_id: Option<String>,
    root_session_id: Option<String>,
    character_model_overrides: String,
    group_chat_prompt_template_id: Option<String>,
    group_chat_roleplay_prompt_template_id: Option<String>,
    #[serde(default)]
    participation: Vec<ParticipationRow>,
    #[serde(default)]
    messages: Vec<MessageRow>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct ParticipationRow {
    id: String,
    character_id: String,
    speak_count: i64,
    last_spoke_turn: Option<i64>,
    last_spoke_at: Option<i64>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct MessageRow {
    id: String,
    role: String,
    content: String,
    speaker_character_id: Option<String>,
    turn_number: i64,
    created_at: i64,
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    total_tokens: Option<i64>,
    first_token_ms: Option<i64>,
    tokens_per_second: Option<f64>,
    mtp_stats: Option<String>,
    selected_variant_id: Option<String>,
    is_pinned: bool,
    attachments: String,
    used_lorebook_entries: String,
    memory_refs: String,
    reasoning: Option<String>,
    selection_reasoning: Option<String>,
    model_id: Option<String>,
    gemini_content: Option<String>,
    usage_json: Option<String>,
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
    speaker_character_id: Option<String>,
    created_at: i64,
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    total_tokens: Option<i64>,
    first_token_ms: Option<i64>,
    tokens_per_second: Option<f64>,
    mtp_stats: Option<String>,
    reasoning: Option<String>,
    selection_reasoning: Option<String>,
    model_id: Option<String>,
    attachments: String,
    gemini_content: Option<String>,
    usage_json: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct StartingSceneRow {
    id: String,
    content: String,
    direction: Option<String>,
    #[serde(alias = "backgroundImagePath")]
    background_image_path: Option<String>,
    #[serde(alias = "createdAt")]
    created_at: i64,
    #[serde(alias = "selectedVariantId")]
    selected_variant_id: Option<String>,
    #[serde(default)]
    variants: Vec<StartingSceneVariantRow>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct StartingSceneVariantRow {
    id: String,
    content: String,
    direction: Option<String>,
    #[serde(alias = "createdAt")]
    created_at: i64,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

pub fn plan_legacy_backup_group_sessions(
    source: LegacyBackupDirectSessionPlan,
) -> Result<LegacyBackupGroupSessionPlan, LegacyBackupGroupSessionError> {
    let document = source
        .source
        .source
        .source
        .source
        .authored
        .configuration
        .source
        .documents
        .iter()
        .find(|document| document.kind == LegacyBackupDocumentKind::GroupSessions);
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
    Ok(LegacyBackupGroupSessionPlan {
        sessions,
        notices,
        source,
    })
}

fn map_sessions(
    rows: Vec<SessionRow>,
    source: &LegacyBackupDirectSessionPlan,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<LegacyBackupGroupSession>, LegacyBackupGroupSessionError> {
    if rows.len() > SESSION_LIMIT {
        return Err(LegacyBackupGroupSessionError::LimitExceeded);
    }
    let authored = &source.source.source.source.source.authored;
    let character_ids = authored
        .characters
        .iter()
        .map(|value| value.id.to_string())
        .collect::<BTreeSet<_>>();
    let group_ids = authored
        .groups
        .iter()
        .map(|value| value.id.to_string())
        .collect::<BTreeSet<_>>();
    let persona_ids = authored
        .personas
        .personas
        .iter()
        .map(|value| value.id.to_string())
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
    let prompt_ids = authored
        .configuration
        .prompts
        .prompts
        .iter()
        .map(|value| value.source_id.clone())
        .collect::<BTreeSet<_>>();
    let mut session_ids = BTreeSet::new();
    let mut participation_ids = BTreeSet::new();
    let mut message_ids = BTreeSet::new();
    let mut variant_ids = BTreeSet::new();
    let mut participation_count = 0_usize;
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
        if row
            .group_character_id
            .as_ref()
            .is_some_and(|id| !contains_case_insensitive(&group_ids, id))
        {
            return Err(orphan(format!("{path}.group_character_id")));
        }
        let members = string_array(&row.character_ids, &format!("{path}.character_ids"))?;
        if members.len() < 2 {
            return Err(malformed(format!("{path}.character_ids")));
        }
        if members
            .iter()
            .any(|id| !contains_case_insensitive(&character_ids, id))
        {
            return Err(orphan(format!("{path}.character_ids")));
        }
        let muted = string_array(
            &row.muted_character_ids,
            &format!("{path}.muted_character_ids"),
        )?;
        if muted
            .iter()
            .any(|id| !contains_case_insensitive_slice(&members, id))
        {
            return Err(orphan(format!("{path}.muted_character_ids")));
        }
        validate_optional_reference(
            row.persona_id.as_deref(),
            &persona_ids,
            &format!("{path}.persona_id"),
        )?;
        validate_optional_exact_reference(
            row.group_chat_prompt_template_id.as_deref(),
            &prompt_ids,
            &format!("{path}.group_chat_prompt_template_id"),
        )?;
        validate_optional_exact_reference(
            row.group_chat_roleplay_prompt_template_id.as_deref(),
            &prompt_ids,
            &format!("{path}.group_chat_roleplay_prompt_template_id"),
        )?;
        let lorebooks = string_array(&row.lorebook_ids, &format!("{path}.lorebook_ids"))?;
        if lorebooks
            .iter()
            .any(|id| !contains_case_insensitive(&lorebook_ids, id))
        {
            return Err(orphan(format!("{path}.lorebook_ids")));
        }
        let overrides = string_map(
            &row.character_model_overrides,
            &format!("{path}.character_model_overrides"),
        )?;
        for (character_id, model_id) in &overrides {
            if !contains_case_insensitive_slice(&members, character_id) {
                return Err(orphan(format!(
                    "{path}.character_model_overrides.character_id"
                )));
            }
            if !contains_case_insensitive(&model_ids, model_id) {
                return Err(orphan(format!("{path}.character_model_overrides.model_id")));
            }
        }
        validate_text(&row.name, &format!("{path}.name"))?;
        if row.name.trim().is_empty() {
            return Err(malformed(format!("{path}.name")));
        }
        let chat_mode = match row.chat_type.as_str() {
            "conversation" | "roleplay" => row.chat_type,
            _ => return Err(malformed(format!("{path}.chat_type"))),
        };
        let speaker_selection = match row.speaker_selection_method.as_deref().unwrap_or("llm") {
            value @ ("llm" | "heuristic" | "round_robin" | "director" | "director_action") => {
                value.to_owned()
            }
            _ => return Err(malformed(format!("{path}.speaker_selection_method"))),
        };
        let memory_policy = match row.memory_type.as_deref().unwrap_or("manual") {
            value @ ("manual" | "dynamic") => value.to_owned(),
            _ => return Err(malformed(format!("{path}.memory_type"))),
        };
        for (field, raw, shape) in [
            (
                "config_overrides",
                row.config_overrides.as_str(),
                JsonShape::Object,
            ),
            ("memories", row.memories.as_str(), JsonShape::Array),
            (
                "memory_embeddings",
                row.memory_embeddings.as_str(),
                JsonShape::Array,
            ),
            (
                "memory_tool_events",
                row.memory_tool_events.as_str(),
                JsonShape::Array,
            ),
        ] {
            validate_json(raw, Some(shape), &format!("{path}.{field}"))?;
        }
        if let Some(raw) = &row.starting_scene {
            validate_starting_scene(raw, &format!("{path}.starting_scene"), notices)?;
        }
        let created_at = timestamp(row.created_at, &format!("{path}.created_at"))?;
        let updated_at = timestamp(row.updated_at, &format!("{path}.updated_at"))?;
        if created_at > updated_at || row.memory_summary_token_count < 0 {
            return Err(malformed(format!("{path}.timestamps_or_memory_count")));
        }
        let participation = map_participation(
            row.participation,
            &path,
            &members,
            &mut participation_ids,
            notices,
        )?;
        let messages = map_messages(
            row.messages,
            &path,
            &members,
            &model_ids,
            &mut message_ids,
            &mut variant_ids,
            notices,
        )?;
        participation_count = checked_total(participation_count, participation.len())?;
        message_count = checked_total(message_count, messages.len())?;
        variant_count = messages.iter().try_fold(variant_count, |total, message| {
            checked_total(total, message.variants.len())
        })?;
        if participation_count > PARTICIPATION_LIMIT
            || message_count > MESSAGE_LIMIT
            || variant_count > VARIANT_LIMIT
        {
            return Err(LegacyBackupGroupSessionError::LimitExceeded);
        }
        let root_session_source_id = row.root_session_id.unwrap_or_else(|| row.id.clone());
        sessions.push(LegacyBackupGroupSession {
            source_id: row.id,
            group_source_id: row.group_character_id,
            name: row.name,
            member_source_ids: members,
            muted_member_source_ids: muted,
            persona_source_id: row.persona_id,
            parent_session_source_id: row.parent_session_id,
            branched_from_message_source_id: row.branched_from_message_id,
            root_session_source_id,
            chat_mode,
            speaker_selection,
            memory_policy,
            character_model_overrides: overrides,
            group_conversation_prompt_source_id: row.group_chat_prompt_template_id,
            group_roleplay_prompt_source_id: row.group_chat_roleplay_prompt_template_id,
            starting_scene_json: row.starting_scene,
            background_image_locator: row.background_image_path,
            lorebook_source_ids: lorebooks,
            disable_character_lorebooks: row.disable_character_lorebooks,
            author_note: row.author_note,
            config_overrides_json: row.config_overrides,
            memories_json: row.memories,
            memory_embeddings_json: row.memory_embeddings,
            memory_summary: row.memory_summary,
            memory_summary_token_count: u64::try_from(row.memory_summary_token_count)
                .map_err(|_| malformed(format!("{path}.memory_summary_token_count")))?,
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
            participation,
            messages,
        });
    }
    validate_session_graph(&sessions)?;
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

fn map_participation(
    rows: Vec<ParticipationRow>,
    session_path: &str,
    members: &[String],
    all_ids: &mut BTreeSet<String>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<LegacyBackupGroupParticipation>, LegacyBackupGroupSessionError> {
    if rows.len() > 1 {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Lossy,
            &format!("{session_path}.participation[].source_order"),
        ));
    }
    let mut character_ids = BTreeSet::new();
    rows.into_iter()
        .enumerate()
        .map(|(index, row)| {
            let path = format!("{session_path}.participation[{index}]");
            report_extra(&path, &row.extra, notices);
            validate_identifier(&row.id, &format!("{path}.id"))?;
            if !all_ids.insert(row.id.clone()) {
                return Err(malformed(format!("{path}.id")));
            }
            if !contains_case_insensitive_slice(members, &row.character_id) {
                return Err(orphan(format!("{path}.character_id")));
            }
            if !character_ids.insert(row.character_id.to_ascii_lowercase()) {
                return Err(malformed(format!("{path}.character_id")));
            }
            Ok(LegacyBackupGroupParticipation {
                source_id: row.id,
                ordinal: u64::try_from(index)
                    .map_err(|_| LegacyBackupGroupSessionError::LimitExceeded)?,
                character_source_id: row.character_id,
                speak_count: count(row.speak_count, &format!("{path}.speak_count"))?,
                last_spoke_turn: optional_count(
                    row.last_spoke_turn,
                    &format!("{path}.last_spoke_turn"),
                )?,
                last_spoke_at: optional_count(row.last_spoke_at, &format!("{path}.last_spoke_at"))?,
            })
        })
        .collect()
}

fn map_messages(
    rows: Vec<MessageRow>,
    session_path: &str,
    members: &[String],
    model_ids: &BTreeSet<String>,
    all_message_ids: &mut BTreeSet<String>,
    all_variant_ids: &mut BTreeSet<String>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<LegacyBackupGroupMessage>, LegacyBackupGroupSessionError> {
    let local_ids = rows
        .iter()
        .map(|row| row.id.clone())
        .collect::<BTreeSet<_>>();
    if local_ids.len() != rows.len() {
        return Err(malformed(format!("{session_path}.messages[].id")));
    }
    let mut messages = Vec::with_capacity(rows.len());
    let mut ordering_keys = BTreeSet::new();
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
        validate_speaker(
            row.speaker_character_id.as_deref(),
            members,
            &format!("{path}.speaker_character_id"),
        )?;
        validate_optional_reference(
            row.model_id.as_deref(),
            model_ids,
            &format!("{path}.model_id"),
        )?;
        if row
            .parent_message_id
            .as_ref()
            .is_some_and(|id| !local_ids.contains(id))
        {
            return Err(orphan(format!("{path}.parent_message_id")));
        }
        validate_text(&row.content, &format!("{path}.content"))?;
        for (field, raw) in [
            ("attachments", row.attachments.as_str()),
            ("used_lorebook_entries", row.used_lorebook_entries.as_str()),
            ("memory_refs", row.memory_refs.as_str()),
        ] {
            validate_json(raw, Some(JsonShape::Array), &format!("{path}.{field}"))?;
        }
        validate_optional_json(
            row.gemini_content.as_deref(),
            &format!("{path}.gemini_content"),
        )?;
        validate_usage_json(row.usage_json.as_deref(), &format!("{path}.usage_json"))?;
        if let Some(raw) = &row.mtp_stats {
            validate_mtp_stats(raw, &format!("{path}.mtp_stats"))?;
        }
        let created_at = timestamp(row.created_at, &format!("{path}.created_at"))?;
        if !ordering_keys.insert((created_at, row.turn_number)) {
            notices.push(notice(
                LegacyBackupConversionNoticeKind::Lossy,
                &format!("{session_path}.messages[].created_at_turn_order"),
            ));
        }
        let usage = usage(&row, &path)?;
        let variants = map_variants(
            row.variants,
            &path,
            members,
            model_ids,
            all_variant_ids,
            notices,
        )?;
        if row
            .selected_variant_id
            .as_ref()
            .is_some_and(|id| !variants.iter().any(|variant| variant.source_id == *id))
        {
            return Err(orphan(format!("{path}.selected_variant_id")));
        }
        messages.push(LegacyBackupGroupMessage {
            source_id: row.id,
            ordinal: u64::try_from(index)
                .map_err(|_| LegacyBackupGroupSessionError::LimitExceeded)?,
            role: row.role,
            content: row.content,
            speaker_character_source_id: row.speaker_character_id,
            turn_number: count(row.turn_number, &format!("{path}.turn_number"))?,
            created_at,
            usage,
            selected_variant_source_id: row.selected_variant_id,
            pinned: row.is_pinned,
            attachments_json: row.attachments,
            used_lorebook_entries_json: row.used_lorebook_entries,
            memory_refs_json: row.memory_refs,
            reasoning: row.reasoning,
            selection_reasoning: row.selection_reasoning,
            model_source_id: row.model_id,
            gemini_content_json: row.gemini_content,
            usage_json: row.usage_json,
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
    members: &[String],
    model_ids: &BTreeSet<String>,
    all_ids: &mut BTreeSet<String>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<LegacyBackupGroupMessageVariant>, LegacyBackupGroupSessionError> {
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
            if !all_ids.insert(row.id.clone()) {
                return Err(malformed(format!("{path}.id")));
            }
            validate_text(&row.content, &format!("{path}.content"))?;
            validate_speaker(
                row.speaker_character_id.as_deref(),
                members,
                &format!("{path}.speaker_character_id"),
            )?;
            validate_optional_reference(
                row.model_id.as_deref(),
                model_ids,
                &format!("{path}.model_id"),
            )?;
            validate_json(
                &row.attachments,
                Some(JsonShape::Array),
                &format!("{path}.attachments"),
            )?;
            validate_optional_json(
                row.gemini_content.as_deref(),
                &format!("{path}.gemini_content"),
            )?;
            validate_usage_json(row.usage_json.as_deref(), &format!("{path}.usage_json"))?;
            if let Some(raw) = &row.mtp_stats {
                validate_mtp_stats(raw, &format!("{path}.mtp_stats"))?;
            }
            let usage = LegacyBackupGroupMessageUsage {
                prompt_tokens: optional_count(row.prompt_tokens, &format!("{path}.prompt_tokens"))?,
                completion_tokens: optional_count(
                    row.completion_tokens,
                    &format!("{path}.completion_tokens"),
                )?,
                total_tokens: optional_count(row.total_tokens, &format!("{path}.total_tokens"))?,
                first_token_ms: optional_count(
                    row.first_token_ms,
                    &format!("{path}.first_token_ms"),
                )?,
                tokens_per_second: finite_nonnegative(
                    row.tokens_per_second,
                    &format!("{path}.tokens_per_second"),
                )?,
                mtp_stats_json: row.mtp_stats,
            };
            Ok(LegacyBackupGroupMessageVariant {
                source_id: row.id,
                ordinal: u64::try_from(index)
                    .map_err(|_| LegacyBackupGroupSessionError::LimitExceeded)?,
                content: row.content,
                speaker_character_source_id: row.speaker_character_id,
                created_at: timestamp(row.created_at, &format!("{path}.created_at"))?,
                usage,
                reasoning: row.reasoning,
                selection_reasoning: row.selection_reasoning,
                model_source_id: row.model_id,
                attachments_json: row.attachments,
                gemini_content_json: row.gemini_content,
                usage_json: row.usage_json,
            })
        })
        .collect()
}

fn usage(
    row: &MessageRow,
    path: &str,
) -> Result<LegacyBackupGroupMessageUsage, LegacyBackupGroupSessionError> {
    Ok(LegacyBackupGroupMessageUsage {
        prompt_tokens: optional_count(row.prompt_tokens, &format!("{path}.prompt_tokens"))?,
        completion_tokens: optional_count(
            row.completion_tokens,
            &format!("{path}.completion_tokens"),
        )?,
        total_tokens: optional_count(row.total_tokens, &format!("{path}.total_tokens"))?,
        first_token_ms: optional_count(row.first_token_ms, &format!("{path}.first_token_ms"))?,
        tokens_per_second: finite_nonnegative(
            row.tokens_per_second,
            &format!("{path}.tokens_per_second"),
        )?,
        mtp_stats_json: row.mtp_stats.clone(),
    })
}

fn validate_session_graph(
    sessions: &[LegacyBackupGroupSession],
) -> Result<(), LegacyBackupGroupSessionError> {
    let by_id = sessions
        .iter()
        .map(|session| (session.source_id.as_str(), session))
        .collect::<BTreeMap<_, _>>();
    for session in sessions {
        let root = by_id
            .get(session.root_session_source_id.as_str())
            .ok_or_else(|| orphan("[].root_session_id"))?;
        if root.parent_session_source_id.is_some() || root.root_session_source_id != root.source_id
        {
            return Err(malformed("[].root_session_id"));
        }
        if root.group_source_id != session.group_source_id {
            return Err(orphan("[].root_session_id"));
        }
        if let Some(parent_id) = &session.parent_session_source_id {
            let parent = by_id
                .get(parent_id.as_str())
                .ok_or_else(|| orphan("[].parent_session_id"))?;
            if parent.root_session_source_id != session.root_session_source_id
                || parent.group_source_id != session.group_source_id
            {
                return Err(orphan("[].parent_session_id"));
            }
            let branch_id = session
                .branched_from_message_source_id
                .as_deref()
                .ok_or_else(|| malformed("[].branched_from_message_id"))?;
            if !session
                .messages
                .iter()
                .any(|message| message.source_id == branch_id)
            {
                return Err(orphan("[].branched_from_message_id"));
            }
        } else if session.branched_from_message_source_id.is_some()
            || session.root_session_source_id != session.source_id
        {
            return Err(malformed("[].parent_session_id"));
        }
        let mut cursor = session.parent_session_source_id.as_deref();
        let mut visited = BTreeSet::new();
        while let Some(id) = cursor {
            if id == session.source_id || !visited.insert(id) {
                return Err(malformed("[].parent_session_id"));
            }
            cursor = by_id
                .get(id)
                .and_then(|parent| parent.parent_session_source_id.as_deref());
        }
    }
    Ok(())
}

fn validate_message_cycles(
    messages: &[LegacyBackupGroupMessage],
    path: &str,
) -> Result<(), LegacyBackupGroupSessionError> {
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

fn validate_starting_scene(
    raw: &str,
    field: &str,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<(), LegacyBackupGroupSessionError> {
    if raw.len() > JSON_LIMIT {
        return Err(LegacyBackupGroupSessionError::LimitExceeded);
    }
    let row: StartingSceneRow = serde_json::from_str(raw).map_err(|_| malformed(field))?;
    validate_identifier(&row.id, &format!("{field}.id"))?;
    validate_text(&row.content, &format!("{field}.content"))?;
    if let Some(direction) = &row.direction {
        validate_text(direction, &format!("{field}.direction"))?;
    }
    if let Some(background) = &row.background_image_path {
        validate_text(background, &format!("{field}.background_image_path"))?;
    }
    timestamp(row.created_at, &format!("{field}.created_at"))?;
    report_extra(field, &row.extra, notices);
    let mut variant_ids = BTreeSet::new();
    for (index, variant) in row.variants.iter().enumerate() {
        let variant_field = format!("{field}.variants[{index}]");
        validate_identifier(&variant.id, &format!("{variant_field}.id"))?;
        if !variant_ids.insert(variant.id.as_str()) {
            return Err(malformed(format!("{field}.variants[].id")));
        }
        validate_text(&variant.content, &format!("{variant_field}.content"))?;
        if let Some(direction) = &variant.direction {
            validate_text(direction, &format!("{variant_field}.direction"))?;
        }
        timestamp(variant.created_at, &format!("{variant_field}.created_at"))?;
        report_extra(&variant_field, &variant.extra, notices);
    }
    if row
        .selected_variant_id
        .as_deref()
        .is_some_and(|id| !variant_ids.contains(id))
    {
        return Err(orphan(format!("{field}.selected_variant_id")));
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum JsonShape {
    Array,
    Object,
}

fn validate_json(
    raw: &str,
    shape: Option<JsonShape>,
    field: &str,
) -> Result<(), LegacyBackupGroupSessionError> {
    if raw.len() > JSON_LIMIT {
        return Err(LegacyBackupGroupSessionError::LimitExceeded);
    }
    let value: Value = serde_json::from_str(raw).map_err(|_| malformed(field))?;
    if shape.is_some_and(|shape| {
        !matches!(
            (shape, &value),
            (JsonShape::Array, Value::Array(_)) | (JsonShape::Object, Value::Object(_))
        )
    }) {
        return Err(malformed(field));
    }
    Ok(())
}

fn validate_optional_json(
    raw: Option<&str>,
    field: &str,
) -> Result<(), LegacyBackupGroupSessionError> {
    if let Some(raw) = raw {
        validate_json(raw, None, field)?;
    }
    Ok(())
}

fn validate_optional_json_shape(
    raw: Option<&str>,
    shape: JsonShape,
    field: &str,
) -> Result<(), LegacyBackupGroupSessionError> {
    if let Some(raw) = raw {
        validate_json(raw, Some(shape), field)?;
    }
    Ok(())
}

fn validate_usage_json(
    raw: Option<&str>,
    field: &str,
) -> Result<(), LegacyBackupGroupSessionError> {
    let Some(raw) = raw else {
        return Ok(());
    };
    validate_optional_json_shape(Some(raw), JsonShape::Object, field)?;
    let value: Value = serde_json::from_str(raw).map_err(|_| malformed(field))?;
    let object = value.as_object().ok_or_else(|| malformed(field))?;
    for key in [
        "promptTokens",
        "completionTokens",
        "totalTokens",
        "cachedPromptTokens",
        "cacheWriteTokens",
        "reasoningTokens",
        "imageTokens",
        "audioTokens",
        "webSearchRequests",
        "firstTokenMs",
    ] {
        if object
            .get(key)
            .is_some_and(|value| !value.is_null() && value.as_u64().is_none())
        {
            return Err(malformed(format!("{field}.{key}")));
        }
    }
    if object.get("apiCost").is_some_and(|value| {
        !value.is_null() && !value.as_f64().is_some_and(|number| number.is_finite())
    }) {
        return Err(malformed(format!("{field}.apiCost")));
    }
    if object.get("tokensPerSecond").is_some_and(|value| {
        !value.is_null()
            && !value
                .as_f64()
                .is_some_and(|number| number.is_finite() && number >= 0.0)
    }) {
        return Err(malformed(format!("{field}.tokensPerSecond")));
    }
    for key in ["responseId", "finishReason"] {
        if object
            .get(key)
            .is_some_and(|value| !value.is_null() && !value.is_string())
        {
            return Err(malformed(format!("{field}.{key}")));
        }
    }
    if let Some(value) = object.get("mtpStats").filter(|value| !value.is_null()) {
        validate_mtp_stats(&value.to_string(), &format!("{field}.mtpStats"))?;
    }
    Ok(())
}

fn validate_mtp_stats(raw: &str, field: &str) -> Result<(), LegacyBackupGroupSessionError> {
    validate_json(raw, Some(JsonShape::Object), field)?;
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
            !value.is_null()
                && !value
                    .as_f64()
                    .is_some_and(|number| number.is_finite() && number >= 0.0)
        }) {
            return Err(malformed(format!("{field}.{key}")));
        }
    }
    Ok(())
}

fn string_array(raw: &str, field: &str) -> Result<Vec<String>, LegacyBackupGroupSessionError> {
    if raw.len() > JSON_LIMIT {
        return Err(LegacyBackupGroupSessionError::LimitExceeded);
    }
    let values: Vec<String> = serde_json::from_str(raw).map_err(|_| malformed(field))?;
    let mut seen = BTreeSet::new();
    for value in &values {
        validate_identifier(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(malformed(field));
        }
    }
    Ok(values)
}

fn string_map(
    raw: &str,
    field: &str,
) -> Result<BTreeMap<String, String>, LegacyBackupGroupSessionError> {
    if raw.len() > JSON_LIMIT {
        return Err(LegacyBackupGroupSessionError::LimitExceeded);
    }
    let values: BTreeMap<String, String> =
        serde_json::from_str(raw).map_err(|_| malformed(field))?;
    let mut seen = BTreeSet::new();
    for (key, value) in &values {
        validate_identifier(key, field)?;
        validate_identifier(value, field)?;
        if !seen.insert(key.to_ascii_lowercase()) {
            return Err(malformed(field));
        }
    }
    Ok(values)
}

fn validate_speaker(
    value: Option<&str>,
    members: &[String],
    field: &str,
) -> Result<(), LegacyBackupGroupSessionError> {
    if let Some(value) = value {
        if !contains_case_insensitive_slice(members, value) {
            return Err(orphan(field));
        }
    }
    Ok(())
}

fn validate_optional_reference(
    value: Option<&str>,
    values: &BTreeSet<String>,
    field: &str,
) -> Result<(), LegacyBackupGroupSessionError> {
    if value.is_some_and(|value| !contains_case_insensitive(values, value)) {
        return Err(orphan(field));
    }
    Ok(())
}

fn validate_optional_exact_reference(
    value: Option<&str>,
    values: &BTreeSet<String>,
    field: &str,
) -> Result<(), LegacyBackupGroupSessionError> {
    if value.is_some_and(|value| !values.contains(value)) {
        return Err(orphan(field));
    }
    Ok(())
}

fn contains_case_insensitive(values: &BTreeSet<String>, expected: &str) -> bool {
    values
        .iter()
        .any(|value| value.eq_ignore_ascii_case(expected))
}
fn contains_case_insensitive_slice(values: &[String], expected: &str) -> bool {
    values
        .iter()
        .any(|value| value.eq_ignore_ascii_case(expected))
}

fn validate_identifier(value: &str, field: &str) -> Result<(), LegacyBackupGroupSessionError> {
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

fn validate_text(value: &str, field: &str) -> Result<(), LegacyBackupGroupSessionError> {
    if value.chars().count() > TEXT_LIMIT || value.contains('\0') {
        Err(malformed(field))
    } else {
        Ok(())
    }
}

fn finite_nonnegative(
    value: Option<f64>,
    field: &str,
) -> Result<Option<f64>, LegacyBackupGroupSessionError> {
    if value.is_some_and(|value| !value.is_finite() || value < 0.0) {
        Err(malformed(field))
    } else {
        Ok(value)
    }
}

fn timestamp(value: i64, field: &str) -> Result<u64, LegacyBackupGroupSessionError> {
    u64::try_from(value).map_err(|_| malformed(field))
}
fn count(value: i64, field: &str) -> Result<u64, LegacyBackupGroupSessionError> {
    u64::try_from(value).map_err(|_| malformed(field))
}
fn optional_count(
    value: Option<i64>,
    field: &str,
) -> Result<Option<u64>, LegacyBackupGroupSessionError> {
    value.map(|value| count(value, field)).transpose()
}
fn checked_total(total: usize, added: usize) -> Result<usize, LegacyBackupGroupSessionError> {
    total
        .checked_add(added)
        .ok_or(LegacyBackupGroupSessionError::LimitExceeded)
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
        document: LegacyBackupDocumentKind::GroupSessions,
        field: field.to_owned(),
    }
}

fn malformed(field: impl Into<String>) -> LegacyBackupGroupSessionError {
    LegacyBackupGroupSessionError::Malformed {
        field: field.into(),
    }
}
fn orphan(field: impl Into<String>) -> LegacyBackupGroupSessionError {
    LegacyBackupGroupSessionError::Orphan {
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
        plan_legacy_backup_configuration, plan_legacy_backup_direct_sessions,
        plan_legacy_backup_pricing, plan_legacy_backup_usage,
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

    fn source(rows: Value, characters: &[String], group_id: &str) -> LegacyBackupDirectSessionPlan {
        let character_rows = characters
            .iter()
            .map(|character| {
                json!({
                    "id": character,
                    "name": format!("Character {character}"),
                    "created_at": 1,
                    "updated_at": 1
                })
            })
            .collect::<Vec<_>>();
        let inventory = LegacyBackupInventory {
            version: 1,
            created_at: 1,
            app_version: "legacy".into(),
            source_hash: ContentHash::parse("88".repeat(32)).expect("source hash"),
            documents: vec![
                document(LegacyBackupDocumentKind::Characters, json!(character_rows)),
                document(
                    LegacyBackupDocumentKind::GroupCharacters,
                    json!([{
                        "id": group_id,
                        "name": "Writers Room",
                        "character_ids": serde_json::to_string(characters).expect("members"),
                        "muted_character_ids": "[]",
                        "created_at": 1,
                        "updated_at": 1,
                        "speaker_selection_method": "director",
                        "memory_type": "dynamic"
                    }]),
                ),
                document(LegacyBackupDocumentKind::GroupSessions, rows),
            ],
            media: Vec::new(),
        };
        let configuration =
            plan_legacy_backup_configuration(inventory).expect("configuration plan");
        let authored = plan_legacy_backup_authored(configuration).expect("authored plan");
        let media = plan_legacy_backup_authored_media(authored).expect("media plan");
        let asr = plan_legacy_backup_asr(media).expect("ASR plan");
        let usage = plan_legacy_backup_usage(asr).expect("usage plan");
        let pricing = plan_legacy_backup_pricing(usage).expect("pricing plan");
        plan_legacy_backup_direct_sessions(pricing).expect("direct session plan")
    }

    fn message(
        message_id: &str,
        speaker_id: Option<&str>,
        parent_id: Option<&str>,
        variant_id: Option<&str>,
    ) -> Value {
        json!({
            "id": message_id,
            "role": if speaker_id.is_some() { "assistant" } else { "user" },
            "content": "A preserved message",
            "speaker_character_id": speaker_id,
            "turn_number": 2,
            "created_at": 10,
            "prompt_tokens": 5,
            "completion_tokens": 3,
            "total_tokens": 8,
            "first_token_ms": 25,
            "tokens_per_second": 11.5,
            "mtp_stats": "{\"draftTokens\":2,\"rounds\":1}",
            "selected_variant_id": variant_id,
            "is_pinned": true,
            "attachments": "[{\"id\":\"image-1\",\"storagePath\":\"photo.png\"}]",
            "used_lorebook_entries": "[\"entry-1\"]",
            "memory_refs": "[\"memory-1\"]",
            "reasoning": "Response reasoning",
            "selection_reasoning": "User selected this member",
            "model_id": null,
            "gemini_content": "[{\"role\":\"model\",\"parts\":[]}]",
            "usage_json": "{\"promptTokens\":5,\"completionTokens\":3,\"totalTokens\":8,\"apiCost\":-0.01}",
            "parent_message_id": parent_id,
            "variants": variant_id.map(|variant| vec![json!({
                "id": variant,
                "content": "Alternative response",
                "speaker_character_id": speaker_id,
                "created_at": 12,
                "prompt_tokens": 5,
                "completion_tokens": 4,
                "total_tokens": 9,
                "first_token_ms": 30,
                "tokens_per_second": 10.0,
                "mtp_stats": null,
                "reasoning": "Alternative reasoning",
                "selection_reasoning": "Director changed the response",
                "model_id": null,
                "attachments": "[]",
                "gemini_content": "{\"parts\":[]}",
                "usage_json": "{\"promptTokens\":5,\"completionTokens\":4,\"totalTokens\":9}"
            })]).unwrap_or_default()
        })
    }

    fn session(
        session_id: &str,
        group_id: &str,
        characters: &[String],
        parent_id: Option<&str>,
        root_id: &str,
        branch_message_id: Option<&str>,
        messages: Vec<Value>,
    ) -> Value {
        let scene = json!({
            "id": id(900),
            "content": "A quiet library",
            "direction": "Keep the scene intimate",
            "backgroundImagePath": "library.png",
            "createdAt": 1,
            "selectedVariantId": id(901),
            "variants": [{
                "id": id(901),
                "content": "A rainy library",
                "direction": null,
                "createdAt": 2
            }]
        });
        json!({
            "id": session_id,
            "group_character_id": group_id,
            "name": "Writers Room Session",
            "character_ids": serde_json::to_string(characters).expect("members"),
            "muted_character_ids": serde_json::to_string(&characters[1..]).expect("muted"),
            "persona_id": null,
            "created_at": 1,
            "updated_at": 20,
            "archived": false,
            "chat_type": "roleplay",
            "starting_scene": serde_json::to_string(&scene).expect("scene"),
            "background_image_path": "group.png",
            "lorebook_ids": "[]",
            "disable_character_lorebooks": true,
            "author_note": "Keep every voice distinct",
            "memories": "[\"A shared memory\"]",
            "memory_embeddings": "[]",
            "memory_summary": "The group met before.",
            "memory_summary_token_count": 5,
            "memory_tool_events": "[{\"kind\":\"cycle\"}]",
            "memory_status": "complete",
            "memory_error": null,
            "memory_progress_step": 2,
            "speaker_selection_method": "director_action",
            "memory_type": "dynamic",
            "config_overrides": "{\"version\":1,\"temperature\":0.7}",
            "parent_session_id": parent_id,
            "branched_from_message_id": branch_message_id,
            "root_session_id": root_id,
            "character_model_overrides": "{}",
            "group_chat_prompt_template_id": null,
            "group_chat_roleplay_prompt_template_id": null,
            "participation": characters.iter().enumerate().map(|(index, character)| json!({
                "id": format!("{session_id}-participation-{index}"),
                "character_id": character,
                "speak_count": index,
                "last_spoke_turn": if index == 0 { Some(2) } else { None },
                "last_spoke_at": if index == 0 { Some(10) } else { None }
            })).collect::<Vec<_>>(),
            "messages": messages
        })
    }

    #[test]
    fn group_sessions_preserve_director_graph_and_nested_runtime_payloads() {
        let characters = vec![id(1), id(2)];
        let group = id(3);
        let root = id(4);
        let branch = id(5);
        let root_message = id(6);
        let branch_message = id(7);
        let variant = id(8);
        let rows = json!([
            session(
                &root,
                &group,
                &characters,
                None,
                &root,
                None,
                vec![message(
                    &root_message,
                    Some(&characters[0]),
                    None,
                    Some(&variant)
                )]
            ),
            session(
                &branch,
                &group,
                &characters,
                Some(&root),
                &root,
                Some(&branch_message),
                vec![message(&branch_message, Some(&characters[1]), None, None)]
            )
        ]);
        let plan = plan_legacy_backup_group_sessions(source(rows, &characters, &group))
            .expect("group session plan");
        assert_eq!(plan.sessions.len(), 2);
        assert_eq!(plan.sessions[0].speaker_selection, "director_action");
        assert_eq!(plan.sessions[0].member_source_ids, characters);
        assert_eq!(plan.sessions[0].participation[0].speak_count, 0);
        assert_eq!(plan.sessions[0].messages[0].variants[0].source_id, variant);
        assert_eq!(
            plan.sessions[0].messages[0].selection_reasoning.as_deref(),
            Some("User selected this member")
        );
        assert!(
            plan.sessions[0].messages[0]
                .usage_json
                .as_deref()
                .is_some_and(|usage| usage.contains("\"apiCost\":-0.01"))
        );
        assert!(plan.sessions[0].starting_scene_json.is_some());
        assert!(plan.notices.iter().any(|notice| {
            notice.kind == LegacyBackupConversionNoticeKind::Lossy
                && notice.field == "[].currentGenerationAttempts"
        }));
    }

    #[test]
    fn director_sessions_need_no_persisted_selected_speaker() {
        let characters = vec![id(20), id(21)];
        let group = id(22);
        let root = id(23);
        let message_id = id(24);
        let mut row = session(
            &root,
            &group,
            &characters,
            None,
            &root,
            None,
            vec![message(&message_id, None, None, None)],
        );
        row["muted_character_ids"] = serde_json::to_string(&characters)
            .expect("all muted")
            .into();
        let plan = plan_legacy_backup_group_sessions(source(json!([row]), &characters, &group))
            .expect("director session without selected speaker");
        assert_eq!(plan.sessions[0].speaker_selection, "director_action");
        assert_eq!(plan.sessions[0].muted_member_source_ids, characters);
    }

    #[test]
    fn group_sessions_reject_orphaned_links_and_malformed_nested_usage() {
        let characters = vec![id(30), id(31)];
        let group = id(32);
        let root = id(33);
        let message_id = id(34);
        let mut orphaned = session(
            &root,
            &group,
            &characters,
            None,
            &root,
            None,
            vec![message(&message_id, Some(&id(99)), None, None)],
        );
        assert!(matches!(
            plan_legacy_backup_group_sessions(source(json!([orphaned]), &characters, &group)),
            Err(LegacyBackupGroupSessionError::Orphan { ref field })
                if field == "[0].messages[0].speaker_character_id"
        ));

        orphaned = session(
            &root,
            &group,
            &characters,
            None,
            &root,
            None,
            vec![message(&message_id, Some(&characters[0]), None, None)],
        );
        orphaned["messages"][0]["usage_json"] = "{\"promptTokens\":-1}".into();
        assert!(matches!(
            plan_legacy_backup_group_sessions(source(json!([orphaned]), &characters, &group)),
            Err(LegacyBackupGroupSessionError::Malformed { ref field })
                if field == "[0].messages[0].usage_json.promptTokens"
        ));
    }
}
