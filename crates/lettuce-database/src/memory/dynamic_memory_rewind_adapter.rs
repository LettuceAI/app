use std::{collections::HashSet, str::FromStr};

use lettuce_conversations::{
    ConversationRepositoryError, DescendantPolicy, TombstoneMessageResult,
};
use lettuce_memory::{
    DynamicMemorySuffixRewind, DynamicMemorySuffixRewindError, DynamicMemorySuffixRewindReceipt,
    DynamicMemorySuffixRewindRepository, MemoryChangeSet, MemoryRepositoryError, MemorySummary,
    PendingSuffixRewind, PendingSuffixRewindRepository,
};
use lettuce_types::{DynamicMemoryRunId, OperationId};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use crate::{
    Database, conversation::conversation_mutations, decode_versioned, encode_versioned,
    memory::dynamic_memory_run_adapter, memory::memory_adapter,
};

const JSON_VERSION: u32 = 1;

fn storage(_: impl std::fmt::Debug) -> DynamicMemorySuffixRewindError {
    DynamicMemorySuffixRewindError::Storage
}

/// Writes a backed-up suffix rewind and the effects it invalidated.
pub(crate) fn insert_restored_rewind_in(
    transaction: &Transaction<'_>,
    rewind: &lettuce_transfer::BackupDynamicMemoryRewind,
) -> Result<(), DynamicMemorySuffixRewindError> {
    transaction
        .execute(
            "INSERT INTO dynamic_memory_suffix_rewinds
                (operation_id,request_digest,conversation_id,invalid_run_id,space_id,
                 source_memory_revision,resulting_memory_revision,restored_summary_run_id,
                 resulting_memory_json,resulting_summary_json,applied_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                rewind.operation_id.to_string(),
                rewind.request_digest.as_str(),
                rewind.conversation_id.to_string(),
                rewind.invalid_run_id.map(|id| id.to_string()),
                rewind.space_id.to_string(),
                i64::try_from(rewind.source_memory_revision.get()).map_err(storage)?,
                i64::try_from(rewind.resulting_memory_revision.get()).map_err(storage)?,
                rewind.restored_summary_run_id.map(|id| id.to_string()),
                encode_versioned(&rewind.resulting_memory, JSON_VERSION).map_err(storage)?,
                rewind
                    .resulting_summary
                    .as_ref()
                    .map(|summary| encode_versioned(summary, JSON_VERSION).map_err(storage))
                    .transpose()?,
                rewind.applied_at.get(),
            ],
        )
        .map_err(storage)?;
    for (ordinal, effect_id) in rewind.invalidated_effect_ids.iter().enumerate() {
        transaction
            .execute(
                "INSERT INTO companion_turn_effect_invalidations
                    (operation_id,conversation_id,effect_id,ordinal)
                 VALUES (?1,?2,?3,?4)",
                params![
                    rewind.operation_id.to_string(),
                    rewind.conversation_id.to_string(),
                    effect_id.to_string(),
                    i64::try_from(ordinal).map_err(storage)?,
                ],
            )
            .map_err(storage)?;
    }
    Ok(())
}

fn memory_error(error: MemoryRepositoryError) -> DynamicMemorySuffixRewindError {
    match error {
        MemoryRepositoryError::NotFound => DynamicMemorySuffixRewindError::NotFound,
        MemoryRepositoryError::AlreadyExists => DynamicMemorySuffixRewindError::Conflict,
        MemoryRepositoryError::Conflict => DynamicMemorySuffixRewindError::Conflict,
        MemoryRepositoryError::Invalid(_) => DynamicMemorySuffixRewindError::Invalid,
        MemoryRepositoryError::Failure(_) => DynamicMemorySuffixRewindError::Storage,
    }
}

fn parse_id<T: FromStr>(value: String) -> Result<T, DynamicMemorySuffixRewindError> {
    value
        .parse()
        .map_err(|_| DynamicMemorySuffixRewindError::Storage)
}

fn request_digest(
    rewind: &DynamicMemorySuffixRewind,
) -> Result<String, DynamicMemorySuffixRewindError> {
    let encoded = encode_versioned(rewind, JSON_VERSION).map_err(storage)?;
    Ok(blake3::hash(encoded.as_bytes()).to_hex().to_string())
}

