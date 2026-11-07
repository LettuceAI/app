use std::collections::{BTreeMap, BTreeSet};

use lettuce_characters::InteractionMode;
use lettuce_companions::{RelationshipState, SoulFact};
use lettuce_types::{CharacterId, PersonaId};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    LegacyBackupConversionNotice, LegacyBackupConversionNoticeKind, LegacyBackupDocumentKind,
    LegacyBackupScheduledNotePlan,
};

const JSON_LIMIT: usize = lettuce_types::limits::MAX_DOCUMENT_BYTES;
const TEXT_LIMIT: usize = lettuce_types::limits::MAX_TEXT_BYTES;

#[derive(Debug)]
pub struct LegacyBackupCompanionSharedMemoryPlan {
    pub states: Vec<LegacyBackupCompanionSharedMemory>,
    pub skipped: Vec<crate::LegacyImportSkip>,
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
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LegacyBackupCompanionSharedMemoryError {
    #[error("legacy backup companion shared-memory document is malformed")]
    Malformed { field: String },
    #[error("legacy backup companion shared-memory document exceeds its record limit: {reason}")]
    LimitExceeded { reason: &'static str },
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
#[serde(rename_all = "camelCase")]
struct LegacyRelationshipState {
    closeness: f64,
    trust: f64,
    affection: f64,
    tension: f64,
    stability: f64,
    interaction_count: u32,
    #[serde(deserialize_with = "crate::deserialize_legacy_timestamp")]
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
    let mut skipped = Vec::new();
    let states = match document {
        Some(document) => {
            let rows: Vec<StateRow> =
                serde_json::from_slice(&document.bytes).map_err(|_| malformed("$"))?;
            map_states(rows, &source, &mut skipped, &mut notices)?
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
    Ok(LegacyBackupCompanionSharedMemoryPlan {
        states,
        skipped,
        notices,
        source,
    })
}

fn map_states(
    rows: Vec<StateRow>,
    source: &LegacyBackupScheduledNotePlan,
    skipped: &mut Vec<crate::LegacyImportSkip>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<LegacyBackupCompanionSharedMemory>, LegacyBackupCompanionSharedMemoryError> {
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
        let relationship_value = validate_json_object(
            &row.relationship_states,
            &format!("{path}.relationship_states"),
        )?;
        let mut relationship_states = map_relationships(
            &relationship_value,
            &persona_ids,
            &format!("{path}.relationship_states"),
            skipped,
            notices,
        )?;
        let episodes = map_episodes(
            row.episodes,
            character_id,
            &persona_ids,
            &direct_sessions,
            &path,
            skipped,
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
        let mut repaired = Vec::new();
        let soul_facts = crate::legacy::legacy_backup_json_values::legacy_soul_growth(
            &soul_value,
            &character_id.to_string(),
            lettuce_types::TimestampMillis::new(row.updated_at),
            &mut repaired,
        );
        if !repaired.is_empty() {
            notices.push(notice(
                LegacyBackupConversionNoticeKind::Lossy,
                &format!("{path}.soul_growth"),
            ));
        }
        let soul_materialization = if soul_facts.is_some() {
            LegacyBackupCompanionMaterialization::ExactInitialSnapshot
        } else {
            LegacyBackupCompanionMaterialization::RetainedEvidence
        };
        let relationship_materialization = if relationship_value
            .as_object()
            .is_some_and(|values| values.len() == relationship_states.len())
            && relationship_states.iter().all(|relationship| {
                relationship.materialization
                    == LegacyBackupCompanionMaterialization::ExactInitialSnapshot
            })
        {
            LegacyBackupCompanionMaterialization::ExactInitialSnapshot
        } else {
            LegacyBackupCompanionMaterialization::RetainedEvidence
        };
        states.push(LegacyBackupCompanionSharedMemory {
            ordinal: u64::try_from(index).map_err(|_| {
                LegacyBackupCompanionSharedMemoryError::LimitExceeded {
                    reason: "shared memory ordinal exceeds the u64 range",
                }
            })?,
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
    skipped: &mut Vec<crate::LegacyImportSkip>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
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
                skip_reference(
                    &field,
                    crate::LegacyImportSkipKind::PersonaReference,
                    crate::LegacyImportSkipReason::MissingPersona,
                    skipped,
                    notices,
                );
                continue;
            }
            Some(id)
        };
        let state = legacy_relationship_state(value).ok_or_else(|| malformed(&field))?;
        for key in value.as_object().into_iter().flat_map(|object| object.keys()) {
            if !matches!(
                key.as_str(),
                "closeness" | "trust" | "affection" | "tension" | "stability"
                    | "interactionCount" | "lastInteractionAt"
            ) {
                notices.push(notice(
                    LegacyBackupConversionNoticeKind::Unsupported,
                    &format!("{field}.{key}"),
                ));
            }
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
    skipped: &mut Vec<crate::LegacyImportSkip>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<LegacyBackupCompanionEpisode>, LegacyBackupCompanionSharedMemoryError> {
    let character_source_id = character_id.to_string();
    let mut identities = BTreeSet::new();
    let mut session_ids = BTreeSet::new();
    let mut episodes = Vec::with_capacity(rows.len());
    for (index, mut row) in rows.into_iter().enumerate() {
        let path = format!("{parent_path}.episodes[{index}]");
        report_extra(&path, &row.extra, notices);
        validate_identifier(&row.session_id, &format!("{path}.session_id"))?;
        if !session_ids.insert(row.session_id.clone()) {
            return Err(malformed(format!("{path}.session_id")));
        }
        if !sessions.contains_key(row.session_id.as_str()) {
            skip_reference(
                &format!("{path}.session_id"),
                crate::LegacyImportSkipKind::SessionLink,
                crate::LegacyImportSkipReason::MissingSession,
                skipped,
                notices,
            );
            continue;
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
                skip_reference(
                    &format!("{path}.persona_key"),
                    crate::LegacyImportSkipKind::PersonaReference,
                    crate::LegacyImportSkipReason::MissingPersona,
                    skipped,
                    notices,
                );
                continue;
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
            if !sessions.contains_key(previous) {
                skip_reference(
                    &format!("{path}.previous_session_id"),
                    crate::LegacyImportSkipKind::SessionLink,
                    crate::LegacyImportSkipReason::MissingSession,
                    skipped,
                    notices,
                );
                row.previous_session_id = None;
            } else if sessions.get(previous).copied() != Some(character_source_id.as_str()) {
                return Err(orphan(format!("{path}.previous_session_id")));
            }
        }
        let started_at = timestamp(row.started_at, &format!("{path}.started_at"))?;
        let ended_at = row
            .ended_at
            .map(|value| timestamp(value, &format!("{path}.ended_at")))
            .transpose()?;
        let updated_at = timestamp(row.updated_at, &format!("{path}.updated_at"))?;
        if ended_at.is_some_and(|value| value < started_at) || updated_at < started_at {
            return Err(malformed(format!("{path}.timestamps")));
        }
        episodes.push(LegacyBackupCompanionEpisode {
            ordinal: u64::try_from(index).map_err(|_| {
                LegacyBackupCompanionSharedMemoryError::LimitExceeded {
                    reason: "episode ordinal exceeds the u64 range",
                }
            })?,
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
        .map(|episode| {
            (episode.conversation_source_id.clone(), (episode.persona_id, episode.episode_index))
        })
        .collect::<BTreeMap<_, _>>();
    for episode in &mut episodes {
        let index = episode.ordinal;
        let Some(previous_id) = episode.previous_conversation_source_id.as_deref() else {
            continue;
        };
        let Some(previous) = by_session.get(previous_id) else {
            skip_reference(
                &format!("{parent_path}.episodes[{index}].previous_session_id"),
                crate::LegacyImportSkipKind::SessionLink,
                crate::LegacyImportSkipReason::IncompatibleReference,
                skipped,
                notices,
            );
            episode.previous_conversation_source_id = None;
            continue;
        };
        if previous.0 != episode.persona_id || previous.1 >= episode.episode_index {
            return Err(malformed(format!(
                "{parent_path}.episodes[{index}].previous_session_id"
            )));
        }
    }
    Ok(episodes)
}

fn skip_reference(
    field: &str,
    kind: crate::LegacyImportSkipKind,
    reason: crate::LegacyImportSkipReason,
    skipped: &mut Vec<crate::LegacyImportSkip>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) {
    skipped.push(crate::LegacyImportSkip {
        kind,
        source_key: format!("companion_shared_memory:{field}"),
        reason,
    });
    notices.push(notice(LegacyBackupConversionNoticeKind::Lossy, field));
}

/// One stored legacy relationship state, when it is in range.
pub(crate) fn legacy_relationship_state(value: &Value) -> Option<RelationshipState> {
    let legacy: LegacyRelationshipState = serde_json::from_value(value.clone()).ok()?;
    let state = RelationshipState {
        closeness: legacy.closeness,
        trust: legacy.trust,
        affection: legacy.affection,
        tension: legacy.tension,
        stability: legacy.stability,
        interaction_count: legacy.interaction_count,
        last_interaction_at: lettuce_types::TimestampMillis::new(legacy.last_interaction_at),
    };
    valid_relationship(&state).then_some(state)
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
        for key in ["evidenceCount", "validFrom", "validUntil"] {
            if object
                .get(key)
                .is_some_and(|value| !value.is_null() && value.as_u64().is_none())
            {
                return Err(malformed(format!("{field}.{key}")));
            }
        }
        for key in ["createdAt", "supersededAt"] {
            if object
                .get(key)
                .is_some_and(|value| {
                    !value.is_null() && value.as_i64().is_none() && value.as_u64().is_none()
                })
            {
                return Err(malformed(format!("{field}.{key}")));
            }
        }
        for key in ["sourceMemoryIds", "supersedes"] {
            if let Some(value) = object.get(key)
                && serde_json::from_value::<Vec<String>>(value.clone()).is_err()
            {
                return Err(malformed(format!("{field}.{key}")));
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
}

fn validate_string_array(
    raw: &str,
    field: &str,
) -> Result<(), LegacyBackupCompanionSharedMemoryError> {
    if raw.len() > JSON_LIMIT {
        return Err(LegacyBackupCompanionSharedMemoryError::LimitExceeded {
            reason: "string array json exceeds the size limit",
        });
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
        return Err(LegacyBackupCompanionSharedMemoryError::LimitExceeded {
            reason: "json document exceeds the size limit",
        });
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

// Legacy episode timestamps are epoch millis and may be negative (pre-1970),
// which the signed `TimestampMillis` preserves. The `_field` stays for call-site
// symmetry with `count` even though any `i64` is a valid instant.
fn timestamp(value: i64, _field: &str) -> Result<i64, LegacyBackupCompanionSharedMemoryError> {
    Ok(value)
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

    fn inventory(
        shared: Option<Value>,
        character_id: &str,
        session_id: &str,
        companion: bool,
    ) -> LegacyBackupInventory {
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
        LegacyBackupInventory {
            warnings: Vec::new(),
            version: 1,
            created_at: 1,
            app_version: "legacy".into(),
            source_hash: ContentHash::parse("aa".repeat(32)).expect("source hash"),
            documents,
            media: Vec::new(),
        }
    }

    fn source(
        shared: Option<Value>,
        character_id: &str,
        session_id: &str,
        companion: bool,
    ) -> LegacyBackupScheduledNotePlan {
        let configuration =
            plan_legacy_backup_configuration(inventory(shared, character_id, session_id, companion))
                .expect("configuration plan");
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

    #[test]
    fn shared_relationships_preserve_effective_dates_on_both_sides_of_epoch() {
        let character_id = id(30);
        let session_id = id(31);
        for timestamp in [i64::MIN, -315_619_200_000, 0, 20, i64::MAX] {
            let mut row = shared_state(&character_id, &session_id);
            let mut relationships: Value = serde_json::from_str(
                row["relationship_states"].as_str().expect("relationship JSON"),
            ).expect("relationships");
            relationships["__default__"]["lastInteractionAt"] = json!(timestamp);
            let raw = serde_json::to_string(&relationships).expect("relationship JSON");
            row["relationship_states"] = json!(&raw);
            let plan = plan_legacy_backup_companion_shared_memory(source(
                Some(json!([row])), &character_id, &session_id, true,
            )).expect("shared-memory plan with signed effective date");
            let shared = &plan.states[0];
            assert_eq!(shared.relationship_states_json, raw);
            assert_eq!(shared.relationship_states[0].state.last_interaction_at.get(), timestamp);
            assert_eq!(shared.relationship_states[0].state.interaction_count, 7);
            assert_eq!(shared.created_at, 1);
            assert_eq!(shared.updated_at, 20);
        }
    }

    #[test]
    fn persona_relationships_accept_signed_dates_and_still_reject_invalid_state() {
        let persona = id(32).parse::<PersonaId>().expect("persona id");
        let key = persona.to_string();
        let path = "[2].relationship_states";
        let field = format!("{path}.{key}");
        let personas = BTreeSet::from([persona]);
        let value = json!({ (key.clone()): {
            "closeness": -0.4, "trust": -0.5, "affection": -0.3,
            "tension": 0.1, "stability": 0.8, "interactionCount": 7,
            "lastInteractionAt": -315_619_200_000_i64
        } });
        let mapped = map_relationships(&value, &personas, path, &mut Vec::new(), &mut Vec::new()).expect("persona relationship");
        assert_eq!(mapped[0].persona_id, Some(persona));
        assert_eq!(mapped[0].state.last_interaction_at.get(), -315_619_200_000);
        for (property, invalid) in [
            ("trust", json!(-1.01)),
            ("tension", json!(-0.1)),
            ("interactionCount", json!(-1)),
            ("lastInteractionAt", json!(-1.5)),
            ("lastInteractionAt", json!("20")),
        ] {
            let mut invalid_state = value.clone();
            invalid_state[&key][property] = invalid;
            assert!(matches!(map_relationships(&invalid_state, &personas, path, &mut Vec::new(), &mut Vec::new()),
                Err(LegacyBackupCompanionSharedMemoryError::Malformed { field: actual }) if actual == field));
        }
    }

    #[test]
    fn deleted_personas_are_reported_without_reassigning_their_state() {
        let character = id(40);
        let session = id(41);
        let missing_persona = id(42);
        let mut row = shared_state(&character, &session);
        let mut relationships: Value = serde_json::from_str(row["relationship_states"].as_str().expect("JSON")).expect("relationships");
        relationships[&missing_persona] = relationships["__default__"].clone();
        let raw = serde_json::to_string(&relationships).expect("JSON");
        row["relationship_states"] = json!(&raw);
        let partial = plan_legacy_backup_companion_shared_memory(source(Some(json!([row.clone()])), &character, &session, true)).expect("stale relationship only");
        assert_eq!(partial.states[0].relationship_materialization, LegacyBackupCompanionMaterialization::RetainedEvidence);
        row["episodes"][0]["persona_key"] = json!(missing_persona);
        let plan = plan_legacy_backup_companion_shared_memory(source(Some(json!([row])), &character, &session, true)).expect("stale persona links");
        assert_eq!(plan.states[0].relationship_states_json, raw);
        assert_eq!(plan.states[0].relationship_states.len(), 1);
        assert_eq!(plan.states[0].relationship_states[0].persona_id, None);
        assert!(plan.states[0].episodes.is_empty());
        assert_eq!(plan.states[0].relationship_states[0].materialization, LegacyBackupCompanionMaterialization::RetainedEvidence);
        assert_eq!(plan.skipped.iter().filter(|skip| skip.reason == crate::LegacyImportSkipReason::MissingPersona).count(), 2);
    }

    #[test]
    fn deleted_episode_sessions_and_missing_predecessors_do_not_abort_the_remaining_history() {
        let character = id(50);
        let session = id(51);
        let missing = id(52);
        let mut row = shared_state(&character, &session);
        row["episodes"][0]["previous_session_id"] = json!(&missing);
        row["episodes"][0]["episode_index"] = json!(2);
        let mut deleted = row["episodes"][0].clone();
        deleted["session_id"] = json!(missing);
        row["episodes"].as_array_mut().expect("episodes").push(deleted);
        let plan = plan_legacy_backup_companion_shared_memory(source(Some(json!([row])), &character, &session, true)).expect("stale episode links");
        assert_eq!(plan.states[0].episodes.len(), 1);
        assert_eq!(plan.states[0].episodes[0].conversation_source_id, session);
        assert_eq!(plan.states[0].episodes[0].previous_conversation_source_id, None);
        assert_eq!(plan.skipped.iter().filter(|skip| skip.reason == crate::LegacyImportSkipReason::MissingSession).count(), 2);
        assert!(plan.notices.iter().any(|notice| notice.field == "[0].episodes[1].session_id"));
    }

    #[test]
    fn extra_relationship_fields_keep_the_legacy_read_behavior_and_are_reported() {
        let character = id(60);
        let session = id(61);
        let mut row = shared_state(&character, &session);
        let mut relationships: Value = serde_json::from_str(row["relationship_states"].as_str().expect("JSON")).expect("relationships");
        relationships["__default__"]["legacyExtra"] = json!({ "kept": true });
        let raw = serde_json::to_string(&relationships).expect("JSON");
        row["relationship_states"] = json!(&raw);
        let plan = plan_legacy_backup_companion_shared_memory(source(Some(json!([row])), &character, &session, true)).expect("extra relationship fields");
        assert_eq!(plan.states[0].relationship_states_json, raw);
        assert_eq!(plan.states[0].relationship_states[0].state.interaction_count, 7);
        assert!(plan.notices.iter().any(|notice| notice.kind == LegacyBackupConversionNoticeKind::Unsupported && notice.field.ends_with("relationship_states.__default__.legacyExtra")));
    }

    #[test]
    fn skipped_episode_sessions_reach_the_sealed_import_plan() {
        let character = id(70);
        let session = id(71);
        let mut row = shared_state(&character, &session);
        row["memory_embeddings"] = json!("[]");
        let mut deleted = row["episodes"][0].clone();
        deleted["session_id"] = json!(id(72));
        row["episodes"].as_array_mut().expect("episodes").push(deleted);
        let plan = crate::plan_legacy_backup_compatibility(inventory(
            Some(json!([row])), &character, &session, true,
        ))
        .expect("compatibility plan with deleted episode session");

        plan.verify_seal().expect("sealed compatibility plan");
        let imported = plan.legacy_import_plan();
        assert!(imported.later_skips.iter().any(|skip| {
            skip.kind == crate::LegacyImportSkipKind::SessionLink
                && skip.reason == crate::LegacyImportSkipReason::MissingSession
                && skip.source_key == "companion_shared_memory:[0].episodes[1].session_id"
        }));
        assert!(plan.notices.iter().any(|notice| {
            notice.kind == LegacyBackupConversionNoticeKind::Lossy
                && notice.document == LegacyBackupDocumentKind::CompanionSharedMemory
                && notice.field == "[0].episodes[1].session_id"
        }));
    }

    #[test]
    fn episode_links_to_a_different_character_still_reject() {
        let character = id(80);
        let session = id(81);
        let other_character = id(82);
        let previous = id(83);
        for wrong_previous in [false, true] {
            let mut row = shared_state(&character, &session);
            if wrong_previous {
                row["episodes"][0]["previous_session_id"] = json!(&previous);
            }
            let sessions = BTreeMap::from([
                (session.as_str(), if wrong_previous { character.as_str() } else { other_character.as_str() }),
                (previous.as_str(), other_character.as_str()),
            ]);
            let episodes = serde_json::from_value(row["episodes"].clone()).expect("episodes");
            let expected = if wrong_previous {
                "[2].episodes[0].previous_session_id"
            } else {
                "[2].episodes[0].session_id"
            };
            assert!(matches!(
                map_episodes(episodes, character.parse().expect("character"), &BTreeSet::new(),
                    &sessions, "[2]", &mut Vec::new(), &mut Vec::new()),
                Err(LegacyBackupCompanionSharedMemoryError::Orphan { field }) if field == expected
            ));
        }
    }

    #[test]
    fn soul_growth_shape_accepts_signed_dates_without_relaxing_counters_or_validity_dates() {
        let value = json!([{ "createdAt": -300, "supersededAt": -200 }]);
        validate_soul_shape(&value, "soul").expect("signed legacy soul dates");
        for field in ["evidenceCount", "validFrom", "validUntil"] {
            let mut invalid = value.clone();
            invalid[0][field] = json!(-1);
            assert!(validate_soul_shape(&invalid, "soul").is_err());
        }
        for field in ["createdAt", "supersededAt"] {
            for invalid_date in [json!(1.5), json!("20")] {
                let mut invalid = value.clone();
                invalid[0][field] = invalid_date;
                assert!(validate_soul_shape(&invalid, "soul").is_err());
            }
        }
    }
}
