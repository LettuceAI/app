use lettuce_memory::{
    MemoryCycleRevert, MemoryCycleRevertError, MemoryCycleRevertRecord, MemoryOrigin,
    MemoryRepositoryError,
};
use lettuce_types::{
    ConversationId, DynamicMemoryRunId, JobId, MemorySpaceId, Revision, TimestampMillis,
};
use rusqlite::{OptionalExtension, Transaction, params};

use super::{
    dynamic_memory_rewind_adapter, dynamic_memory_run_adapter, memory_adapter,
    memory_branch_adapter,
};
use crate::ApiOperationTransaction;

fn storage(_: impl std::fmt::Debug) -> MemoryCycleRevertError {
    MemoryCycleRevertError::Storage
}

fn rewind_error(error: lettuce_memory::DynamicMemorySuffixRewindError) -> MemoryCycleRevertError {
    match error {
        lettuce_memory::DynamicMemorySuffixRewindError::Conflict => {
            MemoryCycleRevertError::Conflict
        }
        lettuce_memory::DynamicMemorySuffixRewindError::NotFound => {
            MemoryCycleRevertError::NotFound
        }
        lettuce_memory::DynamicMemorySuffixRewindError::Invalid => MemoryCycleRevertError::Invalid,
        _ => MemoryCycleRevertError::Storage,
    }
}

fn memory_error(error: MemoryRepositoryError) -> MemoryCycleRevertError {
    match error {
        MemoryRepositoryError::NotFound => MemoryCycleRevertError::NotFound,
        MemoryRepositoryError::Conflict | MemoryRepositoryError::AlreadyExists => {
            MemoryCycleRevertError::Conflict
        }
        MemoryRepositoryError::Invalid(_) => MemoryCycleRevertError::Invalid,
        MemoryRepositoryError::Failure(_) => MemoryCycleRevertError::Storage,
    }
}

pub(crate) const UNDONE: &str = "(EXISTS (SELECT 1 FROM dynamic_memory_cycle_reverts revert WHERE revert.run_id = run.id)
     OR EXISTS (SELECT 1 FROM dynamic_memory_suffix_rewinds rewind
                  JOIN dynamic_memory_runs invalid ON invalid.id = rewind.invalid_run_id
                 WHERE rewind.space_id = run.space_id AND invalid.branch_id = run.branch_id
                   AND invalid.conversation_id = run.conversation_id
                   AND invalid.created_at <= run.created_at AND rewind.applied_at >= run.created_at))";

const EFFECTIVE: &str = "(EXISTS (SELECT 1 FROM dynamic_memory_run_attempts attempt WHERE attempt.run_id = run.id AND attempt.status IN ('created','processing'))
     OR EXISTS (SELECT 1 FROM dynamic_memory_background_tool_results result WHERE result.run_id = run.id)
     OR EXISTS (SELECT 1 FROM dynamic_memory_summary_checkpoints checkpoint WHERE checkpoint.run_id = run.id)
     OR EXISTS (SELECT 1 FROM dynamic_memory_changed_items changed WHERE changed.run_id = run.id))";

