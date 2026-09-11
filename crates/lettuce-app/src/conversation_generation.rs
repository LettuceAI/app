use std::time::Duration;

use lettuce_conversations::{
    ArtifactError, AttachAttemptJob, BeginGeneration, CancelGeneration, ContextAttributions,
    ConversationManager, ConversationReader, ConversationRepository, ConversationRepositoryError,
    ConversationServiceError, FinalizationDraft, GenerationAttempt, GenerationAttemptStatus,
    GenerationCheckpointEnvelope, GenerationCheckpointEvent, GenerationFailureCode,
    GenerationFinalizationResult, GenerationTarget, GenerationTurn, GenerationTurnStatus,
    InferenceOutcome, InferencePort, InferenceRequest, InitialInferenceBinding,
    InitialInferenceRepository, MessageCandidate, MessagePart, ModelSelectionSnapshot,
    OperationKind, OperationToken, PortError, ProviderFailureKind, ProviderNeutralContext,
    ProviderReplayArtifactPort, ResolvedInferenceProfile, SettleCancellation,
    ToolExecutionRepository, ToolExecutionStatus, ToolExecutionTransition, UsageCounters,
    UsageOutcome, UsagePort, UsageRecord, UsageUnavailableReason, ValidationError,
    attempt_job_idempotency_key,
};
use lettuce_inference::{InferenceRuntime, InferenceRuntimeError};
use lettuce_jobs::{
    CancellationPolicy, CancellationReason, ChildLink, Claim, FiniteFraction, IdempotencyKey,
    JobError, JobErrorCode, JobKind, JobMutation, JobOutcome, JobPriority, JobSnapshot, JobSpec,
    JobState, JobStore, JobSubject, OutcomeRef, ProgressSnapshot, RecoveryPolicy,
    ResourceAvailability, ResourceClass, StageSnapshot, StoreError, SubjectKind, WorkerId,
    events::JobEvent,
    handle::{CancellationToken, JobHandle},
};
use lettuce_types::{
    AssetId, ContentHash, ConversationId, GenerationAttemptId, GenerationTurnId, JobId, RequestId,
    TimestampMillis, UsageEventId,
};
use lettuce_usage::{JobInferenceUsageResult, JobUsageLedger, UsageLedger, UsageLedgerError};

use crate::{ConversationInitialInferenceCoordinator, ConversationInitialInferenceError};

