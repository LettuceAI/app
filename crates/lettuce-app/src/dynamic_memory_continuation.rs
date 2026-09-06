use crate::job_inference_usage::{JobInferenceError, run_job_inference};
use lettuce_conversations::{
    ConversationManager, ConversationRepository, FinalizationDraft, GenerationAttempt,
    GenerationAttemptStatus, GenerationCheckpointEvent, GenerationFinalizationResult,
    InferenceCandidate, InferenceOutcome, InferencePort, InferenceRequest, MessagePart,
    ModelSelectionSnapshot, OperationKind, OperationToken, PortError, ProviderReplayArtifactPort,
    ResolvedInferenceProfile, ToolExecution, ToolExecutionOwner, ToolExecutionRepository,
    ToolExecutionStatus, UsageOutcome, UsagePort, UsageRecord, context_with_settled_tool_round,
};
use lettuce_jobs::handle::JobHandle;
use lettuce_memory::{MemoryToolArguments, MemoryToolOutcome, dynamic_memory_tool_request};
use lettuce_types::{ConversationId, Revision, TimestampMillis, UsageEventId};
use lettuce_usage::JobUsageLedger;

pub const MAX_DYNAMIC_MEMORY_TOOL_ROUNDS: u8 = 4;
pub const MAX_DYNAMIC_MEMORY_TOOL_CALLS: u16 = 64;

