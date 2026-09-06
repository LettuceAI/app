use lettuce_companions::{
    CompanionGrowthProposalCheckpoint, CompanionGrowthRun, CompanionGrowthRunRepository,
    CompanionGrowthRunRepositoryError, SoulApplyReceipt, SoulOwner, SoulPolicyError,
    SoulRepository, SoulRepositoryError, growth_prompt_values, growth_tool_request,
    parse_growth_proposals, prepare_growth_change_set,
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
pub struct CompanionGrowthExecutionResult {
    pub receipt: Option<SoulApplyReceipt>,
    pub applied_facts: usize,
    pub proposal_replayed: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum CompanionGrowthExecutionError {
    #[error("companion growth execution ownership is invalid")]
    InvalidOwnership,
    #[error("companion growth prompt is invalid")]
    InvalidPrompt,
    #[error("companion growth inference was cancelled")]
    Cancelled,
    #[error("companion growth inference returned an invalid response")]
    InvalidResponse,
    #[error("companion growth request is too large")]
    ContextTooLarge,
    #[error("companion growth prompt rendering failed: {0}")]
    Prompt(#[from] PromptRenderError),
    #[error("companion growth provider failed: {0}")]
    Inference(PortError),
    #[error("companion growth replay cleanup failed")]
    ReplayCleanup,
    #[error("companion growth run persistence failed: {0}")]
    Run(CompanionGrowthRunRepositoryError),
    #[error("companion growth policy rejected proposals: {0:?}")]
    Policy(SoulPolicyError),
    #[error("companion growth Soul persistence failed: {0:?}")]
    Soul(SoulRepositoryError),
}

#[derive(Debug)]
pub struct CompanionGrowthExecutionCoordinator<'a, R: ?Sized, I: ?Sized> {
    repository: &'a R,
    inference: &'a I,
}

impl<'a, R: ?Sized, I: ?Sized> CompanionGrowthExecutionCoordinator<'a, R, I> {
    #[must_use]
    pub const fn new(repository: &'a R, inference: &'a I) -> Self {
        Self {
            repository,
            inference,
        }
    }
}

impl<
    R: CompanionGrowthRunRepository
        + SoulRepository
        + ProviderReplayArtifactPort
        + lettuce_usage::JobUsageLedger
        + ?Sized,
    I: InferencePort + ?Sized,
> CompanionGrowthExecutionCoordinator<'_, R, I>
{
    pub async fn run(
        &self,
        job_id: JobId,
        prompt: &PromptDocument,
        handle: &JobHandle,
        stream_sink: Option<RequestId>,
        now: TimestampMillis,
    ) -> Result<CompanionGrowthExecutionResult, CompanionGrowthExecutionError> {
        if handle.id() != job_id {
            return Err(CompanionGrowthExecutionError::InvalidOwnership);
        }
        let mut run = self
            .repository
            .load_companion_growth_run(job_id)
            .map_err(CompanionGrowthExecutionError::Run)?;
        let proposal_replayed = run.proposal_checkpoint.is_some();
        if run.proposal_checkpoint.is_none() {
            if handle.cancellation_token().is_cancelled() {
                return Err(CompanionGrowthExecutionError::Cancelled);
            }
            let request = build_request(&run, prompt, handle, stream_sink)?;
            let outcome = crate::job_inference_usage::run_job_inference(
                self.repository,
                self.inference,
                job_id,
                request,
                now,
            )
            .await
            .map_err(PortError::from)
            .map_err(|error| match error {
                PortError::Cancelled => CompanionGrowthExecutionError::Cancelled,
                other => CompanionGrowthExecutionError::Inference(other),
            })?;
            if handle.cancellation_token().is_cancelled() {
                cleanup(self.repository, &outcome)?;
                return Err(CompanionGrowthExecutionError::Cancelled);
            }
            if outcome.validate().is_err() {
                cleanup(self.repository, &outcome)?;
                return Err(CompanionGrowthExecutionError::InvalidResponse);
            }
            if outcome.candidates.len() != 1 {
                cleanup(self.repository, &outcome)?;
                return Err(CompanionGrowthExecutionError::InvalidResponse);
            }
            if matches!(outcome.finish_reason, FinishReason::Cancelled) {
                cleanup(self.repository, &outcome)?;
                return Err(CompanionGrowthExecutionError::Cancelled);
            }
            if matches!(outcome.finish_reason, FinishReason::Error) {
                cleanup(self.repository, &outcome)?;
                return Err(CompanionGrowthExecutionError::InvalidResponse);
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
            let memory_ids = run
                .fresh_memories
                .iter()
                .map(|memory| memory.id.clone())
                .collect::<Vec<_>>();
            let proposals = parse_growth_proposals(
                &candidate.tool_calls,
                (!fallback.is_empty()).then_some(fallback.as_str()),
                &memory_ids,
            );
            let checkpoint = CompanionGrowthProposalCheckpoint {
                usage: outcome.usage.clone(),
                proposals,
                reduced_at: now,
            };
            run = match self
                .repository
                .commit_companion_growth_proposals(job_id, checkpoint)
            {
                Ok(run) => run,
                Err(error) => {
                    cleanup(self.repository, &outcome)?;
                    return Err(CompanionGrowthExecutionError::Run(error));
                }
            };
            cleanup(self.repository, &outcome)?;
        }
        let checkpoint = run
            .proposal_checkpoint
            .as_ref()
            .ok_or(CompanionGrowthExecutionError::InvalidOwnership)?;
        if checkpoint.proposals.is_empty() {
            return Ok(CompanionGrowthExecutionResult {
                receipt: None,
                applied_facts: 0,
                proposal_replayed,
            });
        }
        let change_set = prepare_growth_change_set(
            &run.soul,
            run.soul.revision,
            checkpoint.proposals.clone(),
            checkpoint.reduced_at,
        )
        .map_err(CompanionGrowthExecutionError::Policy)?;
        let applied_facts = change_set.additions.len();
        let receipt = self
            .repository
            .apply(
                SoulOwner::Character(run.character_id),
                run.operation_id,
                change_set,
            )
            .map_err(CompanionGrowthExecutionError::Soul)?;
        Ok(CompanionGrowthExecutionResult {
            receipt: Some(receipt),
            applied_facts,
            proposal_replayed,
        })
    }
}

fn build_request(
    run: &CompanionGrowthRun,
    prompt: &PromptDocument,
    handle: &JobHandle,
    stream_sink: Option<RequestId>,
) -> Result<InferenceRequest, CompanionGrowthExecutionError> {
    if prompt.status != LifecycleStatus::Active
        || prompt.purpose != PromptPurpose::CompanionGrowthcycle
    {
        return Err(CompanionGrowthExecutionError::InvalidPrompt);
    }
    let growth = growth_prompt_values(
        &run.authored_soul,
        &run.soul,
        &run.fresh_memories,
        run.created_at,
    );
    let mut values = PromptRenderValues {
        character_name: run.companion_name.clone(),
        ..PromptRenderValues::default()
    };
    values
        .purpose_values
        .insert(PromptVariable::CompanionName, run.companion_name.clone());
    values.purpose_values.insert(
        PromptVariable::ChangeableCategories,
        growth.changeable_categories,
    );
    values
        .purpose_values
        .insert(PromptVariable::CurrentGrowth, growth.current_growth);
    values
        .purpose_values
        .insert(PromptVariable::NewMemories, growth.new_memories);
    let rendered = render_prompt(
        prompt,
        &PromptRenderContext {
            conditions: lettuce_context::PromptConditionContext {
                chat_mode: PromptEntryChatMode::Direct,
                info_source: PromptEntryInfoSource::Memory,
                message_count: run.fresh_memories.len(),
                participant_count: 2,
                recent_text: run
                    .fresh_memories
                    .iter()
                    .map(|memory| memory.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n"),
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
        .map_err(|_| CompanionGrowthExecutionError::InvalidPrompt)?;
    let in_chat = rendered
        .in_chat
        .iter()
        .map(|entry| {
            rendered_message(entry)
                .map(|message| (entry.depth, message))
                .map_err(|_| CompanionGrowthExecutionError::InvalidPrompt)
        })
        .collect::<Result<Vec<_>, _>>()?;
    insert_in_chat_messages(&mut messages, in_chat);
    if messages.is_empty() {
        return Err(CompanionGrowthExecutionError::InvalidPrompt);
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
        .ok_or(CompanionGrowthExecutionError::ContextTooLarge)?;
    let mut profile = run.profile.clone();
    profile.tool_policy = ToolPolicy::Required;
    profile.output_policy = OutputPolicy::Plain;
    let request = InferenceRequest {
        turn_id: GenerationTurnId::from_uuid(Uuid::new_v5(&run.job_id.as_uuid(), b"growth")),
        attempt_id: GenerationAttemptId::from_uuid(Uuid::new_v5(
            &run.job_id.as_uuid(),
            b"growth-attempt",
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
                selected_messages: u32::try_from(run.fresh_memories.len())
                    .map_err(|_| CompanionGrowthExecutionError::ContextTooLarge)?,
                omitted_messages: 0,
                input_bytes,
                estimated_input_tokens: input_bytes.saturating_add(3) / 4,
                truncated: false,
            },
        },
        cancellation: Some(handle.id()),
        stream_sink,
        media_grants: Vec::new(),
        tools: Some(growth_tool_request()),
    };
    request
        .validate()
        .map_err(|_| CompanionGrowthExecutionError::InvalidPrompt)?;
    Ok(request)
}

fn cleanup<R: ProviderReplayArtifactPort + ?Sized>(
    repository: &R,
    outcome: &lettuce_conversations::InferenceOutcome,
) -> Result<(), CompanionGrowthExecutionError> {
    cleanup_outcome_replays(repository, outcome)
        .map_err(|_| CompanionGrowthExecutionError::ReplayCleanup)
}
