//! SQLite persistence for lorebooks and their typed owner bindings.
//!
//! A lorebook is stored as one aggregate.  The adapter owns child identity,
//! ordering, revision and timestamps; callers only provide authored metadata
//! and entry content.  Binding mutations use the owning aggregate revision as
//! their CAS token and never expose a storage transaction to domain callers.

use std::{collections::HashMap, convert::TryFrom};

use lettuce_characters::Selection;
use lettuce_context::{
    BindingInsertionTarget, BindingMutationResult, BindingRepositoryError,
    CharacterLorebookBindingRepository, DetectionPolicy, GroupLorebookBindingRepository,
    KeywordMatchMode, LifecycleFilter, LifecycleStatus, Lorebook, LorebookBehaviorVersion,
    LorebookBinding, LorebookBindingCreate, LorebookDependencyError, LorebookDependencyReader,
    LorebookDetails, LorebookEntry, LorebookEntryDraft, LorebookEntryInsertionTarget,
    LorebookEntryMutation, LorebookLibraryQuery, LorebookMetadataDraft, LorebookMutationResult,
    LorebookReference, LorebookRepository, LorebookRepositoryError, LorebookValidationError,
    PersonaLorebookBindingRepository,
};
use lettuce_types::{
    CharacterId, GroupId, LorebookEntryId, LorebookId, Page, PersonaId, Revision, TimestampMillis,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::Database;

const JSON_VERSION: u32 = 1;
const CURSOR_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope<T> {
    format_version: u32,
    value: T,
}

fn invalid() -> rusqlite::Error {
    rusqlite::Error::InvalidQuery
}

fn parse_id<T: std::str::FromStr>(value: String) -> rusqlite::Result<T> {
    value.parse().map_err(|_| invalid())
}

fn parse_revision(value: i64) -> rusqlite::Result<Revision> {
    u64::try_from(value)
        .map(Revision::new)
        .map_err(|_| invalid())
}

fn sql_revision(value: Revision) -> Result<i64, LorebookRepositoryError> {
    i64::try_from(value.get()).map_err(|_| failure("revision does not fit SQLite"))
}

fn encode<T: Serialize>(value: &T) -> Result<String, LorebookRepositoryError> {
    serde_json::to_string(&Envelope {
        format_version: JSON_VERSION,
        value,
    })
    .map_err(|_| failure("could not encode lorebook payload"))
}

fn decode<T: DeserializeOwned>(payload: &str) -> rusqlite::Result<T> {
    let envelope: Envelope<T> = serde_json::from_str(payload).map_err(|_| invalid())?;
    if envelope.format_version != JSON_VERSION {
        return Err(invalid());
    }
    Ok(envelope.value)
}

fn failure(message: impl Into<String>) -> LorebookRepositoryError {
    LorebookRepositoryError::Failure(message.into())
}

fn db_error(error: rusqlite::Error) -> LorebookRepositoryError {
    match error {
        rusqlite::Error::InvalidQuery => failure("invalid or corrupt lorebook storage"),
        rusqlite::Error::SqliteFailure(code, _)
            if code.extended_code == 1555 || code.extended_code == 2067 =>
        {
            LorebookRepositoryError::Conflict
        }
        rusqlite::Error::SqliteFailure(code, _)
            if code.extended_code == 787
                || code.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            failure("lorebook storage constraint failed")
        }
        _ => failure("lorebook storage operation failed"),
    }
}

fn binding_failure(message: impl Into<String>) -> BindingRepositoryError {
    BindingRepositoryError::Failure(message.into())
}

fn binding_db_error(error: rusqlite::Error) -> BindingRepositoryError {
    match error {
        rusqlite::Error::InvalidQuery => binding_failure("invalid or corrupt binding storage"),
        rusqlite::Error::SqliteFailure(code, _)
            if code.extended_code == 1555 || code.extended_code == 2067 =>
        {
            BindingRepositoryError::Conflict
        }
        _ => binding_failure("binding storage operation failed"),
    }
}

fn status_name(value: LifecycleStatus) -> &'static str {
    match value {
        LifecycleStatus::Active => "active",
        LifecycleStatus::Archived => "archived",
    }
}

fn status(value: &str) -> rusqlite::Result<LifecycleStatus> {
    match value {
        "active" => Ok(LifecycleStatus::Active),
        "archived" => Ok(LifecycleStatus::Archived),
        _ => Err(invalid()),
    }
}

fn detection_policy_name(value: DetectionPolicy) -> &'static str {
    match value {
        DetectionPolicy::RecentMessageWindow => "recent_message_window",
        DetectionPolicy::LatestUserMessage => "latest_user_message",
    }
}

fn detection_policy(value: &str) -> rusqlite::Result<DetectionPolicy> {
    match value {
        "recent_message_window" => Ok(DetectionPolicy::RecentMessageWindow),
        "latest_user_message" => Ok(DetectionPolicy::LatestUserMessage),
        _ => Err(invalid()),
    }
}

fn behavior_name(value: LorebookBehaviorVersion) -> &'static str {
    match value {
        LorebookBehaviorVersion::LegacyV1 => "legacy_v1",
        LorebookBehaviorVersion::DeterministicV2 => "deterministic_v2",
    }
}

fn behavior(value: &str) -> rusqlite::Result<LorebookBehaviorVersion> {
    match value {
        "legacy_v1" => Ok(LorebookBehaviorVersion::LegacyV1),
        "deterministic_v2" => Ok(LorebookBehaviorVersion::DeterministicV2),
        _ => Err(invalid()),
    }
}

fn match_mode_name(value: KeywordMatchMode) -> &'static str {
    match value {
        KeywordMatchMode::Literal => "literal",
        KeywordMatchMode::Regex => "regex",
    }
}

fn match_mode(value: &str) -> rusqlite::Result<KeywordMatchMode> {
    match value {
        "literal" => Ok(KeywordMatchMode::Literal),
        "regex" => Ok(KeywordMatchMode::Regex),
        _ => Err(invalid()),
    }
}

fn bool_value(value: i64) -> rusqlite::Result<bool> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(invalid()),
    }
}

fn parse_ordinal(value: i64) -> rusqlite::Result<u32> {
    u32::try_from(value).map_err(|_| invalid())
}

fn cursor_encode(
    updated_at: TimestampMillis,
    id: LorebookId,
) -> Result<String, LorebookRepositoryError> {
    let bytes = serde_json::to_vec(&Envelope {
        format_version: CURSOR_VERSION,
        value: (updated_at.get(), id.to_string()),
    })
    .map_err(|_| failure("could not encode lorebook cursor"))?;
    Ok(super::hex_encode(&bytes))
}

fn cursor_decode(
    value: Option<&str>,
) -> Result<Option<(i64, LorebookId)>, LorebookRepositoryError> {
    let Some(value) = value else { return Ok(None) };
    let bytes = super::hex_decode(value).map_err(|_| failure("invalid lorebook cursor"))?;
    let envelope: Envelope<(i64, String)> =
        serde_json::from_slice(&bytes).map_err(|_| failure("invalid lorebook cursor"))?;
    if envelope.format_version != CURSOR_VERSION {
        return Err(failure("invalid lorebook cursor version"));
    }
    let id = envelope
        .value
        .1
        .parse()
        .map_err(|_| failure("invalid lorebook cursor id"))?;
    Ok(Some((envelope.value.0, id)))
}

fn parse_book(row: &Row<'_>) -> rusqlite::Result<Lorebook> {
    let id = parse_id(row.get::<_, String>(0)?)?;
    let icon_kind = row.get::<_, String>(5)?;
    if icon_kind != "image" {
        return Err(invalid());
    }
    Ok(Lorebook {
        id,
        status: status(&row.get::<_, String>(1)?)?,
        name: row.get(2)?,
        detection_policy: detection_policy(&row.get::<_, String>(3)?)?,
        icon_asset_id: row.get::<_, Option<String>>(4)?.map(parse_id).transpose()?,
        behavior_version: behavior(&row.get::<_, String>(6)?)?,
        revision: parse_revision(row.get(7)?)?,
        created_at: TimestampMillis::new(row.get(8)?),
        updated_at: TimestampMillis::new(row.get(9)?),
    })
}

fn parse_entry(row: &Row<'_>, book: LorebookId) -> rusqlite::Result<LorebookEntry> {
    let id = parse_id(row.get::<_, String>(0)?)?;
    let stored_book = parse_id(row.get::<_, String>(1)?)?;
    if stored_book != book {
        return Err(invalid());
    }
    let keywords: Vec<String> = decode(&row.get::<_, String>(5)?)?;
    Ok(LorebookEntry {
        id,
        lorebook_id: stored_book,
        title: row.get(2)?,
        enabled: bool_value(row.get(3)?)?,
        always_active: bool_value(row.get(4)?)?,
        keywords,
        case_sensitive: bool_value(row.get(6)?)?,
        match_mode: match_mode(&row.get::<_, String>(7)?)?,
        content: row.get(8)?,
        priority: row.get(9)?,
        ordinal: parse_ordinal(row.get(10)?)?,
        revision: parse_revision(row.get(11)?)?,
        created_at: TimestampMillis::new(row.get(12)?),
        updated_at: TimestampMillis::new(row.get(13)?),
    })
}

fn load_details(
    connection: &Connection,
    id: LorebookId,
) -> rusqlite::Result<Option<LorebookDetails>> {
    let Some(book) = connection
        .query_row(
            "SELECT id,status,name,detection_policy,icon_asset_id,icon_blob_kind,behavior_version,revision,created_at,updated_at FROM lorebooks WHERE id=?1",
            [id.to_string()],
            parse_book,
        )
        .optional()?
    else {
        return Ok(None);
    };
    let mut statement = connection.prepare(
        "SELECT id,lorebook_id,title,enabled,always_active,keywords_json,case_sensitive,match_mode,content,priority,ordinal,revision,created_at,updated_at FROM lorebook_entries WHERE lorebook_id=?1 ORDER BY ordinal,id",
    )?;
    let mut rows = statement.query([id.to_string()])?;
    let mut entries = Vec::new();
    while let Some(row) = rows.next()? {
        entries.push(parse_entry(row, id)?);
    }
    let details = LorebookDetails { book, entries };
    details.validate().map_err(|_| invalid())?;
    Ok(Some(details))
}

