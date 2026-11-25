use std::collections::HashSet;

use lettuce_memory::{
    MemoryOrigin, MemoryRepositoryError, MemorySummaryUnit, MemorySummaryUnitKind,
    MemorySummaryUnitRepository, MemorySummaryUnitScope, MemoryValidationError,
    NewMemorySummaryUnit,
};
use lettuce_types::{
    ConversationBranchId, ConversationId, DynamicMemoryRunId, MemorySpaceId, MemorySummaryUnitId,
    TimestampMillis,
};
use rusqlite::{OptionalExtension, Row, TransactionBehavior, params};

use crate::Database;

use super::memory_cycle_revert_adapter::UNDONE;

fn storage(error: impl std::fmt::Debug) -> MemoryRepositoryError {
    tracing::warn!(?error, "memory summary unit adapter storage step failed");
    MemoryRepositoryError::Failure("memory summary unit storage step failed".into())
}

fn parse_unit_row(row: &Row<'_>) -> Result<MemorySummaryUnit, MemoryRepositoryError> {
    let id: String = row.get(0).map_err(storage)?;
    let space_id: String = row.get(1).map_err(storage)?;
    let conversation_id: String = row.get(2).map_err(storage)?;
    let branch_id: String = row.get(3).map_err(storage)?;
    let created_by_run_id: String = row.get(4).map_err(storage)?;
    let kind: String = row.get(5).map_err(storage)?;
    let level: i64 = row.get(6).map_err(storage)?;
    let origin: String = row.get(7).map_err(storage)?;
    let window_start: i64 = row.get(8).map_err(storage)?;
    let window_end: i64 = row.get(9).map_err(storage)?;
    let text: String = row.get(10).map_err(storage)?;
    let token_count: i64 = row.get(11).map_err(storage)?;
    let created_at: i64 = row.get(12).map_err(storage)?;
    let time_start: Option<i64> = row.get(13).map_err(storage)?;
    let time_end: Option<i64> = row.get(14).map_err(storage)?;

    let unit = MemorySummaryUnit {
        id: id.parse::<MemorySummaryUnitId>().map_err(storage)?,
        scope: MemorySummaryUnitScope {
            space_id: space_id.parse::<MemorySpaceId>().map_err(storage)?,
            conversation_id: conversation_id.parse::<ConversationId>().map_err(storage)?,
            branch_id: branch_id.parse::<ConversationBranchId>().map_err(storage)?,
        },
        created_by_run_id: created_by_run_id
            .parse::<DynamicMemoryRunId>()
            .map_err(storage)?,
        kind: MemorySummaryUnitKind::parse(&kind).ok_or_else(|| {
            MemoryRepositoryError::Invalid(MemoryValidationError::InvalidSummaryUnit {
                reason: "unknown summary unit kind",
            })
        })?,
        level: u32::try_from(level).map_err(storage)?,
        origin: MemoryOrigin::parse(&origin).ok_or_else(|| {
            MemoryRepositoryError::Invalid(MemoryValidationError::InvalidSummaryUnit {
                reason: "unknown summary unit origin",
            })
        })?,
        window_start: u64::try_from(window_start).map_err(storage)?,
        window_end: u64::try_from(window_end).map_err(storage)?,
        time_start: time_start.map(TimestampMillis::new),
        time_end: time_end.map(TimestampMillis::new),
        text,
        token_count: u32::try_from(token_count).map_err(storage)?,
        created_at: TimestampMillis::new(created_at),
    };
    unit.validate()?;
    Ok(unit)
}

fn validate_active_units(units: &[MemorySummaryUnit]) -> Result<(), MemoryRepositoryError> {
    let mut seed_or_reset_count = 0usize;
    for unit in units {
        if matches!(
            unit.kind,
            MemorySummaryUnitKind::Seed | MemorySummaryUnitKind::Reset
        ) {
            seed_or_reset_count += 1;
            if seed_or_reset_count > 1 {
                return Err(MemoryRepositoryError::Invalid(
                    MemoryValidationError::InvalidSummaryUnit {
                        reason: "multiple active seed or reset units",
                    },
                ));
            }
            continue;
        }
        if unit.window_end <= unit.window_start {
            return Err(MemoryRepositoryError::Invalid(
                MemoryValidationError::InvalidSummaryWindow {
                    reason: "active summary unit window is empty",
                },
            ));
        }
    }
    Ok(())
}

