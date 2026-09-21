//! Memory spaces exchanged by sync.
//!
//! Each device picks its own space ids, so a space is exchanged under its
//! owner: `conversation:<id>` for a conversation's own space and
//! `pool:<character id>` for a companion character's shared pool. The
//! exchanged form is the items and the summary; the space revision stays
//! local. The dynamic-memory cursor is the summary window, which counts path
//! messages, so a device that receives a space continues where the other
//! stopped.

use lettuce_memory::{MemoryItem, MemoryRepositoryError, MemorySpaceSnapshot, MemorySummary};
use lettuce_types::{MemorySpaceId, Revision};
use rusqlite::{Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};

use crate::memory_adapter;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SyncMemorySpace {
    pub items: Vec<MemoryItem>,
    pub summary: Option<MemorySummary>,
}

fn storage(error: impl std::fmt::Debug) -> MemoryRepositoryError {
    MemoryRepositoryError::Failure(format!("{error:?}"))
}

fn exchanged_space_id() -> MemorySpaceId {
    MemorySpaceId::from_uuid(uuid::Uuid::nil())
}

pub(crate) fn sync_memory_space_ids(connection: &Connection) -> rusqlite::Result<Vec<String>> {
    connection
        .prepare(
            "SELECT 'pool:' || character_id FROM companion_memory_pools
             UNION ALL
             SELECT 'conversation:' || binding.conversation_id FROM conversation_memory_spaces binding
              WHERE NOT EXISTS (SELECT 1 FROM companion_memory_pools pool WHERE pool.space_id = binding.space_id)
             ORDER BY 1",
        )?
        .query_map([], |row| row.get(0))?
        .collect()
}

pub(crate) fn valid_memory_space_id(id: &str) -> bool {
    match id.split_once(':') {
        Some(("pool", owner)) => owner.parse::<lettuce_types::CharacterId>().is_ok(),
        Some(("conversation", owner)) => owner.parse::<lettuce_types::ConversationId>().is_ok(),
        _ => false,
    }
}

fn local_space(
    transaction: &Transaction<'_>,
    id: &str,
) -> Result<Option<MemorySpaceId>, MemoryRepositoryError> {
    let space: Option<String> = match id.split_once(':') {
        Some(("pool", owner)) => transaction
            .query_row(
                "SELECT space_id FROM companion_memory_pools WHERE character_id = ?1",
                [owner],
                |row| row.get(0),
            )
            .optional(),
        Some(("conversation", owner)) => transaction
            .query_row(
                "SELECT binding.space_id FROM conversation_memory_spaces binding
                  WHERE binding.conversation_id = ?1
                    AND NOT EXISTS (SELECT 1 FROM companion_memory_pools pool WHERE pool.space_id = binding.space_id)",
                [owner],
                |row| row.get(0),
            )
            .optional(),
        _ => return Err(storage("invalid memory space owner")),
    }
    .map_err(storage)?;
    space.map(|id| id.parse().map_err(storage)).transpose()
}

pub(crate) fn sync_load_memory_space(
    transaction: &Transaction<'_>,
    id: &str,
) -> Result<Option<SyncMemorySpace>, MemoryRepositoryError> {
    let Some(space_id) = local_space(transaction, id)? else {
        return Ok(None);
    };
    let snapshot =
        memory_adapter::get_in(transaction, space_id)?.ok_or(MemoryRepositoryError::NotFound)?;
    let summary =
        memory_adapter::get_summary_in(transaction, space_id)?.map(|summary| MemorySummary {
            space_id: exchanged_space_id(),
            ..summary
        });
    Ok(Some(SyncMemorySpace {
        items: snapshot.items,
        summary,
    }))
}

/// Replaces the owner's local space with the synced items and summary. The
/// owner's space appears with its conversation root, and a summary waits
/// for its source messages.
pub(crate) fn sync_replace_memory_space(
    transaction: &Transaction<'_>,
    id: &str,
    space: &SyncMemorySpace,
) -> Result<(), MemoryRepositoryError> {
    let Some(space_id) = local_space(transaction, id)? else {
        return Err(MemoryRepositoryError::NotFound);
    };
    MemorySpaceSnapshot {
        id: space_id,
        revision: Revision::INITIAL,
        items: space.items.clone(),
    }
    .validate()?;
    if let Some(summary) = &space.summary {
        summary.validate()?;
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
    }
    if sync_load_memory_space(transaction, id)?.as_ref() == Some(space) {
        return Ok(());
    }
    transaction
        .execute(
            "DELETE FROM memory_items WHERE space_id = ?1",
            [space_id.to_string()],
        )
        .map_err(storage)?;
    memory_adapter::insert_items(transaction, space_id, &space.items)?;
    transaction
        .execute(
            "UPDATE memory_spaces SET revision = revision + 1 WHERE id = ?1",
            [space_id.to_string()],
        )
        .map_err(storage)?;
    let summary = space.summary.as_ref().map(|summary| MemorySummary {
        space_id,
        ..summary.clone()
    });
    memory_adapter::replace_summary_in(transaction, space_id, summary.as_ref())?;
    Ok(())
}
