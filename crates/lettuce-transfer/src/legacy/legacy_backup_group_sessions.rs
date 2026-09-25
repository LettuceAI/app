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
const TEXT_LIMIT: usize = 8 * 1024 * 1024;
const JSON_LIMIT: usize = 128 * 1024 * 1024;

#[derive(Debug)]
pub struct LegacyBackupGroupSessionPlan {
    pub sessions: Vec<LegacyBackupGroupSession>,
    pub skipped: Vec<crate::LegacyImportSkip>,
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
    /// The session's own starting scene when it chose one other than its
    /// group's (`Some(None)` when it chose none); `None` follows the group.
    pub starting_scene_override: Option<Option<crate::LegacyBackupSceneCandidate>>,
    pub background_image_locator: Option<String>,
    pub lorebook_source_ids: Vec<String>,
    pub lorebooks_overridden: bool,
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
    let mut skipped = Vec::new();
    let sessions = match document {
        Some(document) => {
            let rows: Vec<SessionRow> =
                serde_json::from_slice(&document.bytes).map_err(|_| malformed("$"))?;
            map_sessions(rows, &source, &mut skipped, &mut notices)?
        }
        None => {
            notices.push(notice(LegacyBackupConversionNoticeKind::Absent, "$"));
            Vec::new()
        }
    };
    notices.sort();
    notices.dedup();
    skipped.sort();
    skipped.dedup();
    Ok(LegacyBackupGroupSessionPlan {
        sessions,
        skipped,
        notices,
        source,
    })
}

/// Legacy `resolve_group_session_config`: a session linked to a group reads
/// each setting from its own `config_overrides`, else from the group, never
/// from its own columns. Values legacy 2.2.1's repair migration rewrote (a
/// string-encoded `startingScene`, a numeric `disableCharacterLorebooks`) are
/// read the way the repair left them.
/// Returns whether the session chose a starting scene other than the group's.
fn resolve_group_session_config(row: &mut SessionRow, group: &Value) -> bool {
    let overrides = serde_json::from_str::<Value>(&row.config_overrides)
        .ok()
        .and_then(|value| value.as_object().cloned());
    let value = |key: &str| {
        overrides
            .as_ref()
            .and_then(|object| object.get(key).cloned())
    };
    let text = |key: &str| group.get(key).and_then(Value::as_str).map(str::to_owned);
    let array = |override_value: Option<Value>, fallback: Option<String>| -> Vec<String> {
        override_value
            .and_then(|value| match value {
                Value::String(raw) => serde_json::from_str(&raw).ok(),
                value => serde_json::from_value(value).ok(),
            })
            .or_else(|| fallback.and_then(|raw| serde_json::from_str(&raw).ok()))
            .unwrap_or_default()
    };
    let members = array(value("characterIds"), text("character_ids"));
    let mut muted = array(value("mutedCharacterIds"), text("muted_character_ids"));
    muted.retain(|id| members.contains(id));
    row.character_ids = serde_json::to_string(&members).unwrap_or_else(|_| "[]".to_owned());
    row.muted_character_ids = serde_json::to_string(&muted).unwrap_or_else(|_| "[]".to_owned());
    row.persona_id = match value("personaId") {
        Some(value) => value.as_str().map(str::to_owned),
        None => text("persona_id"),
    };
    row.chat_type = match value("chatType") {
        Some(value) => value.as_str().unwrap_or("conversation").to_owned(),
        None => text("chat_type").unwrap_or_else(|| "conversation".to_owned()),
    };
    let group_scene = text("starting_scene")
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .filter(|value| !value.is_null());
    row.starting_scene = match value("startingScene") {
        Some(Value::Null) => None,
        Some(Value::String(encoded)) => serde_json::from_str::<Value>(&encoded)
            .ok()
            .filter(Value::is_object)
            .map(|value| value.to_string()),
        Some(value) => Some(value.to_string()),
        None => text("starting_scene")
            .filter(|raw| serde_json::from_str::<Value>(raw).is_ok_and(|value| !value.is_null())),
    };
    row.lorebook_ids = serde_json::to_string(&array(value("lorebookIds"), text("lorebook_ids")))
        .unwrap_or_else(|_| "[]".to_owned());
    row.disable_character_lorebooks = match value("disableCharacterLorebooks") {
        Some(Value::Bool(flag)) => flag,
        Some(Value::Number(flag)) => flag.as_i64().unwrap_or(0) != 0,
        Some(_) => false,
        None => match group.get("disable_character_lorebooks") {
            Some(Value::Bool(flag)) => *flag,
            Some(Value::Number(flag)) => flag.as_i64().unwrap_or(0) != 0,
            _ => false,
        },
    };
    row.speaker_selection_method = Some(match value("speakerSelectionMethod") {
        Some(value) => value.as_str().unwrap_or("llm").to_owned(),
        None => text("speaker_selection_method").unwrap_or_else(|| "llm".to_owned()),
    });
    row.memory_type = Some(match value("memoryType") {
        Some(value) => value.as_str().unwrap_or("manual").to_owned(),
        None => text("memory_type").unwrap_or_else(|| "manual".to_owned()),
    });
    let model_overrides: BTreeMap<String, String> = match value("characterModelOverrides") {
        Some(value) => serde_json::from_value(value).unwrap_or_default(),
        None => text("character_model_overrides")
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default(),
    };
    row.character_model_overrides =
        serde_json::to_string(&model_overrides).unwrap_or_else(|_| "{}".to_owned());
    row.group_chat_prompt_template_id = match value("groupChatPromptTemplateId") {
        Some(value) => value.as_str().map(str::to_owned),
        None => text("group_chat_prompt_template_id"),
    };
    row.group_chat_roleplay_prompt_template_id = match value("groupChatRoleplayPromptTemplateId") {
        Some(value) => value.as_str().map(str::to_owned),
        None => text("group_chat_roleplay_prompt_template_id"),
    };
    row.starting_scene
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        != group_scene
}

