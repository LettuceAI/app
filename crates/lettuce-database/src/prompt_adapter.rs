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
    PromptBootstrapError, PromptBootstrapPort, PromptDependencyError, PromptDependencyReader,
    PromptDocument, PromptEntry, PromptEntryDraft, PromptEntryInsertionTarget, PromptEntryMutation,
    PromptEntryPosition, PromptEntryRole, PromptLibraryQuery, PromptLookupResult,
    PromptMetadataDraft, PromptMutationResult, PromptProvenance, PromptPurpose, PromptReference,
    PromptReferenceOwner, PromptRepository, PromptRepositoryError, PromptValidationError,
    classify_prompt_lookup,
};
use lettuce_types::{
    CharacterId, GroupId, Page, PromptDocumentId, PromptEntryId, Revision, TimestampMillis,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::Database;

const JSON_FORMAT_VERSION: u32 = 1;
const CURSOR_FORMAT_VERSION: u32 = 1;

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
        PromptPurpose::GroupChatConversational => "group_chat_conversational",
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
    })
}

fn purpose_from_name(value: &str) -> Result<PromptPurpose, rusqlite::Error> {
    match value {
        "direct_chat" => Ok(PromptPurpose::DirectChat),
        "companion_chat" => Ok(PromptPurpose::CompanionChat),
        "group_chat_roleplay" => Ok(PromptPurpose::GroupChatRoleplay),
        "group_chat_conversational" => Ok(PromptPurpose::GroupChatConversational),
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
        PromptProvenance::Derived { source } => ("derived", None, Some(source.to_string())),
        PromptProvenance::Imported => ("imported", None, None),
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
    Ok(super::hex_encode(&payload))
}

fn cursor_decode(
    value: Option<&str>,
) -> Result<Option<(i64, PromptDocumentId)>, PromptRepositoryError> {
    let Some(value) = value else { return Ok(None) };
    let bytes = super::hex_decode(value)
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
        name: row.get(1)?,
        role: role_from_name(&row.get::<_, String>(2)?)?,
        content: row.get(3)?,
        enabled: row.get(4)?,
        injection_position: position_from_name(&row.get::<_, String>(5)?)?,
        depth: row.get::<_, i64>(6)?.try_into().map_err(|_| invalid())?,
        conditional_min_messages: row
            .get::<_, Option<i64>>(7)?
            .map(|value| value.try_into().map_err(|_| invalid()))
            .transpose()?,
        interval_turns: row
            .get::<_, Option<i64>>(8)?
            .map(|value| value.try_into().map_err(|_| invalid()))
            .transpose()?,
        system_prompt: row.get(9)?,
        conditions: row
            .get::<_, Option<String>>(10)?
            .map(|value| decode(&value))
            .transpose()?,
        payload: row
            .get::<_, Option<String>>(11)?
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

fn load_document(
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
        "SELECT id,name,role,content,enabled,injection_position,depth,conditional_min_messages,interval_turns,system_prompt,conditions_json,payload_json,ordinal,revision,created_at,updated_at FROM prompt_entries WHERE prompt_id=?1 ORDER BY ordinal ASC,id ASC",
    )?;
    let mut entries = Vec::new();
    for (expected_ordinal, row) in statement
        .query_map([id.to_string()], |row| {
            Ok((
                read_entry(row)?,
                row.get::<_, i64>(12)?,
                revision(row.get(13)?)?,
                TimestampMillis::new(row.get(14)?),
                TimestampMillis::new(row.get(15)?),
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
    if entries.len() > lettuce_context::MAX_PROMPT_ENTRIES {
        return Err(PromptRepositoryError::Invalid(
            PromptValidationError::TooManyEntries,
        ));
    }
    entries
        .iter()
        .try_for_each(|entry| entry.validate().map_err(PromptRepositoryError::Invalid))
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
        "INSERT INTO prompt_entries(id,prompt_id,name,role,content,enabled,injection_position,depth,conditional_min_messages,interval_turns,system_prompt,conditions_json,payload_json,ordinal,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
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

fn insert_root(
    tx: &Transaction<'_>,
    document: &PromptDocument,
) -> Result<(), PromptRepositoryError> {
    let provenance_json = encode(&document.provenance)?;
    let (kind, key, source) = provenance_kind(&document.provenance);
    tx.execute(
        "INSERT INTO prompt_documents(id,status,name,purpose,condense,behavior_version,provenance_kind,built_in_key,derived_source_id,provenance_json,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
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
            document.updated_at.get()
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
        "UPDATE prompt_documents SET status=?2,name=?3,purpose=?4,condense=?5,behavior_version=?6,provenance_kind=?7,built_in_key=?8,derived_source_id=?9,provenance_json=?10,revision=?11,updated_at=?12 WHERE id=?1 AND revision=?13",
        params![
            document.id.to_string(), status_name(document.status), document.name,
            purpose_name(document.purpose), document.condense, behavior_name(document.behavior_version),
            kind, key, source, provenance_json, sql_revision(document.revision)?, document.updated_at.get(),
            sql_revision(expected)?
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
            let entry = PromptEntry {
                id: PromptEntryId::new(),
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
            *entry = PromptEntry {
                id: entry_id,
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
            document.entries = drafts
                .into_iter()
                .map(|draft| PromptEntry {
                    id: PromptEntryId::new(),
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
                })
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
            .map(|draft| PromptEntry {
                id: PromptEntryId::new(),
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
            })
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
        {
            if metadata.purpose != current.purpose
                || metadata.behavior_version != current.behavior_version
            {
                return Err(PromptRepositoryError::Protected);
            }
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
        request.validate()?;
        let mut seeds = request.seeds;
        seeds.sort_by_key(|seed| seed.key.trim().to_owned());
        let mut connection = self
            .connection()
            .map_err(|_| PromptBootstrapError::Failure("database lock failure".into()))?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(bootstrap_storage)?;
        let mut outcomes = Vec::with_capacity(seeds.len());
        for seed in seeds {
            let key = seed.key.trim().to_owned();
            let seed_digest = seed.computed_seed_digest()?;
            let existing_id: Option<String> = tx
                .query_row(
                    "SELECT id FROM prompt_documents WHERE built_in_key=?1",
                    [&key],
                    |row| row.get(0),
                )
                .optional()
                .map_err(bootstrap_storage)?;
            let Some(existing_id) = existing_id else {
                let id = PromptDocumentId::new();
                let metadata = seed.metadata.clone();
                let entries = seed
                    .entries
                    .iter()
                    .cloned()
                    .map(|draft| PromptEntry {
                        id: PromptEntryId::new(),
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
                    })
                    .collect();
                let provenance = seed.provenance()?;
                let document = metadata_document(id, metadata, entries, provenance, now)
                    .map_err(|error| PromptBootstrapError::Failure(error.to_string()))?;
                insert_root(&tx, &document)
                    .map_err(|error| PromptBootstrapError::Failure(error.to_string()))?;
                outcomes.push(BuiltInReconcileOutcome {
                    key,
                    action: BuiltInReconcileAction::Created,
                    document: load_required(&tx, id)
                        .map_err(|error| PromptBootstrapError::Failure(error.to_string()))?,
                });
                continue;
            };
            let id: PromptDocumentId = existing_id
                .parse()
                .map_err(|_| PromptBootstrapError::Failure("invalid built-in id".into()))?;
            let current = load_required(&tx, id)
                .map_err(|error| PromptBootstrapError::Failure(error.to_string()))?;
            let PromptProvenance::BuiltIn {
                seed_digest: current_seed,
                authored_digest: current_authored,
                ..
            } = &current.provenance
            else {
                return Err(PromptBootstrapError::Failure(
                    "built-in key points to non-built-in prompt".into(),
                ));
            };
            let edited = current_authored != current_seed;
            if edited && request.mode == BuiltInReconcileMode::RefreshUnedited {
                outcomes.push(BuiltInReconcileOutcome {
                    key,
                    action: BuiltInReconcileAction::PreservedEdited,
                    document: current,
                });
                continue;
            }
            if !edited && current_seed == &seed_digest && current.status == LifecycleStatus::Active
            {
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
            let entries = seed
                .entries
                .iter()
                .cloned()
                .map(|draft| PromptEntry {
                    id: PromptEntryId::new(),
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
                })
                .collect();
            let mut document =
                metadata_document(id, seed.metadata.clone(), entries, seed.provenance()?, now)
                    .map_err(|error| PromptBootstrapError::Failure(error.to_string()))?;
            document.revision = next_revision(current.revision)
                .map_err(|error| PromptBootstrapError::Failure(error.to_string()))?;
            document.created_at = current.created_at;
            document.status = LifecycleStatus::Active;
            update_root(&tx, &document, current.revision)
                .map_err(|error| PromptBootstrapError::Failure(error.to_string()))?;
            replace_entries(&tx, &document, now, &HashSet::new())
                .map_err(|error| PromptBootstrapError::Failure(error.to_string()))?;
            outcomes.push(BuiltInReconcileOutcome {
                key,
                action,
                document: load_required(&tx, id)
                    .map_err(|error| PromptBootstrapError::Failure(error.to_string()))?,
            });
        }
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
        let mut character = tx.prepare("SELECT id,direct_prompt_id,group_conversation_prompt_id,group_roleplay_prompt_id FROM characters WHERE direct_prompt_id=?1 OR group_conversation_prompt_id=?1 OR group_roleplay_prompt_id=?1 ORDER BY id").map_err(|error| PromptDependencyError::Failure(storage(error).to_string()))?;
        for row in character
            .query_map([prompt_id.to_string()], |row| {
                let id: CharacterId = parse_id(row.get(0)?)?;
                let direct: Option<String> = row.get(1)?;
                let conversation: Option<String> = row.get(2)?;
                let roleplay: Option<String> = row.get(3)?;
                Ok((
                    id,
                    direct.as_deref() == Some(&prompt_id.to_string()),
                    conversation.as_deref() == Some(&prompt_id.to_string()),
                    roleplay.as_deref() == Some(&prompt_id.to_string()),
                ))
            })
            .map_err(|error| PromptDependencyError::Failure(storage(error).to_string()))?
        {
            let (id, direct, conversation, roleplay) =
                row.map_err(|error| PromptDependencyError::Failure(storage(error).to_string()))?;
            if direct {
                references.push(PromptReference {
                    owner: PromptReferenceOwner::Character(id),
                    purpose: PromptPurpose::DirectChat,
                    prompt_id,
                });
            }
            if conversation {
                references.push(PromptReference {
                    owner: PromptReferenceOwner::Character(id),
                    purpose: PromptPurpose::GroupChatConversational,
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
        let mut groups = tx.prepare("SELECT id,group_conversation_prompt_id,group_roleplay_prompt_id FROM groups WHERE group_conversation_prompt_id=?1 OR group_roleplay_prompt_id=?1 ORDER BY id").map_err(|error| PromptDependencyError::Failure(storage(error).to_string()))?;
        for row in groups
            .query_map([prompt_id.to_string()], |row| {
                let id: GroupId = parse_id(row.get(0)?)?;
                let conversation: Option<String> = row.get(1)?;
                let roleplay: Option<String> = row.get(2)?;
                Ok((
                    id,
                    conversation.as_deref() == Some(&prompt_id.to_string()),
                    roleplay.as_deref() == Some(&prompt_id.to_string()),
                ))
            })
            .map_err(|error| PromptDependencyError::Failure(storage(error).to_string()))?
        {
            let (id, conversation, roleplay) =
                row.map_err(|error| PromptDependencyError::Failure(storage(error).to_string()))?;
            if conversation {
                references.push(PromptReference {
                    owner: PromptReferenceOwner::Group(id),
                    purpose: PromptPurpose::GroupChatConversational,
                    prompt_id,
                });
            }
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
    fn built_in_reconcile_preserves_and_resets_edits() {
        let database = Database::open_in_memory().expect("database");
        let seed = BuiltInPromptSeed {
            key: "core".into(),
            seed_version: 1,
            metadata: metadata("Core"),
            entries: vec![draft("one")],
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
        assert_eq!(reset[0].document.name, "Core");
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
            PromptPurpose::GroupChatConversational,
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
        let seed = BuiltInPromptSeed {
            key: "protected".into(),
            seed_version: 1,
            metadata: metadata("Protected"),
            entries: vec![draft("entry")],
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
    fn imported_and_derived_provenance_round_trip_with_storage_columns() {
        let database = Database::open_in_memory().expect("database");
        let source = database
            .create_user_draft(metadata("source"), Vec::new(), TimestampMillis::new(1))
            .expect("source");
        let derived_id = PromptDocumentId::new();
        let imported_id = PromptDocumentId::new();
        let derived = PromptProvenance::Derived { source: source.id };
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
                    "INSERT INTO prompt_documents(id,status,name,purpose,condense,behavior_version,provenance_kind,built_in_key,derived_source_id,provenance_json,revision,created_at,updated_at) VALUES (?1,'active',?2,'direct_chat',0,'legacy_v1',?3,NULL,?4,?5,1,2,2)",
                    params![
                        id.to_string(),
                        format!("{kind} prompt"),
                        kind,
                        source_id,
                        encode(&provenance).expect("provenance")
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
                    "INSERT INTO characters(id,status,name,nickname,normalized_name,normalized_nickname,profile_json,provenance_json,defaults_json,interaction_mode,memory_policy,model_profile_id,default_scene_id,default_starter_id,direct_prompt_id,group_conversation_prompt_id,group_roleplay_prompt_id,voice_profile_id,voice_legacy_locator,voice_autoplay,presentation_json,image_recommendation_json,revision,created_at,updated_at) VALUES (?1,'active','Character',NULL,'character',NULL,'{}','{}','{}','roleplay','manual',NULL,NULL,NULL,?2,?2,?2,NULL,NULL,0,'{}',NULL,1,1,1)",
                    params![character_id.to_string(), document.id.to_string()],
                )
                .expect("character reference");
            connection
                .execute(
                    "INSERT INTO groups(id,status,name,normalized_name,chat_mode,persona_selection_kind,persona_id,speaker_selection,memory_policy,disable_character_lorebooks,group_conversation_prompt_id,group_roleplay_prompt_id,presentation_json,background_asset_id,background_blob_kind,starting_scene_id,revision,created_at,updated_at) VALUES (?1,'active','Group','group','conversation','inherit',NULL,'llm','manual',0,?2,?2,'{}',NULL,'image',NULL,1,1,1)",
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
        assert_eq!(references.len(), 6);
        assert_eq!(
            references
                .iter()
                .map(|reference| reference.purpose)
                .collect::<Vec<_>>(),
            vec![
                PromptPurpose::DirectChat,
                PromptPurpose::GroupChatConversational,
                PromptPurpose::GroupChatRoleplay,
                PromptPurpose::GroupChatConversational,
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
}
