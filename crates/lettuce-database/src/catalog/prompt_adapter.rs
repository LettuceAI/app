//! SQLite persistence for provider-neutral prompt documents.
//!
//! Prompt documents are aggregate roots: entries never escape the root CAS,
//! and every read reconstructs and validates the complete snapshot before it
//! is returned.  JSON is used only for the intentionally extensible condition,
//! payload, and provenance values; each value has a strict format envelope.

use std::collections::{HashMap, HashSet};

use lettuce_context::{
    BuiltInPromptSeed, BuiltInReconcileAction, BuiltInReconcileMode, BuiltInReconcileOutcome,
    BuiltInReconcileRequest, LifecycleFilter, LifecycleStatus, PromptBehaviorVersion,
    PromptBootstrapError, PromptBootstrapPort, PromptBuiltInChange, PromptDependencyError,
    PromptDependencyReader, PromptDocument, PromptEntry, PromptEntryDraft, PromptEntryEdit,
    PromptEntryInsertionTarget, PromptEntryMutation, PromptEntryPosition, PromptEntryRole,
    PromptLibraryQuery, PromptLookupResult, PromptMetadataDraft, PromptMutationResult,
    PromptProvenance, PromptPurpose, PromptReference, PromptReferenceOwner, PromptRepository,
    PromptRepositoryError, PromptSeedBase, PromptSeedBaseEntry, PromptValidationError,
    built_in_update, classify_prompt_lookup,
};
use lettuce_types::{
    CharacterId, GroupId, Page, PromptDocumentId, PromptEntryId, Revision, TimestampMillis,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::Database;

const JSON_FORMAT_VERSION: u32 = 1;
const CURSOR_FORMAT_VERSION: u32 = 1;
const BUILT_IN_PROMPT_NAMESPACE: uuid::Uuid =
    uuid::Uuid::from_u128(0x5c0f_2d4e_8a1b_5e3c_9f76_0b2d_4a81_c3e9);

/// Built-in prompt identities are derived from their keys, so every device
/// seeds the same ids and cross-device references (characters, settings,
/// derived prompts) resolve.
pub(crate) fn built_in_document_id(key: &str) -> PromptDocumentId {
    PromptDocumentId::from_uuid(uuid::Uuid::new_v5(
        &BUILT_IN_PROMPT_NAMESPACE,
        key.as_bytes(),
    ))
}

fn built_in_entry_id(document: PromptDocumentId, entry_key: &str) -> PromptEntryId {
    PromptEntryId::from_uuid(uuid::Uuid::new_v5(
        &document.as_uuid(),
        entry_key.as_bytes(),
    ))
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope<T> {
    format_version: u32,
    value: T,
}

fn encode<T: Serialize>(value: &T) -> Result<String, PromptRepositoryError> {
    serde_json::to_string(&Envelope {
        format_version: JSON_FORMAT_VERSION,
        value,
    })
    .map_err(|_| PromptRepositoryError::Failure("prompt JSON encoding failed".into()))
}

fn decode<T: DeserializeOwned>(value: &str) -> Result<T, rusqlite::Error> {
    let envelope: Envelope<T> =
        serde_json::from_str(value).map_err(|_| rusqlite::Error::InvalidQuery)?;
    if envelope.format_version != JSON_FORMAT_VERSION {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(envelope.value)
}

fn invalid() -> rusqlite::Error {
    rusqlite::Error::InvalidQuery
}

fn storage(error: rusqlite::Error) -> PromptRepositoryError {
    match error {
        rusqlite::Error::InvalidQuery => {
            PromptRepositoryError::Failure("invalid or corrupt prompt storage".into())
        }
        rusqlite::Error::SqliteFailure(code, _) if matches!(code.extended_code, 1555 | 2067) => {
            PromptRepositoryError::Failure("prompt storage uniqueness failure".into())
        }
        rusqlite::Error::SqliteFailure(code, _) if code.extended_code == 787 => {
            PromptRepositoryError::Failure("prompt storage reference failure".into())
        }
        _ => PromptRepositoryError::Failure("prompt storage failure".into()),
    }
}

fn bootstrap_storage(error: rusqlite::Error) -> PromptBootstrapError {
    PromptBootstrapError::Failure(storage(error).to_string())
}

fn parse_id<T: std::str::FromStr>(value: String) -> Result<T, rusqlite::Error> {
    value.parse().map_err(|_| invalid())
}

fn revision(value: i64) -> Result<Revision, rusqlite::Error> {
    u64::try_from(value)
        .map(Revision::new)
        .map_err(|_| invalid())
}

fn sql_revision(value: Revision) -> Result<i64, PromptRepositoryError> {
    i64::try_from(value.get())
        .map_err(|_| PromptRepositoryError::Failure("revision overflow".into()))
}

fn status_name(value: LifecycleStatus) -> &'static str {
    match value {
        LifecycleStatus::Active => "active",
        LifecycleStatus::Archived => "archived",
    }
}

fn status_from_name(value: &str) -> Result<LifecycleStatus, rusqlite::Error> {
    match value {
        "active" => Ok(LifecycleStatus::Active),
        "archived" => Ok(LifecycleStatus::Archived),
        _ => Err(invalid()),
    }
}

fn purpose_name(value: PromptPurpose) -> Option<&'static str> {
    Some(match value {
        PromptPurpose::Undefined => return None,
        PromptPurpose::DirectChat => "direct_chat",
        PromptPurpose::CompanionChat => "companion_chat",
        PromptPurpose::GroupChatRoleplay => "group_chat_roleplay",
        PromptPurpose::DynamicMemorySummarizer => "dynamic_memory_summarizer",
        PromptPurpose::DynamicMemoryManager => "dynamic_memory_manager",
        PromptPurpose::ReplyHelperRoleplay => "reply_helper_roleplay",
        PromptPurpose::ReplyHelperConversational => "reply_helper_conversational",
        PromptPurpose::LorebookEntryWriter => "lorebook_entry_writer",
        PromptPurpose::LorebookKeywordGenerator => "lorebook_keyword_generator",
        PromptPurpose::LorebookGeneratorPlanner => "lorebook_generator_planner",
        PromptPurpose::LorebookGeneratorWriter => "lorebook_generator_writer",
        PromptPurpose::LorebookGeneratorRefine => "lorebook_generator_refine",
        PromptPurpose::LorebookGeneratorCoherence => "lorebook_generator_coherence",
        PromptPurpose::AvatarGeneration => "avatar_generation",
        PromptPurpose::AvatarEditRequest => "avatar_edit_request",
        PromptPurpose::SceneGeneration => "scene_generation",
        PromptPurpose::ScenePromptWriter => "scene_prompt_writer",
        PromptPurpose::DesignReferenceWriter => "design_reference_writer",
        PromptPurpose::CompanionSoulWriter => "companion_soul_writer",
        PromptPurpose::CompanionGrowthcycle => "companion_growthcycle",
        PromptPurpose::CompanionConsolidation => "companion_consolidation",
        PromptPurpose::RuntimeText => "runtime_text",
    })
}

pub(crate) fn purpose_from_name(value: &str) -> Result<PromptPurpose, rusqlite::Error> {
    match value {
        "direct_chat" => Ok(PromptPurpose::DirectChat),
        "companion_chat" => Ok(PromptPurpose::CompanionChat),
        "group_chat_roleplay" | "group_chat_conversational" => Ok(PromptPurpose::GroupChatRoleplay),
        "dynamic_memory_summarizer" => Ok(PromptPurpose::DynamicMemorySummarizer),
        "dynamic_memory_manager" => Ok(PromptPurpose::DynamicMemoryManager),
        "reply_helper_roleplay" => Ok(PromptPurpose::ReplyHelperRoleplay),
        "reply_helper_conversational" => Ok(PromptPurpose::ReplyHelperConversational),
        "lorebook_entry_writer" => Ok(PromptPurpose::LorebookEntryWriter),
        "lorebook_keyword_generator" => Ok(PromptPurpose::LorebookKeywordGenerator),
        "lorebook_generator_planner" => Ok(PromptPurpose::LorebookGeneratorPlanner),
        "lorebook_generator_writer" => Ok(PromptPurpose::LorebookGeneratorWriter),
        "lorebook_generator_refine" => Ok(PromptPurpose::LorebookGeneratorRefine),
        "lorebook_generator_coherence" => Ok(PromptPurpose::LorebookGeneratorCoherence),
        "avatar_generation" => Ok(PromptPurpose::AvatarGeneration),
        "avatar_edit_request" => Ok(PromptPurpose::AvatarEditRequest),
        "scene_generation" => Ok(PromptPurpose::SceneGeneration),
        "scene_prompt_writer" => Ok(PromptPurpose::ScenePromptWriter),
        "design_reference_writer" => Ok(PromptPurpose::DesignReferenceWriter),
        "companion_soul_writer" => Ok(PromptPurpose::CompanionSoulWriter),
        "companion_growthcycle" => Ok(PromptPurpose::CompanionGrowthcycle),
        "companion_consolidation" => Ok(PromptPurpose::CompanionConsolidation),
        "runtime_text" => Ok(PromptPurpose::RuntimeText),
        _ => Err(invalid()),
    }
}

fn behavior_name(value: PromptBehaviorVersion) -> &'static str {
    match value {
        PromptBehaviorVersion::LegacyV1 => "legacy_v1",
        PromptBehaviorVersion::DeterministicV2 => "deterministic_v2",
    }
}

fn behavior_from_name(value: &str) -> Result<PromptBehaviorVersion, rusqlite::Error> {
    match value {
        "legacy_v1" => Ok(PromptBehaviorVersion::LegacyV1),
        "deterministic_v2" => Ok(PromptBehaviorVersion::DeterministicV2),
        _ => Err(invalid()),
    }
}

fn role_name(value: PromptEntryRole) -> &'static str {
    match value {
        PromptEntryRole::System => "system",
        PromptEntryRole::User => "user",
        PromptEntryRole::Assistant => "assistant",
    }
}

fn role_from_name(value: &str) -> Result<PromptEntryRole, rusqlite::Error> {
    match value {
        "system" => Ok(PromptEntryRole::System),
        "user" => Ok(PromptEntryRole::User),
        "assistant" => Ok(PromptEntryRole::Assistant),
        _ => Err(invalid()),
    }
}

fn position_name(value: PromptEntryPosition) -> &'static str {
    match value {
        PromptEntryPosition::Relative => "relative",
        PromptEntryPosition::InChat => "in_chat",
        PromptEntryPosition::Conditional => "conditional",
        PromptEntryPosition::Interval => "interval",
    }
}

fn position_from_name(value: &str) -> Result<PromptEntryPosition, rusqlite::Error> {
    match value {
        "relative" => Ok(PromptEntryPosition::Relative),
        "in_chat" => Ok(PromptEntryPosition::InChat),
        "conditional" => Ok(PromptEntryPosition::Conditional),
        "interval" => Ok(PromptEntryPosition::Interval),
        _ => Err(invalid()),
    }
}

fn provenance_kind(value: &PromptProvenance) -> (&'static str, Option<String>, Option<String>) {
    match value {
        PromptProvenance::BuiltIn { key, .. } => ("built_in", Some(key.trim().to_owned()), None),
        PromptProvenance::User => ("user", None, None),
        PromptProvenance::Derived { source, .. } => ("derived", None, Some(source.to_string())),
        PromptProvenance::Imported => ("imported", None, None),
    }
}

fn derived_source_name(value: &PromptProvenance) -> Option<&str> {
    match value {
        PromptProvenance::Derived { source_name, .. } => Some(source_name.as_str()),
        _ => None,
    }
}

fn provenance_from_columns(
    kind: &str,
    key: Option<String>,
    source: Option<String>,
    payload: &str,
) -> Result<PromptProvenance, rusqlite::Error> {
    let provenance: PromptProvenance = decode(payload)?;
    let (expected_kind, expected_key, expected_source) = provenance_kind(&provenance);
    if kind != expected_kind || key != expected_key || source != expected_source {
        return Err(invalid());
    }
    Ok(provenance)
}

fn cursor_encode(
    updated_at: TimestampMillis,
    id: PromptDocumentId,
) -> Result<String, PromptRepositoryError> {
    let payload = serde_json::to_vec(&Envelope {
        format_version: CURSOR_FORMAT_VERSION,
        value: (updated_at.get(), id.to_string()),
    })
    .map_err(|_| PromptRepositoryError::Failure("prompt cursor encoding failed".into()))?;
    Ok(crate::hex_encode(&payload))
}

fn cursor_decode(
    value: Option<&str>,
) -> Result<Option<(i64, PromptDocumentId)>, PromptRepositoryError> {
    let Some(value) = value else { return Ok(None) };
    let bytes = crate::hex_decode(value)
        .map_err(|_| PromptRepositoryError::Failure("invalid prompt cursor".into()))?;
    let envelope: Envelope<(i64, String)> = serde_json::from_slice(&bytes)
        .map_err(|_| PromptRepositoryError::Failure("invalid prompt cursor".into()))?;
    if envelope.format_version != CURSOR_FORMAT_VERSION {
        return Err(PromptRepositoryError::Failure(
            "invalid prompt cursor version".into(),
        ));
    }
    let id = envelope
        .value
        .1
        .parse()
        .map_err(|_| PromptRepositoryError::Failure("invalid prompt cursor id".into()))?;
    Ok(Some((envelope.value.0, id)))
}

fn read_entry(row: &Row<'_>) -> rusqlite::Result<PromptEntry> {
    Ok(PromptEntry {
        id: parse_id(row.get(0)?)?,
        built_in_entry_key: row.get(1)?,
        name: row.get(2)?,
        role: role_from_name(&row.get::<_, String>(3)?)?,
        content: row.get(4)?,
        enabled: row.get(5)?,
        injection_position: position_from_name(&row.get::<_, String>(6)?)?,
        depth: row.get::<_, i64>(7)?.try_into().map_err(|_| invalid())?,
        conditional_min_messages: row
            .get::<_, Option<i64>>(8)?
            .map(|value| value.try_into().map_err(|_| invalid()))
            .transpose()?,
        interval_turns: row
            .get::<_, Option<i64>>(9)?
            .map(|value| value.try_into().map_err(|_| invalid()))
            .transpose()?,
        system_prompt: row.get(10)?,
        conditions: row
            .get::<_, Option<String>>(11)?
            .map(|value| decode(&value))
            .transpose()?,
        payload: row
            .get::<_, Option<String>>(12)?
            .map(|value| decode(&value))
            .transpose()?,
    })
}