#[derive(Debug, Clone, PartialEq)]
pub enum DynamicMemoryContinuationResult {
    Done {
        summary: Option<String>,
    },
    NextRound {
        executions: Vec<ToolExecution>,
        continued_request: Box<InferenceRequest>,
        outcome: InferenceOutcome,
    },
    Complete {
        candidate: InferenceCandidate,
        outcome: InferenceOutcome,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum DynamicMemoryContinuationTerminal {
    Done { summary: Option<String> },
    Complete { candidate: InferenceCandidate },
}

#[derive(Debug, Clone, PartialEq)]
pub struct DynamicMemoryContinuationLoopResult {
    pub terminal: DynamicMemoryContinuationTerminal,
    /// Every provider response in request order. Kept intact so the later
    /// usage/finalization boundary can aggregate without inventing counters.
    pub outcomes: Vec<InferenceOutcome>,
    pub(crate) usage: lettuce_conversations::UsageCounters,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicMemoryTerminalContext {
    pub conversation_id: ConversationId,
    pub expected_conversation_revision: Revision,
    pub expected_turn_revision: Revision,
    pub operation: OperationToken,
    pub model: ModelSelectionSnapshot,
    /// This timestamp is part of immutable usage evidence and must be reused
    /// across finalization retries.
    pub usage_recorded_at: TimestampMillis,
    pub finalized_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DynamicMemoryTerminalCommit {
    DerivedMemoryDone {
        summary: Option<String>,
        usage_event_id: UsageEventId,
    },
    ConversationFinalized(Box<GenerationFinalizationResult>),
}

#[derive(Debug)]
pub struct DynamicMemoryTerminalCoordinator<'a, R: ?Sized, U: ?Sized> {
    repository: &'a R,
    usage: &'a U,
}

impl<'a, R: ConversationRepository + ?Sized, U: UsagePort + ?Sized>
    DynamicMemoryTerminalCoordinator<'a, R, U>
{
    #[must_use]
    pub const fn new(repository: &'a R, usage: &'a U) -> Self {
        Self { repository, usage }
    }

    pub async fn commit(
        &self,
        attempt: &GenerationAttempt,
        profile: &ResolvedInferenceProfile,
        result: DynamicMemoryContinuationLoopResult,
        context: DynamicMemoryTerminalContext,
    ) -> Result<DynamicMemoryTerminalCommit, DynamicMemoryTerminalError> {
        validate_terminal_identity(attempt, profile, &context)?;
        validate_terminal_repository(
            self.repository,
            attempt,
            &context,
            matches!(
                result.terminal,
                DynamicMemoryContinuationTerminal::Complete { .. }
            ),
        )?;
        let usage_event_id = self
            .usage
            .record(UsageRecord {
                turn_id: attempt.turn_id,
                attempt_id: attempt.id,
                outcome: UsageOutcome::Succeeded,
                usage: result.usage,
                model_profile_id: Some(profile.chat_profile.model_profile_id),
                model_revision: Some(profile.chat_profile.model_revision),
                provider_account_id: Some(profile.chat_profile.provider_account_id),
                provider_account_revision: Some(profile.chat_profile.provider_account_revision),
                recorded_at: context.usage_recorded_at,
            })
            .await
            .map_err(DynamicMemoryTerminalError::Usage)?;

        match result.terminal {
            DynamicMemoryContinuationTerminal::Done { summary } => {
                Ok(DynamicMemoryTerminalCommit::DerivedMemoryDone {
                    summary,
                    usage_event_id,
                })
            }
            DynamicMemoryContinuationTerminal::Complete { candidate } => {
                let finalized = ConversationManager::new(self.repository).finalize_generation_ref(
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
                )?;
                Ok(DynamicMemoryTerminalCommit::ConversationFinalized(
                    Box::new(finalized),
                ))
            }
        }
    }
}

fn validate_terminal_repository<R: ConversationRepository + ?Sized>(
    repository: &R,
    attempt: &GenerationAttempt,
    context: &DynamicMemoryTerminalContext,
    can_replay_finalization: bool,
) -> Result<(), DynamicMemoryTerminalError> {
    if can_replay_finalization
        && repository
            .operation_record(
                context.conversation_id,
                OperationKind::Finalize,
                &context.operation,
            )?
            .is_some()
    {
        return Ok(());
    }
    let aggregate = repository.get(context.conversation_id)?;
    let turn = repository.get_turn(attempt.turn_id)?;
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
        return Err(DynamicMemoryTerminalError::InvalidIdentity);
    }
    Ok(())
}

#[derive(Debug)]
pub struct DynamicMemoryContinuationCoordinator<'a, R: ?Sized, I: ?Sized> {
    repository: &'a R,
    inference: &'a I,
}

impl<
    'a,
    R: ToolExecutionRepository + ProviderReplayArtifactPort + JobUsageLedger + ?Sized,
    I: InferencePort + ?Sized,
> DynamicMemoryContinuationCoordinator<'a, R, I>
{
    #[must_use]
    pub const fn new(repository: &'a R, inference: &'a I) -> Self {
        Self {
            repository,
            inference,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn continue_after_settled_round(
        &self,
        conversation_id: ConversationId,
        attempt: &GenerationAttempt,
        handle: &JobHandle,
        mut request: InferenceRequest,
        settled_round: &[ToolExecution],
        completed_rounds: u8,
        prior_attempt_tool_calls: u16,
        total_tool_calls: u16,
        at: TimestampMillis,
    ) -> Result<DynamicMemoryContinuationResult, DynamicMemoryContinuationError> {
        validate_ownership(
            conversation_id,
            attempt,
            handle,
            &request,
            completed_rounds,
            prior_attempt_tool_calls,
            total_tool_calls,
            settled_round,
        )?;
        let durable =
            self.repository
                .list_tool_executions(conversation_id, attempt.turn_id, attempt.id)?;
        if durable.len() + usize::from(prior_attempt_tool_calls) != usize::from(total_tool_calls)
            || durable.len() < settled_round.len()
            || durable[..durable.len() - settled_round.len()]
                .iter()
                .any(|execution| execution.status != ToolExecutionStatus::Succeeded)
            || durable[durable.len() - settled_round.len()..] != *settled_round
        {
            return Err(DynamicMemoryContinuationError::InvalidOwnership);
        }
        request.context = context_with_settled_tool_round(&request.context, settled_round)?;

        if let Some(summary) = done_summary(settled_round)? {
            return Ok(DynamicMemoryContinuationResult::Done { summary });
        }
        if handle.cancellation_token().is_cancelled() {
            return Err(DynamicMemoryContinuationError::Cancelled);
        }

        request.validate()?;
        let outcome = run_job_inference(
            self.repository,
            self.inference,
            handle.id(),
            request.clone(),
            at,
        )
        .await
        .map_err(|error| match error {
            JobInferenceError::Provider(error) => DynamicMemoryContinuationError::Inference(error),
            JobInferenceError::Evidence => DynamicMemoryContinuationError::Repository(
                lettuce_conversations::ConversationRepositoryError::Storage,
            ),
        })?;
        if handle.cancellation_token().is_cancelled() {
            cleanup_provider_replays(self.repository, &outcome)?;
            return Err(DynamicMemoryContinuationError::Cancelled);
        }
        if let Err(error) = outcome.validate() {
            cleanup_provider_replays(self.repository, &outcome)?;
            return Err(error.into());
        }
        let candidate = match single_candidate(&outcome) {
            Ok(candidate) => candidate,
            Err(error) => {
                cleanup_provider_replays(self.repository, &outcome)?;
                return Err(error);
            }
        };

        if !candidate.tool_calls.is_empty() {
            if !candidate.parts.is_empty()
                && (candidate.provider_replay.is_none()
                    || candidate
                        .parts
                        .iter()
                        .any(|part| !matches!(part, MessagePart::ReasoningSummary { .. })))
            {
                cleanup_provider_replays(self.repository, &outcome)?;
                return Err(DynamicMemoryContinuationError::MixedToolAndContent);
            }
            if candidate
                .tool_calls
                .iter()
                .any(|call| call.provider_replay.as_ref() != candidate.provider_replay.as_ref())
            {
                cleanup_provider_replays(self.repository, &outcome)?;
                return Err(DynamicMemoryContinuationError::InvalidSignedReplay);
            }
            let next_total = total_tool_calls
                .checked_add(
                    u16::try_from(candidate.tool_calls.len())
                        .map_err(|_| DynamicMemoryContinuationError::ToolBudgetExceeded)?,
                )
                .ok_or(DynamicMemoryContinuationError::ToolBudgetExceeded)?;
            if completed_rounds >= MAX_DYNAMIC_MEMORY_TOOL_ROUNDS
                || next_total > MAX_DYNAMIC_MEMORY_TOOL_CALLS
            {
                cleanup_provider_replays(self.repository, &outcome)?;
                return Err(DynamicMemoryContinuationError::ToolBudgetExceeded);
            }
            let executions = match ConversationManager::new(self.repository)
                .request_tool_executions(
                    ToolExecutionOwner {
                        conversation_id,
                        turn_id: attempt.turn_id,
                        attempt_id: attempt.id,
                    },
                    request
                        .tools
                        .as_ref()
                        .ok_or(DynamicMemoryContinuationError::InvalidRequest)?,
                    candidate.tool_calls.clone(),
                    at,
                ) {
                Ok(executions) => executions,
                Err(error) => {
                    cleanup_provider_replays(self.repository, &outcome)?;
                    return Err(error.into());
                }
            };
            return Ok(DynamicMemoryContinuationResult::NextRound {
                executions,
                continued_request: Box::new(request),
                outcome,
            });
        }

        if !candidate
            .parts
            .iter()
            .any(|part| matches!(part, MessagePart::Text { text } if !text.trim().is_empty()))
        {
            cleanup_provider_replays(self.repository, &outcome)?;
            return Err(DynamicMemoryContinuationError::EmptyCompletion);
        }
        Ok(DynamicMemoryContinuationResult::Complete {
            candidate: candidate.clone(),
            outcome,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn continue_until_terminal<F>(
        &self,
        conversation_id: ConversationId,
        attempt: &GenerationAttempt,
        handle: &JobHandle,
        mut request: InferenceRequest,
        initial_settled_round: Vec<ToolExecution>,
        mut outcomes: Vec<InferenceOutcome>,
        mut completed_rounds: u8,
        prior_attempt_tool_calls: u16,
        mut total_tool_calls: u16,
        at: TimestampMillis,
        mut execute_round: F,
    ) -> Result<DynamicMemoryContinuationLoopResult, DynamicMemoryContinuationError>
    where
        F: FnMut(
            &[ToolExecution],
            &JobHandle,
            TimestampMillis,
        ) -> Result<
            crate::DynamicMemoryRoundResult,
            crate::DynamicMemoryRoundExecutionError,
        >,
    {
        let mut settled_round = initial_settled_round;
        loop {
            match self
                .continue_after_settled_round(
                    conversation_id,
                    attempt,
                    handle,
                    request,
                    &settled_round,
                    completed_rounds,
                    prior_attempt_tool_calls,
                    total_tool_calls,
                    at,
                )
                .await?
            {
                DynamicMemoryContinuationResult::Done { summary } => {
                    let usage = aggregate_inference_usage(&outcomes)?;
                    return Ok(DynamicMemoryContinuationLoopResult {
                        terminal: DynamicMemoryContinuationTerminal::Done { summary },
                        outcomes,
                        usage,
                    });
                }
                DynamicMemoryContinuationResult::Complete { candidate, outcome } => {
                    outcomes.push(outcome);
                    let usage = aggregate_inference_usage(&outcomes)?;
                    return Ok(DynamicMemoryContinuationLoopResult {
                        terminal: DynamicMemoryContinuationTerminal::Complete { candidate },
                        outcomes,
                        usage,
                    });
                }
                DynamicMemoryContinuationResult::NextRound {
                    executions,
                    continued_request,
                    outcome,
                } => {
                    outcomes.push(outcome);
                    total_tool_calls = total_tool_calls
                        .checked_add(
                            u16::try_from(executions.len())
                                .map_err(|_| DynamicMemoryContinuationError::ToolBudgetExceeded)?,
                        )
                        .ok_or(DynamicMemoryContinuationError::ToolBudgetExceeded)?;
                    completed_rounds = completed_rounds
                        .checked_add(1)
                        .ok_or(DynamicMemoryContinuationError::ToolBudgetExceeded)?;
                    let validated = validate_admitted_round(self.repository, &executions, at)?;
                    let result = execute_round(&validated, handle, at)?;
                    if result.settled_executions.len() != validated.len()
                        || result.settled_executions.iter().zip(&validated).any(
                            |(settled, admitted)| {
                                settled.id != admitted.id
                                    || settled.status != ToolExecutionStatus::Succeeded
                            },
                        )
                    {
                        return Err(DynamicMemoryContinuationError::InvalidSettledRound);
                    }
                    settled_round = result.settled_executions;
                    request = *continued_request;
                }
            }
        }
    }
}

fn cleanup_provider_replays<A: ProviderReplayArtifactPort + ?Sized>(
    artifacts: &A,
    outcome: &InferenceOutcome,
) -> Result<(), DynamicMemoryContinuationError> {
    let mut artifact_ids = std::collections::BTreeSet::new();
    for candidate in &outcome.candidates {
        if let Some(reference) = &candidate.provider_replay {
            artifact_ids.insert(reference.artifact_id);
        }
        artifact_ids.extend(
            candidate
                .tool_calls
                .iter()
                .filter_map(|call| call.provider_replay.as_ref())
                .map(|reference| reference.artifact_id),
        );
    }
    for artifact_id in artifact_ids {
        artifacts.cleanup_orphan_provider_replay(artifact_id)?;
    }
    Ok(())
}

fn validate_admitted_round<R: ToolExecutionRepository + ?Sized>(
    repository: &R,
    executions: &[ToolExecution],
    at: TimestampMillis,
) -> Result<Vec<ToolExecution>, DynamicMemoryContinuationError> {
    if executions.is_empty()
        || executions
            .iter()
            .any(|execution| execution.status != ToolExecutionStatus::Requested)
    {
        return Err(DynamicMemoryContinuationError::InvalidAdmittedRound);
    }
    for execution in executions {
        MemoryToolArguments::parse(&execution.definition_name, &execution.arguments)?;
    }
    repository
        .transition_tool_execution_batch(
            &executions
                .iter()
                .map(|execution| lettuce_conversations::ToolExecutionTransition {
                    id: execution.id,
                    expected_revision: execution.revision,
                    next: ToolExecutionStatus::Validated,
                    output: None,
                    failure: None,
                })
                .collect::<Vec<_>>(),
            at,
        )
        .map_err(Into::into)
}

pub fn aggregate_inference_usage(
    outcomes: &[InferenceOutcome],
) -> Result<lettuce_conversations::UsageCounters, DynamicMemoryContinuationError> {
    aggregate_usage(
        &outcomes
            .iter()
            .map(|outcome| outcome.usage.clone())
            .collect::<Vec<_>>(),
    )
}

pub(crate) fn aggregate_usage(
    usages: &[Option<lettuce_conversations::InferenceUsage>],
) -> Result<lettuce_conversations::UsageCounters, DynamicMemoryContinuationError> {
    if usages.is_empty() {
        return Ok(lettuce_conversations::UsageCounters::Unavailable(
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
            return Ok(lettuce_conversations::UsageCounters::Unavailable(
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
        input_tokens = input_tokens
            .checked_add(usage.input_tokens)
            .ok_or(DynamicMemoryContinuationError::UsageOverflow)?;
        output_tokens = output_tokens
            .checked_add(usage.output_tokens)
            .ok_or(DynamicMemoryContinuationError::UsageOverflow)?;
    }
    Ok(lettuce_conversations::UsageCounters::Known(
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

fn validate_terminal_identity(
    attempt: &GenerationAttempt,
    profile: &ResolvedInferenceProfile,
    context: &DynamicMemoryTerminalContext,
) -> Result<(), DynamicMemoryTerminalError> {
    attempt.validate()?;
    context.model.validate()?;
    let chat = &profile.chat_profile;
    if !matches!(
        attempt.status,
        GenerationAttemptStatus::Running | GenerationAttemptStatus::Succeeded
    ) || context.model.source_id != chat.model_profile_id
        || context.model.source_revision != chat.model_revision
        || context.model.provider_account_id != chat.provider_account_id
        || context.model.provider_account_revision != chat.provider_account_revision
        || context.model.provider_protocol != chat.provider_protocol
        || context.model.external_model_id != chat.external_model_id
    {
        return Err(DynamicMemoryTerminalError::InvalidIdentity);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_ownership(
    conversation_id: ConversationId,
    attempt: &GenerationAttempt,
    handle: &JobHandle,
    request: &InferenceRequest,
    completed_rounds: u8,
    prior_attempt_tool_calls: u16,
    total_tool_calls: u16,
    settled_round: &[ToolExecution],
) -> Result<(), DynamicMemoryContinuationError> {
    attempt.validate()?;
    if attempt.status != GenerationAttemptStatus::Running
        || attempt.job_id != Some(handle.id())
        || request.turn_id != attempt.turn_id
        || request.attempt_id != attempt.id
        || request.cancellation != Some(handle.id())
        || completed_rounds == 0
        || completed_rounds > MAX_DYNAMIC_MEMORY_TOOL_ROUNDS
        || total_tool_calls == 0
        || total_tool_calls > MAX_DYNAMIC_MEMORY_TOOL_CALLS
        || prior_attempt_tool_calls >= total_tool_calls
        || usize::from(total_tool_calls) < settled_round.len()
    {
        return Err(DynamicMemoryContinuationError::InvalidOwnership);
    }
    if request.tools.as_ref() != Some(&dynamic_memory_tool_request()) {
        return Err(DynamicMemoryContinuationError::InvalidRequest);
    }
    if settled_round.iter().any(|execution| {
        execution.conversation_id != conversation_id
            || execution.turn_id != attempt.turn_id
            || execution.attempt_id != attempt.id
            || execution.status != ToolExecutionStatus::Succeeded
    }) {
        return Err(DynamicMemoryContinuationError::InvalidOwnership);
    }
    Ok(())
}

fn done_summary(
    settled_round: &[ToolExecution],
) -> Result<Option<Option<String>>, DynamicMemoryContinuationError> {
    let mut summary = None;
    for execution in settled_round {
        if execution.definition_name != "done" {
            continue;
        }
        let output = execution
            .output
            .as_ref()
            .ok_or(DynamicMemoryContinuationError::InvalidDoneResult)?;
        let outcome = serde_json::from_value::<MemoryToolOutcome>(output.value.clone())
            .map_err(|_| DynamicMemoryContinuationError::InvalidDoneResult)?;
        let MemoryToolOutcome::Done {
            summary: done_summary,
        } = outcome
        else {
            return Err(DynamicMemoryContinuationError::InvalidDoneResult);
        };
        if summary.replace(done_summary).is_some() {
            return Err(DynamicMemoryContinuationError::InvalidDoneResult);
        }
    }
    Ok(summary)
}

fn single_candidate(
    outcome: &InferenceOutcome,
) -> Result<&InferenceCandidate, DynamicMemoryContinuationError> {
    if outcome.candidates.len() != 1 {
        return Err(DynamicMemoryContinuationError::MultipleCandidates);
    }
    match outcome.finish_reason {
        lettuce_conversations::FinishReason::Stop | lettuce_conversations::FinishReason::Length => {
            Ok(&outcome.candidates[0])
        }
        lettuce_conversations::FinishReason::Cancelled => {
            Err(DynamicMemoryContinuationError::Cancelled)
        }
        lettuce_conversations::FinishReason::Error => {
            Err(DynamicMemoryContinuationError::ProviderFailed)
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DynamicMemoryContinuationError {
    #[error("dynamic-memory continuation ownership is invalid")]
    InvalidOwnership,
    #[error("dynamic-memory continuation request is invalid")]
    InvalidRequest,
    #[error("dynamic-memory continuation was cancelled")]
    Cancelled,
    #[error("dynamic-memory provider continuation failed")]
    ProviderFailed,
    #[error("dynamic-memory continuation returned multiple candidates")]
    MultipleCandidates,
    #[error("dynamic-memory continuation mixed tool calls with content")]
    MixedToolAndContent,
    #[error("dynamic-memory continuation signed replay identity is invalid")]
    InvalidSignedReplay,
    #[error("dynamic-memory continuation exceeded its tool budget")]
    ToolBudgetExceeded,
    #[error("dynamic-memory continuation returned no usable text")]
    EmptyCompletion,
    #[error("dynamic-memory done result is invalid")]
    InvalidDoneResult,
    #[error("dynamic-memory continuation handler returned an invalid settled round")]
    InvalidSettledRound,
    #[error("dynamic-memory continuation admitted an invalid tool round")]
    InvalidAdmittedRound,
    #[error("dynamic-memory continuation usage counters overflowed")]
    UsageOverflow,
    #[error("dynamic-memory continuation contract is invalid: {0}")]
    Validation(#[from] lettuce_conversations::ValidationError),
    #[error("dynamic-memory continuation inference failed: {0}")]
    Inference(#[from] PortError),
    #[error("dynamic-memory continuation persistence failed: {0}")]
    Conversation(#[from] lettuce_conversations::ConversationServiceError),
    #[error("dynamic-memory continuation repository failed: {0}")]
    Repository(#[from] lettuce_conversations::ConversationRepositoryError),
    #[error("dynamic-memory continuation round execution failed: {0}")]
    RoundExecution(#[from] crate::DynamicMemoryRoundExecutionError),
    #[error("dynamic-memory continuation tool call is invalid: {0}")]
    Tool(#[from] lettuce_memory::MemoryToolError),
    #[error("dynamic-memory continuation replay artifact failed: {0}")]
    Artifact(#[from] lettuce_conversations::ArtifactError),
}

#[derive(Debug, thiserror::Error)]
pub enum DynamicMemoryTerminalError {
    #[error("dynamic-memory terminal model or attempt identity is invalid")]
    InvalidIdentity,
    #[error("dynamic-memory terminal usage recording failed: {0}")]
    Usage(PortError),
    #[error("dynamic-memory terminal continuation is invalid: {0}")]
    Continuation(#[from] DynamicMemoryContinuationError),
    #[error("dynamic-memory terminal contract is invalid: {0}")]
    Validation(#[from] lettuce_conversations::ValidationError),
    #[error("dynamic-memory terminal conversation finalization failed: {0}")]
    Conversation(#[from] lettuce_conversations::ConversationServiceError),
    #[error("dynamic-memory terminal repository validation failed: {0}")]
    Repository(#[from] lettuce_conversations::ConversationRepositoryError),
}

#[cfg(test)]
mod tests {
    use lettuce_conversations::{
        ArtifactCodec, ArtifactRetention, ConversationArtifactStore, FinishReason, InferenceUsage,
        InferenceWarningCode, ProtectedArtifactBytes, ProviderReplayArtifactPort,
        ReplayArtifactDraft, ToolExecutionOwner, ToolOutput,
    };
    use lettuce_memory::MemoryToolArguments;
    use lettuce_types::{GenerationAttemptId, GenerationTurnId, ToolExecutionId};
    use serde_json::json;

    use super::*;

    fn settled_done(summary: Option<&str>) -> ToolExecution {
        let owner = ToolExecutionOwner {
            conversation_id: ConversationId::new(),
            turn_id: GenerationTurnId::new(),
            attempt_id: GenerationAttemptId::new(),
        };
        let definition = dynamic_memory_tool_request()
            .definitions
            .into_iter()
            .find(|definition| definition.name == "done")
            .expect("done definition");
        let requested = ToolExecution::requested(
            ToolExecutionId::new(),
            owner,
            0,
            &definition,
            lettuce_conversations::ProposedToolCall {
                provider_call_id: Some("done-1".to_owned()),
                name: "done".to_owned(),
                arguments: match summary {
                    Some(summary) => json!({"summary": summary}),
                    None => json!({}),
                },
                raw_arguments: None,
                provider_replay: None,
            },
            TimestampMillis::new(1),
        )
        .expect("requested");
        let validated = requested
            .transition(
                ToolExecutionStatus::Validated,
                None,
                None,
                TimestampMillis::new(2),
            )
            .expect("validated");
        let running = validated
            .transition(
                ToolExecutionStatus::Running,
                None,
                None,
                TimestampMillis::new(3),
            )
            .expect("running");
        running
            .transition(
                ToolExecutionStatus::Succeeded,
                Some(ToolOutput {
                    value: serde_json::to_value(MemoryToolOutcome::Done {
                        summary: summary.map(str::to_owned),
                    })
                    .expect("outcome"),
                    is_error: false,
                }),
                None,
                TimestampMillis::new(4),
            )
            .expect("succeeded")
    }

    #[test]
    fn done_result_stops_provider_continuation_with_exact_summary() {
        assert_eq!(
            done_summary(&[settled_done(Some("finished"))]).expect("done"),
            Some(Some("finished".to_owned()))
        );
        assert_eq!(
            done_summary(&[settled_done(None)]).expect("done"),
            Some(None)
        );
    }

    #[test]
    fn malformed_done_result_fails_closed() {
        let mut execution = settled_done(None);
        execution.output = Some(ToolOutput {
            value: serde_json::to_value(MemoryToolArguments::Done { summary: None })
                .expect("arguments"),
            is_error: false,
        });
        assert!(matches!(
            done_summary(&[execution]),
            Err(DynamicMemoryContinuationError::InvalidDoneResult)
        ));
    }

    #[test]
    fn continuation_requires_one_successful_provider_candidate() {
        let outcome = InferenceOutcome {
            provider_response_id: None,
            candidates: vec![InferenceCandidate {
                ordinal: 0,
                parts: vec![MessagePart::Text {
                    text: "complete".to_owned(),
                }],
                tool_calls: vec![],
                provider_replay: None,
            }],
            usage: Some(InferenceUsage {
                provider_reported_cost: None,
                cache_write_tokens: None,
                web_search_requests: None,
                cached_input_tokens: None,
                reasoning_tokens: None,
                input_tokens: 4,
                output_tokens: 1,
            }),
            finish_reason: FinishReason::Stop,
            provider_finish_reason: None,
            provider_request_id: None,
            warning_codes: Vec::<InferenceWarningCode>::new(),
        };
        assert_eq!(single_candidate(&outcome).expect("candidate").ordinal, 0);

        let mut failed = outcome;
        failed.finish_reason = FinishReason::Error;
        assert!(matches!(
            single_candidate(&failed),
            Err(DynamicMemoryContinuationError::ProviderFailed)
        ));
    }

    #[test]
    fn rejected_provider_outcome_cleans_one_shared_replay_artifact() {
        let database = lettuce_database::Database::open_in_memory().expect("database");
        let bytes = ProtectedArtifactBytes::new(b"signed replay".to_vec()).expect("bytes");
        let reference = database
            .stage_provider_replay(ReplayArtifactDraft {
                artifact_id: lettuce_types::ReplayArtifactId::new(),
                digest: bytes.digest(),
                schema_version: 1,
                byte_size: u64::try_from(bytes.len()).expect("size"),
                codec: ArtifactCodec::Json,
                retention: ArtifactRetention::Conversation,
                bytes,
            })
            .expect("stage replay");
        let outcome = InferenceOutcome {
            provider_response_id: None,
            candidates: vec![InferenceCandidate {
                ordinal: 0,
                parts: Vec::new(),
                tool_calls: vec![lettuce_conversations::ProposedToolCall {
                    provider_call_id: Some("toolu-1".to_owned()),
                    name: "done".to_owned(),
                    arguments: serde_json::json!({}),
                    raw_arguments: None,
                    provider_replay: Some(reference.clone()),
                }],
                provider_replay: Some(reference.clone()),
            }],
            usage: None,
            finish_reason: FinishReason::Stop,
            provider_finish_reason: Some("tool_use".to_owned()),
            provider_request_id: None,
            warning_codes: Vec::new(),
        };

        cleanup_provider_replays(&database, &outcome).expect("cleanup");
        assert_eq!(
            database.verify_replay(&reference),
            Err(lettuce_conversations::ArtifactError::NotFound)
        );
    }

    #[test]
    fn continuation_limits_are_small_and_explicit() {
        assert_eq!(MAX_DYNAMIC_MEMORY_TOOL_ROUNDS, 4);
        assert_eq!(MAX_DYNAMIC_MEMORY_TOOL_CALLS, 64);
    }

    #[test]
    fn usage_aggregation_never_invents_missing_provider_counters() {
        let outcome = |usage| InferenceOutcome {
            provider_response_id: None,
            candidates: vec![InferenceCandidate {
                ordinal: 0,
                parts: vec![MessagePart::Text {
                    text: "ok".to_owned(),
                }],
                tool_calls: vec![],
                provider_replay: None,
            }],
            usage,
            finish_reason: FinishReason::Stop,
            provider_finish_reason: None,
            provider_request_id: None,
            warning_codes: vec![],
        };
        assert_eq!(
            aggregate_inference_usage(&[
                outcome(Some(InferenceUsage {
                    provider_reported_cost: lettuce_conversations::ProviderReportedCost::new(0.125),
                    cache_write_tokens: Some(3),
                    web_search_requests: Some(0),
                    cached_input_tokens: Some(2),
                    reasoning_tokens: Some(1),
                    input_tokens: 10,
                    output_tokens: 2,
                })),
                outcome(Some(InferenceUsage {
                    provider_reported_cost: lettuce_conversations::ProviderReportedCost::new(0.25),
                    cache_write_tokens: Some(2),
                    web_search_requests: None,
                    cached_input_tokens: Some(0),
                    reasoning_tokens: None,
                    input_tokens: 7,
                    output_tokens: 3,
                })),
            ])
            .expect("aggregate"),
            lettuce_conversations::UsageCounters::Known(InferenceUsage {
                provider_reported_cost: lettuce_conversations::ProviderReportedCost::new(0.375),
                cache_write_tokens: Some(5),
                web_search_requests: None,
                cached_input_tokens: Some(2),
                reasoning_tokens: None,
                input_tokens: 17,
                output_tokens: 5,
            })
        );
        assert_eq!(
            aggregate_inference_usage(&[outcome(None)]).expect("unavailable"),
            lettuce_conversations::UsageCounters::Unavailable(
                lettuce_conversations::UsageUnavailableReason::ProviderOmitted
            )
        );
    }
}