impl MemorySummaryUnitRepository for Database {
    fn active_summary_units(
        &self,
        scope: MemorySummaryUnitScope,
        current_run_id: Option<DynamicMemoryRunId>,
    ) -> Result<Vec<MemorySummaryUnit>, MemoryRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        let query = format!(
            "SELECT u.id, u.space_id, u.conversation_id, u.branch_id, u.created_by_run_id,
                    u.kind, u.level, u.origin, u.window_start, u.window_end,
                    u.text, u.token_count, u.created_at, u.time_start, u.time_end
               FROM memory_summary_units u
               JOIN dynamic_memory_runs run ON run.id = u.created_by_run_id
              WHERE u.space_id = ?1
                AND u.conversation_id = ?2
                AND u.branch_id = ?3
                AND NOT {UNDONE}
                AND (run.id = ?4 OR EXISTS (
                    SELECT 1 FROM dynamic_memory_summary_checkpoints checkpoint
                     WHERE checkpoint.run_id = run.id
                ))
                AND NOT EXISTS (
                    SELECT 1
                      FROM memory_summary_unit_inputs edge
                      JOIN memory_summary_units parent ON parent.id = edge.parent_id
                      JOIN dynamic_memory_runs run ON run.id = parent.created_by_run_id
                     WHERE edge.child_id = u.id
                       AND parent.space_id = u.space_id
                       AND parent.conversation_id = u.conversation_id
                       AND parent.branch_id = u.branch_id
                       AND NOT {UNDONE}
                       AND (run.id = ?4 OR EXISTS (
                           SELECT 1 FROM dynamic_memory_summary_checkpoints checkpoint
                            WHERE checkpoint.run_id = run.id
                       ))
                )
              ORDER BY CASE WHEN u.kind IN ('seed', 'reset') THEN 0 ELSE 1 END,
                       u.window_start ASC,
                       u.window_end ASC,
                       u.id ASC"
        );
        let mut statement = connection.prepare(&query).map_err(storage)?;
        let mut rows = statement
            .query(params![
                scope.space_id.to_string(),
                scope.conversation_id.to_string(),
                scope.branch_id.to_string(),
                current_run_id.map(|id| id.to_string()),
            ])
            .map_err(storage)?;
        let mut units = Vec::new();
        while let Some(row) = rows.next().map_err(storage)? {
            units.push(parse_unit_row(row)?);
        }
        validate_active_units(&units)?;
        Ok(units)
    }

    fn insert_summary_unit(
        &self,
        unit: NewMemorySummaryUnit,
        inputs: &[MemorySummaryUnitId],
    ) -> Result<MemorySummaryUnit, MemoryRepositoryError> {
        unit.validate()?;
        let mut seen_inputs = HashSet::with_capacity(inputs.len());
        if inputs.iter().any(|id| !seen_inputs.insert(*id)) {
            return Err(MemoryRepositoryError::Invalid(
                MemoryValidationError::InvalidSummaryUnit {
                    reason: "duplicate summary unit input",
                },
            ));
        }

        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;

        let existing = transaction
            .query_row(
                "SELECT id, space_id, conversation_id, branch_id, created_by_run_id,
                        kind, level, origin, window_start, window_end,
                        text, token_count, created_at, time_start, time_end
                   FROM memory_summary_units
                  WHERE created_by_run_id = ?1
                    AND kind = ?2
                    AND level = ?3
                    AND window_start = ?4
                    AND window_end = ?5",
                params![
                    unit.created_by_run_id.to_string(),
                    unit.kind.as_str(),
                    i64::from(unit.level),
                    i64::try_from(unit.window_start).map_err(storage)?,
                    i64::try_from(unit.window_end).map_err(storage)?,
                ],
                |row| Ok(parse_unit_row(row)),
            )
            .optional()
            .map_err(storage)?
            .transpose()?;

        if let Some(existing) = existing {
            let mut statement = transaction
                .prepare(
                    "SELECT child_id
                       FROM memory_summary_unit_inputs
                      WHERE parent_id = ?1
                      ORDER BY ordinal ASC",
                )
                .map_err(storage)?;
            let existing_inputs = statement
                .query_map([existing.id.to_string()], |row| row.get::<_, String>(0))
                .map_err(storage)?
                .map(|value| {
                    value
                        .map_err(storage)?
                        .parse::<MemorySummaryUnitId>()
                        .map_err(storage)
                })
                .collect::<Result<Vec<_>, _>>()?;
            if existing.scope == unit.scope
                && existing.origin == unit.origin
                && existing.text == unit.text
                && existing.kind == unit.kind
                && existing.level == unit.level
                && existing.window_start == unit.window_start
                && existing.window_end == unit.window_end
                && existing.time_start == unit.time_start
                && existing.time_end == unit.time_end
                && existing_inputs == inputs
            {
                return Ok(existing);
            }
            return Err(MemoryRepositoryError::Conflict);
        }

        transaction
            .execute(
                "INSERT INTO memory_summary_units (
                    id, space_id, conversation_id, branch_id, created_by_run_id,
                    kind, level, origin, window_start, window_end,
                    text, token_count, created_at, time_start, time_end
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                params![
                    unit.id.to_string(),
                    unit.scope.space_id.to_string(),
                    unit.scope.conversation_id.to_string(),
                    unit.scope.branch_id.to_string(),
                    unit.created_by_run_id.to_string(),
                    unit.kind.as_str(),
                    i64::from(unit.level),
                    unit.origin.as_str(),
                    i64::try_from(unit.window_start).map_err(storage)?,
                    i64::try_from(unit.window_end).map_err(storage)?,
                    unit.text,
                    i64::from(unit.token_count),
                    unit.created_at.get(),
                    unit.time_start.map(TimestampMillis::get),
                    unit.time_end.map(TimestampMillis::get),
                ],
            )
            .map_err(storage)?;

        for (ordinal, child_id) in inputs.iter().enumerate() {
            transaction
                .execute(
                    "INSERT INTO memory_summary_unit_inputs (parent_id, child_id, ordinal)
                     VALUES (?1, ?2, ?3)",
                    params![
                        unit.id.to_string(),
                        child_id.to_string(),
                        i64::try_from(ordinal).map_err(storage)?,
                    ],
                )
                .map_err(storage)?;
        }

        transaction.commit().map_err(storage)?;
        Ok(unit.into_unit())
    }

    fn summary_units_created_by(
        &self,
        run_id: DynamicMemoryRunId,
    ) -> Result<Vec<(MemorySummaryUnit, u32)>, MemoryRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        let mut statement = connection
            .prepare(
                "SELECT u.id, u.space_id, u.conversation_id, u.branch_id, u.created_by_run_id,
                        u.kind, u.level, u.origin, u.window_start, u.window_end,
                        u.text, u.token_count, u.created_at, u.time_start, u.time_end,
                        (SELECT count(*) FROM memory_summary_unit_inputs edge
                          WHERE edge.parent_id = u.id)
                   FROM memory_summary_units u
                  WHERE u.created_by_run_id = ?1
                  ORDER BY u.rowid ASC",
            )
            .map_err(storage)?;
        let mut rows = statement
            .query(params![run_id.to_string()])
            .map_err(storage)?;
        let mut units = Vec::new();
        while let Some(row) = rows.next().map_err(storage)? {
            let inputs: i64 = row.get(15).map_err(storage)?;
            units.push((
                parse_unit_row(row)?,
                u32::try_from(inputs).map_err(storage)?,
            ));
        }
        Ok(units)
    }

    fn summary_unit(
        &self,
        unit_id: MemorySummaryUnitId,
    ) -> Result<Option<MemorySummaryUnit>, MemoryRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        connection
            .query_row(
                "SELECT id, space_id, conversation_id, branch_id, created_by_run_id,
                        kind, level, origin, window_start, window_end,
                        text, token_count, created_at, time_start, time_end
                   FROM memory_summary_units
                  WHERE id = ?1",
                params![unit_id.to_string()],
                |row| Ok(parse_unit_row(row)),
            )
            .optional()
            .map_err(storage)?
            .transpose()
    }

    fn summary_unit_inputs(
        &self,
        unit_id: MemorySummaryUnitId,
    ) -> Result<Vec<MemorySummaryUnit>, MemoryRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        let mut statement = connection
            .prepare(
                "SELECT u.id, u.space_id, u.conversation_id, u.branch_id, u.created_by_run_id,
                        u.kind, u.level, u.origin, u.window_start, u.window_end,
                        u.text, u.token_count, u.created_at, u.time_start, u.time_end
                   FROM memory_summary_unit_inputs edge
                   JOIN memory_summary_units u ON u.id = edge.child_id
                  WHERE edge.parent_id = ?1
                  ORDER BY edge.ordinal ASC",
            )
            .map_err(storage)?;
        let mut rows = statement
            .query(params![unit_id.to_string()])
            .map_err(storage)?;
        let mut units = Vec::new();
        while let Some(row) = rows.next().map_err(storage)? {
            units.push(parse_unit_row(row)?);
        }
        Ok(units)
    }

    fn summary_unit_input_count(
        &self,
        unit_id: MemorySummaryUnitId,
    ) -> Result<u32, MemoryRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        let count: i64 = connection
            .query_row(
                "SELECT count(*) FROM memory_summary_unit_inputs WHERE parent_id = ?1",
                params![unit_id.to_string()],
                |row| row.get(0),
            )
            .map_err(storage)?;
        u32::try_from(count).map_err(storage)
    }
}

