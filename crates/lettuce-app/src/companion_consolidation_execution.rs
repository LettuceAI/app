use lettuce_companions::{
    CompanionConsolidationProposalCheckpoint, CompanionConsolidationRun,
    CompanionConsolidationRunRepository, CompanionConsolidationRunRepositoryError,
    SoulApplyReceipt, SoulOwner, SoulPolicyError, SoulRepository, SoulRepositoryError,
    consolidation_prompt_values, consolidation_tool_request, parse_consolidation_proposal,
    prepare_consolidation_change_set,
};
use lettuce_context::{
    LifecycleStatus, PromptDocument, PromptEntryChatMode, PromptEntryInfoSource, PromptPurpose,
    PromptRenderContext, PromptRenderError, PromptRenderValues, PromptVariable, render_prompt,
};
use lettuce_conversations::{
    ContextAttributions, ContextBudgetReport, FinishReason, GenerationOperation, InferencePort,
    InferenceRequest, MessagePart, OutputPolicy, PortError, PromptAttribution,
    ProviderNeutralContext, ProviderReplayArtifactPort, ToolPolicy,
};
use lettuce_jobs::handle::JobHandle;
use lettuce_types::{GenerationAttemptId, GenerationTurnId, JobId, RequestId, TimestampMillis};
use uuid::Uuid;

