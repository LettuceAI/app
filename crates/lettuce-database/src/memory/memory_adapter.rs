use std::str::FromStr;

use lettuce_memory::{
    DynamicMemoryApprovalRepository, DynamicMemoryPendingApproval, MemoryCategory, MemoryChangeSet,
    MemoryItem, MemoryRepository, MemoryRepositoryError, MemoryRetrievalAccess,
    MemoryRetrievalAccessReceipt, MemoryRetrievalRepository, MemoryShortId, MemorySpaceSnapshot,
    MemorySummary, MemorySummaryChange, MemorySummaryCommit, MemorySummaryRepository, Score,
};
use lettuce_types::{
    ConversationId, MemoryId, MemorySpaceId, MessageId, Revision, TimestampMillis,
};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::Database;

const SELECT_ITEM: &str = "
    SELECT id, short_id, text, category, source_message_id, source_role, observed_at, observed_time_precision,
           superseded_by, superseded_at, supersedes_json, token_count, is_cold, is_pinned,
           importance, persistence_importance, prompt_importance, volatility,
           access_count, created_at, last_accessed_at
      FROM memory_items
     WHERE space_id = ?1
     ORDER BY ordinal";

fn storage(_: impl std::fmt::Debug) -> MemoryRepositoryError {
    MemoryRepositoryError::Failure("sqlite memory operation failed".to_owned())
}

fn category_name(value: MemoryCategory) -> &'static str {
    match value {
        MemoryCategory::CharacterTrait => "character_trait",
        MemoryCategory::Relationship => "relationship",
        MemoryCategory::PlotEvent => "plot_event",
        MemoryCategory::WorldDetail => "world_detail",
        MemoryCategory::Preference => "preference",
        MemoryCategory::Other => "other",
    }
}

fn parse_category(value: &str) -> Result<MemoryCategory, MemoryRepositoryError> {
    match value {
        "character_trait" => Ok(MemoryCategory::CharacterTrait),
        "relationship" => Ok(MemoryCategory::Relationship),
        "plot_event" => Ok(MemoryCategory::PlotEvent),
        "world_detail" => Ok(MemoryCategory::WorldDetail),
        "preference" => Ok(MemoryCategory::Preference),
        "other" => Ok(MemoryCategory::Other),
        _ => Err(storage(value)),
    }
}

pub(crate) fn sql_revision(value: Revision) -> Result<i64, MemoryRepositoryError> {
    i64::try_from(value.get()).map_err(storage)
}

pub(crate) fn parse_revision(value: i64) -> Result<Revision, MemoryRepositoryError> {
    u64::try_from(value).map(Revision::new).map_err(storage)
}

fn parse_id<T: FromStr>(value: String) -> Result<T, MemoryRepositoryError> {
    value.parse().map_err(|_| storage(value))
}

fn parse_score(value: i64) -> Result<Score, MemoryRepositoryError> {
    u16::try_from(value)
        .ok()
        .and_then(Score::from_basis_points)
        .ok_or_else(|| storage(value))
}

pub(crate) fn insert_items(
    transaction: &Transaction<'_>,
    space_id: MemorySpaceId,
    items: &[MemoryItem],
) -> Result<(), MemoryRepositoryError> {
    for (ordinal, item) in items.iter().enumerate() {
        insert_item_at(
            transaction,
            space_id,
            i64::try_from(ordinal).map_err(storage)?,
            item,
        )?;
    }
    Ok(())
}

/// Inserts one item at a given ordinal.
pub(crate) fn insert_item_at(
    transaction: &Transaction<'_>,
    space_id: MemorySpaceId,
    ordinal: i64,
    item: &MemoryItem,
) -> Result<(), MemoryRepositoryError> {
    transaction
        .execute(
            "INSERT INTO memory_items (
                space_id, id, ordinal, text, category, source_message_id, source_role, observed_at, observed_time_precision,
                superseded_by, superseded_at, supersedes_json, token_count, is_cold, is_pinned,
                importance, persistence_importance, prompt_importance, volatility,
                access_count, created_at, last_accessed_at, short_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)",
            params![
                space_id.to_string(),
                item.id.to_string(),
                ordinal,
                item.text,
                category_name(item.category),
                item.source_message_id.map(|id| id.to_string()),
                item.source_role.map(|role| match role {
                    lettuce_conversations::MessageRole::User => "user",
                    lettuce_conversations::MessageRole::Assistant => "assistant",
                    _ => "invalid",
                }),
                item.observed_at.map(TimestampMillis::get),
                item.observed_time_precision,
                item.superseded_by.map(|id| id.to_string()),
                item.superseded_at.map(TimestampMillis::get),
                serde_json::to_string(&item.supersedes).map_err(storage)?,
                i64::from(item.token_count),
                item.is_cold,
                item.is_pinned,
                i64::from(item.importance.basis_points()),
                i64::from(item.persistence_importance.basis_points()),
                i64::from(item.prompt_importance.basis_points()),
                i64::from(item.volatility.basis_points()),
                i64::from(item.access_count),
                item.created_at.get(),
                item.last_accessed_at.get(),
                i64::from(item.short_id.get()),
            ],
        )
        .map_err(storage)?;
    Ok(())
}

/// Creates a companion character's memory pool from an imported snapshot, or
/// makes the conversation a member of the pool an earlier conversation
/// created. Returns whether this call created the pool.
pub(crate) fn insert_pool_space_in(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    character_id: lettuce_types::CharacterId,
    space: &lettuce_memory::MemorySpaceSnapshot,
) -> Result<bool, lettuce_conversations::ConversationRepositoryError> {
    let exists = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM companion_memory_pools WHERE character_id = ?1)",
            [character_id.to_string()],
            |row| row.get::<_, bool>(0),
        )
        .map_err(space_storage_error)?;
    if exists {
        join_companion_pool_in(transaction, conversation_id, character_id)?;
        return Ok(false);
    }
    space.validate().map_err(space_storage_error)?;
    transaction
        .execute(
            "INSERT INTO memory_spaces (id, revision) VALUES (?1, ?2)",
            params![
                space.id.to_string(),
                i64::try_from(space.revision.get()).map_err(space_storage_error)?
            ],
        )
        .map_err(space_storage_error)?;
    transaction
        .execute(
            "INSERT INTO companion_memory_pools (character_id, space_id) VALUES (?1, ?2)",
            params![character_id.to_string(), space.id.to_string()],
        )
        .map_err(space_storage_error)?;
    insert_pool_binding_in(transaction, conversation_id, space.id)?;
    insert_items(transaction, space.id, &space.items).map_err(space_storage_error)?;
    Ok(true)
}