fn map_sessions(
    rows: Vec<SessionRow>,
    source: &LegacyBackupDirectSessionPlan,
    skipped: &mut Vec<crate::LegacyImportSkip>,
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
    let chat_model_ids = authored
        .configuration
        .provider_models
        .model_profiles
        .iter()
        .filter(|value| value.kind == lettuce_models::ModelKind::Chat)
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
    let group_rows = authored
        .configuration
        .source
        .documents
        .iter()
        .find(|document| document.kind == LegacyBackupDocumentKind::GroupCharacters)
        .and_then(|document| serde_json::from_slice::<Vec<Value>>(&document.bytes).ok())
        .unwrap_or_default();
    for (session_index, mut row) in rows.into_iter().enumerate() {
        let path = format!("[{session_index}]");
        let group = row.group_character_id.as_deref().and_then(|group_id| {
            group_rows
                .iter()
                .find(|group| group.get("id").and_then(Value::as_str) == Some(group_id))
        });
        let scene_overridden =
            group.is_some_and(|group| resolve_group_session_config(&mut row, group));
        let lorebooks_overridden = group.is_none()
            || serde_json::from_str::<Value>(&row.config_overrides)
                .ok()
                .is_some_and(|value| value.get("lorebookIds").is_some());
        let group_members = group.map(|group| {
            group
                .get("character_ids")
                .and_then(Value::as_str)
                .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok())
                .unwrap_or_default()
        });
        report_extra(&path, &row.extra, notices);
        validate_identifier(&row.id, &format!("{path}.id"))?;
        if !session_ids.insert(row.id.clone()) {
            return Err(malformed(format!("{path}.id")));
        }
        let mut group_source_id = row
            .group_character_id
            .clone()
            .or_else(|| contains_case_insensitive(&group_ids, &row.id).then(|| row.id.clone()));
        if group_source_id
            .as_ref()
            .is_none_or(|id| !contains_case_insensitive(&group_ids, id))
        {
            group_source_id = None;
            skipped.push(crate::LegacyImportSkip {
                kind: crate::LegacyImportSkipKind::GroupReference,
                source_key: format!("group_sessions.group_character_id:{}", row.id),
                reason: crate::LegacyImportSkipReason::MissingGroup,
            });
        }
        let members = string_array(&row.character_ids, &format!("{path}.character_ids"))?;
        if members.len() < 2 {
            return Err(malformed(format!("{path}.character_ids")));
        }
        for member in members
            .iter()
            .filter(|id| !contains_case_insensitive(&character_ids, id))
        {
            skipped.push(crate::LegacyImportSkip {
                kind: crate::LegacyImportSkipKind::CharacterReference,
                source_key: format!("group_sessions.character_ids:{}:{member}", row.id),
                reason: crate::LegacyImportSkipReason::MissingCharacter,
            });
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
        if row
            .persona_id
            .as_deref()
            .is_some_and(|persona_id| !contains_case_insensitive(&persona_ids, persona_id))
        {
            row.persona_id = None;
            skipped.push(crate::LegacyImportSkip {
                kind: crate::LegacyImportSkipKind::PersonaReference,
                source_key: format!("group_sessions.persona_id:{}", row.id),
                reason: crate::LegacyImportSkipReason::MissingPersona,
            });
        }
        let session_key = row.id.clone();
        let reference = |kind, key: String, reason| crate::LegacyImportSkip {
            kind,
            source_key: key,
            reason,
        };
        let group_conversation_prompt_source_id = row.group_chat_prompt_template_id.clone();
        let group_roleplay_prompt_source_id = row.group_chat_roleplay_prompt_template_id.clone();
        for (field, prompt) in [
            (
                "group_chat_prompt_template_id",
                &group_conversation_prompt_source_id,
            ),
            (
                "group_chat_roleplay_prompt_template_id",
                &group_roleplay_prompt_source_id,
            ),
        ] {
            if prompt
                .as_deref()
                .is_some_and(|prompt| !prompt_ids.contains(prompt))
            {
                skipped.push(reference(
                    crate::LegacyImportSkipKind::PromptReference,
                    format!("group_sessions.{field}:{session_key}"),
                    crate::LegacyImportSkipReason::MissingPrompt,
                ));
            }
        }
        let mut lorebooks = string_array(&row.lorebook_ids, &format!("{path}.lorebook_ids"))?;
        lorebooks.retain(|lorebook_id| {
            let present = contains_case_insensitive(&lorebook_ids, lorebook_id);
            if !present {
                skipped.push(reference(
                    crate::LegacyImportSkipKind::LorebookReference,
                    format!("group_sessions.lorebook_ids:{session_key}:{lorebook_id}"),
                    crate::LegacyImportSkipReason::MissingLorebook,
                ));
            }
            present
        });
        let mut overrides = string_map(
            &row.character_model_overrides,
            &format!("{path}.character_model_overrides"),
        )?;
        overrides.retain(|character_id, model_id| {
            let key =
                format!("group_sessions.character_model_overrides:{session_key}:{character_id}");
            if !contains_case_insensitive_slice(&members, character_id) {
                skipped.push(reference(
                    crate::LegacyImportSkipKind::CharacterReference,
                    key,
                    crate::LegacyImportSkipReason::MissingCharacter,
                ));
                return false;
            }
            *model_id = crate::legacy::legacy_backup_configuration::canonical_model_id(
                model_id,
                notices,
                "group_sessions.character_model_overrides",
            )
            .to_string();
            if !model_ids.contains(model_id.as_str()) {
                skipped.push(reference(
                    crate::LegacyImportSkipKind::ModelReference,
                    key,
                    crate::LegacyImportSkipReason::MissingModelProfile,
                ));
                return false;
            }
            if !chat_model_ids.contains(model_id.as_str()) {
                skipped.push(reference(
                    crate::LegacyImportSkipKind::ModelReference,
                    key,
                    crate::LegacyImportSkipReason::IncompatibleReference,
                ));
                return false;
            }
            if group_members
                .as_ref()
                .is_some_and(|group| !contains_case_insensitive_slice(group, character_id))
            {
                notices.push(LegacyBackupConversionNotice {
                    kind: LegacyBackupConversionNoticeKind::Lossy,
                    document: LegacyBackupDocumentKind::GroupSessions,
                    field: format!("{path}.config_overrides.characterModelOverrides"),
                });
            }
            true
        });
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
        let mut starting_scene_json = row.starting_scene.clone();
        if let Some(raw) = &starting_scene_json
            && validate_starting_scene(raw, &format!("{path}.starting_scene"), notices)?
        {
            starting_scene_json = Some(clear_starting_scene_variant(
                raw,
                &format!("{path}.starting_scene"),
            )?);
            skipped.push(reference(
                crate::LegacyImportSkipKind::SceneReference,
                format!("group_sessions.starting_scene.selected_variant_id:{session_key}"),
                crate::LegacyImportSkipReason::MissingSceneVariant,
            ));
        }
        let lossy = |field: &str| LegacyBackupConversionNotice {
            kind: LegacyBackupConversionNoticeKind::Lossy,
            document: LegacyBackupDocumentKind::GroupSessions,
            field: format!("{path}.config_overrides.{field}"),
        };
        let starting_scene_override = match scene_overridden
            .then(|| {
                crate::legacy::legacy_backup_authored::map_group_starting_scene(
                    starting_scene_json.as_deref().map(canonical_scene_ids),
                    &session_key,
                    &mut Vec::new(),
                    &mut Vec::new(),
                )
            })
            .transpose()
        {
            Ok(scene) => scene.map(|scene| {
                scene.map(|mut scene| {
                    if scene.background.take().is_some() {
                        notices.push(lossy("startingScene.backgroundImagePath"));
                    }
                    scene
                })
            }),
            Err(_) => {
                notices.push(lossy("startingScene"));
                Some(None)
            }
        };
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
            skipped,
            notices,
        )?;
        let (messages, message_skips) = map_messages(
            row.messages,
            &path,
            &members,
            &model_ids,
            &mut message_ids,
            &mut variant_ids,
            notices,
        )?;
        skipped.extend(message_skips);
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
            group_source_id,
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
            group_conversation_prompt_source_id,
            group_roleplay_prompt_source_id,
            starting_scene_json,
            starting_scene_override,
            background_image_locator: row.background_image_path,
            lorebook_source_ids: lorebooks,
            lorebooks_overridden,
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
    repair_session_graph(&mut sessions, skipped);
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
    skipped: &mut Vec<crate::LegacyImportSkip>,
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
                skipped.push(crate::LegacyImportSkip {
                    kind: crate::LegacyImportSkipKind::CharacterReference,
                    source_key: format!("group_participation.character_id:{}", row.id),
                    reason: crate::LegacyImportSkipReason::MissingCharacter,
                });
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
) -> Result<
    (Vec<LegacyBackupGroupMessage>, Vec<crate::LegacyImportSkip>),
    LegacyBackupGroupSessionError,