fn load_required(
    connection: &Connection,
    id: LorebookId,
) -> Result<LorebookDetails, LorebookRepositoryError> {
    load_details(connection, id)
        .map_err(db_error)?
        .ok_or(LorebookRepositoryError::NotFound)
}

fn verify_icon(
    connection: &Connection,
    asset: Option<lettuce_types::AssetId>,
) -> Result<(), LorebookRepositoryError> {
    let Some(asset) = asset else { return Ok(()) };
    let kind = connection
        .query_row(
            "SELECT blob_kind FROM media_assets WHERE id=?1",
            [asset.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(db_error)?
        .ok_or_else(|| failure("lorebook icon asset does not exist"))?;
    if kind != "image" {
        return Err(failure("lorebook icon asset is not an image"));
    }
    Ok(())
}

fn bump_revision(revision: Revision) -> Result<Revision, LorebookRepositoryError> {
    revision.next().map_err(|_| failure("revision overflow"))
}

fn write_book_head(
    tx: &Transaction<'_>,
    book: &Lorebook,
    expected: Revision,
) -> Result<(), LorebookRepositoryError> {
    let changed = tx
        .execute(
            "UPDATE lorebooks SET status=?2,name=?3,detection_policy=?4,icon_asset_id=?5,icon_blob_kind='image',behavior_version=?6,revision=?7,updated_at=?8 WHERE id=?1 AND revision=?9",
            params![
                book.id.to_string(),
                status_name(book.status),
                book.name,
                detection_policy_name(book.detection_policy),
                book.icon_asset_id.map(|id| id.to_string()),
                behavior_name(book.behavior_version),
                sql_revision(book.revision)?,
                book.updated_at.get(),
                sql_revision(expected)?,
            ],
        )
        .map_err(db_error)?;
    if changed != 1 {
        return Err(LorebookRepositoryError::Conflict);
    }
    Ok(())
}

fn insert_entry(
    tx: &Transaction<'_>,
    entry: &LorebookEntry,
) -> Result<(), LorebookRepositoryError> {
    tx.execute(
        "INSERT INTO lorebook_entries (id,lorebook_id,title,enabled,always_active,keywords_json,case_sensitive,match_mode,content,priority,ordinal,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
        params![
            entry.id.to_string(),
            entry.lorebook_id.to_string(),
            entry.title,
            i64::from(entry.enabled),
            i64::from(entry.always_active),
            encode(&entry.keywords)?,
            i64::from(entry.case_sensitive),
            match_mode_name(entry.match_mode),
            entry.content,
            entry.priority,
            i64::from(entry.ordinal),
            sql_revision(entry.revision)?,
            entry.created_at.get(),
            entry.updated_at.get(),
        ],
    )
    .map_err(db_error)?;
    Ok(())
}

fn initial_entry(
    book: LorebookId,
    draft: LorebookEntryDraft,
    ordinal: usize,
    now: TimestampMillis,
) -> Result<LorebookEntry, LorebookRepositoryError> {
    let ordinal = u32::try_from(ordinal).map_err(|_| failure("entry ordinal overflow"))?;
    let entry = LorebookEntry {
        id: LorebookEntryId::new(),
        lorebook_id: book,
        title: draft.title,
        enabled: draft.enabled,
        always_active: draft.always_active,
        keywords: draft.keywords,
        case_sensitive: draft.case_sensitive,
        match_mode: draft.match_mode,
        content: draft.content,
        priority: draft.priority,
        ordinal,
        revision: Revision::INITIAL,
        created_at: now,
        updated_at: now,
    };
    entry.validate()?;
    Ok(entry)
}

fn updated_entry(
    current: &LorebookEntry,
    draft: LorebookEntryDraft,
    now: TimestampMillis,
) -> Result<LorebookEntry, LorebookRepositoryError> {
    let entry = LorebookEntry {
        id: current.id,
        lorebook_id: current.lorebook_id,
        title: draft.title,
        enabled: draft.enabled,
        always_active: draft.always_active,
        keywords: draft.keywords,
        case_sensitive: draft.case_sensitive,
        match_mode: draft.match_mode,
        content: draft.content,
        priority: draft.priority,
        ordinal: current.ordinal,
        revision: bump_revision(current.revision)?,
        created_at: current.created_at,
        updated_at: now,
    };
    entry.validate()?;
    Ok(entry)
}

fn validate_entries_for_book(details: &LorebookDetails) -> Result<(), LorebookRepositoryError> {
    details.validate()?;
    Ok(())
}

fn normalize_entry_metadata(
    entries: &mut [LorebookEntry],
    previous: &[LorebookEntry],
    now: TimestampMillis,
) -> Result<(), LorebookRepositoryError> {
    let previous_by_id: HashMap<_, _> = previous.iter().map(|entry| (entry.id, entry)).collect();
    for (ordinal, entry) in entries.iter_mut().enumerate() {
        entry.ordinal = u32::try_from(ordinal).map_err(|_| failure("entry ordinal overflow"))?;
        let Some(old) = previous_by_id.get(&entry.id) else {
            continue;
        };
        if old.ordinal != entry.ordinal {
            entry.revision = old
                .revision
                .next()
                .map_err(|_| failure("entry revision overflow"))?;
            entry.created_at = old.created_at;
            entry.updated_at = now;
        }
    }
    Ok(())
}

pub(crate) fn insert_lorebook_details(
    tx: &Transaction<'_>,
    details: &LorebookDetails,
) -> Result<LorebookDetails, LorebookRepositoryError> {
    validate_entries_for_book(details)?;
    verify_icon(tx, details.book.icon_asset_id)?;
    if load_details(tx, details.book.id)
        .map_err(db_error)?
        .is_some()
    {
        return Err(LorebookRepositoryError::Conflict);
    }
    tx.execute(
        "INSERT INTO lorebooks (id,status,name,detection_policy,icon_asset_id,icon_blob_kind,behavior_version,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,'image',?6,?7,?8,?9)",
        params![
            details.book.id.to_string(),
            status_name(details.book.status),
            details.book.name,
            detection_policy_name(details.book.detection_policy),
            details.book.icon_asset_id.map(|id| id.to_string()),
            behavior_name(details.book.behavior_version),
            sql_revision(details.book.revision)?,
            details.book.created_at.get(),
            details.book.updated_at.get(),
        ],
    )
    .map_err(db_error)?;
    for entry in &details.entries {
        insert_entry(tx, entry)?;
    }
    load_required(tx, details.book.id)
}

impl LorebookRepository for Database {
    fn create(
        &self,
        metadata: LorebookMetadataDraft,
        entries: Vec<LorebookEntryDraft>,
        now: TimestampMillis,
    ) -> Result<LorebookDetails, LorebookRepositoryError> {
        metadata.validate()?;
        let probe = LorebookDetails {
            book: Lorebook {
                id: LorebookId::new(),
                status: LifecycleStatus::Active,
                name: metadata.name.clone(),
                detection_policy: metadata.detection_policy,
                icon_asset_id: metadata.icon_asset_id,
                behavior_version: metadata.behavior_version,
                revision: Revision::INITIAL,
                created_at: now,
                updated_at: now,
            },
            entries: Vec::new(),
        };
        let mut child_entries = Vec::with_capacity(entries.len());
        for (ordinal, draft) in entries.into_iter().enumerate() {
            child_entries.push(initial_entry(probe.book.id, draft, ordinal, now)?);
        }
        let mut details = probe;
        details.entries = child_entries;
        let mut connection = self
            .connection()
            .map_err(|_| failure("database lock unavailable"))?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let loaded = insert_lorebook_details(&tx, &details)?;
        tx.commit().map_err(db_error)?;
        Ok(loaded)
    }

    fn get(&self, id: LorebookId) -> Result<Option<LorebookDetails>, LorebookRepositoryError> {
        let mut connection = self
            .connection()
            .map_err(|_| failure("database lock unavailable"))?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(db_error)?;
        let result = load_details(&tx, id).map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }

    fn page(&self, query: LorebookLibraryQuery) -> Result<Page<Lorebook>, LorebookRepositoryError> {
        let cursor = cursor_decode(query.page.cursor.as_deref())?;
        let limit = i64::from(query.page.limit.get());
        let mut connection = self
            .connection()
            .map_err(|_| failure("database lock unavailable"))?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(db_error)?;
        let status_clause = match query.status {
            LifecycleFilter::All => "1=1",
            LifecycleFilter::Active => "status='active'",
            LifecycleFilter::Archived => "status='archived'",
        };
        let mut sql = format!("SELECT id,updated_at FROM lorebooks WHERE {status_clause} ");
        if cursor.is_some() {
            sql.push_str("AND (updated_at < ?1 OR (updated_at = ?1 AND id > ?2)) ");
        }
        sql.push_str("ORDER BY updated_at DESC,id ASC LIMIT ?3");
        let mut statement = tx.prepare(&sql).map_err(db_error)?;
        let mut rows = if let Some((updated, id)) = cursor {
            statement
                .query(params![updated, id.to_string(), limit + 1])
                .map_err(db_error)?
        } else {
            statement
                .query(params![0_i64, "", limit + 1])
                .map_err(db_error)?
        };
        let mut keys = Vec::new();
        while let Some(row) = rows.next().map_err(db_error)? {
            keys.push((
                parse_id::<LorebookId>(row.get(0).map_err(db_error)?).map_err(db_error)?,
                TimestampMillis::new(row.get(1).map_err(db_error)?),
            ));
        }
        drop(rows);
        drop(statement);
        let page_limit = usize::try_from(limit).map_err(|_| failure("page limit overflow"))?;
        let has_next = keys.len() > page_limit;
        keys.truncate(page_limit);
        let mut items = Vec::with_capacity(keys.len());
        for (id, _) in &keys {
            items.push(load_required(&tx, *id)?.book);
        }
        let next_cursor = if has_next {
            keys.last()
                .map(|(id, updated)| cursor_encode(*updated, *id))
                .transpose()?
        } else {
            None
        };
        tx.commit().map_err(db_error)?;
        Ok(Page { items, next_cursor })
    }

    fn revise_metadata(
        &self,
        id: LorebookId,
        expected_revision: Revision,
        metadata: LorebookMetadataDraft,
        now: TimestampMillis,
    ) -> Result<LorebookMutationResult, LorebookRepositoryError> {
        metadata.validate()?;
        let mut connection = self
            .connection()
            .map_err(|_| failure("database lock unavailable"))?;
        verify_icon(&connection, metadata.icon_asset_id)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let current = load_required(&tx, id)?;
        if current.book.revision != expected_revision {
            return Err(LorebookRepositoryError::Conflict);
        }
        let proposed_book = Lorebook {
            id,
            status: current.book.status,
            name: metadata.name,
            detection_policy: metadata.detection_policy,
            icon_asset_id: metadata.icon_asset_id,
            behavior_version: metadata.behavior_version,
            revision: bump_revision(current.book.revision)?,
            created_at: current.book.created_at,
            updated_at: now,
        };
        let proposed = LorebookDetails {
            book: proposed_book,
            entries: current.entries,
        };
        validate_entries_for_book(&proposed)?;
        write_book_head(&tx, &proposed.book, expected_revision)?;
        let loaded = load_required(&tx, id)?;
        tx.commit().map_err(db_error)?;
        Ok(LorebookMutationResult {
            book_revision: loaded.book.revision,
            details: loaded,
        })
    }

    fn mutate_entries(
        &self,
        id: LorebookId,
        expected_revision: Revision,
        mutation: LorebookEntryMutation,
        now: TimestampMillis,
    ) -> Result<LorebookMutationResult, LorebookRepositoryError> {
        mutation.validate()?;
        let mut connection = self
            .connection()
            .map_err(|_| failure("database lock unavailable"))?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let current = load_required(&tx, id)?;
        if current.book.revision != expected_revision {
            return Err(LorebookRepositoryError::Conflict);
        }
        let mut entries = current.entries;
        let previous_entries = entries.clone();
        match mutation {
            LorebookEntryMutation::Add { draft, target } => {
                let index = match target {
                    LorebookEntryInsertionTarget::Append => entries.len(),
                    LorebookEntryInsertionTarget::At(index) => index,
                };
                if index > entries.len() {
                    return Err(LorebookRepositoryError::Invalid(
                        LorebookValidationError::InvalidTarget,
                    ));
                }
                entries.insert(index, initial_entry(id, draft, index, now)?);
            }
            LorebookEntryMutation::Update { entry_id, draft } => {
                let Some(index) = entries.iter().position(|entry| entry.id == entry_id) else {
                    return Err(LorebookRepositoryError::EntryNotFound);
                };
                entries[index] = updated_entry(&entries[index], draft, now)?;
            }
            LorebookEntryMutation::Remove { entry_id } => {
                let Some(index) = entries.iter().position(|entry| entry.id == entry_id) else {
                    return Err(LorebookRepositoryError::EntryNotFound);
                };
                entries.remove(index);
            }
            LorebookEntryMutation::Replace { drafts } => {
                entries = drafts
                    .into_iter()
                    .enumerate()
                    .map(|(index, draft)| initial_entry(id, draft, index, now))
                    .collect::<Result<Vec<_>, _>>()?;
            }
            LorebookEntryMutation::Reorder {
                entry_id,
                target_index,
            } => {
                if target_index >= entries.len() {
                    return Err(LorebookRepositoryError::Invalid(
                        LorebookValidationError::InvalidTarget,
                    ));
                }
                let Some(index) = entries.iter().position(|entry| entry.id == entry_id) else {
                    return Err(LorebookRepositoryError::EntryNotFound);
                };
                let item = entries.remove(index);
                entries.insert(target_index, item);
            }
        }
        normalize_entry_metadata(&mut entries, &previous_entries, now)?;
        let book = Lorebook {
            revision: bump_revision(current.book.revision)?,
            updated_at: now,
            ..current.book
        };
        let proposed = LorebookDetails { book, entries };
        validate_entries_for_book(&proposed)?;
        write_book_head(&tx, &proposed.book, expected_revision)?;
        tx.execute(
            "DELETE FROM lorebook_entries WHERE lorebook_id=?1",
            [id.to_string()],
        )
        .map_err(db_error)?;
        for entry in &proposed.entries {
            insert_entry(&tx, entry)?;
        }
        let loaded = load_required(&tx, id)?;
        tx.commit().map_err(db_error)?;
        Ok(LorebookMutationResult {
            book_revision: loaded.book.revision,
            details: loaded,
        })
    }

    fn archive(
        &self,
        id: LorebookId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<LorebookMutationResult, LorebookRepositoryError> {
        self.set_lorebook_status(id, expected_revision, LifecycleStatus::Archived, now)
    }

    fn restore(
        &self,
        id: LorebookId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<LorebookMutationResult, LorebookRepositoryError> {
        self.set_lorebook_status(id, expected_revision, LifecycleStatus::Active, now)
    }
}

impl Database {
    fn set_lorebook_status(
        &self,
        id: LorebookId,
        expected_revision: Revision,
        status: LifecycleStatus,
        now: TimestampMillis,
    ) -> Result<LorebookMutationResult, LorebookRepositoryError> {
        let mut connection = self
            .connection()
            .map_err(|_| failure("database lock unavailable"))?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let current = load_required(&tx, id)?;
        if current.book.revision != expected_revision {
            return Err(LorebookRepositoryError::Conflict);
        }
        let book = Lorebook {
            status,
            revision: bump_revision(current.book.revision)?,
            updated_at: now,
            ..current.book
        };
        let proposed = LorebookDetails {
            book,
            entries: current.entries,
        };
        validate_entries_for_book(&proposed)?;
        write_book_head(&tx, &proposed.book, expected_revision)?;
        let loaded = load_required(&tx, id)?;
        tx.commit().map_err(db_error)?;
        Ok(LorebookMutationResult {
            book_revision: loaded.book.revision,
            details: loaded,
        })
    }
}

#[derive(Clone, Copy)]
enum OwnerKind {
    Character,
    Persona,
    Group,
}

impl OwnerKind {
    fn table(self) -> &'static str {
        match self {
            Self::Character => "characters",
            Self::Persona => "personas",
            Self::Group => "groups",
        }
    }
    fn binding_table(self) -> &'static str {
        match self {
            Self::Character => "character_lorebook_bindings",
            Self::Persona => "persona_lorebook_bindings",
            Self::Group => "group_lorebook_bindings",
        }
    }
    fn owner_column(self) -> &'static str {
        match self {
            Self::Character => "character_id",
            Self::Persona => "persona_id",
            Self::Group => "group_id",
        }
    }
}

fn owner_revision(
    tx: &Transaction<'_>,
    kind: OwnerKind,
    id: &str,
) -> Result<Revision, BindingRepositoryError> {
    let sql = format!("SELECT revision FROM {} WHERE id=?1", kind.table());
    tx.query_row(&sql, [id], |row| row.get::<_, i64>(0))
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => BindingRepositoryError::NotFound,
            other => binding_db_error(other),
        })
        .and_then(|value| parse_revision(value).map_err(binding_db_error))
}

