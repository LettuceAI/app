use lettuce_conversations::{
    ContextAttributions, ContextBudgetReport, FinishReason, GenerationOperation, InferenceOutcome,
    InferencePort, InferenceRequest, MessagePart, MessageRole, PortError, ProviderContextPart,
    ProviderNeutralContext, ProviderNeutralMessage, ProviderReplayArtifactPort,
    ResolvedInferenceProfile, ToolPolicy, TranscriptToolCall, TranscriptToolResult, UsageCounters,
    UsageUnavailableReason,
};
use lettuce_creation::{
    AdmittedCreationToolCall, CreationAttemptFailureCode, CreationAttemptOwner,
    CreationAttemptRecovery, CreationAttemptRepository, CreationAttemptStatus,
    CreationAttemptSuccessSettlement, CreationInferenceAttempt, CreationInferenceRound,
    CreationProposal, CreationRepositoryError, CreationRoundFinishReason,
    CreationTurnAttemptAdmission, CreationWorkflow, CreationWorkflowRepository,
    MAX_CREATION_INFERENCE_ROUNDS, NewCreationAttemptRecovery, NewCreationInferenceRound,
    NewCreationToolCall, NewCreationTurnAttempt, creation_inference_profile_fingerprint,
    reduce_creation_tool_calls,
};
use lettuce_jobs::handle::JobHandle;
use lettuce_types::{
    CreationProposalId, CreationTurnId, CreationWorkflowId, GenerationAttemptId, GenerationTurnId,
    RequestId, Revision, TimestampMillis,
};

#[derive(Debug, Clone, PartialEq)]
pub struct CreationTurnDispatchRequest {
    pub workflow_id: CreationWorkflowId,
    pub expected_workflow_revision: Revision,
    pub base_proposal_id: CreationProposalId,
    pub turn_id: CreationTurnId,
    pub attempt_id: GenerationAttemptId,
    pub planned_proposal_id: CreationProposalId,
    pub user_message: String,
    pub profile: ResolvedInferenceProfile,
    pub now: TimestampMillis,
}

pub fn admit_creation_turn_dispatch<R: CreationAttemptRepository + ?Sized>(
    repository: &R,
    request: CreationTurnDispatchRequest,
    handle: &JobHandle,
) -> Result<CreationTurnAttemptAdmission, CreationContinuationError> {
    let profile_fingerprint = creation_inference_profile_fingerprint(&request.profile)
        .map_err(|_| CreationContinuationError::InvalidProfile)?;
    repository
        .admit_creation_turn_attempt(NewCreationTurnAttempt {
            workflow_id: request.workflow_id,
            expected_workflow_revision: request.expected_workflow_revision,
            base_proposal_id: request.base_proposal_id,
            turn_id: request.turn_id,
            attempt_id: request.attempt_id,
            planned_proposal_id: request.planned_proposal_id,
            user_message: request.user_message,
            job_id: handle.id(),
            profile_fingerprint,
            now: request.now,
        })
        .map_err(Into::into)
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreationRecoveryDispatchRequest {
    pub workflow_id: CreationWorkflowId,
    pub turn_id: CreationTurnId,
    pub parent_attempt_id: GenerationAttemptId,
    pub child_attempt_id: GenerationAttemptId,
    pub planned_proposal_id: CreationProposalId,
    pub profile: ResolvedInferenceProfile,
    pub now: TimestampMillis,
}

pub fn recover_creation_dispatch<R: CreationAttemptRepository + ?Sized>(
    repository: &R,
    request: CreationRecoveryDispatchRequest,
    child_handle: &JobHandle,
) -> Result<CreationAttemptRecovery, CreationContinuationError> {
    let profile_fingerprint = creation_inference_profile_fingerprint(&request.profile)
        .map_err(|_| CreationContinuationError::InvalidProfile)?;
    repository
        .recover_creation_attempt(NewCreationAttemptRecovery {
            owner: CreationAttemptOwner {
                workflow_id: request.workflow_id,
                turn_id: request.turn_id,
            },
            parent_attempt_id: request.parent_attempt_id,
            child_attempt_id: request.child_attempt_id,
            planned_proposal_id: request.planned_proposal_id,
            job_id: child_handle.id(),
            profile_fingerprint,
            now: request.now,
        })
        .map_err(Into::into)
}

#[derive(Debug)]
pub struct CreationContinuationCoordinator<'a, R: ?Sized, I: ?Sized> {
    repository: &'a R,
    inference: &'a I,
}

impl<
    'a,
    R: CreationWorkflowRepository + CreationAttemptRepository + ProviderReplayArtifactPort,
    I: InferencePort + ?Sized,
