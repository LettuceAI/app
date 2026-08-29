use std::collections::{HashMap, HashSet};

use lettuce_conversations::{
    GenerationAttempt, ToolExecution, ToolExecutionRepository, ToolExecutionStatus,
    ToolExecutionTransition, ToolFailure, ToolOutput,
};
use lettuce_embeddings::{
    EmbeddingDimensions, EmbeddingProjectionError, EmbeddingRequest, MemoryEmbeddingProjection,
    MemoryEmbeddingRepair, MemoryEmbeddingRepository,
};
use lettuce_jobs::{Claim, ResourceClass, handle::JobHandle};
use lettuce_memory::{
    CreateMemoryPreparation, DynamicMemoryRoundCommit, DynamicMemoryRoundCommitError,
    DynamicMemoryRoundRepository, MemoryBatchResult, MemoryPolicy, MemoryRepository,
    MemoryRepositoryError, MemorySpaceSnapshot, MemoryToolArguments, MemoryToolCall,
    MemoryToolError, MemoryToolOutcome, MemoryToolReducer,
};
use lettuce_types::{ConversationId, MemoryId, MemorySpaceId, TimestampMillis, ToolExecutionId};

use crate::{EmbeddingGenerationError, EmbeddingService, MemoryEmbeddingEngine};