const STAGE_LABEL: &str = "conversation-generation";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConversationGenerationOperation {
    AttachJob,
    StagePreparing,
    StageSelectingSpeaker,
    StageRunning,
    ResolveSpeaker,
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
            Self::StageSelectingSpeaker => "stage-selecting-speaker",
            Self::StageRunning => "stage-running",
            Self::ResolveSpeaker => "resolve-speaker",
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

#[derive(Debug, Clone)]
pub struct ConversationGenerationClaimContext {
    pub worker_id: WorkerId,
    pub cancellation: CancellationToken,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConversationGenerationCancellationOutcome {
    Requested {
        job: JobSnapshot,
        live_execution_signalled: bool,
    },
    QueuedCancelled(JobSnapshot),
    AlreadyTerminal(JobSnapshot),
    NotFound,
}

#[derive(Debug, thiserror::Error)]
pub enum ConversationGenerationCancellationError {
    #[error("job storage failed: {0}")]
    Store(#[from] StoreError),
    #[error("conversation storage failed: {0}")]
    Conversation(#[from] ConversationRepositoryError),
    #[error("usage storage failed: {0}")]
    Usage(#[from] UsageLedgerError),
    #[error("inference runtime failed: {0}")]
    Runtime(#[from] InferenceRuntimeError),
    #[error("job is not a conversation generation")]
    WrongJobKind,
    #[error("job does not own a valid conversation generation attempt")]
    InvalidWork,
}

impl<C: ConversationRepository + UsageLedger + ?Sized, J: JobStore + ?Sized>
    ConversationGenerationCancellationCoordinator<'_, C, J>
{
    pub fn cancel(
        &self,
        job_id: JobId,
        reason: CancellationReason,
        at: TimestampMillis,
    ) -> Result<ConversationGenerationCancellationOutcome, ConversationGenerationCancellationError>
    {
        let Some(job) = self.jobs.get(job_id)? else {
            return Ok(ConversationGenerationCancellationOutcome::NotFound);
        };
        if job.kind != JobKind::ConversationGeneration {
            return Err(ConversationGenerationCancellationError::WrongJobKind);
        }
        let events = self.jobs.events_since(job_id, None, 1)?;
        let Some(JobEvent::Created {
            input_ref: OutcomeRef::GenerationTurn(turn_id),
            ..
        }) = events.first().map(|event| &event.event)
        else {
            return Err(ConversationGenerationCancellationError::InvalidWork);
        };
        let turn = ConversationReader::get_turn(self.conversations, *turn_id)?;
        let attempt = turn
            .attempts
            .iter()
            .find(|attempt| attempt.job_id == Some(job_id))
            .cloned()
            .ok_or(ConversationGenerationCancellationError::InvalidWork)?;
        if job.state.is_terminal() {
            return Ok(ConversationGenerationCancellationOutcome::AlreadyTerminal(
                job,
            ));
        }
        let at = at.max(job.updated_at);
        let requested = match self
            .jobs
            .append_and_transition(JobMutation::RequestCancellation {
                id: job_id,
                reason,
                at,
            }) {
            Ok(requested) => requested,
            Err(StoreError::AlreadyTerminal) => {
                let job = self
                    .jobs
                    .get(job_id)?
                    .ok_or(ConversationGenerationCancellationError::InvalidWork)?;
                return Ok(ConversationGenerationCancellationOutcome::AlreadyTerminal(
                    job,
                ));
            }
            Err(error) => return Err(error.into()),
        };
        let live_execution_signalled = self.runtime.request_cancel(job_id)?;
        if requested.claim.is_some() {
            return Ok(ConversationGenerationCancellationOutcome::Requested {
                job: requested,
                live_execution_signalled,
            });
        }
        self.settle_unclaimed_turn(&turn, &attempt, &requested, at)?;
        let job = self
            .jobs
            .append_and_transition(JobMutation::FinishQueuedCancellation {
                id: job_id,
                at: at.max(requested.updated_at),
            })?;
        Ok(ConversationGenerationCancellationOutcome::QueuedCancelled(
            job,
        ))
    }

    fn settle_unclaimed_turn(
        &self,
        turn: &GenerationTurn,
        attempt: &GenerationAttempt,
        job: &JobSnapshot,
        at: TimestampMillis,
    ) -> Result<(), ConversationGenerationCancellationError> {
        if is_terminal_attempt(attempt.status) {
            return Ok(());
        }
        let usage_event_id =
            match UsageLedger::get_for_attempt(self.conversations, turn.id, attempt.id)? {
                Some(event) => event.id,
                None => {
                    let provenance = turn.resolved_model.as_ref();
                    UsageLedger::record(
                        self.conversations,
                        UsageRecord {
                            turn_id: turn.id,
                            attempt_id: attempt.id,
                            outcome: UsageOutcome::Cancelled,
                            usage: UsageCounters::Unavailable(
                                UsageUnavailableReason::CancelledBeforeResponse,
                            ),
                            model_profile_id: provenance.map(|model| model.source_id),
                            model_revision: provenance.map(|model| model.source_revision),
                            provider_account_id: provenance.map(|model| model.provider_account_id),
                            provider_account_revision: provenance
                                .map(|model| model.provider_account_revision),
                            recorded_at: job.created_at,
                        },
                    )?
                    .id
                }
            };
        let aggregate = ConversationReader::get(self.conversations, turn.conversation_id)?;
        let token = |operation| {
            operation_token(turn.conversation_id, turn.id, attempt.id, job.id, operation)
        };
        let (conversation_revision, turn_revision) =
            if turn.status == GenerationTurnStatus::CancellationRequested {
                (aggregate.conversation.revision, turn.revision)
            } else {
                let requested = self.conversations.request_cancellation(
                    &CancelGeneration {
                        conversation_id: turn.conversation_id,
                        turn_id: turn.id,
                        attempt_id: attempt.id,
                        expected_revision: aggregate.conversation.revision,
                        expected_turn_revision: turn.revision,
                        operation: token(ConversationGenerationOperation::RequestCancellation),
                    },
                    at,
                )?;
                let aggregate = ConversationReader::get(self.conversations, turn.conversation_id)?;
                (aggregate.conversation.revision, requested.value.revision)
            };
        self.conversations.settle_cancellation(
            &SettleCancellation {
                conversation_id: turn.conversation_id,
                turn_id: turn.id,
                attempt_id: attempt.id,
                expected_revision: conversation_revision,
                expected_turn_revision: turn_revision,
                operation: token(ConversationGenerationOperation::SettleCancellation),
                usage_event_id,
            },
            at,
        )?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct ConversationGenerationCancellationCoordinator<'a, C: ?Sized, J: ?Sized> {
    conversations: &'a C,
    jobs: &'a J,
    runtime: &'a InferenceRuntime,
}

impl<'a, C: ?Sized, J: ?Sized> ConversationGenerationCancellationCoordinator<'a, C, J> {
    pub const fn new(conversations: &'a C, jobs: &'a J, runtime: &'a InferenceRuntime) -> Self {
        Self {
            conversations,
            jobs,
            runtime,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConversationGenerationInput {
    pub model: ModelSelectionSnapshot,
    pub attributions: ContextAttributions,
    pub profile: ResolvedInferenceProfile,
    pub context: ProviderNeutralContext,
    pub media_grants: Vec<AssetId>,
    pub stream_sink: Option<RequestId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationGenerationRunResult {
    pub turn: GenerationTurn,
    pub candidate: MessageCandidate,
    pub usage_event_id: UsageEventId,
    pub outcomes: Vec<InferenceOutcome>,
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
    #[error("conversation generation input preparation failed: {code:?}")]
    PreparationFailed { code: GenerationFailureCode },
    #[error("conversation generation was cancelled")]
    Cancelled { evidence: GenerationUsageEvidence },
    #[error("conversation generation dispatch is pending and requires attempt recovery")]
    Pending { evidence: GenerationUsageEvidence },
    #[error("conversation generation attempt already failed")]
    AlreadyFailed {
        code: GenerationFailureCode,
        evidence: GenerationUsageEvidence,
    },
    #[error("conversation generation provider failed: {error}")]
    Provider {
        error: PortError,
        evidence: GenerationUsageEvidence,
    },
    #[error("conversation generation usage settlement failed: {error}")]
    Usage {
        error: PortError,
        evidence: GenerationUsageEvidence,
    },
    #[error("conversation generation usage counters overflowed")]
    UsageOverflow,
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
            | Self::Provider { evidence, .. }
            | Self::Usage { evidence, .. } => *evidence,
            _ => GenerationUsageEvidence::None,
        }
    }

    #[must_use]
    pub fn terminal_failure(&self) -> Option<ConversationGenerationTerminalFailure> {
        use ConversationGenerationTerminalFailure as Terminal;
        match self {
            Self::InvalidWork | Self::InvalidInput | Self::Validation(_) => Some(Terminal::Invalid),
            Self::PreparationFailed { code } => Some(Terminal::Failed(*code)),
            Self::Cancelled { .. } => Some(Terminal::Cancelled),
            Self::Pending { .. } => Some(Terminal::Interrupted),
            Self::AlreadyFailed { code, .. } => Some(Terminal::Failed(*code)),
            Self::Provider { error, .. } => Some(provider_terminal(error)),
            Self::Usage { error, .. } => match error {
                PortError::Cancelled => Some(Terminal::Cancelled),
                PortError::Rejected => Some(Terminal::Invalid),
                _ => None,
            },
            Self::UsageOverflow => Some(Terminal::Failed(GenerationFailureCode::Internal)),
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

#[derive(Debug)]
pub struct ConversationGenerationJobRunner<'a, R: ?Sized, I: ?Sized> {
    repository: &'a R,
    inference: &'a I,
}

impl<'a, R: ?Sized, I: ?Sized> ConversationGenerationJobRunner<'a, R, I> {
    #[must_use]
    pub const fn new(repository: &'a R, inference: &'a I) -> Self {
        Self {
            repository,
            inference,
        }
    }
}

impl<
    R: ConversationRepository
        + InitialInferenceRepository
        + ProviderReplayArtifactPort
        + JobUsageLedger
        + UsagePort
        + ?Sized,
    I: InferencePort + ?Sized,
> ConversationGenerationJobRunner<'_, R, I>
{
    pub(crate) fn replay_terminal(
        &self,
        work: &ConversationGenerationClaimedWork,
    ) -> Result<Option<ConversationGenerationRunResult>, ConversationGenerationRunError> {
        let job_id = work.handle.id();
        if work.claim.claim.job_id != job_id || work.job.id != job_id {
            return Err(ConversationGenerationRunError::InvalidWork);
        }
        let turn = self.repository.get_turn(work.turn_id)?;
        let attempt = attempt_of(&turn, work.attempt_id)?;
        if turn.conversation_id != work.conversation_id || attempt.job_id != Some(job_id) {
            return Err(ConversationGenerationRunError::InvalidWork);
        }
        match attempt.status {
            GenerationAttemptStatus::Succeeded => self.replay_succeeded(turn, attempt).map(Some),
            GenerationAttemptStatus::Failed => Err(ConversationGenerationRunError::AlreadyFailed {
                code: attempt.failure.unwrap_or(GenerationFailureCode::Internal),
                evidence: GenerationUsageEvidence::event(attempt.usage_event_id),
            }),
            GenerationAttemptStatus::Cancelled => Err(ConversationGenerationRunError::Cancelled {
                evidence: GenerationUsageEvidence::event(attempt.usage_event_id),
            }),
            GenerationAttemptStatus::Interrupted => Err(ConversationGenerationRunError::Pending {
                evidence: GenerationUsageEvidence::event(attempt.usage_event_id),
            }),
            GenerationAttemptStatus::Created
            | GenerationAttemptStatus::Preparing
            | GenerationAttemptStatus::Running => Ok(None),
        }
    }

    pub(crate) fn replay_succeeded_attempt(
        &self,
        conversation_id: ConversationId,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
        job_id: JobId,
    ) -> Result<ConversationGenerationRunResult, ConversationGenerationRunError> {
        let turn = self.repository.get_turn(turn_id)?;
        let attempt = attempt_of(&turn, attempt_id)?;
        if turn.conversation_id != conversation_id
            || attempt.job_id != Some(job_id)
            || attempt.status != GenerationAttemptStatus::Succeeded
        {
            return Err(ConversationGenerationRunError::InvalidWork);
        }
        self.replay_succeeded(turn, attempt)
    }

    pub async fn run(
        &self,
        work: &ConversationGenerationClaimedWork,
        input: ConversationGenerationInput,
        now: TimestampMillis,
    ) -> Result<ConversationGenerationRunResult, ConversationGenerationRunError> {
        let job_id = work.handle.id();
        if work.claim.claim.job_id != job_id || work.job.id != job_id {
            return Err(ConversationGenerationRunError::InvalidWork);
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
            tools: None,
        };
        let binding = InitialInferenceBinding::from_request(conversation_id, &request)?;
        if turn.status == GenerationTurnStatus::CancellationRequested {
            return Err(ConversationGenerationRunError::Cancelled {
                evidence: self.dispatch_evidence(&binding)?,
            });
        }
        let initial = ConversationInitialInferenceCoordinator::new(self.repository, self.inference)
            .run(conversation_id, &work.handle, request, now)
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
        let mut candidate = outcome.candidates[0].clone();
        if !candidate.tool_calls.is_empty() {
            return Err(ConversationGenerationRunError::Provider {
                error: PortError::Rejected,
                evidence,
            });
        }
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
        let usage = self.attempt_job_usage(work, &attempt)?;
        if work.handle.cancellation_token().is_cancelled() {
            return Err(ConversationGenerationRunError::Cancelled { evidence });
        }
        let aggregate = ConversationReader::get(self.repository, conversation_id)?;
        let turn = self.repository.get_turn(work.turn_id)?;
        if turn.status == GenerationTurnStatus::CancellationRequested {
            return Err(ConversationGenerationRunError::Cancelled { evidence });
        }
        candidate.ordinal = match turn.target {
            GenerationTarget::NewAssistant { .. } => 0,
            GenerationTarget::ExistingCandidate {
                prior_candidate_id, ..
            } => self
                .repository
                .get_candidate(prior_candidate_id)?
                .ordinal
                .checked_add(1)
                .ok_or(ConversationGenerationRunError::InvalidInput)?,
        };
        let attempt = attempt_of(&turn, work.attempt_id)?;
        let finalized = self
            .finalize(
                &attempt,
                &input.profile,
                candidate,
                usage,
                FinalizationContext {
                    conversation_id,
                    expected_conversation_revision: aggregate.conversation.revision,
                    expected_turn_revision: turn.revision,
                    operation: token(ConversationGenerationOperation::Finalize),
                    model: input.model,
                    usage_recorded_at: settled_at,
                    finalized_at: now,
                },
                evidence,
            )
            .await?;
        Ok(ConversationGenerationRunResult {
            turn: finalized.value.turn,
            candidate: finalized.value.candidate,
            usage_event_id: finalized.value.usage_event_id,
            outcomes: vec![outcome],
            replayed: false,
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn finalize(
        &self,
        attempt: &GenerationAttempt,
        profile: &ResolvedInferenceProfile,
        candidate: lettuce_conversations::InferenceCandidate,
        usage: UsageCounters,
        context: FinalizationContext,
        evidence: GenerationUsageEvidence,
    ) -> Result<GenerationFinalizationResult, ConversationGenerationRunError> {
        validate_finalization_identity(attempt, profile, &context.model)?;
        let replaying = self
            .repository
            .operation_record(
                context.conversation_id,
                OperationKind::Finalize,
                &context.operation,
            )?
            .is_some();
        if !replaying {
            let aggregate = ConversationReader::get(self.repository, context.conversation_id)?;
            let turn = self.repository.get_turn(attempt.turn_id)?;
            if aggregate.conversation.revision != context.expected_conversation_revision
                || turn.revision != context.expected_turn_revision
                || !aggregate
                    .branches
                    .iter()
                    .any(|branch| branch.id == turn.branch_id)
                || !turn.attempts.iter().any(|stored| {
                    stored.id == attempt.id
                        && stored.job_id == attempt.job_id
                        && stored.status == attempt.status
                })
            {
                return Err(ConversationGenerationRunError::InvalidWork);
            }
        }
        let usage_event_id = UsagePort::record(
            self.repository,
            UsageRecord {
                turn_id: attempt.turn_id,
                attempt_id: attempt.id,
                outcome: UsageOutcome::Succeeded,
                usage,
                model_profile_id: Some(profile.chat_profile.model_profile_id),
                model_revision: Some(profile.chat_profile.model_revision),
                provider_account_id: Some(profile.chat_profile.provider_account_id),
                provider_account_revision: Some(profile.chat_profile.provider_account_revision),
                recorded_at: context.usage_recorded_at,
            },
        )
        .await
        .map_err(|error| ConversationGenerationRunError::Usage { error, evidence })?;
        Ok(
            ConversationManager::new(self.repository).finalize_generation_ref(
                attempt.turn_id,
                attempt.id,
                context.expected_conversation_revision,
                context.expected_turn_revision,
                &context.operation,
                FinalizationDraft {
                    parts: candidate.parts,
                    ordinal: candidate.ordinal,
                    model: context.model,
                    replay: candidate.provider_replay,
                    outcome: GenerationCheckpointEvent::Completed,
                },
                usage_event_id,
                context.finalized_at,
            )?,
        )
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

    fn attempt_job_usage(
        &self,
        work: &ConversationGenerationClaimedWork,
        attempt: &GenerationAttempt,
    ) -> Result<UsageCounters, ConversationGenerationRunError> {
        let records = self
            .repository
            .job_usage(work.handle.id())
            .map_err(|_| ConversationRepositoryError::Storage)?;
        if attempt.job_id != Some(work.handle.id()) {
            return Err(ConversationGenerationRunError::InvalidWork);
        }
        if records.iter().any(|record| {
            record.job_id != work.handle.id() || record.logical_attempt_id != attempt.id
        }) {
            return Err(ConversationGenerationRunError::InvalidWork);
        }
        if records.iter().any(|record| {
            matches!(
                record.result,
                None | Some(JobInferenceUsageResult::InferenceFailed)
            )
        }) {
            return Ok(UsageCounters::Unavailable(
                UsageUnavailableReason::TransportFailed,
            ));
        }
        if records
            .iter()
            .any(|record| matches!(record.result, Some(JobInferenceUsageResult::Cancelled)))
        {
            return Ok(UsageCounters::Unavailable(
                UsageUnavailableReason::CancelledBeforeResponse,
            ));
        }
        let usages = records
            .iter()
            .map(|record| match &record.result {
                Some(JobInferenceUsageResult::Response { usage, .. }) => usage.clone(),
                _ => None,
            })
            .collect::<Vec<_>>();
        aggregate_usage(&usages).ok_or(ConversationGenerationRunError::UsageOverflow)
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
            replayed: true,
        })
    }

    fn stage(
        &self,
        turn: &GenerationTurn,
        work: &ConversationGenerationClaimedWork,
        status: GenerationTurnStatus,
        operation: &OperationToken,
        now: TimestampMillis,
    ) -> Result<GenerationTurn, ConversationGenerationRunError> {
        let sequence = self
            .repository
            .latest_checkpoint_sequence(turn.id, work.attempt_id)?
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(ConversationRepositoryError::Storage)?;
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

    pub fn schedule(
        &self,
        generation: &BeginGeneration,
        now: TimestampMillis,
    ) -> Result<ConversationGenerationAdmission, ConversationGenerationDispatchError> {
        self.admit(
            generation.conversation.id,
            generation.turn.id,
            generation.attempt.id,
            now,
        )
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
        self.claim_with_cancellation(
            turn_id,
            attempt_id,
            ConversationGenerationClaimContext {
                worker_id,
                cancellation: CancellationToken::new(),
            },
            now,
            lease_for,
            allowed,
        )
    }

    pub fn claim_with_cancellation(
        &self,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
        context: ConversationGenerationClaimContext,
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
        let Some(claim) = self
            .jobs
            .claim(job_id, context.worker_id, at, lease_for, allowed)?
        else {
            return Ok(None);
        };
        if claim.input_ref != OutcomeRef::GenerationTurn(turn_id) {
            return Err(ConversationGenerationDispatchError::InvalidWork);
        }
        let handle = JobHandle::with_cancellation(job_id, context.cancellation);
        if let Err(error) = self.jobs.append_and_transition(JobMutation::Start {
            claim: claim.claim.clone(),
            at,
        }) {
            let job = self
                .jobs
                .get(job_id)?
                .ok_or(ConversationGenerationDispatchError::InvalidWork)?;
            if error == StoreError::IllegalTransition
                && handle.cancellation_token().is_cancelled()
                && job.state == JobState::CancellationRequested
                && job.claim.as_ref() == Some(&claim.claim)
            {
                return Ok(Some(ConversationGenerationClaimedWork {
                    conversation_id: turn.conversation_id,
                    turn_id,
                    attempt_id,
                    claim,
                    handle,
                    job,
                }));
            }
            return Err(error.into());
        }
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
            || !matches!(
                work.job.state,
                JobState::Running | JobState::CancellationRequested
            )
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
        let expected_dispatch = match evidence {
            GenerationUsageEvidence::Event(id) => return Ok(id),
            GenerationUsageEvidence::Dispatch(id) => Some(id),
            GenerationUsageEvidence::None => None,
        };
        let records = self.conversations.job_usage(work.handle.id())?;
        let (usage, recorded_at, provenance) = match records.first() {
            Some(first) => {
                if expected_dispatch.is_some_and(|id| !records.iter().any(|record| record.id == id))
                    || records.iter().any(|record| {
                        record.job_id != work.handle.id()
                            || record.logical_attempt_id != attempt.id
                            || record.model_profile_id != first.model_profile_id
                            || record.model_revision != first.model_revision
                            || record.provider_account_id != first.provider_account_id
                            || record.provider_account_revision != first.provider_account_revision
                    })
                {
                    return Err(ConversationGenerationDispatchError::InvalidWork);
                }
                let usage = if records.iter().any(|record| {
                    matches!(
                        record.result,
                        None | Some(JobInferenceUsageResult::InferenceFailed)
                    )
                }) {
                    UsageCounters::Unavailable(UsageUnavailableReason::TransportFailed)
                } else if records
                    .iter()
                    .any(|record| matches!(record.result, Some(JobInferenceUsageResult::Cancelled)))
                {
                    UsageCounters::Unavailable(UsageUnavailableReason::CancelledBeforeResponse)
                } else {
                    let usages = records
                        .iter()
                        .map(|record| match &record.result {
                            Some(JobInferenceUsageResult::Response { usage, .. }) => usage.clone(),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    aggregate_usage(&usages)
                        .ok_or(ConversationGenerationDispatchError::InvalidWork)?
                };
                (
                    usage,
                    records
                        .iter()
                        .map(|record| record.admitted_at)
                        .min()
                        .unwrap_or(first.admitted_at),
                    Some((
                        first.model_profile_id,
                        first.model_revision,
                        first.provider_account_id,
                        first.provider_account_revision,
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
            let executions = self.conversations.list_tool_executions(
                work.conversation_id,
                work.turn_id,
                work.attempt_id,
            )?;
            let active = executions
                .iter()
                .skip_while(|execution| execution.status == ToolExecutionStatus::Succeeded)
                .collect::<Vec<_>>();
            if !active.is_empty() {
                if active
                    .iter()
                    .all(|execution| execution.status == ToolExecutionStatus::Running)
                {
                    self.conversations.transition_tool_execution_batch(
                        &active
                            .iter()
                            .map(|execution| ToolExecutionTransition {
                                id: execution.id,
                                expected_revision: execution.revision,
                                next: ToolExecutionStatus::Interrupted,
                                output: None,
                                failure: None,
                            })
                            .collect::<Vec<_>>(),
                        at,
                    )?;
                } else if !active
                    .iter()
                    .all(|execution| execution.status == ToolExecutionStatus::Interrupted)
                {
                    return Err(ConversationGenerationDispatchError::InvalidWork);
                }
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct FinalizationContext {
    conversation_id: ConversationId,
    expected_conversation_revision: lettuce_types::Revision,
    expected_turn_revision: lettuce_types::Revision,
    operation: OperationToken,
    model: ModelSelectionSnapshot,
    usage_recorded_at: TimestampMillis,
    finalized_at: TimestampMillis,
}

fn validate_finalization_identity(
    attempt: &GenerationAttempt,
    profile: &ResolvedInferenceProfile,
    model: &ModelSelectionSnapshot,
) -> Result<(), ConversationGenerationRunError> {
    attempt.validate()?;
    model.validate()?;
    let chat = &profile.chat_profile;
    if !matches!(
        attempt.status,
        GenerationAttemptStatus::Running | GenerationAttemptStatus::Succeeded
    ) || model.source_id != chat.model_profile_id
        || model.source_revision != chat.model_revision
        || model.provider_account_id != chat.provider_account_id
        || model.provider_account_revision != chat.provider_account_revision
        || model.provider_protocol != chat.provider_protocol
        || model.external_model_id != chat.external_model_id
    {
        return Err(ConversationGenerationRunError::InvalidWork);
    }
    Ok(())
}

fn aggregate_usage(
    usages: &[Option<lettuce_conversations::InferenceUsage>],
) -> Option<lettuce_conversations::UsageCounters> {
    if usages.is_empty() {
        return Some(lettuce_conversations::UsageCounters::Unavailable(
            lettuce_conversations::UsageUnavailableReason::NotAdmitted,
        ));
    }
    let mut input_tokens = 0u64;
    let mut output_tokens = 0u64;
    let mut cached_input_tokens = Some(0u64);
    let mut reasoning_tokens = Some(0u64);
    let mut cache_write_tokens = Some(0u64);
    let mut web_search_requests = Some(0u64);
    let mut provider_reported_cost = lettuce_conversations::ProviderReportedCost::new(0.0);
    for usage in usages {
        let Some(usage) = usage else {
            return Some(lettuce_conversations::UsageCounters::Unavailable(
                lettuce_conversations::UsageUnavailableReason::ProviderOmitted,
            ));
        };
        cached_input_tokens = cached_input_tokens
            .zip(usage.cached_input_tokens)
            .and_then(|(a, b)| a.checked_add(b));
        reasoning_tokens = reasoning_tokens
            .zip(usage.reasoning_tokens)
            .and_then(|(a, b)| a.checked_add(b));
        cache_write_tokens = cache_write_tokens
            .zip(usage.cache_write_tokens)
            .and_then(|(a, b)| a.checked_add(b));
        web_search_requests = web_search_requests
            .zip(usage.web_search_requests)
            .and_then(|(a, b)| a.checked_add(b));
        provider_reported_cost = provider_reported_cost
            .zip(usage.provider_reported_cost)
            .and_then(|(a, b)| a.checked_add(b));
        input_tokens = input_tokens.checked_add(usage.input_tokens)?;
        output_tokens = output_tokens.checked_add(usage.output_tokens)?;
    }
    Some(lettuce_conversations::UsageCounters::Known(
        lettuce_conversations::InferenceUsage {
            provider_reported_cost,
            cache_write_tokens,
            web_search_requests,
            cached_input_tokens,
            reasoning_tokens,
            input_tokens,
            output_tokens,
        },
    ))
}