fn read_document_row(row: &Row<'_>) -> rusqlite::Result<PromptDocument> {
    let id: PromptDocumentId = parse_id(row.get(0)?)?;
    let document = PromptDocument {
        id,
        status: status_from_name(&row.get::<_, String>(1)?)?,
        name: row.get(2)?,
        purpose: purpose_from_name(&row.get::<_, String>(3)?)?,
        entries: Vec::new(),
        condense: row.get(4)?,
        behavior_version: behavior_from_name(&row.get::<_, String>(5)?)?,
        provenance: provenance_from_columns(
            &row.get::<_, String>(6)?,
            row.get(7)?,
            row.get(8)?,
            &row.get::<_, String>(9)?,
        )?,
        revision: revision(row.get(10)?)?,
        created_at: TimestampMillis::new(row.get(11)?),
        updated_at: TimestampMillis::new(row.get(12)?),
    };
    document.validate().map_err(|_| invalid())?;
    Ok(document)
}

pub(crate) fn load_document(
    connection: &Connection,
    id: PromptDocumentId,
) -> rusqlite::Result<Option<PromptDocument>> {
    let Some(mut document) = connection
        .query_row(
            "SELECT id,status,name,purpose,condense,behavior_version,provenance_kind,built_in_key,derived_source_id,provenance_json,revision,created_at,updated_at FROM prompt_documents WHERE id=?1",
            [id.to_string()],
            read_document_row,
        )
        .optional()? else { return Ok(None) };
    let mut statement = connection.prepare(
            "SELECT id,built_in_entry_key,name,role,content,enabled,injection_position,depth,conditional_min_messages,interval_turns,system_prompt,conditions_json,payload_json,ordinal,revision,created_at,updated_at FROM prompt_entries WHERE prompt_id=?1 ORDER BY ordinal ASC,id ASC",
    )?;
    let mut entries = Vec::new();
    for (expected_ordinal, row) in statement
        .query_map([id.to_string()], |row| {
            Ok((
                read_entry(row)?,
                row.get::<_, i64>(13)?,
                revision(row.get(14)?)?,
                TimestampMillis::new(row.get(15)?),
                TimestampMillis::new(row.get(16)?),
            ))
        })?
        .enumerate()
    {
        let (entry, ordinal, entry_revision, created_at, updated_at) = row?;
        if ordinal != i64::try_from(expected_ordinal).map_err(|_| invalid())? {
            return Err(invalid());
        }
        if entry_revision.get() == 0 || created_at > updated_at {
            return Err(invalid());
        }
        entries.push(entry);
    }
    document.entries = entries;
    document.validate().map_err(|_| invalid())?;
    Ok(Some(document))
}

fn load_required(
    connection: &Connection,
    id: PromptDocumentId,
) -> Result<PromptDocument, PromptRepositoryError> {
    load_document(connection, id)
        .map_err(storage)?
        .ok_or(PromptRepositoryError::NotFound)
}

fn next_revision(value: Revision) -> Result<Revision, PromptRepositoryError> {
    value
        .next()
        .map_err(|_| PromptRepositoryError::Failure("revision overflow".into()))
}

fn validate_metadata(metadata: &PromptMetadataDraft) -> Result<(), PromptRepositoryError> {
    metadata.validate().map_err(PromptRepositoryError::Invalid)
}

fn validate_entries(entries: &[PromptEntryDraft]) -> Result<(), PromptRepositoryError> {
    for entry in entries {
        entry.validate().map_err(PromptRepositoryError::Invalid)?;
        if entry.built_in_entry_key.is_some() {
            return Err(PromptRepositoryError::Invalid(
                PromptValidationError::BuiltInEntryKeyNotAllowed,
            ));
        }
    }
    Ok(())
}

fn entry_params(
    entry: &PromptEntry,
    prompt_id: PromptDocumentId,
    ordinal: usize,
    revision: Revision,
    created_at: TimestampMillis,
    updated_at: TimestampMillis,
) -> Result<Vec<Box<dyn rusqlite::ToSql>>, PromptRepositoryError> {
    let conditions = entry.conditions.as_ref().map(encode).transpose()?;
    let payload = entry.payload.as_ref().map(encode).transpose()?;
    Ok(vec![
        Box::new(entry.id.to_string()),
        Box::new(prompt_id.to_string()),
        Box::new(entry.built_in_entry_key.clone()),
        Box::new(entry.name.clone()),
        Box::new(role_name(entry.role)),
        Box::new(entry.content.clone()),
        Box::new(entry.enabled),
        Box::new(position_name(entry.injection_position)),
        Box::new(i64::from(entry.depth)),
        Box::new(entry.conditional_min_messages.map(i64::from)),
        Box::new(entry.interval_turns.map(i64::from)),
        Box::new(entry.system_prompt),
        Box::new(conditions),
        Box::new(payload),
        Box::new(
            i64::try_from(ordinal)
                .map_err(|_| PromptRepositoryError::Failure("entry ordinal overflow".into()))?,
        ),
        Box::new(sql_revision(revision)?),
        Box::new(created_at.get()),
        Box::new(updated_at.get()),
    ])
}

fn insert_entry(
    tx: &Transaction<'_>,
    entry: &PromptEntry,
    prompt_id: PromptDocumentId,
    ordinal: usize,
    revision: Revision,
    created_at: TimestampMillis,
    updated_at: TimestampMillis,
) -> Result<(), PromptRepositoryError> {
    let values = entry_params(entry, prompt_id, ordinal, revision, created_at, updated_at)?;
    tx.execute(
        "INSERT INTO prompt_entries(id,prompt_id,built_in_entry_key,name,role,content,enabled,injection_position,depth,conditional_min_messages,interval_turns,system_prompt,conditions_json,payload_json,ordinal,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)",
        rusqlite::params_from_iter(values.iter().map(|value| value.as_ref() as &dyn rusqlite::ToSql)),
    )
    .map_err(storage)?;
    Ok(())
}

fn metadata_document(
    id: PromptDocumentId,
    metadata: PromptMetadataDraft,
    entries: Vec<PromptEntry>,
    provenance: PromptProvenance,
    now: TimestampMillis,
) -> Result<PromptDocument, PromptRepositoryError> {
    let document = PromptDocument {
        id,
        status: LifecycleStatus::Active,
        name: metadata.name,
        purpose: metadata.purpose,
        entries,
        condense: metadata.condense,
        behavior_version: metadata.behavior_version,
        provenance,
        revision: Revision::INITIAL,
        created_at: now,
        updated_at: now,
    };
    document
        .validate()
        .map_err(PromptRepositoryError::Invalid)?;
    Ok(document)
}

fn entry_from_draft(draft: PromptEntryDraft, id: PromptEntryId) -> PromptEntry {
    draft.into_entry(id)
}

fn authored_entry_equal(entry: &PromptEntry, draft: &PromptEntryDraft) -> bool {
    entry.name == draft.name
        && entry.role == draft.role
        && entry.content == draft.content
        && entry.enabled == draft.enabled
        && entry.injection_position == draft.injection_position
        && entry.depth == draft.depth
        && entry.conditional_min_messages == draft.conditional_min_messages
        && entry.interval_turns == draft.interval_turns
        && entry.system_prompt == draft.system_prompt
        && entry.conditions == draft.conditions
        && entry.payload == draft.payload
}

fn legacy_seed_is_unedited(
    current: &PromptDocument,
    seed: &BuiltInPromptSeed,
    current_seed: &lettuce_types::ContentHash,
    current_authored: &lettuce_types::ContentHash,
) -> bool {
    current_seed == current_authored
        && current.entries.len() == seed.entries.len()
        && current
            .entries
            .iter()
            .zip(&seed.entries)
            .all(|(entry, _)| entry.built_in_entry_key.is_none())
}

fn reconcile_entries(
    current: &PromptDocument,
    seed: &BuiltInPromptSeed,
    legacy_ordinal_match: bool,
) -> (Vec<PromptEntry>, HashSet<PromptEntryId>, bool) {
    let by_key = current
        .entries
        .iter()
        .filter_map(|entry| entry.built_in_entry_key.as_deref().map(|key| (key, entry)))
        .collect::<HashMap<_, _>>();
    let mut touched = HashSet::new();
    let mut changed = current.entries.len() != seed.entries.len();
    let entries = seed
        .entries
        .iter()
        .enumerate()
        .map(|(ordinal, draft)| {
            let matched = by_key
                .get(draft.built_in_entry_key.as_deref().expect("validated seed"))
                .copied()
                .or_else(|| {
                    legacy_ordinal_match
                        .then(|| current.entries.get(ordinal))
                        .flatten()
                });
            if let Some(current_entry) = matched {
                if !authored_entry_equal(current_entry, draft)
                    || current_entry.built_in_entry_key != draft.built_in_entry_key
                    || current.entries.get(ordinal).map(|entry| entry.id) != Some(current_entry.id)
                {
                    touched.insert(current_entry.id);
                    changed = true;
                }
                entry_from_draft(draft.clone(), current_entry.id)
            } else {
                changed = true;
                let entry_key = draft.built_in_entry_key.as_deref().expect("validated seed");
                entry_from_draft(draft.clone(), built_in_entry_id(current.id, entry_key))
            }
        })
        .collect();
    (entries, touched, changed)
}

pub(crate) fn insert_imported_document(
    tx: &Transaction<'_>,
    document: &PromptDocument,
) -> Result<(), PromptRepositoryError> {
    document
        .validate()
        .map_err(|error| PromptRepositoryError::Failure(error.to_string()))?;
    insert_root(tx, document)
}

fn insert_root(
    tx: &Transaction<'_>,
    document: &PromptDocument,
) -> Result<(), PromptRepositoryError> {
    let provenance_json = encode(&document.provenance)?;
    let (kind, key, source) = provenance_kind(&document.provenance);
    tx.execute(
        "INSERT INTO prompt_documents(id,status,name,purpose,condense,behavior_version,provenance_kind,built_in_key,derived_source_id,provenance_json,revision,created_at,updated_at,derived_source_name) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
        params![
            document.id.to_string(),
            status_name(document.status),
            document.name,
            purpose_name(document.purpose),
            document.condense,
            behavior_name(document.behavior_version),
            kind,
            key,
            source,
            provenance_json,
            sql_revision(document.revision)?,
            document.created_at.get(),
            document.updated_at.get(),
            derived_source_name(&document.provenance)
        ],
    )
    .map_err(storage)?;
    for (ordinal, entry) in document.entries.iter().enumerate() {
        insert_entry(
            tx,
            entry,
            document.id,
            ordinal,
            Revision::INITIAL,
            document.created_at,
            document.updated_at,
        )?;
    }
    Ok(())
}

fn replace_entries(
    tx: &Transaction<'_>,
    document: &PromptDocument,
    entry_created_at: TimestampMillis,
    touched: &HashSet<PromptEntryId>,
) -> Result<(), PromptRepositoryError> {
    let mut metadata = HashMap::new();
    {
        let mut statement = tx
            .prepare(
                "SELECT id,revision,created_at,updated_at FROM prompt_entries WHERE prompt_id=?1",
            )
            .map_err(storage)?;
        for row in statement
            .query_map([document.id.to_string()], |row| {
                Ok((
                    parse_id::<PromptEntryId>(row.get(0)?)?,
                    revision(row.get(1)?)?,
                    TimestampMillis::new(row.get(2)?),
                    TimestampMillis::new(row.get(3)?),
                ))
            })
            .map_err(storage)?
        {
            let (id, revision, created_at, updated_at) = row.map_err(storage)?;
            metadata.insert(id, (revision, created_at, updated_at));
        }
    }
    tx.execute(
        "DELETE FROM prompt_entries WHERE prompt_id=?1",
        [document.id.to_string()],
    )
    .map_err(storage)?;
    for (ordinal, entry) in document.entries.iter().enumerate() {
        let (revision, created_at, updated_at) =
            if let Some((old_revision, created_at, old_updated_at)) = metadata.get(&entry.id) {
                let revision = if touched.contains(&entry.id) {
                    next_revision(*old_revision)?
                } else {
                    *old_revision
                };
                let updated_at = if touched.contains(&entry.id) {
                    entry_created_at
                } else {
                    *old_updated_at
                };
                (revision, *created_at, updated_at)
            } else {
                (Revision::INITIAL, entry_created_at, entry_created_at)
            };
        insert_entry(
            tx,
            entry,
            document.id,
            ordinal,
            revision,
            created_at,
            updated_at,
        )?;
    }
    Ok(())
}

fn update_root(
    tx: &Transaction<'_>,
    document: &PromptDocument,
    expected: Revision,
) -> Result<(), PromptRepositoryError> {
    let provenance_json = encode(&document.provenance)?;
    let (kind, key, source) = provenance_kind(&document.provenance);
    let changed = tx.execute(
        "UPDATE prompt_documents SET status=?2,name=?3,purpose=?4,condense=?5,behavior_version=?6,provenance_kind=?7,built_in_key=?8,derived_source_id=?9,provenance_json=?10,revision=?11,updated_at=?12,derived_source_name=?14 WHERE id=?1 AND revision=?13",
        params![
            document.id.to_string(), status_name(document.status), document.name,
            purpose_name(document.purpose), document.condense, behavior_name(document.behavior_version),
            kind, key, source, provenance_json, sql_revision(document.revision)?, document.updated_at.get(),
            sql_revision(expected)?, derived_source_name(&document.provenance)
        ],
    ).map_err(storage)?;
    if changed == 0 {
        let _actual = load_document(tx, document.id)
            .map_err(storage)?
            .ok_or(PromptRepositoryError::NotFound)?
            .revision;
        return Err(PromptRepositoryError::Conflict);
    }
    Ok(())
}

fn check_expected(
    document: &PromptDocument,
    expected: Revision,
) -> Result<(), PromptRepositoryError> {
    if document.revision != expected {
        return Err(PromptRepositoryError::Conflict);
    }
    if document.status == LifecycleStatus::Archived {
        return Err(PromptRepositoryError::Failure(
            "archived prompt cannot be edited".into(),
        ));
    }
    Ok(())
}

fn authored_digest(
    document: &PromptDocument,
    key: &str,
    seed_version: u32,
    required: bool,
    protected: bool,
) -> Result<lettuce_types::ContentHash, PromptRepositoryError> {
    let seed = BuiltInPromptSeed {
        key: key.to_owned(),
        aliases: Vec::new(),
        seed_version,
        metadata: PromptMetadataDraft {
            name: document.name.clone(),
            purpose: document.purpose,
            condense: document.condense,
            behavior_version: document.behavior_version,
        },
        entries: document
            .entries
            .iter()
            .cloned()
            .map(PromptEntryDraft::from)
            .collect(),
        required,
        protected,
    };
    seed.computed_seed_digest()
        .map_err(PromptRepositoryError::Invalid)
}

fn refresh_provenance(document: &mut PromptDocument) -> Result<(), PromptRepositoryError> {
    let current = document.provenance.clone();
    if let PromptProvenance::BuiltIn {
        key,
        seed_version,
        seed_digest,
        required,
        protected,
        seed_base,
        ..
    } = current
    {
        document.provenance = PromptProvenance::BuiltIn {
            authored_digest: authored_digest(document, &key, seed_version, required, protected)?,
            key,
            seed_version,
            seed_digest,
            required,
            protected,
            seed_base,
        };
    }
    Ok(())
}