> {
    let mut skipped = Vec::new();
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
        if row
            .speaker_character_id
            .as_deref()
            .is_some_and(|speaker| !contains_case_insensitive_slice(members, speaker))
        {
            skipped.push(crate::LegacyImportSkip {
                kind: crate::LegacyImportSkipKind::CharacterReference,
                source_key: format!("group_messages.speaker_character_id:{}", row.id),
                reason: crate::LegacyImportSkipReason::MissingCharacter,
            });
        }
        let mut model_source_id = row.model_id.clone();
        if model_source_id
            .take_if(|id| !contains_case_insensitive(model_ids, id))
            .is_some()
        {
            skipped.push(crate::LegacyImportSkip {
                kind: crate::LegacyImportSkipKind::ModelReference,
                source_key: format!("group_messages.model_id:{}", row.id),
                reason: crate::LegacyImportSkipReason::MissingModelProfile,
            });
        }
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
            &mut skipped,
            notices,
        )?;
        let mut selected_variant_source_id = row.selected_variant_id.clone();
        if selected_variant_source_id
            .as_ref()
            .is_some_and(|id| !variants.iter().any(|variant| variant.source_id == *id))
        {
            selected_variant_source_id = variants.last().map(|variant| variant.source_id.clone());
            skipped.push(crate::LegacyImportSkip {
                kind: crate::LegacyImportSkipKind::MessageVariantReference,
                source_key: format!("group_messages.selected_variant_id:{}", row.id),
                reason: crate::LegacyImportSkipReason::MissingMessageVariant,
            });
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
            selected_variant_source_id,
            pinned: row.is_pinned,
            attachments_json: row.attachments,
            used_lorebook_entries_json: row.used_lorebook_entries,
            memory_refs_json: row.memory_refs,
            reasoning: row.reasoning,
            selection_reasoning: row.selection_reasoning,
            model_source_id,
            gemini_content_json: row.gemini_content,
            usage_json: row.usage_json,
            parent_message_source_id: row.parent_message_id,
            variants,
        });
    }
    validate_message_cycles(&messages, session_path)?;
    Ok((messages, skipped))
}

