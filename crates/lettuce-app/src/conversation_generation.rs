use std::time::Duration;

use lettuce_conversations::{
    ArtifactError, AttachAttemptJob, CancelGeneration, ContextAttributions, ConversationManager,
    ConversationReader, ConversationRepository, ConversationRepositoryError,
    ConversationServiceError, GenerationAttempt, GenerationAttemptStatus,
    GenerationCheckpointEnvelope, GenerationCheckpointEvent, GenerationFailureCode, GenerationTurn,
    GenerationTurnStatus, InferenceOutcome, InferencePort, InferenceRequest,
    InitialInferenceBinding, InitialInferenceRepository, MessageCandidate, MessagePart,
    ModelSelectionSnapshot, OperationToken, PortError, ProviderFailureKind, ProviderNeutralContext,
    ProviderReplayArtifactPort, ResolvedInferenceProfile, SettleCancellation, ToolExecution,
    ToolExecutionOwner, ToolExecutionRepository, ToolExecutionStatus, ToolExecutionTransition,
    ToolRequest, UsageCounters, UsageOutcome, UsagePort, UsageRecord, UsageUnavailableReason,
    ValidationError, attempt_job_idempotency_key,
};
use lettuce_embeddings::MemoryEmbeddingRepository;
use lettuce_jobs::{
    CancellationPolicy, CancellationReason, ChildLink, Claim, FiniteFraction, IdempotencyKey,
    JobError, JobErrorCode, JobKind, JobMutation, JobOutcome, JobPriority, JobSnapshot, JobSpec,
    JobState, JobStore, JobSubject, OutcomeRef, ProgressSnapshot, RecoveryPolicy,
    ResourceAvailability, ResourceClass, StageSnapshot, StoreError, SubjectKind, WorkerId,
    handle::JobHandle,
};
use lettuce_memory::{
    DynamicMemoryPreparationRepository, DynamicMemoryRoundRepository, MemoryPolicy,
    MemoryRepository, Score, dynamic_memory_tool_request,
};
use lettuce_types::{
    AssetId, ContentHash, ConversationId, GenerationAttemptId, GenerationTurnId, JobId,
    MemorySpaceId, RequestId, TimestampMillis, UsageEventId,
};
use lettuce_usage::{JobInferenceUsageResult, JobUsageLedger, UsageLedger, UsageLedgerError};

use crate::{
    ConversationInitialInferenceCoordinator, ConversationInitialInferenceError,
    DynamicMemoryContinuationCoordinator, DynamicMemoryContinuationError,
    DynamicMemoryContinuationLoopResult, DynamicMemoryContinuationTerminal,
    DynamicMemoryRoundExecutionError, DynamicMemoryRoundExecutor, DynamicMemoryTerminalCommit,
    DynamicMemoryTerminalContext, DynamicMemoryTerminalCoordinator, DynamicMemoryTerminalError,
    MemoryCreateSeed, MemoryEmbeddingEngine,
};

const STAGE_LABEL: &str = "conversation-generation";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConversationGenerationOperation {
    AttachJob,
    StagePreparing,
    StageRunning,
    Prepare,
    Finalize,
    Fail,
    RequestCancellation,
    SettleCancellation,
    Interrupt,
    Recover,
}

impl ConversationGenerationOperation {
    const fn label(self) -> &'static str {
        match self {
            Self::AttachJob => "attach-job",
            Self::StagePreparing => "stage-preparing",
            Self::StageRunning => "stage-running",
            Self::Prepare => "prepare",
            Self::Finalize => "finalize",
            Self::Fail => "fail",
            Self::RequestCancellation => "request-cancellation",
            Self::SettleCancellation => "settle-cancellation",
            Self::Interrupt => "interrupt",
            Self::Recover => "recover",
        }
    }
}

