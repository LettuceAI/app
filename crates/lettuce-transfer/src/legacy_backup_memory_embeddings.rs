use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use serde_json::Value;

use crate::{
    LegacyBackupCompanionSharedMemoryPlan, LegacyBackupConversionNotice,
    LegacyBackupConversionNoticeKind, LegacyBackupDocumentKind,
};

const OWNER_LIMIT: usize = 30_000;
const MEMORY_LIMIT: usize = 1_000_000;
const JSON_LIMIT: usize = 256 * 1024 * 1024;
const TEXT_LIMIT: usize = 16 * 1024;

#[derive(Debug)]
pub struct LegacyBackupMemoryEmbeddingPlan {
    pub owners: Vec<LegacyBackupMemoryEmbeddingOwner>,
    pub notices: Vec<LegacyBackupConversionNotice>,
    pub source: LegacyBackupCompanionSharedMemoryPlan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyBackupMemoryEmbeddingOwner {
    pub ordinal: u64,
    pub source_id: String,
    pub kind: LegacyBackupMemoryOwnerKind,
    pub memory_embeddings_json: String,
    pub memories: Vec<LegacyBackupMemoryEmbedding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LegacyBackupMemoryOwnerKind {
    DirectConversation,
    GroupConversation,
    CompanionShared,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyBackupMemoryEmbedding {
    pub ordinal: u64,
    pub id: String,
    pub text: String,
    pub embedding: Vec<f32>,
    pub created_at: u64,
    pub token_count: u32,
    pub is_cold: bool,
    pub last_accessed_at: u64,
    pub importance_score: f32,
    pub persistence_importance: f32,
    pub prompt_importance: f32,
    pub volatility: f32,
    pub is_pinned: bool,
    pub access_count: u32,
    pub embedding_source_version: Option<String>,
    pub embedding_dimensions: Option<usize>,
    pub match_score: Option<f32>,
    pub category: Option<String>,
    pub observed_at: Option<u64>,
    pub observed_time_precision: Option<String>,
    pub canonical_entities: Vec<LegacyBackupMemoryEntityAnchor>,
    pub fact_signature: Option<String>,
    pub fact_polarity: Option<i8>,
    pub source_role: Option<String>,
    pub source_message_id: Option<String>,
    pub superseded_by: Option<String>,
    pub superseded_at: Option<u64>,
    pub supersedes: Vec<String>,
    pub materialization: LegacyBackupMemoryMaterialization,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyBackupMemoryEntityAnchor {
    pub label: String,
    pub surface: String,
    pub canonical_key: String,
    pub canonical_name: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyBackupMemoryMaterialization {
    InitialItemAndProjection,
    InitialItemNeedsProjection,
    RetainedEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LegacyBackupMemoryEmbeddingError {
    #[error("legacy backup memory-embedding document is malformed")]
    Malformed { field: String },
    #[error("legacy backup memory-embedding document exceeds its record limit")]
    LimitExceeded,
    #[error("legacy backup memory-embedding graph contains an orphaned link")]
    Orphan { field: String },
    #[error("legacy backup memory-embedding copies disagree")]
    Conflict { field: String },
}

#[derive(Deserialize)]
struct OwnerRow {
    session_id: String,
    session_kind: String,
    memory_embeddings: String,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryRow {
    id: String,
    text: String,
    embedding: Vec<f32>,
    #[serde(default)]
    created_at: u64,
    #[serde(default)]
    token_count: u32,
    #[serde(default)]
    is_cold: bool,
    #[serde(default)]
    last_accessed_at: u64,
    #[serde(default = "full_score")]
    importance_score: f32,
    #[serde(default = "full_score")]
    persistence_importance: f32,
    #[serde(default = "full_score")]
    prompt_importance: f32,
    #[serde(default = "legacy_volatility")]
    volatility: f32,
    #[serde(default)]
    is_pinned: bool,
    #[serde(default)]
    access_count: u32,
    #[serde(default)]
    embedding_source_version: Option<String>,
    #[serde(default)]
    embedding_dimensions: Option<usize>,
    #[serde(default)]
    match_score: Option<f32>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    observed_at: Option<u64>,
    #[serde(default)]
    observed_time_precision: Option<String>,
    #[serde(default)]
    canonical_entities: Vec<EntityAnchorRow>,
    #[serde(default)]
    fact_signature: Option<String>,
    #[serde(default)]
    fact_polarity: Option<i8>,
    #[serde(default)]
    source_role: Option<String>,
    #[serde(default)]
    source_message_id: Option<String>,
    #[serde(default)]
    superseded_by: Option<String>,
    #[serde(default)]
    superseded_at: Option<u64>,
    #[serde(default)]
    supersedes: Vec<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntityAnchorRow {
    label: String,
    surface: String,
    canonical_key: String,
    canonical_name: String,
    #[serde(default)]
    confidence: f32,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

pub fn plan_legacy_backup_memory_embeddings(
    source: LegacyBackupCompanionSharedMemoryPlan,
) -> Result<LegacyBackupMemoryEmbeddingPlan, LegacyBackupMemoryEmbeddingError> {
    let document = source
        .source
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
        .find(|document| document.kind == LegacyBackupDocumentKind::MemoryEmbeddings);
    let mut notices = source.notices.clone();
    let owners = match document {
        Some(document) => {
            let rows: Vec<OwnerRow> =
                serde_json::from_slice(&document.bytes).map_err(|_| malformed("$"))?;
            map_owners(rows, &source, &mut notices)?
        }
        None => {
            notices.push(notice(LegacyBackupConversionNoticeKind::Absent, "$"));
            Vec::new()
        }
    };
    notices.sort();
    notices.dedup();
    Ok(LegacyBackupMemoryEmbeddingPlan {
        owners,
        notices,
        source,
    })
}

fn map_owners(
    rows: Vec<OwnerRow>,
    source: &LegacyBackupCompanionSharedMemoryPlan,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<LegacyBackupMemoryEmbeddingOwner>, LegacyBackupMemoryEmbeddingError> {
    if rows.len() > OWNER_LIMIT {
        return Err(LegacyBackupMemoryEmbeddingError::LimitExceeded);
    }
    let group = &source.source.source;
    let direct = &group.source;
    let embedded = embedded_copies(source);
    let mut identities = BTreeSet::new();
    let mut memory_count = 0usize;
    let mut owners = Vec::with_capacity(rows.len());
    for (index, row) in rows.into_iter().enumerate() {
        let path = format!("[{index}]");
        report_extra(&path, &row.extra, notices);
        validate_identifier(&row.session_id, &format!("{path}.session_id"))?;
        let kind = match row.session_kind.as_str() {
            "session" => LegacyBackupMemoryOwnerKind::DirectConversation,
            "group_session" => LegacyBackupMemoryOwnerKind::GroupConversation,
            "companion_shared" => LegacyBackupMemoryOwnerKind::CompanionShared,
            _ => return Err(malformed(format!("{path}.session_kind"))),
        };
        if !identities.insert((kind, row.session_id.clone())) {
            return Err(malformed(format!("{path}.session_id")));
        }
        let owner_exists = match kind {
            LegacyBackupMemoryOwnerKind::DirectConversation => direct
                .sessions
                .iter()
                .any(|session| session.source_id == row.session_id),
            LegacyBackupMemoryOwnerKind::GroupConversation => group
                .sessions
                .iter()
                .any(|session| session.source_id == row.session_id),
            LegacyBackupMemoryOwnerKind::CompanionShared => source
                .states
                .iter()
                .any(|state| state.character_id.to_string() == row.session_id),
        };
        if !owner_exists {
            return Err(orphan(format!("{path}.session_id")));
        }
        if embedded.get(&(kind, row.session_id.clone())).copied()
            != Some(row.memory_embeddings.as_str())
        {
            return Err(conflict(format!("{path}.memory_embeddings")));
        }
        let parsed: Vec<MemoryRow> = parse_json(&row.memory_embeddings, &path)?;
        if parsed.is_empty() {
            return Err(malformed(format!("{path}.memory_embeddings")));
        }
        memory_count = memory_count
            .checked_add(parsed.len())
            .ok_or(LegacyBackupMemoryEmbeddingError::LimitExceeded)?;
        if memory_count > MEMORY_LIMIT {
            return Err(LegacyBackupMemoryEmbeddingError::LimitExceeded);
        }
        let source_message_ids = source_message_ids(kind, &row.session_id, source);
        let memories = map_memories(parsed, &source_message_ids, &path, notices)?;
        owners.push(LegacyBackupMemoryEmbeddingOwner {
            ordinal: u64::try_from(index)
                .map_err(|_| LegacyBackupMemoryEmbeddingError::LimitExceeded)?,
            source_id: row.session_id,
            kind,
            memory_embeddings_json: row.memory_embeddings,
            memories,
        });
    }
    for ((kind, source_id), raw) in embedded {
        if raw != "[]" && !identities.contains(&(kind, source_id.clone())) {
            return Err(orphan(format!(
                "embedded.{}.{}",
                owner_kind_name(kind),
                source_id
            )));
        }
    }
    Ok(owners)
}

fn embedded_copies(
    source: &LegacyBackupCompanionSharedMemoryPlan,
) -> BTreeMap<(LegacyBackupMemoryOwnerKind, String), &str> {
    let group = &source.source.source;
    let direct = &group.source;
    let mut copies = BTreeMap::new();
    for session in &direct.sessions {
        copies.insert(
            (
                LegacyBackupMemoryOwnerKind::DirectConversation,
                session.source_id.clone(),
            ),
            session.memory_embeddings_json.as_str(),
        );
    }
    for session in &group.sessions {
        copies.insert(
            (
                LegacyBackupMemoryOwnerKind::GroupConversation,
                session.source_id.clone(),
            ),
            session.memory_embeddings_json.as_str(),
        );
    }
    for state in &source.states {
        copies.insert(
            (
                LegacyBackupMemoryOwnerKind::CompanionShared,
                state.character_id.to_string(),
            ),
            state.memory_embeddings_json.as_str(),
        );
    }
    copies
}

fn map_memories(
    rows: Vec<MemoryRow>,
    source_message_ids: &BTreeSet<&str>,
    parent_path: &str,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<LegacyBackupMemoryEmbedding>, LegacyBackupMemoryEmbeddingError> {
    let mut ids = BTreeSet::new();
    let mut created_at_values = BTreeSet::new();
    let mut memories = Vec::with_capacity(rows.len());
    for (index, row) in rows.into_iter().enumerate() {
        let path = format!("{parent_path}.memory_embeddings[{index}]");
        report_extra(&path, &row.extra, notices);
        validate_identifier(&row.id, &format!("{path}.id"))?;
        if !ids.insert(row.id.clone()) {
            return Err(malformed(format!("{path}.id")));
        }
        validate_text(&row.text, &format!("{path}.text"))?;
        if row.text.trim().is_empty() {
            return Err(malformed(format!("{path}.text")));
        }
        validate_scores(&row, &path)?;
        validate_optional_texts(&row, &path)?;
        if row.embedding.iter().any(|value| !value.is_finite())
            || row.match_score.is_some_and(|value| !value.is_finite())
        {
            return Err(malformed(format!("{path}.embedding")));
        }
        if row.created_at > i64::MAX as u64
            || row.last_accessed_at > i64::MAX as u64
            || row.observed_at.is_some_and(|value| value > i64::MAX as u64)
            || row
                .superseded_at
                .is_some_and(|value| value > i64::MAX as u64)
        {
            return Err(malformed(format!("{path}.timestamps")));
        }
        if !created_at_values.insert(row.created_at) {
            notices.push(notice(
                LegacyBackupConversionNoticeKind::Lossy,
                &format!("{parent_path}.memory_embeddings[].created_at_order"),
            ));
        }
        if row.embedding.is_empty() {
            if row
                .embedding_dimensions
                .is_some_and(|dimensions| dimensions != 0)
            {
                return Err(malformed(format!("{path}.embedding_dimensions")));
            }
        } else if row.embedding_dimensions != Some(row.embedding.len())
            || !matches!(row.embedding.len(), 64 | 128 | 256 | 512 | 768)
        {
            return Err(malformed(format!("{path}.embedding_dimensions")));
        }
        if row
            .fact_polarity
            .is_some_and(|value| !(-1..=1).contains(&value))
            || row.superseded_by.as_deref() == Some(row.id.as_str())
            || row.supersedes.iter().any(|id| id == &row.id)
            || row.superseded_by.is_some() != row.superseded_at.is_some()
        {
            return Err(malformed(format!("{path}.attribution")));
        }
        let canonical_entities = row
            .canonical_entities
            .into_iter()
            .enumerate()
            .map(|(anchor_index, anchor)| {
                map_anchor(
                    anchor,
                    &format!("{path}.canonical_entities[{anchor_index}]"),
                    notices,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let item_compatible = uuid::Uuid::parse_str(&row.id).is_ok()
            && matches!(
                row.category.as_deref(),
                Some(
                    "character_trait"
                        | "relationship"
                        | "plot_event"
                        | "world_detail"
                        | "preference"
                        | "other"
                )
            )
            && matches!(
                row.source_role.as_deref(),
                None | Some("user" | "assistant")
            )
            && row.observed_at.is_some() == row.source_role.is_some()
            && row.observed_at.is_some() == row.source_message_id.is_some()
            && row.observed_time_precision.is_some() == row.observed_at.is_some()
            && row
                .observed_time_precision
                .as_deref()
                .is_none_or(|value| value == "turn")
            && row.source_message_id.as_deref().is_none_or(|value| {
                uuid::Uuid::parse_str(value).is_ok() && source_message_ids.contains(value)
            })
            && row
                .superseded_by
                .as_deref()
                .is_none_or(|value| uuid::Uuid::parse_str(value).is_ok())
            && row
                .supersedes
                .iter()
                .all(|value| uuid::Uuid::parse_str(value).is_ok())
            && canonical_entities.is_empty()
            && row.fact_signature.is_none()
            && row.fact_polarity.is_none()
            && row.match_score.is_none();
        let materialization = if item_compatible
            && (row.embedding.is_empty() || row.embedding_source_version.is_none())
        {
            LegacyBackupMemoryMaterialization::InitialItemNeedsProjection
        } else if item_compatible {
            LegacyBackupMemoryMaterialization::InitialItemAndProjection
        } else {
            LegacyBackupMemoryMaterialization::RetainedEvidence
        };
        memories.push(LegacyBackupMemoryEmbedding {
            ordinal: u64::try_from(index)
                .map_err(|_| LegacyBackupMemoryEmbeddingError::LimitExceeded)?,
            id: row.id,
            text: row.text,
            embedding: row.embedding,
            created_at: row.created_at,
            token_count: row.token_count,
            is_cold: row.is_cold,
            last_accessed_at: row.last_accessed_at,
            importance_score: row.importance_score,
            persistence_importance: row.persistence_importance,
            prompt_importance: row.prompt_importance,
            volatility: row.volatility,
            is_pinned: row.is_pinned,
            access_count: row.access_count,
            embedding_source_version: row.embedding_source_version,
            embedding_dimensions: row.embedding_dimensions,
            match_score: row.match_score,
            category: row.category,
            observed_at: row.observed_at,
            observed_time_precision: row.observed_time_precision,
            canonical_entities,
            fact_signature: row.fact_signature,
            fact_polarity: row.fact_polarity,
            source_role: row.source_role,
            source_message_id: row.source_message_id,
            superseded_by: row.superseded_by,
            superseded_at: row.superseded_at,
            supersedes: row.supersedes,
            materialization,
        });
    }
    for (index, memory) in memories.iter().enumerate() {
        for referenced in memory.supersedes.iter().chain(memory.superseded_by.iter()) {
            if !ids.contains(referenced) {
                return Err(orphan(format!(
                    "{parent_path}.memory_embeddings[{index}].supersession"
                )));
            }
        }
    }
    Ok(memories)
}

fn source_message_ids<'a>(
    kind: LegacyBackupMemoryOwnerKind,
    source_id: &str,
    source: &'a LegacyBackupCompanionSharedMemoryPlan,
) -> BTreeSet<&'a str> {
    let group = &source.source.source;
    let direct = &group.source;
    match kind {
        LegacyBackupMemoryOwnerKind::DirectConversation => direct
            .sessions
            .iter()
            .filter(|session| session.source_id == source_id)
            .flat_map(|session| session.messages.iter())
            .map(|message| message.source_id.as_str())
            .collect(),
        LegacyBackupMemoryOwnerKind::GroupConversation => group
            .sessions
            .iter()
            .filter(|session| session.source_id == source_id)
            .flat_map(|session| session.messages.iter())
            .map(|message| message.source_id.as_str())
            .collect(),
        LegacyBackupMemoryOwnerKind::CompanionShared => direct
            .sessions
            .iter()
            .filter(|session| session.character_source_id == source_id)
            .flat_map(|session| session.messages.iter())
            .map(|message| message.source_id.as_str())
            .collect(),
    }
}

fn map_anchor(
    row: EntityAnchorRow,
    path: &str,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<LegacyBackupMemoryEntityAnchor, LegacyBackupMemoryEmbeddingError> {
    report_extra(path, &row.extra, notices);
    for (field, value) in [
        ("label", row.label.as_str()),
        ("surface", row.surface.as_str()),
        ("canonical_key", row.canonical_key.as_str()),
        ("canonical_name", row.canonical_name.as_str()),
    ] {
        validate_text(value, &format!("{path}.{field}"))?;
        if value.trim().is_empty() {
            return Err(malformed(format!("{path}.{field}")));
        }
    }
    if !row.confidence.is_finite() || !(0.0..=1.0).contains(&row.confidence) {
        return Err(malformed(format!("{path}.confidence")));
    }
    Ok(LegacyBackupMemoryEntityAnchor {
        label: row.label,
        surface: row.surface,
        canonical_key: row.canonical_key,
        canonical_name: row.canonical_name,
        confidence: row.confidence,
    })
}

fn validate_scores(row: &MemoryRow, path: &str) -> Result<(), LegacyBackupMemoryEmbeddingError> {
    for (field, value) in [
        ("importance_score", row.importance_score),
        ("persistence_importance", row.persistence_importance),
        ("prompt_importance", row.prompt_importance),
        ("volatility", row.volatility),
    ] {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(malformed(format!("{path}.{field}")));
        }
    }
    if row.is_pinned && row.is_cold {
        return Err(malformed(format!("{path}.is_cold")));
    }
    Ok(())
}

fn validate_optional_texts(
    row: &MemoryRow,
    path: &str,
) -> Result<(), LegacyBackupMemoryEmbeddingError> {
    for (field, value) in [
        (
            "embedding_source_version",
            row.embedding_source_version.as_deref(),
        ),
        ("category", row.category.as_deref()),
        (
            "observed_time_precision",
            row.observed_time_precision.as_deref(),
        ),
        ("fact_signature", row.fact_signature.as_deref()),
        ("source_role", row.source_role.as_deref()),
        ("source_message_id", row.source_message_id.as_deref()),
        ("superseded_by", row.superseded_by.as_deref()),
    ] {
        if let Some(value) = value {
            validate_text(value, &format!("{path}.{field}"))?;
            if value.trim().is_empty() {
                return Err(malformed(format!("{path}.{field}")));
            }
        }
    }
    for (index, value) in row.supersedes.iter().enumerate() {
        validate_identifier(value, &format!("{path}.supersedes[{index}]"))?;
    }
    Ok(())
}

fn parse_json<T: for<'de> Deserialize<'de>>(
    raw: &str,
    path: &str,
) -> Result<T, LegacyBackupMemoryEmbeddingError> {
    if raw.len() > JSON_LIMIT {
        return Err(LegacyBackupMemoryEmbeddingError::LimitExceeded);
    }
    serde_json::from_str(raw).map_err(|_| malformed(format!("{path}.memory_embeddings")))
}

fn validate_text(value: &str, field: &str) -> Result<(), LegacyBackupMemoryEmbeddingError> {
    if value.len() > TEXT_LIMIT || value.contains('\0') {
        Err(malformed(field))
    } else {
        Ok(())
    }
}

fn validate_identifier(value: &str, field: &str) -> Result<(), LegacyBackupMemoryEmbeddingError> {
    if value.trim().is_empty()
        || value.trim() != value
        || value.len() > 1_024
        || value.chars().any(char::is_control)
    {
        Err(malformed(field))
    } else {
        Ok(())
    }
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

fn owner_kind_name(kind: LegacyBackupMemoryOwnerKind) -> &'static str {
    match kind {
        LegacyBackupMemoryOwnerKind::DirectConversation => "session",
        LegacyBackupMemoryOwnerKind::GroupConversation => "group_session",
        LegacyBackupMemoryOwnerKind::CompanionShared => "companion_shared",
    }
}

fn notice(kind: LegacyBackupConversionNoticeKind, field: &str) -> LegacyBackupConversionNotice {
    LegacyBackupConversionNotice {
        kind,
        document: LegacyBackupDocumentKind::MemoryEmbeddings,
        field: field.to_owned(),
    }
}

fn malformed(field: impl Into<String>) -> LegacyBackupMemoryEmbeddingError {
    LegacyBackupMemoryEmbeddingError::Malformed {
        field: field.into(),
    }
}

fn orphan(field: impl Into<String>) -> LegacyBackupMemoryEmbeddingError {
    LegacyBackupMemoryEmbeddingError::Orphan {
        field: field.into(),
    }
}

fn conflict(field: impl Into<String>) -> LegacyBackupMemoryEmbeddingError {
    LegacyBackupMemoryEmbeddingError::Conflict {
        field: field.into(),
    }
}

const fn full_score() -> f32 {
    1.0
}

const fn legacy_volatility() -> f32 {
    0.4
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
        plan_legacy_backup_companion_shared_memory, plan_legacy_backup_configuration,
        plan_legacy_backup_direct_sessions, plan_legacy_backup_group_sessions,
        plan_legacy_backup_pricing, plan_legacy_backup_scheduled_notes, plan_legacy_backup_usage,
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

    fn memory(memory_id: &str, dimensions: usize) -> Value {
        json!({
            "id": memory_id,
            "text": "The garden gate is blue.",
            "embedding": vec![0.25_f32; dimensions],
            "createdAt": 10,
            "tokenCount": 6,
            "isCold": false,
            "lastAccessedAt": 20,
            "importanceScore": 0.9,
            "persistenceImportance": 0.8,
            "promptImportance": 0.7,
            "volatility": 0.4,
            "isPinned": true,
            "accessCount": 3,
            "embeddingSourceVersion": "v4",
            "embeddingDimensions": dimensions,
            "matchScore": null,
            "category": "world_detail",
            "observedAt": null,
            "observedTimePrecision": null,
            "canonicalEntities": [],
            "factSignature": null,
            "factPolarity": null,
            "sourceRole": null,
            "sourceMessageId": null,
            "supersededBy": null,
            "supersededAt": null,
            "supersedes": []
        })
    }

    fn session(session_id: &str, character_id: &str, embeddings: &str) -> Value {
        json!({
            "id": session_id,
            "character_id": character_id,
            "title": "Conversation",
            "parent_session_id": null,
            "branched_from_message_id": null,
            "root_session_id": session_id,
            "background_image_path": null,
            "system_prompt": null,
            "mode": "roleplay",
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
            "memory_embeddings": embeddings,
            "memory_summary": null,
            "memory_summary_token_count": 0,
            "memory_tool_events": "[]",
            "memory_status": "idle",
            "memory_error": null,
            "memory_progress_step": null,
            "archived": false,
            "created_at": 1,
            "updated_at": 20,
            "messages": []
        })
    }

    fn source(
        standalone: Option<Value>,
        embedded: &str,
        character_id: &str,
        session_id: &str,
    ) -> LegacyBackupCompanionSharedMemoryPlan {
        let mut documents = vec![
            document(
                LegacyBackupDocumentKind::Characters,
                json!([{
                    "id": character_id,
                    "name": "Mira",
                    "mode": "roleplay",
                    "created_at": 1,
                    "updated_at": 20
                }]),
            ),
            document(
                LegacyBackupDocumentKind::Sessions,
                json!([session(session_id, character_id, embedded)]),
            ),
        ];
        if let Some(standalone) = standalone {
            documents.push(document(
                LegacyBackupDocumentKind::MemoryEmbeddings,
                standalone,
            ));
        }
        let inventory = LegacyBackupInventory {
            version: 1,
            created_at: 1,
            app_version: "legacy".into(),
            source_hash: ContentHash::parse("bb".repeat(32)).expect("source hash"),
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
        let notes = plan_legacy_backup_scheduled_notes(group).expect("scheduled-note plan");
        plan_legacy_backup_companion_shared_memory(notes).expect("shared-memory plan")
    }

    #[test]
    fn standalone_embeddings_preserve_every_field_and_match_embedded_copy() {
        let character_id = id(1);
        let session_id = id(2);
        let memory_id = id(3);
        let raw = serde_json::to_string(&json!([memory(&memory_id, 64)])).expect("memory JSON");
        let standalone = json!([{
            "session_id": session_id,
            "session_kind": "session",
            "memory_embeddings": raw
        }]);
        let plan = plan_legacy_backup_memory_embeddings(source(
            Some(standalone),
            &raw,
            &character_id,
            &session_id,
        ))
        .expect("embedding plan");
        let owner = &plan.owners[0];
        let memory = &owner.memories[0];
        assert_eq!(owner.kind, LegacyBackupMemoryOwnerKind::DirectConversation);
        assert_eq!(owner.memory_embeddings_json, raw);
        assert_eq!(memory.embedding.len(), 64);
        assert_eq!(memory.token_count, 6);
        assert!(memory.is_pinned);
        assert_eq!(memory.embedding_source_version.as_deref(), Some("v4"));
        assert_eq!(
            memory.materialization,
            LegacyBackupMemoryMaterialization::InitialItemAndProjection
        );
    }

    #[test]
    fn missing_standalone_document_is_explicit_when_embedded_copy_is_empty() {
        let character_id = id(10);
        let session_id = id(11);
        let plan =
            plan_legacy_backup_memory_embeddings(source(None, "[]", &character_id, &session_id))
                .expect("empty embedding plan");
        assert!(plan.owners.is_empty());
        assert!(plan.notices.iter().any(|notice| {
            notice.document == LegacyBackupDocumentKind::MemoryEmbeddings
                && notice.kind == LegacyBackupConversionNoticeKind::Absent
        }));
    }

    #[test]
    fn standalone_embeddings_reject_conflicts_duplicates_and_invalid_dimensions() {
        let character_id = id(20);
        let session_id = id(21);
        let memory_id = id(22);
        let valid_raw =
            serde_json::to_string(&json!([memory(&memory_id, 64)])).expect("valid memory JSON");
        let conflicting_raw = serde_json::to_string(&json!([memory(&memory_id, 128)]))
            .expect("conflicting memory JSON");
        let conflicting = json!([{
            "session_id": session_id,
            "session_kind": "session",
            "memory_embeddings": conflicting_raw
        }]);
        assert!(matches!(
            plan_legacy_backup_memory_embeddings(source(
                Some(conflicting),
                &valid_raw,
                &character_id,
                &session_id,
            )),
            Err(LegacyBackupMemoryEmbeddingError::Conflict { ref field })
                if field == "[0].memory_embeddings"
        ));

        let duplicate_raw =
            serde_json::to_string(&json!([memory(&memory_id, 64), memory(&memory_id, 64)]))
                .expect("duplicate memory JSON");
        let duplicate = json!([{
            "session_id": session_id,
            "session_kind": "session",
            "memory_embeddings": duplicate_raw
        }]);
        assert!(matches!(
            plan_legacy_backup_memory_embeddings(source(
                Some(duplicate),
                &duplicate_raw,
                &character_id,
                &session_id,
            )),
            Err(LegacyBackupMemoryEmbeddingError::Malformed { ref field })
                if field == "[0].memory_embeddings[1].id"
        ));

        let invalid_raw =
            serde_json::to_string(&json!([memory(&memory_id, 63)])).expect("invalid memory JSON");
        let invalid = json!([{
            "session_id": session_id,
            "session_kind": "session",
            "memory_embeddings": invalid_raw
        }]);
        assert!(matches!(
            plan_legacy_backup_memory_embeddings(source(
                Some(invalid),
                &invalid_raw,
                &character_id,
                &session_id,
            )),
            Err(LegacyBackupMemoryEmbeddingError::Malformed { ref field })
                if field == "[0].memory_embeddings[0].embedding_dimensions"
        ));
    }
}