#[cfg(test)]
mod tests {
    use lettuce_memory::{
        MemoryOrigin, MemoryRepositoryError, MemorySummaryUnitKind, MemorySummaryUnitRepository,
        MemorySummaryUnitScope, NewMemorySummaryUnit,
    };
    use lettuce_types::{
        ConversationBranchId, ConversationId, DynamicMemoryAttemptId, DynamicMemoryRunId, JobId,
        MemorySpaceId, MemorySummaryUnitId, OperationId, TimestampMillis,
    };
    use rusqlite::params;

    use crate::Database;

    struct Fixture {
        database: Database,
        scope: MemorySummaryUnitScope,
    }

    impl Fixture {
        fn new() -> Self {
            let database = Database::open_in_memory().expect("database");
            let scope = MemorySummaryUnitScope {
                space_id: MemorySpaceId::new(),
                conversation_id: ConversationId::new(),
                branch_id: ConversationBranchId::new(),
            };
            let mut connection = database.connection().expect("connection");
            let transaction = connection.transaction().expect("transaction");
            transaction
                .execute(
                    "INSERT INTO conversations (id, kind, lifecycle, title, active_branch_id, kind_json, revision, created_at, updated_at)
                     VALUES (?1, 'direct', 'active', 'Chat', ?2, '{\"format_version\":1,\"value\":{\"kind\":\"direct\"}}', 1, 1, 1)",
                    params![scope.conversation_id.to_string(), scope.branch_id.to_string()],
                )
                .expect("conversation");
            transaction
                .execute(
                    "INSERT INTO conversation_branches (conversation_id, id, status, revision, created_at, updated_at)
                     VALUES (?1, ?2, 'active', 1, 1, 1)",
                    params![scope.conversation_id.to_string(), scope.branch_id.to_string()],
                )
                .expect("branch");
            transaction
                .execute(
                    "INSERT INTO memory_spaces (id, revision) VALUES (?1, 1)",
                    [scope.space_id.to_string()],
                )
                .expect("space");
            transaction
                .execute(
                    "INSERT INTO conversation_memory_spaces (conversation_id, branch_id, space_id, pooled)
                     VALUES (?1, ?2, ?3, 0)",
                    params![
                        scope.conversation_id.to_string(),
                        scope.branch_id.to_string(),
                        scope.space_id.to_string(),
                    ],
                )
                .expect("space binding");
            transaction.commit().expect("commit");
            drop(connection);
            Self { database, scope }
        }