/// Makes a companion conversation a member of its character's memory pool,
/// creating the pool space the first time the companion needs one.
pub(crate) fn join_companion_pool_in(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    character_id: lettuce_types::CharacterId,
) -> Result<MemorySpaceId, lettuce_conversations::ConversationRepositoryError> {
    let existing = transaction
        .query_row(
            "SELECT space_id FROM companion_memory_pools WHERE character_id = ?1",
            [character_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(space_storage_error)?;
    let space_id = match existing {
        Some(value) => value.parse().map_err(space_storage_error)?,
        None => {
            let space_id = MemorySpaceId::new();
            transaction
                .execute(
                    "INSERT INTO memory_spaces (id, revision) VALUES (?1, 1)",
                    [space_id.to_string()],
                )
                .map_err(space_storage_error)?;
            transaction
                .execute(
                    "INSERT INTO companion_memory_pools (character_id, space_id) VALUES (?1, ?2)",
                    params![character_id.to_string(), space_id.to_string()],
                )
                .map_err(space_storage_error)?;
            space_id
        }
    };
    insert_pool_binding_in(transaction, conversation_id, space_id)?;
    Ok(space_id)
}

fn insert_pool_binding_in(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    space_id: MemorySpaceId,
) -> Result<(), lettuce_conversations::ConversationRepositoryError> {
    transaction
        .execute(
            "INSERT INTO conversation_memory_spaces (conversation_id, space_id, pooled)
             VALUES (?1, ?2, 1)",
            params![conversation_id.to_string(), space_id.to_string()],
        )
        .map_err(space_storage_error)?;
    Ok(())
}

/// The memory space a conversation uses now: its companion pool while the
/// character shares memory across chats, else its own space (legacy
/// `resolve_effective_memory_owner`, read on every use).
pub(crate) fn active_space_id_in(
    connection: &rusqlite::Connection,
    conversation_id: ConversationId,
) -> rusqlite::Result<Option<MemorySpaceId>> {
    let pool = connection
        .query_row(
            "SELECT pool.character_id, pool.space_id FROM conversation_memory_spaces binding
               JOIN companion_memory_pools pool ON pool.space_id = binding.space_id
              WHERE binding.conversation_id = ?1 AND binding.pooled = 1",
            [conversation_id.to_string()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    if let Some((character_id, space_id)) = pool {
        let character_id = character_id
            .parse()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        if crate::catalog::character_adapter::companion_memory_shared_in(connection, character_id)?
        {
            return space_id
                .parse()
                .map(Some)
                .map_err(|_| rusqlite::Error::InvalidQuery);
        }
    }
    connection
        .query_row(
            "SELECT space_id FROM conversation_memory_spaces
              WHERE conversation_id = ?1 AND pooled = 0",
            [conversation_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|value| value.parse().map_err(|_| rusqlite::Error::InvalidQuery))
        .transpose()
}

/// The conversations that can write a space: its owner, or a pool's members.
pub(crate) fn space_conversations_in(
    connection: &rusqlite::Connection,
    space_id: MemorySpaceId,
    limit: u32,
) -> rusqlite::Result<Vec<String>> {
    connection
        .prepare(
            "SELECT conversation_id FROM conversation_memory_spaces WHERE space_id = ?1
             ORDER BY 1 LIMIT ?2",
        )?
        .query_map(params![space_id.to_string(), limit], |row| row.get(0))?
        .collect()
}

fn space_storage_error<E>(_: E) -> lettuce_conversations::ConversationRepositoryError {
    lettuce_conversations::ConversationRepositoryError::Storage
}

/// Creates a conversation's memory space with its restored id, revision and
/// items on the caller's transaction.
pub(crate) fn insert_space_in(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    space: &lettuce_memory::MemorySpaceSnapshot,
) -> Result<(), lettuce_conversations::ConversationRepositoryError> {
    space.validate().map_err(space_storage_error)?;
    transaction
        .execute(
            "INSERT INTO memory_spaces (id, revision) VALUES (?1, ?2)",
            params![
                space.id.to_string(),
                i64::try_from(space.revision.get()).map_err(space_storage_error)?
            ],
        )
        .map_err(space_storage_error)?;
    transaction
        .execute(
            "INSERT INTO conversation_memory_spaces (conversation_id, space_id)
             VALUES (?1, ?2)",
            params![conversation_id.to_string(), space.id.to_string()],
        )
        .map_err(space_storage_error)?;
    insert_items(transaction, space.id, &space.items).map_err(space_storage_error)
}

pub(crate) fn create_conversation_space_in(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
) -> Result<MemorySpaceId, lettuce_conversations::ConversationRepositoryError> {
    let space_id = MemorySpaceId::new();
    transaction
        .execute(
            "INSERT INTO memory_spaces (id, revision) VALUES (?1, 1)",
            [space_id.to_string()],
        )
        .map_err(|_| lettuce_conversations::ConversationRepositoryError::Storage)?;
    transaction
        .execute(
            "INSERT INTO conversation_memory_spaces (conversation_id, space_id)
             VALUES (?1, ?2)",
            params![conversation_id.to_string(), space_id.to_string()],
        )
        .map_err(|_| lettuce_conversations::ConversationRepositoryError::Storage)?;
    Ok(space_id)
}

pub(crate) fn get_in(
    transaction: &Transaction<'_>,
    id: MemorySpaceId,
) -> Result<Option<MemorySpaceSnapshot>, MemoryRepositoryError> {
    let revision = transaction
        .query_row(
            "SELECT revision FROM memory_spaces WHERE id = ?1",
            [id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(storage)?;
    let Some(revision) = revision else {
        return Ok(None);
    };
    let items = {
        let mut statement = transaction.prepare(SELECT_ITEM).map_err(storage)?;
        let mut rows = statement.query([id.to_string()]).map_err(storage)?;
        let mut items = Vec::new();
        while let Some(row) = rows.next().map_err(storage)? {
            items.push(item_from_row(row)?);
        }
        items
    };
    let snapshot = MemorySpaceSnapshot {
        id,
        revision: parse_revision(revision)?,
        items,
    };
    snapshot.validate()?;
    Ok(Some(snapshot))
}

fn item_from_row(row: &rusqlite::Row<'_>) -> Result<MemoryItem, MemoryRepositoryError> {
    Ok(MemoryItem {
        id: parse_id(row.get::<_, String>(0).map_err(storage)?)?,
        short_id: MemoryShortId::new(row.get(1).map_err(storage)?)
            .ok_or_else(|| storage("invalid memory short id"))?,
        text: row.get(2).map_err(storage)?,
        category: parse_category(&row.get::<_, String>(3).map_err(storage)?)?,
        source_message_id: row
            .get::<_, Option<String>>(4)
            .map_err(storage)?
            .map(parse_id)
            .transpose()?,
        source_role: match row.get::<_, Option<String>>(5).map_err(storage)?.as_deref() {
            Some("user") => Some(lettuce_conversations::MessageRole::User),
            Some("assistant") => Some(lettuce_conversations::MessageRole::Assistant),
            None => None,
            _ => return Err(storage("invalid memory source role")),
        },
        observed_at: row
            .get::<_, Option<i64>>(6)
            .map_err(storage)?
            .map(TimestampMillis::new),
        observed_time_precision: row.get(7).map_err(storage)?,
        superseded_by: row
            .get::<_, Option<String>>(8)
            .map_err(storage)?
            .map(parse_id)
            .transpose()?,
        superseded_at: row
            .get::<_, Option<i64>>(9)
            .map_err(storage)?
            .map(TimestampMillis::new),
        supersedes: serde_json::from_str(&row.get::<_, String>(10).map_err(storage)?)
            .map_err(storage)?,
        token_count: row.get(11).map_err(storage)?,
        is_cold: row.get(12).map_err(storage)?,
        is_pinned: row.get(13).map_err(storage)?,
        importance: parse_score(row.get(14).map_err(storage)?)?,
        persistence_importance: parse_score(row.get(15).map_err(storage)?)?,
        prompt_importance: parse_score(row.get(16).map_err(storage)?)?,
        volatility: parse_score(row.get(17).map_err(storage)?)?,
        access_count: row.get(18).map_err(storage)?,
        created_at: TimestampMillis::new(row.get(19).map_err(storage)?),
        last_accessed_at: TimestampMillis::new(row.get(20).map_err(storage)?),
    })
}

/// One item of a space.
pub(crate) fn get_item_in(
    transaction: &Transaction<'_>,
    space_id: MemorySpaceId,
    item_id: lettuce_types::MemoryId,
) -> Result<Option<MemoryItem>, MemoryRepositoryError> {
    let sql = SELECT_ITEM.replace("WHERE space_id = ?1", "WHERE space_id = ?1 AND id = ?2");
    let mut statement = transaction.prepare(&sql).map_err(storage)?;
    let mut rows = statement
        .query(params![space_id.to_string(), item_id.to_string()])
        .map_err(storage)?;
    rows.next().map_err(storage)?.map(item_from_row).transpose()
}

/// Whether a retrieval access after `after` promoted `memory_id` from cold.
fn promoted_after(
    transaction: &Transaction<'_>,
    space_id: MemorySpaceId,
    memory_id: MemoryId,
    after: TimestampMillis,
) -> Result<bool, MemoryRepositoryError> {
    transaction
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM memory_retrieval_accesses access
                 WHERE access.space_id = ?1 AND access.accessed_at > ?3
                   AND EXISTS (
                       SELECT 1 FROM json_each(access.promoted_memory_ids_json) promoted
                        WHERE promoted.value = ?2
                   )
             )",
            params![space_id.to_string(), memory_id.to_string(), after.get()],
            |row| row.get::<_, bool>(0),
        )
        .map_err(storage)
}

/// Retrieval bookkeeping lands on stored rows between a writer's read and its
/// commit without advancing the revision, so a writer's copy of an existing
/// memory can predate it. The stored row was accessed after the writer's copy
/// when it has more accesses and a last access no earlier than the copy's; then
/// its access count and time are kept, and its hotness and importance too
/// unless the writer cooled a memory that no later access promoted. A stored
/// item keeps its token count whenever the writer sends the same text.
fn merge_stored_fields(
    transaction: &Transaction<'_>,
    space_id: MemorySpaceId,
    written: &MemoryItem,
    stored: Option<&MemoryItem>,
) -> Result<MemoryItem, MemoryRepositoryError> {
    let Some(stored) = stored else {
        return Ok(written.clone());
    };
    let mut merged = written.clone();
    if stored.text == written.text {
        merged.token_count = stored.token_count;
    }
    if stored.access_count <= written.access_count
        || stored.last_accessed_at < written.last_accessed_at
    {
        return Ok(merged);
    }
    merged.access_count = stored.access_count;
    merged.last_accessed_at = stored.last_accessed_at;
    let keep_stored_heat = match (written.is_cold, stored.is_cold) {
        (false, false) => true,
        (true, false) => {
            promoted_after(transaction, space_id, written.id, written.last_accessed_at)?
        }
        (_, true) => false,
    };
    if keep_stored_heat {
        merged.is_cold = stored.is_cold;
        merged.importance = stored.importance;
    }
    Ok(merged)
}

pub(crate) fn compare_and_apply_in(
    transaction: &Transaction<'_>,
    change: &MemoryChangeSet,
) -> Result<MemorySpaceSnapshot, MemoryRepositoryError> {
    change.validate()?;
    let next_revision = change
        .expected_revision
        .next()
        .map_err(|_| MemoryRepositoryError::Conflict)?;
    let stored = get_in(transaction, change.space_id)?.ok_or(MemoryRepositoryError::NotFound)?;
    if stored.revision != change.expected_revision {
        return Err(MemoryRepositoryError::Conflict);
    }
    let stored_items = stored
        .items
        .iter()
        .map(|item| (item.id, item))
        .collect::<std::collections::HashMap<_, _>>();
    let items = change
        .items
        .iter()
        .map(|item| {
            merge_stored_fields(
                transaction,
                change.space_id,
                item,
                stored_items.get(&item.id).copied(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    transaction
        .execute(
            "DELETE FROM memory_items WHERE space_id = ?1",
            [change.space_id.to_string()],
        )
        .map_err(storage)?;
    insert_items(transaction, change.space_id, &items)?;
    let updated = transaction
        .execute(
            "UPDATE memory_spaces SET revision = ?2 WHERE id = ?1 AND revision = ?3",
            params![
                change.space_id.to_string(),
                sql_revision(next_revision)?,
                sql_revision(change.expected_revision)?,
            ],
        )
        .map_err(storage)?;
    if updated != 1 {
        return Err(MemoryRepositoryError::Conflict);
    }
    let snapshot = MemorySpaceSnapshot {
        id: change.space_id,
        revision: next_revision,
        items,
    };
    snapshot.validate()?;
    Ok(snapshot)
}

pub(crate) fn get_summary_in(
    transaction: &Transaction<'_>,
    space_id: MemorySpaceId,
) -> Result<Option<MemorySummary>, MemoryRepositoryError> {
    let row = transaction
        .query_row(
            "SELECT text, token_count, window_start, window_end, updated_at
               FROM memory_summaries
              WHERE space_id = ?1",
            [space_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()
        .map_err(storage)?;
    let Some((text, token_count, window_start, window_end, updated_at)) = row else {
        return Ok(None);
    };
    let source_message_ids = {
        let mut statement = transaction
            .prepare(
                "SELECT message_id
                   FROM memory_summary_source_messages
                  WHERE space_id = ?1
                  ORDER BY ordinal",
            )
            .map_err(storage)?;
        let mut rows = statement.query([space_id.to_string()]).map_err(storage)?;
        let mut ids = Vec::new();
        while let Some(row) = rows.next().map_err(storage)? {
            ids.push(parse_id::<MessageId>(
                row.get::<_, String>(0).map_err(storage)?,
            )?);
        }
        ids
    };
    let summary = MemorySummary {
        space_id,
        text,
        token_count: u32::try_from(token_count).map_err(storage)?,
        window_start: u64::try_from(window_start).map_err(storage)?,
        window_end: u64::try_from(window_end).map_err(storage)?,
        source_message_ids,
        updated_at: TimestampMillis::new(updated_at),
    };
    summary.validate()?;
    Ok(Some(summary))
}

pub(crate) fn replace_summary_in(
    transaction: &Transaction<'_>,
    space_id: MemorySpaceId,
    summary: Option<&MemorySummary>,
) -> Result<Option<MemorySummary>, MemoryRepositoryError> {
    if summary.is_some_and(|summary| summary.space_id != space_id || summary.validate().is_err()) {
        return Err(storage("invalid replacement summary"));
    }
    let bindings = space_conversations_in(transaction, space_id, 2).map_err(storage)?;
    let conversation_id = match (bindings.as_slice(), summary) {
        ([], _) => return Err(MemoryRepositoryError::NotFound),
        ([only], _) => only.clone(),
        (_, Some(summary)) => {
            let source = summary
                .source_message_ids
                .first()
                .ok_or_else(|| storage("summary without source messages"))?;
            transaction
                .query_row(
                    "SELECT conversation_id FROM conversation_messages WHERE id = ?1 LIMIT 1",
                    [source.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(storage)?
                .ok_or(MemoryRepositoryError::NotFound)?
        }
        (_, None) => String::new(),
    };
    transaction
        .execute(
            "DELETE FROM memory_summary_source_messages WHERE space_id = ?1",
            [space_id.to_string()],
        )
        .map_err(storage)?;
    if let Some(summary) = summary {
        transaction
            .execute(
                "INSERT INTO memory_summaries (
                    space_id, conversation_id, text, token_count,
                    window_start, window_end, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(space_id) DO UPDATE SET
                    conversation_id = excluded.conversation_id,
                    text = excluded.text,
                    token_count = excluded.token_count,
                    window_start = excluded.window_start,
                    window_end = excluded.window_end,
                    updated_at = excluded.updated_at",
                params![
                    space_id.to_string(),
                    conversation_id,
                    summary.text,
                    i64::from(summary.token_count),
                    i64::try_from(summary.window_start).map_err(storage)?,
                    i64::try_from(summary.window_end).map_err(storage)?,
                    summary.updated_at.get(),
                ],
            )
            .map_err(storage)?;
        for (ordinal, message_id) in summary.source_message_ids.iter().enumerate() {
            transaction
                .execute(
                    "INSERT INTO memory_summary_source_messages (
                        space_id, conversation_id, message_id, ordinal
                     ) VALUES (?1, ?2, ?3, ?4)",
                    params![
                        space_id.to_string(),
                        conversation_id,
                        message_id.to_string(),
                        i64::try_from(ordinal).map_err(storage)?,
                    ],
                )
                .map_err(storage)?;
        }
    } else {
        transaction
            .execute(
                "DELETE FROM memory_summaries WHERE space_id = ?1",
                [space_id.to_string()],
            )
            .map_err(storage)?;
    }
    get_summary_in(transaction, space_id)
}

pub(crate) fn compare_and_apply_summary_in(
    transaction: &Transaction<'_>,
    change: &MemorySummaryChange,
) -> Result<MemorySummaryCommit, MemoryRepositoryError> {
    change.validate()?;
    let space_id = change.summary.space_id;
    let current_revision = transaction
        .query_row(
            "SELECT revision FROM memory_spaces WHERE id = ?1",
            [space_id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(storage)?
        .ok_or(MemoryRepositoryError::NotFound)?;
    if parse_revision(current_revision)? != change.expected_revision {
        return Err(MemoryRepositoryError::Conflict);
    }
    replace_summary_in(transaction, space_id, Some(&change.summary))?;
    let next_revision = change
        .expected_revision
        .next()
        .map_err(|_| MemoryRepositoryError::Conflict)?;
    let updated = transaction
        .execute(
            "UPDATE memory_spaces SET revision = ?2 WHERE id = ?1 AND revision = ?3",
            params![
                space_id.to_string(),
                sql_revision(next_revision)?,
                sql_revision(change.expected_revision)?,
            ],
        )
        .map_err(storage)?;
    if updated != 1 {
        return Err(MemoryRepositoryError::Conflict);
    }
    let memory = get_in(transaction, space_id)?.ok_or(MemoryRepositoryError::NotFound)?;
    let summary =
        get_summary_in(transaction, space_id)?.ok_or_else(|| storage("missing summary"))?;
    Ok(MemorySummaryCommit { memory, summary })
}

/// Verifies the memory-space revision and advances it once without touching
/// items or the summary.
pub(crate) fn advance_revision_in(
    transaction: &Transaction<'_>,
    space_id: MemorySpaceId,
    expected_revision: Revision,
) -> Result<MemorySpaceSnapshot, MemoryRepositoryError> {
    let next_revision = expected_revision
        .next()
        .map_err(|_| MemoryRepositoryError::Conflict)?;
    let updated = transaction
        .execute(
            "UPDATE memory_spaces SET revision = ?2 WHERE id = ?1 AND revision = ?3",
            params![
                space_id.to_string(),
                sql_revision(next_revision)?,
                sql_revision(expected_revision)?,
            ],
        )
        .map_err(storage)?;
    if updated != 1 {
        return match get_in(transaction, space_id)? {
            Some(_) => Err(MemoryRepositoryError::Conflict),
            None => Err(MemoryRepositoryError::NotFound),
        };
    }
    get_in(transaction, space_id)?.ok_or(MemoryRepositoryError::NotFound)
}

/// The end of this conversation's latest succeeded run in the space that no
/// rewind undid.
pub(crate) fn run_cursor_in(
    transaction: &Transaction<'_>,
    space_id: MemorySpaceId,
    conversation_id: ConversationId,
) -> Result<u64, MemoryRepositoryError> {
    let cursor = transaction
        .query_row(
            "SELECT MAX(run.summary_window_end)
               FROM dynamic_memory_runs run
               JOIN dynamic_memory_summary_checkpoints checkpoint ON checkpoint.run_id = run.id
              WHERE run.space_id = ?1 AND run.conversation_id = ?2
                AND EXISTS (
                    SELECT 1 FROM dynamic_memory_run_attempts attempt
                     WHERE attempt.run_id = run.id AND attempt.status = 'succeeded'
                )
                AND NOT EXISTS (
                    SELECT 1 FROM dynamic_memory_suffix_rewinds rewind
                     WHERE rewind.conversation_id = run.conversation_id
                       AND rewind.space_id = run.space_id
                       AND rewind.applied_at >= checkpoint.settled_at
                )",
            params![space_id.to_string(), conversation_id.to_string()],
            |row| row.get::<_, Option<i64>>(0),
        )
        .map_err(storage)?
        .unwrap_or(0);
    u64::try_from(cursor).map_err(storage)
}

/// The window start of the run that wrote the space's current summary when
/// that run has checkpointed its summary but no attempt of it succeeded.
fn failed_summary_run_start_in(
    transaction: &Transaction<'_>,
    space_id: MemorySpaceId,
    conversation_id: ConversationId,
    window_end: i64,
) -> Result<Option<u64>, MemoryRepositoryError> {
    transaction
        .query_row(
            "SELECT run.summary_window_start
               FROM memory_summaries summary
               JOIN dynamic_memory_runs run
                 ON run.space_id = summary.space_id
                AND run.conversation_id = ?2
                AND run.summary_window_end = summary.window_end
               JOIN dynamic_memory_summary_checkpoints checkpoint
                 ON checkpoint.run_id = run.id AND checkpoint.settled_at = summary.updated_at
              WHERE summary.space_id = ?1 AND summary.window_end = ?3
                AND NOT EXISTS (
                    SELECT 1 FROM dynamic_memory_run_attempts attempt
                     WHERE attempt.run_id = run.id AND attempt.status = 'succeeded'
                )
              ORDER BY checkpoint.settled_at DESC
              LIMIT 1",
            params![
                space_id.to_string(),
                conversation_id.to_string(),
                window_end
            ],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(storage)?
        .map(|start| u64::try_from(start).map_err(storage))
        .transpose()
}

/// Where a conversation's next dynamic-memory window starts: the summary
/// window's end for the conversation that owns the space's summary, otherwise
/// the local run cursor, or the one another device reported when that is
/// further.
pub(crate) fn summary_cursor_in(
    transaction: &Transaction<'_>,
    space_id: MemorySpaceId,
    conversation_id: ConversationId,
) -> Result<u64, MemoryRepositoryError> {
    let summary_owner = transaction
        .query_row(
            "SELECT conversation_id, window_end FROM memory_summaries WHERE space_id = ?1",
            [space_id.to_string()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(storage)?;
    match summary_owner {
        None => Ok(0),
        Some((owner, window_end)) if owner == conversation_id.to_string() => {
            match failed_summary_run_start_in(transaction, space_id, conversation_id, window_end)? {
                None => u64::try_from(window_end).map_err(storage),
                Some(start) => {
                    let succeeded = run_cursor_in(transaction, space_id, conversation_id)?;
                    Ok(if succeeded > 0 { succeeded } else { start })
                }
            }
        }
        Some(_) => {
            let synced = transaction
                .query_row(
                    "SELECT window_end FROM memory_synced_cursors WHERE conversation_id = ?1",
                    [conversation_id.to_string()],
                    |row| row.get::<_, i64>(0),
                )
                .optional()
                .map_err(storage)?
                .map(u64::try_from)
                .transpose()
                .map_err(storage)?
                .unwrap_or(0);
            Ok(run_cursor_in(transaction, space_id, conversation_id)?.max(synced))
        }
    }
}

impl MemoryRepository for Database {
    fn create(
        &self,
        snapshot: MemorySpaceSnapshot,
    ) -> Result<MemorySpaceSnapshot, MemoryRepositoryError> {
        snapshot.validate()?;
        if snapshot.revision != Revision::INITIAL {
            return Err(MemoryRepositoryError::Invalid(
                lettuce_memory::MemoryValidationError::InvalidInitialRevision,
            ));
        }
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let inserted = transaction
            .execute(
                "INSERT OR IGNORE INTO memory_spaces (id, revision) VALUES (?1, ?2)",
                params![snapshot.id.to_string(), sql_revision(snapshot.revision)?],
            )
            .map_err(storage)?;
        if inserted != 1 {
            return Err(MemoryRepositoryError::AlreadyExists);
        }
        insert_items(&transaction, snapshot.id, &snapshot.items)?;
        transaction.commit().map_err(storage)?;
        Ok(snapshot)
    }

    fn get(&self, id: MemorySpaceId) -> Result<Option<MemorySpaceSnapshot>, MemoryRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(storage)?;
        let snapshot = get_in(&transaction, id)?;
        transaction.commit().map_err(storage)?;
        Ok(snapshot)
    }

    fn get_for_conversation(
        &self,
        conversation_id: ConversationId,
    ) -> Result<Option<MemorySpaceSnapshot>, MemoryRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(storage)?;
        let space_id = active_space_id_in(&transaction, conversation_id).map_err(storage)?;
        let snapshot = space_id
            .map(|space_id| get_in(&transaction, space_id))
            .transpose()?
            .flatten();
        if space_id.is_some() && snapshot.is_none() {
            return Err(storage("conversation memory space is missing"));
        }
        transaction.commit().map_err(storage)?;
        Ok(snapshot)
    }

    fn compare_and_apply(
        &self,
        change: MemoryChangeSet,
    ) -> Result<MemorySpaceSnapshot, MemoryRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let snapshot = compare_and_apply_in(&transaction, &change)?;
        transaction.commit().map_err(storage)?;
        Ok(snapshot)
    }
}

impl MemoryRetrievalRepository for Database {
    fn get_retrieval_access(
        &self,
        conversation_id: lettuce_types::ConversationId,
        turn_id: lettuce_types::GenerationTurnId,
        attempt_id: lettuce_types::GenerationAttemptId,
    ) -> Result<Option<MemoryRetrievalAccessReceipt>, MemoryRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        connection
            .query_row(
                "SELECT space_id,expected_revision,resulting_revision,selected_memory_ids_json,accessed_at,promoted_memory_ids_json
                   FROM memory_retrieval_accesses
                  WHERE conversation_id=?1 AND turn_id=?2 AND attempt_id=?3",
                params![
                    conversation_id.to_string(),
                    turn_id.to_string(),
                    attempt_id.to_string(),
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                },
            )
            .optional()
            .map_err(storage)?
            .map(|(space_id, expected, resulting, selected_json, accessed_at, promoted_json)| {
                Ok(MemoryRetrievalAccessReceipt {
                    access: MemoryRetrievalAccess {
                        conversation_id,
                        turn_id,
                        attempt_id,
                        space_id: parse_id(space_id)?,
                        expected_revision: parse_revision(expected)?,
                        selected_memory_ids: serde_json::from_str(&selected_json).map_err(storage)?,
                        accessed_at: TimestampMillis::new(accessed_at),
                    },
                    resulting_revision: parse_revision(resulting)?,
                    promoted_memory_ids: serde_json::from_str(&promoted_json).map_err(storage)?,
                })
            })
            .transpose()
    }

    fn apply_retrieval_access(
        &self,
        access: MemoryRetrievalAccess,
    ) -> Result<MemoryRetrievalAccessReceipt, MemoryRepositoryError> {
        if access.expected_revision.get() == 0
            || access.selected_memory_ids.is_empty()
            || access.selected_memory_ids.len() > 4096
            || access
                .selected_memory_ids
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != access.selected_memory_ids.len()
        {
            return Err(MemoryRepositoryError::Conflict);
        }
        let selected_json = serde_json::to_string(&access.selected_memory_ids).map_err(storage)?;
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let existing = transaction
            .query_row(
                "SELECT space_id,expected_revision,resulting_revision,selected_memory_ids_json,accessed_at,promoted_memory_ids_json
                   FROM memory_retrieval_accesses
                  WHERE conversation_id=?1 AND turn_id=?2 AND attempt_id=?3",
                params![
                    access.conversation_id.to_string(),
                    access.turn_id.to_string(),
                    access.attempt_id.to_string(),
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                },
            )
            .optional()
            .map_err(storage)?;
        if let Some((space_id, expected, resulting, stored_json, accessed_at, promoted_json)) =
            existing
        {
            if parse_id::<MemorySpaceId>(space_id)? != access.space_id
                || parse_revision(expected)? != access.expected_revision
                || stored_json != selected_json
                || TimestampMillis::new(accessed_at) != access.accessed_at
            {
                return Err(MemoryRepositoryError::Conflict);
            }
            transaction.commit().map_err(storage)?;
            return Ok(MemoryRetrievalAccessReceipt {
                access,
                resulting_revision: parse_revision(resulting)?,
                promoted_memory_ids: serde_json::from_str(&promoted_json).map_err(storage)?,
            });
        }
        let current_revision = transaction
            .query_row(
                "SELECT revision FROM memory_spaces WHERE id=?1",
                [access.space_id.to_string()],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(storage)?
            .ok_or(MemoryRepositoryError::NotFound)?;
        if parse_revision(current_revision)? < access.expected_revision {
            return Err(MemoryRepositoryError::Conflict);
        }
        let resulting_revision = access.expected_revision;
        let mut promoted_memory_ids = Vec::new();
        for id in &access.selected_memory_ids {
            let is_cold = transaction
                .query_row(
                    "SELECT is_cold FROM memory_items WHERE space_id=?1 AND id=?2 AND superseded_by IS NULL",
                    params![access.space_id.to_string(), id.to_string()],
                    |row| row.get::<_, bool>(0),
                )
                .optional()
                .map_err(storage)?;
            if is_cold == Some(true) {
                promoted_memory_ids.push(*id);
            }
        }
        let promoted_json = serde_json::to_string(&promoted_memory_ids).map_err(storage)?;
        for id in &access.selected_memory_ids {
            transaction
                .execute(
                    "UPDATE memory_items
                        SET importance=CASE WHEN is_cold=1 THEN 9000 ELSE min(10000,importance+2000) END,
                            access_count=min(4294967295,access_count+CASE WHEN is_cold=1 THEN 2 ELSE 1 END),
                            is_cold=0,
                            last_accessed_at=max(last_accessed_at,?3)
                      WHERE space_id=?1 AND id=?2 AND superseded_by IS NULL",
                    params![access.space_id.to_string(), id.to_string(), access.accessed_at.get()],
                )
                .map_err(storage)?;
        }
        transaction
            .execute(
                "INSERT INTO memory_retrieval_accesses (
                    conversation_id,turn_id,attempt_id,space_id,expected_revision,
                    resulting_revision,selected_memory_ids_json,accessed_at,promoted_memory_ids_json
                 ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![
                    access.conversation_id.to_string(),
                    access.turn_id.to_string(),
                    access.attempt_id.to_string(),
                    access.space_id.to_string(),
                    sql_revision(access.expected_revision)?,
                    sql_revision(resulting_revision)?,
                    selected_json,
                    access.accessed_at.get(),
                    promoted_json,
                ],
            )
            .map_err(storage)?;
        transaction.commit().map_err(storage)?;
        Ok(MemoryRetrievalAccessReceipt {
            access,
            resulting_revision,
            promoted_memory_ids,
        })
    }
}

impl MemorySummaryRepository for Database {
    fn get_summary(
        &self,
        space_id: MemorySpaceId,
    ) -> Result<Option<MemorySummary>, MemoryRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(storage)?;
        let summary = get_summary_in(&transaction, space_id)?;
        transaction.commit().map_err(storage)?;
        Ok(summary)
    }

    fn summary_cursor(
        &self,
        space_id: MemorySpaceId,
        conversation_id: ConversationId,
    ) -> Result<u64, MemoryRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(storage)?;
        let cursor = summary_cursor_in(&transaction, space_id, conversation_id)?;
        transaction.commit().map_err(storage)?;
        Ok(cursor)
    }

    fn compare_and_apply_summary(
        &self,
        change: MemorySummaryChange,
    ) -> Result<MemorySummaryCommit, MemoryRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let commit = compare_and_apply_summary_in(&transaction, &change)?;
        transaction.commit().map_err(storage)?;
        Ok(commit)
    }
}

fn get_pending_approval_in(
    connection: &rusqlite::Connection,
    conversation_id: ConversationId,
) -> Result<Option<DynamicMemoryPendingApproval>, MemoryRepositoryError> {
    connection
        .query_row(
            "SELECT prompted_message_count,pending,skipped,updated_at
               FROM dynamic_memory_pending_approvals WHERE conversation_id=?1",
            [conversation_id.to_string()],
            |row| {
                Ok(DynamicMemoryPendingApproval {
                    conversation_id,
                    prompted_message_count: u64::try_from(row.get::<_, i64>(0)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    pending: row.get(1)?,
                    skipped: row.get(2)?,
                    updated_at: TimestampMillis::new(row.get(3)?),
                })
            },
        )
        .optional()
        .map_err(storage)
}

impl DynamicMemoryApprovalRepository for Database {
    fn get_dynamic_memory_pending_approval(
        &self,
        conversation_id: ConversationId,
    ) -> Result<Option<DynamicMemoryPendingApproval>, MemoryRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        get_pending_approval_in(&connection, conversation_id)
    }

    fn prompt_dynamic_memory_if_due(
        &self,
        conversation_id: ConversationId,
        unsummarized_message_count: u64,
        message_interval: u32,
        at: TimestampMillis,
    ) -> Result<Option<DynamicMemoryPendingApproval>, MemoryRepositoryError> {
        if message_interval == 0 || unsummarized_message_count == 0 {
            return Err(storage("invalid dynamic memory approval input"));
        }
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let existing = get_pending_approval_in(&transaction, conversation_id)?;
        let baseline = existing
            .as_ref()
            .map_or(0, |approval| approval.prompted_message_count);
        if unsummarized_message_count.saturating_sub(baseline) < u64::from(message_interval) {
            transaction.commit().map_err(storage)?;
            return Ok(None);
        }
        transaction
            .execute(
                "INSERT INTO dynamic_memory_pending_approvals
                    (conversation_id,prompted_message_count,pending,skipped,updated_at)
                 VALUES (?1,?2,1,0,?3)
                 ON CONFLICT(conversation_id) DO UPDATE SET
                    prompted_message_count=excluded.prompted_message_count,
                    pending=1,
                    updated_at=excluded.updated_at",
                params![
                    conversation_id.to_string(),
                    i64::try_from(unsummarized_message_count).map_err(storage)?,
                    at.get(),
                ],
            )
            .map_err(storage)?;
        let approval = get_pending_approval_in(&transaction, conversation_id)?
            .ok_or_else(|| storage("missing dynamic memory approval"))?;
        transaction.commit().map_err(storage)?;
        Ok(Some(approval))
    }

    fn clear_dynamic_memory_pending_approval(
        &self,
        conversation_id: ConversationId,
    ) -> Result<(), MemoryRepositoryError> {
        self.connection()
            .map_err(storage)?
            .execute(
                "DELETE FROM dynamic_memory_pending_approvals WHERE conversation_id=?1",
                [conversation_id.to_string()],
            )
            .map_err(storage)?;
        Ok(())
    }

    fn skip_dynamic_memory_pending_approval(
        &self,
        conversation_id: ConversationId,
        at: TimestampMillis,
    ) -> Result<Option<DynamicMemoryPendingApproval>, MemoryRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        connection
            .execute(
                "UPDATE dynamic_memory_pending_approvals
                    SET pending=0,skipped=1,updated_at=?2
                  WHERE conversation_id=?1 AND pending=1",
                params![conversation_id.to_string(), at.get()],
            )
            .map_err(storage)?;
        get_pending_approval_in(&connection, conversation_id)
    }
}

#[cfg(test)]
mod tests {
    use lettuce_memory::{
        MemoryCategory, MemoryChangeSet, MemoryItem, MemoryRepository, MemoryRepositoryError,
        MemorySpaceSnapshot, MemorySummary, MemorySummaryChange, MemorySummaryRepository, Score,
    };
    use lettuce_types::{MemoryId, MemorySpaceId, MessageId, Revision, TimestampMillis};

    use super::Database;

    fn item(id: MemoryId, text: &str) -> MemoryItem {
        MemoryItem {
            id,
            short_id: lettuce_memory::MemoryShortId::derived(id),
            text: text.to_owned(),
            category: MemoryCategory::Other,
            source_message_id: None,
            source_role: None,
            observed_at: None,
            observed_time_precision: None,
            superseded_by: None,
            superseded_at: None,
            supersedes: Vec::new(),
            token_count: 3,
            is_cold: false,
            is_pinned: false,
            importance: Score::FULL,
            persistence_importance: Score::FULL,
            prompt_importance: Score::FULL,
            volatility: Score::LEGACY_VOLATILITY,
            access_count: 0,
            created_at: TimestampMillis::new(1),
            last_accessed_at: TimestampMillis::new(1),
        }
    }

    fn snapshot(id: MemorySpaceId, items: Vec<MemoryItem>) -> MemorySpaceSnapshot {
        MemorySpaceSnapshot {
            id,
            revision: Revision::INITIAL,
            items,
        }
    }

    #[test]
    fn creates_reads_and_cas_replaces_a_memory_space() {
        let database = Database::open_in_memory().expect("database");
        let space_id = MemorySpaceId::new();
        let mut first = item(MemoryId::new(), "first");
        first.source_message_id = Some(MessageId::new());
        first.source_role = Some(lettuce_conversations::MessageRole::User);
        first.observed_at = Some(TimestampMillis::new(42));
        first.observed_time_precision = Some("turn".to_owned());
        first.superseded_by = Some(MemoryId::new());
        first.superseded_at = Some(TimestampMillis::new(43));
        first.supersedes = vec![MemoryId::new()];
        let created = database
            .create(snapshot(space_id, vec![first.clone()]))
            .expect("create");
        assert_eq!(database.get(space_id).expect("get"), Some(created));

        let second = item(MemoryId::new(), "second");
        let changed = database
            .compare_and_apply(MemoryChangeSet {
                space_id,
                expected_revision: Revision::INITIAL,
                items: vec![first, second],
            })
            .expect("compare and apply");
        assert_eq!(changed.revision, Revision::new(2));
        assert_eq!(changed.items.len(), 2);
        assert_eq!(database.get(space_id).expect("get"), Some(changed));
    }

    #[test]
    fn stale_compare_and_apply_keeps_the_committed_snapshot() {
        let database = Database::open_in_memory().expect("database");
        let space_id = MemorySpaceId::new();
        let original = database
            .create(snapshot(space_id, vec![item(MemoryId::new(), "original")]))
            .expect("create");
        let current = database
            .compare_and_apply(MemoryChangeSet {
                space_id,
                expected_revision: original.revision,
                items: vec![item(MemoryId::new(), "current")],
            })
            .expect("first change");
        assert_eq!(
            database.compare_and_apply(MemoryChangeSet {
                space_id,
                expected_revision: original.revision,
                items: vec![item(MemoryId::new(), "stale")],
            }),
            Err(MemoryRepositoryError::Conflict)
        );
        assert_eq!(database.get(space_id).expect("get"), Some(current));
    }

    #[test]
    fn a_cycle_commit_keeps_retrieval_access_that_landed_after_its_read() {
        use lettuce_memory::{MemoryRetrievalAccess, MemoryRetrievalRepository};
        let database = Database::open_in_memory().expect("database");
        database
            .connection()
            .expect("connection")
            .execute_batch("PRAGMA foreign_keys = OFF")
            .expect("fixture mode");
        let space_id = MemorySpaceId::new();
        let hot = item(MemoryId::new(), "hot");
        let mut cold = item(MemoryId::new(), "cold");
        cold.is_cold = true;
        cold.importance = Score::from_basis_points(2_000).expect("score");
        let cooled = item(MemoryId::new(), "cooled by the cycle");
        let read = database
            .create(snapshot(
                space_id,
                vec![hot.clone(), cold.clone(), cooled.clone()],
            ))
            .expect("create");
        database
            .apply_retrieval_access(MemoryRetrievalAccess {
                conversation_id: lettuce_types::ConversationId::new(),
                turn_id: lettuce_types::GenerationTurnId::new(),
                attempt_id: lettuce_types::GenerationAttemptId::new(),
                space_id,
                expected_revision: read.revision,
                selected_memory_ids: vec![hot.id, cold.id, cooled.id],
                accessed_at: TimestampMillis::new(90),
            })
            .expect("retrieval between the read and the commit");
        let mut soft_deleted = cooled.clone();
        soft_deleted.is_cold = true;
        soft_deleted.importance = Score::from_basis_points(2_000).expect("score");
        let created = item(MemoryId::new(), "new fact");
        let committed = database
            .compare_and_apply(MemoryChangeSet {
                space_id,
                expected_revision: read.revision,
                items: vec![hot.clone(), cold.clone(), soft_deleted, created.clone()],
            })
            .expect("cycle commit");
        let find = |id| {
            committed
                .items
                .iter()
                .find(|memory| memory.id == id)
                .expect("memory")
                .clone()
        };
        let hot_after = find(hot.id);
        assert_eq!(hot_after.access_count, 1);
        assert_eq!(hot_after.last_accessed_at, TimestampMillis::new(90));
        let promoted = find(cold.id);
        assert!(!promoted.is_cold);
        assert_eq!(promoted.access_count, 2);
        assert_eq!(
            promoted.importance,
            Score::from_basis_points(9_000).expect("score")
        );
        let cooled_after = find(cooled.id);
        assert!(cooled_after.is_cold);
        assert_eq!(cooled_after.access_count, 1);
        assert_eq!(find(created.id), created);
    }

    #[test]
    fn retrieval_access_time_never_moves_back_and_a_same_time_access_survives_a_commit() {
        use lettuce_memory::{MemoryRetrievalAccess, MemoryRetrievalRepository};
        let database = Database::open_in_memory().expect("database");
        database
            .connection()
            .expect("connection")
            .execute_batch("PRAGMA foreign_keys = OFF")
            .expect("fixture mode");
        let space_id = MemorySpaceId::new();
        let mut memory = item(MemoryId::new(), "memory");
        memory.last_accessed_at = TimestampMillis::new(100);
        let read = database
            .create(snapshot(space_id, vec![memory.clone()]))
            .expect("create");
        database
            .apply_retrieval_access(MemoryRetrievalAccess {
                conversation_id: lettuce_types::ConversationId::new(),
                turn_id: lettuce_types::GenerationTurnId::new(),
                attempt_id: lettuce_types::GenerationAttemptId::new(),
                space_id,
                expected_revision: read.revision,
                selected_memory_ids: vec![memory.id],
                accessed_at: TimestampMillis::new(60),
            })
            .expect("access after a clock step back");
        let accessed = database.get(space_id).expect("get").expect("space");
        assert_eq!(
            accessed.items[0].last_accessed_at,
            TimestampMillis::new(100)
        );
        assert_eq!(accessed.items[0].access_count, 1);
        let committed = database
            .compare_and_apply(MemoryChangeSet {
                space_id,
                expected_revision: read.revision,
                items: vec![memory],
            })
            .expect("commit from the earlier read");
        assert_eq!(committed.items[0].access_count, 1);
        assert_eq!(
            database.get(space_id).expect("get").expect("space"),
            committed
        );
    }

    #[test]
    fn retrieval_access_updates_rows_without_conflicting_with_a_memory_cycle() {
        use lettuce_memory::{MemoryRetrievalAccess, MemoryRetrievalRepository};
        let database = Database::open_in_memory().expect("database");
        database
            .connection()
            .expect("connection")
            .execute_batch("PRAGMA foreign_keys = OFF")
            .expect("fixture mode");
        let space_id = MemorySpaceId::new();
        let kept = MemoryId::new();
        let mut cold = item(MemoryId::new(), "cold");
        cold.is_cold = true;
        let created = database
            .create(snapshot(space_id, vec![item(kept, "kept"), cold.clone()]))
            .expect("create");
        let cycle = database
            .compare_and_apply(MemoryChangeSet {
                space_id,
                expected_revision: created.revision,
                items: vec![item(kept, "kept"), cold.clone()],
            })
            .expect("cycle commit");
        let removed = MemoryId::new();
        let receipt = database
            .apply_retrieval_access(MemoryRetrievalAccess {
                conversation_id: lettuce_types::ConversationId::new(),
                turn_id: lettuce_types::GenerationTurnId::new(),
                attempt_id: lettuce_types::GenerationAttemptId::new(),
                space_id,
                expected_revision: created.revision,
                selected_memory_ids: vec![kept, cold.id, removed],
                accessed_at: TimestampMillis::new(90),
            })
            .expect("stale retrieval access still applies");
        assert_eq!(receipt.resulting_revision, created.revision);
        assert_eq!(receipt.promoted_memory_ids, vec![cold.id]);
        let stored = database.get(space_id).expect("get").expect("space");
        assert_eq!(stored.revision, cycle.revision);
        let promoted = stored
            .items
            .iter()
            .find(|memory| memory.id == cold.id)
            .expect("promoted");
        assert!(!promoted.is_cold);
        assert_eq!(promoted.access_count, 2);
        assert_eq!(promoted.last_accessed_at, TimestampMillis::new(90));
        database
            .compare_and_apply(MemoryChangeSet {
                space_id,
                expected_revision: cycle.revision,
                items: stored.items.clone(),
            })
            .expect("a cycle that read before the retrieval still commits");
    }

    #[test]
    fn item_collision_rolls_back_new_space_creation() {
        let database = Database::open_in_memory().expect("database");
        let item_id = MemoryId::new();
        database
            .create(snapshot(MemorySpaceId::new(), vec![item(item_id, "owned")]))
            .expect("first space");
        let second_space_id = MemorySpaceId::new();
        assert!(matches!(
            database.create(snapshot(second_space_id, vec![item(item_id, "collision")])),
            Err(MemoryRepositoryError::Failure(_))
        ));
        assert_eq!(database.get(second_space_id).expect("get"), None);
    }

    #[test]
    fn summary_cas_persists_ordered_cursor_and_advances_root_revision() {
        let database = Database::open_in_memory().expect("database");
        database
            .connection()
            .expect("connection")
            .execute_batch("PRAGMA foreign_keys = OFF")
            .expect("fixture mode");
        let space_id = MemorySpaceId::new();
        let created = database
            .create(snapshot(space_id, vec![item(MemoryId::new(), "memory")]))
            .expect("create");
        database
            .connection()
            .expect("connection")
            .execute(
                "INSERT INTO conversation_memory_spaces (conversation_id, space_id) VALUES (?1, ?2)",
                rusqlite::params!["conversation", space_id.to_string()],
            )
            .expect("binding");
        let source_message_ids = vec![MessageId::new(), MessageId::new()];
        let summary = MemorySummary {
            space_id,
            text: "Mira learned the route.".to_owned(),
            token_count: 6,
            window_start: 0,
            window_end: 2,
            source_message_ids,
            updated_at: TimestampMillis::new(50),
        };
        let committed = database
            .compare_and_apply_summary(MemorySummaryChange {
                expected_revision: created.revision,
                summary: summary.clone(),
            })
            .expect("summary commit");
        assert_eq!(committed.memory.revision, Revision::new(2));
        assert_eq!(committed.memory.items, created.items);
        assert_eq!(committed.summary, summary);
        assert_eq!(database.get_summary(space_id).expect("get"), Some(summary));
    }

    #[test]
    fn long_summaries_windows_and_memories_are_kept_whole() {
        let database = Database::open_in_memory().expect("database");
        database
            .connection()
            .expect("connection")
            .execute_batch("PRAGMA foreign_keys = OFF")
            .expect("fixture mode");
        let space_id = MemorySpaceId::new();
        let long_memory = "m".repeat(64 * 1024);
        let created = database
            .create(snapshot(
                space_id,
                vec![item(MemoryId::new(), &long_memory)],
            ))
            .expect("create a memory over 16 KiB");
        assert_eq!(created.items[0].text, long_memory);
        database
            .connection()
            .expect("connection")
            .execute(
                "INSERT INTO conversation_memory_spaces (conversation_id, space_id) VALUES (?1, ?2)",
                rusqlite::params!["conversation", space_id.to_string()],
            )
            .expect("binding");
        let source_message_ids = (0..1100).map(|_| MessageId::new()).collect::<Vec<_>>();
        let summary = MemorySummary {
            space_id,
            text: "s".repeat(20_000),
            token_count: 5_000,
            window_start: 0,
            window_end: 1100,
            source_message_ids,
            updated_at: TimestampMillis::new(50),
        };
        database
            .compare_and_apply_summary(MemorySummaryChange {
                expected_revision: created.revision,
                summary: summary.clone(),
            })
            .expect("summary over 6000 bytes and 1024 messages");
        assert_eq!(database.get_summary(space_id).expect("get"), Some(summary));
    }

    #[test]
    fn stale_summary_cas_preserves_current_summary() {
        let database = Database::open_in_memory().expect("database");
        database
            .connection()
            .expect("connection")
            .execute_batch("PRAGMA foreign_keys = OFF")
            .expect("fixture mode");
        let space_id = MemorySpaceId::new();
        let created = database.create(snapshot(space_id, vec![])).expect("create");
        database
            .connection()
            .expect("connection")
            .execute(
                "INSERT INTO conversation_memory_spaces (conversation_id, space_id) VALUES (?1, ?2)",
                rusqlite::params!["conversation", space_id.to_string()],
            )
            .expect("binding");
        let source_id = MessageId::new();
        let current = MemorySummary {
            space_id,
            text: "Current summary".to_owned(),
            token_count: 2,
            window_start: 0,
            window_end: 1,
            source_message_ids: vec![source_id],
            updated_at: TimestampMillis::new(10),
        };
        database
            .compare_and_apply_summary(MemorySummaryChange {
                expected_revision: created.revision,
                summary: current.clone(),
            })
            .expect("first commit");
        assert_eq!(
            database.compare_and_apply_summary(MemorySummaryChange {
                expected_revision: created.revision,
                summary: MemorySummary {
                    text: "Stale summary".to_owned(),
                    ..current.clone()
                },
            }),
            Err(MemoryRepositoryError::Conflict)
        );
        assert_eq!(database.get_summary(space_id).expect("get"), Some(current));
    }

    #[test]
    fn companion_conversations_share_one_pool_and_keep_their_own_summary_cursor() {
        let database = Database::open_in_memory().expect("database");
        database
            .connection()
            .expect("connection")
            .execute_batch("PRAGMA foreign_keys = OFF")
            .expect("fixture mode");
        let character_id = lettuce_types::CharacterId::new();
        let first = lettuce_types::ConversationId::new();
        let second = lettuce_types::ConversationId::new();
        let (first_space, second_space) = {
            let mut connection = database.connection().expect("connection");
            let transaction = connection.transaction().expect("transaction");
            let first_space = crate::memory::memory_adapter::join_companion_pool_in(
                &transaction,
                first,
                character_id,
            )
            .expect("first binding");
            let second_space = crate::memory::memory_adapter::join_companion_pool_in(
                &transaction,
                second,
                character_id,
            )
            .expect("second binding");
            transaction.commit().expect("commit");
            (first_space, second_space)
        };
        assert_eq!(first_space, second_space);
        database
            .connection()
            .expect("connection")
            .execute(
                "INSERT INTO memory_summaries (space_id, conversation_id, text, token_count, window_start, window_end, updated_at) VALUES (?1, ?2, 'Shared summary', 2, 0, 2, 10)",
                rusqlite::params![first_space.to_string(), first.to_string()],
            )
            .expect("summary");
        assert_eq!(
            lettuce_memory::MemorySummaryRepository::summary_cursor(&database, first_space, first)
                .expect("cursor"),
            2
        );
        assert_eq!(
            database
                .summary_cursor(first_space, second)
                .expect("cursor"),
            0
        );
        database
            .connection()
            .expect("connection")
            .execute_batch(&format!(
                "PRAGMA foreign_keys = OFF; INSERT INTO memory_synced_cursors (conversation_id, window_end) VALUES ('{second}', 7)"
            ))
            .expect("synced cursor");
        assert_eq!(
            database
                .summary_cursor(first_space, second)
                .expect("cursor from another device"),
            7
        );
        assert_eq!(
            database
                .summary_cursor(first_space, first)
                .expect("owner cursor"),
            2
        );

        let private_space = MemorySpaceId::new();
        let connection = database.connection().expect("connection");
        connection
            .execute(
                "INSERT INTO memory_spaces (id, revision) VALUES (?1, 1)",
                [private_space.to_string()],
            )
            .expect("space");
        connection
            .execute(
                "INSERT INTO conversation_memory_spaces (conversation_id, space_id) VALUES (?1, ?2)",
                rusqlite::params![lettuce_types::ConversationId::new().to_string(), private_space.to_string()],
            )
            .expect("private binding");
        assert!(
            connection
                .execute(
                    "INSERT INTO conversation_memory_spaces (conversation_id, space_id) VALUES (?1, ?2)",
                    rusqlite::params![lettuce_types::ConversationId::new().to_string(), private_space.to_string()],
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "INSERT INTO conversation_memory_spaces (conversation_id, space_id) VALUES (?1, ?2)",
                    rusqlite::params![lettuce_types::ConversationId::new().to_string(), first_space.to_string()],
                )
                .is_err(),
            "a pool is never a conversation's own space"
        );
    }
}
