//! Memory exchanged by sync.
//!
//! Each device picks its own space ids, so memory is exchanged under the
//! space's owner: `conversation:<id>` for a conversation's own space and
//! `pool:<character id>` for a companion character's shared pool. Every item
//! is its own entity (`<owner>/<memory id>`), so a retrieval that touches a
//! few items journals only those, concurrent additions on two devices are
//! both kept and a deleted item stays deleted. Short ids (the numbers memory
//! tools use) and ordinals are numbered by each device. The summary is one
//! entity per owner. The dynamic-memory cursor is the summary window, which
//! counts path messages, so the summary's owner conversation continues where
//! the other device stopped.

use lettuce_memory::{MemoryItem, MemoryRepositoryError, MemoryShortId, MemorySummary};
use lettuce_types::{MemoryId, MemorySpaceId};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::memory_adapter;

fn storage(error: impl std::fmt::Debug) -> MemoryRepositoryError {
    MemoryRepositoryError::Failure(format!("{error:?}"))
}

fn exchanged_space_id() -> MemorySpaceId {
    MemorySpaceId::from_uuid(uuid::Uuid::nil())
}

const OWNERS: &str = "SELECT 'pool:' || character_id AS owner, space_id FROM companion_memory_pools
     UNION ALL
     SELECT 'conversation:' || conversation_id AS owner, space_id FROM conversation_memory_spaces
      WHERE pooled = 0";

pub(crate) fn valid_owner(owner: &str) -> bool {
    match owner.split_once(':') {
        Some(("pool", id)) => id.parse::<lettuce_types::CharacterId>().is_ok(),
        Some(("conversation", id)) => id.parse::<lettuce_types::ConversationId>().is_ok(),
        _ => false,
    }
}

pub(crate) fn split_item_id(id: &str) -> Option<(&str, MemoryId)> {
    let (owner, item) = id.split_once('/')?;
    valid_owner(owner).then_some(())?;
    Some((owner, item.parse().ok()?))
}

fn local_space(
    transaction: &Transaction<'_>,
    owner: &str,
) -> Result<Option<MemorySpaceId>, MemoryRepositoryError> {
    let space: Option<String> = transaction
        .query_row(
            &format!("SELECT space_id FROM ({OWNERS}) WHERE owner = ?1"),
            [owner],
            |row| row.get(0),
        )
        .optional()
        .map_err(storage)?;
    space.map(|id| id.parse().map_err(storage)).transpose()
}

pub(crate) fn sync_memory_item_ids(connection: &Connection) -> rusqlite::Result<Vec<String>> {
    connection
        .prepare(&format!(
            "SELECT owners.owner || '/' || item.id FROM memory_items item
               JOIN ({OWNERS}) owners ON owners.space_id = item.space_id
              ORDER BY 1"
        ))?
        .query_map([], |row| row.get(0))?
        .collect()
}

pub(crate) fn sync_memory_summary_owners(connection: &Connection) -> rusqlite::Result<Vec<String>> {
    connection
        .prepare(&format!(
            "SELECT owners.owner FROM memory_summaries summary
               JOIN ({OWNERS}) owners ON owners.space_id = summary.space_id
              ORDER BY 1"
        ))?
        .query_map([], |row| row.get(0))?
        .collect()
}

fn exchanged_item(item: &MemoryItem) -> MemoryItem {
    MemoryItem {
        short_id: MemoryShortId::new(0).unwrap_or(item.short_id),
        ..item.clone()
    }
}

pub(crate) fn sync_load_memory_item(
    transaction: &Transaction<'_>,
    id: &str,
) -> Result<Option<MemoryItem>, MemoryRepositoryError> {
    let (owner, item_id) = split_item_id(id).ok_or_else(|| storage("invalid memory item id"))?;
    let Some(space_id) = local_space(transaction, owner)? else {
        return Ok(None);
    };
    Ok(memory_adapter::get_item_in(transaction, space_id, item_id)?
        .map(|item| exchanged_item(&item)))
}

