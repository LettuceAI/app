use std::collections::{BTreeMap, BTreeSet};

use lettuce_characters::InteractionMode;
use lettuce_companions::{RelationshipState, SoulFact, SoulState, validate_state};
use lettuce_types::{CharacterId, PersonaId, Revision};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    LegacyBackupConversionNotice, LegacyBackupConversionNoticeKind, LegacyBackupDocumentKind,
    LegacyBackupScheduledNotePlan,
};

const STATE_LIMIT: usize = 10_000;
const EPISODE_LIMIT: usize = 100_000;
const JSON_LIMIT: usize = 16 * 1024 * 1024;
const TEXT_LIMIT: usize = 1_000_000;

#[derive(Debug)]
pub struct LegacyBackupCompanionSharedMemoryPlan {
    pub states: Vec<LegacyBackupCompanionSharedMemory>,
    pub notices: Vec<LegacyBackupConversionNotice>,
    pub source: LegacyBackupScheduledNotePlan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyBackupCompanionSharedMemory {
    pub ordinal: u64,
    pub character_id: CharacterId,
    pub memories_json: String,
    pub memory_embeddings_json: String,
    pub memory_summary: Option<String>,
    pub memory_summary_token_count: u64,
    pub memory_tool_events_json: String,
    pub memory_status: Option<String>,
    pub memory_error: Option<String>,
    pub memory_progress_step: Option<u64>,
    pub soul_growth_json: String,
    pub soul_facts: Option<Vec<SoulFact>>,
    pub relationship_states_json: String,
    pub relationship_states: Vec<LegacyBackupRelationshipState>,
    pub episodes: Vec<LegacyBackupCompanionEpisode>,
    pub created_at: u64,
    pub updated_at: u64,
    pub memory_materialization: LegacyBackupCompanionMaterialization,
    pub soul_materialization: LegacyBackupCompanionMaterialization,
    pub relationship_materialization: LegacyBackupCompanionMaterialization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyBackupCompanionMaterialization {
    ExactInitialSnapshot,
    RetainedEvidence,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyBackupRelationshipState {
    pub persona_id: Option<PersonaId>,
    pub state: RelationshipState,
    pub conversation_source_ids: Vec<String>,
    pub materialization: LegacyBackupCompanionMaterialization,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyBackupCompanionEpisode {
    pub ordinal: u64,
    pub conversation_source_id: String,
    pub persona_id: Option<PersonaId>,
    pub episode_index: u32,
    pub previous_conversation_source_id: Option<String>,
    pub started_at: u64,
    pub ended_at: Option<u64>,
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LegacyBackupCompanionSharedMemoryError {
    #[error("legacy backup companion shared-memory document is malformed")]
    Malformed { field: String },
    #[error("legacy backup companion shared-memory document exceeds its record limit")]
    LimitExceeded,
    #[error("legacy backup companion shared-memory graph contains an orphaned link")]
    Orphan { field: String },
}

#[derive(Deserialize)]
struct StateRow {
    character_id: String,
    memories: String,
    #[serde(default = "empty_array")]
    memory_embeddings: String,
    memory_summary: Option<String>,
    memory_summary_token_count: i64,
    memory_tool_events: String,
    memory_status: Option<String>,
    memory_error: Option<String>,
    memory_progress_step: Option<i64>,
    soul_growth: String,
    relationship_states: String,
    #[serde(default)]
    episodes: Vec<EpisodeRow>,
    created_at: i64,
    updated_at: i64,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct EpisodeRow {
    session_id: String,
    #[serde(default = "default_persona_key")]
    persona_key: String,
    #[serde(default = "first_episode")]
    episode_index: i64,
    previous_session_id: Option<String>,
    #[serde(default)]
    started_at: i64,
    ended_at: Option<i64>,
    #[serde(default)]
    updated_at: i64,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LegacyRelationshipState {
    closeness: f64,
    trust: f64,
    affection: f64,
    tension: f64,
    stability: f64,
    interaction_count: u32,
    last_interaction_at: i64,
}

pub fn plan_legacy_backup_companion_shared_memory(
    source: LegacyBackupScheduledNotePlan,
) -> Result<LegacyBackupCompanionSharedMemoryPlan, LegacyBackupCompanionSharedMemoryError> {
    let document = source
        .source
        .source
        .source
        .source
        .source
        .source
        .authored
        .configuration
        .source
        .documents
        .iter()
        .find(|document| document.kind == LegacyBackupDocumentKind::CompanionSharedMemory);
    let mut notices = source.notices.clone();
    let states = match document {
        Some(document) => {
            let rows: Vec<StateRow> =
                serde_json::from_slice(&document.bytes).map_err(|_| malformed("$"))?;
            map_states(rows, &source, &mut notices)?
        }
        None => {
            notices.push(notice(LegacyBackupConversionNoticeKind::Absent, "$"));
            Vec::new()
        }
    };
    notices.sort();
    notices.dedup();
    Ok(LegacyBackupCompanionSharedMemoryPlan {
        states,
        notices,
        source,
    })
}

fn map_states(
    rows: Vec<StateRow>,
    source: &LegacyBackupScheduledNotePlan,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<LegacyBackupCompanionSharedMemory>, LegacyBackupCompanionSharedMemoryError> {
    if rows.len() > STATE_LIMIT {
        return Err(LegacyBackupCompanionSharedMemoryError::LimitExceeded);
    }
    let direct = &source.source.source;
    let authored = &direct.source.source.source.source.authored;
    let companion_ids = authored
        .characters
        .iter()
        .filter(|character| character.defaults.interaction_mode == InteractionMode::Companion)
        .map(|character| character.id)
        .collect::<BTreeSet<_>>();
    let all_character_ids = authored
        .characters
        .iter()
        .map(|character| character.id)
        .collect::<BTreeSet<_>>();
    let persona_ids = authored
        .personas
        .personas
        .iter()
        .map(|persona| persona.id)
        .collect::<BTreeSet<_>>();
    let direct_sessions = direct
        .sessions
        .iter()
        .map(|session| {
            (
                session.source_id.as_str(),
                session.character_source_id.as_str(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut owners = BTreeSet::new();
    let mut episode_count = 0usize;
    let mut states = Vec::with_capacity(rows.len());
    for (index, row) in rows.into_iter().enumerate() {
        let path = format!("[{index}]");
        report_extra(&path, &row.extra, notices);
        let character_id = row
            .character_id
            .parse::<CharacterId>()
            .map_err(|_| malformed(format!("{path}.character_id")))?;
        if !owners.insert(character_id) {
            return Err(malformed(format!("{path}.character_id")));
        }
        if !all_character_ids.contains(&character_id) {
            return Err(orphan(format!("{path}.character_id")));
        }
        if !companion_ids.contains(&character_id) {
            return Err(malformed(format!("{path}.character_id")));
        }
        validate_string_array(&row.memories, &format!("{path}.memories"))?;
        validate_json_array(&row.memory_embeddings, &format!("{path}.memory_embeddings"))?;
        validate_json_array(
            &row.memory_tool_events,
            &format!("{path}.memory_tool_events"),
        )?;
        validate_text(
            row.memory_summary.as_deref(),
            &format!("{path}.memory_summary"),
        )?;
        validate_text(
            row.memory_status.as_deref(),
            &format!("{path}.memory_status"),
        )?;
        validate_text(row.memory_error.as_deref(), &format!("{path}.memory_error"))?;
        let soul_value = validate_json_array(&row.soul_growth, &format!("{path}.soul_growth"))?;
        validate_soul_shape(&soul_value, &format!("{path}.soul_growth"))?;
        let soul_facts = exact_soul_facts(&soul_value);
        let relationship_value = validate_json_object(
            &row.relationship_states,
            &format!("{path}.relationship_states"),
        )?;
        let mut relationship_states = map_relationships(
            &relationship_value,
            &persona_ids,
            &format!("{path}.relationship_states"),
        )?;
        episode_count = episode_count
            .checked_add(row.episodes.len())
            .ok_or(LegacyBackupCompanionSharedMemoryError::LimitExceeded)?;
        if episode_count > EPISODE_LIMIT {
            return Err(LegacyBackupCompanionSharedMemoryError::LimitExceeded);
        }
        let episodes = map_episodes(
            row.episodes,
            character_id,
            &persona_ids,
            &direct_sessions,
            &path,
            notices,
        )?;
        for relationship in &mut relationship_states {
            relationship.conversation_source_ids = episodes
                .iter()
                .filter(|episode| episode.persona_id == relationship.persona_id)
                .map(|episode| episode.conversation_source_id.clone())
                .collect();
            relationship.materialization = if relationship.conversation_source_ids.is_empty() {
                LegacyBackupCompanionMaterialization::RetainedEvidence
            } else {
                LegacyBackupCompanionMaterialization::ExactInitialSnapshot
            };
        }
        let created_at = count(row.created_at, &format!("{path}.created_at"))?;
        let updated_at = count(row.updated_at, &format!("{path}.updated_at"))?;
        if created_at > updated_at {
            return Err(malformed(format!("{path}.updated_at")));
        }
        let soul_materialization = if soul_facts.is_some() {
            LegacyBackupCompanionMaterialization::ExactInitialSnapshot
        } else {
            LegacyBackupCompanionMaterialization::RetainedEvidence
        };
        let relationship_materialization = if relationship_states.iter().all(|relationship| {
            relationship.materialization
                == LegacyBackupCompanionMaterialization::ExactInitialSnapshot
        }) {
            LegacyBackupCompanionMaterialization::ExactInitialSnapshot
        } else {
            LegacyBackupCompanionMaterialization::RetainedEvidence
        };
        states.push(LegacyBackupCompanionSharedMemory {
            ordinal: u64::try_from(index)
                .map_err(|_| LegacyBackupCompanionSharedMemoryError::LimitExceeded)?,
            character_id,
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
            memory_progress_step: row
                .memory_progress_step
                .map(|value| count(value, &format!("{path}.memory_progress_step")))
                .transpose()?,
            soul_growth_json: row.soul_growth,
            soul_facts,
            relationship_states_json: row.relationship_states,
            relationship_states,
            episodes,
            created_at,
            updated_at,
            memory_materialization: LegacyBackupCompanionMaterialization::RetainedEvidence,
            soul_materialization,
            relationship_materialization,
        });
    }
    Ok(states)
}

fn map_relationships(
    value: &Value,
    persona_ids: &BTreeSet<PersonaId>,
    path: &str,
) -> Result<Vec<LegacyBackupRelationshipState>, LegacyBackupCompanionSharedMemoryError> {
    let object = value.as_object().ok_or_else(|| malformed(path))?;
    let mut relationships = Vec::with_capacity(object.len());
    for (key, value) in object {
        let field = format!("{path}.{key}");
        let persona_id = if key == "__default__" {
            None
        } else {
            let id = key.parse::<PersonaId>().map_err(|_| malformed(&field))?;
            if !persona_ids.contains(&id) {
                return Err(orphan(field));
            }
            Some(id)
        };
        let legacy: LegacyRelationshipState =
            serde_json::from_value(value.clone()).map_err(|_| malformed(&field))?;
        let state = RelationshipState {
            closeness: legacy.closeness,
            trust: legacy.trust,
            affection: legacy.affection,
            tension: legacy.tension,
            stability: legacy.stability,
            interaction_count: legacy.interaction_count,
            last_interaction_at: lettuce_types::TimestampMillis::new(legacy.last_interaction_at),
        };
        if !valid_relationship(&state) {
            return Err(malformed(field));
        }
        relationships.push(LegacyBackupRelationshipState {
            persona_id,
            state,
            conversation_source_ids: Vec::new(),
            materialization: LegacyBackupCompanionMaterialization::RetainedEvidence,
        });
    }
    Ok(relationships)
}

fn map_episodes(
    rows: Vec<EpisodeRow>,
    character_id: CharacterId,
    persona_ids: &BTreeSet<PersonaId>,
    sessions: &BTreeMap<&str, &str>,
    parent_path: &str,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<LegacyBackupCompanionEpisode>, LegacyBackupCompanionSharedMemoryError> {
    let character_source_id = character_id.to_string();
    let mut identities = BTreeSet::new();
    let mut session_ids = BTreeSet::new();
    let mut episodes = Vec::with_capacity(rows.len());
    for (index, row) in rows.into_iter().enumerate() {
        let path = format!("{parent_path}.episodes[{index}]");
        report_extra(&path, &row.extra, notices);
        validate_identifier(&row.session_id, &format!("{path}.session_id"))?;
        if !session_ids.insert(row.session_id.clone()) {
            return Err(malformed(format!("{path}.session_id")));
        }
        if sessions.get(row.session_id.as_str()).copied() != Some(character_source_id.as_str()) {
            return Err(orphan(format!("{path}.session_id")));
        }
        let persona_id = if row.persona_key == "__default__" || row.persona_key.trim().is_empty() {
            None
        } else {
            let id = row
                .persona_key
                .parse::<PersonaId>()
                .map_err(|_| malformed(format!("{path}.persona_key")))?;
            if !persona_ids.contains(&id) {
                return Err(orphan(format!("{path}.persona_key")));
            }
            Some(id)
        };
        let episode_index = u32::try_from(row.episode_index)
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| malformed(format!("{path}.episode_index")))?;
        if !identities.insert((persona_id, episode_index)) {
            return Err(malformed(format!("{path}.episode_index")));
        }
        if let Some(previous) = row.previous_session_id.as_deref() {
            validate_identifier(previous, &format!("{path}.previous_session_id"))?;
            if sessions.get(previous).copied() != Some(character_source_id.as_str()) {
                return Err(orphan(format!("{path}.previous_session_id")));
            }
        }
        let started_at = count(row.started_at, &format!("{path}.started_at"))?;
        let ended_at = row
            .ended_at
            .map(|value| count(value, &format!("{path}.ended_at")))
            .transpose()?;
        let updated_at = count(row.updated_at, &format!("{path}.updated_at"))?;
        if ended_at.is_some_and(|value| value < started_at) || updated_at < started_at {
            return Err(malformed(format!("{path}.timestamps")));
        }
        episodes.push(LegacyBackupCompanionEpisode {
            ordinal: u64::try_from(index)
                .map_err(|_| LegacyBackupCompanionSharedMemoryError::LimitExceeded)?,
            conversation_source_id: row.session_id,
            persona_id,
            episode_index,
            previous_conversation_source_id: row.previous_session_id,
            started_at,
            ended_at,
            updated_at,
        });
    }
    let by_session = episodes
        .iter()
        .map(|episode| (episode.conversation_source_id.as_str(), episode))
        .collect::<BTreeMap<_, _>>();
    for (index, episode) in episodes.iter().enumerate() {
        let Some(previous_id) = episode.previous_conversation_source_id.as_deref() else {
            continue;
        };
        let Some(previous) = by_session.get(previous_id) else {
            return Err(orphan(format!(
                "{parent_path}.episodes[{index}].previous_session_id"
            )));
        };
        if previous.persona_id != episode.persona_id
            || previous.episode_index >= episode.episode_index
        {
            return Err(malformed(format!(
                "{parent_path}.episodes[{index}].previous_session_id"
            )));
        }
    }
    Ok(episodes)
}

fn exact_soul_facts(value: &Value) -> Option<Vec<SoulFact>> {
    let facts = serde_json::from_value::<Vec<SoulFact>>(value.clone()).ok()?;
    let state = SoulState {
        revision: Revision::INITIAL,
        facts: facts.clone(),
    };
    validate_state(&state).ok().map(|()| facts)
}

fn validate_soul_shape(
    value: &Value,
    path: &str,
) -> Result<(), LegacyBackupCompanionSharedMemoryError> {
    for (index, item) in value
        .as_array()
        .ok_or_else(|| malformed(path))?
        .iter()
        .enumerate()
    {
        let field = format!("{path}[{index}]");
        let object = item.as_object().ok_or_else(|| malformed(&field))?;
        for key in [
            "id",
            "category",
            "value",
            "kind",
            "policy",
            "slot",
            "supersededBy",
        ] {
            if object
                .get(key)
                .is_some_and(|value| !value.is_null() && value.as_str().is_none())
            {
                return Err(malformed(format!("{field}.{key}")));
            }
        }
        for key in ["confidence", "weight"] {
            if object
                .get(key)
                .is_some_and(|value| !value.as_f64().is_some_and(|number| number.is_finite()))
            {
                return Err(malformed(format!("{field}.{key}")));
            }
        }
        for key in [
            "evidenceCount",
            "validFrom",
            "validUntil",
            "createdAt",
            "supersededAt",
        ] {
            if object
                .get(key)
                .is_some_and(|value| !value.is_null() && value.as_u64().is_none())
            {
                return Err(malformed(format!("{field}.{key}")));
            }
        }
        for key in ["sourceMemoryIds", "supersedes"] {
            if let Some(value) = object.get(key) {
                if serde_json::from_value::<Vec<String>>(value.clone()).is_err() {
                    return Err(malformed(format!("{field}.{key}")));
                }
            }
        }
        if object
            .get("locked")
            .is_some_and(|value| value.as_bool().is_none())
        {
            return Err(malformed(format!("{field}.locked")));
        }
    }
    Ok(())
}

fn valid_relationship(state: &RelationshipState) -> bool {
    [state.closeness, state.trust, state.affection]
        .into_iter()
        .all(|value| value.is_finite() && (-1.0..=1.0).contains(&value))
        && [state.tension, state.stability]
            .into_iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(&value))
        && state.last_interaction_at.get() >= 0
}

fn validate_string_array(
    raw: &str,
    field: &str,
) -> Result<(), LegacyBackupCompanionSharedMemoryError> {
    if raw.len() > JSON_LIMIT {
        return Err(LegacyBackupCompanionSharedMemoryError::LimitExceeded);
    }
    let values: Vec<String> = serde_json::from_str(raw).map_err(|_| malformed(field))?;
    if values
        .iter()
        .any(|value| value.chars().count() > TEXT_LIMIT || value.contains('\0'))
    {
        return Err(malformed(field));
    }
    Ok(())
}

fn validate_json_array(
    raw: &str,
    field: &str,
) -> Result<Value, LegacyBackupCompanionSharedMemoryError> {
    validate_json(raw, field, true)
}

fn validate_json_object(
    raw: &str,
    field: &str,
) -> Result<Value, LegacyBackupCompanionSharedMemoryError> {
    validate_json(raw, field, false)
}

fn validate_json(
    raw: &str,
    field: &str,
    array: bool,
) -> Result<Value, LegacyBackupCompanionSharedMemoryError> {
    if raw.len() > JSON_LIMIT {
        return Err(LegacyBackupCompanionSharedMemoryError::LimitExceeded);
    }
    let value: Value = serde_json::from_str(raw).map_err(|_| malformed(field))?;
    if (array && !value.is_array()) || (!array && !value.is_object()) {
        return Err(malformed(field));
    }
    Ok(value)
}

fn validate_text(
    value: Option<&str>,
    field: &str,
) -> Result<(), LegacyBackupCompanionSharedMemoryError> {
    if value.is_some_and(|value| value.chars().count() > TEXT_LIMIT || value.contains('\0')) {
        Err(malformed(field))
    } else {
        Ok(())
    }
}

fn validate_identifier(
    value: &str,
    field: &str,
) -> Result<(), LegacyBackupCompanionSharedMemoryError> {
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

fn count(value: i64, field: &str) -> Result<u64, LegacyBackupCompanionSharedMemoryError> {
    u64::try_from(value).map_err(|_| malformed(field))
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
        document: LegacyBackupDocumentKind::CompanionSharedMemory,
        field: field.to_owned(),
    }
}

fn malformed(field: impl Into<String>) -> LegacyBackupCompanionSharedMemoryError {
    LegacyBackupCompanionSharedMemoryError::Malformed {
        field: field.into(),
    }
}

fn orphan(field: impl Into<String>) -> LegacyBackupCompanionSharedMemoryError {
    LegacyBackupCompanionSharedMemoryError::Orphan {
        field: field.into(),
    }
}

fn empty_array() -> String {
    "[]".to_owned()
}

fn default_persona_key() -> String {
    "__default__".to_owned()
}

const fn first_episode() -> i64 {
    1
}

#[cfg(test)]
mod tests {
    use lettuce_types::ContentHash;
    use serde_json::{Value, json};
    use uuid::Uuid;
    use zeroize::Zeroizing;

    use super::*;
    use crate::{
        LegacyBackupDocument, LegacyBackupInventory, plan_legacy_backup_asr,
        plan_legacy_backup_authored, plan_legacy_backup_authored_media,
        plan_legacy_backup_configuration, plan_legacy_backup_direct_sessions,
        plan_legacy_backup_group_sessions, plan_legacy_backup_pricing,
        plan_legacy_backup_scheduled_notes, plan_legacy_backup_usage,
    };

    fn id(value: u128) -> String {
        Uuid::from_u128(value).to_string()
    }

    fn document(kind: LegacyBackupDocumentKind, value: Value) -> LegacyBackupDocument {
        LegacyBackupDocument {
            kind,
            bytes: Zeroizing::new(serde_json::to_vec(&value).expect("fixture document")),
        }
    }

    fn direct_session(session_id: &str, character_id: &str) -> Value {
        json!({
            "id": session_id,
            "character_id": character_id,
            "title": "Conversation",
            "parent_session_id": null,
            "branched_from_message_id": null,
            "root_session_id": session_id,
            "background_image_path": null,
            "system_prompt": null,
            "mode": "companion",
            "selected_scene_id": null,
            "author_note": null,
            "persona_id": null,
            "persona_disabled": false,
            "voice_autoplay": false,
            "prompt_template_id": null,
            "lorebook_ids_override": null,
            "temperature": null,
            "top_p": null,
            "max_output_tokens": null,
            "frequency_penalty": null,
            "presence_penalty": null,
            "top_k": null,
            "advanced_model_settings": null,
            "companion_state": null,
            "memories": "[]",
            "memory_embeddings": "[]",
            "memory_summary": null,
            "memory_summary_token_count": 0,
            "memory_tool_events": "[]",
            "memory_status": "idle",
            "memory_error": null,
            "memory_progress_step": null,
            "archived": false,
            "created_at": 1,
            "updated_at": 2,
            "messages": []
        })
    }

    fn shared_state(character_id: &str, session_id: &str) -> Value {
        json!({
            "character_id": character_id,
            "memories": "[\" Remember the garden. \"]",
            "memory_embeddings": "[{\"id\":\"memory-1\",\"text\":\"Garden\",\"embedding\":[0.5]}]",
            "memory_summary": " A shared summary ",
            "memory_summary_token_count": 4,
            "memory_tool_events": "[{\"id\":\"cycle-1\",\"status\":\"complete\"}]",
            "memory_status": "idle",
            "memory_error": null,
            "memory_progress_step": 2,
            "soul_growth": serde_json::to_string(&json!([{
                "id": "fact-1",
                "category": "traits",
                "value": "Patient",
                "kind": "add",
                "policy": "adaptive",
                "slot": "temperament",
                "confidence": 0.9,
                "evidenceCount": 2,
                "weight": 0.8,
                "validFrom": 10,
                "validUntil": null,
                "locked": false,
                "sourceMemoryIds": ["memory-1"],
                "createdAt": 10,
                "supersedes": [],
                "supersededBy": null,
                "supersededAt": null
            }])).expect("soul JSON"),
            "relationship_states": serde_json::to_string(&json!({
                "__default__": {
                    "closeness": 0.4,
                    "trust": 0.5,
                    "affection": 0.3,
                    "tension": 0.1,
                    "stability": 0.8,
                    "interactionCount": 7,
                    "lastInteractionAt": 20
                }
            })).expect("relationship JSON"),
            "episodes": [{
                "session_id": session_id,
                "persona_key": "__default__",
                "episode_index": 1,
                "previous_session_id": null,
                "started_at": 10,
                "ended_at": null,
                "updated_at": 20
            }],
            "created_at": 1,
            "updated_at": 20
        })
    }

    fn source(
        shared: Option<Value>,
        character_id: &str,
        session_id: &str,
        companion: bool,
    ) -> LegacyBackupScheduledNotePlan {
        let mut documents = vec![
            document(
                LegacyBackupDocumentKind::Characters,
                json!([{
                    "id": character_id,
                    "name": "Mira",
                    "mode": if companion { "companion" } else { "roleplay" },
                    "created_at": 1,
                    "updated_at": 2
                }]),
            ),
            document(
                LegacyBackupDocumentKind::Sessions,
                json!([direct_session(session_id, character_id)]),
            ),
        ];
        if let Some(shared) = shared {
            documents.push(document(
                LegacyBackupDocumentKind::CompanionSharedMemory,
                shared,
            ));
        }
        let inventory = LegacyBackupInventory {
            version: 1,
            created_at: 1,
            app_version: "legacy".into(),
            source_hash: ContentHash::parse("aa".repeat(32)).expect("source hash"),
            documents,
            media: Vec::new(),
        };
        let configuration =
            plan_legacy_backup_configuration(inventory).expect("configuration plan");
        let authored = plan_legacy_backup_authored(configuration).expect("authored plan");
        let media = plan_legacy_backup_authored_media(authored).expect("media plan");
        let asr = plan_legacy_backup_asr(media).expect("ASR plan");
        let usage = plan_legacy_backup_usage(asr).expect("usage plan");
        let pricing = plan_legacy_backup_pricing(usage).expect("pricing plan");
        let direct = plan_legacy_backup_direct_sessions(pricing).expect("direct session plan");
        let group = plan_legacy_backup_group_sessions(direct).expect("group session plan");
        plan_legacy_backup_scheduled_notes(group).expect("scheduled-note plan")
    }

    #[test]
    fn shared_memory_preserves_state_and_exact_snapshot_candidates() {
        let character_id = id(1);
        let session_id = id(2);
        let rows = json!([shared_state(&character_id, &session_id)]);
        let plan = plan_legacy_backup_companion_shared_memory(source(
            Some(rows),
            &character_id,
            &session_id,
            true,
        ))
        .expect("shared-memory plan");
        let state = &plan.states[0];
        assert_eq!(state.ordinal, 0);
        assert_eq!(state.memories_json, "[\" Remember the garden. \"]");
        assert_eq!(state.memory_summary.as_deref(), Some(" A shared summary "));
        assert_eq!(state.soul_facts.as_ref().map(Vec::len), Some(1));
        assert_eq!(state.relationship_states[0].state.interaction_count, 7);
        assert_eq!(
            state.relationship_states[0]
                .conversation_source_ids
                .as_slice(),
            [session_id.as_str()]
        );
        assert_eq!(state.episodes[0].conversation_source_id, session_id);
        assert_eq!(
            state.memory_materialization,
            LegacyBackupCompanionMaterialization::RetainedEvidence
        );
        assert_eq!(
            state.soul_materialization,
            LegacyBackupCompanionMaterialization::ExactInitialSnapshot
        );
    }

    #[test]
    fn missing_shared_memory_document_is_explicit() {
        let character_id = id(10);
        let session_id = id(11);
        let plan = plan_legacy_backup_companion_shared_memory(source(
            None,
            &character_id,
            &session_id,
            true,
        ))
        .expect("empty shared-memory plan");
        assert!(plan.states.is_empty());
        assert!(plan.notices.iter().any(|notice| {
            notice.document == LegacyBackupDocumentKind::CompanionSharedMemory
                && notice.kind == LegacyBackupConversionNoticeKind::Absent
        }));
    }

    #[test]
    fn shared_memory_rejects_duplicate_or_wrong_owners_and_bad_nested_state() {
        let character_id = id(20);
        let session_id = id(21);
        let row = shared_state(&character_id, &session_id);
        assert!(matches!(
            plan_legacy_backup_companion_shared_memory(source(
                Some(json!([row.clone(), row.clone()])),
                &character_id,
                &session_id,
                true,
            )),
            Err(LegacyBackupCompanionSharedMemoryError::Malformed { ref field })
                if field == "[1].character_id"
        ));
        assert!(matches!(
            plan_legacy_backup_companion_shared_memory(source(
                Some(json!([row.clone()])),
                &character_id,
                &session_id,
                false,
            )),
            Err(LegacyBackupCompanionSharedMemoryError::Malformed { ref field })
                if field == "[0].character_id"
        ));
        let mut invalid = row;
        invalid["relationship_states"] = json!("{\"__default__\":{\"trust\":2}}");
        assert!(matches!(
            plan_legacy_backup_companion_shared_memory(source(
                Some(json!([invalid])),
                &character_id,
                &session_id,
                true,
            )),
            Err(LegacyBackupCompanionSharedMemoryError::Malformed { ref field })
                if field == "[0].relationship_states.__default__"
        ));
    }
}