pub(crate) fn insert_revert_record_in(
    transaction: &Transaction<'_>,
    record: &MemoryCycleRevertRecord,
) -> Result<(), MemoryRepositoryError> {
    record.validate()?;
    transaction
        .execute(
            "INSERT INTO dynamic_memory_cycle_reverts
                (run_id,conversation_id,space_id,source_memory_revision,
                 resulting_memory_revision,restored_summary_run_id,reverted_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![
                record.run_id.to_string(),
                record.conversation_id.to_string(),
                record.space_id.to_string(),
                i64::try_from(record.source_revision.get())
                    .map_err(|_| MemoryRepositoryError::Failure("revision overflow".into()))?,
                i64::try_from(record.resulting_revision.get())
                    .map_err(|_| MemoryRepositoryError::Failure("revision overflow".into()))?,
                record.restored_summary_run_id.map(|id| id.to_string()),
                record.reverted_at.get(),
            ],
        )
        .map_err(|_| MemoryRepositoryError::Failure("cycle revert write failed".into()))?;
    Ok(())
}

pub(crate) fn revert_records_in(
    transaction: &Transaction<'_>,
) -> Result<Vec<MemoryCycleRevertRecord>, MemoryRepositoryError> {
    let failure =
        |_: &dyn std::fmt::Debug| MemoryRepositoryError::Failure("cycle revert read failed".into());
    let rows = transaction
        .prepare(
            "SELECT run_id,conversation_id,space_id,source_memory_revision,
                    resulting_memory_revision,restored_summary_run_id,reverted_at
               FROM dynamic_memory_cycle_reverts ORDER BY reverted_at,run_id",
        )
        .and_then(|mut statement| {
            statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, i64>(6)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(|error| failure(&error))?;
    rows.into_iter()
        .map(
            |(run, conversation, space, source, resulting, restored, at)| {
                Ok(MemoryCycleRevertRecord {
                    run_id: run.parse().map_err(|error| failure(&error))?,
                    conversation_id: conversation.parse().map_err(|error| failure(&error))?,
                    space_id: space.parse().map_err(|error| failure(&error))?,
                    source_revision: Revision::new(
                        u64::try_from(source).map_err(|error| failure(&error))?,
                    ),
                    resulting_revision: Revision::new(
                        u64::try_from(resulting).map_err(|error| failure(&error))?,
                    ),
                    restored_summary_run_id: restored
                        .map(|id| id.parse())
                        .transpose()
                        .map_err(|error| failure(&error))?,
                    reverted_at: TimestampMillis::new(at),
                })
            },
        )
        .collect()
}

pub(crate) fn dismissal_records_in(
    transaction: &Transaction<'_>,
) -> Result<Vec<(MemorySpaceId, JobId, TimestampMillis)>, MemoryRepositoryError> {
    let failure = |_: &dyn std::fmt::Debug| {
        MemoryRepositoryError::Failure("error dismissal read failed".into())
    };
    let rows = transaction
        .prepare(
            "SELECT space_id,job_id,dismissed_at FROM memory_error_dismissals ORDER BY space_id",
        )
        .and_then(|mut statement| {
            statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(|error| failure(&error))?;
    rows.into_iter()
        .map(|(space, job, at)| {
            Ok((
                space.parse().map_err(|error| failure(&error))?,
                job.parse().map_err(|error| failure(&error))?,
                TimestampMillis::new(at),
            ))
        })
        .collect()
}

pub(crate) fn insert_dismissal_in(
    transaction: &Transaction<'_>,
    space_id: MemorySpaceId,
    job_id: JobId,
    at: TimestampMillis,
) -> Result<(), MemoryRepositoryError> {
    transaction
        .execute(
            "INSERT INTO memory_error_dismissals (space_id,job_id,dismissed_at) VALUES (?1,?2,?3)
             ON CONFLICT(space_id) DO UPDATE SET job_id=excluded.job_id,dismissed_at=excluded.dismissed_at",
            params![space_id.to_string(), job_id.to_string(), at.get()],
        )
        .map_err(|_| MemoryRepositoryError::Failure("error dismissal write failed".into()))?;
    Ok(())
}

fn active_space(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
) -> Result<MemorySpaceId, MemoryRepositoryError> {
    let branch: Option<String> = transaction
        .query_row(
            "SELECT conversation.active_branch_id FROM conversations conversation
               JOIN conversation_branches branch
                 ON branch.conversation_id = conversation.id AND branch.id = conversation.active_branch_id
              WHERE conversation.id = ?1 AND conversation.lifecycle <> 'tombstoned'
                AND branch.status = 'active'",
            [conversation_id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| MemoryRepositoryError::Failure("conversation read failed".into()))?;
    let branch = branch
        .ok_or(MemoryRepositoryError::NotFound)?
        .parse()
        .map_err(|_| MemoryRepositoryError::Failure("branch id is invalid".into()))?;
    memory_adapter::branch_space_id_in(transaction, conversation_id, branch)
        .map_err(|_| MemoryRepositoryError::Failure("memory space read failed".into()))?
        .ok_or(MemoryRepositoryError::NotFound)
}

impl ApiOperationTransaction<'_, '_> {
    /// Hides the failure of `job_id` from the active space's status. The
    /// space is the conversation's active resolved one, so a shared pool
    /// dismisses it for every chat that shares it.
    pub fn dismiss_memory_error(
        &self,
        conversation_id: ConversationId,
        job_id: JobId,
        at: TimestampMillis,
    ) -> Result<(), MemoryRepositoryError> {
        let space_id = active_space(self.transaction, conversation_id)?;
        insert_dismissal_in(self.transaction, space_id, job_id, at)
    }

    /// Undoes one finished cycle's recorded outcomes and the summary it
    /// published, only while no later cycle started from its result.
    pub fn revert_memory_cycle(
        &self,
        revert: &MemoryCycleRevert,
    ) -> Result<MemoryCycleRevertRecord, MemoryCycleRevertError> {
        let transaction = self.transaction;
        let space_id = active_space(transaction, revert.conversation_id).map_err(memory_error)?;
        let run = dynamic_memory_run_adapter::load_run_in(transaction, revert.run_id).map_err(
            |error| match error {
                lettuce_memory::DynamicMemoryRunRepositoryError::NotFound => {
                    MemoryCycleRevertError::NotFound
                }
                _ => MemoryCycleRevertError::Storage,
            },
        )?;
        if run.space_id != space_id {
            return Err(MemoryCycleRevertError::NotFound);
        }
        let flags: (bool, bool, bool) = transaction
            .query_row(
                &format!(
                    "SELECT {UNDONE},
                            EXISTS (SELECT 1 FROM dynamic_memory_run_attempts attempt WHERE attempt.run_id = run.id AND attempt.status IN ('created','processing')),
                            EXISTS (SELECT 1 FROM dynamic_memory_background_tool_results result WHERE result.run_id = run.id)
                         OR EXISTS (SELECT 1 FROM dynamic_memory_summary_checkpoints checkpoint WHERE checkpoint.run_id = run.id)
                       FROM dynamic_memory_runs run WHERE run.id = ?1"
                ),
                [run.id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(storage)?;
        let (undone, running, recorded) = flags;
        if undone {
            return Err(MemoryCycleRevertError::AlreadyReverted);
        }
        if running {
            return Err(MemoryCycleRevertError::Running);
        }
        if !recorded {
            return Err(MemoryCycleRevertError::NothingToRevert);
        }
        let current = memory_adapter::get_in(transaction, space_id)
            .map_err(memory_error)?
            .ok_or(MemoryCycleRevertError::NotFound)?;
        if current.revision != revert.expected_revision {
            return Err(MemoryCycleRevertError::Conflict);
        }
        let later: Option<String> = transaction
            .query_row(
                &format!(
                    "SELECT run.id FROM dynamic_memory_runs run
                      WHERE run.space_id = ?1
                        AND (run.created_at > ?2 OR (run.created_at = ?2 AND run.id > ?3))
                        AND NOT {UNDONE} AND {EFFECTIVE}
                      ORDER BY run.created_at, run.id LIMIT 1"
                ),
                params![
                    space_id.to_string(),
                    run.created_at.get(),
                    run.id.to_string()
                ],
                |row| row.get(0),
            )
            .optional()
            .map_err(storage)?;
        if let Some(later) = later {
            return Err(MemoryCycleRevertError::Dependent {
                later_run_id: later.parse().map_err(storage)?,
            });
        }
        let changed = dynamic_memory_run_adapter::changed_items_in(transaction, run.id)
            .map_err(storage)?;
        let edited = memory_adapter::manual_edited_items_in(
            transaction,
            space_id,
            run.starting_memory.revision,
        )
        .map_err(memory_error)?;
        if let Some(memory_id) = changed.iter().find(|id| edited.contains(id)) {
            return Err(MemoryCycleRevertError::UserEdited {
                memory_id: *memory_id,
            });
        }
        let mut summary =
            memory_adapter::get_summary_in(transaction, space_id).map_err(memory_error)?;
        let memory = dynamic_memory_rewind_adapter::undo_events(
            transaction,
            &current,
            run.conversation_id,
            vec![run.id],
            &[],
            &mut summary,
            true,
        )
        .map_err(rewind_error)?;
        let pooled: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM companion_memory_pools WHERE space_id = ?1)",
                [space_id.to_string()],
                |row| row.get(0),
            )
            .map_err(storage)?;
        let mut restored_summary_run_id: Option<DynamicMemoryRunId> = None;
        if !pooled
            && let Some(checkpoint) =
                dynamic_memory_run_adapter::load_summary_checkpoint_in(transaction, run.id)
                    .map_err(|_| MemoryCycleRevertError::Storage)?
            && let Some(published) = &summary
            && published.origin == MemoryOrigin::Model
            && published.text == checkpoint.summary.text
            && published.window_end == checkpoint.summary.window_end
        {
            let (prior_run_id, mut prior) = dynamic_memory_rewind_adapter::prior_summary(
                transaction,
                run.conversation_id,
                space_id,
                run.id,
                run.summary_window.start,
            )
            .map_err(rewind_error)?;
            if prior_run_id.is_none() {
                prior = memory_branch_adapter::inherited_summary_in(
                    transaction,
                    run.conversation_id,
                    run.branch_id,
                )
                .map_err(storage)?
                .filter(|inherited| inherited.window_end <= run.summary_window.start)
                .map(|mut inherited| {
                    inherited.space_id = space_id;
                    inherited.branch_id = run.branch_id;
                    inherited
                });
            }
            memory_adapter::replace_summary_in(transaction, space_id, prior.as_ref())
                .map_err(memory_error)?;
            restored_summary_run_id = prior_run_id;
        }
        transaction
            .execute(
                "DELETE FROM memory_synced_cursors WHERE conversation_id = ?1 AND branch_id = ?2",
                params![run.conversation_id.to_string(), run.branch_id.to_string()],
            )
            .map_err(storage)?;
        let record = MemoryCycleRevertRecord {
            run_id: run.id,
            conversation_id: run.conversation_id,
            space_id,
            source_revision: current.revision,
            resulting_revision: memory.revision,
            restored_summary_run_id,
            reverted_at: revert.at,
        };
        insert_revert_record_in(transaction, &record).map_err(memory_error)?;
        Ok(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;
    use crate::memory::dynamic_memory_run_adapter::tests::{
        checkpointed_window, conversation_fixture, finish, fixture_branch,
    };
    use lettuce_memory::*;
    use lettuce_types::*;

    #[derive(Debug)]
    enum Failure {
        Revert(MemoryCycleRevertError),
        Operation,
    }
    impl From<crate::ApiOperationError> for Failure {
        fn from(_: crate::ApiOperationError) -> Self {
            Self::Operation
        }
    }
    impl From<MemoryCycleRevertError> for Failure {
        fn from(error: MemoryCycleRevertError) -> Self {
            Self::Revert(error)
        }
    }
    impl From<MemoryRepositoryError> for Failure {
        fn from(_: MemoryRepositoryError) -> Self {
            Self::Operation
        }
    }

    struct Cycle {
        run_id: DynamicMemoryRunId,
        item: MemoryId,
    }

    #[allow(clippy::too_many_arguments)]
    fn cycle(
        database: &Database,
        conversation_id: ConversationId,
        space_id: MemorySpaceId,
        messages: &[DynamicMemorySourceMessage],
        summary: &str,
        text: &str,
        at: i64,
        start: u64,
    ) -> Cycle {
        cycle_with_supersedes(database, conversation_id, space_id, messages, summary, text, at, start, None)
    }

    #[allow(clippy::too_many_arguments)]
    fn cycle_with_supersedes(
        database: &Database,
        conversation_id: ConversationId,
        space_id: MemorySpaceId,
        messages: &[DynamicMemorySourceMessage],
        summary: &str,
        text: &str,
        at: i64,
        start: u64,
        supersedes: Option<MemoryId>,
    ) -> Cycle {
        let attempt = checkpointed_window(
            database,
            conversation_id,
            space_id,
            messages,
            summary,
            at,
            start,
        );
        let call_id = ToolExecutionId::new();
        database
            .admit_dynamic_memory_inference_round(
                attempt.run_id,
                attempt.id,
                0,
                0,
                NewDynamicMemoryInferenceRound {
                    ordinal: 0,
                    request_context: lettuce_conversations::ProviderNeutralContext {
                        messages: vec![],
                        attributions: Default::default(),
                        budget: Default::default(),
                    },
                    parts: vec![],
                    provider_replay: None,
                    usage: None,
                    finish_reason: DynamicMemoryRoundFinishReason::Stop,
                    kind: DynamicMemoryRoundKind::Manager,
                    provider_request_id: None,
                    calls: vec![NewDynamicMemoryToolCall {
                        id: call_id,
                        definition_version: 1,
                        call: lettuce_conversations::ProposedToolCall {
                            provider_call_id: Some("create".into()),
                            name: "create_memory".into(),
                            arguments: serde_json::json!({"text": text}),
                            raw_arguments: None,
                            provider_replay: None,
                        },
                    }],
                    admitted_at: TimestampMillis::new(at + 2),
                },
            )
            .expect("round");
        let memory = database.get(space_id).expect("memory").expect("space");
        let id = MemoryId::new();
        let mut item = MemoryItem::written(
            id,
            MemoryShortId::derived(id),
            text.into(),
            TimestampMillis::new(at + 3),
        );
        let mut items = memory.items.clone();
        if let Some(target) = supersedes {
            item.supersedes.push(target);
            let previous = items.iter_mut().find(|item| item.id == target).expect("superseded target");
            previous.superseded_by = Some(id);
            previous.superseded_at = Some(TimestampMillis::new(at + 3));
        }
        items.push(item.clone());
        database
            .commit_dynamic_memory_background_round(
                DynamicMemoryBackgroundRoundCommit {
                    run_id: attempt.run_id,
                    attempt_id: attempt.id,
                    round_ordinal: 0,
                    space_id,
                    expected_memory_revision: memory.revision,
                    change: Some(MemoryChangeSet {
                        space_id,
                        expected_revision: memory.revision,
                        items,
                    }),
                    results: vec![MemoryToolResult {
                        execution_id: call_id,
                        outcome: MemoryToolOutcome::Created {
                            id,
                            short_id: item.short_id,
                            memories: vec![ListedMemory {
                                short_id: item.short_id,
                                text: text.into(),
                            }],
                        },
                    }],
                },
                TimestampMillis::new(at + 3),
            )
            .expect("create tool");
        finish(database, &attempt, true, at + 4);
        Cycle {
            run_id: attempt.run_id,
            item: id,
        }
    }

    fn revert(
        database: &Database,
        conversation_id: ConversationId,
        run_id: DynamicMemoryRunId,
        key: &str,
        at: i64,
    ) -> Result<MemoryCycleRevertRecord, Failure> {
        let space = database
            .connection()
            .expect("connection")
            .query_row(
                "SELECT space_id FROM dynamic_memory_runs WHERE id=?1",
                [run_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .expect("space");
        let revision = database
            .get(space.parse().expect("space"))
            .expect("memory")
            .expect("space")
            .revision;
        database.commit_api_operation(
            "memory_cycle_revert",
            key,
            "digest",
            TimestampMillis::new(at),
            |scope| {
                scope
                    .revert_memory_cycle(&MemoryCycleRevert {
                        conversation_id,
                        run_id,
                        expected_revision: revision,
                        at: TimestampMillis::new(at),
                    })
                    .map_err(Failure::from)
            },
        )
    }

    fn two_cycles(database: &Database) -> (ConversationId, MemorySpaceId, Cycle, Cycle) {
        let (conversation_id, space_id, messages) = conversation_fixture(database);
        let first = cycle(
            database,
            conversation_id,
            space_id,
            &messages,
            "First summary",
            "First fact",
            100,
            0,
        );
        let second = cycle(
            database,
            conversation_id,
            space_id,
            &messages,
            "Second summary",
            "Second fact",
            200,
            2,
        );
        (conversation_id, space_id, first, second)
    }

    #[test]
    fn a_middle_cycle_with_a_later_cycle_is_refused_naming_it_and_writes_nothing() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, space_id, first, second) = two_cycles(&database);
        let before = database.get(space_id).expect("memory").expect("space");
        let summary_before = MemorySummaryRepository::get_summary(&database, space_id).expect("s");
        match revert(&database, conversation_id, first.run_id, "middle", 300) {
            Err(Failure::Revert(MemoryCycleRevertError::Dependent { later_run_id })) => {
                assert_eq!(later_run_id, second.run_id);
            }
            other => panic!("expected the later cycle to block the revert, got {other:?}"),
        }
        assert_eq!(
            database.get(space_id).expect("memory").expect("space"),
            before
        );
        assert_eq!(
            MemorySummaryRepository::get_summary(&database, space_id).expect("s"),
            summary_before
        );
        assert!(
            revert_records_in(&database.connection().expect("c").transaction().expect("t"))
                .expect("records")
                .is_empty()
        );
    }

    #[test]
    fn reverting_the_latest_cycle_undoes_its_items_restores_the_prior_summary_and_the_cursor() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, space_id, first, second) = two_cycles(&database);
        let branch_id = fixture_branch(&database, conversation_id);
        let before = database.get(space_id).expect("memory").expect("space");
        assert!(before.items.iter().any(|item| item.id == second.item));
        let record = revert(&database, conversation_id, second.run_id, "latest", 300)
            .expect("revert the latest cycle");
        assert_eq!(record.source_revision, before.revision);
        assert_eq!(
            record.resulting_revision,
            before.revision.next().expect("next")
        );
        assert_eq!(record.restored_summary_run_id, Some(first.run_id));
        let after = database.get(space_id).expect("memory").expect("space");
        assert_eq!(after.revision, record.resulting_revision);
        assert!(after.items.iter().all(|item| item.id != second.item));
        assert!(after.items.iter().any(|item| item.id == first.item));
        assert_eq!(
            MemorySummaryRepository::get_summary(&database, space_id)
                .expect("summary")
                .expect("restored")
                .text,
            "First summary"
        );
        let window_end = MemorySummaryRepository::summary_cursor(
            &database,
            space_id,
            conversation_id,
            branch_id,
        )
        .expect("cursor");
        assert_eq!(window_end, 2);
        assert!(matches!(
            revert(&database, conversation_id, second.run_id, "again", 400),
            Err(Failure::Revert(MemoryCycleRevertError::AlreadyReverted))
        ));
        revert(&database, conversation_id, first.run_id, "now-latest", 500)
            .expect("the earlier cycle is revertable once the later one is reverted");
        assert!(
            MemorySummaryRepository::get_summary(&database, space_id)
                .expect("summary")
                .is_none()
        );
        assert!(
            database
                .get(space_id)
                .expect("memory")
                .expect("space")
                .items
                .iter()
                .all(|item| item.id != first.item)
        );
    }

    #[test]
    fn a_stale_revision_and_a_failed_receipt_write_leave_memory_untouched() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, space_id, _, second) = two_cycles(&database);
        let before = database.get(space_id).expect("memory").expect("space");
        let stale = database.commit_api_operation(
            "memory_cycle_revert",
            "stale",
            "digest",
            TimestampMillis::new(300),
            |scope| {
                scope
                    .revert_memory_cycle(&MemoryCycleRevert {
                        conversation_id,
                        run_id: second.run_id,
                        expected_revision: Revision::INITIAL,
                        at: TimestampMillis::new(300),
                    })
                    .map_err(Failure::from)
            },
        );
        assert!(matches!(
            stale,
            Err(Failure::Revert(MemoryCycleRevertError::Conflict))
        ));
        database
            .connection()
            .expect("connection")
            .execute_batch(
                "CREATE TRIGGER fail_revert_marker BEFORE INSERT ON dynamic_memory_cycle_reverts
                 BEGIN SELECT RAISE(ABORT,'injected marker failure'); END;",
            )
            .expect("inject");
        assert!(revert(&database, conversation_id, second.run_id, "crash", 300).is_err());
        assert_eq!(
            database.get(space_id).expect("memory").expect("space"),
            before
        );
        assert_eq!(
            MemorySummaryRepository::get_summary(&database, space_id)
                .expect("summary")
                .expect("kept")
                .text,
            "Second summary"
        );
    }

    #[test]
    fn a_user_summary_written_after_the_cycle_survives_its_revert() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, space_id, _, second) = two_cycles(&database);
        let branch_id = fixture_branch(&database, conversation_id);
        let memory = database.get(space_id).expect("memory").expect("space");
        let edit = MemoryManualEdit {
            id: OperationId::new(),
            conversation_id,
            branch_id,
            conversation_revision: Revision::INITIAL,
            expected_revision: memory.revision,
            space_id,
            context_revisions: vec![],
            mutation: MemoryManualMutation::Summary {
                summary: Some(MemorySummary {
                    origin: MemoryOrigin::User,
                    space_id,
                    branch_id,
                    text: "Mine".into(),
                    token_count: None,
                    window_start: 0,
                    window_end: 0,
                    source_message_ids: vec![],
                    updated_at: TimestampMillis::new(250),
                }),
            },
            at: TimestampMillis::new(250),
        };
        database
            .commit_api_operation::<MemoryManualHistory, Failure>(
                "memory_summary_update",
                "user-summary",
                "digest",
                edit.at,
                |scope| {
                    scope
                        .apply_memory_manual_edit(&edit, None)
                        .map_err(Failure::from)
                },
            )
            .expect("user summary");
        revert(&database, conversation_id, second.run_id, "keep-user", 300).expect("revert");
        let summary = MemorySummaryRepository::get_summary(&database, space_id)
            .expect("summary")
            .expect("kept");
        assert_eq!(summary.text, "Mine");
        assert_eq!(summary.origin, MemoryOrigin::User);
    }

    #[test]
    fn a_later_user_edit_of_an_item_the_cycle_created_refuses_the_revert_naming_it() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, space_id, _, second) = two_cycles(&database);
        let branch_id = fixture_branch(&database, conversation_id);
        let memory = database.get(space_id).expect("memory").expect("space");
        let edit = MemoryManualEdit {
            id: OperationId::new(),
            conversation_id,
            branch_id,
            conversation_revision: Revision::INITIAL,
            expected_revision: memory.revision,
            space_id,
            context_revisions: vec![],
            mutation: MemoryManualMutation::Pin {
                memory_id: second.item,
                pinned: true,
            },
            at: TimestampMillis::new(250),
        };
        database
            .commit_api_operation::<MemoryManualHistory, Failure>(
                "memory_pin",
                "pin-created",
                "digest",
                edit.at,
                |scope| {
                    scope
                        .apply_memory_manual_edit(&edit, None)
                        .map_err(Failure::from)
                },
            )
            .expect("user pin");
        let before = database.get(space_id).expect("memory").expect("space");
        match revert(&database, conversation_id, second.run_id, "pinned", 300) {
            Err(Failure::Revert(MemoryCycleRevertError::UserEdited { memory_id })) => {
                assert_eq!(memory_id, second.item);
            }
            other => panic!("expected the user edit to refuse the revert, got {other:?}"),
        }
        assert_eq!(
            database.get(space_id).expect("memory").expect("space"),
            before
        );
    }
    #[test]
    fn a_user_edit_of_a_superseded_item_refuses_the_cycle_revert() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, space_id, first, second) = two_cycles(&database);
        let run = database.load_dynamic_memory_run(second.run_id).expect("run");
        let third = cycle_with_supersedes(&database, conversation_id, space_id,
            &run.source_messages, "Third summary", "Superseding item", 300, 2, Some(first.item));
        let before = database.get(space_id).expect("memory").expect("space");
        let edit = MemoryManualEdit {
            id: OperationId::new(), conversation_id,
            branch_id: fixture_branch(&database, conversation_id),
            conversation_revision: Revision::INITIAL,
            expected_revision: before.revision, space_id, context_revisions: vec![],
            mutation: MemoryManualMutation::Pin { memory_id: first.item, pinned: true },
            at: TimestampMillis::new(350),
        };
        database.commit_api_operation::<MemoryManualHistory, Failure>(
            "memory_pin", "pin-superseded", "digest", edit.at,
            |scope| scope.apply_memory_manual_edit(&edit, None).map_err(Failure::from),
        ).expect("user edit");
        let before = database.get(space_id).expect("memory").expect("space");
        match revert(&database, conversation_id, third.run_id, "superseded", 400) {
            Err(Failure::Revert(MemoryCycleRevertError::UserEdited { memory_id })) => assert_eq!(memory_id, first.item),
            other => panic!("expected superseded item dependency, got {other:?}"),
        }
        assert_eq!(database.get(space_id).expect("memory").expect("space"), before);
    }

}