/// Writes one synced item into its owner's space: an existing item takes the
/// synced values in place (keeping its local short id and ordinal), a new one
/// takes a free ordinal and its derived short id, or the first free one. A
/// full space waits until a synced deletion or a local trim frees room.
/// Returns `false` for an item id that belongs to another space here.
pub(crate) fn sync_put_memory_item(
    transaction: &Transaction<'_>,
    id: &str,
    item: &MemoryItem,
) -> Result<bool, MemoryRepositoryError> {
    let (owner, item_id) = split_item_id(id).ok_or_else(|| storage("invalid memory item id"))?;
    if item.id != item_id {
        return Err(MemoryRepositoryError::Invalid(
            lettuce_memory::MemoryValidationError::DuplicateItemId,
        ));
    }
    let Some(space_id) = local_space(transaction, owner)? else {
        return Err(MemoryRepositoryError::NotFound);
    };
    lettuce_memory::MemorySpaceSnapshot {
        id: space_id,
        revision: lettuce_types::Revision::INITIAL,
        items: vec![item.clone()],
    }
    .validate()?;
    let placed: Option<(String, i64, i64, String, u32)> = transaction
        .query_row(
            "SELECT space_id, ordinal, short_id, text, token_count FROM memory_items WHERE id = ?1",
            [item_id.to_string()],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()
        .map_err(storage)?;
    let token_count = match &placed {
        Some((_, _, _, text, stored)) if *text == item.text => *stored,
        _ => item.token_count,
    };
    let (ordinal, short_id) = match placed {
        Some((space, _, _, _, _)) if space != space_id.to_string() => return Ok(false),
        Some((_, ordinal, short_id, _, _)) => {
            transaction
                .execute(
                    "DELETE FROM memory_items WHERE space_id = ?1 AND id = ?2",
                    params![space_id.to_string(), item_id.to_string()],
                )
                .map_err(storage)?;
            (
                ordinal,
                u32::try_from(short_id)
                    .ok()
                    .and_then(MemoryShortId::new)
                    .ok_or_else(|| storage("invalid memory short id"))?,
            )
        }
        None => {
            let used =
                |sql: &str| -> Result<std::collections::HashSet<i64>, MemoryRepositoryError> {
                    transaction
                        .prepare(sql)
                        .and_then(|mut statement| {
                            statement
                                .query_map([space_id.to_string()], |row| row.get::<_, i64>(0))?
                                .collect::<rusqlite::Result<std::collections::HashSet<_>>>()
                        })
                        .map_err(storage)
                };
            let ordinals = used("SELECT ordinal FROM memory_items WHERE space_id = ?1")?;
            if ordinals.len() >= lettuce_memory::MAX_MEMORY_ITEMS {
                return Err(MemoryRepositoryError::NotFound);
            }
            let short_ids = used("SELECT short_id FROM memory_items WHERE space_id = ?1")?;
            let ordinal = (0..)
                .find(|candidate| !ordinals.contains(candidate))
                .ok_or_else(|| storage("no free memory ordinal"))?;
            let derived = MemoryShortId::derived(item_id);
            let short_id = if short_ids.contains(&i64::from(derived.get())) {
                (0..MemoryShortId::SPACE)
                    .find(|candidate| !short_ids.contains(&i64::from(*candidate)))
                    .and_then(MemoryShortId::new)
                    .ok_or_else(|| storage("no free memory short id"))?
            } else {
                derived
            };
            (ordinal, short_id)
        }
    };
    memory_adapter::insert_item_at(
        transaction,
        space_id,
        ordinal,
        &MemoryItem {
            short_id,
            token_count,
            ..item.clone()
        },
    )?;
    bump_revision(transaction, space_id)?;
    Ok(true)
}

fn bump_revision(
    transaction: &Transaction<'_>,
    space_id: MemorySpaceId,
) -> Result<(), MemoryRepositoryError> {
    transaction
        .execute(
            "UPDATE memory_spaces SET revision = revision + 1 WHERE id = ?1",
            [space_id.to_string()],
        )
        .map_err(storage)?;
    Ok(())
}

pub(crate) fn sync_delete_memory_item(
    transaction: &Transaction<'_>,
    id: &str,
) -> Result<bool, MemoryRepositoryError> {
    let (owner, item_id) = split_item_id(id).ok_or_else(|| storage("invalid memory item id"))?;
    let Some(space_id) = local_space(transaction, owner)? else {
        return Ok(true);
    };
    let removed = transaction
        .execute(
            "DELETE FROM memory_items WHERE space_id = ?1 AND id = ?2",
            params![space_id.to_string(), item_id.to_string()],
        )
        .map_err(storage)?;
    if removed > 0 {
        bump_revision(transaction, space_id)?;
    }
    Ok(true)
}

pub(crate) fn sync_load_memory_summary(
    transaction: &Transaction<'_>,
    owner: &str,
) -> Result<Option<MemorySummary>, MemoryRepositoryError> {
    let Some(space_id) = local_space(transaction, owner)? else {
        return Ok(None);
    };
    Ok(
        memory_adapter::get_summary_in(transaction, space_id)?.map(|summary| MemorySummary {
            space_id: exchanged_space_id(),
            ..summary
        }),
    )
}

/// Replaces the owner's summary once its space and source messages exist.
pub(crate) fn sync_replace_memory_summary(
    transaction: &Transaction<'_>,
    owner: &str,
    summary: &MemorySummary,
) -> Result<(), MemoryRepositoryError> {
    summary.validate()?;
    let Some(space_id) = local_space(transaction, owner)? else {
        return Err(MemoryRepositoryError::NotFound);
    };
    for message in &summary.source_message_ids {
        let present: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM conversation_messages WHERE id = ?1)",
                [message.to_string()],
                |row| row.get(0),
            )
            .map_err(storage)?;
        if !present {
            return Err(MemoryRepositoryError::NotFound);
        }
    }
    if sync_load_memory_summary(transaction, owner)?.as_ref() == Some(summary) {
        return Ok(());
    }
    memory_adapter::replace_summary_in(
        transaction,
        space_id,
        Some(&MemorySummary {
            space_id,
            ..summary.clone()
        }),
    )?;
    bump_revision(transaction, space_id)
}

