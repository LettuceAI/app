use std::str::FromStr;

use lettuce_memory::{
    DynamicMemoryPendingApproval, MemoryActivityCycle, MemoryReadRepository, MemoryReadScope,
    MemoryRepositoryError, MemorySummary, MemoryToolResult,
};
use lettuce_types::{
    ConversationBranchId, ConversationId, DynamicMemoryAttemptId, DynamicMemoryRunId,
    MemorySpaceId, MessageId, Revision, TimestampMillis,
};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use super::{dynamic_memory_run_adapter, memory_adapter, memory_branch_adapter};
use crate::Database;

fn storage(_: impl std::fmt::Debug) -> MemoryRepositoryError {
    MemoryRepositoryError::Failure("sqlite memory read failed".into())
}

pub(crate) fn resolved_summary_in(
    tx: &Transaction<'_>,
    space_id: MemorySpaceId,
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
) -> Result<Option<MemorySummary>, MemoryRepositoryError> {
    if let Some(summary) = memory_adapter::get_summary_in(tx, space_id)? {
        return Ok(Some(summary));
    }
    let own: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_memory_spaces WHERE conversation_id=?1 AND branch_id=?2 AND space_id=?3 AND pooled=0)", params![conversation_id.to_string(),branch_id.to_string(),space_id.to_string()], |row| row.get(0)).map_err(storage)?;
    if !own {
        return Ok(None);
    }
    let Some(mut summary) =
        memory_branch_adapter::inherited_summary_in(tx, conversation_id, branch_id)
            .map_err(storage)?
    else {
        return Ok(None);
    };
    summary.space_id = space_id;
    summary.branch_id = branch_id;
    if memory_branch_adapter::load_materialised_summary_in(tx, space_id, branch_id)
        .map_err(storage)?
        .is_none()
    {
        memory_branch_adapter::store_materialised_summary_in(
            tx,
            conversation_id,
            branch_id,
            space_id,
            &summary,
        )
        .map_err(storage)?;
    }
    Ok(Some(summary))
}

impl MemoryReadRepository for Database {
    fn read_memory_scope(
        &self,
        conversation_id: ConversationId,
        conversation_revision: Revision,
    ) -> Result<MemoryReadScope, MemoryRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let row: Option<(String, Option<String>)> = tx.query_row("SELECT branch.id,coalesce(branch.head_message_id,branch.fork_message_id) FROM conversations conversation JOIN conversation_branches branch ON branch.conversation_id=conversation.id AND branch.id=conversation.active_branch_id WHERE conversation.id=?1 AND conversation.revision=?2 AND conversation.lifecycle<>'tombstoned' AND branch.status='active'", params![conversation_id.to_string(),i64::try_from(conversation_revision.get()).map_err(storage)?], |row| Ok((row.get(0)?,row.get(1)?))).optional().map_err(storage)?;
        let (branch, head) = row.ok_or(MemoryRepositoryError::Conflict)?;
        let branch_id = ConversationBranchId::from_str(&branch).map_err(storage)?;
        let space_id = memory_adapter::branch_space_id_in(&tx, conversation_id, branch_id)
            .map_err(storage)?
            .ok_or(MemoryRepositoryError::NotFound)?;
        let memory =
            memory_adapter::get_in(&tx, space_id)?.ok_or(MemoryRepositoryError::NotFound)?;
        let pooled = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM companion_memory_pools WHERE space_id=?1)",
                [space_id.to_string()],
                |row| row.get(0),
            )
            .map_err(storage)?;
        let summary = resolved_summary_in(&tx, space_id, conversation_id, branch_id)?;
        let message_count = head
            .map(|head| {
                MessageId::from_str(&head).map_err(storage).and_then(|id| {
                    memory_branch_adapter::message_position_in(&tx, conversation_id, id)
                        .map_err(storage)
                })
            })
            .transpose()?
            .unwrap_or(0);
        let summary_cursor =
            memory_adapter::summary_cursor_in(&tx, space_id, conversation_id, branch_id)?;
        let approval = tx.query_row("SELECT prompted_message_count,pending,skipped,updated_at FROM dynamic_memory_pending_approvals WHERE conversation_id=?1 AND branch_id=?2", params![conversation_id.to_string(),branch_id.to_string()], |row| {
            Ok((row.get::<_, i64>(0)?,row.get::<_, bool>(1)?,row.get::<_, bool>(2)?,row.get::<_, i64>(3)?))
        }).optional().map_err(storage)?.map(|(count,pending,skipped,at)| {
            Ok::<_, MemoryRepositoryError>(DynamicMemoryPendingApproval { conversation_id,branch_id,prompted_message_count:u64::try_from(count).map_err(storage)?,pending,skipped,updated_at:TimestampMillis::new(at) })
        }).transpose()?;
        let ids = tx
            .prepare("SELECT id FROM dynamic_memory_runs WHERE space_id=?1 ORDER BY created_at,id")
            .and_then(|mut statement| {
                statement
                    .query_map([space_id.to_string()], |row| row.get::<_, String>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(storage)?;
        let cycles = ids.into_iter().map(|id| {
            let run_id = DynamicMemoryRunId::from_str(&id).map_err(storage)?;
            let run = dynamic_memory_run_adapter::load_run_in(&tx, run_id).map_err(storage)?;
            let attempt: String = tx.query_row("SELECT id FROM dynamic_memory_run_attempts WHERE run_id=?1 ORDER BY ordinal DESC LIMIT 1", [&id], |row| row.get(0)).map_err(storage)?;
            let latest_attempt = dynamic_memory_run_adapter::load_attempt_in(&tx, DynamicMemoryAttemptId::from_str(&attempt).map_err(storage)?).map_err(storage)?;
            let rows = tx.prepare("SELECT call_id,outcome_json FROM dynamic_memory_background_tool_results WHERE run_id=?1 ORDER BY settled_at,ordinal").and_then(|mut statement| statement.query_map([&id], |row| Ok((row.get::<_, String>(0)?,row.get::<_, String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()).map_err(storage)?;
            let results = rows.into_iter().map(|(call,outcome)| Ok::<_, MemoryRepositoryError>(MemoryToolResult { execution_id: call.parse().map_err(storage)?,outcome:crate::decode_versioned(&outcome,1).map_err(storage)? })).collect::<Result<Vec<_>, _>>()?;
            let reverted = tx.query_row("SELECT EXISTS(SELECT 1 FROM dynamic_memory_suffix_rewinds rewind JOIN dynamic_memory_runs invalid ON invalid.id=rewind.invalid_run_id WHERE rewind.space_id=?1 AND invalid.branch_id=?2 AND invalid.conversation_id=?3 AND invalid.created_at<=?4 AND rewind.applied_at>=?4)", params![space_id.to_string(),run.branch_id.to_string(),run.conversation_id.to_string(),run.created_at.get()], |row| row.get(0)).map_err(storage)?;
            Ok(MemoryActivityCycle { run,latest_attempt,results,reverted })
        }).collect::<Result<Vec<_>, MemoryRepositoryError>>()?;
        let space_conversations = memory_adapter::space_conversations_in(&tx, space_id, u32::MAX)
            .map_err(storage)?
            .into_iter()
            .map(|id| id.parse().map_err(storage))
            .collect::<Result<Vec<_>, _>>()?;
        tx.commit().map_err(storage)?;
        Ok(MemoryReadScope {
            conversation_id,
            branch_id,
            memory,
            pooled,
            summary,
            message_count,
            summary_cursor,
            approval,
            cycles,
            space_conversations,
        })
    }
}