fn load_receipt(
    connection: &Connection,
    operation_id: OperationId,
    expected_digest: Option<&str>,
) -> Result<Option<DynamicMemorySuffixRewindReceipt>, DynamicMemorySuffixRewindError> {
    let row = connection
        .query_row(
            "SELECT request_digest,conversation_id,invalid_run_id,resulting_memory_json,
                    resulting_summary_json,applied_at
               FROM dynamic_memory_suffix_rewinds WHERE operation_id=?1",
            [operation_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            },
        )
        .optional()
        .map_err(storage)?;
    let Some((digest, conversation_id, invalid_run_id, memory_json, summary_json, applied_at)) =
        row
    else {
        return Ok(None);
    };
    if expected_digest.is_some_and(|expected| digest != expected) {
        return Err(DynamicMemorySuffixRewindError::Conflict);
    }
    let invalidated_effect_ids = {
        let mut statement = connection
            .prepare(
                "SELECT effect_id FROM companion_turn_effect_invalidations
                 WHERE operation_id=?1 ORDER BY ordinal",
            )
            .map_err(storage)?;
        statement
            .query_map([operation_id.to_string()], |row| row.get::<_, String>(0))
            .map_err(storage)?
            .map(|value| parse_id(value.map_err(storage)?))
            .collect::<Result<Vec<_>, _>>()?
    };
    Ok(Some(DynamicMemorySuffixRewindReceipt {
        operation_id,
        conversation_id: parse_id(conversation_id)?,
        invalid_run_id: invalid_run_id.map(parse_id).transpose()?,
        memory: decode_versioned(&memory_json, JSON_VERSION).map_err(storage)?,
        summary: summary_json
            .map(|value| decode_versioned(&value, JSON_VERSION).map_err(storage))
            .transpose()?,
        invalidated_effect_ids,
        applied_at: lettuce_types::TimestampMillis::new(applied_at),
    }))
}

