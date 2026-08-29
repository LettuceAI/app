use std::collections::HashMap;

use lettuce_memory::{
    DynamicMemoryPreparationPlan, DynamicMemoryPreparationPlanError,
    DynamicMemoryPreparationRepository, DynamicMemoryRecoveredChild, MemoryToolArguments,
};
use lettuce_types::{ConversationId, GenerationAttemptId, GenerationTurnId, Revision};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::{Database, decode_versioned, encode_versioned, tool_adapter};

const PREPARATION_PLAN_VERSION: u32 = 1;

fn storage<T>(_: T) -> DynamicMemoryPreparationPlanError {
    DynamicMemoryPreparationPlanError::Storage
}

fn conflict<T>(_: T) -> DynamicMemoryPreparationPlanError {
    DynamicMemoryPreparationPlanError::Conflict
}

fn encoded(
    plan: &DynamicMemoryPreparationPlan,
) -> Result<(String, String), DynamicMemoryPreparationPlanError> {
    let document = encode_versioned(plan, PREPARATION_PLAN_VERSION).map_err(storage)?;
    let digest = blake3::hash(document.as_bytes()).to_hex().to_string();
    Ok((document, digest))
}

fn verify_durable_identity(
    transaction: &Transaction<'_>,
    plan: &DynamicMemoryPreparationPlan,
    require_running: bool,
) -> Result<(), DynamicMemoryPreparationPlanError> {
    let attempt_job = transaction
        .query_row(
            "SELECT job_id FROM generation_attempts
              WHERE conversation_id = ?1 AND turn_id = ?2 AND id = ?3",
            params![
                plan.conversation_id.to_string(),
                plan.turn_id.to_string(),
                plan.attempt_id.to_string(),
            ],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(storage)?
        .flatten();
    if attempt_job.as_deref() != Some(&plan.job_id.to_string()) {
        return Err(DynamicMemoryPreparationPlanError::Conflict);
    }
    let memory_revision = transaction
        .query_row(
            "SELECT revision FROM memory_spaces WHERE id = ?1",
            [plan.space_id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(storage)?;
    if memory_revision
        .and_then(|value| u64::try_from(value).ok())
        .map(Revision::new)
        != Some(plan.expected_memory_revision)
    {
        return Err(DynamicMemoryPreparationPlanError::Conflict);
    }

    let end_ordinal = usize::from(plan.first_execution_ordinal)
        .checked_add(plan.execution_ids.len())
        .ok_or(DynamicMemoryPreparationPlanError::Conflict)?;
    if end_ordinal > usize::from(u16::MAX) + 1 {
        return Err(DynamicMemoryPreparationPlanError::Conflict);
    }
    let durable = {
        let mut statement = transaction
            .prepare(
                "SELECT id, ordinal FROM tool_executions
                  WHERE conversation_id = ?1 AND turn_id = ?2 AND attempt_id = ?3
                    AND ordinal >= ?4 AND ordinal < ?5
                  ORDER BY ordinal",
            )
            .map_err(storage)?;
        statement
            .query_map(
                params![
                    plan.conversation_id.to_string(),
                    plan.turn_id.to_string(),
                    plan.attempt_id.to_string(),
                    i64::from(plan.first_execution_ordinal),
                    i64::try_from(end_ordinal).map_err(storage)?,
                ],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .map_err(storage)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(storage)?
    };
    if durable.len() != plan.execution_ids.len()
        || durable.iter().zip(&plan.execution_ids).enumerate().any(
            |(offset, ((id, ordinal), expected_id))| {
                id != &expected_id.to_string()
                    || *ordinal
                        != i64::from(plan.first_execution_ordinal)
                            + i64::try_from(offset).unwrap_or(i64::MAX)
            },
        )
    {
        return Err(DynamicMemoryPreparationPlanError::Conflict);
    }

    let creates = plan
        .creates
        .iter()
        .map(|create| (create.execution_id, create))
        .collect::<HashMap<_, _>>();
    for execution_id in &plan.execution_ids {
        let execution = tool_adapter::get_in(transaction, *execution_id).map_err(conflict)?;
        if execution.conversation_id != plan.conversation_id
            || execution.turn_id != plan.turn_id
            || execution.attempt_id != plan.attempt_id
            || execution.definition_version != 1
            || if require_running {
                execution.status != lettuce_conversations::ToolExecutionStatus::Running
            } else {
                !matches!(
                    execution.status,
                    lettuce_conversations::ToolExecutionStatus::Running
                        | lettuce_conversations::ToolExecutionStatus::Interrupted
                )
            }
        {
            return Err(DynamicMemoryPreparationPlanError::Conflict);
        }
        let arguments =
            MemoryToolArguments::parse(&execution.definition_name, &execution.arguments)
                .map_err(conflict)?;
        match arguments {
            MemoryToolArguments::CreateMemory { text, .. } => {
                if creates
                    .get(execution_id)
                    .map(|create| create.source_text.as_str())
                    != Some(text.as_str())
                {
                    return Err(DynamicMemoryPreparationPlanError::Conflict);
                }
            }
            _ if creates.contains_key(execution_id) => {
                return Err(DynamicMemoryPreparationPlanError::Conflict);
            }
            _ => {}
        }
    }
    Ok(())
}

fn hydrate_verified(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    turn_id: GenerationTurnId,
    attempt_id: GenerationAttemptId,
    first_execution_ordinal: Option<u16>,
) -> Result<Option<DynamicMemoryPreparationPlan>, DynamicMemoryPreparationPlanError> {
    let row = transaction
        .query_row(
            "SELECT first_execution_ordinal, job_id, space_id,
                    expected_memory_revision, plan_json, plan_digest
               FROM dynamic_memory_preparation_plans
              WHERE conversation_id = ?1 AND turn_id = ?2 AND attempt_id = ?3
                AND (?4 IS NULL OR first_execution_ordinal = ?4)
              ORDER BY first_execution_ordinal DESC
              LIMIT 1",
            params![
                conversation_id.to_string(),
                turn_id.to_string(),
                attempt_id.to_string(),
                first_execution_ordinal.map(i64::from),
            ],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .optional()
        .map_err(storage)?;
    let Some((first_ordinal, job_id, space_id, expected_revision, document, digest)) = row else {
        return Ok(None);
    };
    if blake3::hash(document.as_bytes()).to_hex().as_str() != digest {
        return Err(DynamicMemoryPreparationPlanError::Storage);
    }
    let plan =
        decode_versioned::<DynamicMemoryPreparationPlan>(&document, PREPARATION_PLAN_VERSION)
            .map_err(storage)?;
    plan.validate()?;
    if plan.conversation_id != conversation_id
        || plan.turn_id != turn_id
        || plan.attempt_id != attempt_id
        || i64::from(plan.first_execution_ordinal) != first_ordinal
        || plan.job_id.to_string() != job_id
        || plan.space_id.to_string() != space_id
        || i64::try_from(plan.expected_memory_revision.get()).ok() != Some(expected_revision)
    {
        return Err(DynamicMemoryPreparationPlanError::Storage);
    }
    verify_durable_identity(transaction, &plan, false)?;
    Ok(Some(plan))
}

fn insert_plan_in(
    transaction: &Transaction<'_>,
    plan: &DynamicMemoryPreparationPlan,
) -> Result<(), DynamicMemoryPreparationPlanError> {
    let (document, digest) = encoded(plan)?;
    transaction
        .execute(
            "INSERT INTO dynamic_memory_preparation_plans (
                conversation_id, turn_id, attempt_id, first_execution_ordinal, job_id, space_id,
                expected_memory_revision, plan_json, plan_digest
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                plan.conversation_id.to_string(),
                plan.turn_id.to_string(),
                plan.attempt_id.to_string(),
                i64::from(plan.first_execution_ordinal),
                plan.job_id.to_string(),
                plan.space_id.to_string(),
                i64::try_from(plan.expected_memory_revision.get()).map_err(storage)?,
                document,
                digest,
            ],
        )
        .map_err(storage)?;
    Ok(())
}

fn executions_in(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    turn_id: GenerationTurnId,
    attempt_id: GenerationAttemptId,
) -> Result<Vec<lettuce_conversations::ToolExecution>, DynamicMemoryPreparationPlanError> {
    let ids = {
        let mut statement = transaction
            .prepare(
                "SELECT id FROM tool_executions
                  WHERE conversation_id = ?1 AND turn_id = ?2 AND attempt_id = ?3
                  ORDER BY ordinal",
            )
            .map_err(storage)?;
        statement
            .query_map(
                params![
                    conversation_id.to_string(),
                    turn_id.to_string(),
                    attempt_id.to_string(),
                ],
                |row| row.get::<_, String>(0),
            )
            .map_err(storage)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(storage)?
    };
    ids.into_iter()
        .map(|id| {
            let id = id.parse().map_err(storage)?;
            tool_adapter::get_in(transaction, id).map_err(conflict)
        })
        .collect()
}

fn remap_plan(
    parent: &DynamicMemoryPreparationPlan,
    child_attempt_id: GenerationAttemptId,
    child_job_id: lettuce_types::JobId,
    child_ids: &[lettuce_types::ToolExecutionId],
) -> Result<DynamicMemoryPreparationPlan, DynamicMemoryPreparationPlanError> {
    if child_ids.len() != parent.execution_ids.len() {
        return Err(DynamicMemoryPreparationPlanError::Conflict);
    }
    let remap = parent
        .execution_ids
        .iter()
        .copied()
        .zip(child_ids.iter().copied())
        .collect::<HashMap<_, _>>();
    let mut child = parent.clone();
    child.attempt_id = child_attempt_id;
    child.job_id = child_job_id;
    child.first_execution_ordinal = 0;
    child.execution_ids = child_ids.to_vec();
    for create in &mut child.creates {
        create.execution_id = remap
            .get(&create.execution_id)
            .copied()
            .ok_or(DynamicMemoryPreparationPlanError::Conflict)?;
    }
    child.validate()?;
    Ok(child)
}

impl DynamicMemoryPreparationRepository for Database {
    fn put_preparation_plan(
        &self,
        plan: DynamicMemoryPreparationPlan,
    ) -> Result<DynamicMemoryPreparationPlan, DynamicMemoryPreparationPlanError> {
        plan.validate()?;
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let exists = transaction
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM dynamic_memory_preparation_plans
                     WHERE conversation_id = ?1 AND turn_id = ?2 AND attempt_id = ?3
                       AND first_execution_ordinal = ?4
                 )",
                params![
                    plan.conversation_id.to_string(),
                    plan.turn_id.to_string(),
                    plan.attempt_id.to_string(),
                    i64::from(plan.first_execution_ordinal),
                ],
                |row| row.get::<_, bool>(0),
            )
            .map_err(storage)?;
        if exists {
            let stored = hydrate_verified(
                &transaction,
                plan.conversation_id,
                plan.turn_id,
                plan.attempt_id,
                Some(plan.first_execution_ordinal),
            )?
            .ok_or(DynamicMemoryPreparationPlanError::Storage)?;
            if stored == plan {
                transaction.commit().map_err(storage)?;
                return Ok(stored);
            }
            return Err(DynamicMemoryPreparationPlanError::Conflict);
        }
        verify_durable_identity(&transaction, &plan, true)?;
        insert_plan_in(&transaction, &plan)?;
        transaction.commit().map_err(storage)?;
        Ok(plan)
    }

    fn get_preparation_plan(
        &self,
        conversation_id: ConversationId,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
    ) -> Result<Option<DynamicMemoryPreparationPlan>, DynamicMemoryPreparationPlanError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(storage)?;
        let plan = hydrate_verified(&transaction, conversation_id, turn_id, attempt_id, None)?;
        transaction.commit().map_err(storage)?;
        Ok(plan)
    }

    fn recover_preparation_into_child(
        &self,
        conversation_id: ConversationId,
        turn_id: GenerationTurnId,
        parent_attempt_id: GenerationAttemptId,
        child_attempt_id: GenerationAttemptId,
        child_job_id: lettuce_types::JobId,
        at: lettuce_types::TimestampMillis,
    ) -> Result<DynamicMemoryRecoveredChild, DynamicMemoryPreparationPlanError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let parent = hydrate_verified(
            &transaction,
            conversation_id,
            turn_id,
            parent_attempt_id,
            None,
        )?
        .ok_or(DynamicMemoryPreparationPlanError::Conflict)?;
        let child_identity = transaction
            .query_row(
                "SELECT parent_attempt_id, status, job_id
                   FROM generation_attempts
                  WHERE conversation_id = ?1 AND turn_id = ?2 AND id = ?3",
                params![
                    conversation_id.to_string(),
                    turn_id.to_string(),
                    child_attempt_id.to_string(),
                ],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(storage)?;
        if child_identity
            != Some((
                Some(parent_attempt_id.to_string()),
                "running".to_owned(),
                Some(child_job_id.to_string()),
            ))
        {
            return Err(DynamicMemoryPreparationPlanError::Conflict);
        }
        let parent_executions =
            executions_in(&transaction, conversation_id, turn_id, parent_attempt_id)?
                .into_iter()
                .filter(|execution| parent.execution_ids.contains(&execution.id))
                .collect::<Vec<_>>();
        if parent_executions
            .iter()
            .map(|execution| execution.id)
            .collect::<Vec<_>>()
            != parent.execution_ids
        {
            return Err(DynamicMemoryPreparationPlanError::Conflict);
        }
        if parent_executions.iter().any(|execution| {
            execution.status != lettuce_conversations::ToolExecutionStatus::Interrupted
        }) {
            return Err(DynamicMemoryPreparationPlanError::Conflict);
        }
        let existing_child =
            executions_in(&transaction, conversation_id, turn_id, child_attempt_id)?;
        if !existing_child.is_empty() {
            let stored = hydrate_verified(
                &transaction,
                conversation_id,
                turn_id,
                child_attempt_id,
                Some(0),
            )?
            .ok_or(DynamicMemoryPreparationPlanError::Conflict)?;
            let expected = remap_plan(
                &parent,
                child_attempt_id,
                child_job_id,
                &existing_child
                    .iter()
                    .map(|execution| execution.id)
                    .collect::<Vec<_>>(),
            )?;
            if stored != expected {
                return Err(DynamicMemoryPreparationPlanError::Conflict);
            }
            transaction.commit().map_err(storage)?;
            return Ok(DynamicMemoryRecoveredChild {
                plan: stored,
                executions: existing_child,
            });
        }

        let mut requested = Vec::with_capacity(parent_executions.len());
        for (ordinal, parent_execution) in parent_executions.iter().enumerate() {
            let execution = lettuce_conversations::ToolExecution {
                id: lettuce_types::ToolExecutionId::new(),
                conversation_id,
                turn_id,
                attempt_id: child_attempt_id,
                ordinal: u16::try_from(ordinal).map_err(storage)?,
                definition_name: parent_execution.definition_name.clone(),
                definition_version: parent_execution.definition_version,
                provider_call_id: parent_execution.provider_call_id.clone(),
                arguments: parent_execution.arguments.clone(),
                raw_arguments: parent_execution.raw_arguments.clone(),
                provider_replay: parent_execution.provider_replay.clone(),
                status: lettuce_conversations::ToolExecutionStatus::Requested,
                output: None,
                failure: None,
                revision: Revision::INITIAL,
                requested_at: at,
                started_at: None,
                finished_at: None,
                updated_at: at,
            };
            requested.push(tool_adapter::insert_in(&transaction, &execution).map_err(conflict)?);
        }
        let mut running = Vec::with_capacity(requested.len());
        for execution in requested {
            let validated = tool_adapter::transition_in(
                &transaction,
                &lettuce_conversations::ToolExecutionTransition {
                    id: execution.id,
                    expected_revision: execution.revision,
                    next: lettuce_conversations::ToolExecutionStatus::Validated,
                    output: None,
                    failure: None,
                },
                at,
            )
            .map_err(conflict)?;
            running.push(
                tool_adapter::transition_in(
                    &transaction,
                    &lettuce_conversations::ToolExecutionTransition {
                        id: validated.id,
                        expected_revision: validated.revision,
                        next: lettuce_conversations::ToolExecutionStatus::Running,
                        output: None,
                        failure: None,
                    },
                    at,
                )
                .map_err(conflict)?,
            );
        }
        let plan = remap_plan(
            &parent,
            child_attempt_id,
            child_job_id,
            &running
                .iter()
                .map(|execution| execution.id)
                .collect::<Vec<_>>(),
        )?;
        verify_durable_identity(&transaction, &plan, true)?;
        insert_plan_in(&transaction, &plan)?;
        transaction.commit().map_err(storage)?;
        Ok(DynamicMemoryRecoveredChild {
            plan,
            executions: running,
        })
    }
}

#[cfg(test)]
mod tests {
    use lettuce_conversations::{
        ProposedToolCall, ToolExecution, ToolExecutionOwner, ToolExecutionRepository,
        ToolExecutionStatus, ToolExecutionTransition,
    };
    use lettuce_memory::{
        CreateMemoryPreparation, DynamicMemoryPreparationPlan, DynamicMemoryPreparationPlanError,
        DynamicMemoryPreparationRepository, MemoryPolicy, MemoryRepository, MemorySpaceSnapshot,
        PersistedMemoryCreatePreparation, Score, dynamic_memory_tool_request,
    };
    use lettuce_types::{
        ConversationId, GenerationAttemptId, GenerationTurnId, JobId, MemoryId, MemorySpaceId,
        Revision, TimestampMillis, ToolExecutionId,
    };
    use serde_json::json;

    use crate::Database;

    fn score(value: u16) -> Score {
        Score::from_basis_points(value).expect("valid score")
    }

    fn policy() -> MemoryPolicy {
        MemoryPolicy {
            max_entries: 64,
            hot_token_budget: 2_048,
            cold_threshold: score(2_000),
            delete_confidence_default: score(5_000),
            max_hard_delete_ratio_per_cycle: score(5_000),
        }
    }

    struct Fixture {
        database: Database,
        plan: DynamicMemoryPreparationPlan,
    }

    fn fixture() -> Fixture {
        let database = Database::open_in_memory().expect("database");
        database
            .connection()
            .expect("connection")
            .execute_batch("PRAGMA foreign_keys = OFF")
            .expect("fixture mode");
        let conversation_id = ConversationId::new();
        let turn_id = GenerationTurnId::new();
        let attempt_id = GenerationAttemptId::new();
        let job_id = JobId::new();
        database
            .connection()
            .expect("connection")
            .execute(
                "INSERT INTO generation_attempts (
                    conversation_id, turn_id, id, ordinal, parent_attempt_id,
                    status, job_idempotency_key, job_id, started_at, finished_at,
                    usage_event_id, usage_outcome, failure
                 ) VALUES (?1, ?2, ?3, 0, NULL, 'running', ?4, ?5, 1, NULL,
                           NULL, NULL, NULL)",
                rusqlite::params![
                    conversation_id.to_string(),
                    turn_id.to_string(),
                    attempt_id.to_string(),
                    format!("generation.{turn_id}.{attempt_id}"),
                    job_id.to_string(),
                ],
            )
            .expect("attempt");
        let space_id = MemorySpaceId::new();
        database
            .create(MemorySpaceSnapshot {
                id: space_id,
                revision: Revision::INITIAL,
                items: vec![],
            })
            .expect("memory space");
        let owner = ToolExecutionOwner {
            conversation_id,
            turn_id,
            attempt_id,
        };
        let definitions = dynamic_memory_tool_request().definitions;
        let calls = [
            (
                "create_memory",
                json!({"text": "Mira prefers tea", "category": "preference"}),
            ),
            ("done", json!({"summary": "updated"})),
        ];
        let requested = calls
            .iter()
            .enumerate()
            .map(|(ordinal, (name, arguments))| {
                let definition = definitions
                    .iter()
                    .find(|definition| definition.name == *name)
                    .expect("definition");
                ToolExecution::requested(
                    ToolExecutionId::new(),
                    owner,
                    u16::try_from(ordinal).expect("ordinal"),
                    definition,
                    ProposedToolCall {
                        provider_call_id: Some(format!("call-{ordinal}")),
                        name: (*name).to_owned(),
                        arguments: arguments.clone(),
                        raw_arguments: None,
                        provider_replay: None,
                    },
                    TimestampMillis::new(1),
                )
                .expect("execution")
            })
            .collect::<Vec<_>>();
        let requested = database
            .append_tool_executions(0, &requested)
            .expect("persist executions");
        let validated = database
            .transition_tool_execution_batch(
                &requested
                    .iter()
                    .map(|execution| ToolExecutionTransition {
                        id: execution.id,
                        expected_revision: execution.revision,
                        next: ToolExecutionStatus::Validated,
                        output: None,
                        failure: None,
                    })
                    .collect::<Vec<_>>(),
                TimestampMillis::new(2),
            )
            .expect("validate");
        let running = database
            .transition_tool_execution_batch(
                &validated
                    .iter()
                    .map(|execution| ToolExecutionTransition {
                        id: execution.id,
                        expected_revision: execution.revision,
                        next: ToolExecutionStatus::Running,
                        output: None,
                        failure: None,
                    })
                    .collect::<Vec<_>>(),
                TimestampMillis::new(3),
            )
            .expect("start");
        let memory_id = MemoryId::new();
        let plan = DynamicMemoryPreparationPlan {
            conversation_id,
            turn_id,
            attempt_id,
            job_id,
            space_id,
            expected_memory_revision: Revision::INITIAL,
            first_execution_ordinal: running[0].ordinal,
            policy: policy(),
            duplicate_threshold: score(9_000),
            execution_ids: running.iter().map(|execution| execution.id).collect(),
            creates: vec![PersistedMemoryCreatePreparation {
                execution_id: running[0].id,
                source_text: "Mira prefers tea".to_owned(),
                preparation: CreateMemoryPreparation {
                    id: memory_id,
                    token_count: 4,
                    created_at: TimestampMillis::new(4),
                    semantic_duplicate: None,
                },
                embedding_source_revision: "v4-test".to_owned(),
                embedding_dimensions: 128,
            }],
        };
        Fixture { database, plan }
    }

    #[test]
    fn preparation_plan_insert_is_immutable_and_idempotent() {
        let fixture = fixture();
        let first = fixture
            .database
            .put_preparation_plan(fixture.plan.clone())
            .expect("first insert");
        let retry = fixture
            .database
            .put_preparation_plan(fixture.plan.clone())
            .expect("exact retry");
        assert_eq!(retry, first);
        assert_eq!(
            fixture
                .database
                .get_preparation_plan(
                    fixture.plan.conversation_id,
                    fixture.plan.turn_id,
                    fixture.plan.attempt_id,
                )
                .expect("read"),
            Some(first)
        );
        let running = fixture
            .database
            .list_tool_executions(
                fixture.plan.conversation_id,
                fixture.plan.turn_id,
                fixture.plan.attempt_id,
            )
            .expect("running executions");
        fixture
            .database
            .transition_tool_execution_batch(
                &running
                    .iter()
                    .map(|execution| ToolExecutionTransition {
                        id: execution.id,
                        expected_revision: execution.revision,
                        next: ToolExecutionStatus::Interrupted,
                        output: None,
                        failure: None,
                    })
                    .collect::<Vec<_>>(),
                TimestampMillis::new(5),
            )
            .expect("interrupt round");
        assert_eq!(
            fixture.database.get_preparation_plan(
                fixture.plan.conversation_id,
                fixture.plan.turn_id,
                fixture.plan.attempt_id,
            ),
            Ok(Some(fixture.plan.clone()))
        );

        let mut changed = fixture.plan;
        changed.policy.hot_token_budget += 1;
        assert_eq!(
            fixture.database.put_preparation_plan(changed),
            Err(DynamicMemoryPreparationPlanError::Conflict)
        );
    }

    #[test]
    fn preparation_plan_rejects_mutable_or_mismatched_inputs() {
        let fixture = fixture();
        let mut wrong_source = fixture.plan.clone();
        wrong_source.creates[0].source_text = "different".to_owned();
        assert_eq!(
            fixture.database.put_preparation_plan(wrong_source),
            Err(DynamicMemoryPreparationPlanError::Conflict)
        );

        fixture
            .database
            .put_preparation_plan(fixture.plan.clone())
            .expect("plan");
        fixture
            .database
            .connection()
            .expect("connection")
            .execute(
                "UPDATE memory_spaces SET revision = 2 WHERE id = ?1",
                [fixture.plan.space_id.to_string()],
            )
            .expect("advance memory");
        assert_eq!(
            fixture.database.get_preparation_plan(
                fixture.plan.conversation_id,
                fixture.plan.turn_id,
                fixture.plan.attempt_id,
            ),
            Err(DynamicMemoryPreparationPlanError::Conflict)
        );
    }

    #[test]
    fn preparation_plans_are_immutable_per_round_within_one_attempt() {
        let fixture = fixture();
        fixture
            .database
            .put_preparation_plan(fixture.plan.clone())
            .expect("first plan");
        let first_round = fixture
            .database
            .list_tool_executions(
                fixture.plan.conversation_id,
                fixture.plan.turn_id,
                fixture.plan.attempt_id,
            )
            .expect("first round");
        fixture
            .database
            .transition_tool_execution_batch(
                &first_round
                    .iter()
                    .map(|execution| ToolExecutionTransition {
                        id: execution.id,
                        expected_revision: execution.revision,
                        next: ToolExecutionStatus::Succeeded,
                        output: Some(lettuce_conversations::ToolOutput {
                            value: json!({"status": "settled"}),
                            is_error: false,
                        }),
                        failure: None,
                    })
                    .collect::<Vec<_>>(),
                TimestampMillis::new(5),
            )
            .expect("settle first round");

        let definition = dynamic_memory_tool_request()
            .definitions
            .into_iter()
            .find(|definition| definition.name == "done")
            .expect("done definition");
        let owner = ToolExecutionOwner {
            conversation_id: fixture.plan.conversation_id,
            turn_id: fixture.plan.turn_id,
            attempt_id: fixture.plan.attempt_id,
        };
        let requested = ToolExecution::requested(
            ToolExecutionId::new(),
            owner,
            2,
            &definition,
            ProposedToolCall {
                provider_call_id: Some("call-2".to_owned()),
                name: "done".to_owned(),
                arguments: json!({"summary": "finished"}),
                raw_arguments: None,
                provider_replay: None,
            },
            TimestampMillis::new(6),
        )
        .expect("second call");
        let requested = fixture
            .database
            .append_tool_executions(2, &[requested])
            .expect("append second round");
        let validated = fixture
            .database
            .transition_tool_execution_batch(
                &[ToolExecutionTransition {
                    id: requested[0].id,
                    expected_revision: requested[0].revision,
                    next: ToolExecutionStatus::Validated,
                    output: None,
                    failure: None,
                }],
                TimestampMillis::new(7),
            )
            .expect("validate second round");
        let running = fixture
            .database
            .transition_tool_execution_batch(
                &[ToolExecutionTransition {
                    id: validated[0].id,
                    expected_revision: validated[0].revision,
                    next: ToolExecutionStatus::Running,
                    output: None,
                    failure: None,
                }],
                TimestampMillis::new(8),
            )
            .expect("start second round");
        let mut second = fixture.plan.clone();
        second.first_execution_ordinal = 2;
        second.execution_ids = vec![running[0].id];
        second.creates.clear();
        fixture
            .database
            .put_preparation_plan(second.clone())
            .expect("second plan");

        assert_eq!(
            fixture
                .database
                .get_preparation_plan(second.conversation_id, second.turn_id, second.attempt_id,)
                .expect("latest plan"),
            Some(second)
        );
        let count = fixture
            .database
            .connection()
            .expect("connection")
            .query_row(
                "SELECT COUNT(*) FROM dynamic_memory_preparation_plans WHERE attempt_id = ?1",
                [fixture.plan.attempt_id.to_string()],
                |row| row.get::<_, i64>(0),
            )
            .expect("plan count");
        assert_eq!(count, 2);
    }

    #[test]
    fn interrupted_plan_remaps_atomically_into_immediate_running_child() {
        let fixture = fixture();
        fixture
            .database
            .put_preparation_plan(fixture.plan.clone())
            .expect("parent plan");
        let parent_executions = fixture
            .database
            .list_tool_executions(
                fixture.plan.conversation_id,
                fixture.plan.turn_id,
                fixture.plan.attempt_id,
            )
            .expect("parent executions");
        fixture
            .database
            .transition_tool_execution_batch(
                &parent_executions
                    .iter()
                    .map(|execution| ToolExecutionTransition {
                        id: execution.id,
                        expected_revision: execution.revision,
                        next: ToolExecutionStatus::Interrupted,
                        output: None,
                        failure: None,
                    })
                    .collect::<Vec<_>>(),
                TimestampMillis::new(5),
            )
            .expect("interrupt tools");
        let child_attempt_id = GenerationAttemptId::new();
        let child_job_id = JobId::new();
        {
            let connection = fixture.database.connection().expect("connection");
            connection
                .execute(
                    "UPDATE generation_attempts
                        SET status = 'interrupted', finished_at = 5,
                            usage_event_id = ?2, usage_outcome = 'interrupted'
                      WHERE id = ?1",
                    rusqlite::params![
                        fixture.plan.attempt_id.to_string(),
                        lettuce_types::UsageEventId::new().to_string(),
                    ],
                )
                .expect("interrupt parent attempt");
            connection
                .execute(
                    "INSERT INTO generation_attempts (
                        conversation_id, turn_id, id, ordinal, parent_attempt_id,
                        status, job_idempotency_key, job_id, started_at, finished_at,
                        usage_event_id, usage_outcome, failure
                     ) VALUES (?1, ?2, ?3, 1, ?4, 'running', ?5, ?6, 6, NULL,
                               NULL, NULL, NULL)",
                    rusqlite::params![
                        fixture.plan.conversation_id.to_string(),
                        fixture.plan.turn_id.to_string(),
                        child_attempt_id.to_string(),
                        fixture.plan.attempt_id.to_string(),
                        format!("generation.{}.{child_attempt_id}", fixture.plan.turn_id),
                        child_job_id.to_string(),
                    ],
                )
                .expect("running child");
        }

        let recovered = fixture
            .database
            .recover_preparation_into_child(
                fixture.plan.conversation_id,
                fixture.plan.turn_id,
                fixture.plan.attempt_id,
                child_attempt_id,
                child_job_id,
                TimestampMillis::new(7),
            )
            .expect("recover child");
        assert_eq!(recovered.plan.attempt_id, child_attempt_id);
        assert_eq!(recovered.plan.job_id, child_job_id);
        assert_eq!(recovered.executions.len(), parent_executions.len());
        assert!(
            recovered
                .executions
                .iter()
                .all(|execution| execution.status == ToolExecutionStatus::Running)
        );
        assert_eq!(
            recovered
                .executions
                .iter()
                .map(|execution| &execution.arguments)
                .collect::<Vec<_>>(),
            parent_executions
                .iter()
                .map(|execution| &execution.arguments)
                .collect::<Vec<_>>()
        );
        assert!(
            recovered
                .executions
                .iter()
                .zip(&parent_executions)
                .all(|(child, parent)| child.id != parent.id)
        );
        assert_eq!(
            recovered.plan.creates[0].execution_id,
            recovered.executions[0].id
        );
        let retry = fixture
            .database
            .recover_preparation_into_child(
                fixture.plan.conversation_id,
                fixture.plan.turn_id,
                fixture.plan.attempt_id,
                child_attempt_id,
                child_job_id,
                TimestampMillis::new(70),
            )
            .expect("exact retry");
        assert_eq!(retry, recovered);
        assert_eq!(
            fixture.database.recover_preparation_into_child(
                fixture.plan.conversation_id,
                fixture.plan.turn_id,
                fixture.plan.attempt_id,
                child_attempt_id,
                JobId::new(),
                TimestampMillis::new(8),
            ),
            Err(DynamicMemoryPreparationPlanError::Conflict)
        );
    }
}