        fn insert_run(
            &self,
            created_at: i64,
            start: i64,
            end: i64,
        ) -> (DynamicMemoryRunId, DynamicMemoryAttemptId) {
            let run_id = DynamicMemoryRunId::new();
            let attempt_id = DynamicMemoryAttemptId::new();
            let job_id = JobId::new();
            let connection = self.database.connection().expect("connection");
            connection
                .execute(
                    "INSERT INTO dynamic_memory_runs (
                        id, conversation_id, branch_id, space_id, time_awareness_enabled,
                        supersession_enabled, structured_fallback_format, summary_message_interval,
                        summary_window_start, summary_window_end, starting_memory_json,
                        profile_json, tool_request_json, created_at
                    ) VALUES (?1, ?2, ?3, ?4, 0, 0, 'xml', 20, ?5, ?6,
                              '{\"format_version\":1,\"value\":{}}',
                              '{\"format_version\":1,\"value\":{}}',
                              '{\"format_version\":1,\"value\":{\"definitions\":[]}}', ?7)",
                    params![
                        run_id.to_string(),
                        self.scope.conversation_id.to_string(),
                        self.scope.branch_id.to_string(),
                        self.scope.space_id.to_string(),
                        start,
                        end,
                        created_at,
                    ],
                )
                .expect("run");
            connection
                .execute(
                    "INSERT INTO dynamic_memory_run_attempts (
                        job_attempt, run_id, id, ordinal, retry_parent_id, job_id, status,
                        failure, revision, created_at, started_at, finished_at, updated_at
                    ) VALUES (NULL, ?1, ?2, 0, NULL, ?3, 'succeeded', NULL, 1, ?4, ?4, ?4, ?4)",
                    params![
                        run_id.to_string(),
                        attempt_id.to_string(),
                        job_id.to_string(),
                        created_at,
                    ],
                )
                .expect("attempt");
            (run_id, attempt_id)
        }

        fn insert_checkpoint(
            &self,
            run_id: DynamicMemoryRunId,
            attempt_id: DynamicMemoryAttemptId,
            expected_rev: i64,
            settled_at: i64,
        ) {
            let connection = self.database.connection().expect("connection");
            connection
                .execute(
                    "INSERT INTO dynamic_memory_summary_checkpoints (
                        run_id, attempt_id, space_id, expected_memory_revision,
                        resulting_memory_revision, summary_text, token_count,
                        request_context_json, settled_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, 'checkpoint summary', 10,
                              '{\"format_version\":1,\"value\":{}}', ?6)",
                    params![
                        run_id.to_string(),
                        attempt_id.to_string(),
                        self.scope.space_id.to_string(),
                        expected_rev,
                        expected_rev + 1,
                        settled_at,
                    ],
                )
                .expect("checkpoint");
        }

        fn unit(
            &self,
            run_id: DynamicMemoryRunId,
            kind: MemorySummaryUnitKind,
            level: u32,
            start: u64,
            end: u64,
            text: &str,
        ) -> NewMemorySummaryUnit {
            NewMemorySummaryUnit {
                id: MemorySummaryUnitId::new(),
                scope: self.scope,
                created_by_run_id: run_id,
                kind,
                level,
                origin: MemoryOrigin::Model,
                window_start: start,
                window_end: end,
                time_start: None,
                time_end: None,
                text: text.to_owned(),
                token_count: 25,
                created_at: TimestampMillis::new(100),
            }
        }
    }

    #[test]
    fn uncheckpointed_units_are_visible_only_to_their_own_run() {
        let fixture = Fixture::new();
        let (run_a, _) = fixture.insert_run(10, 0, 20);
        let (run_b, _) = fixture.insert_run(20, 20, 40);

        let unit = fixture
            .database
            .insert_summary_unit(
                fixture.unit(run_a, MemorySummaryUnitKind::Cycle, 0, 0, 20, "Cycle A"),
                &[],
            )
            .expect("insert cycle");

        let for_a = fixture
            .database
            .active_summary_units(fixture.scope, Some(run_a))
            .expect("active for run a");
        assert_eq!(for_a, vec![unit]);

        let for_b = fixture
            .database
            .active_summary_units(fixture.scope, Some(run_b))
            .expect("active for run b");
        assert!(for_b.is_empty());

        let for_readers = fixture
            .database
            .active_summary_units(fixture.scope, None)
            .expect("active without a current run");
        assert!(for_readers.is_empty());
    }

    #[test]
    fn reverting_run_hides_chapter_and_restores_inputs_in_order() {
        let fixture = Fixture::new();
        let (run_1, attempt_1) = fixture.insert_run(10, 0, 20);
        let (run_2, attempt_2) = fixture.insert_run(20, 20, 40);
        let (run_3, attempt_3) = fixture.insert_run(30, 40, 60);
        let (run_4, _) = fixture.insert_run(40, 60, 80);

        let cycle_1 = fixture
            .database
            .insert_summary_unit(
                fixture.unit(run_1, MemorySummaryUnitKind::Cycle, 0, 0, 20, "Cycle 1"),
                &[],
            )
            .expect("cycle 1");
        fixture.insert_checkpoint(run_1, attempt_1, 1, 15);

        let cycle_2 = fixture
            .database
            .insert_summary_unit(
                fixture.unit(run_2, MemorySummaryUnitKind::Cycle, 0, 20, 40, "Cycle 2"),
                &[],
            )
            .expect("cycle 2");
        fixture.insert_checkpoint(run_2, attempt_2, 2, 25);

        let cycle_3 = fixture
            .database
            .insert_summary_unit(
                fixture.unit(run_3, MemorySummaryUnitKind::Cycle, 0, 40, 60, "Cycle 3"),
                &[],
            )
            .expect("cycle 3");
        let chapter = fixture
            .database
            .insert_summary_unit(
                fixture.unit(run_3, MemorySummaryUnitKind::Chapter, 1, 0, 40, "Chapter 1"),
                &[cycle_1.id, cycle_2.id],
            )
            .expect("chapter");
        fixture.insert_checkpoint(run_3, attempt_3, 3, 35);

        let before = fixture
            .database
            .active_summary_units(fixture.scope, Some(run_4))
            .expect("active before revert");
        assert_eq!(before, vec![chapter.clone(), cycle_3.clone()]);

        assert_eq!(
            fixture.database.summary_unit(chapter.id).expect("lookup"),
            Some(chapter.clone())
        );
        assert_eq!(
            fixture
                .database
                .summary_unit_inputs(chapter.id)
                .expect("inputs"),
            vec![cycle_1.clone(), cycle_2.clone()]
        );
        assert_eq!(
            fixture
                .database
                .summary_unit_input_count(chapter.id)
                .expect("count"),
            2
        );
        assert_eq!(
            fixture
                .database
                .summary_unit_input_count(cycle_3.id)
                .expect("count"),
            0
        );
        assert_eq!(
            fixture
                .database
                .summary_units_created_by(run_3)
                .expect("created by run 3"),
            vec![(cycle_3.clone(), 0), (chapter.clone(), 2)]
        );

        fixture
            .database
            .connection()
            .expect("connection")
            .execute(
                "INSERT INTO dynamic_memory_cycle_reverts (
                    run_id, conversation_id, space_id, source_memory_revision,
                    resulting_memory_revision, restored_summary_run_id, reverted_at
                ) VALUES (?1, ?2, ?3, 4, 5, ?4, 38)",
                params![
                    run_3.to_string(),
                    fixture.scope.conversation_id.to_string(),
                    fixture.scope.space_id.to_string(),
                    run_2.to_string(),
                ],
            )
            .expect("revert");

        let after = fixture
            .database
            .active_summary_units(fixture.scope, Some(run_4))
            .expect("active after revert");
        assert_eq!(after, vec![cycle_1, cycle_2]);
    }

    #[test]
    fn suffix_rewind_hides_units_of_every_undone_run() {
        let fixture = Fixture::new();
        let (run_1, attempt_1) = fixture.insert_run(10, 0, 20);
        let (run_2, attempt_2) = fixture.insert_run(20, 20, 40);
        let (run_3, attempt_3) = fixture.insert_run(30, 40, 60);
        let (run_4, _) = fixture.insert_run(50, 20, 40);

        let cycle_1 = fixture
            .database
            .insert_summary_unit(
                fixture.unit(run_1, MemorySummaryUnitKind::Cycle, 0, 0, 20, "Cycle 1"),
                &[],
            )
            .expect("cycle 1");
        fixture.insert_checkpoint(run_1, attempt_1, 1, 15);

        fixture
            .database
            .insert_summary_unit(
                fixture.unit(run_2, MemorySummaryUnitKind::Cycle, 0, 20, 40, "Cycle 2"),
                &[],
            )
            .expect("cycle 2");
        fixture.insert_checkpoint(run_2, attempt_2, 2, 25);

        fixture
            .database
            .insert_summary_unit(
                fixture.unit(run_3, MemorySummaryUnitKind::Cycle, 0, 40, 60, "Cycle 3"),
                &[],
            )
            .expect("cycle 3");
        fixture.insert_checkpoint(run_3, attempt_3, 3, 35);

        fixture
            .database
            .connection()
            .expect("connection")
            .execute(
                "INSERT INTO dynamic_memory_suffix_rewinds (
                    operation_id, request_digest, conversation_id, space_id, invalid_run_id,
                    source_memory_revision, resulting_memory_revision,
                    restored_summary_run_id, resulting_memory_json, applied_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, 4, 5, ?6, '{\"format_version\":1,\"value\":{}}', 40)",
                params![
                    OperationId::new().to_string(),
                    "0".repeat(64),
                    fixture.scope.conversation_id.to_string(),
                    fixture.scope.space_id.to_string(),
                    run_2.to_string(),
                    run_1.to_string(),
                ],
            )
            .expect("rewind");

        let active = fixture
            .database
            .active_summary_units(fixture.scope, Some(run_4))
            .expect("active after rewind");
        assert_eq!(active, vec![cycle_1]);
    }

    #[test]
    fn seed_and_reset_hide_previously_active_units() {
        let fixture = Fixture::new();
        let (run_1, attempt_1) = fixture.insert_run(10, 0, 20);
        let (run_2, attempt_2) = fixture.insert_run(20, 20, 40);
        let (run_3, _) = fixture.insert_run(30, 40, 60);

        let cycle_1 = fixture
            .database
            .insert_summary_unit(
                fixture.unit(run_1, MemorySummaryUnitKind::Cycle, 0, 0, 20, "Cycle 1"),
                &[],
            )
            .expect("cycle 1");
        fixture.insert_checkpoint(run_1, attempt_1, 1, 15);

        let seed = fixture
            .database
            .insert_summary_unit(
                fixture.unit(
                    run_2,
                    MemorySummaryUnitKind::Seed,
                    0,
                    0,
                    20,
                    "Edited summary",
                ),
                &[cycle_1.id],
            )
            .expect("seed");
        fixture.insert_checkpoint(run_2, attempt_2, 2, 25);

        let after_seed = fixture
            .database
            .active_summary_units(fixture.scope, Some(run_3))
            .expect("active after seed");
        assert_eq!(after_seed, vec![seed.clone()]);

        let reset = fixture
            .database
            .insert_summary_unit(
                fixture.unit(run_3, MemorySummaryUnitKind::Reset, 0, 0, 0, ""),
                &[seed.id],
            )
            .expect("reset");

        let after_reset = fixture
            .database
            .active_summary_units(fixture.scope, Some(run_3))
            .expect("active after reset");
        assert_eq!(after_reset, vec![reset]);
    }

    #[test]
    fn duplicate_insert_reuses_identical_unit_and_rejects_conflicting_text() {
        let fixture = Fixture::new();
        let (run_1, _) = fixture.insert_run(10, 0, 20);

        let first = fixture
            .database
            .insert_summary_unit(
                fixture.unit(run_1, MemorySummaryUnitKind::Cycle, 0, 0, 20, "Cycle 1"),
                &[],
            )
            .expect("first insert");

        let replay = fixture
            .database
            .insert_summary_unit(
                fixture.unit(run_1, MemorySummaryUnitKind::Cycle, 0, 0, 20, "Cycle 1"),
                &[],
            )
            .expect("idempotent replay");
        assert_eq!(replay, first);

        let conflict = fixture.database.insert_summary_unit(
            fixture.unit(
                run_1,
                MemorySummaryUnitKind::Cycle,
                0,
                0,
                20,
                "Different text",
            ),
            &[],
        );
        assert_eq!(conflict, Err(MemoryRepositoryError::Conflict));
    }

    #[test]
    fn overlapping_and_gapped_recent_windows_stay_active_in_order() {
        let fixture = Fixture::new();
        let (run_1, attempt_1) = fixture.insert_run(10, 0, 20);
        let (run_2, attempt_2) = fixture.insert_run(20, 15, 35);
        let (run_3, _) = fixture.insert_run(30, 40, 60);

        let cycle_1 = fixture
            .database
            .insert_summary_unit(
                fixture.unit(run_1, MemorySummaryUnitKind::Cycle, 0, 0, 20, "Cycle 1"),
                &[],
            )
            .expect("cycle 1");
        fixture.insert_checkpoint(run_1, attempt_1, 1, 15);

        let overlapping = fixture
            .database
            .insert_summary_unit(
                fixture.unit(run_2, MemorySummaryUnitKind::Cycle, 0, 15, 35, "Cycle 2"),
                &[],
            )
            .expect("overlapping cycle");
        fixture.insert_checkpoint(run_2, attempt_2, 2, 25);

        let gapped = fixture
            .database
            .insert_summary_unit(
                fixture.unit(run_3, MemorySummaryUnitKind::Cycle, 0, 40, 60, "Cycle 3"),
                &[],
            )
            .expect("gapped cycle");

        let active = fixture
            .database
            .active_summary_units(fixture.scope, Some(run_3))
            .expect("overlaps and gaps stay readable");
        assert_eq!(active, vec![cycle_1, overlapping, gapped]);
    }

    #[test]
    fn time_range_round_trips_and_conflicting_retry_range_returns_conflict() {
        let fixture = Fixture::new();
        let (run_1, attempt_1) = fixture.insert_run(10, 0, 20);
        let (run_2, _) = fixture.insert_run(20, 20, 40);

        let mut ranged = fixture.unit(run_1, MemorySummaryUnitKind::Cycle, 0, 0, 20, "Cycle 1");
        ranged.time_start = Some(TimestampMillis::new(1_700_000_000_000));
        ranged.time_end = Some(TimestampMillis::new(1_700_000_600_000));
        let inserted = fixture
            .database
            .insert_summary_unit(ranged.clone(), &[])
            .expect("insert ranged");
        assert_eq!(
            inserted.time_start,
            Some(TimestampMillis::new(1_700_000_000_000))
        );
        assert_eq!(
            inserted.time_end,
            Some(TimestampMillis::new(1_700_000_600_000))
        );

        let fetched = fixture
            .database
            .summary_unit(inserted.id)
            .expect("lookup")
            .expect("present");
        assert_eq!(fetched, inserted);

        let mut retry_same = fixture.unit(run_1, MemorySummaryUnitKind::Cycle, 0, 0, 20, "Cycle 1");
        retry_same.time_start = inserted.time_start;
        retry_same.time_end = inserted.time_end;
        let replayed = fixture
            .database
            .insert_summary_unit(retry_same, &[])
            .expect("replay same range");
        assert_eq!(replayed, inserted);

        let mut retry_diff = fixture.unit(run_1, MemorySummaryUnitKind::Cycle, 0, 0, 20, "Cycle 1");
        retry_diff.time_start = Some(TimestampMillis::new(1_700_000_000_000));
        retry_diff.time_end = Some(TimestampMillis::new(1_700_000_900_000));
        assert_eq!(
            fixture.database.insert_summary_unit(retry_diff, &[]),
            Err(MemoryRepositoryError::Conflict)
        );

        fixture.insert_checkpoint(run_1, attempt_1, 1, 15);

        let unranged = fixture
            .database
            .insert_summary_unit(
                fixture.unit(run_2, MemorySummaryUnitKind::Cycle, 0, 20, 40, "Cycle 2"),
                &[],
            )
            .expect("insert unranged");
        assert_eq!(unranged.time_start, None);
        assert_eq!(unranged.time_end, None);
        assert_eq!(
            fixture.database.summary_unit(unranged.id).expect("lookup"),
            Some(unranged)
        );
    }
}