fn mutate_entries_in_memory(
    document: &mut PromptDocument,
    mutation: PromptEntryMutation,
    now: TimestampMillis,
) -> Result<HashSet<PromptEntryId>, PromptRepositoryError> {
    mutation
        .validate()
        .map_err(PromptRepositoryError::Invalid)?;
    let mut touched = HashSet::new();
    match mutation {
        PromptEntryMutation::Add { draft, target } => {
            if draft.built_in_entry_key.is_some() {
                return Err(PromptRepositoryError::Invalid(
                    PromptValidationError::BuiltInEntryKeyNotAllowed,
                ));
            }
            let entry = entry_from_draft(draft, PromptEntryId::new());
            let index = match target {
                PromptEntryInsertionTarget::Append => document.entries.len(),
                PromptEntryInsertionTarget::At(index) if index <= document.entries.len() => index,
                PromptEntryInsertionTarget::At(_) => {
                    return Err(PromptRepositoryError::Failure(
                        "entry insertion index out of bounds".into(),
                    ));
                }
            };
            touched.insert(entry.id);
            document.entries.insert(index, entry);
        }
        PromptEntryMutation::Update { entry_id, draft } => {
            let entry = document
                .entries
                .iter_mut()
                .find(|entry| entry.id == entry_id)
                .ok_or(PromptRepositoryError::EntryNotFound)?;
            if draft.built_in_entry_key.is_some()
                && draft.built_in_entry_key != entry.built_in_entry_key
            {
                return Err(PromptRepositoryError::Invalid(
                    PromptValidationError::BuiltInEntryKeyNotAllowed,
                ));
            }
            let built_in_entry_key = entry.built_in_entry_key.clone();
            *entry = PromptEntry {
                id: entry_id,
                built_in_entry_key,
                name: draft.name,
                role: draft.role,
                content: draft.content,
                enabled: draft.enabled,
                injection_position: draft.injection_position,
                depth: draft.depth,
                conditional_min_messages: draft.conditional_min_messages,
                interval_turns: draft.interval_turns,
                system_prompt: draft.system_prompt,
                conditions: draft.conditions,
                payload: draft.payload,
            };
            touched.insert(entry_id);
        }
        PromptEntryMutation::Remove { entry_id } => {
            let index = document
                .entries
                .iter()
                .position(|entry| entry.id == entry_id)
                .ok_or(PromptRepositoryError::EntryNotFound)?;
            document.entries.remove(index);
        }
        PromptEntryMutation::Replace { drafts } => {
            if drafts
                .iter()
                .any(|draft| draft.built_in_entry_key.is_some())
            {
                return Err(PromptRepositoryError::Invalid(
                    PromptValidationError::BuiltInEntryKeyNotAllowed,
                ));
            }
            document.entries = drafts
                .into_iter()
                .map(|draft| entry_from_draft(draft, PromptEntryId::new()))
                .collect();
        }
        PromptEntryMutation::Reorder {
            entry_id,
            target_index,
        } => {
            if target_index >= document.entries.len() {
                return Err(PromptRepositoryError::Failure(
                    "entry reorder index out of bounds".into(),
                ));
            }
            let index = document
                .entries
                .iter()
                .position(|entry| entry.id == entry_id)
                .ok_or(PromptRepositoryError::EntryNotFound)?;
            let entry = document.entries.remove(index);
            document.entries.insert(target_index, entry);
            touched.extend(document.entries.iter().map(|entry| entry.id));
        }
    }
    document.revision = next_revision(document.revision)?;
    document.updated_at = now;
    document
        .validate()
        .map_err(PromptRepositoryError::Invalid)?;
    Ok(touched)
}

fn load_page(
    tx: &Transaction<'_>,
    ids: Vec<PromptDocumentId>,
    limit: usize,
) -> Result<Page<PromptDocument>, PromptRepositoryError> {
    let has_more = ids.len() > limit;
    let ids = ids.into_iter().take(limit).collect::<Vec<_>>();
    let mut items = Vec::with_capacity(ids.len());
    for id in ids {
        items.push(load_required(tx, id)?);
    }
    let next_cursor = if has_more {
        items
            .last()
            .map(|item| cursor_encode(item.updated_at, item.id))
            .transpose()?
    } else {
        None
    };
    Ok(Page { items, next_cursor })
}

/// The seed base of a copy: every base key of the source carries over with
/// its base digest. A source entry that reappears unchanged in the copy is
/// held by that copy entry; a key without one is held by no entry, so it
/// only reports a change when the seed moves.
fn copied_seed_base(source: &PromptDocument, copy: &[PromptEntry]) -> Option<PromptSeedBase> {
    let base = source.provenance.seed_base()?;
    let mut claimed = HashSet::new();
    let entries = base
        .entries
        .iter()
        .map(|base_entry| {
            let held = source
                .entry_for_seed_key(&base_entry.entry_key)
                .map(PromptEntry::authored_digest)
                .and_then(|digest| {
                    copy.iter().find(|entry| {
                        !claimed.contains(&entry.id) && entry.authored_digest() == digest
                    })
                });
            if let Some(held) = held {
                claimed.insert(held.id);
            }
            PromptSeedBaseEntry {
                entry_key: base_entry.entry_key.clone(),
                entry_id: held.map(|held| held.id),
                seed_entry_digest: base_entry.seed_entry_digest.clone(),
            }
        })
        .collect();
    Some(PromptSeedBase {
        built_in_key: base.built_in_key.clone(),
        entries,
    })
}

/// Creates a user prompt inside `tx`.
pub(crate) fn create_user_draft_in(
    tx: &Transaction<'_>,
    metadata: PromptMetadataDraft,
    entries: Vec<PromptEntryDraft>,
    now: TimestampMillis,
    source: Option<PromptDocumentId>,
) -> Result<PromptDocument, PromptRepositoryError> {
    validate_metadata(&metadata)?;
    validate_entries(&entries)?;
    let entries: Vec<PromptEntry> = entries
        .into_iter()
        .map(|draft| entry_from_draft(draft, PromptEntryId::new()))
        .collect();
    let provenance = match source {
        Some(id) => {
            let source = load_required(tx, id)?;
            PromptProvenance::Derived {
                seed_base: copied_seed_base(&source, &entries),
                source: source.id,
                source_name: source.name,
            }
        }
        None => PromptProvenance::User,
    };
    let document = metadata_document(PromptDocumentId::new(), metadata, entries, provenance, now)?;
    insert_root(tx, &document)?;
    load_required(tx, document.id)
}

/// Replaces a prompt's metadata and entries in one revision. An entry edit
/// naming an existing entry keeps its id and built-in key; one without an id
/// is new; entries left out are removed.
pub(crate) fn revise_document_in(
    tx: &Transaction<'_>,
    id: PromptDocumentId,
    expected_revision: Revision,
    metadata: PromptMetadataDraft,
    edits: Vec<PromptEntryEdit>,
    now: TimestampMillis,
) -> Result<PromptDocument, PromptRepositoryError> {
    validate_metadata(&metadata)?;
    let current = load_required(tx, id)?;
    check_expected(&current, expected_revision)?;
    if let PromptProvenance::BuiltIn {
        protected: true, ..
    } = current.provenance
        && (metadata.purpose != current.purpose
            || metadata.behavior_version != current.behavior_version)
    {
        return Err(PromptRepositoryError::Protected);
    }
    let mut seen = HashSet::new();
    let mut touched = HashSet::new();
    let mut entries = Vec::with_capacity(edits.len());
    for (index, edit) in edits.into_iter().enumerate() {
        edit.draft
            .validate()
            .map_err(PromptRepositoryError::Invalid)?;
        if edit.draft.built_in_entry_key.is_some() {
            return Err(PromptRepositoryError::Invalid(
                PromptValidationError::BuiltInEntryKeyNotAllowed,
            ));
        }
        let entry = match edit.entry_id {
            Some(entry_id) => {
                if !seen.insert(entry_id) {
                    return Err(PromptRepositoryError::Invalid(
                        PromptValidationError::DuplicateEntry(entry_id),
                    ));
                }
                let (position, existing) = current
                    .entries
                    .iter()
                    .enumerate()
                    .find(|(_, entry)| entry.id == entry_id)
                    .ok_or(PromptRepositoryError::EntryNotFound)?;
                let mut entry = entry_from_draft(edit.draft, entry_id);
                entry.built_in_entry_key = existing.built_in_entry_key.clone();
                if &entry != existing || position != index {
                    touched.insert(entry_id);
                }
                entry
            }
            None => {
                let entry = entry_from_draft(edit.draft, PromptEntryId::new());
                touched.insert(entry.id);
                entry
            }
        };
        entries.push(entry);
    }
    let mut proposed = current.clone();
    proposed.name = metadata.name;
    proposed.purpose = metadata.purpose;
    proposed.condense = metadata.condense;
    proposed.behavior_version = metadata.behavior_version;
    proposed.entries = entries;
    proposed.revision = next_revision(expected_revision)?;
    proposed.updated_at = now;
    refresh_provenance(&mut proposed)?;
    proposed
        .validate()
        .map_err(PromptRepositoryError::Invalid)?;
    update_root(tx, &proposed, expected_revision)?;
    replace_entries(tx, &proposed, now, &touched)?;
    load_required(tx, id)
}

impl Database {
    /// See `revise_document_in`.
    pub fn revise_prompt_document(
        &self,
        id: PromptDocumentId,
        expected_revision: Revision,
        metadata: PromptMetadataDraft,
        edits: Vec<PromptEntryEdit>,
        now: TimestampMillis,
    ) -> Result<PromptDocument, PromptRepositoryError> {
        let mut connection = self
            .connection()
            .map_err(|_| PromptRepositoryError::Failure("database lock failure".into()))?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let document = revise_document_in(&tx, id, expected_revision, metadata, edits, now)?;
        tx.commit().map_err(storage)?;
        Ok(document)
    }
}

impl PromptRepository for Database {
    fn create_user_draft(
        &self,
        metadata: PromptMetadataDraft,
        entries: Vec<PromptEntryDraft>,
        now: TimestampMillis,
    ) -> Result<PromptDocument, PromptRepositoryError> {
        validate_metadata(&metadata)?;
        validate_entries(&entries)?;
        let entries = entries
            .into_iter()
            .map(|draft| entry_from_draft(draft, PromptEntryId::new()))
            .collect();
        let document = metadata_document(
            PromptDocumentId::new(),
            metadata,
            entries,
            PromptProvenance::User,
            now,
        )?;
        let mut connection = self
            .connection()
            .map_err(|_| PromptRepositoryError::Failure("database lock failure".into()))?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        insert_root(&tx, &document)?;
        let stored = load_required(&tx, document.id)?;
        tx.commit().map_err(storage)?;
        Ok(stored)
    }

    fn get(&self, id: PromptDocumentId) -> Result<Option<PromptDocument>, PromptRepositoryError> {
        let mut connection = self
            .connection()
            .map_err(|_| PromptRepositoryError::Failure("database lock failure".into()))?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(storage)?;
        let document = load_document(&tx, id).map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(document)
    }

    fn page(
        &self,
        query: PromptLibraryQuery,
    ) -> Result<Page<PromptDocument>, PromptRepositoryError> {
        let cursor = cursor_decode(query.page.cursor.as_deref())?;
        let limit = usize::from(query.page.limit.get()).max(1);
        let mut connection = self
            .connection()
            .map_err(|_| PromptRepositoryError::Failure("database lock failure".into()))?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(storage)?;
        let purpose = match query.purpose {
            Some(PromptPurpose::Undefined) => {
                return Err(PromptRepositoryError::Invalid(
                    PromptValidationError::UndefinedPurpose,
                ));
            }
            Some(purpose) => purpose_name(purpose),
            None => None,
        };
        let (time, id) = cursor
            .map(|(time, id)| (Some(time), Some(id.to_string())))
            .unwrap_or((None, None));
        let mut statement = tx.prepare(
            "SELECT id FROM prompt_documents WHERE (?1='all' OR status=?1) AND (?2 IS NULL OR purpose=?2) AND (?3 IS NULL OR updated_at < ?3 OR (updated_at = ?3 AND id > ?4)) ORDER BY updated_at DESC,id ASC LIMIT ?5",
        ).map_err(storage)?;
        let ids = statement
            .query_map(
                params![
                    match query.status {
                        LifecycleFilter::All => "all",
                        LifecycleFilter::Active => "active",
                        LifecycleFilter::Archived => "archived",
                    },
                    purpose,
                    time,
                    id,
                    i64::try_from(limit + 1).unwrap_or(i64::MAX)
                ],
                |row| parse_id(row.get(0)?),
            )
            .map_err(storage)?
            .collect::<rusqlite::Result<Vec<PromptDocumentId>>>()
            .map_err(storage)?;
        drop(statement);
        let page = load_page(&tx, ids, limit)?;
        tx.commit().map_err(storage)?;
        Ok(page)
    }

    fn revise_metadata(
        &self,
        id: PromptDocumentId,
        expected_revision: Revision,
        metadata: PromptMetadataDraft,
        now: TimestampMillis,
    ) -> Result<PromptMutationResult, PromptRepositoryError> {
        validate_metadata(&metadata)?;
        let mut connection = self
            .connection()
            .map_err(|_| PromptRepositoryError::Failure("database lock failure".into()))?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let current = load_required(&tx, id)?;
        check_expected(&current, expected_revision)?;
        if let PromptProvenance::BuiltIn {
            protected: true, ..
        } = current.provenance
            && (metadata.purpose != current.purpose
                || metadata.behavior_version != current.behavior_version)
        {
            return Err(PromptRepositoryError::Protected);
        }
        let mut proposed = current.clone();
        proposed.name = metadata.name;
        proposed.purpose = metadata.purpose;
        proposed.condense = metadata.condense;
        proposed.behavior_version = metadata.behavior_version;
        proposed.revision = next_revision(expected_revision)?;
        proposed.updated_at = now;
        refresh_provenance(&mut proposed)?;
        proposed
            .validate()
            .map_err(PromptRepositoryError::Invalid)?;
        update_root(&tx, &proposed, expected_revision)?;
        let stored = load_required(&tx, id)?;
        tx.commit().map_err(storage)?;
        Ok(PromptMutationResult { document: stored })
    }

    fn mutate_entries(
        &self,
        id: PromptDocumentId,
        expected_revision: Revision,
        mutation: PromptEntryMutation,
        now: TimestampMillis,
    ) -> Result<PromptMutationResult, PromptRepositoryError> {
        let mut connection = self
            .connection()
            .map_err(|_| PromptRepositoryError::Failure("database lock failure".into()))?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let current = load_required(&tx, id)?;
        check_expected(&current, expected_revision)?;
        let mut proposed = current.clone();
        let touched = mutate_entries_in_memory(&mut proposed, mutation, now)?;
        refresh_provenance(&mut proposed)?;
        update_root(&tx, &proposed, expected_revision)?;
        replace_entries(&tx, &proposed, now, &touched)?;
        let stored = load_required(&tx, id)?;
        tx.commit().map_err(storage)?;
        Ok(PromptMutationResult { document: stored })
    }