const TOOL_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedMemoryCreate {
    pub execution_id: ToolExecutionId,
    pub preparation: CreateMemoryPreparation,
    pub projection: Option<PreparedMemoryProjection>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PreparedMemoryProjection {
    Ready(MemoryEmbeddingProjection),
    RepairNeeded(MemoryEmbeddingRepair),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryCreateSeed {
    pub execution_id: ToolExecutionId,
    pub id: MemoryId,
    pub token_count: u32,
    pub created_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicMemoryRoundResult {
    pub snapshot: MemorySpaceSnapshot,
    pub reduction: MemoryBatchResult,
    pub outputs: Vec<(ToolExecutionId, ToolOutput)>,
    pub projection_repairs_pending: Vec<MemoryId>,
    pub settled_executions: Vec<ToolExecution>,
}

#[derive(Debug)]
pub struct DynamicMemoryHandler<'a, R: MemoryRepository + MemoryEmbeddingRepository + ?Sized> {
    repository: &'a R,
}

impl<'a, R: MemoryRepository + MemoryEmbeddingRepository + ?Sized> DynamicMemoryHandler<'a, R> {
    #[must_use]
    pub const fn new(repository: &'a R) -> Self {
        Self { repository }
    }

    #[cfg(test)]
    fn apply_admitted_round(
        &self,
        space_id: MemorySpaceId,
        policy: &MemoryPolicy,
        executions: &[ToolExecution],
        prepared_creates: &[PreparedMemoryCreate],
    ) -> Result<DynamicMemoryRoundResult, DynamicMemoryHandlerError> {
        let snapshot = self
            .repository
            .get(space_id)?
            .ok_or(MemoryRepositoryError::NotFound)?;
        let calls = prepare_calls(space_id, executions, prepared_creates)?;
        let reduction = MemoryToolReducer.reduce(&snapshot, policy, &calls)?;
        let outputs = reduction
            .results
            .iter()
            .map(|result| {
                let is_error = matches!(result.outcome, MemoryToolOutcome::Rejected { .. });
                serde_json::to_value(&result.outcome)
                    .map(|value| (result.execution_id, ToolOutput { value, is_error }))
                    .map_err(|_| DynamicMemoryHandlerError::OutputSerialization)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let stored = match &reduction.change {
            Some(change) => self.repository.compare_and_apply(change.clone())?,
            None => snapshot,
        };
        let projection_repairs_pending =
            persist_created_projections(self.repository, &stored, &reduction, prepared_creates);
        Ok(DynamicMemoryRoundResult {
            snapshot: stored,
            reduction,
            outputs,
            projection_repairs_pending,
            settled_executions: Vec::new(),
        })
    }
}

impl<
    'a,
    R: DynamicMemoryRoundRepository + MemoryEmbeddingRepository + ToolExecutionRepository + ?Sized,
> DynamicMemoryHandler<'a, R>
{
    pub fn start_validated_round(
        &self,
        executions: &[ToolExecution],
        at: TimestampMillis,
    ) -> Result<Vec<ToolExecution>, DynamicMemoryCoordinatorError> {
        validate_round(executions, ToolExecutionStatus::Validated)?;
        self.repository
            .transition_tool_execution_batch(
                &executions
                    .iter()
                    .map(|execution| ToolExecutionTransition {
                        id: execution.id,
                        expected_revision: execution.revision,
                        next: ToolExecutionStatus::Running,
                        output: None,
                        failure: None,
                    })
                    .collect::<Vec<_>>(),
                at,
            )
            .map_err(Into::into)
    }

    pub fn settle_running_round(
        &self,
        space_id: MemorySpaceId,
        policy: &MemoryPolicy,
        executions: &[ToolExecution],
        prepared_creates: &[PreparedMemoryCreate],
        at: TimestampMillis,
    ) -> Result<DynamicMemoryRoundResult, DynamicMemoryCoordinatorError> {
        validate_round(executions, ToolExecutionStatus::Running)?;
        let snapshot = self
            .repository
            .get(space_id)?
            .ok_or(MemoryRepositoryError::NotFound)?;
        let calls = prepare_calls(space_id, executions, prepared_creates)?;
        let reduction = MemoryToolReducer.reduce(&snapshot, policy, &calls)?;
        let outputs = reduction
            .results
            .iter()
            .map(|result| {
                let is_error = matches!(result.outcome, MemoryToolOutcome::Rejected { .. });
                serde_json::to_value(&result.outcome)
                    .map(|value| (result.execution_id, ToolOutput { value, is_error }))
                    .map_err(|_| DynamicMemoryHandlerError::OutputSerialization)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let output_by_id = outputs.iter().cloned().collect::<HashMap<_, _>>();
        let execution_transitions = executions
            .iter()
            .map(|execution| {
                let output = output_by_id
                    .get(&execution.id)
                    .cloned()
                    .ok_or(DynamicMemoryCoordinatorError::OutputMismatch)?;
                Ok(ToolExecutionTransition {
                    id: execution.id,
                    expected_revision: execution.revision,
                    next: ToolExecutionStatus::Succeeded,
                    output: Some(output),
                    failure: None,
                })
            })
            .collect::<Result<Vec<_>, DynamicMemoryCoordinatorError>>()?;
        let committed = self.repository.commit_dynamic_memory_round(
            DynamicMemoryRoundCommit {
                space_id,
                change: reduction.change.clone(),
                execution_transitions,
            },
            at,
        )?;
        let projection_repairs_pending = persist_created_projections(
            self.repository,
            &committed.snapshot,
            &reduction,
            prepared_creates,
        );
        Ok(DynamicMemoryRoundResult {
            snapshot: committed.snapshot,
            reduction,
            outputs,
            projection_repairs_pending,
            settled_executions: committed.executions,
        })
    }

    pub fn fail_running_round(
        &self,
        space_id: MemorySpaceId,
        executions: &[ToolExecution],
        failure: ToolFailure,
        at: TimestampMillis,
    ) -> Result<Vec<ToolExecution>, DynamicMemoryCoordinatorError> {
        validate_round(executions, ToolExecutionStatus::Running)?;
        self.repository
            .commit_dynamic_memory_round(
                DynamicMemoryRoundCommit {
                    space_id,
                    change: None,
                    execution_transitions: executions
                        .iter()
                        .map(|execution| ToolExecutionTransition {
                            id: execution.id,
                            expected_revision: execution.revision,
                            next: ToolExecutionStatus::Failed,
                            output: None,
                            failure: Some(failure.clone()),
                        })
                        .collect(),
                },
                at,
            )
            .map(|committed| committed.executions)
            .map_err(Into::into)
    }

    pub fn recover_attempt_round(
        &self,
        conversation_id: ConversationId,
        attempt: &GenerationAttempt,
        handle: &JobHandle,
    ) -> Result<DynamicMemoryRecovery, DynamicMemoryCoordinatorError> {
        if attempt.job_id != Some(handle.id()) {
            return Err(DynamicMemoryCoordinatorError::InvalidJobOwnership);
        }
        let executions =
            self.repository
                .list_tool_executions(conversation_id, attempt.turn_id, attempt.id)?;
        classify_recovery(executions)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DynamicMemoryRecovery {
    TerminalReplay { executions: Vec<ToolExecution> },
    ValidatedStart { executions: Vec<ToolExecution> },
    RestartBlocked { executions: Vec<ToolExecution> },
}

fn classify_recovery(
    executions: Vec<ToolExecution>,
) -> Result<DynamicMemoryRecovery, DynamicMemoryCoordinatorError> {
    if executions.is_empty() {
        return Err(DynamicMemoryCoordinatorError::InvalidRound);
    }
    if executions
        .iter()
        .all(|execution| execution.status.is_terminal())
    {
        return Ok(DynamicMemoryRecovery::TerminalReplay { executions });
    }
    if executions
        .iter()
        .all(|execution| execution.status == ToolExecutionStatus::Validated)
    {
        validate_round(&executions, ToolExecutionStatus::Validated)?;
        return Ok(DynamicMemoryRecovery::ValidatedStart { executions });
    }
    if executions.iter().all(|execution| {
        matches!(
            execution.status,
            ToolExecutionStatus::Running | ToolExecutionStatus::Interrupted
        )
    }) {
        return Ok(DynamicMemoryRecovery::RestartBlocked { executions });
    }
    Err(DynamicMemoryCoordinatorError::InvalidRound)
}

fn validate_round(
    executions: &[ToolExecution],
    expected_status: ToolExecutionStatus,
) -> Result<(), DynamicMemoryCoordinatorError> {
    if executions.is_empty() {
        return Err(DynamicMemoryCoordinatorError::InvalidRound);
    }
    let owner = (
        executions[0].conversation_id,
        executions[0].turn_id,
        executions[0].attempt_id,
    );
    let mut previous = None;
    let mut ids = HashSet::with_capacity(executions.len());
    for execution in executions {
        execution
            .validate()
            .map_err(|_| DynamicMemoryCoordinatorError::InvalidRound)?;
        if execution.status != expected_status
            || owner
                != (
                    execution.conversation_id,
                    execution.turn_id,
                    execution.attempt_id,
                )
            || previous.is_some_and(|ordinal| execution.ordinal != ordinal + 1)
            || !ids.insert(execution.id)
        {
            return Err(DynamicMemoryCoordinatorError::InvalidRound);
        }
        previous = Some(execution.ordinal);
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum DynamicMemoryCoordinatorError {
    #[error("dynamic-memory execution round is invalid")]
    InvalidRound,
    #[error("dynamic-memory result does not cover every execution")]
    OutputMismatch,
    #[error("dynamic-memory recovery job does not own the generation attempt")]
    InvalidJobOwnership,
    #[error("dynamic-memory handler failed: {0}")]
    Handler(#[from] DynamicMemoryHandlerError),
    #[error("conversation repository failed: {0}")]
    Conversation(#[from] lettuce_conversations::ConversationRepositoryError),
    #[error("dynamic-memory round commit failed: {0}")]
    Commit(#[from] DynamicMemoryRoundCommitError),
    #[error("dynamic-memory repository failed: {0}")]
    Repository(#[from] MemoryRepositoryError),
    #[error("dynamic-memory tool call is invalid: {0}")]
    Tool(#[from] MemoryToolError),
}

fn persist_created_projections<R: MemoryEmbeddingRepository + ?Sized>(
    repository: &R,
    stored: &MemorySpaceSnapshot,
    reduction: &MemoryBatchResult,
    prepared_creates: &[PreparedMemoryCreate],
) -> Vec<MemoryId> {
    let created = reduction
        .results
        .iter()
        .filter_map(|result| match result.outcome {
            MemoryToolOutcome::Created { id } => Some((result.execution_id, id)),
            _ => None,
        })
        .collect::<HashMap<_, _>>();
    let mut pending = Vec::new();
    for prepared in prepared_creates {
        let Some(&created_id) = created.get(&prepared.execution_id) else {
            continue;
        };
        if !stored.items.iter().any(|item| item.id == created_id) {
            continue;
        }
        let Some(projection) = &prepared.projection else {
            continue;
        };
        let result = match projection {
            PreparedMemoryProjection::Ready(projection) => {
                repository.put_ready(projection.clone()).or_else(|_| {
                    repository.mark_repair_needed(MemoryEmbeddingRepair {
                        space_id: projection.space_id,
                        memory_id: projection.memory_id,
                        source_text: projection.source_text.clone(),
                        source_revision: projection.vector.source_revision.clone(),
                        dimensions: projection.dimensions,
                        updated_at: projection.updated_at,
                    })
                })
            }
            PreparedMemoryProjection::RepairNeeded(repair) => {
                repository.mark_repair_needed(repair.clone())
            }
        };
        if result.is_err() {
            pending.push(created_id);
        }
    }
    pending
}

#[derive(Debug)]
pub struct DynamicMemoryCreatePreparer<
    'a,
    E: MemoryEmbeddingEngine + ?Sized,
    R: MemoryEmbeddingRepository + ?Sized,
> {
    engine: &'a E,
    repository: &'a R,
}

impl<'a, E: MemoryEmbeddingEngine + ?Sized, R: MemoryEmbeddingRepository + ?Sized>
    DynamicMemoryCreatePreparer<'a, E, R>
{
    #[must_use]
    pub const fn new(engine: &'a E, repository: &'a R) -> Self {
        Self { engine, repository }
    }

    pub fn prepare_admitted(
        &self,
        space_id: MemorySpaceId,
        executions: &[ToolExecution],
        seeds: &[MemoryCreateSeed],
        duplicate_threshold: lettuce_memory::Score,
        claim: &Claim,
        handle: &JobHandle,
    ) -> Result<Vec<PreparedMemoryCreate>, DynamicMemoryPreparationError> {
        validate_embedding_admission(claim, handle)?;
        let cancellation = handle.cancellation_token();
        let source_revision = self.engine.source_revision();
        let existing = self
            .repository
            .list_ready(space_id, source_revision, EmbeddingDimensions::D128)?
            .into_iter()
            .map(|projection| (projection.memory_id, projection.vector))
            .collect::<Vec<_>>();
        let seed_count = seeds.len();
        let mut seeds = seeds
            .iter()
            .map(|seed| (seed.execution_id, *seed))
            .collect::<HashMap<_, _>>();
        if seeds.len() != seed_count
            || seeds.len()
                != executions
                    .iter()
                    .filter(|execution| execution.definition_name == "create_memory")
                    .count()
        {
            return Err(DynamicMemoryPreparationError::InvalidSeeds);
        }
        let mut prepared = Vec::with_capacity(seeds.len());
        for execution in executions {
            execution
                .validate()
                .map_err(|_| DynamicMemoryPreparationError::InvalidExecution)?;
            if execution.status != ToolExecutionStatus::Running
                || execution.definition_version != TOOL_VERSION
            {
                return Err(DynamicMemoryPreparationError::InvalidExecution);
            }
            let arguments =
                MemoryToolArguments::parse(&execution.definition_name, &execution.arguments)?;
            let MemoryToolArguments::CreateMemory { text, .. } = arguments else {
                continue;
            };
            let seed = seeds
                .remove(&execution.id)
                .ok_or(DynamicMemoryPreparationError::InvalidSeeds)?;
            if cancellation.is_cancelled() {
                return Err(DynamicMemoryPreparationError::Cancelled);
            }
            let generated = self.engine.embed_memory(
                &EmbeddingRequest {
                    text: text.clone(),
                    dimensions: EmbeddingDimensions::D128,
                },
                &cancellation,
            );
            let (semantic_duplicate, projection) = match generated {
                Ok(vector) => {
                    let evidence = EmbeddingService::semantic_duplicate_evidence(
                        &vector,
                        &existing,
                        duplicate_threshold,
                    );
                    (
                        evidence,
                        PreparedMemoryProjection::Ready(MemoryEmbeddingProjection {
                            space_id,
                            memory_id: seed.id,
                            source_text: text,
                            vector,
                            dimensions: EmbeddingDimensions::D128,
                            updated_at: seed.created_at,
                        }),
                    )
                }
                Err(EmbeddingGenerationError::Cancelled) => {
                    return Err(DynamicMemoryPreparationError::Cancelled);
                }
                Err(EmbeddingGenerationError::Unavailable) => (
                    None,
                    PreparedMemoryProjection::RepairNeeded(MemoryEmbeddingRepair {
                        space_id,
                        memory_id: seed.id,
                        source_text: text,
                        source_revision: source_revision.to_owned(),
                        dimensions: EmbeddingDimensions::D128,
                        updated_at: seed.created_at,
                    }),
                ),
            };
            prepared.push(PreparedMemoryCreate {
                execution_id: execution.id,
                preparation: CreateMemoryPreparation {
                    id: seed.id,
                    token_count: seed.token_count,
                    created_at: seed.created_at,
                    semantic_duplicate,
                },
                projection: Some(projection),
            });
        }
        if !seeds.is_empty() {
            return Err(DynamicMemoryPreparationError::InvalidSeeds);
        }
        Ok(prepared)
    }
}

fn validate_embedding_admission(
    claim: &Claim,
    handle: &JobHandle,
) -> Result<(), DynamicMemoryPreparationError> {
    let required = [
        ResourceClass::ModelLoad,
        ResourceClass::DiskRead,
        ResourceClass::Cpu,
    ];
    if claim.claim.job_id != handle.id()
        || claim.cancellation_policy != lettuce_jobs::CancellationPolicy::Cooperative
        || required
            .iter()
            .any(|resource| !claim.resources.contains(resource))
    {
        return Err(DynamicMemoryPreparationError::InvalidAdmission);
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum DynamicMemoryPreparationError {
    #[error("embedding job admission is invalid")]
    InvalidAdmission,
    #[error("dynamic-memory execution is invalid or not running")]
    InvalidExecution,
    #[error("dynamic-memory create seeds are invalid")]
    InvalidSeeds,
    #[error("dynamic-memory preparation was cancelled")]
    Cancelled,
    #[error("dynamic-memory tool call is invalid: {0}")]
    Tool(#[from] MemoryToolError),
    #[error("embedding projection repository failed: {0}")]
    Projection(#[from] EmbeddingProjectionError),
}

fn prepare_calls(
    space_id: MemorySpaceId,
    executions: &[ToolExecution],
    prepared_creates: &[PreparedMemoryCreate],
) -> Result<Vec<MemoryToolCall>, DynamicMemoryHandlerError> {
    if executions.is_empty() {
        return Err(DynamicMemoryHandlerError::EmptyRound);
    }
    let owner = (
        executions[0].conversation_id,
        executions[0].turn_id,
        executions[0].attempt_id,
    );
    let mut previous_ordinal = None;
    let mut execution_ids = HashSet::with_capacity(executions.len());
    let mut preparations = prepared_creates
        .iter()
        .map(|prepared| (prepared.execution_id, prepared.clone()))
        .collect::<HashMap<_, _>>();
    if preparations.len() != prepared_creates.len() {
        return Err(DynamicMemoryHandlerError::DuplicatePreparation);
    }

    let mut calls = Vec::with_capacity(executions.len());
    for execution in executions {
        execution
            .validate()
            .map_err(|_| DynamicMemoryHandlerError::InvalidExecution)?;
        if execution.status != ToolExecutionStatus::Running
            || execution.definition_version != TOOL_VERSION
            || owner
                != (
                    execution.conversation_id,
                    execution.turn_id,
                    execution.attempt_id,
                )
            || previous_ordinal.is_some_and(|previous| execution.ordinal != previous + 1)
            || !execution_ids.insert(execution.id)
        {
            return Err(DynamicMemoryHandlerError::InvalidExecution);
        }
        let arguments =
            MemoryToolArguments::parse(&execution.definition_name, &execution.arguments)?;
        let create = if let MemoryToolArguments::CreateMemory { text, .. } = &arguments {
            let prepared = preparations
                .remove(&execution.id)
                .ok_or(DynamicMemoryHandlerError::MissingPreparation)?;
            if let Some(projection) = &prepared.projection {
                let valid = match projection {
                    PreparedMemoryProjection::Ready(projection) => {
                        projection.space_id == space_id
                            && projection.memory_id == prepared.preparation.id
                            && projection.source_text == *text
                    }
                    PreparedMemoryProjection::RepairNeeded(repair) => {
                        repair.space_id == space_id
                            && repair.memory_id == prepared.preparation.id
                            && repair.source_text == *text
                    }
                };
                if !valid {
                    return Err(DynamicMemoryHandlerError::InvalidPreparation);
                }
            }
            Some(prepared.preparation)
        } else {
            None
        };
        calls.push(MemoryToolCall {
            execution_id: execution.id,
            arguments,
            create,
        });
        previous_ordinal = Some(execution.ordinal);
    }
    if !preparations.is_empty() {
        return Err(DynamicMemoryHandlerError::UnknownPreparation);
    }
    Ok(calls)
}

#[derive(Debug, thiserror::Error)]
pub enum DynamicMemoryHandlerError {
    #[error("dynamic-memory round is empty")]
    EmptyRound,
    #[error("dynamic-memory execution is invalid or not running")]
    InvalidExecution,
    #[error("dynamic-memory create preparation is duplicated")]
    DuplicatePreparation,
    #[error("dynamic-memory create execution is not prepared")]
    MissingPreparation,
    #[error("dynamic-memory create preparation has no matching execution")]
    UnknownPreparation,
    #[error("dynamic-memory create preparation does not match its memory call")]
    InvalidPreparation,
    #[error("dynamic-memory tool call is invalid: {0}")]
    Tool(#[from] MemoryToolError),
    #[error("dynamic-memory repository failed: {0}")]
    Repository(#[from] MemoryRepositoryError),
    #[error("dynamic-memory output serialization failed")]
    OutputSerialization,
}

#[cfg(test)]
mod tests {
    use lettuce_conversations::{
        ProposedToolCall, ToolExecution, ToolExecutionOwner, ToolExecutionStatus,
    };
    use lettuce_embeddings::{
        EmbeddingDimensions, EmbeddingRequest, EmbeddingVector, MemoryEmbeddingProjection,
        MemoryEmbeddingRepository,
    };
    use lettuce_jobs::{
        AttemptNo, CancellationPolicy, Claim, ClaimRef, LeaseId, OutcomeRef, RecoveryPolicy,
        ResourceClass, WorkerId, handle::CancellationToken, handle::JobHandle,
    };
    use lettuce_memory::{
        CreateMemoryPreparation, MemoryCategory, MemoryItem, MemoryPolicy, MemoryRepository,
        MemorySpaceSnapshot, MemoryToolOutcome, Score, dynamic_memory_tool_request,
    };
    use lettuce_types::{
        ConversationId, GenerationAttemptId, GenerationTurnId, JobId, MemoryId, MemorySpaceId,
        Revision, TimestampMillis, ToolExecutionId,
    };
    use serde_json::{Value, json};

    use super::{
        DynamicMemoryCreatePreparer, DynamicMemoryHandlerError, DynamicMemoryPreparationError,
        DynamicMemoryRecovery, MemoryCreateSeed, PreparedMemoryCreate, classify_recovery,
    };
    use crate::{AppBackend, EmbeddingGenerationError, MemoryEmbeddingEngine};

    struct FakeEmbeddingEngine {
        unavailable: bool,
    }

    impl MemoryEmbeddingEngine for FakeEmbeddingEngine {
        fn source_revision(&self) -> &str {
            "v4-test"
        }

        fn embed_memory(
            &self,
            request: &EmbeddingRequest,
            cancellation: &CancellationToken,
        ) -> Result<EmbeddingVector, EmbeddingGenerationError> {
            if cancellation.is_cancelled() {
                return Err(EmbeddingGenerationError::Cancelled);
            }
            if self.unavailable {
                return Err(EmbeddingGenerationError::Unavailable);
            }
            let mut values = vec![0.0; request.dimensions.get()];
            values[0] = 1.0;
            Ok(EmbeddingVector {
                source_revision: self.source_revision().to_owned(),
                values,
            })
        }
    }

    fn admitted_embedding_job() -> (Claim, JobHandle) {
        let id = JobId::new();
        (
            Claim {
                claim: ClaimRef {
                    job_id: id,
                    worker_id: WorkerId::new(),
                    attempt: AttemptNo::new(1),
                    lease_id: LeaseId::new(),
                },
                lease_expires_at: TimestampMillis::new(100),
                input_ref: OutcomeRef::MemoryRun(MemoryId::new()),
                recovery_policy: RecoveryPolicy::Restart,
                cancellation_policy: CancellationPolicy::Cooperative,
                resources: vec![
                    ResourceClass::ModelLoad,
                    ResourceClass::DiskRead,
                    ResourceClass::Cpu,
                ],
            },
            JobHandle::new(id),
        )
    }

    fn score(value: u16) -> Score {
        match Score::from_basis_points(value) {
            Some(score) => score,
            None => panic!("test score must be valid"),
        }
    }

    fn policy() -> MemoryPolicy {
        MemoryPolicy {
            max_entries: 10,
            hot_token_budget: 100,
            cold_threshold: score(2_000),
            delete_confidence_default: score(5_000),
            max_hard_delete_ratio_per_cycle: score(5_000),
        }
    }

    fn memory_item(id: MemoryId, text: &str) -> MemoryItem {
        MemoryItem {
            id,
            text: text.to_owned(),
            category: MemoryCategory::Other,
            token_count: 3,
            is_cold: false,
            is_pinned: false,
            importance: Score::FULL,
            persistence_importance: Score::FULL,
            prompt_importance: Score::FULL,
            volatility: Score::LEGACY_VOLATILITY,
            access_count: 0,
            created_at: TimestampMillis::new(1),
            last_accessed_at: TimestampMillis::new(1),
        }
    }

    fn validated_execution(
        owner: ToolExecutionOwner,
        ordinal: u16,
        name: &str,
        arguments: Value,
    ) -> ToolExecution {
        let definition = dynamic_memory_tool_request()
            .definitions
            .into_iter()
            .find(|definition| definition.name == name);
        let definition = match definition {
            Some(definition) => definition,
            None => panic!("test definition must exist"),
        };
        let requested = ToolExecution::requested(
            ToolExecutionId::new(),
            owner,
            ordinal,
            &definition,
            ProposedToolCall {
                provider_call_id: Some(format!("call-{ordinal}")),
                name: name.to_owned(),
                arguments,
                raw_arguments: None,
                provider_replay: None,
            },
            TimestampMillis::new(1),
        );
        let requested = match requested {
            Ok(execution) => execution,
            Err(error) => panic!("request failed: {error}"),
        };
        match requested.transition(
            ToolExecutionStatus::Validated,
            None,
            None,
            TimestampMillis::new(2),
        ) {
            Ok(execution) => execution,
            Err(error) => panic!("validation failed: {error}"),
        }
    }

    fn running_execution(
        owner: ToolExecutionOwner,
        ordinal: u16,
        name: &str,
        arguments: Value,
    ) -> ToolExecution {
        let validated = validated_execution(owner, ordinal, name, arguments);
        match validated.transition(
            ToolExecutionStatus::Running,
            None,
            None,
            TimestampMillis::new(3),
        ) {
            Ok(execution) => execution,
            Err(error) => panic!("start failed: {error}"),
        }
    }

    fn owner() -> ToolExecutionOwner {
        ToolExecutionOwner {
            conversation_id: ConversationId::new(),
            turn_id: GenerationTurnId::new(),
            attempt_id: GenerationAttemptId::new(),
        }
    }

    #[test]
    fn admitted_round_persists_once_and_returns_settlement_outputs() {
        let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let space_id = MemorySpaceId::new();
        MemoryRepository::create(
            backend.database(),
            MemorySpaceSnapshot {
                id: space_id,
                revision: Revision::INITIAL,
                items: vec![],
            },
        )
        .expect("memory space");
        let owner = owner();
        let create = running_execution(
            owner,
            4,
            "create_memory",
            json!({"text": "Mira prefers tea", "category": "preference"}),
        );
        let done = running_execution(owner, 5, "done", json!({"summary": "updated"}));
        let memory_id = MemoryId::new();
        let result = backend
            .dynamic_memory_handler()
            .apply_admitted_round(
                space_id,
                &policy(),
                &[create.clone(), done.clone()],
                &[PreparedMemoryCreate {
                    execution_id: create.id,
                    preparation: CreateMemoryPreparation {
                        id: memory_id,
                        token_count: 4,
                        created_at: TimestampMillis::new(4),
                        semantic_duplicate: None,
                    },
                    projection: None,
                }],
            )
            .expect("handle round");
        assert_eq!(result.snapshot.revision, Revision::new(2));
        assert_eq!(result.snapshot.items[0].id, memory_id);
        assert_eq!(
            result.outputs.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            vec![create.id, done.id]
        );
        assert!(result.outputs.iter().all(|(_, output)| !output.is_error));
        assert_eq!(
            MemoryRepository::get(backend.database(), space_id)
                .expect("stored")
                .map(|snapshot| snapshot.revision),
            Some(Revision::new(2))
        );
    }

    #[test]
    fn invalid_or_unprepared_round_does_not_mutate_memory() {
        let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let space_id = MemorySpaceId::new();
        let original = MemoryRepository::create(
            backend.database(),
            MemorySpaceSnapshot {
                id: space_id,
                revision: Revision::INITIAL,
                items: vec![],
            },
        )
        .expect("memory space");
        let execution = running_execution(
            owner(),
            0,
            "create_memory",
            json!({"text": "Mira prefers tea", "category": "preference"}),
        );
        assert!(matches!(
            backend.dynamic_memory_handler().apply_admitted_round(
                space_id,
                &policy(),
                &[execution],
                &[],
            ),
            Err(DynamicMemoryHandlerError::MissingPreparation)
        ));
        assert_eq!(
            MemoryRepository::get(backend.database(), space_id).expect("stored"),
            Some(original)
        );
    }

    #[test]
    fn admitted_preparation_persists_ready_projection_after_memory_commit() {
        let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let space_id = MemorySpaceId::new();
        MemoryRepository::create(
            backend.database(),
            MemorySpaceSnapshot {
                id: space_id,
                revision: Revision::INITIAL,
                items: vec![],
            },
        )
        .expect("space");
        let execution = running_execution(
            owner(),
            0,
            "create_memory",
            json!({"text": "Mira prefers tea", "category": "preference"}),
        );
        let memory_id = MemoryId::new();
        let (claim, handle) = admitted_embedding_job();
        let engine = FakeEmbeddingEngine { unavailable: false };
        let prepared = DynamicMemoryCreatePreparer::new(&engine, backend.database())
            .prepare_admitted(
                space_id,
                std::slice::from_ref(&execution),
                &[MemoryCreateSeed {
                    execution_id: execution.id,
                    id: memory_id,
                    token_count: 4,
                    created_at: TimestampMillis::new(4),
                }],
                score(9_000),
                &claim,
                &handle,
            )
            .expect("prepare");
        let result = backend
            .dynamic_memory_handler()
            .apply_admitted_round(space_id, &policy(), &[execution], &prepared)
            .expect("apply");

        assert!(result.projection_repairs_pending.is_empty());
        let projections = backend
            .database()
            .list_ready(space_id, "v4-test", EmbeddingDimensions::D128)
            .expect("projections");
        assert_eq!(projections.len(), 1);
        assert_eq!(projections[0].memory_id, memory_id);
    }

    #[test]
    fn unavailable_embedding_preserves_create_and_records_repair() {
        let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let space_id = MemorySpaceId::new();
        MemoryRepository::create(
            backend.database(),
            MemorySpaceSnapshot {
                id: space_id,
                revision: Revision::INITIAL,
                items: vec![],
            },
        )
        .expect("space");
        let execution = running_execution(
            owner(),
            0,
            "create_memory",
            json!({"text": "Mira prefers tea", "category": "preference"}),
        );
        let memory_id = MemoryId::new();
        let (claim, handle) = admitted_embedding_job();
        let engine = FakeEmbeddingEngine { unavailable: true };
        let prepared = DynamicMemoryCreatePreparer::new(&engine, backend.database())
            .prepare_admitted(
                space_id,
                std::slice::from_ref(&execution),
                &[MemoryCreateSeed {
                    execution_id: execution.id,
                    id: memory_id,
                    token_count: 4,
                    created_at: TimestampMillis::new(4),
                }],
                score(9_000),
                &claim,
                &handle,
            )
            .expect("prepare");
        let result = backend
            .dynamic_memory_handler()
            .apply_admitted_round(space_id, &policy(), &[execution], &prepared)
            .expect("apply");

        assert_eq!(result.snapshot.items[0].id, memory_id);
        assert!(result.projection_repairs_pending.is_empty());
        let repairs = backend
            .database()
            .list_repairs(space_id, "v4-test", EmbeddingDimensions::D128)
            .expect("repairs");
        assert_eq!(repairs.len(), 1);
        assert_eq!(repairs[0].memory_id, memory_id);
    }

    #[test]
    fn preparation_uses_live_same_revision_projection_for_duplicate_evidence() {
        let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let space_id = MemorySpaceId::new();
        let existing_id = MemoryId::new();
        let existing = memory_item(existing_id, "Mira likes green tea");
        MemoryRepository::create(
            backend.database(),
            MemorySpaceSnapshot {
                id: space_id,
                revision: Revision::INITIAL,
                items: vec![existing.clone()],
            },
        )
        .expect("space");
        backend
            .database()
            .put_ready(MemoryEmbeddingProjection {
                space_id,
                memory_id: existing_id,
                source_text: existing.text,
                vector: EmbeddingVector {
                    source_revision: "v4-test".to_owned(),
                    values: {
                        let mut values = vec![0.0; 128];
                        values[0] = 1.0;
                        values
                    },
                },
                dimensions: EmbeddingDimensions::D128,
                updated_at: TimestampMillis::new(2),
            })
            .expect("existing projection");
        let execution = running_execution(
            owner(),
            0,
            "create_memory",
            json!({"text": "Mira strongly prefers tea", "category": "preference"}),
        );
        let (claim, handle) = admitted_embedding_job();
        let engine = FakeEmbeddingEngine { unavailable: false };
        let prepared = DynamicMemoryCreatePreparer::new(&engine, backend.database())
            .prepare_admitted(
                space_id,
                std::slice::from_ref(&execution),
                &[MemoryCreateSeed {
                    execution_id: execution.id,
                    id: MemoryId::new(),
                    token_count: 4,
                    created_at: TimestampMillis::new(4),
                }],
                score(9_000),
                &claim,
                &handle,
            )
            .expect("prepare");
        let result = backend
            .dynamic_memory_handler()
            .apply_admitted_round(space_id, &policy(), &[execution], &prepared)
            .expect("apply");

        assert!(matches!(
            result.reduction.results[0].outcome,
            MemoryToolOutcome::DuplicateSkipped { existing_id: id } if id == existing_id
        ));
        assert_eq!(result.snapshot.items.len(), 1);
    }

    #[test]
    fn cancelled_embedding_job_stops_before_memory_preparation() {
        let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let space_id = MemorySpaceId::new();
        MemoryRepository::create(
            backend.database(),
            MemorySpaceSnapshot {
                id: space_id,
                revision: Revision::INITIAL,
                items: vec![],
            },
        )
        .expect("space");
        let execution = running_execution(
            owner(),
            0,
            "create_memory",
            json!({"text": "Mira prefers tea", "category": "preference"}),
        );
        let (claim, handle) = admitted_embedding_job();
        handle.request_cancel();
        let engine = FakeEmbeddingEngine { unavailable: false };
        let result = DynamicMemoryCreatePreparer::new(&engine, backend.database())
            .prepare_admitted(
                space_id,
                std::slice::from_ref(&execution),
                &[MemoryCreateSeed {
                    execution_id: execution.id,
                    id: MemoryId::new(),
                    token_count: 4,
                    created_at: TimestampMillis::new(4),
                }],
                score(9_000),
                &claim,
                &handle,
            );

        assert!(matches!(
            result,
            Err(DynamicMemoryPreparationError::Cancelled)
        ));
        assert!(
            MemoryRepository::get(backend.database(), space_id)
                .expect("space")
                .is_some_and(|snapshot| snapshot.items.is_empty())
        );
    }

    #[test]
    fn recovery_classifies_terminal_validated_and_running_without_rerunning() {
        let owner = owner();
        let validated = validated_execution(owner, 0, "done", json!({"summary": "complete"}));
        assert!(matches!(
            classify_recovery(vec![validated.clone()]),
            Ok(DynamicMemoryRecovery::ValidatedStart { .. })
        ));
        let running = validated
            .transition(
                ToolExecutionStatus::Running,
                None,
                None,
                TimestampMillis::new(3),
            )
            .expect("running");
        assert!(matches!(
            classify_recovery(vec![running.clone()]),
            Ok(DynamicMemoryRecovery::RestartBlocked { .. })
        ));
        let terminal = running
            .transition(
                ToolExecutionStatus::Succeeded,
                Some(lettuce_conversations::ToolOutput {
                    value: json!({"kind": "done", "summary": "complete"}),
                    is_error: false,
                }),
                None,
                TimestampMillis::new(4),
            )
            .expect("terminal");
        let recovered = classify_recovery(vec![terminal.clone()]).expect("recovery");
        assert_eq!(
            recovered,
            DynamicMemoryRecovery::TerminalReplay {
                executions: vec![terminal]
            }
        );
    }
}