fn map_variants(
    rows: Vec<VariantRow>,
    message_path: &str,
    members: &[String],
    model_ids: &BTreeSet<String>,
    all_ids: &mut BTreeSet<String>,
    skipped: &mut Vec<crate::LegacyImportSkip>,
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
            if row
                .speaker_character_id
                .as_deref()
                .is_some_and(|speaker| !contains_case_insensitive_slice(members, speaker))
            {
                skipped.push(crate::LegacyImportSkip {
                    kind: crate::LegacyImportSkipKind::CharacterReference,
                    source_key: format!("group_message_variants.speaker_character_id:{}", row.id),
                    reason: crate::LegacyImportSkipReason::MissingCharacter,
                });
            }
            let mut model_source_id = row.model_id.clone();
            if model_source_id
                .take_if(|id| !contains_case_insensitive(model_ids, id))
                .is_some()
            {
                skipped.push(crate::LegacyImportSkip {
                    kind: crate::LegacyImportSkipKind::ModelReference,
                    source_key: format!("group_message_variants.model_id:{}", row.id),
                    reason: crate::LegacyImportSkipReason::MissingModelProfile,
                });
            }
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
                model_source_id,
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

fn repair_session_graph(
    sessions: &mut [LegacyBackupGroupSession],
    skipped: &mut Vec<crate::LegacyImportSkip>,
) {
    let mut links = sessions
        .iter()
        .map(
            |session| crate::legacy::legacy_backup_sessions::LegacySessionLinks {
                source_id: session.source_id.clone(),
                owner_source_id: session.group_source_id.clone(),
                parent_session_source_id: session.parent_session_source_id.clone(),
                root_session_source_id: session.root_session_source_id.clone(),
                branched_from_message_source_id: session.branched_from_message_source_id.clone(),
                message_source_ids: session
                    .messages
                    .iter()
                    .map(|message| message.source_id.clone())
                    .collect(),
            },
        )
        .collect::<Vec<_>>();
    crate::legacy::legacy_backup_sessions::repair_legacy_session_links(
        &mut links,
        "group_sessions",
        skipped,
    );
    for (session, links) in sessions.iter_mut().zip(links) {
        session.parent_session_source_id = links.parent_session_source_id;
        session.root_session_source_id = links.root_session_source_id;
        session.branched_from_message_source_id = links.branched_from_message_source_id;
    }
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

fn clear_starting_scene_variant(
    raw: &str,
    field: &str,
) -> Result<String, LegacyBackupGroupSessionError> {
    let mut value: Value = serde_json::from_str(raw).map_err(|_| malformed(field))?;
    if let Some(object) = value.as_object_mut() {
        for key in ["selected_variant_id", "selectedVariantId"] {
            if let Some(slot) = object.get_mut(key) {
                *slot = Value::Null;
            }
        }
    }
    serde_json::to_string(&value).map_err(|_| malformed(field))
}

/// Validates a group starting scene snapshot and reports whether its selected
/// variant no longer exists.
fn validate_starting_scene(
    raw: &str,
    field: &str,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<bool, LegacyBackupGroupSessionError> {
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
    Ok(row
        .selected_variant_id
        .as_deref()
        .is_some_and(|id| !variant_ids.contains(id)))
}

fn canonical_scene_ids(raw: &str) -> String {
    let Ok(mut scene) = serde_json::from_str::<Value>(raw) else {
        return raw.to_owned();
    };
    let canonical = |value: &mut Value, kind: &str| {
        if let Some(id) = value.as_str()
            && uuid::Uuid::parse_str(id).is_err()
        {
            *value = Value::String(
                uuid::Uuid::new_v5(
                    &crate::legacy::legacy_backup_configuration::LEGACY_ID_NAMESPACE,
                    format!("{kind}:{id}").as_bytes(),
                )
                .to_string(),
            );
        }
    };
    if let Some(id) = scene.get_mut("id") {
        canonical(id, "scene");
    }
    for key in ["selectedVariantId", "selected_variant_id"] {
        if let Some(id) = scene.get_mut(key) {
            canonical(id, "scene_variant");
        }
    }
    if let Some(variants) = scene.get_mut("variants").and_then(Value::as_array_mut) {
        for variant in variants {
            if let Some(id) = variant.get_mut("id") {
                canonical(id, "scene_variant");
            }
        }
    }
    scene.to_string()
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
        source_with(rows, characters, characters, group_id, Vec::new())
    }

    fn source_with(
        rows: Value,
        characters: &[String],
        group_members: &[String],
        group_id: &str,
        extra: Vec<LegacyBackupDocument>,
    ) -> LegacyBackupDirectSessionPlan {
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
            documents: [
                document(LegacyBackupDocumentKind::Characters, json!(character_rows)),
                document(
                    LegacyBackupDocumentKind::GroupCharacters,
                    json!([{
                        "id": group_id,
                        "name": "Writers Room",
                        "character_ids": serde_json::to_string(group_members).expect("members"),
                        "muted_character_ids": "[]",
                        "created_at": 1,
                        "updated_at": 1,
                        "speaker_selection_method": "director",
                        "memory_type": "dynamic"
                    }]),
                ),
                document(LegacyBackupDocumentKind::GroupSessions, rows),
            ]
            .into_iter()
            .chain(extra)
            .collect(),
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

    fn set_override(row: &mut Value, key: &str, value: Value) {
        let mut overrides: Value = serde_json::from_str(
            row["config_overrides"]
                .as_str()
                .expect("config overrides fixture"),
        )
        .expect("config overrides json");
        overrides[key] = value;
        row["config_overrides"] = json!(overrides.to_string());
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
            "config_overrides": json!({
                "version": 1,
                "temperature": 0.7,
                "mutedCharacterIds": &characters[1..],
                "chatType": "roleplay",
                "startingScene": scene,
                "disableCharacterLorebooks": true,
                "speakerSelectionMethod": "director_action",
                "memoryType": "dynamic"
            })
            .to_string(),
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
    fn linked_sessions_read_the_group_unless_overridden_like_legacy() {
        let characters = vec![id(80), id(81)];
        let group = id(82);
        let root = id(83);
        let mut row = session(
            &root,
            &group,
            &characters,
            None,
            &root,
            None,
            vec![message(&id(84), None, None, None)],
        );
        row["config_overrides"] = json!("{\"version\":1}");
        let plan =
            plan_legacy_backup_group_sessions(source(json!([row.clone()]), &characters, &group))
                .expect("group values");
        let session = &plan.sessions[0];
        assert_eq!(session.chat_mode, "conversation");
        assert_eq!(session.speaker_selection, "director");
        assert!(session.muted_member_source_ids.is_empty());
        assert!(session.starting_scene_json.is_none());
        assert!(!session.disable_character_lorebooks);
        assert!(!session.lorebooks_overridden);
        assert_eq!(session.starting_scene_override, None);

        let scene = row["starting_scene"].as_str().expect("scene").to_owned();
        set_override(&mut row, "startingScene", json!(scene));
        set_override(&mut row, "disableCharacterLorebooks", json!(1));
        set_override(&mut row, "personaId", json!(id(85)));
        let plan = plan_legacy_backup_group_sessions(source(json!([row]), &characters, &group))
            .expect("pre-2.2.1 override encodings and a deleted persona");
        let session = &plan.sessions[0];
        assert!(session.starting_scene_json.is_some());
        assert!(session.disable_character_lorebooks);
        assert_eq!(session.persona_source_id, None);
        let scene = session
            .starting_scene_override
            .as_ref()
            .expect("the session chose a scene")
            .as_ref()
            .expect("a scene, not none");
        assert_eq!(scene.background, None);
        assert_eq!(scene.variants.len(), 1);
        assert!(scene.selected_variant_id.is_some());
        assert!(plan.notices.iter().any(|notice| {
            notice.field == "[0].config_overrides.startingScene.backgroundImagePath"
        }));
        assert!(
            !plan
                .notices
                .iter()
                .any(|notice| notice.field == "[0].config_overrides.startingScene")
        );
        assert!(plan.skipped.iter().any(|skip| {
            skip.reason == crate::LegacyImportSkipReason::MissingPersona
                && skip.source_key == format!("group_sessions.persona_id:{root}")
        }));
    }

    #[test]
    fn session_model_overrides_map_legacy_model_ids_like_the_group() {
        let characters = vec![id(90), id(91), id(92)];
        let group_members = characters[..2].to_vec();
        let group = id(93);
        let root = id(94);
        let provider = id(95);
        let mut row = session(
            &root,
            &group,
            &group_members,
            None,
            &root,
            None,
            vec![message(&id(96), None, None, None)],
        );
        set_override(&mut row, "characterIds", json!(characters));
        set_override(
            &mut row,
            "characterModelOverrides",
            json!({ characters[0].clone(): "legacy-gpt", characters[2].clone(): "legacy-gpt" }),
        );
        let models = vec![
            document(
                LegacyBackupDocumentKind::ProviderCredentials,
                json!([{
                    "id": provider,
                    "provider_id": "openai",
                    "label": "OpenAI",
                    "api_key": "sk-test",
                    "config": "{}"
                }]),
            ),
            document(
                LegacyBackupDocumentKind::Models,
                json!([{
                    "id": "legacy-gpt",
                    "name": "gpt-4o",
                    "provider_id": "openai",
                    "provider_credential_id": provider,
                    "provider_label": "OpenAI",
                    "display_name": "GPT-4o",
                    "created_at": 1,
                    "model_type": "chat",
                    "input_scopes": "[\"text\"]",
                    "output_scopes": "[\"text\"]"
                }]),
            ),
        ];
        let plan = plan_legacy_backup_group_sessions(source_with(
            json!([row]),
            &characters,
            &group_members,
            &group,
            models,
        ))
        .expect("session overrides with a legacy model id");
        let model = crate::legacy::legacy_backup_configuration::canonical_model_id(
            "legacy-gpt",
            &mut Vec::new(),
            "model",
        )
        .to_string();
        let session = &plan.sessions[0];
        assert_eq!(
            session.character_model_overrides,
            BTreeMap::from([
                (characters[0].clone(), model.clone()),
                (characters[2].clone(), model),
            ])
        );
        assert!(plan.skipped.is_empty(), "{:?}", plan.skipped);
        assert!(plan.notices.iter().any(|notice| {
            notice.kind == LegacyBackupConversionNoticeKind::Lossy
                && notice.field == "[0].config_overrides.characterModelOverrides"
        }));
    }

    #[test]
    fn a_session_scene_with_legacy_non_uuid_ids_is_still_its_own() {
        let characters = vec![id(100), id(101)];
        let group = id(102);
        let root = id(103);
        let mut row = session(
            &root,
            &group,
            &characters,
            None,
            &root,
            None,
            vec![message(&id(104), None, None, None)],
        );
        set_override(
            &mut row,
            "startingScene",
            json!({
                "id": "1700000000000-0.5",
                "content": "A tavern",
                "createdAt": 1,
                "selectedVariantId": "1700000000001-0.5",
                "variants": [{"id": "1700000000001-0.5", "content": "A loud tavern", "createdAt": 2}]
            }),
        );
        let plan = plan_legacy_backup_group_sessions(source(json!([row]), &characters, &group))
            .expect("non-UUID scene ids");
        let scene = plan.sessions[0]
            .starting_scene_override
            .clone()
            .expect("own scene")
            .expect("a scene");
        assert_eq!(scene.selected_variant_id, Some(scene.variants[0].id));
        assert!(
            !plan
                .notices
                .iter()
                .any(|notice| notice.field == "[0].config_overrides.startingScene")
        );
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
        set_override(&mut row, "mutedCharacterIds", json!(characters));
        let plan = plan_legacy_backup_group_sessions(source(json!([row]), &characters, &group))
            .expect("director session without selected speaker");
        assert_eq!(plan.sessions[0].speaker_selection, "director_action");
        assert_eq!(plan.sessions[0].muted_member_source_ids, characters);
    }

    #[test]
    fn group_sessions_clear_and_record_stale_references() {
        let characters = vec![id(40), id(41)];
        let group = id(42);
        let root = id(43);
        let message_id = id(44);
        let variant_id = id(45);
        let missing_lorebook = id(46);
        let missing_model = id(47);
        let former_member = id(48);
        let missing_variant = id(49);
        let mut row = session(
            &root,
            &group,
            &characters,
            None,
            &root,
            None,
            vec![message(
                &message_id,
                Some(&characters[0]),
                None,
                Some(&variant_id),
            )],
        );
        set_override(
            &mut row,
            "groupChatPromptTemplateId",
            json!("deleted-prompt"),
        );
        set_override(&mut row, "lorebookIds", json!([missing_lorebook]));
        set_override(
            &mut row,
            "characterModelOverrides",
            json!({ characters[0].clone(): missing_model, former_member.clone(): missing_model }),
        );
        row["messages"][0]["model_id"] = json!(missing_model);
        row["messages"][0]["variants"][0]["model_id"] = json!(missing_model);
        row["messages"][0]["selected_variant_id"] = json!(missing_variant);
        let mut scene: Value = serde_json::from_str(
            row["starting_scene"]
                .as_str()
                .expect("starting scene fixture"),
        )
        .expect("starting scene json");
        scene["selectedVariantId"] = json!(id(50));
        set_override(&mut row, "startingScene", scene);
        let plan = plan_legacy_backup_group_sessions(source(json!([row]), &characters, &group))
            .expect("stale group session references are cleared");
        let session = &plan.sessions[0];
        assert_eq!(
            session.group_conversation_prompt_source_id.as_deref(),
            Some("deleted-prompt")
        );
        assert!(session.lorebook_source_ids.is_empty());
        assert!(session.lorebooks_overridden);
        assert!(session.character_model_overrides.is_empty());
        let message = &session.messages[0];
        assert_eq!(message.model_source_id, None);
        assert_eq!(message.variants[0].model_source_id, None);
        assert_eq!(
            message.selected_variant_source_id.as_deref(),
            Some(variant_id.as_str())
        );
        let scene: Value = serde_json::from_str(
            session
                .starting_scene_json
                .as_deref()
                .expect("starting scene kept"),
        )
        .expect("starting scene json");
        assert!(scene["selectedVariantId"].is_null());
        let skip = |kind, source_key: String, reason| crate::LegacyImportSkip {
            kind,
            source_key,
            reason,
        };
        let mut expected = vec![
            skip(
                crate::LegacyImportSkipKind::PromptReference,
                format!("group_sessions.group_chat_prompt_template_id:{root}"),
                crate::LegacyImportSkipReason::MissingPrompt,
            ),
            skip(
                crate::LegacyImportSkipKind::LorebookReference,
                format!("group_sessions.lorebook_ids:{root}:{missing_lorebook}"),
                crate::LegacyImportSkipReason::MissingLorebook,
            ),
            skip(
                crate::LegacyImportSkipKind::ModelReference,
                format!(
                    "group_sessions.character_model_overrides:{root}:{}",
                    characters[0]
                ),
                crate::LegacyImportSkipReason::MissingModelProfile,
            ),
            skip(
                crate::LegacyImportSkipKind::CharacterReference,
                format!("group_sessions.character_model_overrides:{root}:{former_member}"),
                crate::LegacyImportSkipReason::MissingCharacter,
            ),
            skip(
                crate::LegacyImportSkipKind::SceneReference,
                format!("group_sessions.starting_scene.selected_variant_id:{root}"),
                crate::LegacyImportSkipReason::MissingSceneVariant,
            ),
            skip(
                crate::LegacyImportSkipKind::ModelReference,
                format!("group_messages.model_id:{message_id}"),
                crate::LegacyImportSkipReason::MissingModelProfile,
            ),
            skip(
                crate::LegacyImportSkipKind::ModelReference,
                format!("group_message_variants.model_id:{variant_id}"),
                crate::LegacyImportSkipReason::MissingModelProfile,
            ),
            skip(
                crate::LegacyImportSkipKind::MessageVariantReference,
                format!("group_messages.selected_variant_id:{message_id}"),
                crate::LegacyImportSkipReason::MissingMessageVariant,
            ),
        ];
        expected.sort();
        assert_eq!(plan.skipped, expected);
    }

    #[test]
    fn group_sessions_keep_deleted_members_as_unknown_participants() {
        let characters = vec![id(60), id(61)];
        let deleted = id(62);
        let group = id(63);
        let root = id(64);
        let message_id = id(65);
        let members = vec![
            characters[0].clone(),
            characters[1].clone(),
            deleted.clone(),
        ];
        let mut row = session(
            &root,
            &group,
            &members,
            None,
            &root,
            None,
            vec![message(&message_id, Some(&deleted), None, None)],
        );
        set_override(&mut row, "characterIds", json!(members));
        let plan = plan_legacy_backup_group_sessions(source(json!([row]), &characters, &group))
            .expect("deleted members are kept");
        assert_eq!(plan.sessions[0].member_source_ids, members);
        assert_eq!(
            plan.sessions[0].messages[0]
                .speaker_character_source_id
                .as_deref(),
            Some(deleted.as_str())
        );
        assert_eq!(
            plan.skipped,
            vec![crate::LegacyImportSkip {
                kind: crate::LegacyImportSkipKind::CharacterReference,
                source_key: format!("group_sessions.character_ids:{root}:{deleted}"),
                reason: crate::LegacyImportSkipReason::MissingCharacter,
            }]
        );
    }

    #[test]
    fn unlinked_group_sessions_link_the_group_legacy_created_for_them() {
        let characters = vec![id(80), id(81)];
        let group = id(82);
        let root = id(83);
        let mut row = session(&root, &group, &characters, None, &root, None, Vec::new());
        row["group_character_id"] = Value::Null;
        row["background_image_path"] = Value::Null;
        row["starting_scene"] = Value::Null;
        let plan = plan_legacy_backup_group_sessions(source(json!([row]), &characters, &group))
            .expect("an unlinked session gets its own group");
        assert_eq!(
            plan.sessions[0].group_source_id.as_deref(),
            Some(root.as_str())
        );
        assert!(plan.skipped.is_empty());
    }

    #[test]
    fn group_sessions_clear_links_to_missing_groups() {
        let characters = vec![id(90), id(91)];
        let group = id(92);
        let root = id(93);
        let mut row = session(&root, &group, &characters, None, &root, None, Vec::new());
        row["group_character_id"] = json!(id(94));
        let plan = plan_legacy_backup_group_sessions(source(json!([row]), &characters, &group))
            .expect("a missing group link is cleared");
        assert_eq!(plan.sessions[0].group_source_id, None);
        assert_eq!(
            plan.skipped,
            vec![crate::LegacyImportSkip {
                kind: crate::LegacyImportSkipKind::GroupReference,
                source_key: format!("group_sessions.group_character_id:{root}"),
                reason: crate::LegacyImportSkipReason::MissingGroup,
            }]
        );
    }

    #[test]
    fn group_sessions_keep_participation_of_removed_members() {
        let characters = vec![id(70), id(71)];
        let removed = id(72);
        let group = id(73);
        let root = id(74);
        let mut row = session(&root, &group, &characters, None, &root, None, Vec::new());
        row["participation"]
            .as_array_mut()
            .expect("participation fixture")
            .push(json!({
                "id": "removed-participation",
                "character_id": removed,
                "speak_count": 3,
                "last_spoke_turn": 1,
                "last_spoke_at": 5
            }));
        let plan = plan_legacy_backup_group_sessions(source(json!([row]), &characters, &group))
            .expect("participation of a removed member is kept");
        assert_eq!(plan.sessions[0].participation.len(), 3);
        assert_eq!(
            plan.skipped,
            vec![crate::LegacyImportSkip {
                kind: crate::LegacyImportSkipKind::CharacterReference,
                source_key: "group_participation.character_id:removed-participation".to_owned(),
                reason: crate::LegacyImportSkipReason::MissingCharacter,
            }]
        );
    }

    #[test]
    fn group_branches_of_a_deleted_parent_are_kept_as_their_own_root() {
        let characters = vec![id(40), id(41)];
        let group = id(42);
        let branch = id(43);
        let deleted_parent = id(44);
        let branch_message = id(45);
        let rows = json!([session(
            &branch,
            &group,
            &characters,
            Some(&deleted_parent),
            &deleted_parent,
            Some(&branch_message),
            vec![message(&branch_message, Some(&characters[0]), None, None)],
        )]);
        let plan = plan_legacy_backup_group_sessions(source(rows, &characters, &group))
            .expect("a branch whose parent was deleted is imported");
        let session = &plan.sessions[0];
        assert_eq!(session.parent_session_source_id, None);
        assert_eq!(session.root_session_source_id, branch);
        assert_eq!(
            session.branched_from_message_source_id.as_deref(),
            Some(branch_message.as_str())
        );
        assert_eq!(
            plan.skipped,
            vec![
                crate::LegacyImportSkip {
                    kind: crate::LegacyImportSkipKind::SessionLink,
                    source_key: format!("group_sessions.parent_session_id:{branch}"),
                    reason: crate::LegacyImportSkipReason::MissingSession,
                },
                crate::LegacyImportSkip {
                    kind: crate::LegacyImportSkipKind::SessionLink,
                    source_key: format!("group_sessions.root_session_id:{branch}"),
                    reason: crate::LegacyImportSkipReason::MissingSession,
                },
            ]
        );
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
        let plan =
            plan_legacy_backup_group_sessions(source(json!([orphaned]), &characters, &group))
                .expect("a non-member speaker is kept");
        assert_eq!(
            plan.skipped,
            vec![crate::LegacyImportSkip {
                kind: crate::LegacyImportSkipKind::CharacterReference,
                source_key: format!("group_messages.speaker_character_id:{message_id}"),
                reason: crate::LegacyImportSkipReason::MissingCharacter,
            }]
        );

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