    fn archive(
        &self,
        id: PromptDocumentId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<PromptMutationResult, PromptRepositoryError> {
        self.set_prompt_status(id, expected_revision, LifecycleStatus::Archived, now)
    }

    fn restore(
        &self,
        id: PromptDocumentId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<PromptMutationResult, PromptRepositoryError> {
        self.set_prompt_status(id, expected_revision, LifecycleStatus::Active, now)
    }

    fn lookup_exact(
        &self,
        id: PromptDocumentId,
        purpose: PromptPurpose,
    ) -> Result<PromptLookupResult, PromptRepositoryError> {
        Ok(classify_prompt_lookup(self.get(id)?, purpose))
    }
}

impl Database {
    fn set_prompt_status(
        &self,
        id: PromptDocumentId,
        expected_revision: Revision,
        status: LifecycleStatus,
        now: TimestampMillis,
    ) -> Result<PromptMutationResult, PromptRepositoryError> {
        let mut connection = self
            .connection()
            .map_err(|_| PromptRepositoryError::Failure("database lock failure".into()))?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let current = load_required(&tx, id)?;
        if current.revision != expected_revision {
            return Err(PromptRepositoryError::Conflict);
        }
        if status == LifecycleStatus::Archived && current.status == LifecycleStatus::Archived {
            return Err(PromptRepositoryError::Failure(
                "prompt is already archived".into(),
            ));
        }
        if status == LifecycleStatus::Active && current.status == LifecycleStatus::Active {
            return Err(PromptRepositoryError::Failure(
                "prompt is already active".into(),
            ));
        }
        if status == LifecycleStatus::Archived {
            if let PromptProvenance::BuiltIn {
                protected: true, ..
            } = current.provenance
            {
                return Err(PromptRepositoryError::Protected);
            }
            if let PromptProvenance::BuiltIn { required: true, .. } = current.provenance {
                return Err(PromptRepositoryError::Required);
            }
        }
        let mut proposed = current;
        proposed.status = status;
        proposed.revision = next_revision(expected_revision)?;
        proposed.updated_at = now;
        update_root(&tx, &proposed, expected_revision)?;
        let stored = load_required(&tx, id)?;
        tx.commit().map_err(storage)?;
        Ok(PromptMutationResult { document: stored })
    }
}

impl PromptBootstrapPort for Database {
    fn reconcile_built_ins(
        &self,
        request: BuiltInReconcileRequest,
        now: TimestampMillis,
    ) -> Result<Vec<BuiltInReconcileOutcome>, PromptBootstrapError> {
        let mut connection = self
            .connection()
            .map_err(|_| PromptBootstrapError::Failure("database lock failure".into()))?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(bootstrap_storage)?;
        let outcomes = reconcile_built_ins_in(&tx, request, now)?;
        tx.commit().map_err(bootstrap_storage)?;
        Ok(outcomes)
    }
}

impl PromptDependencyReader for Database {
    fn references_to(
        &self,
        prompt_id: PromptDocumentId,
    ) -> Result<Vec<PromptReference>, PromptDependencyError> {
        let mut connection = self
            .connection()
            .map_err(|_| PromptDependencyError::Failure("database lock failure".into()))?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(|error| PromptDependencyError::Failure(storage(error).to_string()))?;
        if load_document(&tx, prompt_id)
            .map_err(|error| PromptDependencyError::Failure(storage(error).to_string()))?
            .is_none()
        {
            return Err(PromptDependencyError::NotFound);
        }
        let mut references = Vec::new();
        let mut character = tx.prepare("SELECT id,direct_prompt_id,group_roleplay_prompt_id FROM characters WHERE direct_prompt_id=?1 OR group_roleplay_prompt_id=?1 ORDER BY id").map_err(|error| PromptDependencyError::Failure(storage(error).to_string()))?;
        for row in character
            .query_map([prompt_id.to_string()], |row| {
                let id: CharacterId = parse_id(row.get(0)?)?;
                let direct: Option<String> = row.get(1)?;
                let roleplay: Option<String> = row.get(2)?;
                Ok((
                    id,
                    direct.as_deref() == Some(&prompt_id.to_string()),
                    roleplay.as_deref() == Some(&prompt_id.to_string()),
                ))
            })
            .map_err(|error| PromptDependencyError::Failure(storage(error).to_string()))?
        {
            let (id, direct, roleplay) =
                row.map_err(|error| PromptDependencyError::Failure(storage(error).to_string()))?;
            if direct {
                references.push(PromptReference {
                    owner: PromptReferenceOwner::Character(id),
                    purpose: PromptPurpose::DirectChat,
                    prompt_id,
                });
            }
            if roleplay {
                references.push(PromptReference {
                    owner: PromptReferenceOwner::Character(id),
                    purpose: PromptPurpose::GroupChatRoleplay,
                    prompt_id,
                });
            }
        }
        drop(character);
        let mut groups = tx.prepare("SELECT id,group_roleplay_prompt_id FROM groups WHERE group_roleplay_prompt_id=?1 ORDER BY id").map_err(|error| PromptDependencyError::Failure(storage(error).to_string()))?;
        for row in groups
            .query_map([prompt_id.to_string()], |row| {
                let id: GroupId = parse_id(row.get(0)?)?;
                let roleplay: Option<String> = row.get(1)?;
                Ok((id, roleplay.as_deref() == Some(&prompt_id.to_string())))
            })
            .map_err(|error| PromptDependencyError::Failure(storage(error).to_string()))?
        {
            let (id, roleplay) =
                row.map_err(|error| PromptDependencyError::Failure(storage(error).to_string()))?;
            if roleplay {
                references.push(PromptReference {
                    owner: PromptReferenceOwner::Group(id),
                    purpose: PromptPurpose::GroupChatRoleplay,
                    prompt_id,
                });
            }
        }
        drop(groups);
        let mut starters = tx.prepare("SELECT character_id,id FROM conversation_starters WHERE prompt_id=?1 ORDER BY character_id,id").map_err(|error| PromptDependencyError::Failure(storage(error).to_string()))?;
        for row in starters
            .query_map([prompt_id.to_string()], |row| {
                Ok(PromptReference {
                    owner: PromptReferenceOwner::Starter {
                        character_id: parse_id(row.get(0)?)?,
                        starter_id: parse_id(row.get(1)?)?,
                    },
                    purpose: PromptPurpose::DirectChat,
                    prompt_id,
                })
            })
            .map_err(|error| PromptDependencyError::Failure(storage(error).to_string()))?
        {
            references.push(
                row.map_err(|error| PromptDependencyError::Failure(storage(error).to_string()))?,
            );
        }
        drop(starters);
        references
            .sort_by_key(|reference| format!("{:?}:{:?}", reference.owner, reference.purpose));
        tx.commit()
            .map_err(|error| PromptDependencyError::Failure(storage(error).to_string()))?;
        Ok(references)
    }
}

/// Prompt ids ordered so a derived prompt follows its source.
pub(crate) fn sync_prompt_ids(connection: &Connection) -> rusqlite::Result<Vec<String>> {
    connection
        .prepare(
            "WITH RECURSIVE depth(id, level) AS (
               SELECT id, 0 FROM prompt_documents
                WHERE derived_source_id IS NULL
                   OR derived_source_id NOT IN (SELECT id FROM prompt_documents)
               UNION ALL
               SELECT document.id, depth.level + 1 FROM prompt_documents document
               JOIN depth ON document.derived_source_id = depth.id
             )
             SELECT id FROM depth ORDER BY level, id",
        )?
        .query_map([], |row| row.get(0))?
        .collect()
}

/// Writes a synced prompt document exactly (root in place, entries replaced
/// keeping local entry bookkeeping by id). Returns `false` when a different
/// local prompt already holds the same built-in key (pre-deterministic ids):
/// the incoming snapshot is then not materialized. A missing derivation
/// source reports `NotFound` (it arrives earlier in origin order).
pub(crate) fn sync_replace_prompt(
    tx: &Transaction<'_>,
    document: &PromptDocument,
) -> Result<bool, PromptRepositoryError> {
    document
        .validate()
        .map_err(|error| PromptRepositoryError::Failure(error.to_string()))?;
    let (_, key, _) = provenance_kind(&document.provenance);
    if let Some(key) = key {
        let other: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM prompt_documents WHERE built_in_key=?1 AND id<>?2)",
                params![key, document.id.to_string()],
                |row| row.get(0),
            )
            .map_err(storage)?;
        if other {
            return Ok(false);
        }
    }
    if load_document(tx, document.id).map_err(storage)?.is_none() {
        insert_root(tx, document)?;
        return Ok(true);
    }
    let provenance_json = encode(&document.provenance)?;
    let (kind, key, source) = provenance_kind(&document.provenance);
    tx.execute(
        "UPDATE prompt_documents SET status=?2,name=?3,purpose=?4,condense=?5,behavior_version=?6,provenance_kind=?7,built_in_key=?8,derived_source_id=?9,provenance_json=?10,revision=?11,created_at=?12,updated_at=?13,derived_source_name=?14 WHERE id=?1",
        params![
            document.id.to_string(), status_name(document.status), document.name,
            purpose_name(document.purpose), document.condense, behavior_name(document.behavior_version),
            kind, key, source, provenance_json, sql_revision(document.revision)?,
            document.created_at.get(), document.updated_at.get(), derived_source_name(&document.provenance)
        ],
    ).map_err(storage)?;
    replace_entries(tx, document, document.updated_at, &HashSet::new())?;
    Ok(true)
}

pub(crate) fn reconcile_built_ins_in(
    tx: &Transaction<'_>,
    request: BuiltInReconcileRequest,
    now: TimestampMillis,
) -> Result<Vec<BuiltInReconcileOutcome>, PromptBootstrapError> {
    request.validate()?;
    let mut seeds = request.seeds;
    seeds.sort_by_key(|seed| seed.key.trim().to_owned());
    let mut outcomes = Vec::with_capacity(seeds.len());
    for seed in &seeds {
        let key = seed.key.trim().to_owned();
        let seed_digest = seed.computed_seed_digest()?;
        let candidate_keys = std::iter::once(key.clone())
            .chain(seed.aliases.iter().map(|alias| alias.trim().to_owned()))
            .collect::<HashSet<_>>();
        let mut matching_ids = Vec::new();
        let mut statement = tx
            .prepare(
                "SELECT id,built_in_key FROM prompt_documents WHERE provenance_kind='built_in'",
            )
            .map_err(bootstrap_storage)?;
        for row in statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(bootstrap_storage)?
        {
            let (id, stored_key) = row.map_err(bootstrap_storage)?;
            if candidate_keys.contains(&stored_key) {
                matching_ids.push(id);
            }
        }
        if matching_ids.len() > 1 {
            return Err(PromptBootstrapError::AliasConflict);
        }
        let existing_id = matching_ids.pop();
        let Some(existing_id) = existing_id else {
            let id = built_in_document_id(&key);
            let metadata = seed.metadata.clone();
            let entries = seed
                .entries
                .iter()
                .cloned()
                .map(|draft| {
                    let entry_id = built_in_entry_id(
                        id,
                        draft.built_in_entry_key.as_deref().expect("validated seed"),
                    );
                    entry_from_draft(draft, entry_id)
                })
                .collect();
            let provenance = seed.provenance()?;
            let document = metadata_document(id, metadata, entries, provenance, now)
                .map_err(|error| PromptBootstrapError::Failure(error.to_string()))?;
            insert_root(tx, &document)
                .map_err(|error| PromptBootstrapError::Failure(error.to_string()))?;
            outcomes.push(BuiltInReconcileOutcome {
                key,
                action: BuiltInReconcileAction::Created,
                document: load_required(tx, id)
                    .map_err(|error| PromptBootstrapError::Failure(error.to_string()))?,
            });
            continue;
        };
        let id: PromptDocumentId = existing_id
            .parse()
            .map_err(|_| PromptBootstrapError::Failure("invalid built-in id".into()))?;
        let mut current = load_required(tx, id)
            .map_err(|error| PromptBootstrapError::Failure(error.to_string()))?;
        let (current_seed, current_authored) = match &current.provenance {
            PromptProvenance::BuiltIn {
                seed_digest,
                authored_digest,
                ..
            } => (seed_digest.clone(), authored_digest.clone()),
            _ => {
                return Err(PromptBootstrapError::InvalidStoredBuiltIn);
            }
        };
        let name_kept = current_authored == current_seed
            && match &current.provenance {
                PromptProvenance::BuiltIn {
                    key,
                    seed_version,
                    required,
                    protected,
                    ..
                } => {
                    authored_digest(&current, key, *seed_version, *required, *protected)
                        .map_err(|error| PromptBootstrapError::Failure(error.to_string()))?
                        != current_seed
                }
                _ => false,
            };
        let mut canonical_provenance = seed.provenance()?;
        let alias_only = match &current.provenance {
            PromptProvenance::BuiltIn {
                key: current_key, ..
            } => current_key != &key,
            _ => false,
        };
        if (alias_only || current_authored != current_seed)
            && let PromptProvenance::BuiltIn {
                authored_digest,
                seed_base,
                ..
            } = &mut canonical_provenance
        {
            *authored_digest = current_authored.clone();
            if let Some(stored) = current.provenance.seed_base() {
                *seed_base = Some(stored.clone());
            }
        }
        if current.provenance != canonical_provenance {
            let (kind, canonical_key, source) = provenance_kind(&canonical_provenance);
            let payload = encode(&canonical_provenance)
                .map_err(|error| PromptBootstrapError::Failure(error.to_string()))?;
            tx.execute(
                    "UPDATE prompt_documents SET provenance_kind=?2,built_in_key=?3,derived_source_id=?4,provenance_json=?5,derived_source_name=?6 WHERE id=?1",
                    params![
                        id.to_string(),
                        kind,
                        canonical_key,
                        source,
                        payload,
                        derived_source_name(&canonical_provenance)
                    ],
                )
                .map_err(bootstrap_storage)?;
            current.provenance = canonical_provenance.clone();
        }
        let legacy_unedited =
            legacy_seed_is_unedited(&current, &seed, &current_seed, &current_authored);
        let edited = current_authored != current_seed && !legacy_unedited;
        if edited && request.mode == BuiltInReconcileMode::RefreshUnedited {
            outcomes.push(BuiltInReconcileOutcome {
                key,
                action: BuiltInReconcileAction::PreservedEdited,
                document: current,
            });
            continue;
        }
        if !edited && current_seed == seed_digest && current.status == LifecycleStatus::Active {
            outcomes.push(BuiltInReconcileOutcome {
                key,
                action: BuiltInReconcileAction::RefreshedUnedited,
                document: current,
            });
            continue;
        }
        let action = if edited {
            BuiltInReconcileAction::ResetEdited
        } else {
            BuiltInReconcileAction::RefreshedUnedited
        };
        let legacy_ordinal_match = legacy_unedited;
        let (entries, touched, entries_changed) =
            reconcile_entries(&current, &seed, legacy_ordinal_match);
        let mut metadata = seed.metadata.clone();
        let keep_name = request.mode == BuiltInReconcileMode::ResetToSeed || name_kept;
        if keep_name {
            metadata.name = current.name.clone();
        }
        let metadata_changed = current.name != metadata.name
            || current.purpose != metadata.purpose
            || current.condense != metadata.condense
            || current.behavior_version != metadata.behavior_version;
        let root_changed =
            metadata_changed || entries_changed || current.status != LifecycleStatus::Active;
        let mut document = metadata_document(id, metadata, entries, seed.provenance()?, now)
            .map_err(|error| PromptBootstrapError::Failure(error.to_string()))?;
        let kept_name = std::mem::replace(&mut document.name, seed.metadata.name.clone());
        refresh_provenance(&mut document)
            .map_err(|error| PromptBootstrapError::Failure(error.to_string()))?;
        document.name = kept_name;
        document.revision = if root_changed {
            next_revision(current.revision)
                .map_err(|error| PromptBootstrapError::Failure(error.to_string()))?
        } else {
            current.revision
        };
        document.created_at = current.created_at;
        document.status = LifecycleStatus::Active;
        document.updated_at = if root_changed {
            now
        } else {
            current.updated_at
        };
        update_root(tx, &document, current.revision)
            .map_err(|error| PromptBootstrapError::Failure(error.to_string()))?;
        replace_entries(tx, &document, now, &touched)
            .map_err(|error| PromptBootstrapError::Failure(error.to_string()))?;
        outcomes.push(BuiltInReconcileOutcome {
            key,
            action,
            document: load_required(tx, id)
                .map_err(|error| PromptBootstrapError::Failure(error.to_string()))?,
        });
    }
    backfill_derived_seed_bases(tx, &seeds)?;
    Ok(outcomes)
}