fn prior_summary(
    transaction: &Transaction<'_>,
    conversation_id: lettuce_types::ConversationId,
    space_id: lettuce_types::MemorySpaceId,
    invalid_run_id: DynamicMemoryRunId,
    invalid_window_start: u64,
) -> Result<(Option<DynamicMemoryRunId>, Option<MemorySummary>), DynamicMemorySuffixRewindError> {
    let prior_id = transaction
        .query_row(
            "SELECT run.id
               FROM dynamic_memory_runs run
               JOIN dynamic_memory_summary_checkpoints checkpoint ON checkpoint.run_id=run.id
              WHERE run.conversation_id=?1 AND run.id<>?2 AND run.summary_window_end<=?3
                AND run.space_id=?4 AND run.branch_id=(SELECT branch_id FROM dynamic_memory_runs WHERE id=?2)
                AND NOT EXISTS (
                    SELECT 1 FROM dynamic_memory_suffix_rewinds rewind
                      JOIN dynamic_memory_runs undone ON undone.id = rewind.invalid_run_id
                     WHERE rewind.conversation_id = run.conversation_id
                       AND undone.branch_id = run.branch_id
                       AND undone.created_at <= run.created_at
                       AND rewind.applied_at >= run.created_at
                )
                AND EXISTS (
                    SELECT 1 FROM dynamic_memory_run_attempts attempt
                     WHERE attempt.run_id=run.id AND attempt.status='succeeded'
                )
              ORDER BY run.summary_window_end DESC, run.summary_window_start DESC,
                       checkpoint.settled_at DESC, run.id DESC
              LIMIT 1",
            params![
                conversation_id.to_string(),
                invalid_run_id.to_string(),
                i64::try_from(invalid_window_start).map_err(storage)?,
                space_id.to_string(),
            ],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(storage)?
        .map(parse_id)
        .transpose()?;
    let summary = prior_id
        .map(|run_id| {
            dynamic_memory_run_adapter::load_summary_checkpoint_in(transaction, run_id)
                .map_err(|_| DynamicMemorySuffixRewindError::Storage)?
                .map(|checkpoint| checkpoint.summary)
                .ok_or(DynamicMemorySuffixRewindError::Storage)
        })
        .transpose()?;
    Ok((prior_id, summary))
}

/// Reverts only what the rewound conversation's invalid run and its later
/// runs' memory tools did, latest first: decay, retrieval access, user edits
/// and, in a shared companion pool, the other conversations' changes stay.
/// Runs an earlier rewind already reverted are skipped.
pub(crate) fn undo_runs_and_manual(
    transaction: &Transaction<'_>,
    current: &lettuce_memory::MemorySpaceSnapshot,
    conversation_id: lettuce_types::ConversationId,
    invalid_run_id: Option<DynamicMemoryRunId>,
    manual: &[lettuce_memory::MemoryManualHistory],
    summary: &mut Option<MemorySummary>,
) -> Result<lettuce_memory::MemorySpaceSnapshot, DynamicMemorySuffixRewindError> {
    let run_ids = if let Some(invalid_run_id) = invalid_run_id {
        let mut statement = transaction.prepare("SELECT run.id FROM dynamic_memory_runs run JOIN dynamic_memory_runs invalid ON invalid.id=?2 WHERE run.conversation_id=?1 AND run.space_id=invalid.space_id AND run.branch_id=invalid.branch_id AND run.created_at>=invalid.created_at AND NOT EXISTS(SELECT 1 FROM dynamic_memory_suffix_rewinds rewind JOIN dynamic_memory_runs undone ON undone.id=rewind.invalid_run_id WHERE rewind.conversation_id=run.conversation_id AND undone.branch_id=run.branch_id AND undone.created_at<=run.created_at AND rewind.applied_at>=run.created_at) ORDER BY run.created_at DESC,run.id DESC").map_err(storage)?;
        statement
            .query_map(
                params![conversation_id.to_string(), invalid_run_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .map_err(storage)?
            .map(|value| parse_id::<DynamicMemoryRunId>(value.map_err(storage)?))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };
    enum Undo {
        Tool {
            before: Vec<lettuce_memory::MemoryItem>,
            outcome: lettuce_memory::MemoryToolOutcome,
        },
        Manual(Box<lettuce_memory::MemoryManualHistory>),
    }
    let mut events = Vec::new();
    for run_id in run_ids {
        let run = dynamic_memory_run_adapter::load_run_in(transaction, run_id).map_err(storage)?;
        let rows = transaction.prepare("SELECT result.outcome_json,result.settled_at,settlement.resulting_memory_revision,result.ordinal FROM dynamic_memory_background_tool_results result JOIN dynamic_memory_background_round_settlements settlement ON settlement.run_id=result.run_id AND settlement.attempt_id=result.attempt_id AND settlement.round_ordinal=result.round_ordinal WHERE result.run_id=?1 GROUP BY result.round_ordinal,result.ordinal ORDER BY result.round_ordinal,result.ordinal").and_then(|mut statement| statement.query_map([run_id.to_string()], |row| Ok((row.get::<_, String>(0)?,row.get::<_, i64>(1)?,row.get::<_, i64>(2)?,row.get::<_, i64>(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()).map_err(storage)?;
        let history = super::memory_manual_adapter::history_in(
            transaction,
            conversation_id,
            run.branch_id,
            run.space_id,
        )
        .map_err(memory_error)?;
        for (json, at, revision, ordinal) in rows {
            let mut before = run.starting_memory.items.clone();
            for edit in history.iter().rev() {
                let edit_revision =
                    i64::try_from(edit.resulting_revision.get()).map_err(storage)?;
                if edit.edit.at.get() < run.created_at.get()
                    || (edit.edit.at.get(), edit_revision) > (at, revision)
                {
                    continue;
                }
                if let Some(item) = &edit.after_item {
                    before.retain(|value| value.id != item.id);
                    before.push(item.clone());
                } else if let Some(item) = &edit.before_item {
                    before.retain(|value| value.id != item.id);
                }
            }
            events.push((
                at,
                revision,
                ordinal,
                Undo::Tool {
                    before,
                    outcome: decode_versioned(&json, JSON_VERSION).map_err(storage)?,
                },
            ));
        }
    }
    for edit in manual {
        events.push((
            edit.edit.at.get(),
            i64::try_from(edit.resulting_revision.get()).map_err(storage)?,
            i64::MAX,
            Undo::Manual(Box::new(edit.clone())),
        ));
    }
    events.sort_by_key(|(at, revision, ordinal, _)| (*at, *revision, *ordinal));
    let mut items = current.items.clone();
    for (_, _, _, event) in events.into_iter().rev() {
        match event {
            Undo::Tool { before, outcome } => {
                lettuce_memory::undo_memory_tool_outcomes(&mut items, &before, &[outcome])
            }
            Undo::Manual(edit) => {
                lettuce_memory::undo_manual_memory_edit(&mut items, summary, &edit)
                    .map_err(memory_error)?
            }
        }
    }
    if items == current.items && manual.is_empty() && invalid_run_id.is_none() {
        return Ok(current.clone());
    }
    memory_adapter::compare_and_apply_in(
        transaction,
        &MemoryChangeSet {
            space_id: current.id,
            expected_revision: current.revision,
            items,
        },
    )
    .map_err(memory_error)
}

impl PendingSuffixRewindRepository for Database {
    fn tombstone_suffix(
        &self,
        pending: &PendingSuffixRewind,
        now: lettuce_types::TimestampMillis,
    ) -> Result<TombstoneMessageResult, ConversationRepositoryError> {
        if pending.tombstone.descendants != DescendantPolicy::Tombstone
            || pending.summary_message_interval == 0
        {
            return Err(ConversationRepositoryError::Invalid(
                lettuce_conversations::ValidationError::InvalidValue {
                    field: "pending_suffix_rewind",
                },
            ));
        }
        let payload = encode_versioned(pending, JSON_VERSION)
            .map_err(|_| ConversationRepositoryError::Storage)?;
        conversation_mutations::tombstone_with(
            self,
            &pending.tombstone,
            now,
            true,
            |transaction, _| {
                transaction
                    .execute(
                        "INSERT INTO dynamic_memory_pending_suffix_rewinds
                            (conversation_id,operation_key,pending_json,recorded_at)
                         VALUES (?1,?2,?3,?4)",
                        params![
                            pending.tombstone.conversation_id.to_string(),
                            pending.tombstone.operation.key.as_str(),
                            payload,
                            now.get(),
                        ],
                    )
                    .map_err(|_| ConversationRepositoryError::Storage)?;
                Ok(())
            },
        )
    }

    fn record_empty_suffix(
        &self,
        pending: &PendingSuffixRewind,
        now: lettuce_types::TimestampMillis,
    ) -> Result<
        lettuce_conversations::MutationCommit<lettuce_conversations::Conversation>,
        ConversationRepositoryError,
    > {
        conversation_mutations::record_empty_suffix(
            self,
            pending.tombstone.conversation_id,
            pending.after_message_id,
            pending.tombstone.expected_revision,
            &pending.tombstone.operation,
            now,
        )
    }

    fn pending_suffix_rewinds(
        &self,
        conversation_id: Option<lettuce_types::ConversationId>,
    ) -> Result<Vec<PendingSuffixRewind>, DynamicMemorySuffixRewindError> {
        let connection = self.connection().map_err(storage)?;
        let mut statement = connection
            .prepare(
                "SELECT pending_json FROM dynamic_memory_pending_suffix_rewinds
                  WHERE ?1 IS NULL OR conversation_id = ?1
                  ORDER BY recorded_at, conversation_id, operation_key",
            )
            .map_err(storage)?;
        statement
            .query_map([conversation_id.map(|id| id.to_string())], |row| {
                row.get::<_, String>(0)
            })
            .map_err(storage)?
            .map(|payload| {
                decode_versioned::<PendingSuffixRewind>(&payload.map_err(storage)?, JSON_VERSION)
                    .map_err(storage)
            })
            .collect()
    }

    fn clear_pending_suffix_rewind(
        &self,
        pending: &PendingSuffixRewind,
    ) -> Result<(), DynamicMemorySuffixRewindError> {
        let connection = self.connection().map_err(storage)?;
        connection
            .execute(
                "DELETE FROM dynamic_memory_pending_suffix_rewinds
                  WHERE conversation_id = ?1 AND operation_key = ?2",
                params![
                    pending.tombstone.conversation_id.to_string(),
                    pending.tombstone.operation.key.as_str(),
                ],
            )
            .map_err(storage)?;
        Ok(())
    }

    fn pending_rewind_failure(
        &self,
        conversation_id: lettuce_types::ConversationId,
    ) -> Result<Option<lettuce_memory::OwedRewindFailure>, DynamicMemorySuffixRewindError> {
        let connection = self.connection().map_err(storage)?;
        connection
            .query_row(
                "SELECT failure FROM dynamic_memory_pending_suffix_rewinds
                  WHERE conversation_id = ?1 AND failure IS NOT NULL
                  ORDER BY recorded_at, operation_key LIMIT 1",
                [conversation_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map(|value| value.map(|code| lettuce_memory::OwedRewindFailure::parse(&code)))
            .map_err(storage)
    }

    fn fail_pending_suffix_rewind(
        &self,
        pending: &PendingSuffixRewind,
        failure: lettuce_memory::OwedRewindFailure,
    ) -> Result<(), DynamicMemorySuffixRewindError> {
        let connection = self.connection().map_err(storage)?;
        connection
            .execute(
                "UPDATE dynamic_memory_pending_suffix_rewinds SET failure = ?3
                  WHERE conversation_id = ?1 AND operation_key = ?2",
                params![
                    pending.tombstone.conversation_id.to_string(),
                    pending.tombstone.operation.key.as_str(),
                    failure.as_str(),
                ],
            )
            .map_err(storage)?;
        Ok(())
    }
}

impl DynamicMemorySuffixRewindRepository for Database {
    fn has_manual_memory_suffix(
        &self,
        conversation_id: lettuce_types::ConversationId,
        branch_id: lettuce_types::ConversationBranchId,
        removed_message_ids: &[lettuce_types::MessageId],
    ) -> Result<bool, DynamicMemorySuffixRewindError> {
        let connection = self.connection().map_err(storage)?;
        let anchors=connection.prepare("SELECT anchor_message_id FROM memory_manual_edits WHERE conversation_id=?1 AND branch_id=?2 AND undone_at IS NULL AND anchor_message_id IS NOT NULL").and_then(|mut statement| statement.query_map(params![conversation_id.to_string(),branch_id.to_string()], |row| row.get::<_, String>(0))?.collect::<rusqlite::Result<Vec<_>>>()).map_err(storage)?;
        Ok(anchors.into_iter().any(|id| {
            id.parse()
                .ok()
                .is_some_and(|id| removed_message_ids.contains(&id))
        }))
    }

    fn get_dynamic_memory_suffix_rewind(
        &self,
        operation_id: OperationId,
    ) -> Result<Option<DynamicMemorySuffixRewindReceipt>, DynamicMemorySuffixRewindError> {
        let connection = self.connection().map_err(storage)?;
        load_receipt(&connection, operation_id, None)
    }

    fn rewind_dynamic_memory_suffix(
        &self,
        rewind: DynamicMemorySuffixRewind,
    ) -> Result<DynamicMemorySuffixRewindReceipt, DynamicMemorySuffixRewindError> {
        if rewind.expected_memory_revision.get() == 0
            || rewind.invalidated_effect_ids.len() > 512
            || rewind
                .invalidated_effect_ids
                .iter()
                .copied()
                .collect::<HashSet<_>>()
                .len()
                != rewind.invalidated_effect_ids.len()
        {
            return Err(DynamicMemorySuffixRewindError::Invalid);
        }
        let digest = request_digest(&rewind)?;
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        if let Some(receipt) = load_receipt(&transaction, rewind.operation_id, Some(&digest))? {
            transaction.commit().map_err(storage)?;
            return Ok(receipt);
        }
        let pending_json: Option<String> = transaction.query_row("SELECT pending.pending_json FROM dynamic_memory_pending_suffix_rewinds pending JOIN conversation_operations operation ON operation.conversation_id=pending.conversation_id AND operation.operation_key=pending.operation_key AND operation.kind='tombstone' WHERE operation.id=?1", [rewind.operation_id.to_string()], |row| row.get(0)).optional().map_err(storage)?;
        let pending = pending_json
            .map(|json| {
                decode_versioned::<PendingSuffixRewind>(&json, JSON_VERSION).map_err(storage)
            })
            .transpose()?;
        if rewind.invalid_run_id.is_none()
            && rewind.invalidated_effect_ids.is_empty()
            && pending.is_none()
        {
            return Err(DynamicMemorySuffixRewindError::Invalid);
        }
        let branch: String = transaction
            .query_row(
                "SELECT coalesce(
                    (SELECT branch_id FROM dynamic_memory_runs WHERE conversation_id = ?1 AND id = ?2),
                    (SELECT turn.branch_id FROM companion_turn_effects effect JOIN conversation_turns turn ON turn.id = effect.turn_id WHERE effect.conversation_id = ?1 AND effect.id = ?3),
                    (SELECT branch_id FROM conversation_messages WHERE conversation_id=?1 AND id=?4)
                )",
                params![rewind.conversation_id.to_string(), rewind.invalid_run_id.map(|id| id.to_string()), rewind.invalidated_effect_ids.first().map(|id| id.to_string()), pending.as_ref().map(|value| value.tombstone.message_id.to_string())],
                |row| row.get(0),
            )
            .map_err(storage)?;
        let branch_id = branch.parse().map_err(storage)?;
        let space_id = match rewind.invalid_run_id {
            Some(run_id) => {
                let space: String = transaction.query_row(
                    "SELECT space_id FROM dynamic_memory_runs WHERE conversation_id = ?1 AND id = ?2",
                    params![rewind.conversation_id.to_string(), run_id.to_string()],
                    |row| row.get(0),
                ).map_err(storage)?;
                parse_id(space)?
            }
            None => {
                memory_adapter::branch_space_id_in(&transaction, rewind.conversation_id, branch_id)
                    .map_err(storage)?
                    .ok_or(DynamicMemorySuffixRewindError::NotFound)?
            }
        };
        let current = memory_adapter::get_in(&transaction, space_id)
            .map_err(memory_error)?
            .ok_or(DynamicMemorySuffixRewindError::NotFound)?;
        if current.revision != rewind.expected_memory_revision {
            return Err(DynamicMemorySuffixRewindError::Conflict);
        }
        let shared_pool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM companion_memory_pools WHERE space_id = ?1)",
                [space_id.to_string()],
                |row| row.get::<_, bool>(0),
            )
            .map_err(storage)?;

        let history = super::memory_manual_adapter::history_in(
            &transaction,
            rewind.conversation_id,
            branch_id,
            space_id,
        )
        .map_err(memory_error)?;
        let manual = if pending.is_some() {
            history.into_iter().filter_map(|edit| edit.anchor_message_id.map(|id| (edit,id))).map(|(edit,id)| {
                let removed=transaction.query_row("SELECT visibility='tombstoned' FROM conversation_messages WHERE conversation_id=?1 AND id=?2",params![rewind.conversation_id.to_string(),id.to_string()],|row| row.get::<_,bool>(0)).map_err(storage)?;
                Ok::<_,DynamicMemorySuffixRewindError>(removed.then_some(edit))
            }).collect::<Result<Vec<_>,_>>()?.into_iter().flatten().collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let mut own_summary =
            memory_adapter::get_summary_in(&transaction, space_id).map_err(memory_error)?;
        let memory = undo_runs_and_manual(
            &transaction,
            &current,
            rewind.conversation_id,
            rewind.invalid_run_id,
            &manual,
            &mut own_summary,
        )?;
        for edit in &manual {
            transaction
                .execute(
                    "UPDATE memory_manual_edits SET undone_at=?2 WHERE id=?1 AND undone_at IS NULL",
                    params![edit.edit.id.to_string(), rewind.at.get()],
                )
                .map_err(storage)?;
        }
        let (memory, restored_summary_run_id, summary) = match rewind.invalid_run_id {
            Some(invalid_run_id) => {
                let run = dynamic_memory_run_adapter::load_run_in(&transaction, invalid_run_id)
                    .map_err(|error| match error {
                        lettuce_memory::DynamicMemoryRunRepositoryError::NotFound => {
                            DynamicMemorySuffixRewindError::NotFound
                        }
                        lettuce_memory::DynamicMemoryRunRepositoryError::Conflict => {
                            DynamicMemorySuffixRewindError::Conflict
                        }
                        lettuce_memory::DynamicMemoryRunRepositoryError::Invalid => {
                            DynamicMemorySuffixRewindError::Invalid
                        }
                        lettuce_memory::DynamicMemoryRunRepositoryError::Storage => {
                            DynamicMemorySuffixRewindError::Storage
                        }
                    })?;
                if run.conversation_id != rewind.conversation_id || run.space_id != space_id {
                    return Err(DynamicMemorySuffixRewindError::Conflict);
                }
                if shared_pool {
                    (memory, None, own_summary.clone())
                } else {
                    let (prior_run_id, mut summary) = prior_summary(
                        &transaction,
                        rewind.conversation_id,
                        space_id,
                        invalid_run_id,
                        run.summary_window.start,
                    )?;
                    if prior_run_id.is_none() {
                        summary = crate::memory::memory_branch_adapter::inherited_summary_in(
                            &transaction,
                            rewind.conversation_id,
                            run.branch_id,
                        )
                        .map_err(|_| DynamicMemorySuffixRewindError::Storage)?
                        .filter(|inherited| inherited.window_end <= run.summary_window.start)
                        .map(|mut inherited| {
                            inherited.space_id = space_id;
                            inherited.branch_id = run.branch_id;
                            inherited
                        });
                    }
                    memory_adapter::replace_summary_in(&transaction, space_id, summary.as_ref())
                        .map_err(memory_error)?;
                    (memory, prior_run_id, summary)
                }
            }
            None => {
                memory_adapter::replace_summary_in(&transaction, space_id, own_summary.as_ref())
                    .map_err(memory_error)?;
                (memory, None, own_summary)
            }
        };

        memory_adapter::replace_summary_in(&transaction, space_id, summary.as_ref())
            .map_err(memory_error)?;

        transaction
            .execute(
                "INSERT INTO dynamic_memory_suffix_rewinds
                    (operation_id,request_digest,conversation_id,invalid_run_id,space_id,
                     source_memory_revision,resulting_memory_revision,restored_summary_run_id,
                     resulting_memory_json,resulting_summary_json,applied_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                params![
                    rewind.operation_id.to_string(),
                    digest,
                    rewind.conversation_id.to_string(),
                    rewind.invalid_run_id.map(|id| id.to_string()),
                    space_id.to_string(),
                    i64::try_from(rewind.expected_memory_revision.get()).map_err(storage)?,
                    i64::try_from(memory.revision.get()).map_err(storage)?,
                    restored_summary_run_id.map(|id| id.to_string()),
                    encode_versioned(&memory, JSON_VERSION).map_err(storage)?,
                    summary
                        .as_ref()
                        .map(|summary| encode_versioned(summary, JSON_VERSION).map_err(storage))
                        .transpose()?,
                    rewind.at.get(),
                ],
            )
            .map_err(storage)?;
        transaction
            .execute(
                "DELETE FROM memory_synced_cursors WHERE conversation_id = ?1 AND branch_id = ?2",
                params![rewind.conversation_id.to_string(), branch_id.to_string()],
            )
            .map_err(storage)?;

        for (ordinal, effect_id) in rewind.invalidated_effect_ids.iter().enumerate() {
            let owner = transaction
                .query_row(
                    "SELECT conversation_id FROM companion_turn_effects WHERE id=?1",
                    [effect_id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(storage)?
                .ok_or(DynamicMemorySuffixRewindError::NotFound)?;
            if owner != rewind.conversation_id.to_string() {
                return Err(DynamicMemorySuffixRewindError::Conflict);
            }
            transaction
                .execute(
                    "INSERT INTO companion_turn_effect_invalidations
                        (operation_id,conversation_id,effect_id,ordinal)
                     VALUES (?1,?2,?3,?4)",
                    params![
                        rewind.operation_id.to_string(),
                        rewind.conversation_id.to_string(),
                        effect_id.to_string(),
                        i64::try_from(ordinal).map_err(storage)?,
                    ],
                )
                .map_err(|error| match error.sqlite_error_code() {
                    Some(rusqlite::ErrorCode::ConstraintViolation) => {
                        DynamicMemorySuffixRewindError::Conflict
                    }
                    _ => DynamicMemorySuffixRewindError::Storage,
                })?;
        }
        transaction
            .execute(
                "DELETE FROM dynamic_memory_pending_approvals WHERE conversation_id=?1 AND branch_id=?2",
                params![rewind.conversation_id.to_string(),branch_id.to_string()],
            )
            .map_err(storage)?;
        let receipt = load_receipt(&transaction, rewind.operation_id, Some(&digest))?
            .ok_or(DynamicMemorySuffixRewindError::Storage)?;
        transaction.commit().map_err(storage)?;
        Ok(receipt)
    }
}

#[cfg(test)]
mod manual_undo_tests {
    use super::*;
    use lettuce_memory::*;
    use lettuce_types::*;

    #[derive(Debug)]
    struct Failure;
    impl From<crate::ApiOperationError> for Failure {
        fn from(_: crate::ApiOperationError) -> Self {
            Self
        }
    }
    impl From<MemoryRepositoryError> for Failure {
        fn from(_: MemoryRepositoryError) -> Self {
            Self
        }
    }

    #[test]
    fn interleaved_manual_and_tool_undo_preserves_kept_edits_and_rolls_back_marker_failure() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, space_id, messages) =
            super::super::dynamic_memory_run_adapter::tests::conversation_fixture(&database);
        let branch_id = super::super::dynamic_memory_run_adapter::tests::fixture_branch(
            &database,
            conversation_id,
        );
        database
            .connection()
            .expect("connection")
            .execute(
                "UPDATE conversation_branches SET head_message_id=?2 WHERE id=?1",
                params![
                    branch_id.to_string(),
                    messages.last().expect("head").message_id.to_string()
                ],
            )
            .expect("head");
        let id = MemoryId::new();
        let original = MemoryItem::written(
            id,
            MemoryShortId::derived(id),
            "Original".into(),
            TimestampMillis::new(5),
        );
        let starting = database
            .compare_and_apply(MemoryChangeSet {
                space_id,
                expected_revision: Revision::INITIAL,
                items: vec![original.clone()],
            })
            .expect("seed");
        let run_id = DynamicMemoryRunId::new();
        let attempt_id = DynamicMemoryAttemptId::new();
        let run = database
            .admit_dynamic_memory_run_attempt(NewDynamicMemoryRunAttempt {
                run_id,
                attempt_id,
                conversation_id,
                branch_id,
                space_id,
                starting_memory: starting.clone(),
                cycle_start_change: None,
                source_messages: messages,
                profile: super::super::dynamic_memory_run_adapter::tests::profile(),
                time_awareness_enabled: false,
                supersession_enabled: false,
                structured_fallback_format: DynamicMemoryStructuredFallbackFormat::Json,
                summary_window: DynamicMemorySummaryWindow {
                    message_interval: 2,
                    start: 0,
                    end: 2,
                },
                tool_request: dynamic_memory_tool_request_for_run(
                    DynamicMemoryToolOptions {
                        group: false,
                        supersession_enabled: false,
                        require_source_message_id: false,
                    },
                    &|key| key.to_owned(),
                ),
                job_id: JobId::new(),
                job_attempt: None,
                now: TimestampMillis::new(10),
            })
            .expect("run");
        database
            .transition_dynamic_memory_attempt(
                attempt_id,
                run.attempt.revision,
                DynamicMemoryAttemptStatus::Processing,
                None,
                TimestampMillis::new(11),
            )
            .expect("processing");
        let edit = MemoryManualEdit {
            id: OperationId::new(),
            conversation_id,
            branch_id,
            conversation_revision: Revision::INITIAL,
            expected_revision: starting.revision,
            space_id,
            context_revisions: vec![],
            mutation: MemoryManualMutation::Update {
                memory_id: id,
                text: Some("Kept edit".into()),
                category: MemoryFieldChange::Keep,
                observed_at: MemoryFieldChange::Keep,
                token_count: None,
            },
            at: TimestampMillis::new(12),
        };
        let history = database
            .commit_api_operation::<MemoryManualHistory, Failure>(
                "memory_update",
                "interleaved-edit",
                "digest",
                edit.at,
                |scope| {
                    scope
                        .apply_memory_manual_edit(&edit, None)
                        .map_err(Failure::from)
                },
            )
            .expect("manual edit");
        let call_id = ToolExecutionId::new();
        database.admit_dynamic_memory_inference_round(run_id,attempt_id,0,0,NewDynamicMemoryInferenceRound { ordinal:0,request_context:lettuce_conversations::ProviderNeutralContext { messages:vec![],attributions:Default::default(),budget:Default::default() },parts:vec![],provider_replay:None,usage:None,finish_reason:DynamicMemoryRoundFinishReason::Stop,kind:DynamicMemoryRoundKind::Manager,provider_request_id:None,calls:vec![NewDynamicMemoryToolCall { id:call_id,definition_version:1,call:lettuce_conversations::ProposedToolCall { provider_call_id:Some("delete".into()),name:"delete_memory".into(),arguments:serde_json::json!({"id":original.short_id.to_string(),"confidence":1.0}),raw_arguments:None,provider_replay:None } }],admitted_at:TimestampMillis::new(13) }).expect("round");
        database
            .commit_dynamic_memory_background_round(
                DynamicMemoryBackgroundRoundCommit {
                    run_id,
                    attempt_id,
                    round_ordinal: 0,
                    space_id,
                    expected_memory_revision: history.resulting_revision,
                    change: Some(MemoryChangeSet {
                        space_id,
                        expected_revision: history.resulting_revision,
                        items: vec![],
                    }),
                    results: vec![MemoryToolResult {
                        execution_id: call_id,
                        outcome: MemoryToolOutcome::Deleted {
                            id,
                            short_id: original.short_id,
                            text: "Kept edit".into(),
                            memories: vec![],
                        },
                    }],
                },
                TimestampMillis::new(14),
            )
            .expect("delete tool");
        let current = MemoryRepository::get(&database, space_id)
            .expect("memory")
            .expect("space");
        {
            let mut connection = database.connection().expect("connection");
            let tx = connection.transaction().expect("transaction");
            let kept =
                undo_runs_and_manual(&tx, &current, conversation_id, Some(run_id), &[], &mut None)
                    .expect("undo only run");
            assert_eq!(kept.items[0].text, "Kept edit");
        }
        assert_eq!(
            MemoryRepository::get(&database, space_id).expect("rollback"),
            Some(current.clone())
        );
        database.connection().expect("connection").execute_batch("CREATE TRIGGER fail_undo_marker BEFORE UPDATE ON memory_manual_edits BEGIN SELECT RAISE(ABORT,'injected marker failure'); END;").expect("inject");
        {
            let mut connection = database.connection().expect("connection");
            let tx = connection.transaction().expect("transaction");
            let undone = undo_runs_and_manual(
                &tx,
                &current,
                conversation_id,
                Some(run_id),
                std::slice::from_ref(&history),
                &mut None,
            )
            .expect("undo together");
            assert_eq!(undone.items[0].text, "Original");
            assert!(
                tx.execute(
                    "UPDATE memory_manual_edits SET undone_at=15 WHERE id=?1",
                    [edit.id.to_string()]
                )
                .is_err()
            );
        }
        assert_eq!(
            MemoryRepository::get(&database, space_id).expect("rollback marker failure"),
            Some(current)
        );
        let mut connection = database.connection().expect("connection");
        let tx = connection.transaction().expect("transaction");
        assert_eq!(
            super::super::memory_manual_adapter::history_in(
                &tx,
                conversation_id,
                branch_id,
                space_id
            )
            .expect("unchanged history"),
            vec![history]
        );
    }
}