pub(crate) fn operation_token(
    conversation_id: ConversationId,
    turn_id: GenerationTurnId,
    attempt_id: GenerationAttemptId,
    job_id: JobId,
    operation: ConversationGenerationOperation,
) -> OperationToken {
    let label = operation.label();
    let key = IdempotencyKey::new(format!("generation.{turn_id}.{attempt_id}.{label}"))
        .expect("generated generation operation key is valid");
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"lettuce-conversation-generation-operation-v1\0");
    for part in [
        conversation_id.to_string(),
        turn_id.to_string(),
        attempt_id.to_string(),
        job_id.to_string(),
        label.to_owned(),
    ] {
        hasher.update(part.as_bytes());
        hasher.update(b"\0");
    }
    OperationToken {
        key,
        request_digest: ContentHash::parse(hasher.finalize().to_hex().as_str())
            .expect("blake3 hex digest is a valid content hash"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerationUsageEvidence {
    None,
    Dispatch(UsageEventId),
    Event(UsageEventId),
}

impl GenerationUsageEvidence {
    fn event(id: Option<UsageEventId>) -> Self {
        id.map_or(Self::None, Self::Event)
    }
}

#[derive(Debug, Clone)]
pub struct ConversationGenerationAdmission {
    pub job: JobSnapshot,
    pub attempt: GenerationAttempt,
    pub created: bool,
}

#[derive(Debug, Clone)]
pub struct ConversationGenerationClaimedWork {
    pub conversation_id: ConversationId,
    pub turn_id: GenerationTurnId,
    pub attempt_id: GenerationAttemptId,
    pub claim: Claim,
    pub handle: JobHandle,
    pub job: JobSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationGenerationMemoryInput {
    pub space_id: MemorySpaceId,
    pub policy: MemoryPolicy,
    pub duplicate_threshold: Score,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConversationGenerationInput {
    pub model: ModelSelectionSnapshot,
    pub attributions: ContextAttributions,
    pub profile: ResolvedInferenceProfile,
    pub context: ProviderNeutralContext,
    pub tools: Option<ToolRequest>,
    pub media_grants: Vec<AssetId>,
    pub stream_sink: Option<RequestId>,
    pub memory: Option<ConversationGenerationMemoryInput>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationGenerationRunResult {
    pub turn: GenerationTurn,
    pub candidate: MessageCandidate,
    pub usage_event_id: UsageEventId,
    pub outcomes: Vec<InferenceOutcome>,
    pub rounds: u8,
    pub replayed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationGenerationTerminalFailure {
    Cancelled,
    Interrupted,
    Failed(GenerationFailureCode),
    Invalid,
}

#[derive(Debug, thiserror::Error)]
pub enum ConversationGenerationRunError {
    #[error("conversation generation claimed work is inconsistent")]
    InvalidWork,
    #[error("conversation generation input is invalid")]
    InvalidInput,
    #[error("conversation generation was cancelled")]
    Cancelled { evidence: GenerationUsageEvidence },
    #[error("conversation generation dispatch is pending and requires attempt recovery")]
    Pending { evidence: GenerationUsageEvidence },
    #[error("conversation generation attempt already failed")]
    AlreadyFailed {
        code: GenerationFailureCode,
        evidence: GenerationUsageEvidence,
    },
    #[error("conversation generation tool rounds cannot be resumed")]
    ToolRoundsUnrecoverable { evidence: GenerationUsageEvidence },
    #[error("conversation generation provider failed: {error}")]
    Provider {
        error: PortError,
        evidence: GenerationUsageEvidence,
    },
    #[error("conversation generation tool continuation failed: {error}")]
    Continuation {
        error: DynamicMemoryContinuationError,
        evidence: GenerationUsageEvidence,
    },
    #[error("conversation generation tool round failed: {error}")]
    Round {
        error: DynamicMemoryRoundExecutionError,
        evidence: GenerationUsageEvidence,
    },
    #[error("conversation generation terminal settlement failed: {error}")]
    Terminal {
        error: DynamicMemoryTerminalError,
        evidence: GenerationUsageEvidence,
    },
    #[error("conversation generation repository failed: {0}")]
    Repository(#[from] ConversationRepositoryError),
    #[error("conversation generation persistence failed: {0}")]
    Conversation(#[from] ConversationServiceError),
    #[error("conversation generation replay cleanup failed: {0}")]
    Artifact(#[from] ArtifactError),
    #[error("conversation generation contract is invalid: {0}")]
    Validation(#[from] ValidationError),
}

impl ConversationGenerationRunError {
    #[must_use]
    pub fn evidence(&self) -> GenerationUsageEvidence {
        match self {
            Self::Cancelled { evidence }
            | Self::Pending { evidence }
            | Self::AlreadyFailed { evidence, .. }
            | Self::ToolRoundsUnrecoverable { evidence }
            | Self::Provider { evidence, .. }
            | Self::Continuation { evidence, .. }
            | Self::Round { evidence, .. }
            | Self::Terminal { evidence, .. } => *evidence,
            _ => GenerationUsageEvidence::None,
        }
    }

    #[must_use]
    pub fn terminal_failure(&self) -> Option<ConversationGenerationTerminalFailure> {
        use ConversationGenerationTerminalFailure as Terminal;
        match self {
            Self::InvalidWork | Self::InvalidInput | Self::Validation(_) => Some(Terminal::Invalid),
            Self::Cancelled { .. } => Some(Terminal::Cancelled),
            Self::Pending { .. } => Some(Terminal::Interrupted),
            Self::AlreadyFailed { code, .. } => Some(Terminal::Failed(*code)),
            Self::ToolRoundsUnrecoverable { .. } => {
                Some(Terminal::Failed(GenerationFailureCode::RecoveryUnavailable))
            }
            Self::Provider { error, .. } => Some(provider_terminal(error)),
            Self::Continuation { error, .. } => continuation_terminal(error),
            Self::Round { error, .. } => round_terminal(error),
            Self::Terminal { error, .. } => match error {
                DynamicMemoryTerminalError::Usage(PortError::Cancelled) => {
                    Some(Terminal::Cancelled)
                }
                DynamicMemoryTerminalError::Usage(PortError::Rejected)
                | DynamicMemoryTerminalError::Validation(_)
                | DynamicMemoryTerminalError::InvalidIdentity => Some(Terminal::Invalid),
                DynamicMemoryTerminalError::Continuation(error) => continuation_terminal(error),
                DynamicMemoryTerminalError::Conversation(error) => service_terminal(error),
                DynamicMemoryTerminalError::Repository(error) => repository_terminal(error),
                DynamicMemoryTerminalError::Usage(_) => None,
            },
            Self::Repository(error) => repository_terminal(error),
            Self::Conversation(error) => service_terminal(error),
            Self::Artifact(_) => None,
        }
    }
}

fn repository_terminal(
    error: &ConversationRepositoryError,
) -> Option<ConversationGenerationTerminalFailure> {
    match error {
        ConversationRepositoryError::StaleRevision { .. }
        | ConversationRepositoryError::Storage
        | ConversationRepositoryError::ArtifactReference(_) => None,
        ConversationRepositoryError::NotFound
        | ConversationRepositoryError::Conflict
        | ConversationRepositoryError::JobAlreadyAttached
        | ConversationRepositoryError::JobInUse
        | ConversationRepositoryError::Dependency
        | ConversationRepositoryError::Invalid(_)
        | ConversationRepositoryError::Unsupported => {
            Some(ConversationGenerationTerminalFailure::Invalid)
        }
    }
}

fn service_terminal(
    error: &ConversationServiceError,
) -> Option<ConversationGenerationTerminalFailure> {
    match error {
        ConversationServiceError::Repository(error) => repository_terminal(error),
        ConversationServiceError::Invalid(_)
        | ConversationServiceError::Dependency
        | ConversationServiceError::Unsupported => {
            Some(ConversationGenerationTerminalFailure::Invalid)
        }
    }
}

fn provider_terminal(error: &PortError) -> ConversationGenerationTerminalFailure {
    use ConversationGenerationTerminalFailure as Terminal;
    match error {
        PortError::Cancelled => Terminal::Cancelled,
        PortError::Unavailable => Terminal::Failed(GenerationFailureCode::ProviderUnavailable),
        PortError::Provider(failure) if failure.kind == ProviderFailureKind::Unavailable => {
            Terminal::Failed(GenerationFailureCode::ProviderUnavailable)
        }
        PortError::Provider(_) | PortError::Rejected => {
            Terminal::Failed(GenerationFailureCode::ProviderRejected)
        }
        PortError::Empty => Terminal::Failed(GenerationFailureCode::EmptyOutput),
    }
}

fn continuation_terminal(
    error: &DynamicMemoryContinuationError,
) -> Option<ConversationGenerationTerminalFailure> {
    use ConversationGenerationTerminalFailure as Terminal;
    match error {
        DynamicMemoryContinuationError::Cancelled => Some(Terminal::Cancelled),
        DynamicMemoryContinuationError::Inference(error) => Some(provider_terminal(error)),
        DynamicMemoryContinuationError::EmptyCompletion => {
            Some(Terminal::Failed(GenerationFailureCode::EmptyOutput))
        }
        DynamicMemoryContinuationError::ProviderFailed
        | DynamicMemoryContinuationError::MultipleCandidates
        | DynamicMemoryContinuationError::MixedToolAndContent
        | DynamicMemoryContinuationError::InvalidSignedReplay
        | DynamicMemoryContinuationError::ToolBudgetExceeded
        | DynamicMemoryContinuationError::InvalidDoneResult
        | DynamicMemoryContinuationError::InvalidRequest
        | DynamicMemoryContinuationError::Tool(_)
        | DynamicMemoryContinuationError::Validation(_) => {
            Some(Terminal::Failed(GenerationFailureCode::ProviderRejected))
        }
        DynamicMemoryContinuationError::RoundExecution(error) => round_terminal(error),
        DynamicMemoryContinuationError::InvalidOwnership
        | DynamicMemoryContinuationError::InvalidSettledRound
        | DynamicMemoryContinuationError::InvalidAdmittedRound
        | DynamicMemoryContinuationError::UsageOverflow => {
            Some(Terminal::Failed(GenerationFailureCode::Internal))
        }
        DynamicMemoryContinuationError::Conversation(_)
        | DynamicMemoryContinuationError::Repository(_)
        | DynamicMemoryContinuationError::Artifact(_) => None,
    }
}

fn round_terminal(
    error: &DynamicMemoryRoundExecutionError,
) -> Option<ConversationGenerationTerminalFailure> {
    use ConversationGenerationTerminalFailure as Terminal;
    match error {
        DynamicMemoryRoundExecutionError::Preparation(
            crate::DynamicMemoryPreparationError::Cancelled,
        ) => Some(Terminal::Cancelled),
        DynamicMemoryRoundExecutionError::Preparation(_)
        | DynamicMemoryRoundExecutionError::Coordinator(_) => {
            Some(Terminal::Failed(GenerationFailureCode::Internal))
        }
        DynamicMemoryRoundExecutionError::Repository(_)
        | DynamicMemoryRoundExecutionError::Conversation(_) => None,
    }
}

#[derive(Debug)]
pub struct ConversationGenerationJobRunner<'a, E: ?Sized, R: ?Sized, I: ?Sized> {
    engine: &'a E,
    repository: &'a R,
    inference: &'a I,
}

impl<'a, E: ?Sized, R: ?Sized, I: ?Sized> ConversationGenerationJobRunner<'a, E, R, I> {
    #[must_use]
    pub const fn new(engine: &'a E, repository: &'a R, inference: &'a I) -> Self {
        Self {
            engine,
            repository,
            inference,
        }
    }
}

impl<
    E: MemoryEmbeddingEngine + ?Sized,
    R: ConversationRepository
        + InitialInferenceRepository
        + ToolExecutionRepository
        + ProviderReplayArtifactPort
        + JobUsageLedger
        + UsagePort
        + DynamicMemoryRoundRepository
        + DynamicMemoryPreparationRepository
        + MemoryEmbeddingRepository
        + MemoryRepository
        + ?Sized,
    I: InferencePort + ?Sized,
> ConversationGenerationJobRunner<'_, E, R, I>
{
    pub async fn run<F>(
        &self,
        work: &ConversationGenerationClaimedWork,
        input: ConversationGenerationInput,
        now: TimestampMillis,
        mut seeds_for_round: F,
    ) -> Result<ConversationGenerationRunResult, ConversationGenerationRunError>
    where
        F: FnMut(&[ToolExecution]) -> Vec<MemoryCreateSeed>,
    {
        let job_id = work.handle.id();
        if work.claim.claim.job_id != job_id || work.job.id != job_id {
            return Err(ConversationGenerationRunError::InvalidWork);
        }
        match (&input.tools, &input.memory) {
            (None, None) => {}
            (Some(tools), Some(_)) if *tools == dynamic_memory_tool_request() => {}
            _ => return Err(ConversationGenerationRunError::InvalidInput),
        }
        let conversation_id = work.conversation_id;
        let mut turn = self.repository.get_turn(work.turn_id)?;
        let attempt = attempt_of(&turn, work.attempt_id)?;
        if turn.conversation_id != conversation_id || attempt.job_id != Some(job_id) {
            return Err(ConversationGenerationRunError::InvalidWork);
        }
        match attempt.status {
            GenerationAttemptStatus::Succeeded => return self.replay_succeeded(turn, attempt),
            GenerationAttemptStatus::Failed => {
                return Err(ConversationGenerationRunError::AlreadyFailed {
                    code: attempt.failure.unwrap_or(GenerationFailureCode::Internal),
                    evidence: GenerationUsageEvidence::event(attempt.usage_event_id),
                });
            }
            GenerationAttemptStatus::Cancelled => {
                return Err(ConversationGenerationRunError::Cancelled {
                    evidence: GenerationUsageEvidence::event(attempt.usage_event_id),
                });
            }
            GenerationAttemptStatus::Interrupted => {
                return Err(ConversationGenerationRunError::Pending {
                    evidence: GenerationUsageEvidence::event(attempt.usage_event_id),
                });
            }
            GenerationAttemptStatus::Created
            | GenerationAttemptStatus::Preparing
            | GenerationAttemptStatus::Running => {}
        }
        if work.handle.cancellation_token().is_cancelled() {
            return Err(ConversationGenerationRunError::Cancelled {
                evidence: GenerationUsageEvidence::None,
            });
        }
        let token = |operation| {
            operation_token(
                conversation_id,
                work.turn_id,
                work.attempt_id,
                job_id,
                operation,
            )
        };
        if turn.status == GenerationTurnStatus::Recovering && turn.resolved_model.is_some() {
            turn = self.stage(
                &turn,
                work,
                1,
                GenerationTurnStatus::Running,
                &token(ConversationGenerationOperation::StageRunning),
                now,
            )?;
        }
        if matches!(
            turn.status,
            GenerationTurnStatus::Created | GenerationTurnStatus::Recovering
        ) {
            turn = self.stage(
                &turn,
                work,
                1,
                GenerationTurnStatus::Preparing,
                &token(ConversationGenerationOperation::StagePreparing),
                now,
            )?;
        }
        if matches!(
            turn.status,
            GenerationTurnStatus::Preparing | GenerationTurnStatus::SelectingSpeaker
        ) {
            if turn.resolved_model.is_some() {
                return Err(ConversationGenerationRunError::InvalidWork);
            }
            let aggregate = ConversationReader::get(self.repository, conversation_id)?;
            turn = self
                .repository
                .prepare_generation(
                    &lettuce_conversations::PrepareGeneration {
                        conversation_id,
                        turn_id: work.turn_id,
                        attempt_id: work.attempt_id,
                        job_id,
                        expected_revision: aggregate.conversation.revision,
                        expected_turn_revision: turn.revision,
                        operation: token(ConversationGenerationOperation::Prepare),
                        model: input.model.clone(),
                        attributions: input.attributions.clone(),
                    },
                    now,
                )?
                .value;
        }
        if turn.status == GenerationTurnStatus::ContextPrepared {
            if work.handle.cancellation_token().is_cancelled() {
                return Err(ConversationGenerationRunError::Cancelled {
                    evidence: GenerationUsageEvidence::None,
                });
            }
            turn = self.stage(
                &turn,
                work,
                2,
                GenerationTurnStatus::Running,
                &token(ConversationGenerationOperation::StageRunning),
                now,
            )?;
        }
        if !matches!(
            turn.status,
            GenerationTurnStatus::Running | GenerationTurnStatus::CancellationRequested
        ) {
            return Err(ConversationGenerationRunError::InvalidWork);
        }
        let attempt = attempt_of(&turn, work.attempt_id)?;
        let request = InferenceRequest {
            turn_id: work.turn_id,
            attempt_id: work.attempt_id,
            operation: turn.operation,
            profile: input.profile.clone(),
            context: input.context.clone(),
            cancellation: Some(job_id),
            stream_sink: input.stream_sink,
            media_grants: input.media_grants.clone(),
            tools: input.tools.clone(),
        };
        let binding = InitialInferenceBinding::from_request(conversation_id, &request)?;
        if turn.status == GenerationTurnStatus::CancellationRequested {
            return Err(ConversationGenerationRunError::Cancelled {
                evidence: self.dispatch_evidence(&binding)?,
            });
        }
        let initial = ConversationInitialInferenceCoordinator::new(self.repository, self.inference)
            .run(conversation_id, &work.handle, request.clone(), now)
            .await;
        let record = self.repository.initial_inference(&binding)?;
        let evidence = record
            .as_ref()
            .map_or(GenerationUsageEvidence::None, |record| {
                GenerationUsageEvidence::Dispatch(record.usage_event_id)
            });
        let outcome = match initial {
            Ok(outcome) => outcome,
            Err(error) => return Err(initial_error(error, evidence)),
        };
        let settled_at = record
            .and_then(|record| record.settled_at)
            .ok_or(ConversationRepositoryError::Storage)?;
        if outcome.candidates.len() != 1 {
            return Err(ConversationGenerationRunError::Provider {
                error: PortError::Rejected,
                evidence,
            });
        }
        match outcome.finish_reason {
            lettuce_conversations::FinishReason::Cancelled => {
                return Err(ConversationGenerationRunError::Cancelled { evidence });
            }
            lettuce_conversations::FinishReason::Error => {
                return Err(ConversationGenerationRunError::Provider {
                    error: PortError::Rejected,
                    evidence,
                });
            }
            lettuce_conversations::FinishReason::Stop
            | lettuce_conversations::FinishReason::Length => {}
        }
        let candidate = outcome.candidates[0].clone();
        let (loop_result, rounds) = if candidate.tool_calls.is_empty() {
            if !candidate
                .parts
                .iter()
                .any(|part| matches!(part, MessagePart::Text { text } if !text.trim().is_empty()))
            {
                return Err(ConversationGenerationRunError::Provider {
                    error: PortError::Empty,
                    evidence,
                });
            }
            (
                DynamicMemoryContinuationLoopResult {
                    terminal: DynamicMemoryContinuationTerminal::Complete { candidate },
                    outcomes: vec![outcome],
                },
                0,
            )
        } else {
            let tools = request
                .tools
                .as_ref()
                .ok_or(ConversationGenerationRunError::InvalidInput)?;
            let memory = input
                .memory
                .as_ref()
                .ok_or(ConversationGenerationRunError::InvalidInput)?;
            if !self
                .repository
                .list_tool_executions(conversation_id, work.turn_id, work.attempt_id)?
                .is_empty()
            {
                return Err(ConversationGenerationRunError::ToolRoundsUnrecoverable { evidence });
            }
            if work.handle.cancellation_token().is_cancelled() {
                return Err(ConversationGenerationRunError::Cancelled { evidence });
            }
            let requested = ConversationManager::new(self.repository).request_tool_executions(
                ToolExecutionOwner {
                    conversation_id,
                    turn_id: work.turn_id,
                    attempt_id: work.attempt_id,
                },
                tools,
                candidate.tool_calls.clone(),
                now,
            )?;
            let validated = self.repository.transition_tool_execution_batch(
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
                now,
            )?;
            let executor = DynamicMemoryRoundExecutor::new(
                self.engine,
                self.repository,
                &work.claim,
                memory.space_id,
                &memory.policy,
                memory.duplicate_threshold,
            );
            let seeds = seeds_for_round(&validated);
            let first = executor
                .execute_admitted_round(&validated, &seeds, &work.handle, now)
                .map_err(|error| ConversationGenerationRunError::Round { error, evidence })?;
            let first_calls = u16::try_from(validated.len())
                .map_err(|_| ConversationGenerationRunError::InvalidInput)?;
            let result = DynamicMemoryContinuationCoordinator::new(self.repository, self.inference)
                .continue_until_terminal(
                    conversation_id,
                    &attempt,
                    &work.handle,
                    request,
                    first.settled_executions,
                    vec![outcome],
                    1,
                    first_calls,
                    now,
                    |executions, handle, at| {
                        let seeds = seeds_for_round(executions);
                        executor.execute_admitted_round(executions, &seeds, handle, at)
                    },
                )
                .await
                .map_err(|error| ConversationGenerationRunError::Continuation {
                    error,
                    evidence,
                })?;
            let rounds = u8::try_from(result.outcomes.len().saturating_sub(1))
                .map_err(|_| ConversationGenerationRunError::InvalidInput)?;
            (result, rounds)
        };
        let outcomes = loop_result.outcomes.clone();
        if work.handle.cancellation_token().is_cancelled() {
            return Err(ConversationGenerationRunError::Cancelled { evidence });
        }
        let aggregate = ConversationReader::get(self.repository, conversation_id)?;
        let turn = self.repository.get_turn(work.turn_id)?;
        if turn.status == GenerationTurnStatus::CancellationRequested {
            return Err(ConversationGenerationRunError::Cancelled { evidence });
        }
        let attempt = attempt_of(&turn, work.attempt_id)?;
        let commit = DynamicMemoryTerminalCoordinator::new(self.repository, self.repository)
            .commit(
                &attempt,
                &input.profile,
                loop_result,
                DynamicMemoryTerminalContext {
                    conversation_id,
                    expected_conversation_revision: aggregate.conversation.revision,
                    expected_turn_revision: turn.revision,
                    operation: token(ConversationGenerationOperation::Finalize),
                    model: input.model,
                    usage_recorded_at: settled_at,
                    finalized_at: now,
                },
            )
            .await
            .map_err(|error| ConversationGenerationRunError::Terminal { error, evidence })?;
        match commit {
            DynamicMemoryTerminalCommit::ConversationFinalized(finalized) => {
                Ok(ConversationGenerationRunResult {
                    turn: finalized.value.turn,
                    candidate: finalized.value.candidate,
                    usage_event_id: finalized.value.usage_event_id,
                    outcomes,
                    rounds,
                    replayed: false,
                })
            }
            DynamicMemoryTerminalCommit::DerivedMemoryDone { usage_event_id, .. } => {
                Err(ConversationGenerationRunError::Provider {
                    error: PortError::Empty,
                    evidence: GenerationUsageEvidence::Event(usage_event_id),
                })
            }
        }
    }

    fn dispatch_evidence(
        &self,
        binding: &InitialInferenceBinding,
    ) -> Result<GenerationUsageEvidence, ConversationGenerationRunError> {
        Ok(self
            .repository
            .initial_inference(binding)?
            .map_or(GenerationUsageEvidence::None, |record| {
                GenerationUsageEvidence::Dispatch(record.usage_event_id)
            }))
    }

    fn replay_succeeded(
        &self,
        turn: GenerationTurn,
        attempt: GenerationAttempt,
    ) -> Result<ConversationGenerationRunResult, ConversationGenerationRunError> {
        let candidate_id = turn
            .selected_candidate_id
            .ok_or(ConversationRepositoryError::Storage)?;
        let candidate = self.repository.get_candidate(candidate_id)?;
        if candidate.turn_id != turn.id || candidate.attempt_id != attempt.id {
            return Err(ConversationRepositoryError::Storage.into());
        }
        Ok(ConversationGenerationRunResult {
            usage_event_id: attempt
                .usage_event_id
                .ok_or(ConversationRepositoryError::Storage)?,
            turn,
            candidate,
            outcomes: Vec::new(),
            rounds: 0,
            replayed: true,
        })
    }

    fn stage(
        &self,
        turn: &GenerationTurn,
        work: &ConversationGenerationClaimedWork,
        sequence: u64,
        status: GenerationTurnStatus,
        operation: &OperationToken,
        now: TimestampMillis,
    ) -> Result<GenerationTurn, ConversationGenerationRunError> {
        Ok(self
            .repository
            .append_event(
                turn.id,
                turn.revision,
                operation,
                GenerationCheckpointEnvelope {
                    turn_id: turn.id,
                    attempt_id: work.attempt_id,
                    job_id: Some(work.handle.id()),
                    correlation_id: None,
                    sequence,
                    event: GenerationCheckpointEvent::Stage { status },
                },
                now,
            )?
            .value)
    }
}

fn attempt_of(
    turn: &GenerationTurn,
    attempt_id: GenerationAttemptId,
) -> Result<GenerationAttempt, ConversationGenerationRunError> {
    turn.attempts
        .iter()
        .find(|attempt| attempt.id == attempt_id)
        .cloned()
        .ok_or(ConversationGenerationRunError::InvalidWork)
}

fn initial_error(
    error: ConversationInitialInferenceError,
    evidence: GenerationUsageEvidence,
) -> ConversationGenerationRunError {
    match error {
        ConversationInitialInferenceError::Pending => {
            ConversationGenerationRunError::Pending { evidence }
        }
        ConversationInitialInferenceError::Cancelled => {
            ConversationGenerationRunError::Cancelled { evidence }
        }
        ConversationInitialInferenceError::Inference(error) => {
            ConversationGenerationRunError::Provider { error, evidence }
        }
        ConversationInitialInferenceError::InvalidOwnership
        | ConversationInitialInferenceError::InvalidModel => {
            ConversationGenerationRunError::InvalidWork
        }
        ConversationInitialInferenceError::Validation(error) => {
            ConversationGenerationRunError::Validation(error)
        }
        ConversationInitialInferenceError::Repository(error) => {
            ConversationGenerationRunError::Repository(error)
        }
        ConversationInitialInferenceError::Artifact(error) => {
            ConversationGenerationRunError::Artifact(error)
        }
    }
}

#[derive(Debug)]
pub enum ConversationGenerationSettledWork {
    Succeeded {
        result: Box<ConversationGenerationRunResult>,
        job: JobSnapshot,
    },
    Failed {
        error: ConversationGenerationRunError,
        job: JobSnapshot,
    },
    Cancelled {
        error: ConversationGenerationRunError,
        job: JobSnapshot,
    },
    Interrupted {
        error: ConversationGenerationRunError,
        job: JobSnapshot,
        child_attempt_id: GenerationAttemptId,
        child_job: Box<JobSnapshot>,
    },
    RetryScheduled {
        error: ConversationGenerationRunError,
        job: JobSnapshot,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum ConversationGenerationDispatchError {
    #[error("conversation generation job operation failed: {0}")]
    Jobs(#[from] StoreError),
    #[error("conversation generation repository operation failed: {0}")]
    Repository(#[from] ConversationRepositoryError),
    #[error("conversation generation usage ledger operation failed: {0}")]
    Usage(#[from] UsageLedgerError),
    #[error("conversation generation claimed work is inconsistent")]
    InvalidWork,
    #[error("conversation generation is only supported for direct conversations")]
    GroupUnsupported,
}

#[derive(Debug)]
pub struct ConversationGenerationDispatchCoordinator<'a, C: ?Sized, J: ?Sized> {
    conversations: &'a C,
    jobs: &'a J,
}

impl<'a, C: ?Sized, J: ?Sized> ConversationGenerationDispatchCoordinator<'a, C, J> {
    #[must_use]
    pub const fn new(conversations: &'a C, jobs: &'a J) -> Self {
        Self {
            conversations,
            jobs,
        }
    }
}

impl<
    C: ConversationRepository + ToolExecutionRepository + JobUsageLedger + UsageLedger + ?Sized,
    J: JobStore + ?Sized,
> ConversationGenerationDispatchCoordinator<'_, C, J>
{
    pub fn admit(
        &self,
        conversation_id: ConversationId,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
        now: TimestampMillis,
    ) -> Result<ConversationGenerationAdmission, ConversationGenerationDispatchError> {
        let aggregate = ConversationReader::get(self.conversations, conversation_id)?;
        if aggregate.conversation.kind.is_group() {
            return Err(ConversationGenerationDispatchError::GroupUnsupported);
        }
        let turn = self.conversations.get_turn(turn_id)?;
        if turn.conversation_id != conversation_id {
            return Err(ConversationGenerationDispatchError::InvalidWork);
        }
        let attempt = turn
            .attempts
            .iter()
            .find(|attempt| attempt.id == attempt_id)
            .cloned()
            .ok_or(ConversationGenerationDispatchError::InvalidWork)?;
        let key = attempt_job_idempotency_key(turn_id, attempt_id);
        if attempt.job_idempotency_key != key {
            return Err(ConversationGenerationDispatchError::InvalidWork);
        }
        if let Some(job_id) = attempt.job_id {
            let job = self
                .jobs
                .get(job_id)?
                .ok_or(ConversationGenerationDispatchError::InvalidWork)?;
            if job.kind != JobKind::ConversationGeneration {
                return Err(ConversationGenerationDispatchError::InvalidWork);
            }
            return Ok(ConversationGenerationAdmission {
                job,
                attempt,
                created: false,
            });
        }
        if !matches!(
            attempt.status,
            GenerationAttemptStatus::Created | GenerationAttemptStatus::Preparing
        ) {
            return Err(ConversationGenerationDispatchError::InvalidWork);
        }
        let subject = JobSubject::new(SubjectKind::Conversation, conversation_id.to_string())
            .map_err(|_| ConversationGenerationDispatchError::InvalidWork)?;
        let created = self.jobs.create_or_get(
            JobSpec::new(
                JobKind::ConversationGeneration,
                subject,
                OutcomeRef::GenerationTurn(turn_id),
            )
            .with_idempotency_key(key)
            .with_resources(vec![
                ResourceClass::Network,
                ResourceClass::ModelLoad,
                ResourceClass::DiskRead,
                ResourceClass::DiskWrite,
                ResourceClass::Cpu,
            ])
            .with_priority(JobPriority::Interactive)
            .with_policies(RecoveryPolicy::Restart, CancellationPolicy::Cooperative),
        )?;
        let attached = self.conversations.attach_attempt_job(
            &AttachAttemptJob {
                conversation_id,
                turn_id,
                attempt_id,
                expected_revision: aggregate.conversation.revision,
                expected_turn_revision: turn.revision,
                operation: operation_token(
                    conversation_id,
                    turn_id,
                    attempt_id,
                    created.job.id,
                    ConversationGenerationOperation::AttachJob,
                ),
                job_id: created.job.id,
            },
            now,
        )?;
        Ok(ConversationGenerationAdmission {
            job: created.job,
            attempt: attached.value,
            created: true,
        })
    }

    pub fn claim(
        &self,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
        worker_id: WorkerId,
        now: TimestampMillis,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Option<ConversationGenerationClaimedWork>, ConversationGenerationDispatchError>
    {
        let turn = self.conversations.get_turn(turn_id)?;
        let attempt = turn
            .attempts
            .iter()
            .find(|attempt| attempt.id == attempt_id)
            .ok_or(ConversationGenerationDispatchError::InvalidWork)?;
        let job_id = attempt
            .job_id
            .ok_or(ConversationGenerationDispatchError::InvalidWork)?;
        let job = self
            .jobs
            .get(job_id)?
            .ok_or(ConversationGenerationDispatchError::InvalidWork)?;
        if job.kind != JobKind::ConversationGeneration {
            return Err(ConversationGenerationDispatchError::InvalidWork);
        }
        let at = now.max(job.updated_at);
        let Some(claim) = self.jobs.claim(job_id, worker_id, at, lease_for, allowed)? else {
            return Ok(None);
        };
        if claim.input_ref != OutcomeRef::GenerationTurn(turn_id) {
            return Err(ConversationGenerationDispatchError::InvalidWork);
        }
        let handle = JobHandle::new(job_id);
        self.jobs.append_and_transition(JobMutation::Start {
            claim: claim.claim.clone(),
            at,
        })?;
        let job = self.jobs.append_and_transition(JobMutation::StageChanged {
            claim: claim.claim.clone(),
            stage: StageSnapshot::new(STAGE_LABEL, false).expect("constant job stage is valid"),
            at,
        })?;
        Ok(Some(ConversationGenerationClaimedWork {
            conversation_id: turn.conversation_id,
            turn_id,
            attempt_id,
            claim,
            handle,
            job,
        }))
    }

    pub fn settle(
        &self,
        work: ConversationGenerationClaimedWork,
        result: Result<ConversationGenerationRunResult, ConversationGenerationRunError>,
        cancellation_reason: CancellationReason,
        now: TimestampMillis,
    ) -> Result<ConversationGenerationSettledWork, ConversationGenerationDispatchError> {
        let job_id = work.claim.claim.job_id;
        if work.handle.id() != job_id
            || work.job.id != job_id
            || work.job.kind != JobKind::ConversationGeneration
            || work.job.state != JobState::Running
            || work.claim.input_ref != OutcomeRef::GenerationTurn(work.turn_id)
        {
            return Err(ConversationGenerationDispatchError::InvalidWork);
        }
        let at = now.max(work.job.updated_at);
        let error = match result {
            Ok(result) => {
                if result.turn.id != work.turn_id || result.candidate.attempt_id != work.attempt_id
                {
                    return Err(ConversationGenerationDispatchError::InvalidWork);
                }
                self.jobs.append_and_transition(JobMutation::Progress {
                    claim: work.claim.claim.clone(),
                    progress: ProgressSnapshot {
                        fraction: Some(
                            FiniteFraction::new(1.0).expect("constant job progress is valid"),
                        ),
                        ..ProgressSnapshot::default()
                    },
                    at,
                })?;
                let job = self.jobs.append_and_transition(JobMutation::Succeed {
                    claim: work.claim.claim,
                    outcome: JobOutcome::Success {
                        result_ref: OutcomeRef::GenerationTurn(work.turn_id),
                    },
                    at,
                })?;
                return Ok(ConversationGenerationSettledWork::Succeeded {
                    result: Box::new(result),
                    job,
                });
            }
            Err(error) => error,
        };
        match error.terminal_failure() {
            Some(ConversationGenerationTerminalFailure::Cancelled) => {
                if self
                    .turn_side(self.cancel_turn(&work, error.evidence(), at))?
                    .is_none()
                {
                    return self.retry(work, error, at);
                }
                self.jobs
                    .append_and_transition(JobMutation::RequestCancellation {
                        id: job_id,
                        reason: cancellation_reason,
                        at,
                    })?;
                self.jobs
                    .append_and_transition(JobMutation::RequestCleanup {
                        claim: work.claim.claim.clone(),
                        at,
                    })?;
                let job = self
                    .jobs
                    .append_and_transition(JobMutation::FinishCancellation {
                        claim: work.claim.claim,
                        at,
                    })?;
                Ok(ConversationGenerationSettledWork::Cancelled { error, job })
            }
            Some(ConversationGenerationTerminalFailure::Failed(code)) => {
                if self
                    .turn_side(self.fail_turn(&work, code, error.evidence(), at))?
                    .is_none()
                {
                    return self.retry(work, error, at);
                }
                let job = self.jobs.append_and_transition(JobMutation::Fail {
                    claim: work.claim.claim,
                    error: job_error(code),
                    at,
                })?;
                Ok(ConversationGenerationSettledWork::Failed { error, job })
            }
            Some(ConversationGenerationTerminalFailure::Interrupted) => {
                let has_rounds = !self
                    .conversations
                    .list_tool_executions(work.conversation_id, work.turn_id, work.attempt_id)?
                    .is_empty();
                if has_rounds {
                    let code = GenerationFailureCode::RecoveryUnavailable;
                    if self
                        .turn_side(self.fail_turn(&work, code, error.evidence(), at))?
                        .is_none()
                    {
                        return self.retry(work, error, at);
                    }
                    let job = self.jobs.append_and_transition(JobMutation::Fail {
                        claim: work.claim.claim,
                        error: job_error(code),
                        at,
                    })?;
                    return Ok(ConversationGenerationSettledWork::Failed { error, job });
                }
                let Some((child_attempt_id, child_job)) =
                    self.turn_side(self.interrupt_and_recover(&work, error.evidence(), at))?
                else {
                    return self.retry(work, error, at);
                };
                let already_linked = self.jobs.get(job_id)?.is_some_and(|job| {
                    job.children
                        .iter()
                        .any(|link| link.child_id == child_job.id)
                });
                if !already_linked {
                    self.jobs.append_and_transition(JobMutation::AttachChild {
                        parent_id: job_id,
                        child: ChildLink {
                            child_id: child_job.id,
                            required: false,
                        },
                        at,
                    })?;
                }
                let job = self.jobs.append_and_transition(JobMutation::Interrupt {
                    claim: work.claim.claim,
                    at,
                })?;
                Ok(ConversationGenerationSettledWork::Interrupted {
                    error,
                    job,
                    child_attempt_id,
                    child_job: Box::new(child_job),
                })
            }
            Some(ConversationGenerationTerminalFailure::Invalid) => {
                let settled = match self.current(&work) {
                    Ok((_, turn, _)) if turn.status == GenerationTurnStatus::Created => {
                        self.cancel_turn(&work, error.evidence(), at)
                    }
                    Ok(_) => {
                        self.fail_turn(&work, GenerationFailureCode::Internal, error.evidence(), at)
                    }
                    Err(error) => Err(error),
                };
                match settled {
                    Ok(())
                    | Err(
                        ConversationGenerationDispatchError::InvalidWork
                        | ConversationGenerationDispatchError::Repository(_)
                        | ConversationGenerationDispatchError::Usage(_),
                    ) => {}
                    Err(error) => return Err(error),
                }
                let job = self.jobs.append_and_transition(JobMutation::Fail {
                    claim: work.claim.claim,
                    error: JobError::new(
                        JobErrorCode::InvalidInput,
                        false,
                        "conversation-generation-invalid-input",
                    )
                    .expect("constant job error is valid"),
                    at,
                })?;
                Ok(ConversationGenerationSettledWork::Failed { error, job })
            }
            None => self.retry(work, error, at),
        }
    }

    fn retry(
        &self,
        work: ConversationGenerationClaimedWork,
        error: ConversationGenerationRunError,
        at: TimestampMillis,
    ) -> Result<ConversationGenerationSettledWork, ConversationGenerationDispatchError> {
        let job = self
            .jobs
            .append_and_transition(JobMutation::RetryScheduled {
                claim: work.claim.claim,
                at,
            })?;
        Ok(ConversationGenerationSettledWork::RetryScheduled { error, job })
    }

    fn turn_side<T>(
        &self,
        result: Result<T, ConversationGenerationDispatchError>,
    ) -> Result<Option<T>, ConversationGenerationDispatchError> {
        match result {
            Ok(value) => Ok(Some(value)),
            Err(
                ConversationGenerationDispatchError::Repository(_)
                | ConversationGenerationDispatchError::Usage(_),
            ) => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn current(
        &self,
        work: &ConversationGenerationClaimedWork,
    ) -> Result<
        (lettuce_types::Revision, GenerationTurn, GenerationAttempt),
        ConversationGenerationDispatchError,
    > {
        let turn = self.conversations.get_turn(work.turn_id)?;
        let aggregate = ConversationReader::get(self.conversations, work.conversation_id)?;
        let attempt = turn
            .attempts
            .iter()
            .find(|attempt| attempt.id == work.attempt_id)
            .cloned()
            .ok_or(ConversationGenerationDispatchError::InvalidWork)?;
        if turn.conversation_id != work.conversation_id || attempt.job_id != Some(work.handle.id())
        {
            return Err(ConversationGenerationDispatchError::InvalidWork);
        }
        Ok((aggregate.conversation.revision, turn, attempt))
    }

    fn attempt_usage_event(
        &self,
        work: &ConversationGenerationClaimedWork,
        turn: &GenerationTurn,
        attempt: &GenerationAttempt,
        evidence: GenerationUsageEvidence,
        outcome: UsageOutcome,
    ) -> Result<UsageEventId, ConversationGenerationDispatchError> {
        if let Some(existing) = self.conversations.get_for_attempt(turn.id, attempt.id)? {
            return Ok(existing.id);
        }
        let dispatch = match evidence {
            GenerationUsageEvidence::Event(id) => return Ok(id),
            GenerationUsageEvidence::Dispatch(id) => self
                .conversations
                .job_usage(work.handle.id())?
                .into_iter()
                .find(|record| record.id == id),
            GenerationUsageEvidence::None => None,
        };
        let (usage, recorded_at, provenance) = match dispatch {
            Some(record) => {
                let usage = match &record.result {
                    Some(JobInferenceUsageResult::Response {
                        usage: Some(usage), ..
                    }) => UsageCounters::Known(usage.clone()),
                    Some(JobInferenceUsageResult::Response { usage: None, .. }) => {
                        UsageCounters::Unavailable(UsageUnavailableReason::ProviderOmitted)
                    }
                    Some(JobInferenceUsageResult::Cancelled) => {
                        UsageCounters::Unavailable(UsageUnavailableReason::CancelledBeforeResponse)
                    }
                    Some(JobInferenceUsageResult::InferenceFailed) | None => {
                        UsageCounters::Unavailable(UsageUnavailableReason::TransportFailed)
                    }
                };
                (
                    usage,
                    record.admitted_at,
                    Some((
                        record.model_profile_id,
                        record.model_revision,
                        record.provider_account_id,
                        record.provider_account_revision,
                    )),
                )
            }
            None => {
                let reason = if outcome == UsageOutcome::Cancelled {
                    UsageUnavailableReason::CancelledBeforeResponse
                } else {
                    UsageUnavailableReason::NotAdmitted
                };
                (
                    UsageCounters::Unavailable(reason),
                    work.job.created_at,
                    turn.resolved_model.as_ref().map(|model| {
                        (
                            model.source_id,
                            model.source_revision,
                            model.provider_account_id,
                            model.provider_account_revision,
                        )
                    }),
                )
            }
        };
        let (model_profile_id, model_revision, provider_account_id, provider_account_revision) =
            match provenance {
                Some((model, model_revision, account, account_revision)) => (
                    Some(model),
                    Some(model_revision),
                    Some(account),
                    Some(account_revision),
                ),
                None => (None, None, None, None),
            };
        Ok(self
            .conversations
            .record(UsageRecord {
                turn_id: turn.id,
                attempt_id: attempt.id,
                outcome,
                usage,
                model_profile_id,
                model_revision,
                provider_account_id,
                provider_account_revision,
                recorded_at,
            })?
            .id)
    }

    fn cancel_turn(
        &self,
        work: &ConversationGenerationClaimedWork,
        evidence: GenerationUsageEvidence,
        at: TimestampMillis,
    ) -> Result<(), ConversationGenerationDispatchError> {
        let (conversation_revision, turn, attempt) = self.current(work)?;
        if is_terminal_attempt(attempt.status) {
            return Ok(());
        }
        let usage_event_id =
            self.attempt_usage_event(work, &turn, &attempt, evidence, UsageOutcome::Cancelled)?;
        let token = |operation| {
            operation_token(
                work.conversation_id,
                work.turn_id,
                work.attempt_id,
                work.handle.id(),
                operation,
            )
        };
        let (conversation_revision, turn_revision) =
            if turn.status == GenerationTurnStatus::CancellationRequested {
                (conversation_revision, turn.revision)
            } else {
                let requested = self.conversations.request_cancellation(
                    &CancelGeneration {
                        conversation_id: work.conversation_id,
                        turn_id: work.turn_id,
                        attempt_id: work.attempt_id,
                        expected_revision: conversation_revision,
                        expected_turn_revision: turn.revision,
                        operation: token(ConversationGenerationOperation::RequestCancellation),
                    },
                    at,
                )?;
                let aggregate = ConversationReader::get(self.conversations, work.conversation_id)?;
                (aggregate.conversation.revision, requested.value.revision)
            };
        self.conversations.settle_cancellation(
            &SettleCancellation {
                conversation_id: work.conversation_id,
                turn_id: work.turn_id,
                attempt_id: work.attempt_id,
                expected_revision: conversation_revision,
                expected_turn_revision: turn_revision,
                operation: token(ConversationGenerationOperation::SettleCancellation),
                usage_event_id,
            },
            at,
        )?;
        Ok(())
    }

    fn fail_turn(
        &self,
        work: &ConversationGenerationClaimedWork,
        code: GenerationFailureCode,
        evidence: GenerationUsageEvidence,
        at: TimestampMillis,
    ) -> Result<(), ConversationGenerationDispatchError> {
        let (conversation_revision, turn, attempt) = self.current(work)?;
        if is_terminal_attempt(attempt.status) {
            return Ok(());
        }
        let usage_event_id =
            self.attempt_usage_event(work, &turn, &attempt, evidence, UsageOutcome::Failed)?;
        self.conversations.fail_generation(
            work.turn_id,
            work.attempt_id,
            conversation_revision,
            turn.revision,
            &operation_token(
                work.conversation_id,
                work.turn_id,
                work.attempt_id,
                work.handle.id(),
                ConversationGenerationOperation::Fail,
            ),
            code,
            usage_event_id,
            at,
        )?;
        Ok(())
    }

    fn interrupt_and_recover(
        &self,
        work: &ConversationGenerationClaimedWork,
        evidence: GenerationUsageEvidence,
        at: TimestampMillis,
    ) -> Result<(GenerationAttemptId, JobSnapshot), ConversationGenerationDispatchError> {
        let token = |operation| {
            operation_token(
                work.conversation_id,
                work.turn_id,
                work.attempt_id,
                work.handle.id(),
                operation,
            )
        };
        let (conversation_revision, turn, attempt) = self.current(work)?;
        if attempt.status != GenerationAttemptStatus::Interrupted {
            if is_terminal_attempt(attempt.status) {
                return Err(ConversationGenerationDispatchError::InvalidWork);
            }
            let usage_event_id = self.attempt_usage_event(
                work,
                &turn,
                &attempt,
                evidence,
                UsageOutcome::Interrupted,
            )?;
            self.conversations.interrupt_generation(
                work.turn_id,
                work.attempt_id,
                conversation_revision,
                turn.revision,
                &token(ConversationGenerationOperation::Interrupt),
                usage_event_id,
                at,
            )?;
        }
        let (conversation_revision, turn, _) = self.current(work)?;
        let child_attempt_id = if turn.status == GenerationTurnStatus::Interrupted {
            self.conversations
                .recover_generation(
                    work.turn_id,
                    work.attempt_id,
                    conversation_revision,
                    turn.revision,
                    &token(ConversationGenerationOperation::Recover),
                    at,
                )?
                .value
                .attempt
                .id
        } else {
            turn.attempts
                .iter()
                .find(|candidate| candidate.parent_attempt_id == Some(work.attempt_id))
                .map(|candidate| candidate.id)
                .ok_or(ConversationGenerationDispatchError::InvalidWork)?
        };
        let admission = self.admit(work.conversation_id, work.turn_id, child_attempt_id, at)?;
        Ok((child_attempt_id, admission.job))
    }
}

const fn is_terminal_attempt(status: GenerationAttemptStatus) -> bool {
    matches!(
        status,
        GenerationAttemptStatus::Succeeded
            | GenerationAttemptStatus::Failed
            | GenerationAttemptStatus::Cancelled
            | GenerationAttemptStatus::Interrupted
    )
}

fn job_error(code: GenerationFailureCode) -> JobError {
    let (job_code, retryable, message) = match code {
        GenerationFailureCode::ProviderUnavailable | GenerationFailureCode::TimedOut => (
            JobErrorCode::ResourceUnavailable,
            true,
            "conversation-generation-provider-unavailable",
        ),
        GenerationFailureCode::ProviderRejected => (
            JobErrorCode::WorkerFailed,
            false,
            "conversation-generation-provider-rejected",
        ),
        GenerationFailureCode::EmptyOutput => (
            JobErrorCode::WorkerFailed,
            false,
            "conversation-generation-empty-output",
        ),
        GenerationFailureCode::InvalidConversation
        | GenerationFailureCode::MissingModel
        | GenerationFailureCode::ContextUnavailable
        | GenerationFailureCode::SpeakerUnavailable => (
            JobErrorCode::InvalidInput,
            false,
            "conversation-generation-invalid-input",
        ),
        GenerationFailureCode::Cancelled => (
            JobErrorCode::Cancelled,
            false,
            "conversation-generation-cancelled",
        ),
        GenerationFailureCode::RecoveryUnavailable => (
            JobErrorCode::WorkerFailed,
            false,
            "conversation-generation-recovery-unavailable",
        ),
        GenerationFailureCode::Internal => (
            JobErrorCode::WorkerFailed,
            false,
            "conversation-generation-internal",
        ),
    };
    JobError::new(job_code, retryable, message).expect("constant job error is valid")
}