/// Gives a copy of a built-in that predates seed bases its base once: every
/// seed key is recorded, held by the copy entry equal to its current seed
/// entry when there is one. The write touches the provenance column only, so
/// the revision and `updated_at` stay.
fn backfill_derived_seed_bases(
    tx: &Transaction<'_>,
    seeds: &[BuiltInPromptSeed],
) -> Result<(), PromptBootstrapError> {
    let failure = |error: PromptRepositoryError| PromptBootstrapError::Failure(error.to_string());
    let mut statement = tx
        .prepare(
            "SELECT derived.id,source.built_in_key FROM prompt_documents derived JOIN prompt_documents source ON source.id=derived.derived_source_id WHERE derived.provenance_kind='derived' AND source.provenance_kind='built_in' AND instr(derived.provenance_json,'\"seed_base\"')=0",
        )
        .map_err(bootstrap_storage)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(bootstrap_storage)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(bootstrap_storage)?;
    for (id, source_key) in rows {
        let Some(seed) = seeds.iter().find(|seed| seed.is_named(&source_key)) else {
            continue;
        };
        let id: PromptDocumentId = id
            .parse()
            .map_err(|_| PromptBootstrapError::Failure("invalid derived id".into()))?;
        let mut document = load_required(tx, id).map_err(failure)?;
        let mut claimed = HashSet::new();
        let mut base = seed.seed_base();
        for entry in &mut base.entries {
            let held = document.entries.iter().find(|candidate| {
                !claimed.contains(&candidate.id)
                    && candidate.authored_digest() == entry.seed_entry_digest
            });
            entry.entry_id = held.map(|held| held.id);
            if let Some(held) = held {
                claimed.insert(held.id);
            }
        }
        document.provenance.set_seed_base(Some(base));
        tx.execute(
            "UPDATE prompt_documents SET provenance_json=?2 WHERE id=?1",
            params![
                id.to_string(),
                encode(&document.provenance).map_err(failure)?
            ],
        )
        .map_err(bootstrap_storage)?;
    }
    Ok(())
}