> CreationContinuationCoordinator<'a, R, I>
{
    #[must_use]
    pub const fn new(repository: &'a R, inference: &'a I) -> Self {
        Self {
            repository,
            inference,
        }
    }

    pub async fn run(
        &self,
        attempt_id: GenerationAttemptId,
        profile: ResolvedInferenceProfile,
        handle: &JobHandle,
        stream_sink: Option<RequestId>,
        now: TimestampMillis,
    ) -> Result<CreationContinuationResult, CreationContinuationError> {
        let mut attempt = self.repository.load_creation_attempt(attempt_id)?;
        if attempt.job_id != handle.id() {
            return Err(CreationContinuationError::InvalidJobOwnership);
        }
        if creation_inference_profile_fingerprint(&profile)
            .map_err(|_| CreationContinuationError::InvalidProfile)?
            != attempt.profile_fingerprint
        {
            return Err(CreationContinuationError::InvalidProfile);
        }
        if attempt.status == CreationAttemptStatus::Created {
            attempt = self.repository.transition_creation_attempt(
                attempt.id,
                attempt.revision,
                CreationAttemptStatus::Running,
                None,
                now,
            )?;
        }
        if attempt.status == CreationAttemptStatus::Succeeded {
            return self.reconstruct_completed(attempt);
        }
        if attempt.status != CreationAttemptStatus::Running {
            return Err(CreationContinuationError::AttemptNotRunnable);
        }
        let owner = CreationAttemptOwner {
            workflow_id: attempt.workflow_id,
            turn_id: attempt.turn_id,
        };
        let workflow = self.repository.load_workflow(attempt.workflow_id)?;
        let turn = self.repository.load_turn(attempt.turn_id)?;
        let base = self.repository.load_proposal(attempt.base_proposal_id)?;
        if turn.workflow_id != attempt.workflow_id
            || turn.base_proposal_id != attempt.base_proposal_id
            || base.stage != attempt.stage
            || base.draft.kind() != attempt.target
            || (workflow.current_proposal_id != attempt.base_proposal_id
                && workflow.current_proposal_id != attempt.planned_proposal_id)
        {
            return Err(CreationContinuationError::InvalidOwnership);
        }
        let mut request =
            build_creation_inference_request(&attempt, &turn, &base, profile, handle, stream_sink)?;
        let mut rounds = self
            .repository
            .list_creation_inference_rounds(owner, attempt.id)?;
        let mut calls = Vec::new();
        let mut visible_parts = Vec::new();
        let mut terminal = false;
        for (index, round) in rounds.iter().enumerate() {
            if terminal {
                return Err(CreationContinuationError::InvalidRoundHistory);
            }
            let replayed = replay_round(&base, &attempt, &request.context, &calls, round)?;
            request.context = replayed.context;
            calls = replayed.calls;
            visible_parts.extend(round.parts.clone());
            terminal = replayed.terminal;
            if terminal && index + 1 != rounds.len() {
                return Err(CreationContinuationError::InvalidRoundHistory);
            }
        }
        if terminal {
            return self.commit_success(attempt, workflow, rounds, calls, visible_parts);
        }
        if rounds.len() >= usize::from(MAX_CREATION_INFERENCE_ROUNDS) {
            self.fail_attempt(&attempt, CreationAttemptFailureCode::RoundLimit, now)?;
            return Err(CreationContinuationError::RoundLimit);
        }

        loop {
            if handle.cancellation_token().is_cancelled() {
                self.cancel_attempt(&attempt, now)?;
                return Err(CreationContinuationError::Cancelled);
            }
            request.validate()?;
            let outcome = match self.inference.run(request.clone()).await {
                Ok(outcome) => outcome,
                Err(PortError::Cancelled) => {
                    self.cancel_attempt(&attempt, now)?;
                    return Err(CreationContinuationError::Cancelled);
                }
                Err(error) => {
                    let code = match error {
                        PortError::Rejected => CreationAttemptFailureCode::ProviderRejected,
                        PortError::Empty => CreationAttemptFailureCode::EmptyResponse,
                        PortError::Unavailable | PortError::Provider(_) => {
                            CreationAttemptFailureCode::ProviderUnavailable
                        }
                        PortError::Cancelled => unreachable!(),
                    };
                    self.fail_attempt(&attempt, code, now)?;
                    return Err(CreationContinuationError::Inference(error));
                }
            };
            if handle.cancellation_token().is_cancelled() {
                cleanup_outcome_replays(self.repository, &outcome)?;
                self.cancel_attempt(&attempt, now)?;
                return Err(CreationContinuationError::Cancelled);
            }
            let candidate = match validate_outcome(&outcome) {
                Ok(candidate) => candidate,
                Err(CreationContinuationError::Cancelled) => {
                    cleanup_outcome_replays(self.repository, &outcome)?;
                    self.cancel_attempt(&attempt, now)?;
                    return Err(CreationContinuationError::Cancelled);
                }
                Err(error) => {
                    cleanup_outcome_replays(self.repository, &outcome)?;
                    self.fail_attempt(&attempt, error.failure_code(), now)?;
                    return Err(error);
                }
            };
            let round_ordinal =
                u8::try_from(rounds.len()).map_err(|_| CreationContinuationError::RoundLimit)?;
            let next_call_ordinal = u16::try_from(calls.len())
                .map_err(|_| CreationContinuationError::InvalidRoundHistory)?;
            let new_round = match plan_round(&attempt, round_ordinal, candidate, &outcome, now) {
                Ok(round) => round,
                Err(CreationContinuationError::Cancelled) => {
                    cleanup_outcome_replays(self.repository, &outcome)?;
                    self.cancel_attempt(&attempt, now)?;
                    return Err(CreationContinuationError::Cancelled);
                }
                Err(error) => {
                    cleanup_outcome_replays(self.repository, &outcome)?;
                    self.fail_attempt(&attempt, error.failure_code(), now)?;
                    return Err(error);
                }
            };
            let round = match self.repository.admit_creation_inference_round(
                owner,
                attempt.id,
                round_ordinal,
                next_call_ordinal,
                new_round,
            ) {
                Ok(round) => round,
                Err(error) => {
                    cleanup_outcome_replays(self.repository, &outcome)?;
                    return Err(error.into());
                }
            };
            let replayed = replay_round(&base, &attempt, &request.context, &calls, &round)?;
            request.context = replayed.context;
            calls = replayed.calls;
            visible_parts.extend(round.parts.clone());
            terminal = replayed.terminal;
            rounds.push(round);
            if terminal {
                return self.commit_success(attempt, workflow, rounds, calls, visible_parts);
            }
            if rounds.len() >= usize::from(MAX_CREATION_INFERENCE_ROUNDS) {
                self.fail_attempt(&attempt, CreationAttemptFailureCode::RoundLimit, now)?;
                return Err(CreationContinuationError::RoundLimit);
            }
        }
    }

    fn reconstruct_completed(
        &self,
        attempt: CreationInferenceAttempt,
    ) -> Result<CreationContinuationResult, CreationContinuationError> {
        let owner = CreationAttemptOwner {
            workflow_id: attempt.workflow_id,
            turn_id: attempt.turn_id,
        };
        let workflow = self.repository.load_workflow(attempt.workflow_id)?;
        let rounds = self
            .repository
            .list_creation_inference_rounds(owner, attempt.id)?;
        let calls = flatten_calls(&rounds);
        let proposal = if calls.is_empty() {
            None
        } else {
            Some(self.repository.load_proposal(attempt.planned_proposal_id)?)
        };
        Ok(CreationContinuationResult {
            attempt,
            workflow,
            proposal,
            visible_parts: rounds
                .iter()
                .flat_map(|round| round.parts.clone())
                .collect(),
            usage: aggregate_round_usage(&rounds)?,
            rounds,
        })
    }

    fn commit_success(
        &self,
        attempt: CreationInferenceAttempt,
        workflow: CreationWorkflow,
        rounds: Vec<CreationInferenceRound>,
        calls: Vec<AdmittedCreationToolCall>,
        visible_parts: Vec<MessagePart>,
    ) -> Result<CreationContinuationResult, CreationContinuationError> {
        let finished_at = rounds
            .last()
            .map(|round| round.admitted_at)
            .ok_or(CreationContinuationError::InvalidRoundHistory)?;
        let proposal = if calls.is_empty() {
            None
        } else {
            Some(
                reduce_creation_tool_calls(
                    &self.repository.load_proposal(attempt.base_proposal_id)?,
                    attempt.planned_proposal_id,
                    attempt.turn_id,
                    &calls,
                    finished_at,
                )?
                .proposal,
            )
        };
        let settled =
            self.repository
                .settle_creation_attempt_success(CreationAttemptSuccessSettlement {
                    owner: CreationAttemptOwner {
                        workflow_id: attempt.workflow_id,
                        turn_id: attempt.turn_id,
                    },
                    attempt_id: attempt.id,
                    expected_attempt_revision: attempt.revision,
                    expected_workflow_revision: workflow.revision,
                    proposal,
                    now: finished_at,
                })?;
        Ok(CreationContinuationResult {
            attempt: settled.attempt,
            workflow: settled.workflow,
            proposal: settled.proposal,
            visible_parts,
            usage: aggregate_round_usage(&rounds)?,
            rounds,
        })
    }

    fn fail_attempt(
        &self,
        attempt: &CreationInferenceAttempt,
        code: CreationAttemptFailureCode,
        at: TimestampMillis,
    ) -> Result<(), CreationContinuationError> {
        self.repository.transition_creation_attempt(
            attempt.id,
            attempt.revision,
            CreationAttemptStatus::Failed,
            Some(code),
            at,
        )?;
        Ok(())
    }

    fn cancel_attempt(
        &self,
        attempt: &CreationInferenceAttempt,
        at: TimestampMillis,
    ) -> Result<(), CreationContinuationError> {
        self.repository.transition_creation_attempt(
            attempt.id,
            attempt.revision,
            CreationAttemptStatus::Cancelled,
            None,
            at,
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreationContinuationResult {
    pub attempt: CreationInferenceAttempt,
    pub workflow: CreationWorkflow,
    pub proposal: Option<CreationProposal>,
    pub rounds: Vec<CreationInferenceRound>,
    pub visible_parts: Vec<MessagePart>,
    pub usage: UsageCounters,
}

fn build_creation_inference_request(
    attempt: &CreationInferenceAttempt,
    turn: &lettuce_creation::CreationTurn,
    base: &CreationProposal,
    profile: ResolvedInferenceProfile,
    handle: &JobHandle,
    stream_sink: Option<RequestId>,
) -> Result<InferenceRequest, CreationContinuationError> {
    if profile.tool_policy != ToolPolicy::Allowed || base.id != attempt.base_proposal_id {
        return Err(CreationContinuationError::InvalidProfile);
    }
    let draft = serde_json::to_string(&base.draft)
        .map_err(|_| CreationContinuationError::InvalidOwnership)?;
    let target = match attempt.target {
        lettuce_creation::CreationTargetKind::Character => "character",
        lettuce_creation::CreationTargetKind::Persona => "persona",
        lettuce_creation::CreationTargetKind::Lorebook => "lorebook",
    };
    let system = format!(
        "You collaborate with the user on a roleplay {target}. Every draft change must use one of the declared tools. Ask concise plain-text questions when details are missing. Stop calling tools when user input is needed. Use show_preview when a drafting proposal is ready and request_confirmation only during review. Current durable draft JSON: {draft}"
    );
    let input_bytes = system
        .len()
        .checked_add(turn.user_message.len())
        .and_then(|size| u32::try_from(size).ok())
        .ok_or(CreationContinuationError::ContextTooLarge)?;
    let context = ProviderNeutralContext {
        messages: vec![
            ProviderNeutralMessage {
                role: MessageRole::System,
                parts: vec![ProviderContextPart::Text { text: system }],
            },
            ProviderNeutralMessage {
                role: MessageRole::User,
                parts: vec![ProviderContextPart::Text {
                    text: turn.user_message.clone(),
                }],
            },
        ],
        attributions: ContextAttributions::default(),
        budget: ContextBudgetReport {
            selected_messages: 2,
            omitted_messages: 0,
            input_bytes,
            estimated_input_tokens: input_bytes.saturating_add(3) / 4,
            truncated: false,
        },
    };
    let request = InferenceRequest {
        turn_id: GenerationTurnId::from_uuid(turn.id.as_uuid()),
        attempt_id: attempt.id,
        operation: GenerationOperation::Send,
        profile,
        context,
        cancellation: Some(handle.id()),
        stream_sink,
        media_grants: Vec::new(),
        tools: Some(attempt.tool_request.clone()),
    };
    request.validate()?;
    Ok(request)
}

fn plan_round(
    attempt: &CreationInferenceAttempt,
    ordinal: u8,
    candidate: &lettuce_conversations::InferenceCandidate,
    outcome: &InferenceOutcome,
    admitted_at: TimestampMillis,
) -> Result<NewCreationInferenceRound, CreationContinuationError> {
    let calls = candidate
        .tool_calls
        .iter()
        .map(|call| {
            let definition = attempt
                .tool_request
                .definitions
                .iter()
                .find(|definition| definition.name == call.name)
                .ok_or(CreationContinuationError::UndeclaredTool)?;
            Ok::<_, CreationContinuationError>(NewCreationToolCall {
                id: lettuce_types::ToolExecutionId::new(),
                definition_version: definition.version,
                call: call.clone(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let finish_reason = match outcome.finish_reason {
        FinishReason::Stop => CreationRoundFinishReason::Stop,
        FinishReason::Length => CreationRoundFinishReason::Length,
        FinishReason::Cancelled => return Err(CreationContinuationError::Cancelled),
        FinishReason::Error => return Err(CreationContinuationError::ProviderFailed),
    };
    let round = NewCreationInferenceRound {
        ordinal,
        parts: candidate.parts.clone(),
        provider_replay: candidate.provider_replay.clone(),
        usage: outcome.usage.clone(),
        finish_reason,
        provider_request_id: outcome.provider_request_id.clone(),
        calls,
        admitted_at,
    };
    round
        .validate()
        .map_err(|_| CreationContinuationError::InvalidCandidate)?;
    Ok(round)
}

struct ReplayedRound {
    context: ProviderNeutralContext,
    calls: Vec<AdmittedCreationToolCall>,
    terminal: bool,
}

fn replay_round(
    base: &CreationProposal,
    attempt: &CreationInferenceAttempt,
    context: &ProviderNeutralContext,
    prior_calls: &[AdmittedCreationToolCall],
    round: &CreationInferenceRound,
) -> Result<ReplayedRound, CreationContinuationError> {
    round
        .validate()
        .map_err(|_| CreationContinuationError::InvalidRoundHistory)?;
    let mut calls = prior_calls.to_vec();
    calls.extend(round.calls.iter().map(|evidence| AdmittedCreationToolCall {
        definition_version: evidence.definition_version,
        call: evidence.call.clone(),
    }));
    let (outputs, proposal_stage) = if calls.is_empty() {
        (Vec::new(), base.stage)
    } else {
        let batch = reduce_creation_tool_calls(
            base,
            attempt.planned_proposal_id,
            attempt.turn_id,
            &calls,
            round.admitted_at,
        )?;
        let first = usize::from(round.first_call_ordinal);
        (batch.outputs[first..].to_vec(), batch.proposal.stage)
    };
    let mut assistant_parts = round
        .parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::Text { text } | MessagePart::ReasoningSummary { text } => {
                Some(ProviderContextPart::Text { text: text.clone() })
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assistant_parts.extend(round.calls.iter().map(|evidence| {
        ProviderContextPart::ToolCall(TranscriptToolCall {
            execution_id: evidence.id,
            provider_call_id: evidence.call.provider_call_id.clone(),
            name: evidence.call.name.clone(),
            arguments: evidence.call.arguments.clone(),
            raw_arguments: evidence.call.raw_arguments.clone(),
            provider_replay: evidence.call.provider_replay.clone(),
        })
    }));
    let mut continued = context.clone();
    if !assistant_parts.is_empty() {
        continued.messages.push(ProviderNeutralMessage {
            role: MessageRole::Assistant,
            parts: assistant_parts,
        });
    }
    if !round.calls.is_empty() {
        continued.messages.push(ProviderNeutralMessage {
            role: MessageRole::User,
            parts: round
                .calls
                .iter()
                .zip(outputs)
                .map(|(evidence, output)| {
                    ProviderContextPart::ToolResult(TranscriptToolResult {
                        execution_id: evidence.id,
                        provider_call_id: evidence.call.provider_call_id.clone(),
                        name: evidence.call.name.clone(),
                        output,
                    })
                })
                .collect(),
        });
    }
    continued.validate()?;
    Ok(ReplayedRound {
        context: continued,
        calls,
        terminal: round.calls.is_empty() || proposal_stage != attempt.stage,
    })
}

fn validate_outcome(
    outcome: &InferenceOutcome,
) -> Result<&lettuce_conversations::InferenceCandidate, CreationContinuationError> {
    outcome
        .validate()
        .map_err(|_| CreationContinuationError::InvalidCandidate)?;
    if outcome.candidates.len() != 1 {
        return Err(CreationContinuationError::MultipleCandidates);
    }
    match outcome.finish_reason {
        FinishReason::Stop | FinishReason::Length => Ok(&outcome.candidates[0]),
        FinishReason::Cancelled => Err(CreationContinuationError::Cancelled),
        FinishReason::Error => Err(CreationContinuationError::ProviderFailed),
    }
}

fn cleanup_outcome_replays<R: ProviderReplayArtifactPort + ?Sized>(
    repository: &R,
    outcome: &InferenceOutcome,
) -> Result<(), CreationContinuationError> {
    let mut ids = std::collections::BTreeSet::new();
    for candidate in &outcome.candidates {
        if let Some(replay) = &candidate.provider_replay {
            ids.insert(replay.artifact_id);
        }
        ids.extend(
            candidate
                .tool_calls
                .iter()
                .filter_map(|call| call.provider_replay.as_ref())
                .map(|replay| replay.artifact_id),
        );
    }
    for id in ids {
        repository.cleanup_orphan_provider_replay(id)?;
    }
    Ok(())
}

fn flatten_calls(rounds: &[CreationInferenceRound]) -> Vec<AdmittedCreationToolCall> {
    rounds
        .iter()
        .flat_map(|round| &round.calls)
        .map(|evidence| AdmittedCreationToolCall {
            definition_version: evidence.definition_version,
            call: evidence.call.clone(),
        })
        .collect()
}

fn aggregate_round_usage(
    rounds: &[CreationInferenceRound],
) -> Result<UsageCounters, CreationContinuationError> {
    let mut input_tokens = 0u64;
    let mut output_tokens = 0u64;
    for round in rounds {
        let Some(usage) = &round.usage else {
            return Ok(UsageCounters::Unavailable(
                UsageUnavailableReason::ProviderOmitted,
            ));
        };
        input_tokens = input_tokens
            .checked_add(usage.input_tokens)
            .ok_or(CreationContinuationError::UsageOverflow)?;
        output_tokens = output_tokens
            .checked_add(usage.output_tokens)
            .ok_or(CreationContinuationError::UsageOverflow)?;
    }
    Ok(UsageCounters::Known(
        lettuce_conversations::InferenceUsage {
            input_tokens,
            output_tokens,
        },
    ))
}

impl CreationContinuationError {
    const fn failure_code(&self) -> CreationAttemptFailureCode {
        match self {
            Self::ProviderFailed | Self::UndeclaredTool => {
                CreationAttemptFailureCode::ProviderRejected
            }
            Self::MultipleCandidates | Self::InvalidCandidate => {
                CreationAttemptFailureCode::ProviderRejected
            }
            _ => CreationAttemptFailureCode::Internal,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CreationContinuationError {
    #[error("creation repository failed: {0}")]
    Repository(#[from] CreationRepositoryError),
    #[error("creation tool reduction failed: {0}")]
    Tool(#[from] lettuce_creation::CreationToolContractError),
    #[error("provider inference failed: {0}")]
    Inference(PortError),
    #[error("provider replay storage failed: {0}")]
    Artifact(#[from] lettuce_conversations::ArtifactError),
    #[error("creation inference request is invalid: {0}")]
    Validation(#[from] lettuce_conversations::ValidationError),
    #[error("creation attempt is not runnable")]
    AttemptNotRunnable,
    #[error("creation attempt ownership is invalid")]
    InvalidOwnership,
    #[error("creation attempt job ownership is invalid")]
    InvalidJobOwnership,
    #[error("creation inference profile is invalid")]
    InvalidProfile,
    #[error("creation prompt context is too large")]
    ContextTooLarge,
    #[error("creation inference round history is invalid")]
    InvalidRoundHistory,
    #[error("provider returned multiple creation candidates")]
    MultipleCandidates,
    #[error("provider returned an invalid creation candidate")]
    InvalidCandidate,
    #[error("provider returned an undeclared creation tool")]
    UndeclaredTool,
    #[error("provider failed the creation request")]
    ProviderFailed,
    #[error("creation request was cancelled")]
    Cancelled,
    #[error("creation inference reached its round limit")]
    RoundLimit,
    #[error("creation usage counters overflowed")]
    UsageOverflow,
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{Arc, Barrier, Mutex},
        thread,
    };

    use super::*;
    use async_trait::async_trait;
    use lettuce_conversations::{
        InferenceCandidate, InferenceUsage, InferenceWarningCode, ProposedToolCall,
    };
    use lettuce_creation::{
        CreationAttemptRepository, CreationDraft, CreationStage, CreationTarget,
        CreationWorkflowRepository, NewCreationAttempt, NewCreationTurn, NewCreationWorkflow,
    };
    use lettuce_database::Database;
    use lettuce_models::{
        CapabilityStatus, ChatParameterResolutionInput, ChatRequirements, ExpectedModelIdentity,
        ModelCapabilities, ModelKind, ModelProfile, ModelProfileConfig, ProviderAccount,
        ProviderConfig, ProviderProtocol,
    };
    use lettuce_settings::SecretOwnerId;
    use lettuce_types::{
        CreationProposalId, CreationTurnId, CreationWorkflowId, GenerationAttemptId, JobId,
        ModelProfileId, ProviderAccountId, Revision,
    };

    struct ScriptedInference {
        outcomes: Mutex<VecDeque<Result<InferenceOutcome, PortError>>>,
        requests: Mutex<Vec<InferenceRequest>>,
    }

    #[async_trait]
    impl InferencePort for ScriptedInference {
        async fn run(&self, request: InferenceRequest) -> Result<InferenceOutcome, PortError> {
            self.requests.lock().expect("requests").push(request);
            self.outcomes
                .lock()
                .expect("outcomes")
                .pop_front()
                .expect("scripted outcome")
        }
    }

    fn profile() -> ResolvedInferenceProfile {
        let account_id = ProviderAccountId::new();
        let profile_id = ModelProfileId::new();
        let account = ProviderAccount {
            id: account_id,
            secret_owner_id: SecretOwnerId::new(),
            provider_kind: "ollama".into(),
            protocol: ProviderProtocol::Ollama,
            label: "Ollama".into(),
            endpoint: Some("http://127.0.0.1:11434".into()),
            enabled: true,
            streaming_enabled: false,
            allow_invalid_tls: false,
            api_key_ref: None,
            secret_headers: Vec::new(),
            config: ProviderConfig::Standard,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        };
        let model = ModelProfile {
            id: profile_id,
            provider_account_id: account_id,
            external_model_id: "test-model".into(),
            display_name: "Test model".into(),
            kind: ModelKind::Chat,
            config: ModelProfileConfig {
                lorebook_generator_parameters: Default::default(),
                chat_parameters: Default::default(),
                capabilities: ModelCapabilities {
                    input_modalities: lettuce_models::ModalityCapabilities {
                        text: CapabilityStatus::Supported,
                        ..Default::default()
                    },
                    output_modalities: lettuce_models::ModalityCapabilities {
                        text: CapabilityStatus::Supported,
                        ..Default::default()
                    },
                    tools: CapabilityStatus::Supported,
                    ..Default::default()
                },
            },
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        };
        let expected = ExpectedModelIdentity {
            model_profile_id: profile_id,
            model_revision: model.revision,
            provider_account_id: account_id,
            provider_account_revision: account.revision,
            external_model_id: model.external_model_id.clone(),
            display_name: model.display_name.clone(),
            provider_protocol: account.protocol,
            model_kind: ModelKind::Chat,
        };
        let chat_profile = lettuce_models::resolve_chat_profile(
            &expected,
            &model,
            &account,
            &ChatParameterResolutionInput::default(),
            &ChatRequirements {
                require_tools: true,
                ..Default::default()
            },
        )
        .expect("resolved profile");
        ResolvedInferenceProfile {
            chat_profile,
            tool_policy: ToolPolicy::Allowed,
            output_policy: lettuce_conversations::OutputPolicy::Plain,
            safety_policy: lettuce_conversations::SafetyContext::Standard,
            correlation_id: None,
        }
    }

    fn tool_call(name: &str, arguments: serde_json::Value, id: &str) -> ProposedToolCall {
        ProposedToolCall {
            provider_call_id: Some(id.into()),
            name: name.into(),
            arguments,
            raw_arguments: None,
            provider_replay: None,
        }
    }

    fn outcome(
        parts: Vec<MessagePart>,
        calls: Vec<ProposedToolCall>,
        input: u64,
        output: u64,
    ) -> InferenceOutcome {
        InferenceOutcome {
            candidates: vec![InferenceCandidate {
                ordinal: 0,
                parts,
                tool_calls: calls,
                provider_replay: None,
            }],
            usage: Some(InferenceUsage {
                input_tokens: input,
                output_tokens: output,
            }),
            finish_reason: FinishReason::Stop,
            provider_finish_reason: Some("stop".into()),
            provider_request_id: Some(format!("request-{input}")),
            warning_codes: Vec::<InferenceWarningCode>::new(),
        }
    }

    fn setup(
        database: &Database,
        handle: &JobHandle,
        profile: &ResolvedInferenceProfile,
    ) -> GenerationAttemptId {
        let workflow_id = CreationWorkflowId::new();
        let base_id = CreationProposalId::new();
        database
            .create_workflow(NewCreationWorkflow {
                id: workflow_id,
                initial_proposal_id: base_id,
                target: CreationTarget::NewPersona,
                initial_draft: CreationDraft::Persona {
                    name: None,
                    description: None,
                },
                now: TimestampMillis::new(1),
            })
            .expect("workflow");
        let turn = database
            .record_user_turn(NewCreationTurn {
                id: CreationTurnId::new(),
                workflow_id,
                base_proposal_id: base_id,
                user_message: "Create a patient navigator".into(),
                now: TimestampMillis::new(2),
            })
            .expect("turn");
        let attempt_id = GenerationAttemptId::new();
        database
            .create_creation_attempt(NewCreationAttempt {
                id: attempt_id,
                owner: CreationAttemptOwner {
                    workflow_id,
                    turn_id: turn.id,
                },
                base_proposal_id: base_id,
                planned_proposal_id: CreationProposalId::new(),
                retry_parent_id: None,
                job_id: handle.id(),
                profile_fingerprint: creation_inference_profile_fingerprint(profile)
                    .expect("profile fingerprint"),
                now: TimestampMillis::new(3),
            })
            .expect("attempt");
        attempt_id
    }

    #[tokio::test]
    async fn runs_two_native_rounds_and_replays_the_committed_result_exactly() {
        let database = Database::open_in_memory().expect("database");
        let handle = JobHandle::new(JobId::new());
        let profile = profile();
        let attempt_id = setup(&database, &handle, &profile);
        let inference = ScriptedInference {
            outcomes: Mutex::new(VecDeque::from([
                Ok(outcome(
                    vec![MessagePart::Text {
                        text: "I have a direction.".into(),
                    }],
                    vec![
                        tool_call(
                            "set_persona_name",
                            serde_json::json!({"name": "Navigator"}),
                            "call-name",
                        ),
                        tool_call(
                            "set_persona_description",
                            serde_json::json!({"description": "Maps careful routes."}),
                            "call-description",
                        ),
                    ],
                    10,
                    4,
                )),
                Ok(outcome(
                    vec![MessagePart::Text {
                        text: "The draft is ready.".into(),
                    }],
                    vec![tool_call(
                        "show_preview",
                        serde_json::json!({}),
                        "call-preview",
                    )],
                    12,
                    3,
                )),
            ])),
            requests: Mutex::new(Vec::new()),
        };
        let result = CreationContinuationCoordinator::new(&database, &inference)
            .run(
                attempt_id,
                profile.clone(),
                &handle,
                None,
                TimestampMillis::new(4),
            )
            .await
            .expect("continuation");
        assert_eq!(result.attempt.status, CreationAttemptStatus::Succeeded);
        assert_eq!(result.workflow.stage, CreationStage::AwaitingReview);
        assert_eq!(result.rounds.len(), 2);
        assert_eq!(
            result.usage,
            UsageCounters::Known(InferenceUsage {
                input_tokens: 22,
                output_tokens: 7,
            })
        );
        let proposal = result.proposal.as_ref().expect("proposal");
        assert!(matches!(
            &proposal.draft,
            CreationDraft::Persona { name, description }
                if name.as_deref() == Some("Navigator")
                    && description.as_deref() == Some("Maps careful routes.")
        ));
        {
            let requests = inference.requests.lock().expect("requests");
            assert_eq!(requests.len(), 2);
            assert!(
                requests[1]
                    .context
                    .messages
                    .iter()
                    .flat_map(|message| &message.parts)
                    .any(|part| matches!(part, ProviderContextPart::ToolResult(_)))
            );
        }

        let settlement = CreationAttemptSuccessSettlement {
            owner: CreationAttemptOwner {
                workflow_id: result.attempt.workflow_id,
                turn_id: result.attempt.turn_id,
            },
            attempt_id: result.attempt.id,
            expected_attempt_revision: Revision::new(
                result
                    .attempt
                    .revision
                    .get()
                    .checked_sub(1)
                    .expect("pre-settlement revision"),
            ),
            expected_workflow_revision: Revision::INITIAL,
            proposal: result.proposal.clone(),
            now: result.attempt.finished_at.expect("finished"),
        };
        let settlement_replay = database
            .settle_creation_attempt_success(settlement.clone())
            .expect("exact atomic settlement replay");
        assert_eq!(settlement_replay.attempt, result.attempt);
        assert_eq!(settlement_replay.workflow, result.workflow);
        assert_eq!(settlement_replay.proposal, result.proposal);
        let mut changed_settlement = settlement;
        changed_settlement.expected_workflow_revision = result.workflow.revision;
        assert_eq!(
            database.settle_creation_attempt_success(changed_settlement),
            Err(CreationRepositoryError::Conflict)
        );

        let replay = CreationContinuationCoordinator::new(&database, &inference)
            .run(
                attempt_id,
                profile.clone(),
                &handle,
                None,
                TimestampMillis::new(20),
            )
            .await
            .expect("exact completed replay");
        assert_eq!(replay.proposal, result.proposal);
        assert_eq!(replay.rounds, result.rounds);
        assert_eq!(inference.requests.lock().expect("requests").len(), 2);
    }

    #[test]
    fn turn_and_first_attempt_admit_atomically_and_replay_exactly() {
        let database = Database::open_in_memory().expect("database");
        let workflow_id = CreationWorkflowId::new();
        let base_proposal_id = CreationProposalId::new();
        let workflow = database
            .create_workflow(NewCreationWorkflow {
                id: workflow_id,
                initial_proposal_id: base_proposal_id,
                target: CreationTarget::NewPersona,
                initial_draft: CreationDraft::Persona {
                    name: None,
                    description: None,
                },
                now: TimestampMillis::new(1),
            })
            .expect("workflow");
        let handle = JobHandle::new(JobId::new());
        let request = CreationTurnDispatchRequest {
            workflow_id,
            expected_workflow_revision: workflow.revision,
            base_proposal_id,
            turn_id: CreationTurnId::new(),
            attempt_id: GenerationAttemptId::new(),
            planned_proposal_id: CreationProposalId::new(),
            user_message: "Create a navigator".into(),
            profile: profile(),
            now: TimestampMillis::new(2),
        };
        let admitted = admit_creation_turn_dispatch(&database, request.clone(), &handle)
            .expect("atomic admission");
        assert_eq!(
            admit_creation_turn_dispatch(&database, request.clone(), &handle)
                .expect("exact replay"),
            admitted
        );
        let mut changed = request.clone();
        changed.user_message = "Changed retry".into();
        assert!(matches!(
            admit_creation_turn_dispatch(&database, changed, &handle),
            Err(CreationContinuationError::Repository(
                CreationRepositoryError::Conflict
            ))
        ));
        let mut changed_revision = request.clone();
        changed_revision.expected_workflow_revision = Revision::new(
            request
                .expected_workflow_revision
                .get()
                .checked_add(1)
                .expect("revision"),
        );
        assert!(matches!(
            admit_creation_turn_dispatch(&database, changed_revision, &handle),
            Err(CreationContinuationError::Repository(
                CreationRepositoryError::Conflict
            ))
        ));

        let rolled_back_turn_id = CreationTurnId::new();
        let mut reused_job = request;
        reused_job.turn_id = rolled_back_turn_id;
        reused_job.attempt_id = GenerationAttemptId::new();
        reused_job.planned_proposal_id = CreationProposalId::new();
        assert!(matches!(
            admit_creation_turn_dispatch(&database, reused_job, &handle),
            Err(CreationContinuationError::Repository(
                CreationRepositoryError::Conflict
            ))
        ));
        assert_eq!(
            database.load_turn(rolled_back_turn_id),
            Err(CreationRepositoryError::NotFound)
        );
    }

    #[test]
    fn interrupted_attempt_recovers_into_an_empty_bound_child() {
        let database = Database::open_in_memory().expect("database");
        let workflow_id = CreationWorkflowId::new();
        let base_proposal_id = CreationProposalId::new();
        let workflow = database
            .create_workflow(NewCreationWorkflow {
                id: workflow_id,
                initial_proposal_id: base_proposal_id,
                target: CreationTarget::NewPersona,
                initial_draft: CreationDraft::Persona {
                    name: None,
                    description: None,
                },
                now: TimestampMillis::new(1),
            })
            .expect("workflow");
        let parent_handle = JobHandle::new(JobId::new());
        let profile = profile();
        let admitted = admit_creation_turn_dispatch(
            &database,
            CreationTurnDispatchRequest {
                workflow_id,
                expected_workflow_revision: workflow.revision,
                base_proposal_id,
                turn_id: CreationTurnId::new(),
                attempt_id: GenerationAttemptId::new(),
                planned_proposal_id: CreationProposalId::new(),
                user_message: "Create a navigator".into(),
                profile: profile.clone(),
                now: TimestampMillis::new(2),
            },
            &parent_handle,
        )
        .expect("admission");
        let parent = database
            .transition_creation_attempt(
                admitted.attempt.id,
                admitted.attempt.revision,
                CreationAttemptStatus::Running,
                None,
                TimestampMillis::new(3),
            )
            .expect("running");
        let owner = CreationAttemptOwner {
            workflow_id,
            turn_id: admitted.turn.id,
        };
        database
            .admit_creation_inference_round(
                owner,
                parent.id,
                0,
                0,
                NewCreationInferenceRound {
                    ordinal: 0,
                    parts: Vec::new(),
                    provider_replay: None,
                    usage: Some(InferenceUsage {
                        input_tokens: 4,
                        output_tokens: 1,
                    }),
                    finish_reason: CreationRoundFinishReason::Stop,
                    provider_request_id: Some("partial-request".into()),
                    calls: vec![NewCreationToolCall {
                        id: lettuce_types::ToolExecutionId::new(),
                        definition_version: 1,
                        call: tool_call(
                            "set_persona_name",
                            serde_json::json!({"name": "Navigator"}),
                            "partial-call",
                        ),
                    }],
                    admitted_at: TimestampMillis::new(4),
                },
            )
            .expect("partial round");
        let child_handle = JobHandle::new(JobId::new());
        let recovery_request = CreationRecoveryDispatchRequest {
            workflow_id,
            turn_id: admitted.turn.id,
            parent_attempt_id: parent.id,
            child_attempt_id: GenerationAttemptId::new(),
            planned_proposal_id: CreationProposalId::new(),
            profile,
            now: TimestampMillis::new(5),
        };
        assert!(matches!(
            recover_creation_dispatch(&database, recovery_request.clone(), &parent_handle),
            Err(CreationContinuationError::Repository(
                CreationRepositoryError::Conflict
            ))
        ));
        let mut changed_profile = recovery_request.clone();
        changed_profile.profile = self::profile();
        assert!(matches!(
            recover_creation_dispatch(&database, changed_profile, &child_handle),
            Err(CreationContinuationError::Repository(
                CreationRepositoryError::Conflict
            ))
        ));
        assert_eq!(
            database
                .load_creation_attempt(parent.id)
                .expect("parent remains running")
                .status,
            CreationAttemptStatus::Running
        );
        let recovered =
            recover_creation_dispatch(&database, recovery_request.clone(), &child_handle)
                .expect("recovery");
        assert_eq!(recovered.parent.status, CreationAttemptStatus::Interrupted);
        assert_eq!(recovered.child.status, CreationAttemptStatus::Created);
        assert_eq!(recovered.child.retry_parent_id, Some(parent.id));
        assert_eq!(
            database
                .list_creation_inference_rounds(owner, parent.id)
                .expect("parent rounds")
                .len(),
            1
        );
        assert!(
            database
                .list_creation_inference_rounds(owner, recovered.child.id)
                .expect("child rounds")
                .is_empty()
        );
        assert_eq!(
            recover_creation_dispatch(&database, recovery_request, &child_handle)
                .expect("exact recovery replay"),
            recovered
        );
    }

    #[test]
    fn concurrent_interrupted_recovery_converges_on_one_child() {
        let path = std::env::temp_dir().join(format!(
            "lettuce-creation-recovery-{}.db",
            GenerationAttemptId::new()
        ));
        let setup_database = Database::open(&path).expect("setup database");
        let parent_handle = JobHandle::new(JobId::new());
        let profile = profile();
        let parent_id = setup(&setup_database, &parent_handle, &profile);
        let parent = setup_database
            .load_creation_attempt(parent_id)
            .expect("parent");
        let parent = setup_database
            .transition_creation_attempt(
                parent.id,
                parent.revision,
                CreationAttemptStatus::Running,
                None,
                TimestampMillis::new(4),
            )
            .expect("running parent");
        drop(setup_database);

        let recovery = NewCreationAttemptRecovery {
            owner: CreationAttemptOwner {
                workflow_id: parent.workflow_id,
                turn_id: parent.turn_id,
            },
            parent_attempt_id: parent.id,
            child_attempt_id: GenerationAttemptId::new(),
            planned_proposal_id: CreationProposalId::new(),
            job_id: JobId::new(),
            profile_fingerprint: parent.profile_fingerprint,
            now: TimestampMillis::new(5),
        };
        let first = Database::open(&path).expect("first database");
        let second = Database::open(&path).expect("second database");
        let barrier = Arc::new(Barrier::new(2));
        let first_barrier = Arc::clone(&barrier);
        let first_recovery = recovery.clone();
        let first_thread = thread::spawn(move || {
            first_barrier.wait();
            first.recover_creation_attempt(first_recovery)
        });
        let second_barrier = Arc::clone(&barrier);
        let second_thread = thread::spawn(move || {
            second_barrier.wait();
            second.recover_creation_attempt(recovery)
        });
        let first_result = first_thread
            .join()
            .expect("first thread")
            .expect("first recovery");
        let second_result = second_thread
            .join()
            .expect("second thread")
            .expect("second recovery");
        assert_eq!(first_result, second_result);
        assert_eq!(
            first_result.parent.status,
            CreationAttemptStatus::Interrupted
        );
        assert_eq!(first_result.child.status, CreationAttemptStatus::Created);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn text_only_completion_succeeds_without_fabricating_a_proposal() {
        let database = Database::open_in_memory().expect("database");
        let handle = JobHandle::new(JobId::new());
        let profile = profile();
        let attempt_id = setup(&database, &handle, &profile);
        let inference = ScriptedInference {
            outcomes: Mutex::new(VecDeque::from([Ok(outcome(
                vec![MessagePart::Text {
                    text: "What setting should this persona inhabit?".into(),
                }],
                Vec::new(),
                5,
                2,
            ))])),
            requests: Mutex::new(Vec::new()),
        };
        let result = CreationContinuationCoordinator::new(&database, &inference)
            .run(
                attempt_id,
                profile.clone(),
                &handle,
                None,
                TimestampMillis::new(4),
            )
            .await
            .expect("text completion");
        assert!(result.proposal.is_none());
        assert_eq!(result.workflow.stage, CreationStage::Drafting);
        assert_eq!(result.visible_parts.len(), 1);
        assert_eq!(result.attempt.status, CreationAttemptStatus::Succeeded);
    }

    #[tokio::test]
    async fn cancellation_settles_the_attempt_before_provider_dispatch() {
        let database = Database::open_in_memory().expect("database");
        let handle = JobHandle::new(JobId::new());
        let profile = profile();
        let attempt_id = setup(&database, &handle, &profile);
        let inference = ScriptedInference {
            outcomes: Mutex::new(VecDeque::new()),
            requests: Mutex::new(Vec::new()),
        };
        handle.request_cancel();
        assert!(matches!(
            CreationContinuationCoordinator::new(&database, &inference)
                .run(
                    attempt_id,
                    profile.clone(),
                    &handle,
                    None,
                    TimestampMillis::new(4),
                )
                .await,
            Err(CreationContinuationError::Cancelled)
        ));
        assert_eq!(
            database
                .load_creation_attempt(attempt_id)
                .expect("attempt")
                .status,
            CreationAttemptStatus::Cancelled
        );
        assert!(inference.requests.lock().expect("requests").is_empty());
    }

    #[tokio::test]
    async fn provider_unavailability_fails_the_attempt_without_a_round() {
        let database = Database::open_in_memory().expect("database");
        let handle = JobHandle::new(JobId::new());
        let profile = profile();
        let attempt_id = setup(&database, &handle, &profile);
        let inference = ScriptedInference {
            outcomes: Mutex::new(VecDeque::from([Err(PortError::Unavailable)])),
            requests: Mutex::new(Vec::new()),
        };
        assert!(matches!(
            CreationContinuationCoordinator::new(&database, &inference)
                .run(
                    attempt_id,
                    profile.clone(),
                    &handle,
                    None,
                    TimestampMillis::new(4),
                )
                .await,
            Err(CreationContinuationError::Inference(PortError::Unavailable))
        ));
        let attempt = database.load_creation_attempt(attempt_id).expect("attempt");
        assert_eq!(attempt.status, CreationAttemptStatus::Failed);
        assert_eq!(
            attempt.failure,
            Some(CreationAttemptFailureCode::ProviderUnavailable)
        );
        assert!(
            database
                .list_creation_inference_rounds(
                    CreationAttemptOwner {
                        workflow_id: attempt.workflow_id,
                        turn_id: attempt.turn_id,
                    },
                    attempt.id,
                )
                .expect("rounds")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn dispatch_rejects_the_wrong_job_or_resolved_profile_before_starting() {
        let database = Database::open_in_memory().expect("database");
        let handle = JobHandle::new(JobId::new());
        let profile = profile();
        let attempt_id = setup(&database, &handle, &profile);
        let inference = ScriptedInference {
            outcomes: Mutex::new(VecDeque::new()),
            requests: Mutex::new(Vec::new()),
        };
        assert!(matches!(
            CreationContinuationCoordinator::new(&database, &inference)
                .run(
                    attempt_id,
                    profile.clone(),
                    &JobHandle::new(JobId::new()),
                    None,
                    TimestampMillis::new(4),
                )
                .await,
            Err(CreationContinuationError::InvalidJobOwnership)
        ));
        assert!(matches!(
            CreationContinuationCoordinator::new(&database, &inference)
                .run(
                    attempt_id,
                    self::profile(),
                    &handle,
                    None,
                    TimestampMillis::new(4),
                )
                .await,
            Err(CreationContinuationError::InvalidProfile)
        ));
        assert_eq!(
            database
                .load_creation_attempt(attempt_id)
                .expect("attempt")
                .status,
            CreationAttemptStatus::Created
        );
        assert!(inference.requests.lock().expect("requests").is_empty());
    }

    #[tokio::test]
    async fn eight_non_terminal_rounds_fail_with_the_durable_round_limit() {
        let database = Database::open_in_memory().expect("database");
        let handle = JobHandle::new(JobId::new());
        let profile = profile();
        let attempt_id = setup(&database, &handle, &profile);
        let outcomes = (0..MAX_CREATION_INFERENCE_ROUNDS)
            .map(|ordinal| {
                Ok(outcome(
                    Vec::new(),
                    vec![tool_call(
                        "set_persona_name",
                        serde_json::json!({"name": format!("Navigator {ordinal}")}),
                        &format!("call-{ordinal}"),
                    )],
                    1,
                    1,
                ))
            })
            .collect();
        let inference = ScriptedInference {
            outcomes: Mutex::new(outcomes),
            requests: Mutex::new(Vec::new()),
        };
        assert!(matches!(
            CreationContinuationCoordinator::new(&database, &inference)
                .run(
                    attempt_id,
                    profile.clone(),
                    &handle,
                    None,
                    TimestampMillis::new(4),
                )
                .await,
            Err(CreationContinuationError::RoundLimit)
        ));
        let attempt = database.load_creation_attempt(attempt_id).expect("attempt");
        assert_eq!(attempt.status, CreationAttemptStatus::Failed);
        assert_eq!(
            attempt.failure,
            Some(CreationAttemptFailureCode::RoundLimit)
        );
        assert_eq!(
            database
                .list_creation_inference_rounds(
                    CreationAttemptOwner {
                        workflow_id: attempt.workflow_id,
                        turn_id: attempt.turn_id,
                    },
                    attempt.id,
                )
                .expect("rounds")
                .len(),
            usize::from(MAX_CREATION_INFERENCE_ROUNDS)
        );
        assert_eq!(
            inference.requests.lock().expect("requests").len(),
            usize::from(MAX_CREATION_INFERENCE_ROUNDS)
        );
    }
}