fn owner_bump(
    tx: &Transaction<'_>,
    kind: OwnerKind,
    id: &str,
    expected: Revision,
    now: TimestampMillis,
) -> Result<Revision, BindingRepositoryError> {
    let next = expected
        .next()
        .map_err(|_| binding_failure("owner revision overflow"))?;
    let sql = format!(
        "UPDATE {} SET revision=?1,updated_at=?2 WHERE id=?3 AND revision=?4",
        kind.table()
    );
    let changed = tx
        .execute(
            &sql,
            params![
                i64::try_from(next.get())
                    .map_err(|_| binding_failure("owner revision overflow"))?,
                now.get(),
                id,
                i64::try_from(expected.get())
                    .map_err(|_| binding_failure("owner revision overflow"))?
            ],
        )
        .map_err(binding_db_error)?;
    if changed != 1 {
        return Err(BindingRepositoryError::Conflict);
    }
    Ok(next)
}

fn parse_binding(row: &Row<'_>) -> rusqlite::Result<LorebookBinding> {
    let enabled = bool_value(row.get(2)?)?;
    Ok(LorebookBinding {
        lorebook_id: parse_id(row.get::<_, String>(1)?)?,
        enabled,
        ordinal: parse_ordinal(row.get(3)?)?,
        revision: parse_revision(row.get(4)?)?,
        created_at: TimestampMillis::new(row.get(5)?),
        updated_at: TimestampMillis::new(row.get(6)?),
    })
}

fn read_bindings(
    tx: &Transaction<'_>,
    kind: OwnerKind,
    owner: &str,
) -> Result<Vec<LorebookBinding>, BindingRepositoryError> {
    let sql = format!(
        "SELECT {},lorebook_id,enabled,ordinal,revision,created_at,updated_at FROM {} WHERE {}=?1 ORDER BY ordinal,lorebook_id",
        kind.owner_column(),
        kind.binding_table(),
        kind.owner_column()
    );
    let mut statement = tx.prepare(&sql).map_err(binding_db_error)?;
    let mut rows = statement.query([owner]).map_err(binding_db_error)?;
    let mut values = Vec::new();
    while let Some(row) = rows.next().map_err(binding_db_error)? {
        values.push(parse_binding(row).map_err(binding_db_error)?);
    }
    lettuce_context::validate_bindings(&values)
        .map_err(|_| binding_failure("corrupt binding ordering"))?;
    Ok(values)
}

fn normalize_binding_metadata(
    bindings: &mut [LorebookBinding],
    previous: &[LorebookBinding],
    now: TimestampMillis,
) -> Result<(), BindingRepositoryError> {
    let previous_by_id: HashMap<_, _> = previous
        .iter()
        .map(|binding| (binding.lorebook_id, binding))
        .collect();
    for (ordinal, binding) in bindings.iter_mut().enumerate() {
        binding.ordinal =
            u32::try_from(ordinal).map_err(|_| binding_failure("binding ordinal overflow"))?;
        let Some(old) = previous_by_id.get(&binding.lorebook_id) else {
            continue;
        };
        if old.ordinal != binding.ordinal {
            binding.revision = old
                .revision
                .next()
                .map_err(|_| binding_failure("binding revision overflow"))?;
            binding.created_at = old.created_at;
            binding.updated_at = now;
        }
    }
    Ok(())
}