/// Applies the user's per-entry decisions on a built-in update. Every pending
/// entry key needs a choice; a choice for a key that is no longer pending
/// reports `Conflict`. Both decisions advance the seed base to the seed, so
/// the update stops being pending.
pub(crate) fn apply_built_in_update_in(
    tx: &Transaction<'_>,
    id: PromptDocumentId,
    expected_revision: Revision,
    seed: &BuiltInPromptSeed,
    choices: &[(String, bool)],
    now: TimestampMillis,
) -> Result<PromptDocument, PromptRepositoryError> {
    let current = load_required(tx, id)?;
    check_expected(&current, expected_revision)?;
    let changes = built_in_update(&current, seed);
    if changes.is_empty() {
        return Err(PromptRepositoryError::Conflict);
    }
    let take: HashMap<&str, bool> = choices
        .iter()
        .map(|(key, take)| (key.as_str(), *take))
        .collect();
    if take
        .keys()
        .any(|key| !changes.iter().any(|change| change.entry_key() == *key))
    {
        return Err(PromptRepositoryError::Conflict);
    }
    if let Some(change) = changes
        .iter()
        .find(|change| !take.contains_key(change.entry_key()))
    {
        return Err(PromptRepositoryError::Invalid(
            PromptValidationError::MissingBuiltInUpdateChoice(change.entry_key().to_owned()),
        ));
    }
    let derived = matches!(current.provenance, PromptProvenance::Derived { .. });
    let seed_key = |entry: &PromptEntryDraft| {
        entry
            .built_in_entry_key
            .as_deref()
            .expect("validated seed")
            .to_owned()
    };
    let mut held: HashMap<String, PromptEntryId> = seed
        .entries
        .iter()
        .filter_map(|entry| {
            let key = seed_key(entry);
            let id = current.entry_for_seed_key(&key)?.id;
            Some((key, id))
        })
        .collect();
    let mut entries = current.entries.clone();
    let mut touched = HashSet::new();
    for change in &changes {
        if !take[change.entry_key()] {
            continue;
        }
        match change {
            PromptBuiltInChange::Updated {
                current: old, new, ..
            }
            | PromptBuiltInChange::Conflict {
                current: old, new, ..
            } => {
                let position = entries
                    .iter()
                    .position(|entry| entry.id == old.id)
                    .ok_or(PromptRepositoryError::EntryNotFound)?;
                let mut entry = entry_from_draft(new.clone(), old.id);
                entry.built_in_entry_key = old.built_in_entry_key.clone();
                entries[position] = entry;
                touched.insert(old.id);
            }
            PromptBuiltInChange::Removed { current: old, .. } => {
                entries.retain(|entry| entry.id != old.id);
            }
            PromptBuiltInChange::Added { .. } => {}
        }
    }
    for (index, new) in seed.entries.iter().enumerate() {
        let key = seed_key(new);
        if !matches!(
            changes.iter().find(|change| change.entry_key() == key),
            Some(PromptBuiltInChange::Added { .. })
        ) || !take[key.as_str()]
        {
            continue;
        }
        let position = seed.entries[..index]
            .iter()
            .rev()
            .find_map(|earlier| held.get(&seed_key(earlier)))
            .and_then(|earlier| entries.iter().position(|entry| entry.id == *earlier))
            .map_or(0, |position| position + 1);
        let entry_id = if derived {
            PromptEntryId::new()
        } else {
            built_in_entry_id(current.id, &key)
        };
        let mut entry = entry_from_draft(new.clone(), entry_id);
        if derived {
            entry.built_in_entry_key = None;
        }
        entries.insert(position, entry);
        held.insert(key, entry_id);
    }
    let mut proposed = current.clone();
    proposed.entries = entries;
    proposed.revision = next_revision(expected_revision)?;
    proposed.updated_at = now;
    proposed.provenance.set_seed_base(Some(PromptSeedBase {
        built_in_key: seed.key.trim().to_owned(),
        entries: seed
            .entries
            .iter()
            .map(|entry| {
                let key = seed_key(entry);
                PromptSeedBaseEntry {
                    entry_id: held.get(&key).copied().filter(|_| derived),
                    entry_key: key,
                    seed_entry_digest: entry.authored_digest(),
                }
            })
            .collect(),
    }));
    refresh_provenance(&mut proposed)?;
    proposed
        .validate()
        .map_err(PromptRepositoryError::Invalid)?;
    update_root(tx, &proposed, expected_revision)?;
    replace_entries(tx, &proposed, now, &touched)?;
    load_required(tx, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_context::{
        PromptEntryChatMode, PromptEntryCondition, PromptEntryImageSlot, PromptEntryInfoSource,
        PromptEntryPayload,
    };

    fn metadata(name: &str) -> PromptMetadataDraft {
        PromptMetadataDraft {
            name: name.into(),
            purpose: PromptPurpose::DirectChat,
            condense: false,
            behavior_version: PromptBehaviorVersion::LegacyV1,
        }
    }

    fn draft(name: &str) -> PromptEntryDraft {
        PromptEntryDraft {
            built_in_entry_key: None,
            name: name.into(),
            role: PromptEntryRole::System,
            content: format!("content {name}"),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: Some(PromptEntryCondition::All {
                conditions: vec![
                    PromptEntryCondition::ChatMode {
                        value: PromptEntryChatMode::Direct,
                    },
                    PromptEntryCondition::InfoSource {
                        value: PromptEntryInfoSource::Messages,
                    },
                ],
            }),
            payload: Some(PromptEntryPayload::ImageSlot {
                slot: PromptEntryImageSlot::Character,
            }),
        }
    }

    #[test]
    fn user_prompt_round_trips_and_mutations_are_root_cas() {
        let database = Database::open_in_memory().expect("database");
        let document = database
            .create_user_draft(
                metadata("Prompt"),
                vec![draft("one")],
                TimestampMillis::new(1),
            )
            .expect("create");
        assert_eq!(
            database.get(document.id).expect("get"),
            Some(document.clone())
        );
        let added = database
            .mutate_entries(
                document.id,
                document.revision,
                PromptEntryMutation::Add {
                    draft: draft("two"),
                    target: PromptEntryInsertionTarget::Append,
                },
                TimestampMillis::new(2),
            )
            .expect("add")
            .document;
        assert_eq!(added.entries.len(), 2);
        assert!(matches!(
            database.mutate_entries(
                document.id,
                document.revision,
                PromptEntryMutation::Remove {
                    entry_id: added.entries[0].id
                },
                TimestampMillis::new(3)
            ),
            Err(PromptRepositoryError::Conflict)
        ));
        let revised = database
            .revise_metadata(
                added.id,
                added.revision,
                PromptMetadataDraft {
                    name: "Renamed".into(),
                    purpose: PromptPurpose::SceneGeneration,
                    condense: true,
                    behavior_version: PromptBehaviorVersion::DeterministicV2,
                },
                TimestampMillis::new(3),
            )
            .expect("revise")
            .document;
        assert_eq!(revised.purpose, PromptPurpose::SceneGeneration);
        let archived = database
            .archive(revised.id, revised.revision, TimestampMillis::new(4))
            .expect("archive")
            .document;
        assert!(matches!(
            database
                .lookup_exact(archived.id, archived.purpose)
                .expect("lookup"),
            PromptLookupResult::Archived { .. }
        ));
        let restored = database
            .restore(archived.id, archived.revision, TimestampMillis::new(5))
            .expect("restore")
            .document;
        assert!(matches!(
            database
                .lookup_exact(restored.id, restored.purpose)
                .expect("lookup"),
            PromptLookupResult::Available { .. }
        ));
    }

    #[test]
    fn whole_document_revision_keeps_entry_identity_and_built_in_keys() {
        let database = Database::open_in_memory().expect("database");
        let mut seed_entry = draft("one");
        seed_entry.built_in_entry_key = Some("one".into());
        let mut second_entry = draft("two");
        second_entry.built_in_entry_key = Some("two".into());
        let created = database
            .reconcile_built_ins(
                BuiltInReconcileRequest {
                    seeds: vec![BuiltInPromptSeed {
                        key: "core".into(),
                        aliases: Vec::new(),
                        seed_version: 1,
                        metadata: metadata("Core"),
                        entries: vec![seed_entry, second_entry],
                        required: true,
                        protected: true,
                    }],
                    mode: BuiltInReconcileMode::RefreshUnedited,
                },
                TimestampMillis::new(1),
            )
            .expect("seed");
        let document = &created[0].document;
        let edit = |entry: &PromptEntry, content: &str| PromptEntryEdit {
            entry_id: Some(entry.id),
            draft: PromptEntryDraft {
                content: content.into(),
                built_in_entry_key: None,
                ..PromptEntryDraft::from(entry.clone())
            },
        };
        let mut changed_purpose = metadata("Renamed");
        changed_purpose.purpose = PromptPurpose::CompanionChat;
        assert_eq!(
            database.revise_prompt_document(
                document.id,
                document.revision,
                changed_purpose,
                Vec::new(),
                TimestampMillis::new(2),
            ),
            Err(PromptRepositoryError::Protected)
        );
        let revised = database
            .revise_prompt_document(
                document.id,
                document.revision,
                metadata("Renamed"),
                vec![
                    PromptEntryEdit {
                        entry_id: None,
                        draft: draft("three"),
                    },
                    edit(&document.entries[0], "changed one"),
                ],
                TimestampMillis::new(2),
            )
            .expect("revise");
        assert_eq!(revised.name, "Renamed");
        assert_eq!(revised.revision.get(), document.revision.get() + 1);
        assert_eq!(revised.entries.len(), 2);
        assert_eq!(revised.entries[1].id, document.entries[0].id);
        assert_eq!(
            revised.entries[1].built_in_entry_key.as_deref(),
            Some("one")
        );
        assert_eq!(revised.entries[1].content, "changed one");
        assert_eq!(
            database.revise_prompt_document(
                document.id,
                revised.revision,
                metadata("Renamed"),
                vec![edit(&document.entries[1], "gone")],
                TimestampMillis::new(3),
            ),
            Err(PromptRepositoryError::EntryNotFound)
        );
        assert_eq!(
            database.revise_prompt_document(
                document.id,
                document.revision,
                metadata("Stale"),
                Vec::new(),
                TimestampMillis::new(3),
            ),
            Err(PromptRepositoryError::Conflict)
        );
    }

    #[test]
    fn built_in_reconcile_preserves_and_resets_edits() {
        let database = Database::open_in_memory().expect("database");
        let mut seed_entry = draft("one");
        seed_entry.built_in_entry_key = Some("one".into());
        let seed = BuiltInPromptSeed {
            key: "core".into(),
            aliases: Vec::new(),
            seed_version: 1,
            metadata: metadata("Core"),
            entries: vec![seed_entry],
            required: true,
            protected: false,
        };
        let request = BuiltInReconcileRequest {
            seeds: vec![seed.clone()],
            mode: BuiltInReconcileMode::RefreshUnedited,
        };
        let created = database
            .reconcile_built_ins(request.clone(), TimestampMillis::new(1))
            .expect("create");
        assert_eq!(created[0].action, BuiltInReconcileAction::Created);
        let edited = database
            .revise_metadata(
                created[0].document.id,
                created[0].document.revision,
                metadata("Edited"),
                TimestampMillis::new(2),
            )
            .expect("edit")
            .document;
        let preserved = database
            .reconcile_built_ins(request, TimestampMillis::new(3))
            .expect("preserve");
        assert_eq!(preserved[0].action, BuiltInReconcileAction::PreservedEdited);
        assert_eq!(preserved[0].document, edited);
        let reset = database
            .reconcile_built_ins(
                BuiltInReconcileRequest {
                    seeds: vec![seed],
                    mode: BuiltInReconcileMode::ResetToSeed,
                },
                TimestampMillis::new(4),
            )
            .expect("reset");
        assert_eq!(reset[0].action, BuiltInReconcileAction::ResetEdited);
        assert_eq!(reset[0].document.name, "Edited");
        assert_eq!(reset[0].document.entries[0].content, "content one");
        assert_eq!(
            reset[0].document.entries[0].id,
            created[0].document.entries[0].id
        );
    }

    #[test]
    fn reset_of_a_renamed_built_in_counts_as_unedited_and_keeps_its_name() {
        let database = Database::open_in_memory().expect("database");
        let seed_with = |version: u32, content: &str| {
            let mut entry = draft("one");
            entry.built_in_entry_key = Some("one".into());
            entry.content = content.into();
            BuiltInPromptSeed {
                key: "core".into(),
                aliases: Vec::new(),
                seed_version: version,
                metadata: metadata("Core"),
                entries: vec![entry],
                required: true,
                protected: false,
            }
        };
        let request =
            |seed: BuiltInPromptSeed, mode: BuiltInReconcileMode| BuiltInReconcileRequest {
                seeds: vec![seed],
                mode,
            };
        let refresh = |seed| request(seed, BuiltInReconcileMode::RefreshUnedited);
        let created = database
            .reconcile_built_ins(refresh(seed_with(1, "first")), TimestampMillis::new(1))
            .expect("create");
        database
            .revise_metadata(
                created[0].document.id,
                created[0].document.revision,
                metadata("Mine"),
                TimestampMillis::new(2),
            )
            .expect("rename");
        let reset = database
            .reconcile_built_ins(
                request(seed_with(1, "first"), BuiltInReconcileMode::ResetToSeed),
                TimestampMillis::new(3),
            )
            .expect("reset");
        assert_eq!(reset[0].action, BuiltInReconcileAction::ResetEdited);
        assert_eq!(reset[0].document.name, "Mine");
        let unchanged = database
            .reconcile_built_ins(refresh(seed_with(1, "first")), TimestampMillis::new(4))
            .expect("startup");
        assert_eq!(unchanged[0].document.name, "Mine");
        assert_ne!(unchanged[0].action, BuiltInReconcileAction::PreservedEdited);
        let refreshed = database
            .reconcile_built_ins(refresh(seed_with(2, "second")), TimestampMillis::new(5))
            .expect("later seed");
        assert_eq!(
            refreshed[0].action,
            BuiltInReconcileAction::RefreshedUnedited
        );
        assert_eq!(refreshed[0].document.name, "Mine");
        assert_eq!(refreshed[0].document.entries[0].content, "second");
        let again = database
            .reconcile_built_ins(refresh(seed_with(3, "third")), TimestampMillis::new(6))
            .expect("next seed");
        assert_eq!(again[0].action, BuiltInReconcileAction::RefreshedUnedited);
        assert_eq!(again[0].document.name, "Mine");
        assert_eq!(again[0].document.entries[0].content, "third");
    }

    #[test]
    fn built_in_reconcile_matches_entry_keys_across_catalog_changes() {
        let database = Database::open_in_memory().expect("database");
        let mut first = draft("first");
        first.built_in_entry_key = Some("first".into());
        let mut second = draft("second");
        second.built_in_entry_key = Some("second".into());
        let seed = BuiltInPromptSeed {
            key: "catalog".into(),
            aliases: Vec::new(),
            seed_version: 1,
            metadata: metadata("Catalog"),
            entries: vec![first.clone(), second.clone()],
            required: true,
            protected: false,
        };
        let created = database
            .reconcile_built_ins(
                BuiltInReconcileRequest {
                    seeds: vec![seed.clone()],
                    mode: BuiltInReconcileMode::RefreshUnedited,
                },
                TimestampMillis::new(10),
            )
            .expect("initial reconcile")[0]
            .document
            .clone();
        let first_id = created.entries[0].id;
        let second_id = created.entries[1].id;

        let mut third = draft("third");
        third.built_in_entry_key = Some("third".into());
        let mut changed_second = second.clone();
        changed_second.content = "changed second".into();
        let changed_seed = BuiltInPromptSeed {
            seed_version: 2,
            entries: vec![changed_second, first.clone(), third],
            ..seed
        };
        let refreshed = database
            .reconcile_built_ins(
                BuiltInReconcileRequest {
                    seeds: vec![changed_seed.clone()],
                    mode: BuiltInReconcileMode::RefreshUnedited,
                },
                TimestampMillis::new(20),
            )
            .expect("changed reconcile")[0]
            .document
            .clone();
        assert_eq!(refreshed.entries[0].id, second_id);
        assert_eq!(refreshed.entries[1].id, first_id);
        assert_eq!(
            refreshed.entries[2].built_in_entry_key.as_deref(),
            Some("third")
        );
        let connection = database.connection().expect("database lock");
        let entry_times = |key: &str| {
            connection
                .query_row(
                    "SELECT revision,created_at,updated_at FROM prompt_entries WHERE prompt_id=?1 AND built_in_entry_key=?2",
                    rusqlite::params![refreshed.id.to_string(), key],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?)),
                )
                .expect("entry metadata")
        };
        assert_eq!(entry_times("second"), (2, 10, 20));
        assert_eq!(entry_times("first"), (2, 10, 20));
        assert_eq!(entry_times("third"), (1, 20, 20));
        drop(connection);

        let mut edited = draft("edited");
        edited.built_in_entry_key = None;
        let edited = database
            .mutate_entries(
                refreshed.id,
                refreshed.revision,
                PromptEntryMutation::Update {
                    entry_id: refreshed.entries[1].id,
                    draft: edited,
                },
                TimestampMillis::new(30),
            )
            .expect("edit entry")
            .document;
        assert_eq!(
            edited.entries[1].built_in_entry_key.as_deref(),
            Some("first")
        );
        let reset = database
            .reconcile_built_ins(
                BuiltInReconcileRequest {
                    seeds: vec![changed_seed],
                    mode: BuiltInReconcileMode::ResetToSeed,
                },
                TimestampMillis::new(40),
            )
            .expect("reset edited entry")[0]
            .document
            .clone();
        assert_eq!(reset.entries[0].id, second_id);
        assert_eq!(reset.entries[1].id, first_id);
        assert_eq!(reset.entries[1].content, "content first");
        let connection = database.connection().expect("database lock");
        assert_eq!(
            connection
                .query_row(
                    "SELECT revision,created_at,updated_at FROM prompt_entries WHERE prompt_id=?1 AND built_in_entry_key='first'",
                    [reset.id.to_string()],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?)),
                )
                .expect("reset entry metadata"),
            (4, 10, 40)
        );
    }

    #[test]
    fn legacy_unkeyed_entries_are_ordinal_matched_and_key_assignment_is_touched() {
        let database = Database::open_in_memory().expect("database");
        let mut first = draft("first");
        first.built_in_entry_key = Some("first".into());
        let mut second = draft("second");
        second.built_in_entry_key = Some("second".into());
        let seed = BuiltInPromptSeed {
            key: "legacy-catalog".into(),
            aliases: Vec::new(),
            seed_version: 1,
            metadata: metadata("Legacy catalog"),
            entries: vec![first, second],
            required: true,
            protected: false,
        };
        let created = database
            .reconcile_built_ins(
                BuiltInReconcileRequest {
                    seeds: vec![seed.clone()],
                    mode: BuiltInReconcileMode::RefreshUnedited,
                },
                TimestampMillis::new(10),
            )
            .expect("create")[0]
            .document
            .clone();
        let ids = created
            .entries
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>();
        let connection = database.connection().expect("database lock");
        connection
            .pragma_update(None, "ignore_check_constraints", true)
            .expect("ignore checks");
        connection
            .execute(
                "UPDATE prompt_entries SET built_in_entry_key=NULL WHERE prompt_id=?1",
                [created.id.to_string()],
            )
            .expect("erase legacy keys");
        connection
            .pragma_update(None, "ignore_check_constraints", false)
            .expect("restore checks");
        drop(connection);

        let upgraded_seed = BuiltInPromptSeed {
            seed_version: 2,
            ..seed
        };
        let upgraded = database
            .reconcile_built_ins(
                BuiltInReconcileRequest {
                    seeds: vec![upgraded_seed],
                    mode: BuiltInReconcileMode::RefreshUnedited,
                },
                TimestampMillis::new(20),
            )
            .expect("upgrade")[0]
            .document
            .clone();
        assert_eq!(upgraded.entries[0].id, ids[0]);
        assert_eq!(upgraded.entries[1].id, ids[1]);
        let connection = database.connection().expect("database lock");
        let metadata = connection
            .prepare(
                "SELECT revision,created_at,updated_at FROM prompt_entries WHERE prompt_id=?1 ORDER BY ordinal",
            )
            .expect("entry metadata")
            .query_map([created.id.to_string()], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?))
            })
            .expect("entry rows")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("metadata rows");
        assert_eq!(metadata, vec![(2, 10, 20), (2, 10, 20)]);
    }

    #[test]
    fn exact_lookup_and_keyset_page_are_strict() {
        let database = Database::open_in_memory().expect("database");
        let first = database
            .create_user_draft(metadata("first"), Vec::new(), TimestampMillis::new(1))
            .expect("first");
        let second = database
            .create_user_draft(metadata("second"), Vec::new(), TimestampMillis::new(1))
            .expect("second");
        let page = database
            .page(PromptLibraryQuery {
                page: lettuce_types::PageRequest {
                    cursor: None,
                    limit: lettuce_types::PageLimit::new(1),
                },
                status: LifecycleFilter::Active,
                purpose: None,
            })
            .expect("page");
        assert_eq!(page.items.len(), 1);
        assert!(page.next_cursor.is_some());
        let next = database
            .page(PromptLibraryQuery {
                page: lettuce_types::PageRequest {
                    cursor: page.next_cursor,
                    limit: lettuce_types::PageLimit::new(1),
                },
                status: LifecycleFilter::Active,
                purpose: None,
            })
            .expect("next");
        assert_eq!(next.items.len(), 1);
        assert_ne!(first.id, second.id);
        assert!(matches!(
            database
                .lookup_exact(first.id, PromptPurpose::SceneGeneration)
                .expect("lookup"),
            PromptLookupResult::PurposeMismatch { .. }
        ));
        assert!(matches!(
            database.page(PromptLibraryQuery {
                page: lettuce_types::PageRequest::default(),
                status: LifecycleFilter::Active,
                purpose: Some(PromptPurpose::Undefined),
            }),
            Err(PromptRepositoryError::Invalid(
                PromptValidationError::UndefinedPurpose
            ))
        ));
    }

    #[test]
    fn every_purpose_and_behavior_scalar_round_trips() {
        let database = Database::open_in_memory().expect("database");
        let purposes = [
            PromptPurpose::DirectChat,
            PromptPurpose::CompanionChat,
            PromptPurpose::GroupChatRoleplay,
            PromptPurpose::DynamicMemorySummarizer,
            PromptPurpose::DynamicMemoryManager,
            PromptPurpose::ReplyHelperRoleplay,
            PromptPurpose::ReplyHelperConversational,
            PromptPurpose::LorebookEntryWriter,
            PromptPurpose::LorebookKeywordGenerator,
            PromptPurpose::LorebookGeneratorPlanner,
            PromptPurpose::LorebookGeneratorWriter,
            PromptPurpose::LorebookGeneratorRefine,
            PromptPurpose::LorebookGeneratorCoherence,
            PromptPurpose::AvatarGeneration,
            PromptPurpose::AvatarEditRequest,
            PromptPurpose::SceneGeneration,
            PromptPurpose::ScenePromptWriter,
            PromptPurpose::DesignReferenceWriter,
            PromptPurpose::CompanionSoulWriter,
            PromptPurpose::CompanionGrowthcycle,
            PromptPurpose::CompanionConsolidation,
            PromptPurpose::RuntimeText,
        ];
        for (index, purpose) in purposes.into_iter().enumerate() {
            let document = database
                .create_user_draft(
                    PromptMetadataDraft {
                        name: format!("purpose-{index}"),
                        purpose,
                        condense: index % 2 == 0,
                        behavior_version: if index % 2 == 0 {
                            PromptBehaviorVersion::LegacyV1
                        } else {
                            PromptBehaviorVersion::DeterministicV2
                        },
                    },
                    Vec::new(),
                    TimestampMillis::new(index as i64),
                )
                .expect("create purpose");
            assert_eq!(database.get(document.id).expect("reload"), Some(document));
        }
    }

    #[test]
    fn condition_and_payload_envelopes_are_strict_and_exhaustive() {
        let database = Database::open_in_memory().expect("database");
        let conditions = vec![
            PromptEntryCondition::ChatMode {
                value: PromptEntryChatMode::Direct,
            },
            PromptEntryCondition::InfoSource {
                value: PromptEntryInfoSource::Memory,
            },
            PromptEntryCondition::SceneGenerationEnabled { value: true },
            PromptEntryCondition::AvatarGenerationEnabled { value: true },
            PromptEntryCondition::IsLocalImageGenerationModel { value: true },
            PromptEntryCondition::IsSceneGenerationLocalImageModel { value: true },
            PromptEntryCondition::SceneImageProtocol {
                value: lettuce_context::SceneImageProtocolKind::Local,
            },
            PromptEntryCondition::HasScene { value: true },
            PromptEntryCondition::HasSceneDirection { value: true },
            PromptEntryCondition::HasPersona { value: true },
            PromptEntryCondition::MessageCountAtLeast { value: 2 },
            PromptEntryCondition::ParticipantCountAtLeast { value: 2 },
            PromptEntryCondition::KeywordAny {
                values: vec!["rain".into()],
            },
            PromptEntryCondition::KeywordAll {
                values: vec!["rain".into()],
            },
            PromptEntryCondition::KeywordNone {
                values: vec!["sun".into()],
            },
            PromptEntryCondition::DynamicMemoryEnabled { value: true },
            PromptEntryCondition::HasMemorySummary { value: true },
            PromptEntryCondition::HasKeyMemories { value: true },
            PromptEntryCondition::HasLorebookContent { value: true },
            PromptEntryCondition::DoesAuthorNoteExists { value: true },
            PromptEntryCondition::HasActiveScheduledNote { value: true },
            PromptEntryCondition::HasSubjectDescription { value: true },
            PromptEntryCondition::HasCurrentDescription { value: true },
            PromptEntryCondition::HasCharacterReferenceImages { value: true },
            PromptEntryCondition::HasChatBackground { value: true },
            PromptEntryCondition::HasPersonaReferenceImages { value: true },
            PromptEntryCondition::HasCharacterReferenceText { value: true },
            PromptEntryCondition::HasPersonaReferenceText { value: true },
            PromptEntryCondition::InputScopeAny {
                values: vec!["chat".into()],
            },
            PromptEntryCondition::OutputScopeAny {
                values: vec!["text".into()],
            },
            PromptEntryCondition::ProviderIdAny {
                values: vec!["provider".into()],
            },
            PromptEntryCondition::ReasoningEnabled { value: true },
            PromptEntryCondition::VisionEnabled { value: true },
            PromptEntryCondition::IsTimeAwarenessEnabled { value: true },
            PromptEntryCondition::IsCompanionMode { value: true },
            PromptEntryCondition::Any {
                conditions: Vec::new(),
            },
            PromptEntryCondition::Not {
                condition: Box::new(PromptEntryCondition::HasScene { value: false }),
            },
        ];
        let mut entry = draft("exhaustive");
        entry.conditions = Some(PromptEntryCondition::All { conditions });
        let document = database
            .create_user_draft(metadata("exhaustive"), vec![entry], TimestampMillis::new(1))
            .expect("create");
        assert_eq!(database.get(document.id).expect("reload"), Some(document));
        for slot in [
            lettuce_context::PromptEntryImageSlot::Character,
            lettuce_context::PromptEntryImageSlot::Persona,
            lettuce_context::PromptEntryImageSlot::ChatBackground,
            lettuce_context::PromptEntryImageSlot::Avatar,
            lettuce_context::PromptEntryImageSlot::References,
            lettuce_context::PromptEntryImageSlot::Cast,
        ] {
            let mut entry = draft("slot");
            entry.conditions = None;
            entry.payload = Some(PromptEntryPayload::ImageSlot { slot });
            let value = database
                .create_user_draft(metadata("payload"), vec![entry], TimestampMillis::new(2))
                .expect("create payload");
            assert_eq!(database.get(value.id).expect("reload"), Some(value));
        }
    }

    #[test]
    fn protected_and_required_built_ins_follow_lifecycle_policy() {
        let database = Database::open_in_memory().expect("database");
        let mut seed_entry = draft("entry");
        seed_entry.built_in_entry_key = Some("entry".into());
        let seed = BuiltInPromptSeed {
            key: "protected".into(),
            aliases: Vec::new(),
            seed_version: 1,
            metadata: metadata("Protected"),
            entries: vec![seed_entry],
            required: true,
            protected: true,
        };
        let document = database
            .reconcile_built_ins(
                BuiltInReconcileRequest {
                    seeds: vec![seed],
                    mode: BuiltInReconcileMode::RefreshUnedited,
                },
                TimestampMillis::new(1),
            )
            .expect("reconcile")[0]
            .document
            .clone();
        assert!(matches!(
            database.archive(document.id, document.revision, TimestampMillis::new(2)),
            Err(PromptRepositoryError::Protected)
        ));
        assert!(matches!(
            database.revise_metadata(
                document.id,
                document.revision,
                PromptMetadataDraft {
                    name: "same identity".into(),
                    purpose: PromptPurpose::SceneGeneration,
                    condense: false,
                    behavior_version: PromptBehaviorVersion::LegacyV1,
                },
                TimestampMillis::new(2)
            ),
            Err(PromptRepositoryError::Protected)
        ));
        let changed = database
            .revise_metadata(
                document.id,
                document.revision,
                PromptMetadataDraft {
                    name: "editable name".into(),
                    purpose: document.purpose,
                    condense: true,
                    behavior_version: document.behavior_version,
                },
                TimestampMillis::new(2),
            )
            .expect("protected content metadata remains editable")
            .document;
        assert_eq!(changed.name, "editable name");
        assert!(matches!(
            database.archive(changed.id, changed.revision, TimestampMillis::new(3)),
            Err(PromptRepositoryError::Protected)
        ));
    }

    #[test]
    fn malformed_json_is_storage_failure_not_a_silent_default() {
        let database = Database::open_in_memory().expect("database");
        let document = database
            .create_user_draft(
                metadata("corrupt"),
                vec![draft("entry")],
                TimestampMillis::new(1),
            )
            .expect("create");
        database
            .connection()
            .expect("lock")
            .execute(
                r#"UPDATE prompt_entries SET conditions_json='{"format_version":99,"value":{}}' WHERE prompt_id=?1"#,
                [document.id.to_string()],
            )
            .expect("corrupt row");
        assert!(matches!(
            database.get(document.id),
            Err(PromptRepositoryError::Failure(_))
        ));
    }

    #[test]
    fn corrupt_builtin_entry_key_is_rejected_on_read() {
        let database = Database::open_in_memory().expect("database");
        let mut entry = draft("entry");
        entry.built_in_entry_key = Some("stable.entry".into());
        let document = database
            .reconcile_built_ins(
                BuiltInReconcileRequest {
                    seeds: vec![BuiltInPromptSeed {
                        key: "corruptible".into(),
                        aliases: Vec::new(),
                        seed_version: 1,
                        metadata: metadata("Corruptible"),
                        entries: vec![entry],
                        required: false,
                        protected: false,
                    }],
                    mode: BuiltInReconcileMode::RefreshUnedited,
                },
                TimestampMillis::new(1),
            )
            .expect("seed")[0]
            .document
            .clone();
        let connection = database.connection().expect("database lock");
        connection
            .pragma_update(None, "ignore_check_constraints", true)
            .expect("ignore checks");
        connection
            .execute(
                "UPDATE prompt_entries SET built_in_entry_key=' ' WHERE prompt_id=?1",
                [document.id.to_string()],
            )
            .expect("corrupt key");
        connection
            .pragma_update(None, "ignore_check_constraints", false)
            .expect("restore checks");
        drop(connection);
        assert!(matches!(
            database.get(document.id),
            Err(PromptRepositoryError::Failure(_))
        ));
    }

    #[test]
    fn imported_and_derived_provenance_round_trip_with_storage_columns() {
        let database = Database::open_in_memory().expect("database");
        let source = database
            .create_user_draft(metadata("source"), Vec::new(), TimestampMillis::new(1))
            .expect("source");
        let derived_id = PromptDocumentId::new();
        let imported_id = PromptDocumentId::new();
        let derived = PromptProvenance::Derived {
            source: source.id,
            source_name: "source".into(),
            seed_base: None,
        };
        let imported = PromptProvenance::Imported;
        let connection = database.connection().expect("lock");
        for (id, kind, provenance, source_id) in [
            (
                derived_id,
                "derived",
                derived.clone(),
                Some(source.id.to_string()),
            ),
            (imported_id, "imported", imported.clone(), None),
        ] {
            connection
                .execute(
                    "INSERT INTO prompt_documents(id,status,name,purpose,condense,behavior_version,provenance_kind,built_in_key,derived_source_id,provenance_json,revision,created_at,updated_at,derived_source_name) VALUES (?1,'active',?2,'direct_chat',0,'legacy_v1',?3,NULL,?4,?5,1,2,2,?6)",
                    params![
                        id.to_string(),
                        format!("{kind} prompt"),
                        kind,
                        source_id,
                        encode(&provenance).expect("provenance"),
                        derived_source_name(&provenance)
                    ],
                )
                .expect("insert provenance");
            drop(provenance);
        }
        drop(connection);
        assert_eq!(
            database
                .get(derived_id)
                .expect("load")
                .expect("present")
                .provenance,
            derived
        );
        assert_eq!(
            database
                .get(imported_id)
                .expect("load")
                .expect("present")
                .provenance,
            imported
        );
        database
            .connection()
            .expect("lock")
            .execute(
                "DELETE FROM prompt_documents WHERE id=?1",
                [source.id.to_string()],
            )
            .expect("derivation history does not restrict deletion");
        assert_eq!(
            database
                .get(derived_id)
                .expect("load")
                .expect("present")
                .provenance,
            derived
        );
        let connection = database.connection().expect("lock");
        assert!(
            sync_prompt_ids(&connection)
                .expect("sync ids")
                .contains(&derived_id.to_string())
        );
    }

    #[test]
    fn reverse_dependencies_report_character_group_and_starter_owners() {
        let database = Database::open_in_memory().expect("database");
        let document = database
            .create_user_draft(metadata("referenced"), Vec::new(), TimestampMillis::new(1))
            .expect("prompt");
        let character_id = CharacterId::new();
        let group_id = GroupId::new();
        let starter_id = lettuce_types::ConversationStarterId::new();
        {
            let connection = database.connection().expect("lock");
            connection
                .execute(
                    "INSERT INTO characters(id,status,name,nickname,normalized_name,normalized_nickname,profile_json,provenance_json,defaults_json,interaction_mode,memory_policy,model_profile_id,default_scene_id,default_starter_id,direct_prompt_id,group_conversation_prompt_id,group_roleplay_prompt_id,voice_profile_id,voice_legacy_locator,voice_autoplay,presentation_json,image_recommendation_json,revision,created_at,updated_at) VALUES (?1,'active','Character',NULL,'character',NULL,'{}','{}','{}','roleplay','manual',NULL,NULL,NULL,?2,NULL,?2,NULL,NULL,0,'{}',NULL,1,1,1)",
                    params![character_id.to_string(), document.id.to_string()],
                )
                .expect("character reference");
            connection
                .execute(
                    "INSERT INTO groups(id,status,name,normalized_name,chat_mode,persona_selection_kind,persona_id,speaker_selection,memory_policy,disable_character_lorebooks,group_conversation_prompt_id,group_roleplay_prompt_id,presentation_json,background_asset_id,background_blob_kind,starting_scene_id,revision,created_at,updated_at) VALUES (?1,'active','Group','group','conversation','inherit',NULL,'llm','manual',0,NULL,?2,'{}',NULL,'image',NULL,1,1,1)",
                    params![group_id.to_string(), document.id.to_string()],
                )
                .expect("group reference");
            connection
                .execute(
                    "INSERT INTO conversation_starters(character_id,id,name,ordinal,scene_id,prompt_id,lorebooks_json,revision,created_at,updated_at) VALUES (?1,?2,'Starter',0,NULL,?3,'{}',1,1,1)",
                    params![character_id.to_string(), starter_id.to_string(), document.id.to_string()],
                )
                .expect("starter reference");
        }
        let references = database.references_to(document.id).expect("dependencies");
        assert_eq!(references.len(), 4);
        assert_eq!(
            references
                .iter()
                .map(|reference| reference.purpose)
                .collect::<Vec<_>>(),
            vec![
                PromptPurpose::DirectChat,
                PromptPurpose::GroupChatRoleplay,
                PromptPurpose::GroupChatRoleplay,
                PromptPurpose::DirectChat,
            ]
        );
        assert!(references.iter().any(|reference| matches!(
            reference.owner,
            PromptReferenceOwner::Character(id) if id == character_id
        )));
        assert!(references.iter().any(|reference| matches!(
            reference.owner,
            PromptReferenceOwner::Group(id) if id == group_id
        )));
        assert!(references.iter().any(|reference| matches!(
            reference.owner,
            PromptReferenceOwner::Starter { character_id: id, starter_id: sid }
                if id == character_id && sid == starter_id
        )));
    }

    #[test]
    fn two_database_handles_use_root_cas_for_prompt_writes() {
        let path =
            std::env::temp_dir().join(format!("lettuce-prompt-{}.sqlite", uuid::Uuid::new_v4()));
        let first = Database::open(&path).expect("first database");
        let second = Database::open(&path).expect("second database");
        let document = first
            .create_user_draft(metadata("race"), Vec::new(), TimestampMillis::new(1))
            .expect("create");
        let snapshot = second.get(document.id).expect("snapshot").expect("present");
        let changed = first
            .revise_metadata(
                document.id,
                document.revision,
                metadata("changed"),
                TimestampMillis::new(2),
            )
            .expect("first revision")
            .document;
        assert_eq!(snapshot.revision, Revision::INITIAL);
        assert_eq!(changed.revision, Revision::new(2));
        assert!(matches!(
            second.revise_metadata(
                document.id,
                snapshot.revision,
                metadata("stale"),
                TimestampMillis::new(3)
            ),
            Err(PromptRepositoryError::Conflict)
        ));
        assert_eq!(
            second.get(document.id).expect("reload").expect("present"),
            changed
        );
        drop(second);
        drop(first);
        std::fs::remove_file(path).expect("remove database");
    }

    fn core_seed(version: u32, entries: &[(&str, &str)]) -> BuiltInPromptSeed {
        BuiltInPromptSeed {
            key: "core".into(),
            aliases: Vec::new(),
            seed_version: version,
            metadata: metadata("Core"),
            entries: entries
                .iter()
                .map(|(key, content)| {
                    let mut entry = draft(key);
                    entry.built_in_entry_key = Some((*key).into());
                    entry.content = (*content).into();
                    entry
                })
                .collect(),
            required: false,
            protected: false,
        }
    }

    fn reconcile(
        database: &Database,
        seed: BuiltInPromptSeed,
        now: i64,
    ) -> BuiltInReconcileOutcome {
        database
            .reconcile_built_ins(
                BuiltInReconcileRequest {
                    seeds: vec![seed],
                    mode: BuiltInReconcileMode::RefreshUnedited,
                },
                TimestampMillis::new(now),
            )
            .expect("reconcile")
            .remove(0)
    }

    fn edit_content(
        database: &Database,
        id: PromptDocumentId,
        key: &str,
        content: &str,
        now: i64,
    ) -> PromptDocument {
        let document = database.get(id).expect("get").expect("exists");
        let edits = document
            .entries
            .iter()
            .map(|entry| {
                let mut draft = PromptEntryDraft::from(entry.clone());
                if entry.built_in_entry_key.as_deref() == Some(key) {
                    draft.content = content.into();
                }
                draft.built_in_entry_key = None;
                PromptEntryEdit {
                    entry_id: Some(entry.id),
                    draft,
                }
            })
            .collect();
        database
            .revise_prompt_document(
                id,
                document.revision,
                metadata(&document.name),
                edits,
                TimestampMillis::new(now),
            )
            .expect("edit")
    }

    fn stored_row(database: &Database, id: PromptDocumentId) -> (String, i64, i64) {
        database
            .connection()
            .expect("lock")
            .query_row(
                "SELECT provenance_json,revision,updated_at FROM prompt_documents WHERE id=?1",
                [id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("row")
    }

    fn strip_seed_base(database: &Database, id: PromptDocumentId) {
        let mut document = database.get(id).expect("get").expect("exists");
        document.provenance.set_seed_base(None);
        database
            .connection()
            .expect("lock")
            .execute(
                "UPDATE prompt_documents SET provenance_json=?2 WHERE id=?1",
                params![
                    id.to_string(),
                    encode(&document.provenance).expect("encode")
                ],
            )
            .expect("strip");
    }

    fn copy_of(
        database: &Database,
        source: PromptDocumentId,
        drafts: Vec<PromptEntryDraft>,
        now: i64,
    ) -> PromptDocument {
        let mut connection = database.connection().expect("lock");
        let tx = connection.transaction().expect("transaction");
        let copy = create_user_draft_in(
            &tx,
            metadata("Copy"),
            drafts,
            TimestampMillis::new(now),
            Some(source),
        )
        .expect("copy");
        tx.commit().expect("commit");
        copy
    }

    fn drafts_of(document: &PromptDocument) -> Vec<PromptEntryDraft> {
        document
            .entries
            .iter()
            .map(|entry| {
                let mut draft = PromptEntryDraft::from(entry.clone());
                draft.built_in_entry_key = None;
                draft
            })
            .collect()
    }

    fn apply(
        database: &Database,
        id: PromptDocumentId,
        seed: &BuiltInPromptSeed,
        choices: &[(&str, bool)],
        now: i64,
    ) -> Result<PromptDocument, PromptRepositoryError> {
        let revision = database.get(id).expect("get").expect("exists").revision;
        let choices: Vec<(String, bool)> = choices
            .iter()
            .map(|(key, take)| ((*key).to_owned(), *take))
            .collect();
        let mut connection = database.connection().expect("lock");
        let tx = connection.transaction().expect("transaction");
        let document =
            apply_built_in_update_in(&tx, id, revision, seed, &choices, TimestampMillis::new(now))?;
        tx.commit().expect("commit");
        Ok(document)
    }

    fn contents(document: &PromptDocument) -> Vec<(&str, &str)> {
        document
            .entries
            .iter()
            .map(|entry| (entry.name.as_str(), entry.content.as_str()))
            .collect()
    }

    #[test]
    fn reconcile_keeps_the_seed_base_of_an_edited_built_in_and_leaves_the_row_alone() {
        let database = Database::open_in_memory().expect("database");
        let first = core_seed(1, &[("a", "a1"), ("b", "b1")]);
        let created = reconcile(&database, first.clone(), 1);
        assert_eq!(created.action, BuiltInReconcileAction::Created);
        assert_eq!(
            created.document.provenance.seed_base(),
            Some(&first.seed_base())
        );
        let id = created.document.id;
        edit_content(&database, id, "a", "mine", 2);
        let second = core_seed(2, &[("a", "a2"), ("b", "b1")]);
        let kept = reconcile(&database, second.clone(), 3);
        assert_eq!(kept.action, BuiltInReconcileAction::PreservedEdited);
        assert_eq!(
            kept.document.provenance.seed_base(),
            Some(&first.seed_base())
        );
        assert_eq!(kept.document.entries[0].content, "mine");
        let row = stored_row(&database, id);
        let again = reconcile(&database, second, 4);
        assert_eq!(again.action, BuiltInReconcileAction::PreservedEdited);
        assert_eq!(stored_row(&database, id), row);
    }

    #[test]
    fn reconcile_backfills_a_missing_seed_base_with_the_current_seed() {
        let database = Database::open_in_memory().expect("database");
        let first = core_seed(1, &[("a", "a1")]);
        let id = reconcile(&database, first, 1).document.id;
        edit_content(&database, id, "a", "mine", 2);
        strip_seed_base(&database, id);
        assert_eq!(
            database
                .get(id)
                .expect("get")
                .expect("exists")
                .provenance
                .seed_base(),
            None
        );
        let second = core_seed(2, &[("a", "a2")]);
        let outcome = reconcile(&database, second.clone(), 3);
        assert_eq!(outcome.action, BuiltInReconcileAction::PreservedEdited);
        assert_eq!(
            outcome.document.provenance.seed_base(),
            Some(&second.seed_base())
        );
        let row = stored_row(&database, id);
        reconcile(&database, second, 4);
        assert_eq!(stored_row(&database, id), row);
    }

    #[test]
    fn a_copy_builds_its_seed_base_from_the_entries_it_kept() {
        let database = Database::open_in_memory().expect("database");
        let seed = core_seed(1, &[("a", "a1"), ("b", "b1"), ("c", "c1")]);
        let source = reconcile(&database, seed.clone(), 1).document;
        let source = edit_content(&database, source.id, "a", "mine a", 2);
        let mut drafts = drafts_of(&source);
        drafts[1].content = "tweaked b".into();
        drafts.push(draft("own"));
        let copy = copy_of(&database, source.id, drafts, 3);
        let base = copy.provenance.seed_base().expect("base");
        assert_eq!(base.built_in_key, "core");
        let seed_base = seed.seed_base();
        assert_eq!(
            base.entries,
            vec![
                PromptSeedBaseEntry {
                    entry_key: "a".into(),
                    entry_id: Some(copy.entries[0].id),
                    seed_entry_digest: seed_base.entries[0].seed_entry_digest.clone(),
                },
                PromptSeedBaseEntry {
                    entry_key: "b".into(),
                    entry_id: None,
                    seed_entry_digest: seed_base.entries[1].seed_entry_digest.clone(),
                },
                PromptSeedBaseEntry {
                    entry_key: "c".into(),
                    entry_id: Some(copy.entries[2].id),
                    seed_entry_digest: seed_base.entries[2].seed_entry_digest.clone(),
                },
            ]
        );
        assert!(built_in_update(&copy, &seed).is_empty());
        let grandchild = copy_of(&database, copy.id, drafts_of(&copy), 4);
        let inherited = grandchild.provenance.seed_base().expect("inherited");
        assert_eq!(
            inherited
                .entries
                .iter()
                .map(|entry| entry.entry_id)
                .collect::<Vec<_>>(),
            vec![
                Some(grandchild.entries[0].id),
                None,
                Some(grandchild.entries[2].id)
            ]
        );
        assert!(built_in_update(&grandchild, &seed).is_empty());
        let user = database
            .create_user_draft(metadata("Own"), vec![draft("x")], TimestampMillis::new(5))
            .expect("user");
        let from_user = copy_of(&database, user.id, vec![draft("x")], 6);
        assert_eq!(from_user.provenance.seed_base(), None);
    }

    #[test]
    fn an_existing_copy_gets_its_seed_base_once_without_a_revision_bump() {
        let database = Database::open_in_memory().expect("database");
        let seed = core_seed(1, &[("a", "a1"), ("b", "b1")]);
        let source = reconcile(&database, seed.clone(), 1).document;
        let mut drafts = drafts_of(&source);
        drafts[1].content = "tweaked b".into();
        let copy = copy_of(&database, source.id, drafts, 2);
        strip_seed_base(&database, copy.id);
        let before = stored_row(&database, copy.id);
        reconcile(&database, seed.clone(), 3);
        let filled = database.get(copy.id).expect("get").expect("exists");
        let base = filled.provenance.seed_base().expect("base");
        assert_eq!(base.entries.len(), 2);
        assert_eq!(base.entries[0].entry_key, "a");
        assert_eq!(base.entries[0].entry_id, Some(copy.entries[0].id));
        assert_eq!(base.entries[1].entry_key, "b");
        assert_eq!(base.entries[1].entry_id, None);
        assert!(built_in_update(&filled, &seed).is_empty());
        assert_eq!(
            base.entries[0].seed_entry_digest,
            seed.seed_base().entries[0].seed_entry_digest
        );
        let after = stored_row(&database, copy.id);
        assert_eq!((after.1, after.2), (before.1, before.2));
        assert_ne!(after.0, before.0);
        reconcile(&database, seed, 4);
        assert_eq!(stored_row(&database, copy.id), after);
    }

    #[test]
    fn applying_a_built_in_update_takes_or_keeps_every_kind_of_change() {
        let build = || {
            let database = Database::open_in_memory().expect("database");
            let first = core_seed(1, &[("a", "a1"), ("b", "b1"), ("d", "d1")]);
            let id = reconcile(&database, first, 1).document.id;
            edit_content(&database, id, "b", "mine b", 2);
            let second = core_seed(2, &[("a", "a2"), ("e", "e1"), ("b", "b2")]);
            let outcome = reconcile(&database, second.clone(), 3);
            assert_eq!(outcome.action, BuiltInReconcileAction::PreservedEdited);
            (database, id, second)
        };
        let all = [("a", true), ("e", true), ("b", true), ("d", true)];

        let (database, id, second) = build();
        let before = database.get(id).expect("get").expect("exists");
        let taken = apply(&database, id, &second, &all, 4).expect("apply");
        assert_eq!(
            contents(&taken),
            vec![("a", "a2"), ("e", "e1"), ("b", "b2")]
        );
        assert_eq!(taken.entries[0].id, before.entries[0].id);
        assert_eq!(taken.entries[2].id, before.entries[1].id);
        assert_eq!(taken.entries[1].id, built_in_entry_id(id, "e"));
        assert_eq!(taken.entries[1].built_in_entry_key.as_deref(), Some("e"));
        assert_eq!(taken.revision, Revision::new(before.revision.get() + 1));
        assert_eq!(taken.updated_at, TimestampMillis::new(4));
        assert_eq!(taken.provenance.seed_base(), Some(&second.seed_base()));
        assert!(built_in_update(&taken, &second).is_empty());
        let PromptProvenance::BuiltIn {
            seed_digest,
            authored_digest,
            ..
        } = &taken.provenance
        else {
            panic!("built in");
        };
        assert_eq!(seed_digest, authored_digest);

        let (database, id, second) = build();
        let before = database.get(id).expect("get").expect("exists");
        let none = [("a", false), ("e", false), ("b", false), ("d", false)];
        let kept = apply(&database, id, &second, &none, 4).expect("apply");
        assert_eq!(contents(&kept), contents(&before));
        assert_eq!(kept.provenance.seed_base(), Some(&second.seed_base()));
        assert!(built_in_update(&kept, &second).is_empty());
        assert_eq!(kept.revision, Revision::new(before.revision.get() + 1));

        let (database, id, second) = build();
        let mixed = [("a", true), ("e", false), ("b", false), ("d", true)];
        let document = apply(&database, id, &second, &mixed, 4).expect("apply");
        assert_eq!(contents(&document), vec![("a", "a2"), ("b", "mine b")]);
        assert!(built_in_update(&document, &second).is_empty());
    }

    #[test]
    fn an_added_entry_goes_after_the_nearest_preceding_seed_entry_that_exists() {
        let database = Database::open_in_memory().expect("database");
        let first = core_seed(1, &[("a", "a1"), ("c", "c1")]);
        let id = reconcile(&database, first, 1).document.id;
        edit_content(&database, id, "c", "mine c", 2);
        let second = core_seed(
            2,
            &[
                ("x", "x1"),
                ("a", "a1"),
                ("b", "b1"),
                ("y", "y1"),
                ("c", "c1"),
            ],
        );
        reconcile(&database, second.clone(), 3);
        let choices = [("x", true), ("b", true), ("y", true)];
        let document = apply(&database, id, &second, &choices, 4).expect("apply");
        assert_eq!(
            contents(&document),
            vec![
                ("x", "x1"),
                ("a", "a1"),
                ("b", "b1"),
                ("y", "y1"),
                ("c", "mine c")
            ]
        );
    }

    #[test]
    fn a_stale_or_incomplete_set_of_choices_is_refused() {
        let database = Database::open_in_memory().expect("database");
        let first = core_seed(1, &[("a", "a1"), ("b", "b1")]);
        let id = reconcile(&database, first, 1).document.id;
        edit_content(&database, id, "a", "mine a", 2);
        let second = core_seed(2, &[("a", "a2"), ("b", "b2")]);
        reconcile(&database, second.clone(), 3);
        assert!(matches!(
            apply(&database, id, &second, &[("a", true), ("zzz", true)], 4),
            Err(PromptRepositoryError::Conflict)
        ));
        assert!(matches!(
            apply(&database, id, &second, &[("a", true)], 4),
            Err(PromptRepositoryError::Invalid(
                PromptValidationError::MissingBuiltInUpdateChoice(key)
            )) if key == "b"
        ));
        let revision = database.get(id).expect("get").expect("exists").revision;
        apply(&database, id, &second, &[("a", true), ("b", true)], 5).expect("apply");
        assert!(matches!(
            apply(&database, id, &second, &[("a", true), ("b", true)], 6),
            Err(PromptRepositoryError::Conflict)
        ));
        let mut connection = database.connection().expect("lock");
        let tx = connection.transaction().expect("transaction");
        assert!(matches!(
            apply_built_in_update_in(&tx, id, revision, &second, &[], TimestampMillis::new(7)),
            Err(PromptRepositoryError::Conflict)
        ));
        let current = load_required(&tx, id).expect("current").revision;
        assert!(matches!(
            apply_built_in_update_in(&tx, id, current, &second, &[], TimestampMillis::new(8)),
            Err(PromptRepositoryError::Conflict)
        ));
    }

    #[test]
    fn a_copy_applies_an_update_through_its_recorded_entry_ids() {
        let database = Database::open_in_memory().expect("database");
        let first = core_seed(1, &[("a", "a1"), ("b", "b1")]);
        let source = reconcile(&database, first, 1).document;
        let copy = copy_of(&database, source.id, drafts_of(&source), 2);
        let other = copy_of(&database, source.id, drafts_of(&source), 5);
        let second = core_seed(2, &[("a", "a2"), ("c", "c1"), ("b", "b1")]);
        reconcile(&database, second.clone(), 3);
        let pending = built_in_update(
            &database.get(copy.id).expect("get").expect("exists"),
            &second,
        );
        assert_eq!(
            pending
                .iter()
                .map(PromptBuiltInChange::entry_key)
                .collect::<Vec<_>>(),
            vec!["a", "c"]
        );
        let taken =
            apply(&database, copy.id, &second, &[("a", true), ("c", true)], 4).expect("apply");
        assert_eq!(
            contents(&taken),
            vec![("a", "a2"), ("c", "c1"), ("b", "b1")]
        );
        assert_eq!(taken.entries[0].id, copy.entries[0].id);
        assert!(
            taken
                .entries
                .iter()
                .all(|entry| entry.built_in_entry_key.is_none())
        );
        let base = taken.provenance.seed_base().expect("base");
        assert_eq!(
            base.entries
                .iter()
                .map(|entry| entry.entry_id)
                .collect::<Vec<_>>(),
            vec![
                Some(taken.entries[0].id),
                Some(taken.entries[1].id),
                Some(taken.entries[2].id)
            ]
        );
        assert!(built_in_update(&taken, &second).is_empty());

        let kept = apply(
            &database,
            other.id,
            &second,
            &[("a", false), ("c", false)],
            6,
        )
        .expect("apply");
        assert_eq!(contents(&kept), vec![("a", "a1"), ("b", "b1")]);
        let base = kept.provenance.seed_base().expect("base");
        assert_eq!(
            base.entries
                .iter()
                .map(|entry| entry.entry_id)
                .collect::<Vec<_>>(),
            vec![Some(kept.entries[0].id), None, Some(kept.entries[1].id)]
        );
        assert!(built_in_update(&kept, &second).is_empty());
    }
}