use crate::{cleanup_outcome_replays, insert_in_chat_messages, rendered_message};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanionConsolidationExecutionResult {
    pub receipt: Option<SoulApplyReceipt>,
    pub applied_changes: usize,
    pub proposal_replayed: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum CompanionConsolidationExecutionError {
    #[error("companion consolidation execution ownership is invalid")]
    InvalidOwnership,
    #[error("companion consolidation prompt is invalid")]
    InvalidPrompt,
    #[error("companion consolidation inference was cancelled")]
    Cancelled,
    #[error("companion consolidation inference returned an invalid response")]
    InvalidResponse,
    #[error("companion consolidation request is too large")]
    ContextTooLarge,
    #[error("companion consolidation prompt rendering failed: {0}")]
    Prompt(#[from] PromptRenderError),
    #[error("companion consolidation provider failed: {0}")]
    Inference(PortError),
    #[error("companion consolidation replay cleanup failed")]
    ReplayCleanup,
    #[error("companion consolidation run persistence failed: {0}")]
    Run(CompanionConsolidationRunRepositoryError),
    #[error("companion consolidation policy rejected proposals: {0:?}")]
    Policy(SoulPolicyError),
    #[error("companion consolidation Soul persistence failed: {0:?}")]
    Soul(SoulRepositoryError),
}

#[derive(Debug)]
pub struct CompanionConsolidationExecutionCoordinator<'a, R: ?Sized, I: ?Sized> {
    repository: &'a R,
    inference: &'a I,
}

impl<'a, R: ?Sized, I: ?Sized> CompanionConsolidationExecutionCoordinator<'a, R, I> {
    #[must_use]
    pub const fn new(repository: &'a R, inference: &'a I) -> Self {
        Self {
            repository,
            inference,
        }
    }
}

impl<
    R: CompanionConsolidationRunRepository
        + SoulRepository
        + ProviderReplayArtifactPort
        + lettuce_usage::JobUsageLedger
        + ?Sized,
    I: InferencePort + ?Sized,
> CompanionConsolidationExecutionCoordinator<'_, R, I>
{
    pub async fn run(
        &self,
        job_id: JobId,
        prompt: &PromptDocument,
        handle: &JobHandle,
        stream_sink: Option<RequestId>,
        now: TimestampMillis,
    ) -> Result<CompanionConsolidationExecutionResult, CompanionConsolidationExecutionError> {
        if handle.id() != job_id {
            return Err(CompanionConsolidationExecutionError::InvalidOwnership);
        }
        let mut run = self
            .repository
            .load_companion_consolidation_run(job_id)
            .map_err(CompanionConsolidationExecutionError::Run)?;
        let proposal_replayed = run.proposal_checkpoint.is_some();
        if run.proposal_checkpoint.is_none() {
            if handle.cancellation_token().is_cancelled() {
                return Err(CompanionConsolidationExecutionError::Cancelled);
            }
            let outcome = crate::job_inference_usage::run_job_inference(
                self.repository,
                self.inference,
                job_id,
                build_request(&run, prompt, handle, stream_sink)?,
                now,
            )
            .await
            .map_err(PortError::from)
            .map_err(|error| match error {
                PortError::Cancelled => CompanionConsolidationExecutionError::Cancelled,
                other => CompanionConsolidationExecutionError::Inference(other),
            })?;
            if handle.cancellation_token().is_cancelled()
                || matches!(outcome.finish_reason, FinishReason::Cancelled)
            {
                cleanup(self.repository, &outcome)?;
                return Err(CompanionConsolidationExecutionError::Cancelled);
            }
            if outcome.validate().is_err()
                || outcome.candidates.len() != 1
                || matches!(outcome.finish_reason, FinishReason::Error)
            {
                cleanup(self.repository, &outcome)?;
                return Err(CompanionConsolidationExecutionError::InvalidResponse);
            }
            let candidate = &outcome.candidates[0];
            let fallback = candidate
                .parts
                .iter()
                .filter_map(|part| match part {
                    MessagePart::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<String>();
            let checkpoint = CompanionConsolidationProposalCheckpoint {
                usage: outcome.usage.clone(),
                proposal: parse_consolidation_proposal(
                    &candidate.tool_calls,
                    (!fallback.is_empty()).then_some(fallback.as_str()),
                ),
                reduced_at: now,
            };
            run = match self
                .repository
                .commit_companion_consolidation_proposal(job_id, checkpoint)
            {
                Ok(run) => run,
                Err(error) => {
                    cleanup(self.repository, &outcome)?;
                    return Err(CompanionConsolidationExecutionError::Run(error));
                }
            };
            cleanup(self.repository, &outcome)?;
        }
        let checkpoint = run
            .proposal_checkpoint
            .as_ref()
            .ok_or(CompanionConsolidationExecutionError::InvalidOwnership)?;
        if checkpoint.proposal.core_adjustments.is_empty()
            && checkpoint.proposal.retire_ids.is_empty()
        {
            return Ok(CompanionConsolidationExecutionResult {
                receipt: None,
                applied_changes: 0,
                proposal_replayed,
            });
        }
        let change_set = prepare_consolidation_change_set(
            &run.soul,
            run.soul.revision,
            checkpoint.proposal.core_adjustments.clone(),
            checkpoint.proposal.retire_ids.clone(),
            checkpoint.reduced_at,
        )
        .map_err(CompanionConsolidationExecutionError::Policy)?;
        let applied_changes = change_set.additions.len() + change_set.supersessions.len();
        let receipt = self
            .repository
            .apply(
                SoulOwner::Character(run.character_id),
                run.operation_id,
                change_set,
            )
            .map_err(CompanionConsolidationExecutionError::Soul)?;
        Ok(CompanionConsolidationExecutionResult {
            receipt: Some(receipt),
            applied_changes,
            proposal_replayed,
        })
    }
}

fn build_request(
    run: &CompanionConsolidationRun,
    prompt: &PromptDocument,
    handle: &JobHandle,
    stream_sink: Option<RequestId>,
) -> Result<InferenceRequest, CompanionConsolidationExecutionError> {
    if prompt.status != LifecycleStatus::Active
        || prompt.purpose != PromptPurpose::CompanionConsolidation
    {
        return Err(CompanionConsolidationExecutionError::InvalidPrompt);
    }
    let consolidation = consolidation_prompt_values(&run.authored_soul, &run.soul, run.created_at);
    let mut values = PromptRenderValues {
        character_name: run.companion_name.clone(),
        ..PromptRenderValues::default()
    };
    values
        .purpose_values
        .insert(PromptVariable::CompanionName, run.companion_name.clone());
    values
        .purpose_values
        .insert(PromptVariable::AuthoredCore, consolidation.authored_core);
    values
        .purpose_values
        .insert(PromptVariable::CurrentCore, consolidation.current_core);
    values.purpose_values.insert(
        PromptVariable::AccumulatedGrowth,
        consolidation.accumulated_growth,
    );
    let rendered = render_prompt(
        prompt,
        &PromptRenderContext {
            conditions: lettuce_context::PromptConditionContext {
                chat_mode: PromptEntryChatMode::Direct,
                info_source: PromptEntryInfoSource::Memory,
                message_count: run.soul.facts.len(),
                participant_count: 2,
                dynamic_memory_enabled: true,
                companion_mode_enabled: true,
                provider_id: Some(run.profile.chat_profile.provider_kind.clone()),
                ..Default::default()
            },
            values,
        },
    )?;
    let mut messages = rendered
        .relative
        .iter()
        .map(rendered_message)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| CompanionConsolidationExecutionError::InvalidPrompt)?;
    let in_chat = rendered
        .in_chat
        .iter()
        .map(|entry| {
            rendered_message(entry)
                .map(|message| (entry.depth, message))
                .map_err(|_| CompanionConsolidationExecutionError::InvalidPrompt)
        })
        .collect::<Result<Vec<_>, _>>()?;
    insert_in_chat_messages(&mut messages, in_chat);
    if messages.is_empty() {
        return Err(CompanionConsolidationExecutionError::InvalidPrompt);
    }
    let input_bytes = messages
        .iter()
        .flat_map(|message| &message.parts)
        .try_fold(0usize, |total, part| match part {
            lettuce_conversations::ProviderContextPart::Text { text } => {
                total.checked_add(text.len())
            }
            _ => None,
        })
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(CompanionConsolidationExecutionError::ContextTooLarge)?;
    let mut profile = run.profile.clone();
    profile.tool_policy = ToolPolicy::Required;
    profile.output_policy = OutputPolicy::Plain;
    let request = InferenceRequest {
        turn_id: GenerationTurnId::from_uuid(Uuid::new_v5(&run.job_id.as_uuid(), b"consolidation")),
        attempt_id: GenerationAttemptId::from_uuid(Uuid::new_v5(
            &run.job_id.as_uuid(),
            b"consolidation-attempt",
        )),
        operation: GenerationOperation::Send,
        profile,
        context: ProviderNeutralContext {
            messages,
            attributions: ContextAttributions {
                prompt: Some(PromptAttribution {
                    document_id: prompt.id,
                    revision: prompt.revision,
                    selected_entry_ids: rendered
                        .relative
                        .iter()
                        .chain(&rendered.in_chat)
                        .map(|entry| entry.entry_id)
                        .collect(),
                }),
                ..Default::default()
            },
            budget: ContextBudgetReport {
                selected_messages: u32::try_from(run.soul.facts.len())
                    .map_err(|_| CompanionConsolidationExecutionError::ContextTooLarge)?,
                omitted_messages: 0,
                input_bytes,
                estimated_input_tokens: input_bytes.saturating_add(3) / 4,
                truncated: false,
            },
        },
        cancellation: Some(handle.id()),
        stream_sink,
        media_grants: Vec::new(),
        tools: Some(consolidation_tool_request()),
    };
    request
        .validate()
        .map_err(|_| CompanionConsolidationExecutionError::InvalidPrompt)?;
    Ok(request)
}

fn cleanup<R: ProviderReplayArtifactPort + ?Sized>(
    repository: &R,
    outcome: &lettuce_conversations::InferenceOutcome,
) -> Result<(), CompanionConsolidationExecutionError> {
    cleanup_outcome_replays(repository, outcome)
        .map_err(|_| CompanionConsolidationExecutionError::ReplayCleanup)
}