fn binding_mutation(
    tx: &Transaction<'_>,
    kind: OwnerKind,
    owner: &str,
    expected: Revision,
    operation: BindingOperation,
    now: TimestampMillis,
) -> Result<BindingMutationResult, BindingRepositoryError> {
    let actual = owner_revision(tx, kind, owner)?;
    if actual != expected {
        return Err(BindingRepositoryError::Conflict);
    }
    let mut bindings = read_bindings(tx, kind, owner)?;
    let previous_bindings = bindings.clone();
    match operation {
        BindingOperation::Bind(create) => {
            if bindings
                .iter()
                .any(|binding| binding.lorebook_id == create.lorebook_id)
            {
                return Err(BindingRepositoryError::Conflict);
            }
            let exists_active = tx
                .query_row(
                    "SELECT status FROM lorebooks WHERE id=?1",
                    [create.lorebook_id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(binding_db_error)?;
            if exists_active.as_deref() != Some("active") {
                return Err(BindingRepositoryError::NotFound);
            }
            let index = match create.target {
                BindingInsertionTarget::Append => bindings.len(),
                BindingInsertionTarget::At(index) => index,
            };
            if index > bindings.len() {
                return Err(BindingRepositoryError::Invalid(
                    lettuce_context::BindingValidationError::InvalidTarget,
                ));
            }
            bindings.insert(
                index,
                LorebookBinding {
                    lorebook_id: create.lorebook_id,
                    enabled: true,
                    ordinal: 0,
                    revision: Revision::INITIAL,
                    created_at: now,
                    updated_at: now,
                },
            );
        }
        BindingOperation::Unbind(id) => {
            let Some(index) = bindings
                .iter()
                .position(|binding| binding.lorebook_id == id)
            else {
                return Err(BindingRepositoryError::NotFound);
            };
            bindings.remove(index);
        }
        BindingOperation::Enable(id, enabled) => {
            let Some(binding) = bindings
                .iter_mut()
                .find(|binding| binding.lorebook_id == id)
            else {
                return Err(BindingRepositoryError::NotFound);
            };
            if binding.enabled != enabled {
                binding.enabled = enabled;
                binding.revision = binding
                    .revision
                    .next()
                    .map_err(|_| binding_failure("binding revision overflow"))?;
                binding.updated_at = now;
            }
        }
        BindingOperation::Reorder(id, target) => {
            if target >= bindings.len() {
                return Err(BindingRepositoryError::Invalid(
                    lettuce_context::BindingValidationError::InvalidTarget,
                ));
            }
            let Some(index) = bindings
                .iter()
                .position(|binding| binding.lorebook_id == id)
            else {
                return Err(BindingRepositoryError::NotFound);
            };
            let binding = bindings.remove(index);
            bindings.insert(target, binding);
        }
    }
    normalize_binding_metadata(&mut bindings, &previous_bindings, now)?;
    lettuce_context::validate_bindings(&bindings).map_err(BindingRepositoryError::Invalid)?;
    let table = kind.binding_table();
    let column = kind.owner_column();
    let delete_sql = format!("DELETE FROM {table} WHERE {column}=?1");
    tx.execute(&delete_sql, [owner]).map_err(binding_db_error)?;
    let insert_sql = format!(
        "INSERT INTO {table} ({column},lorebook_id,enabled,ordinal,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7)"
    );
    for binding in &bindings {
        tx.execute(
            &insert_sql,
            params![
                owner,
                binding.lorebook_id.to_string(),
                i64::from(binding.enabled),
                i64::from(binding.ordinal),
                i64::try_from(binding.revision.get())
                    .map_err(|_| binding_failure("binding revision overflow"))?,
                binding.created_at.get(),
                binding.updated_at.get()
            ],
        )
        .map_err(binding_db_error)?;
    }
    let reloaded = read_bindings(tx, kind, owner)?;
    if reloaded != bindings {
        return Err(binding_failure("binding write did not round-trip"));
    }
    let owner_revision = owner_bump(tx, kind, owner, expected, now)?;
    Ok(BindingMutationResult {
        bindings: reloaded,
        owner_revision,
    })
}

enum BindingOperation {
    Bind(LorebookBindingCreate),
    Unbind(LorebookId),
    Enable(LorebookId, bool),
    Reorder(LorebookId, usize),
}

fn list_bindings(
    database: &Database,
    kind: OwnerKind,
    owner: String,
) -> Result<Vec<LorebookBinding>, BindingRepositoryError> {
    let mut connection = database
        .connection()
        .map_err(|_| binding_failure("database lock unavailable"))?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(binding_db_error)?;
    owner_revision(&tx, kind, &owner)?;
    let result = read_bindings(&tx, kind, &owner)?;
    tx.commit().map_err(binding_db_error)?;
    Ok(result)
}

fn mutate_bindings(
    database: &Database,
    kind: OwnerKind,
    owner: String,
    expected: Revision,
    operation: BindingOperation,
    now: TimestampMillis,
) -> Result<BindingMutationResult, BindingRepositoryError> {
    let mut connection = database
        .connection()
        .map_err(|_| binding_failure("database lock unavailable"))?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(binding_db_error)?;
    let result = binding_mutation(&tx, kind, &owner, expected, operation, now)?;
    tx.commit().map_err(binding_db_error)?;
    Ok(result)
}

macro_rules! impl_binding_repository {
    ($trait:path, $kind:expr, $owner:ty, $list:ident, $bind:ident, $unbind:ident, $enable:ident, $reorder:ident) => {
        impl $trait for Database {
            fn $list(&self, owner: $owner) -> Result<Vec<LorebookBinding>, BindingRepositoryError> {
                list_bindings(self, $kind, owner.to_string())
            }
            fn $bind(
                &self,
                owner: $owner,
                expected: Revision,
                binding: LorebookBindingCreate,
                now: TimestampMillis,
            ) -> Result<BindingMutationResult, BindingRepositoryError> {
                mutate_bindings(
                    self,
                    $kind,
                    owner.to_string(),
                    expected,
                    BindingOperation::Bind(binding),
                    now,
                )
            }
            fn $unbind(
                &self,
                owner: $owner,
                expected: Revision,
                lorebook_id: LorebookId,
                now: TimestampMillis,
            ) -> Result<BindingMutationResult, BindingRepositoryError> {
                mutate_bindings(
                    self,
                    $kind,
                    owner.to_string(),
                    expected,
                    BindingOperation::Unbind(lorebook_id),
                    now,
                )
            }
            fn $enable(
                &self,
                owner: $owner,
                expected: Revision,
                lorebook_id: LorebookId,
                enabled: bool,
                now: TimestampMillis,
            ) -> Result<BindingMutationResult, BindingRepositoryError> {
                mutate_bindings(
                    self,
                    $kind,
                    owner.to_string(),
                    expected,
                    BindingOperation::Enable(lorebook_id, enabled),
                    now,
                )
            }
            fn $reorder(
                &self,
                owner: $owner,
                expected: Revision,
                lorebook_id: LorebookId,
                target_ordinal: usize,
                now: TimestampMillis,
            ) -> Result<BindingMutationResult, BindingRepositoryError> {
                mutate_bindings(
                    self,
                    $kind,
                    owner.to_string(),
                    expected,
                    BindingOperation::Reorder(lorebook_id, target_ordinal),
                    now,
                )
            }
        }
    };
}

impl_binding_repository!(
    CharacterLorebookBindingRepository,
    OwnerKind::Character,
    CharacterId,
    list_character_bindings,
    bind_character_lorebook,
    unbind_character_lorebook,
    set_character_lorebook_enabled,
    reorder_character_lorebook
);
impl_binding_repository!(
    PersonaLorebookBindingRepository,
    OwnerKind::Persona,
    PersonaId,
    list_persona_bindings,
    bind_persona_lorebook,
    unbind_persona_lorebook,
    set_persona_lorebook_enabled,
    reorder_persona_lorebook
);
impl_binding_repository!(
    GroupLorebookBindingRepository,
    OwnerKind::Group,
    GroupId,
    list_group_bindings,
    bind_group_lorebook,
    unbind_group_lorebook,
    set_group_lorebook_enabled,
    reorder_group_lorebook
);

impl LorebookDependencyReader for Database {
    fn references_to(
        &self,
        lorebook_id: LorebookId,
    ) -> Result<Vec<LorebookReference>, LorebookDependencyError> {
        let mut connection = self
            .connection()
            .map_err(|_| LorebookDependencyError::Failure("database lock unavailable".into()))?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(|error| LorebookDependencyError::Failure(error.to_string()))?;
        let mut references = Vec::new();
        for (table, column, map) in [
            ("character_lorebook_bindings", "character_id", 0_u8),
            ("persona_lorebook_bindings", "persona_id", 1_u8),
            ("group_lorebook_bindings", "group_id", 2_u8),
        ] {
            let sql =
                format!("SELECT {column} FROM {table} WHERE lorebook_id=?1 ORDER BY {column}");
            let mut statement = tx
                .prepare(&sql)
                .map_err(|error| LorebookDependencyError::Failure(error.to_string()))?;
            let mut rows = statement
                .query([lorebook_id.to_string()])
                .map_err(|error| LorebookDependencyError::Failure(error.to_string()))?;
            while let Some(row) = rows
                .next()
                .map_err(|error| LorebookDependencyError::Failure(error.to_string()))?
            {
                let owner = row
                    .get::<_, String>(0)
                    .map_err(|error| LorebookDependencyError::Failure(error.to_string()))?;
                match map {
                    0 => references.push(LorebookReference::Character {
                        id: parse_id(owner)
                            .map_err(|error| LorebookDependencyError::Failure(error.to_string()))?,
                    }),
                    1 => references.push(LorebookReference::Persona {
                        id: parse_id(owner)
                            .map_err(|error| LorebookDependencyError::Failure(error.to_string()))?,
                    }),
                    _ => references.push(LorebookReference::Group {
                        id: parse_id(owner)
                            .map_err(|error| LorebookDependencyError::Failure(error.to_string()))?,
                    }),
                }
            }
        }
        let mut statement = tx.prepare("SELECT character_id,id,lorebooks_json FROM conversation_starters ORDER BY character_id,id").map_err(|error| LorebookDependencyError::Failure(error.to_string()))?;
        let mut rows = statement
            .query([])
            .map_err(|error| LorebookDependencyError::Failure(error.to_string()))?;
        while let Some(row) = rows
            .next()
            .map_err(|error| LorebookDependencyError::Failure(error.to_string()))?
        {
            let character_id: CharacterId = parse_id(
                row.get(0)
                    .map_err(|error| LorebookDependencyError::Failure(error.to_string()))?,
            )
            .map_err(|error| LorebookDependencyError::Failure(error.to_string()))?;
            let starter_id: lettuce_types::ConversationStarterId = parse_id(
                row.get(1)
                    .map_err(|error| LorebookDependencyError::Failure(error.to_string()))?,
            )
            .map_err(|error| LorebookDependencyError::Failure(error.to_string()))?;
            let envelope: Envelope<Selection<Vec<LorebookId>>> = serde_json::from_str(
                &row.get::<_, String>(2)
                    .map_err(|error| LorebookDependencyError::Failure(error.to_string()))?,
            )
            .map_err(|error| LorebookDependencyError::Failure(error.to_string()))?;
            if envelope.format_version != JSON_VERSION {
                return Err(LorebookDependencyError::Failure(
                    "invalid starter lorebook payload version".into(),
                ));
            }
            if let Selection::Explicit(ids) = envelope.value {
                if ids.contains(&lorebook_id) {
                    references.push(LorebookReference::Starter {
                        character_id,
                        starter_id,
                    });
                }
            }
        }
        drop(rows);
        drop(statement);
        tx.commit()
            .map_err(|error| LorebookDependencyError::Failure(error.to_string()))?;
        Ok(references)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_context::{
        CharacterLorebookBindingRepository, GroupLorebookBindingRepository,
        LorebookEntryInsertionTarget, LorebookEntryMutation, LorebookRepository,
        PersonaLorebookBindingRepository,
    };
    use lettuce_media::{
        AssetKind, AssetOrigin, AssetProvenanceV1, BlobState, MediaAsset, MediaAssetRepository,
        MediaBlob, MediaBlobRepository, MediaKind, RetentionClass,
    };
    use lettuce_types::{ContentHash, ConversationStarterId, MediaBlobId};

    fn metadata() -> LorebookMetadataDraft {
        LorebookMetadataDraft {
            name: "World notes".into(),
            detection_policy: DetectionPolicy::RecentMessageWindow,
            icon_asset_id: None,
            behavior_version: LorebookBehaviorVersion::LegacyV1,
        }
    }

    fn draft(title: &str, content: &str) -> LorebookEntryDraft {
        LorebookEntryDraft {
            title: title.into(),
            enabled: true,
            always_active: false,
            keywords: vec![title.into()],
            case_sensitive: false,
            match_mode: KeywordMatchMode::Literal,
            content: content.into(),
            priority: 0,
        }
    }

    fn insert_character(database: &Database, id: CharacterId) {
        let connection = database.connection().expect("database lock");
        connection
            .execute(
                "INSERT INTO characters (id,status,name,nickname,normalized_name,normalized_nickname,profile_json,provenance_json,defaults_json,interaction_mode,memory_policy,model_profile_id,default_scene_id,default_starter_id,direct_prompt_id,group_conversation_prompt_id,group_roleplay_prompt_id,voice_profile_id,voice_legacy_locator,voice_autoplay,presentation_json,image_recommendation_json,revision,created_at,updated_at) VALUES (?1,'active','Character',NULL,'character',NULL,'{}','{}','{}','roleplay','manual',NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,0,'{}',NULL,1,1,1)",
                [id.to_string()],
            )
            .expect("character");
    }

    fn insert_persona(database: &Database, id: PersonaId) {
        let connection = database.connection().expect("database lock");
        connection
            .execute(
                "INSERT INTO personas (id,status,title,normalized_title,nickname,normalized_nickname,description,design_description,avatar_crop_json,image_recommendation_json,revision,created_at,updated_at) VALUES (?1,'active','Persona','persona',NULL,NULL,'Description',NULL,NULL,NULL,1,1,1)",
                [id.to_string()],
            )
            .expect("persona");
    }

    fn insert_group(database: &Database, id: GroupId) {
        let connection = database.connection().expect("database lock");
        connection
            .execute(
                "INSERT INTO groups (id,status,name,normalized_name,chat_mode,persona_selection_kind,persona_id,speaker_selection,memory_policy,disable_character_lorebooks,group_conversation_prompt_id,group_roleplay_prompt_id,presentation_json,background_asset_id,background_blob_kind,starting_scene_id,revision,created_at,updated_at) VALUES (?1,'active','Group','group','conversation','inherit',NULL,'llm','manual',0,NULL,NULL,'{}',NULL,'image',NULL,1,1,1)",
                [id.to_string()],
            )
            .expect("group");
    }

    fn insert_starter(
        database: &Database,
        character_id: CharacterId,
        starter_id: ConversationStarterId,
        book_id: LorebookId,
    ) {
        let lorebooks = serde_json::to_string(&Envelope {
            format_version: JSON_VERSION,
            value: Selection::Explicit(vec![book_id]),
        })
        .expect("starter lorebooks");
        let connection = database.connection().expect("database lock");
        connection
            .execute(
                "INSERT INTO conversation_starters (character_id,id,name,ordinal,scene_id,prompt_id,lorebooks_json,revision,created_at,updated_at) VALUES (?1,?2,'Starter',0,NULL,NULL,?3,1,1,1)",
                rusqlite::params![character_id.to_string(), starter_id.to_string(), lorebooks],
            )
            .expect("starter");
    }

    fn image_asset(database: &Database, byte: char) -> lettuce_types::AssetId {
        let blob = MediaBlob {
            id: MediaBlobId::new(),
            content_hash: ContentHash::parse(format!("{:02x}", byte as u8).repeat(32))
                .expect("hash"),
            kind: MediaKind::Image,
            mime_type: "image/png".into(),
            byte_size: 1,
            width: Some(1),
            height: Some(1),
            duration_ms: None,
            validation_version: 1,
            state: BlobState::Staged,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        };
        let blob = MediaBlobRepository::register(database, blob).expect("blob");
        let blob = MediaBlobRepository::finalize_staged_to_ready(
            database,
            blob.id,
            TimestampMillis::new(1),
        )
        .expect("ready blob");
        let asset = MediaAsset::new(
            lettuce_types::AssetId::new(),
            blob.id,
            AssetKind::Illustration,
            AssetOrigin::Upload,
            RetentionClass::Library,
            AssetProvenanceV1::default(),
            Revision::INITIAL,
            TimestampMillis::new(1),
            TimestampMillis::new(1),
        )
        .expect("asset");
        MediaAssetRepository::create(database, asset)
            .expect("asset create")
            .id
    }

    fn audio_asset(database: &Database, byte: char) -> lettuce_types::AssetId {
        let blob = MediaBlob {
            id: MediaBlobId::new(),
            content_hash: ContentHash::parse(format!("{:02x}", byte as u8).repeat(32))
                .expect("hash"),
            kind: MediaKind::Audio,
            mime_type: "audio/mpeg".into(),
            byte_size: 1,
            width: None,
            height: None,
            duration_ms: Some(1),
            validation_version: 1,
            state: BlobState::Staged,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        };
        let blob = MediaBlobRepository::register(database, blob).expect("blob");
        let blob = MediaBlobRepository::finalize_staged_to_ready(
            database,
            blob.id,
            TimestampMillis::new(1),
        )
        .expect("ready blob");
        let asset = MediaAsset::new(
            lettuce_types::AssetId::new(),
            blob.id,
            AssetKind::OtherAudio,
            AssetOrigin::Upload,
            RetentionClass::Library,
            AssetProvenanceV1::default(),
            Revision::INITIAL,
            TimestampMillis::new(1),
            TimestampMillis::new(1),
        )
        .expect("asset");
        MediaAssetRepository::create(database, asset)
            .expect("asset create")
            .id
    }

    #[test]
    fn aggregate_round_trip_and_all_entry_mutations_are_atomic() {
        let database = Database::open_in_memory().expect("database");
        let created = LorebookRepository::create(
            &database,
            metadata(),
            vec![draft("Rain", "Rain notes")],
            TimestampMillis::new(1),
        )
        .expect("create");
        assert_eq!(created.book.revision, Revision::INITIAL);
        assert_eq!(created.entries[0].ordinal, 0);
        let added = LorebookRepository::mutate_entries(
            &database,
            created.book.id,
            created.book.revision,
            LorebookEntryMutation::Add {
                draft: draft("Wind", "Wind notes"),
                target: LorebookEntryInsertionTarget::At(0),
            },
            TimestampMillis::new(2),
        )
        .expect("add");
        assert_eq!(added.details.entries[0].title, "Wind");
        assert_eq!(added.details.entries[1].ordinal, 1);
        let updated = LorebookRepository::mutate_entries(
            &database,
            created.book.id,
            added.book_revision,
            LorebookEntryMutation::Update {
                entry_id: added.details.entries[0].id,
                draft: draft("Gale", "Gale notes"),
            },
            TimestampMillis::new(3),
        )
        .expect("update");
        assert_eq!(updated.details.entries[0].title, "Gale");
        assert_eq!(updated.details.entries[0].revision, Revision::new(2));
        let reordered = LorebookRepository::mutate_entries(
            &database,
            created.book.id,
            updated.book_revision,
            LorebookEntryMutation::Reorder {
                entry_id: updated.details.entries[0].id,
                target_index: 1,
            },
            TimestampMillis::new(4),
        )
        .expect("reorder");
        assert_eq!(reordered.details.entries[1].title, "Gale");
        let removed = LorebookRepository::mutate_entries(
            &database,
            created.book.id,
            reordered.book_revision,
            LorebookEntryMutation::Remove {
                entry_id: reordered.details.entries[0].id,
            },
            TimestampMillis::new(5),
        )
        .expect("remove");
        assert_eq!(removed.details.entries.len(), 1);
        let archived = LorebookRepository::archive(
            &database,
            created.book.id,
            removed.book_revision,
            TimestampMillis::new(6),
        )
        .expect("archive");
        assert_eq!(archived.details.book.status, LifecycleStatus::Archived);
        let restored = LorebookRepository::restore(
            &database,
            created.book.id,
            archived.book_revision,
            TimestampMillis::new(7),
        )
        .expect("restore");
        assert_eq!(restored.details.book.status, LifecycleStatus::Active);
        assert_eq!(
            LorebookRepository::get(&database, created.book.id).expect("get"),
            Some(restored.details)
        );
    }

    #[test]
    fn entry_position_changes_bump_only_entries_whose_ordinals_changed() {
        let database = Database::open_in_memory().expect("database");
        let created = LorebookRepository::create(
            &database,
            metadata(),
            vec![draft("A", "a"), draft("B", "b"), draft("C", "c")],
            TimestampMillis::new(1),
        )
        .expect("create");
        let ids = created
            .entries
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>();
        let added = LorebookRepository::mutate_entries(
            &database,
            created.book.id,
            created.book.revision,
            LorebookEntryMutation::Add {
                draft: draft("X", "x"),
                target: LorebookEntryInsertionTarget::At(1),
            },
            TimestampMillis::new(2),
        )
        .expect("add middle");
        let by_id = |id| {
            added
                .details
                .entries
                .iter()
                .find(|entry| entry.id == id)
                .expect("entry exists")
        };
        assert_eq!(by_id(ids[0]).revision, Revision::INITIAL);
        assert_eq!(by_id(ids[0]).updated_at, TimestampMillis::new(1));
        assert_eq!(by_id(ids[1]).revision, Revision::new(2));
        assert_eq!(by_id(ids[1]).updated_at, TimestampMillis::new(2));
        assert_eq!(by_id(ids[2]).revision, Revision::new(2));
        assert_eq!(by_id(ids[2]).updated_at, TimestampMillis::new(2));
        let removed = LorebookRepository::mutate_entries(
            &database,
            created.book.id,
            added.book_revision,
            LorebookEntryMutation::Remove {
                entry_id: added.details.entries[1].id,
            },
            TimestampMillis::new(3),
        )
        .expect("remove middle");
        let by_id = |id| {
            removed
                .details
                .entries
                .iter()
                .find(|entry| entry.id == id)
                .expect("entry exists")
        };
        assert_eq!(by_id(ids[0]).revision, Revision::INITIAL);
        assert_eq!(by_id(ids[1]).revision, Revision::new(3));
        assert_eq!(by_id(ids[1]).updated_at, TimestampMillis::new(3));
        assert_eq!(by_id(ids[2]).revision, Revision::new(3));
        let reordered = LorebookRepository::mutate_entries(
            &database,
            created.book.id,
            removed.book_revision,
            LorebookEntryMutation::Reorder {
                entry_id: ids[2],
                target_index: 0,
            },
            TimestampMillis::new(4),
        )
        .expect("reorder");
        let by_id = |id| {
            reordered
                .details
                .entries
                .iter()
                .find(|entry| entry.id == id)
                .expect("entry exists")
        };
        assert_eq!(by_id(ids[2]).revision, Revision::new(4));
        assert_eq!(by_id(ids[1]).revision, Revision::new(4));
        assert_eq!(by_id(ids[0]).revision, Revision::new(2));
        let replaced = LorebookRepository::mutate_entries(
            &database,
            created.book.id,
            reordered.book_revision,
            LorebookEntryMutation::Replace {
                drafts: vec![draft("N", "n")],
            },
            TimestampMillis::new(5),
        )
        .expect("replace");
        assert_eq!(replaced.details.entries[0].revision, Revision::INITIAL);
        assert_eq!(
            replaced.details.entries[0].created_at,
            TimestampMillis::new(5)
        );
    }

    #[test]
    fn page_uses_status_and_keyset_cursor_and_bad_cursor_is_rejected() {
        let database = Database::open_in_memory().expect("database");
        let first =
            LorebookRepository::create(&database, metadata(), Vec::new(), TimestampMillis::new(1))
                .expect("first");
        let second = LorebookRepository::create(
            &database,
            LorebookMetadataDraft {
                name: "Second".into(),
                ..metadata()
            },
            Vec::new(),
            TimestampMillis::new(1),
        )
        .expect("second");
        let page = LorebookRepository::page(
            &database,
            LorebookLibraryQuery {
                page: lettuce_types::PageRequest {
                    cursor: None,
                    limit: lettuce_types::PageLimit::new(1),
                },
                status: LifecycleFilter::Active,
            },
        )
        .expect("page");
        assert_eq!(page.items.len(), 1);
        assert!(page.next_cursor.is_some());
        let next = LorebookRepository::page(
            &database,
            LorebookLibraryQuery {
                page: lettuce_types::PageRequest {
                    cursor: page.next_cursor,
                    limit: lettuce_types::PageLimit::new(1),
                },
                status: LifecycleFilter::Active,
            },
        )
        .expect("next page");
        assert_eq!(next.items.len(), 1);
        assert_ne!(next.items[0].id, page.items[0].id);
        assert_eq!(vec![page.items[0].id, next.items[0].id], {
            let mut ids = vec![first.book.id, second.book.id];
            ids.sort();
            ids
        });
        let bad = LorebookRepository::page(
            &database,
            LorebookLibraryQuery {
                page: lettuce_types::PageRequest {
                    cursor: Some("not-a-cursor".into()),
                    limit: lettuce_types::PageLimit::new(1),
                },
                status: LifecycleFilter::All,
            },
        );
        assert!(bad.is_err());
    }

    #[test]
    fn malformed_keyword_payload_is_storage_failure_not_silent_data() {
        let database = Database::open_in_memory().expect("database");
        let created = LorebookRepository::create(
            &database,
            metadata(),
            vec![draft("Rain", "notes")],
            TimestampMillis::new(1),
        )
        .expect("create");
        let connection = database.connection().expect("database lock");
        connection
            .execute(
                "UPDATE lorebook_entries SET keywords_json='{}' WHERE lorebook_id=?1",
                [created.book.id.to_string()],
            )
            .expect("corrupt row");
        drop(connection);
        assert!(matches!(
            LorebookRepository::get(&database, created.book.id),
            Err(LorebookRepositoryError::Failure(_))
        ));
    }

    #[test]
    fn character_binding_mutations_use_owner_cas_and_keep_archived_books_visible() {
        let database = Database::open_in_memory().expect("database");
        let character_id = CharacterId::new();
        insert_character(&database, character_id);
        let first =
            LorebookRepository::create(&database, metadata(), Vec::new(), TimestampMillis::new(1))
                .expect("first");
        let second = LorebookRepository::create(
            &database,
            LorebookMetadataDraft {
                name: "Second".into(),
                ..metadata()
            },
            Vec::new(),
            TimestampMillis::new(1),
        )
        .expect("second");
        let bound = CharacterLorebookBindingRepository::bind_character_lorebook(
            &database,
            character_id,
            Revision::INITIAL,
            LorebookBindingCreate {
                lorebook_id: first.book.id,
                target: BindingInsertionTarget::Append,
            },
            TimestampMillis::new(2),
        )
        .expect("bind");
        assert_eq!(bound.owner_revision, Revision::new(2));
        let inserted = CharacterLorebookBindingRepository::bind_character_lorebook(
            &database,
            character_id,
            bound.owner_revision,
            LorebookBindingCreate {
                lorebook_id: second.book.id,
                target: BindingInsertionTarget::At(0),
            },
            TimestampMillis::new(3),
        )
        .expect("bind at");
        assert_eq!(
            inserted
                .bindings
                .iter()
                .map(|binding| binding.lorebook_id)
                .collect::<Vec<_>>(),
            vec![second.book.id, first.book.id]
        );
        assert!(matches!(
            CharacterLorebookBindingRepository::set_character_lorebook_enabled(
                &database,
                character_id,
                bound.owner_revision,
                first.book.id,
                false,
                TimestampMillis::new(4),
            ),
            Err(BindingRepositoryError::Conflict)
        ));
        let disabled = CharacterLorebookBindingRepository::set_character_lorebook_enabled(
            &database,
            character_id,
            inserted.owner_revision,
            first.book.id,
            false,
            TimestampMillis::new(4),
        )
        .expect("disable");
        assert!(!disabled.bindings[1].enabled);
        let archived = LorebookRepository::archive(
            &database,
            second.book.id,
            second.book.revision,
            TimestampMillis::new(5),
        )
        .expect("archive second");
        assert_eq!(archived.details.book.status, LifecycleStatus::Archived);
        let listed =
            CharacterLorebookBindingRepository::list_character_bindings(&database, character_id)
                .expect("list");
        assert_eq!(listed.len(), 2);
        assert!(matches!(
            LorebookDependencyReader::references_to(&database, first.book.id),
            Ok(refs) if refs == vec![LorebookReference::Character { id: character_id }]
        ));
    }

    #[test]
    fn binding_position_changes_bump_only_bindings_whose_ordinals_changed() {
        let database = Database::open_in_memory().expect("database");
        let character_id = CharacterId::new();
        insert_character(&database, character_id);
        let books = (0..3)
            .map(|index| {
                LorebookRepository::create(
                    &database,
                    LorebookMetadataDraft {
                        name: format!("Book {index}"),
                        ..metadata()
                    },
                    Vec::new(),
                    TimestampMillis::new(1),
                )
                .expect("book")
                .book
                .id
            })
            .collect::<Vec<_>>();
        let mut owner_revision = Revision::INITIAL;
        for book_id in books {
            owner_revision = CharacterLorebookBindingRepository::bind_character_lorebook(
                &database,
                character_id,
                owner_revision,
                LorebookBindingCreate {
                    lorebook_id: book_id,
                    target: BindingInsertionTarget::Append,
                },
                TimestampMillis::new(2),
            )
            .expect("bind")
            .owner_revision;
        }
        let initial =
            CharacterLorebookBindingRepository::list_character_bindings(&database, character_id)
                .expect("initial bindings");
        let ids = initial
            .iter()
            .map(|binding| binding.lorebook_id)
            .collect::<Vec<_>>();
        let inserted_book = LorebookRepository::create(
            &database,
            LorebookMetadataDraft {
                name: "Inserted".into(),
                ..metadata()
            },
            Vec::new(),
            TimestampMillis::new(2),
        )
        .expect("inserted book")
        .book
        .id;
        let inserted = CharacterLorebookBindingRepository::bind_character_lorebook(
            &database,
            character_id,
            owner_revision,
            LorebookBindingCreate {
                lorebook_id: inserted_book,
                target: BindingInsertionTarget::At(1),
            },
            TimestampMillis::new(3),
        )
        .expect("bind at");
        let by_id = |id| {
            inserted
                .bindings
                .iter()
                .find(|binding| binding.lorebook_id == id)
                .expect("binding exists")
        };
        assert_eq!(by_id(ids[0]).revision, Revision::INITIAL);
        assert_eq!(by_id(ids[1]).revision, Revision::new(2));
        assert_eq!(by_id(ids[2]).revision, Revision::new(2));
        let removed = CharacterLorebookBindingRepository::unbind_character_lorebook(
            &database,
            character_id,
            inserted.owner_revision,
            inserted_book,
            TimestampMillis::new(4),
        )
        .expect("unbind");
        let by_id = |id| {
            removed
                .bindings
                .iter()
                .find(|binding| binding.lorebook_id == id)
                .expect("binding exists")
        };
        assert_eq!(by_id(ids[0]).revision, Revision::INITIAL);
        assert_eq!(by_id(ids[1]).revision, Revision::new(3));
        assert_eq!(by_id(ids[2]).revision, Revision::new(3));
        let reordered = CharacterLorebookBindingRepository::reorder_character_lorebook(
            &database,
            character_id,
            removed.owner_revision,
            ids[2],
            0,
            TimestampMillis::new(5),
        )
        .expect("reorder");
        let by_id = |id| {
            reordered
                .bindings
                .iter()
                .find(|binding| binding.lorebook_id == id)
                .expect("binding exists")
        };
        assert_eq!(by_id(ids[2]).revision, Revision::new(4));
        assert_eq!(by_id(ids[1]).revision, Revision::new(4));
        assert_eq!(by_id(ids[0]).revision, Revision::new(2));
        let enabled = CharacterLorebookBindingRepository::set_character_lorebook_enabled(
            &database,
            character_id,
            reordered.owner_revision,
            ids[1],
            false,
            TimestampMillis::new(6),
        )
        .expect("enable");
        assert_eq!(
            enabled
                .bindings
                .iter()
                .find(|binding| binding.lorebook_id == ids[1])
                .expect("binding exists")
                .revision,
            Revision::new(5)
        );
        assert_eq!(
            enabled
                .bindings
                .iter()
                .find(|binding| binding.lorebook_id == ids[2])
                .expect("binding exists")
                .revision,
            Revision::new(4)
        );
    }

    #[test]
    fn file_backed_two_handle_binding_cas_race_has_one_winner() {
        for attempt in 0..20 {
            let path = std::env::temp_dir().join(format!(
                "lettuce-binding-race-{}-{attempt}.sqlite",
                LorebookId::new()
            ));
            let first = Database::open(&path).expect("first database");
            let second = Database::open(&path).expect("second database");
            let character_id = CharacterId::new();
            insert_character(&first, character_id);
            let first_book =
                LorebookRepository::create(&first, metadata(), Vec::new(), TimestampMillis::new(1))
                    .expect("first book");
            let second_book = LorebookRepository::create(
                &first,
                LorebookMetadataDraft {
                    name: "Second".into(),
                    ..metadata()
                },
                Vec::new(),
                TimestampMillis::new(1),
            )
            .expect("second book");
            let first_id = first_book.book.id;
            let second_id = second_book.book.id;
            let (left, right) = std::thread::scope(|scope| {
                let left = scope.spawn(|| {
                    CharacterLorebookBindingRepository::bind_character_lorebook(
                        &first,
                        character_id,
                        Revision::INITIAL,
                        LorebookBindingCreate {
                            lorebook_id: first_id,
                            target: BindingInsertionTarget::Append,
                        },
                        TimestampMillis::new(2),
                    )
                });
                let right = scope.spawn(|| {
                    CharacterLorebookBindingRepository::bind_character_lorebook(
                        &second,
                        character_id,
                        Revision::INITIAL,
                        LorebookBindingCreate {
                            lorebook_id: second_id,
                            target: BindingInsertionTarget::Append,
                        },
                        TimestampMillis::new(2),
                    )
                });
                (
                    left.join().expect("left thread"),
                    right.join().expect("right thread"),
                )
            });
            assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
            let bindings =
                CharacterLorebookBindingRepository::list_character_bindings(&first, character_id)
                    .expect("bindings");
            assert_eq!(bindings.len(), 1);
            assert_eq!(bindings[0].ordinal, 0);
            assert_eq!(bindings[0].revision, Revision::INITIAL);
            let connection = first.connection().expect("database lock");
            let owner_revision: i64 = connection
                .query_row(
                    "SELECT revision FROM characters WHERE id=?1",
                    [character_id.to_string()],
                    |row| row.get(0),
                )
                .expect("owner revision");
            assert_eq!(owner_revision, 2);
            drop(connection);
            drop(first);
            drop(second);
            let _ = std::fs::remove_file(&path);
            let _ = std::fs::remove_file(path.with_extension("sqlite-shm"));
            let _ = std::fs::remove_file(path.with_extension("sqlite-wal"));
        }
    }

    #[test]
    fn entry_write_failure_rolls_back_book_and_children_together() {
        let database = Database::open_in_memory().expect("database");
        let before = LorebookRepository::create(
            &database,
            metadata(),
            vec![draft("Rain", "notes")],
            TimestampMillis::new(1),
        )
        .expect("create");
        let connection = database.connection().expect("database lock");
        connection
            .execute(
                "CREATE TRIGGER fail_lorebook_entry_insert BEFORE INSERT ON lorebook_entries BEGIN SELECT RAISE(ABORT, 'test rollback'); END",
                [],
            )
            .expect("trigger");
        drop(connection);
        assert!(
            LorebookRepository::mutate_entries(
                &database,
                before.book.id,
                before.book.revision,
                LorebookEntryMutation::Add {
                    draft: draft("Wind", "wind notes"),
                    target: LorebookEntryInsertionTarget::Append,
                },
                TimestampMillis::new(2),
            )
            .is_err()
        );
        let connection = database.connection().expect("database lock");
        connection
            .execute("DROP TRIGGER fail_lorebook_entry_insert", [])
            .expect("drop trigger");
        drop(connection);
        assert_eq!(
            LorebookRepository::get(&database, before.book.id).expect("get"),
            Some(before)
        );
    }

    #[test]
    fn all_owner_binding_ports_and_starter_reverse_references_are_typed() {
        let database = Database::open_in_memory().expect("database");
        let character_id = CharacterId::new();
        let persona_id = PersonaId::new();
        let group_id = GroupId::new();
        let starter_id = ConversationStarterId::new();
        insert_character(&database, character_id);
        insert_persona(&database, persona_id);
        insert_group(&database, group_id);
        let book =
            LorebookRepository::create(&database, metadata(), Vec::new(), TimestampMillis::new(1))
                .expect("book");
        let second_book = LorebookRepository::create(
            &database,
            LorebookMetadataDraft {
                name: "Second book".into(),
                ..metadata()
            },
            Vec::new(),
            TimestampMillis::new(1),
        )
        .expect("second book");
        insert_starter(&database, character_id, starter_id, book.book.id);
        let create = |book_id| LorebookBindingCreate {
            lorebook_id: book_id,
            target: BindingInsertionTarget::Append,
        };
        let character = CharacterLorebookBindingRepository::bind_character_lorebook(
            &database,
            character_id,
            Revision::INITIAL,
            create(book.book.id),
            TimestampMillis::new(2),
        )
        .expect("character bind");
        let persona = PersonaLorebookBindingRepository::bind_persona_lorebook(
            &database,
            persona_id,
            Revision::INITIAL,
            create(book.book.id),
            TimestampMillis::new(2),
        )
        .expect("persona bind");
        let group = GroupLorebookBindingRepository::bind_group_lorebook(
            &database,
            group_id,
            Revision::INITIAL,
            create(book.book.id),
            TimestampMillis::new(2),
        )
        .expect("group bind");
        assert_eq!(character.owner_revision, Revision::new(2));
        assert_eq!(persona.owner_revision, Revision::new(2));
        assert_eq!(group.owner_revision, Revision::new(2));
        let character = CharacterLorebookBindingRepository::bind_character_lorebook(
            &database,
            character_id,
            character.owner_revision,
            create(second_book.book.id),
            TimestampMillis::new(2),
        )
        .expect("second character bind");
        let persona = PersonaLorebookBindingRepository::bind_persona_lorebook(
            &database,
            persona_id,
            persona.owner_revision,
            create(second_book.book.id),
            TimestampMillis::new(2),
        )
        .expect("second persona bind");
        let group = GroupLorebookBindingRepository::bind_group_lorebook(
            &database,
            group_id,
            group.owner_revision,
            create(second_book.book.id),
            TimestampMillis::new(2),
        )
        .expect("second group bind");
        let character = CharacterLorebookBindingRepository::reorder_character_lorebook(
            &database,
            character_id,
            character.owner_revision,
            second_book.book.id,
            0,
            TimestampMillis::new(3),
        )
        .expect("character reorder");
        let persona = PersonaLorebookBindingRepository::reorder_persona_lorebook(
            &database,
            persona_id,
            persona.owner_revision,
            second_book.book.id,
            0,
            TimestampMillis::new(3),
        )
        .expect("persona reorder");
        let group = GroupLorebookBindingRepository::reorder_group_lorebook(
            &database,
            group_id,
            group.owner_revision,
            second_book.book.id,
            0,
            TimestampMillis::new(3),
        )
        .expect("group reorder");
        assert_eq!(character.bindings[0].lorebook_id, second_book.book.id);
        assert_eq!(persona.bindings[0].lorebook_id, second_book.book.id);
        assert_eq!(group.bindings[0].lorebook_id, second_book.book.id);
        assert_eq!(
            LorebookDependencyReader::references_to(&database, book.book.id)
                .expect("reverse references"),
            vec![
                LorebookReference::Character { id: character_id },
                LorebookReference::Persona { id: persona_id },
                LorebookReference::Group { id: group_id },
                LorebookReference::Starter {
                    character_id,
                    starter_id,
                },
            ]
        );
        let character = CharacterLorebookBindingRepository::set_character_lorebook_enabled(
            &database,
            character_id,
            character.owner_revision,
            book.book.id,
            false,
            TimestampMillis::new(4),
        )
        .expect("character disable");
        let persona = PersonaLorebookBindingRepository::set_persona_lorebook_enabled(
            &database,
            persona_id,
            persona.owner_revision,
            book.book.id,
            false,
            TimestampMillis::new(4),
        )
        .expect("persona disable");
        let group = GroupLorebookBindingRepository::set_group_lorebook_enabled(
            &database,
            group_id,
            group.owner_revision,
            book.book.id,
            false,
            TimestampMillis::new(4),
        )
        .expect("group disable");
        assert!(
            character
                .bindings
                .iter()
                .find(|binding| binding.lorebook_id == book.book.id)
                .is_some_and(|binding| !binding.enabled)
        );
        assert!(
            persona
                .bindings
                .iter()
                .find(|binding| binding.lorebook_id == book.book.id)
                .is_some_and(|binding| !binding.enabled)
        );
        assert!(
            group
                .bindings
                .iter()
                .find(|binding| binding.lorebook_id == book.book.id)
                .is_some_and(|binding| !binding.enabled)
        );
        let archived = LorebookRepository::archive(
            &database,
            book.book.id,
            book.book.revision,
            TimestampMillis::new(4),
        )
        .expect("archive");
        assert_eq!(archived.details.book.status, LifecycleStatus::Archived);
        let archived_list =
            PersonaLorebookBindingRepository::list_persona_bindings(&database, persona_id)
                .expect("archived reference list");
        assert_eq!(archived_list.len(), 2);
        let archived_book = LorebookRepository::create(
            &database,
            LorebookMetadataDraft {
                name: "Archived only".into(),
                ..metadata()
            },
            Vec::new(),
            TimestampMillis::new(4),
        )
        .expect("archived-only book");
        let archived_book = LorebookRepository::archive(
            &database,
            archived_book.book.id,
            archived_book.book.revision,
            TimestampMillis::new(4),
        )
        .expect("archive-only book");
        assert!(matches!(
            CharacterLorebookBindingRepository::bind_character_lorebook(
                &database,
                character_id,
                character.owner_revision,
                create(archived_book.details.book.id),
                TimestampMillis::new(5),
            ),
            Err(BindingRepositoryError::NotFound)
        ));
        let character = CharacterLorebookBindingRepository::unbind_character_lorebook(
            &database,
            character_id,
            character.owner_revision,
            book.book.id,
            TimestampMillis::new(5),
        )
        .expect("character unbind");
        let persona = PersonaLorebookBindingRepository::unbind_persona_lorebook(
            &database,
            persona_id,
            persona.owner_revision,
            book.book.id,
            TimestampMillis::new(5),
        )
        .expect("persona unbind");
        let group = GroupLorebookBindingRepository::unbind_group_lorebook(
            &database,
            group_id,
            group.owner_revision,
            book.book.id,
            TimestampMillis::new(5),
        )
        .expect("group unbind");
        let character = CharacterLorebookBindingRepository::unbind_character_lorebook(
            &database,
            character_id,
            character.owner_revision,
            second_book.book.id,
            TimestampMillis::new(5),
        )
        .expect("second character unbind");
        let persona = PersonaLorebookBindingRepository::unbind_persona_lorebook(
            &database,
            persona_id,
            persona.owner_revision,
            second_book.book.id,
            TimestampMillis::new(5),
        )
        .expect("second persona unbind");
        let group = GroupLorebookBindingRepository::unbind_group_lorebook(
            &database,
            group_id,
            group.owner_revision,
            second_book.book.id,
            TimestampMillis::new(5),
        )
        .expect("second group unbind");
        assert!(character.bindings.is_empty());
        assert!(persona.bindings.is_empty());
        assert!(group.bindings.is_empty());
        assert!(
            LorebookDependencyReader::references_to(&database, book.book.id)
                .expect("reverse references after unbind")
                .iter()
                .any(|reference| matches!(
                    reference,
                    LorebookReference::Starter { starter_id: id, .. } if *id == starter_id
                ))
        );
    }

    #[test]
    fn missing_owner_and_book_are_not_found_without_partial_owner_updates() {
        let database = Database::open_in_memory().expect("database");
        let character_id = CharacterId::new();
        let book =
            LorebookRepository::create(&database, metadata(), Vec::new(), TimestampMillis::new(1))
                .expect("book");
        let create = |lorebook_id| LorebookBindingCreate {
            lorebook_id,
            target: BindingInsertionTarget::Append,
        };
        assert!(matches!(
            CharacterLorebookBindingRepository::bind_character_lorebook(
                &database,
                character_id,
                Revision::INITIAL,
                create(book.book.id),
                TimestampMillis::new(2),
            ),
            Err(BindingRepositoryError::NotFound)
        ));
        insert_character(&database, character_id);
        assert!(matches!(
            CharacterLorebookBindingRepository::bind_character_lorebook(
                &database,
                character_id,
                Revision::INITIAL,
                create(LorebookId::new()),
                TimestampMillis::new(2),
            ),
            Err(BindingRepositoryError::NotFound)
        ));
        assert!(
            CharacterLorebookBindingRepository::list_character_bindings(&database, character_id)
                .expect("list")
                .is_empty()
        );
    }

    #[test]
    fn two_handles_hold_lorebook_snapshot_and_enforce_cas() {
        let path =
            std::env::temp_dir().join(format!("lettuce-lorebook-{}.sqlite", LorebookId::new()));
        let first = Database::open(&path).expect("first database");
        let second = Database::open(&path).expect("second database");
        let created = LorebookRepository::create(
            &first,
            metadata(),
            vec![draft("Rain", "notes")],
            TimestampMillis::new(1),
        )
        .expect("create");
        let mut connection = first.connection().expect("first lock");
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .expect("snapshot transaction");
        let old = load_details(&tx, created.book.id)
            .expect("old snapshot")
            .expect("old book");
        let revised = LorebookRepository::revise_metadata(
            &second,
            created.book.id,
            created.book.revision,
            LorebookMetadataDraft {
                name: "Changed".into(),
                ..metadata()
            },
            TimestampMillis::new(2),
        )
        .expect("revision");
        assert_eq!(revised.book_revision, Revision::new(2));
        assert_eq!(
            load_details(&tx, created.book.id)
                .expect("snapshot read")
                .expect("snapshot book"),
            old
        );
        tx.commit().expect("commit snapshot");
        drop(connection);
        assert!(matches!(
            LorebookRepository::revise_metadata(
                &first,
                created.book.id,
                created.book.revision,
                metadata(),
                TimestampMillis::new(3),
            ),
            Err(LorebookRepositoryError::Conflict)
        ));
        drop(first);
        drop(second);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("sqlite-shm"));
        let _ = std::fs::remove_file(path.with_extension("sqlite-wal"));
    }

    #[test]
    fn icon_foreign_keys_and_scalar_order_timestamp_corruption_are_strict() {
        let database = Database::open_in_memory().expect("database");
        let image = image_asset(&database, 'y');
        let audio = audio_asset(&database, 'z');
        let icon_book = LorebookRepository::create(
            &database,
            LorebookMetadataDraft {
                icon_asset_id: Some(image),
                ..metadata()
            },
            Vec::new(),
            TimestampMillis::new(1),
        )
        .expect("image icon book");
        assert_eq!(icon_book.book.icon_asset_id, Some(image));
        assert!(matches!(
            LorebookRepository::create(
                &database,
                LorebookMetadataDraft {
                    icon_asset_id: Some(audio),
                    ..metadata()
                },
                Vec::new(),
                TimestampMillis::new(1),
            ),
            Err(LorebookRepositoryError::Failure(_))
        ));
        assert!(matches!(
            LorebookRepository::create(
                &database,
                LorebookMetadataDraft {
                    icon_asset_id: Some(lettuce_types::AssetId::new()),
                    ..metadata()
                },
                Vec::new(),
                TimestampMillis::new(1),
            ),
            Err(LorebookRepositoryError::Failure(_))
        ));
        let status =
            LorebookRepository::create(&database, metadata(), Vec::new(), TimestampMillis::new(1))
                .expect("status book");
        let timestamps = LorebookRepository::create(
            &database,
            LorebookMetadataDraft {
                name: "Timestamps".into(),
                ..metadata()
            },
            Vec::new(),
            TimestampMillis::new(1),
        )
        .expect("timestamp book");
        let entries = LorebookRepository::create(
            &database,
            LorebookMetadataDraft {
                name: "Entries".into(),
                ..metadata()
            },
            vec![draft("One", "one"), draft("Two", "two")],
            TimestampMillis::new(1),
        )
        .expect("entries book");
        for statement in [
            format!(
                "UPDATE lorebooks SET status='corrupt' WHERE id='{}'",
                status.book.id
            ),
            format!(
                "UPDATE lorebooks SET created_at=2,updated_at=1 WHERE id='{}'",
                timestamps.book.id
            ),
            format!(
                "UPDATE lorebook_entries SET ordinal=9 WHERE id='{}'",
                entries.entries[0].id
            ),
        ] {
            let connection = database.connection().expect("database lock");
            connection
                .execute_batch("PRAGMA ignore_check_constraints=ON")
                .expect("ignore checks");
            connection.execute(&statement, []).expect("corrupt row");
            connection
                .execute_batch("PRAGMA ignore_check_constraints=OFF")
                .expect("restore checks");
            drop(connection);
            let id = if statement.contains("status") {
                status.book.id
            } else if statement.contains("created_at") {
                timestamps.book.id
            } else {
                entries.book.id
            };
            assert!(matches!(
                LorebookRepository::get(&database, id),
                Err(LorebookRepositoryError::Failure(_))
            ));
        }
        let owner = CharacterId::new();
        insert_character(&database, owner);
        let binding_book = LorebookRepository::create(
            &database,
            LorebookMetadataDraft {
                name: "Binding corruption".into(),
                ..metadata()
            },
            Vec::new(),
            TimestampMillis::new(1),
        )
        .expect("binding book");
        CharacterLorebookBindingRepository::bind_character_lorebook(
            &database,
            owner,
            Revision::INITIAL,
            LorebookBindingCreate {
                lorebook_id: binding_book.book.id,
                target: BindingInsertionTarget::Append,
            },
            TimestampMillis::new(2),
        )
        .expect("binding");
        let connection = database.connection().expect("database lock");
        connection
            .execute_batch("PRAGMA ignore_check_constraints=ON")
            .expect("ignore checks");
        connection
            .execute(
                "UPDATE character_lorebook_bindings SET enabled=7 WHERE character_id=?1",
                [owner.to_string()],
            )
            .expect("corrupt binding");
        connection
            .execute_batch("PRAGMA ignore_check_constraints=OFF")
            .expect("restore checks");
        drop(connection);
        assert!(matches!(
            CharacterLorebookBindingRepository::list_character_bindings(&database, owner),
            Err(BindingRepositoryError::Failure(_))
        ));
    }
}
