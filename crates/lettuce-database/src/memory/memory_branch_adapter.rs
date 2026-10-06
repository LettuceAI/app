use std::collections::HashMap;

use lettuce_conversations::ConversationRepositoryError;
use lettuce_memory::MemorySpaceSnapshot;
use lettuce_types::{
    ConversationBranchId, ConversationId, DynamicMemoryRunId, MemoryId, MemorySpaceId, MessageId,
    Revision,
};
use rusqlite::{OptionalExtension, Transaction, params};

use super::{dynamic_memory_run_adapter, memory_adapter};

fn storage(_: impl std::fmt::Debug) -> ConversationRepositoryError {
    ConversationRepositoryError::Storage
}

pub(crate) fn create_empty_branch_space_in(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    parent_branch_id: ConversationBranchId,
    branch_id: ConversationBranchId,
) -> Result<(), ConversationRepositoryError> {
    if own_space_in(transaction, conversation_id, parent_branch_id)?.is_some()
        || (parent_is_tombstoned(transaction, conversation_id, parent_branch_id)?
            && root_has_own_space(transaction, conversation_id)?)
    {
        memory_adapter::create_conversation_space_in(transaction, conversation_id, branch_id)?;
    }
    Ok(())
}

pub(crate) fn branch_has_parent(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
) -> Result<bool, ConversationRepositoryError> {
    transaction
        .query_row(
            "SELECT parent_branch_id IS NOT NULL FROM conversation_branches WHERE conversation_id = ?1 AND id = ?2",
            params![conversation_id.to_string(), branch_id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(storage)
        .map(|value| value.unwrap_or(false))
}

fn parent_is_tombstoned(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
) -> Result<bool, ConversationRepositoryError> {
    transaction
        .query_row(
            "SELECT status = 'tombstoned' FROM conversation_branches WHERE conversation_id = ?1 AND id = ?2",
            params![conversation_id.to_string(), branch_id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(storage)
        .map(|value| value.unwrap_or(false))
}

fn root_has_own_space(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
) -> Result<bool, ConversationRepositoryError> {
    transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM conversation_memory_spaces binding
               JOIN conversation_branches branch
                 ON branch.conversation_id = binding.conversation_id AND branch.id = binding.branch_id
              WHERE binding.conversation_id = ?1 AND binding.pooled = 0
                AND branch.parent_branch_id IS NULL)",
            [conversation_id.to_string()],
            |row| row.get(0),
        )
        .map_err(storage)
}

pub(crate) fn own_space_in(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
) -> Result<Option<MemorySpaceId>, ConversationRepositoryError> {
    transaction.query_row(
        "SELECT space_id FROM conversation_memory_spaces WHERE conversation_id = ?1 AND branch_id = ?2 AND pooled = 0",
        params![conversation_id.to_string(), branch_id.to_string()],
        |row| row.get::<_, String>(0),
    ).optional().map_err(storage)?.map(|id| id.parse().map_err(storage)).transpose()
}

pub(crate) fn seed_branch_space_in(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    parent_branch_id: ConversationBranchId,
    branch_id: ConversationBranchId,
    fork_message_id: MessageId,
    deterministic: bool,
) -> Result<(), ConversationRepositoryError> {
    let Some(parent_space_id) = own_space_in(transaction, conversation_id, parent_branch_id)?
    else {
        return Ok(());
    };
    let position = message_position_in(transaction, conversation_id, fork_message_id)?;
    let (cut, summary) = branch_state_in(
        transaction,
        conversation_id,
        parent_branch_id,
        parent_space_id,
        position,
        None,
    )?;
    let seed = match cut {
        Some(snapshot) => snapshot,
        None => memory_adapter::get_in(transaction, parent_space_id)
            .map_err(storage)?
            .ok_or(ConversationRepositoryError::Storage)?,
    };
    let space_id = if deterministic {
        MemorySpaceId::from_uuid(uuid::Uuid::new_v5(&branch_id.as_uuid(), b"memory-space"))
    } else {
        MemorySpaceId::new()
    };
    let ids: HashMap<_, _> = seed
        .items
        .iter()
        .map(|item| {
            let id = if deterministic {
                MemoryId::from_uuid(uuid::Uuid::new_v5(
                    &branch_id.as_uuid(),
                    item.id.as_uuid().as_bytes(),
                ))
            } else {
                MemoryId::new()
            };
            (item.id, id)
        })
        .collect();
    let mut items = seed.items.clone();
    for item in &mut items {
        item.id = ids[&item.id];
        item.superseded_by = item.superseded_by.and_then(|id| ids.get(&id).copied());
        if item.superseded_by.is_none() {
            item.superseded_at = None;
        }
        item.supersedes = item
            .supersedes
            .iter()
            .filter_map(|id| ids.get(id).copied())
            .collect();
    }
    memory_adapter::insert_space_in(
        transaction,
        conversation_id,
        branch_id,
        &MemorySpaceSnapshot {
            id: space_id,
            revision: Revision::INITIAL,
            items,
        },
    )?;
    for item in &seed.items {
        transaction.execute(
            "INSERT INTO memory_embedding_projections (space_id,memory_id,source_revision,dimensions,source_text,status,vector,updated_at)
             SELECT ?1,?2,source_revision,dimensions,source_text,status,vector,updated_at FROM memory_embedding_projections
              WHERE space_id = ?3 AND memory_id = ?4 AND source_text = ?5",
            params![space_id.to_string(), ids[&item.id].to_string(), seed.id.to_string(), item.id.to_string(), item.text],
        ).map_err(storage)?;
    }
    if let Some(mut summary) = summary {
        summary.space_id = space_id;
        summary.branch_id = branch_id;
        memory_adapter::replace_summary_in(transaction, space_id, Some(&summary))
            .map_err(storage)?;
    }
    Ok(())
}

pub(crate) fn seed_new_conversation_space_in(
    transaction: &Transaction<'_>,
    source_conversation_id: ConversationId,
    source_branch_id: ConversationBranchId,
    target: (ConversationId, ConversationBranchId),
    at_message_id: Option<MessageId>,
    message_ids: &HashMap<MessageId, MessageId>,
) -> Result<HashMap<MemoryId, MemoryId>, ConversationRepositoryError> {
    let (conversation_id, branch_id) = target;
    let pooled_character: Option<String> = transaction.query_row(
        "SELECT pool.character_id FROM conversation_memory_spaces binding JOIN companion_memory_pools pool ON pool.space_id = binding.space_id WHERE binding.conversation_id = ?1 AND binding.branch_id = ?2 AND binding.pooled = 1",
        params![conversation_id.to_string(), branch_id.to_string()], |row| row.get(0),
    ).optional().map_err(storage)?;
    if let Some(character_id) = pooled_character {
        let character_id = character_id.parse().map_err(storage)?;
        if crate::catalog::character_adapter::companion_memory_shared_in(transaction, character_id)
            .map_err(storage)?
        {
            return Ok(HashMap::new());
        }
    }
    let Some(source_space) = own_space_in(transaction, source_conversation_id, source_branch_id)?
    else {
        return Ok(HashMap::new());
    };
    let (cut, mut summary) = if let Some(message_id) = at_message_id {
        let position = message_position_in(transaction, source_conversation_id, message_id)?;
        branch_state_in(
            transaction,
            source_conversation_id,
            source_branch_id,
            source_space,
            position,
            None,
        )?
    } else {
        (
            None,
            memory_adapter::get_summary_in(transaction, source_space).map_err(storage)?,
        )
    };
    let seed = cut.map_or_else(
        || {
            memory_adapter::get_in(transaction, source_space)
                .map_err(storage)?
                .ok_or(ConversationRepositoryError::Storage)
        },
        Ok,
    )?;
    let space_id = match own_space_in(transaction, conversation_id, branch_id)? {
        Some(id) => {
            let current = memory_adapter::get_in(transaction, id)
                .map_err(storage)?
                .ok_or(ConversationRepositoryError::Storage)?;
            if !current.items.is_empty()
                || memory_adapter::get_summary_in(transaction, id)
                    .map_err(storage)?
                    .is_some()
            {
                return Err(ConversationRepositoryError::Conflict);
            }
            id
        }
        None => {
            memory_adapter::create_conversation_space_in(transaction, conversation_id, branch_id)?
        }
    };
    let ids: HashMap<_, _> = seed
        .items
        .iter()
        .map(|item| (item.id, MemoryId::new()))
        .collect();
    let mut items = seed.items.clone();
    for item in &mut items {
        item.id = ids[&item.id];
        item.superseded_by = item.superseded_by.and_then(|id| ids.get(&id).copied());
        if item.superseded_by.is_none() {
            item.superseded_at = None;
        }
        item.supersedes = item
            .supersedes
            .iter()
            .filter_map(|id| ids.get(id).copied())
            .collect();
        item.source_message_id = item
            .source_message_id
            .and_then(|id| message_ids.get(&id).copied());
        if item.source_message_id.is_none() {
            item.source_role = None;
            item.observed_at = None;
            item.observed_time_precision = None;
        }
    }
    if summary.as_ref().is_some_and(|value| {
        value
            .source_message_ids
            .iter()
            .any(|id| !message_ids.contains_key(id))
    }) {
        summary = None;
        transaction
            .execute(
                "DELETE FROM memory_synced_cursors WHERE conversation_id = ?1 AND branch_id = ?2",
                params![conversation_id.to_string(), branch_id.to_string()],
            )
            .map_err(storage)?;
    }
    if let Some(summary) = &mut summary {
        summary.space_id = space_id;
        summary.branch_id = branch_id;
        summary.source_message_ids = summary
            .source_message_ids
            .iter()
            .filter_map(|id| message_ids.get(id).copied())
            .collect();
        summary.validate().map_err(|_| {
            ConversationRepositoryError::Invalid(
                lettuce_conversations::ValidationError::InvalidReference {
                    field: "copy.summary.source_messages",
                },
            )
        })?;
    }
    memory_adapter::insert_items(transaction, space_id, &items).map_err(storage)?;
    for item in &seed.items {
        transaction.execute(
            "INSERT INTO memory_embedding_projections (space_id,memory_id,source_revision,dimensions,source_text,status,vector,updated_at)
             SELECT ?1,?2,source_revision,dimensions,source_text,status,vector,updated_at FROM memory_embedding_projections
              WHERE space_id = ?3 AND memory_id = ?4 AND source_text = ?5",
            params![space_id.to_string(), ids[&item.id].to_string(), seed.id.to_string(), item.id.to_string(), item.text],
        ).map_err(storage)?;
    }
    memory_adapter::replace_summary_in(transaction, space_id, summary.as_ref()).map_err(storage)?;
    Ok(ids)
}

pub(crate) fn message_position_in(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    message_id: MessageId,
) -> Result<u64, ConversationRepositoryError> {
    let position: i64 = transaction.query_row(
        "WITH RECURSIVE prefix(id) AS (
            SELECT ?2 UNION ALL SELECT message.parent_message_id
              FROM conversation_messages message JOIN prefix ON message.id = prefix.id
             WHERE message.conversation_id = ?1 AND message.parent_message_id IS NOT NULL
         ) SELECT count(*) FROM conversation_messages message JOIN prefix ON message.id = prefix.id
            WHERE message.conversation_id = ?1 AND message.role IN ('user','assistant') AND message.visibility = 'visible'",
        params![conversation_id.to_string(), message_id.to_string()],
        |row| row.get(0),
    ).map_err(storage)?;
    u64::try_from(position).map_err(storage)
}

struct OwnRun {
    run: lettuce_memory::DynamicMemoryRun,
    in_flight: bool,
    succeeded: bool,
}

/// The branch's own runs in its own space, in window order. With `cutoff`
/// only runs that existed then count, and a run that had not settled by then
/// is in flight.
fn own_runs_in(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
    space_id: MemorySpaceId,
    cutoff: Option<i64>,
) -> Result<Vec<OwnRun>, ConversationRepositoryError> {
    let ids = {
        let mut statement = transaction
            .prepare(
                "SELECT run.id FROM dynamic_memory_runs run
                  WHERE run.conversation_id = ?1 AND run.branch_id = ?2 AND run.space_id = ?3
                    AND (?4 IS NULL OR run.created_at < ?4)
                    AND NOT EXISTS (
                        SELECT 1 FROM dynamic_memory_suffix_rewinds rewind
                        JOIN dynamic_memory_runs invalid ON invalid.id = rewind.invalid_run_id
                        WHERE rewind.conversation_id = run.conversation_id AND invalid.branch_id = run.branch_id
                          AND invalid.created_at <= run.created_at AND rewind.applied_at >= run.created_at
                          AND (?4 IS NULL OR rewind.applied_at < ?4)
                    )
                  ORDER BY run.summary_window_start, run.summary_window_end, run.created_at, run.id",
            )
            .map_err(storage)?;
        statement
            .query_map(
                params![
                    conversation_id.to_string(),
                    branch_id.to_string(),
                    space_id.to_string(),
                    cutoff
                ],
                |row| row.get::<_, String>(0),
            )
            .map_err(storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage)?
    };
    ids.into_iter()
        .map(|id| {
            let (in_flight, succeeded): (bool, bool) = transaction
                .query_row(
                    "SELECT
                        EXISTS(SELECT 1 FROM dynamic_memory_run_attempts WHERE run_id = ?1 AND status IN ('created','processing'))
                          OR (?2 IS NOT NULL AND coalesce((SELECT max(coalesce(finished_at, updated_at)) FROM dynamic_memory_run_attempts WHERE run_id = ?1), 0) > ?2),
                        EXISTS(SELECT 1 FROM dynamic_memory_run_attempts WHERE run_id = ?1 AND status = 'succeeded'
                                  AND (?2 IS NULL OR finished_at <= ?2))",
                    params![id, cutoff],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(storage)?;
            let run_id: DynamicMemoryRunId = id.parse().map_err(storage)?;
            let run =
                dynamic_memory_run_adapter::load_run_in(transaction, run_id).map_err(storage)?;
            Ok(OwnRun {
                run,
                in_flight,
                succeeded,
            })
        })
        .collect()
}

/// The branch's memory as of `position`: the starting snapshot of its first
/// own run that reaches past `position` or is in flight (none means its
/// current items), and the summary it had then. Ancestor branches count only
/// through the state this branch was seeded with.
fn branch_state_in(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
    space_id: MemorySpaceId,
    position: u64,
    cutoff: Option<i64>,
) -> Result<
    (
        Option<MemorySpaceSnapshot>,
        Option<lettuce_memory::MemorySummary>,
    ),
    ConversationRepositoryError,
> {
    let mut cut = None;
    let mut summary = None;
    for own in own_runs_in(transaction, conversation_id, branch_id, space_id, cutoff)? {
        if own.run.summary_window.end > position || own.in_flight {
            cut = Some(own.run.starting_memory);
            break;
        }
        if own.succeeded
            && let Some(checkpoint) =
                dynamic_memory_run_adapter::load_summary_checkpoint_in(transaction, own.run.id)
                    .map_err(storage)?
        {
            summary = Some(checkpoint.summary);
        }
    }
    if summary.is_none() {
        summary = base_summary_in(transaction, conversation_id, branch_id, space_id, position)?;
    }
    Ok((cut, summary))
}

/// The summary a branch started from, before any of its own runs: its stored
/// summary while no own run has replaced it, otherwise what its parent had at
/// the fork when the branch was created.
fn base_summary_in(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
    space_id: MemorySpaceId,
    position: u64,
) -> Result<Option<lettuce_memory::MemorySummary>, ConversationRepositoryError> {
    let replaced: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM dynamic_memory_runs run
                JOIN dynamic_memory_summary_checkpoints checkpoint ON checkpoint.run_id = run.id
               WHERE run.conversation_id = ?1 AND run.branch_id = ?2 AND run.space_id = ?3)",
            params![
                conversation_id.to_string(),
                branch_id.to_string(),
                space_id.to_string()
            ],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if !replaced {
        let stored = memory_adapter::get_summary_in(transaction, space_id)
            .map_err(storage)?
            .filter(|summary| summary.window_end <= position);
        if stored.is_some() {
            return Ok(stored);
        }
        if !branch_has_parent(transaction, conversation_id, branch_id)? {
            return Ok(None);
        }
        return Ok(
            load_materialised_summary_in(transaction, space_id, branch_id)?
                .filter(|summary| summary.window_end <= position),
        );
    }
    Ok(
        inherited_summary_in(transaction, conversation_id, branch_id)?
            .filter(|summary| summary.window_end <= position),
    )
}

/// What the branch's parent had as of the fork message when the branch was
/// created; the root branch inherits nothing.
pub(crate) fn inherited_summary_in(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
) -> Result<Option<lettuce_memory::MemorySummary>, ConversationRepositoryError> {
    let point: Option<(Option<String>, Option<String>, i64)> = transaction
        .query_row(
            "SELECT parent_branch_id, fork_message_id, created_at FROM conversation_branches
              WHERE conversation_id = ?1 AND id = ?2",
            params![conversation_id.to_string(), branch_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(storage)?;
    let Some((Some(parent), Some(fork_message), created_at)) = point else {
        return Ok(None);
    };
    if let Some(space_id) = own_space_in(transaction, conversation_id, branch_id)?
        && let Some(summary) = load_materialised_summary_in(transaction, space_id, branch_id)?
    {
        return Ok(Some(summary));
    }
    let parent: ConversationBranchId = parent.parse().map_err(storage)?;
    let Some(parent_space) = own_space_in(transaction, conversation_id, parent)? else {
        return Ok(None);
    };
    let position = message_position_in(
        transaction,
        conversation_id,
        fork_message.parse().map_err(storage)?,
    )?;
    Ok(branch_state_in(
        transaction,
        conversation_id,
        parent,
        parent_space,
        position,
        Some(created_at),
    )?
    .1)
}

pub(crate) fn load_materialised_summary_in(
    transaction: &Transaction<'_>,
    space_id: MemorySpaceId,
    branch_id: ConversationBranchId,
) -> Result<Option<lettuce_memory::MemorySummary>, ConversationRepositoryError> {
    let row: Option<(String, i64, i64, i64, i64)> = transaction
        .query_row(
            "SELECT text, token_count, window_start, window_end, updated_at
               FROM memory_inherited_summaries WHERE space_id = ?1",
            [space_id.to_string()],
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
    let Some((text, token_count, window_start, window_end, updated_at)) = row else {
        return Ok(None);
    };
    let source_message_ids = {
        let mut statement = transaction
            .prepare(
                "SELECT message_id FROM memory_inherited_summary_source_messages
                  WHERE space_id = ?1 ORDER BY ordinal",
            )
            .map_err(storage)?;
        statement
            .query_map([space_id.to_string()], |row| row.get::<_, String>(0))
            .map_err(storage)?
            .map(|id| id.map_err(storage)?.parse().map_err(storage))
            .collect::<Result<Vec<MessageId>, ConversationRepositoryError>>()?
    };
    let summary = lettuce_memory::MemorySummary {
        space_id,
        branch_id,
        text,
        token_count: u32::try_from(token_count).map_err(storage)?,
        window_start: u64::try_from(window_start).map_err(storage)?,
        window_end: u64::try_from(window_end).map_err(storage)?,
        source_message_ids,
        updated_at: lettuce_types::TimestampMillis::new(updated_at),
    };
    summary.validate().map_err(storage)?;
    Ok(Some(summary))
}

/// Stores, in each live child's own space, the summary it currently inherits
/// from `branch_id`, so the child stays complete once `branch_id` is purged.
pub(crate) fn materialise_inherited_summaries_in(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
) -> Result<(), ConversationRepositoryError> {
    let children = {
        let mut statement = transaction
            .prepare(
                "SELECT id FROM conversation_branches
                  WHERE conversation_id = ?1 AND parent_branch_id = ?2 AND status = 'active'
                  ORDER BY created_at, id",
            )
            .map_err(storage)?;
        statement
            .query_map(
                params![conversation_id.to_string(), branch_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .map_err(storage)?
            .map(|id| id.map_err(storage)?.parse().map_err(storage))
            .collect::<Result<Vec<ConversationBranchId>, ConversationRepositoryError>>()?
    };
    for child in children {
        let Some(space_id) = own_space_in(transaction, conversation_id, child)? else {
            continue;
        };
        if load_materialised_summary_in(transaction, space_id, child)?.is_some() {
            continue;
        }
        let Some(summary) = inherited_summary_in(transaction, conversation_id, child)? else {
            continue;
        };
        store_materialised_summary_in(transaction, conversation_id, child, space_id, &summary)?;
    }
    Ok(())
}

pub(crate) fn store_materialised_summary_in(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
    space_id: MemorySpaceId,
    summary: &lettuce_memory::MemorySummary,
) -> Result<(), ConversationRepositoryError> {
    transaction
        .execute(
            "INSERT INTO memory_inherited_summaries (
                space_id, conversation_id, branch_id, text, token_count,
                window_start, window_end, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                space_id.to_string(),
                conversation_id.to_string(),
                branch_id.to_string(),
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
                "INSERT INTO memory_inherited_summary_source_messages (
                    space_id, conversation_id, message_id, ordinal
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    space_id.to_string(),
                    conversation_id.to_string(),
                    message_id.to_string(),
                    i64::try_from(ordinal).map_err(storage)?,
                ],
            )
            .map_err(storage)?;
    }
    Ok(())
}

/// Keeps the summary a fork was just seeded with as its inherited summary,
/// for a fork whose recorded parent is not the branch it was seeded from.
pub(crate) fn pin_seeded_summary_in(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
) -> Result<(), ConversationRepositoryError> {
    let Some(space_id) = own_space_in(transaction, conversation_id, branch_id)? else {
        return Ok(());
    };
    if let Some(summary) = memory_adapter::get_summary_in(transaction, space_id).map_err(storage)? {
        store_materialised_summary_in(transaction, conversation_id, branch_id, space_id, &summary)?;
    }
    Ok(())
}