pub(crate) fn sync_delete_memory_summary(
    transaction: &Transaction<'_>,
    owner: &str,
) -> Result<bool, MemoryRepositoryError> {
    let Some(space_id) = local_space(transaction, owner)? else {
        return Ok(true);
    };
    if memory_adapter::get_summary_in(transaction, space_id)?.is_some() {
        memory_adapter::replace_summary_in(transaction, space_id, None)?;
        bump_revision(transaction, space_id)?;
    }
    Ok(true)
}

pub(crate) fn sync_memory_cursor_ids(connection: &Connection) -> rusqlite::Result<Vec<String>> {
    connection
        .prepare(
            "SELECT conversation_id FROM conversation_memory_spaces WHERE pooled = 1 ORDER BY 1",
        )?
        .query_map([], |row| row.get(0))?
        .collect()
}

/// A pool conversation's dynamic-memory cursor from this device's own runs,
/// when it has one. Only run cursors are exchanged, so a cursor received from
/// another device is never echoed back.
pub(crate) fn sync_load_memory_cursor(
    transaction: &Transaction<'_>,
    conversation_id: lettuce_types::ConversationId,
) -> Result<Option<u64>, MemoryRepositoryError> {
    let space: Option<String> = transaction
        .query_row(
            "SELECT space_id FROM conversation_memory_spaces
              WHERE conversation_id = ?1 AND pooled = 1",
            [conversation_id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(storage)?;
    let Some(space) = space else {
        return Ok(None);
    };
    let cursor = memory_adapter::run_cursor_in(
        transaction,
        space.parse().map_err(storage)?,
        conversation_id,
    )?;
    Ok((cursor > 0).then_some(cursor))
}

/// Stores the run cursor another device reported (last writer wins, so a
/// rewind there lowers it here); a local rewind clears it.
pub(crate) fn sync_store_memory_cursor(
    transaction: &Transaction<'_>,
    conversation_id: lettuce_types::ConversationId,
    window_end: u64,
) -> Result<(), MemoryRepositoryError> {
    let present: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ?1)",
            [conversation_id.to_string()],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if !present {
        return Err(MemoryRepositoryError::NotFound);
    }
    transaction
        .execute(
            "INSERT INTO memory_synced_cursors (conversation_id, window_end) VALUES (?1, ?2)
             ON CONFLICT(conversation_id) DO UPDATE SET window_end = excluded.window_end",
            params![
                conversation_id.to_string(),
                i64::try_from(window_end).map_err(storage)?
            ],
        )
        .map_err(storage)?;
    Ok(())
}
