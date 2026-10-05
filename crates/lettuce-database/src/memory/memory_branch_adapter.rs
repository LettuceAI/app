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
    if own_space_in(transaction, conversation_id, parent_branch_id)?.is_some() {
        memory_adapter::create_conversation_space_in(transaction, conversation_id, branch_id)?;
    }
    Ok(())
}

fn own_space_in(
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
    let position: i64 = transaction.query_row(
        "WITH RECURSIVE prefix(id) AS (
            SELECT ?2 UNION ALL SELECT message.parent_message_id
              FROM conversation_messages message JOIN prefix ON message.id = prefix.id
             WHERE message.conversation_id = ?1 AND message.parent_message_id IS NOT NULL
         ) SELECT count(*) FROM conversation_messages message JOIN prefix ON message.id = prefix.id
            WHERE message.conversation_id = ?1 AND message.role IN ('user','assistant') AND message.visibility = 'visible'",
        params![conversation_id.to_string(), fork_message_id.to_string()],
        |row| row.get(0),
    ).map_err(storage)?;
    let run_ids = {
        let mut statement = transaction.prepare(
            "WITH RECURSIVE lineage(branch_id) AS (
                SELECT ?2 UNION ALL SELECT branch.parent_branch_id FROM conversation_branches branch
                  JOIN lineage ON branch.id = lineage.branch_id
                 WHERE branch.conversation_id = ?1 AND branch.parent_branch_id IS NOT NULL
            ), ancestry(message_id) AS (
                SELECT coalesce(head_message_id,fork_message_id) FROM conversation_branches WHERE conversation_id = ?1 AND id = ?2
                UNION ALL SELECT message.parent_message_id FROM conversation_messages message JOIN ancestry ON message.id = ancestry.message_id
                 WHERE message.conversation_id = ?1 AND message.parent_message_id IS NOT NULL
            ) SELECT run.id FROM dynamic_memory_runs run
              JOIN conversation_memory_spaces binding ON binding.conversation_id = run.conversation_id AND binding.branch_id = run.branch_id AND binding.space_id = run.space_id AND binding.pooled = 0
              WHERE run.conversation_id = ?1 AND run.branch_id IN (SELECT branch_id FROM lineage)
                AND (run.branch_id = ?2 OR NOT EXISTS (
                    SELECT 1 FROM dynamic_memory_run_source_messages source WHERE source.run_id = run.id
                    AND source.message_id NOT IN (SELECT message_id FROM ancestry)
                ))
                AND NOT EXISTS (
                    SELECT 1 FROM dynamic_memory_suffix_rewinds rewind
                    JOIN dynamic_memory_runs invalid ON invalid.id = rewind.invalid_run_id
                    WHERE rewind.conversation_id = run.conversation_id AND invalid.branch_id = run.branch_id
                      AND invalid.created_at <= run.created_at AND rewind.applied_at >= run.created_at
                )
              ORDER BY run.summary_window_start, run.summary_window_end, run.created_at, run.id",
        ).map_err(storage)?;
        statement
            .query_map(
                params![conversation_id.to_string(), parent_branch_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .map_err(storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage)?
    };
    let mut seed = memory_adapter::get_in(transaction, parent_space_id)
        .map_err(storage)?
        .ok_or(ConversationRepositoryError::Storage)?;
    let mut summary = None;
    for id in run_ids {
        let run_id: DynamicMemoryRunId = id.parse().map_err(storage)?;
        let run = dynamic_memory_run_adapter::load_run_in(transaction, run_id).map_err(storage)?;
        let settled: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM dynamic_memory_run_attempts WHERE run_id = ?1 AND status IN ('succeeded','failed','cancelled'))
                AND NOT EXISTS(SELECT 1 FROM dynamic_memory_run_attempts WHERE run_id = ?1 AND status IN ('created','processing'))",
            [id], |row| row.get(0),
        ).map_err(storage)?;
        if run.summary_window.end > u64::try_from(position).map_err(storage)? || !settled {
            seed = run.starting_memory;
            break;
        }
        let succeeded: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM dynamic_memory_run_attempts WHERE run_id = ?1 AND status = 'succeeded')",
            [run_id.to_string()], |row| row.get(0),
        ).map_err(storage)?;
        if succeeded {
            if let Some(checkpoint) =
                dynamic_memory_run_adapter::load_summary_checkpoint_in(transaction, run_id)
                    .map_err(storage)?
            {
                summary = Some(checkpoint.summary);
            }
        }
    }
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
