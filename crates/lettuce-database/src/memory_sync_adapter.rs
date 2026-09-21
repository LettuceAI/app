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
     SELECT 'conversation:' || binding.conversation_id AS owner, binding.space_id FROM conversation_memory_spaces binding
      WHERE NOT EXISTS (SELECT 1 FROM companion_memory_pools pool WHERE pool.space_id = binding.space_id)";

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
/// synced values (keeping its local short id), a new one is appended with a
/// free short id. Returns `false` for an item id that belongs to another
/// space here.
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
    let placed: Option<String> = transaction
        .query_row(
            "SELECT space_id FROM memory_items WHERE id = ?1",
            [item_id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(storage)?;
    if placed
        .as_ref()
        .is_some_and(|space| *space != space_id.to_string())
    {
        return Ok(false);
    }
    let mut space =
        memory_adapter::get_in(transaction, space_id)?.ok_or(MemoryRepositoryError::NotFound)?;
    let local = space.items.iter().position(|stored| stored.id == item_id);
    let short_id = match local {
        Some(index) => space.items[index].short_id,
        None => {
            let taken = space
                .items
                .iter()
                .map(|stored| stored.short_id.get())
                .collect::<std::collections::HashSet<_>>();
            let derived = MemoryShortId::derived(item_id);
            if taken.contains(&derived.get()) {
                (0..=999_999)
                    .find(|candidate| !taken.contains(candidate))
                    .and_then(MemoryShortId::new)
                    .ok_or_else(|| storage("no free memory short id"))?
            } else {
                derived
            }
        }
    };
    let placed_item = MemoryItem {
        short_id,
        ..item.clone()
    };
    match local {
        Some(index) => space.items[index] = placed_item,
        None => space.items.push(placed_item),
    }
    space.validate()?;
    replace_items(transaction, space_id, &space.items)?;
    Ok(true)
}

fn replace_items(
    transaction: &Transaction<'_>,
    space_id: MemorySpaceId,
    items: &[MemoryItem],
) -> Result<(), MemoryRepositoryError> {
    transaction
        .execute(
            "DELETE FROM memory_items WHERE space_id = ?1",
            [space_id.to_string()],
        )
        .map_err(storage)?;
    memory_adapter::insert_items(transaction, space_id, items)?;
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
    let Some(mut space) = memory_adapter::get_in(transaction, space_id)? else {
        return Ok(true);
    };
    let before = space.items.len();
    space.items.retain(|item| item.id != item_id);
    if space.items.len() != before {
        replace_items(transaction, space_id, &space.items)?;
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
    transaction
        .execute(
            "UPDATE memory_spaces SET revision = revision + 1 WHERE id = ?1",
            params![space_id.to_string()],
        )
        .map_err(storage)?;
    Ok(())
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
        transaction
            .execute(
                "UPDATE memory_spaces SET revision = revision + 1 WHERE id = ?1",
                params![space_id.to_string()],
            )
            .map_err(storage)?;
    }
    Ok(true)
}

pub(crate) fn sync_memory_cursor_ids(connection: &Connection) -> rusqlite::Result<Vec<String>> {
    connection
        .prepare(
            "SELECT binding.conversation_id FROM conversation_memory_spaces binding
               JOIN companion_memory_pools pool ON pool.space_id = binding.space_id
              ORDER BY 1",
        )?
        .query_map([], |row| row.get(0))?
        .collect()
}

/// A pool conversation's dynamic-memory cursor, when it has one. Only pool
/// conversations that do not own the pool's summary need it exchanged, but
/// the owner's cursor travels too so both devices agree.
pub(crate) fn sync_load_memory_cursor(
    transaction: &Transaction<'_>,
    conversation_id: lettuce_types::ConversationId,
) -> Result<Option<u64>, MemoryRepositoryError> {
    let space: Option<String> = transaction
        .query_row(
            "SELECT binding.space_id FROM conversation_memory_spaces binding
               JOIN companion_memory_pools pool ON pool.space_id = binding.space_id
              WHERE binding.conversation_id = ?1",
            [conversation_id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(storage)?;
    let Some(space) = space else {
        return Ok(None);
    };
    let cursor = memory_adapter::summary_cursor_in(
        transaction,
        space.parse().map_err(storage)?,
        conversation_id,
    )?;
    Ok((cursor > 0).then_some(cursor))
}

/// Raises the cursor another device reported; cursors only move forward
/// through sync.
pub(crate) fn sync_raise_memory_cursor(
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
             ON CONFLICT(conversation_id) DO UPDATE SET window_end = max(window_end, excluded.window_end)",
            params![
                conversation_id.to_string(),
                i64::try_from(window_end).map_err(storage)?
            ],
        )
        .map_err(storage)?;
    Ok(())
}
