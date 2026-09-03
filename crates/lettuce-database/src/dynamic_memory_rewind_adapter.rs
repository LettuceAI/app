use std::{collections::HashSet, str::FromStr};

use lettuce_memory::{
    DynamicMemorySuffixRewind, DynamicMemorySuffixRewindError, DynamicMemorySuffixRewindReceipt,
    DynamicMemorySuffixRewindRepository, MemoryChangeSet, MemoryRepositoryError, MemorySummary,
};
use lettuce_types::{DynamicMemoryRunId, MemorySpaceId, OperationId};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use crate::{
    Database, decode_versioned, dynamic_memory_run_adapter, encode_versioned, memory_adapter,
};

const JSON_VERSION: u32 = 1;

fn storage(_: impl std::fmt::Debug) -> DynamicMemorySuffixRewindError {
    DynamicMemorySuffixRewindError::Storage
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
    invalid_run_id: DynamicMemoryRunId,
    invalid_window_start: u64,
) -> Result<(Option<DynamicMemoryRunId>, Option<MemorySummary>), DynamicMemorySuffixRewindError> {
    let prior_id = transaction
        .query_row(
            "SELECT run.id
               FROM dynamic_memory_runs run
               JOIN dynamic_memory_summary_checkpoints checkpoint ON checkpoint.run_id=run.id
              WHERE run.conversation_id=?1 AND run.id<>?2 AND run.summary_window_end<=?3
              ORDER BY run.summary_window_end DESC, run.summary_window_start DESC,
                       checkpoint.settled_at DESC, run.id DESC
              LIMIT 1",
            params![
                conversation_id.to_string(),
                invalid_run_id.to_string(),
                i64::try_from(invalid_window_start).map_err(storage)?,
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

impl DynamicMemorySuffixRewindRepository for Database {
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
            || (rewind.invalid_run_id.is_none() && rewind.invalidated_effect_ids.is_empty())
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
        let space_id = transaction
            .query_row(
                "SELECT space_id FROM conversation_memory_spaces WHERE conversation_id=?1",
                [rewind.conversation_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(storage)?
            .map(parse_id::<MemorySpaceId>)
            .transpose()?
            .ok_or(DynamicMemorySuffixRewindError::NotFound)?;
        let current = memory_adapter::get_in(&transaction, space_id)
            .map_err(memory_error)?
            .ok_or(DynamicMemorySuffixRewindError::NotFound)?;
        if current.revision != rewind.expected_memory_revision {
            return Err(DynamicMemorySuffixRewindError::Conflict);
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
                let (prior_run_id, summary) = prior_summary(
                    &transaction,
                    rewind.conversation_id,
                    invalid_run_id,
                    run.summary_window.start,
                )?;
                let memory = memory_adapter::compare_and_apply_in(
                    &transaction,
                    &MemoryChangeSet {
                        space_id,
                        expected_revision: rewind.expected_memory_revision,
                        items: run.starting_memory.items,
                    },
                )
                .map_err(memory_error)?;
                memory_adapter::replace_summary_in(&transaction, space_id, summary.as_ref())
                    .map_err(memory_error)?;
                (memory, prior_run_id, summary)
            }
            None => (
                current,
                None,
                memory_adapter::get_summary_in(&transaction, space_id).map_err(memory_error)?,
            ),
        };

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
                "DELETE FROM dynamic_memory_pending_approvals WHERE conversation_id=?1",
                [rewind.conversation_id.to_string()],
            )
            .map_err(storage)?;
        let receipt = load_receipt(&transaction, rewind.operation_id, Some(&digest))?
            .ok_or(DynamicMemorySuffixRewindError::Storage)?;
        transaction.commit().map_err(storage)?;
        Ok(receipt)
    }
}
