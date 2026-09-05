use lettuce_context::{
    LifecycleStatus, PromptDocument, PromptEntryChatMode, PromptEntryInfoSource, PromptPurpose,
    PromptRenderContext, PromptRenderError, PromptRenderValues, PromptVariable, render_prompt,
};
use lettuce_conversations::{
    ContextAttributions, ContextBudgetReport, FinishReason, GenerationOperation, InferencePort,
    InferenceRequest, MessageRole, OutputPolicy, PortError, PromptAttribution, ProposedToolCall,
    ProviderContextPart, ProviderNeutralContext, ProviderNeutralMessage,
    ProviderReplayArtifactPort, ToolPolicy,
};
use lettuce_creation::{
    STAGED_LOREBOOK_COHERENCE_FINAL_INSTRUCTION, StagedLorebookCoherenceAttempt,
    StagedLorebookCoherenceDecision, StagedLorebookPlannerUsage, StagedLorebookPlanningRun,
    StagedLorebookRepository, StagedLorebookRepositoryError,
    reduce_staged_lorebook_coherence_calls, staged_lorebook_coherence_tool_request,
};
use lettuce_jobs::handle::JobHandle;
use lettuce_types::{GenerationAttemptId, GenerationTurnId, RequestId, TimestampMillis};
use uuid::Uuid;

use crate::{
    cleanup_outcome_replays, condense_prompt_messages, insert_in_chat_messages, rendered_message,
};

#[derive(Debug, Clone, PartialEq)]
pub struct StagedLorebookCoherenceExecutionResult {
    pub run: StagedLorebookPlanningRun,
    pub replayed: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum StagedLorebookCoherenceExecutionError {
    #[error("staged lorebook coherence execution ownership is invalid")]
    InvalidOwnership,
    #[error("staged lorebook coherence prompt is invalid")]
    InvalidPrompt,
    #[error("staged lorebook coherence inference was cancelled")]
    Cancelled,
    #[error("staged lorebook coherence inference returned an invalid response")]
    InvalidResponse,
    #[error("staged lorebook coherence request is too large")]
    ContextTooLarge,
    #[error("staged lorebook coherence prompt rendering failed: {0}")]
    Prompt(#[from] PromptRenderError),
    #[error("staged lorebook coherence provider failed: {0}")]
    Inference(PortError),
    #[error("staged lorebook coherence replay cleanup failed")]
    ReplayCleanup,
    #[error("staged lorebook coherence persistence failed: {0}")]
    Repository(StagedLorebookRepositoryError),
}

#[derive(Debug)]
pub struct StagedLorebookCoherenceExecutionCoordinator<'a, R: ?Sized, I: ?Sized> {
    repository: &'a R,
    inference: &'a I,
}

impl<'a, R: ?Sized, I: ?Sized> StagedLorebookCoherenceExecutionCoordinator<'a, R, I> {
    #[must_use]
    pub const fn new(repository: &'a R, inference: &'a I) -> Self {
        Self {
            repository,
            inference,
        }
    }
}

impl<R, I> StagedLorebookCoherenceExecutionCoordinator<'_, R, I>
where
    R: StagedLorebookRepository + ProviderReplayArtifactPort + ?Sized,
    I: InferencePort + ?Sized,
{
    pub async fn run(
        &self,
        project_request_id: RequestId,
        coherence_request_id: RequestId,
        prompt: &PromptDocument,
        handle: &JobHandle,
        stream_sink: Option<RequestId>,
        now: TimestampMillis,
    ) -> Result<StagedLorebookCoherenceExecutionResult, StagedLorebookCoherenceExecutionError> {
        let project = self
            .repository
            .load_staged_lorebook(project_request_id)
            .map_err(StagedLorebookCoherenceExecutionError::Repository)?;
        let run = project
            .coherence_runs
            .iter()
            .find(|run| run.request_id == coherence_request_id)
            .cloned()
            .ok_or(StagedLorebookCoherenceExecutionError::InvalidOwnership)?;
        if run.job_id != handle.id() {
            return Err(StagedLorebookCoherenceExecutionError::InvalidOwnership);
        }
        if project.project.stage == lettuce_creation::StagedLorebookStage::Cancelled {
            return Err(StagedLorebookCoherenceExecutionError::Cancelled);
        }
        if let Some(attempt) = run.attempt.clone() {
            return settle(self.repository, project_request_id, &run, attempt, true);
        }
        let prompt = run.prompt_snapshot.as_ref().unwrap_or(prompt);
        validate_ownership(&run, prompt, handle)?;
        if handle.cancellation_token().is_cancelled() {
            return Err(StagedLorebookCoherenceExecutionError::Cancelled);
        }
        let outcome = self
            .inference
            .run(build_request(&run, prompt, handle, stream_sink)?)
            .await
            .map_err(|error| {
                if matches!(error, PortError::Cancelled) {
                    StagedLorebookCoherenceExecutionError::Cancelled
                } else {
                    StagedLorebookCoherenceExecutionError::Inference(error)
                }
            })?;
        if handle.cancellation_token().is_cancelled()
            || matches!(outcome.finish_reason, FinishReason::Cancelled)
        {
            cleanup_outcome_replays(self.repository, &outcome)
                .map_err(|_| StagedLorebookCoherenceExecutionError::ReplayCleanup)?;
            return Err(StagedLorebookCoherenceExecutionError::Cancelled);
        }
        let valid = outcome.validate().is_ok()
            && !matches!(outcome.finish_reason, FinishReason::Error)
            && outcome.candidates.len() == 1;
        let calls = outcome
            .candidates
            .first()
            .filter(|_| valid)
            .map(|candidate| candidate.tool_calls.clone())
            .unwrap_or_default();
        let decision = if valid {
            reduce_staged_lorebook_coherence_calls(&project.project.drafts, &calls)
                .map(StagedLorebookCoherenceDecision::Proposals)
                .unwrap_or(StagedLorebookCoherenceDecision::Invalid)
        } else {
            StagedLorebookCoherenceDecision::Invalid
        };
        let attempt = StagedLorebookCoherenceAttempt {
            calls: strip_replay(calls),
            decision,
            usage: outcome
                .usage
                .as_ref()
                .map(|usage| StagedLorebookPlannerUsage {
                    input_tokens: usage.input_tokens,
                    output_tokens: usage.output_tokens,
                }),
            provider_finish_reason: outcome.provider_finish_reason.clone(),
            provider_request_id: outcome.provider_request_id.clone(),
            completed_at: now,
        };
        cleanup_outcome_replays(self.repository, &outcome)
            .map_err(|_| StagedLorebookCoherenceExecutionError::ReplayCleanup)?;
        if self
            .repository
            .load_staged_lorebook(project_request_id)
            .map_err(StagedLorebookCoherenceExecutionError::Repository)?
            .project
            .stage
            == lettuce_creation::StagedLorebookStage::Cancelled
        {
            return Err(StagedLorebookCoherenceExecutionError::Cancelled);
        }
        let stored = self
            .repository
            .commit_staged_lorebook_coherence_attempt(
                project_request_id,
                coherence_request_id,
                attempt,
            )
            .map_err(StagedLorebookCoherenceExecutionError::Repository)?;
        let run = stored
            .coherence_runs
            .iter()
            .find(|run| run.request_id == coherence_request_id)
            .ok_or(StagedLorebookCoherenceExecutionError::InvalidResponse)?;
        settle(
            self.repository,
            project_request_id,
            run,
            run.attempt
                .clone()
                .ok_or(StagedLorebookCoherenceExecutionError::InvalidResponse)?,
            false,
        )
    }
}

fn settle<R: StagedLorebookRepository + ?Sized>(
    repository: &R,
    project_request_id: RequestId,
    run: &lettuce_creation::StagedLorebookCoherenceRun,
    attempt: StagedLorebookCoherenceAttempt,
    replayed: bool,
) -> Result<StagedLorebookCoherenceExecutionResult, StagedLorebookCoherenceExecutionError> {
    let StagedLorebookCoherenceDecision::Proposals(proposals) = attempt.decision else {
        return Err(StagedLorebookCoherenceExecutionError::InvalidResponse);
    };
    let run = repository
        .submit_staged_lorebook_coherence(
            project_request_id,
            run.project_revision,
            proposals,
            attempt.completed_at,
        )
        .map_err(StagedLorebookCoherenceExecutionError::Repository)?;
    Ok(StagedLorebookCoherenceExecutionResult { run, replayed })
}

fn validate_ownership(
    run: &lettuce_creation::StagedLorebookCoherenceRun,
    prompt: &PromptDocument,
    handle: &JobHandle,
) -> Result<(), StagedLorebookCoherenceExecutionError> {
    if run.job_id != handle.id()
        || prompt.id != run.prompt_id
        || prompt.revision != run.prompt_revision
        || prompt.status != LifecycleStatus::Active
        || prompt.purpose != PromptPurpose::LorebookGeneratorCoherence
    {
        return Err(StagedLorebookCoherenceExecutionError::InvalidOwnership);
    }
    Ok(())
}

fn build_request(
    run: &lettuce_creation::StagedLorebookCoherenceRun,
    prompt: &PromptDocument,
    handle: &JobHandle,
    stream_sink: Option<RequestId>,
) -> Result<InferenceRequest, StagedLorebookCoherenceExecutionError> {
    let mut profile = run.profile.clone();
    profile.tool_policy = ToolPolicy::Required;
    profile.output_policy = OutputPolicy::Plain;
    let request = InferenceRequest {
        turn_id: GenerationTurnId::from_uuid(Uuid::new_v5(
            &run.job_id.as_uuid(),
            b"staged-lorebook-coherence",
        )),
        attempt_id: GenerationAttemptId::from_uuid(Uuid::new_v5(&run.job_id.as_uuid(), b"native")),
        operation: GenerationOperation::Send,
        profile,
        context: render_context(run, prompt)?,
        cancellation: Some(handle.id()),
        stream_sink,
        media_grants: Vec::new(),
        tools: Some(staged_lorebook_coherence_tool_request()),
    };
    request
        .validate()
        .map_err(|_| StagedLorebookCoherenceExecutionError::InvalidPrompt)?;
    Ok(request)
}

fn render_context(
    run: &lettuce_creation::StagedLorebookCoherenceRun,
    prompt: &PromptDocument,
) -> Result<ProviderNeutralContext, StagedLorebookCoherenceExecutionError> {
    let mut values = PromptRenderValues::default();
    values
        .purpose_values
        .insert(PromptVariable::DraftedEntries, run.drafted_entries.clone());
    let rendered = render_prompt(
        prompt,
        &PromptRenderContext {
            conditions: lettuce_context::PromptConditionContext {
                chat_mode: PromptEntryChatMode::Direct,
                info_source: PromptEntryInfoSource::Messages,
                participant_count: 1,
                input_scopes: vec!["text".into()],
                output_scopes: vec!["text".into()],
                provider_id: Some(run.profile.chat_profile.provider_kind.clone()),
                reasoning_enabled: run.profile.chat_profile.parameters.reasoning_mode
                    == Some(lettuce_models::ReasoningMode::Enabled)
                    || run
                        .profile
                        .chat_profile
                        .parameters
                        .reasoning_effort
                        .is_some()
                    || run
                        .profile
                        .chat_profile
                        .parameters
                        .reasoning_budget_tokens
                        .is_some(),
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
        .map_err(|_| StagedLorebookCoherenceExecutionError::InvalidPrompt)?;
    if prompt.condense {
        condense_prompt_messages(&mut messages);
    }
    let in_chat = rendered
        .in_chat
        .iter()
        .map(|entry| {
            rendered_message(entry)
                .map(|message| (entry.depth, message))
                .map_err(|_| StagedLorebookCoherenceExecutionError::InvalidPrompt)
        })
        .collect::<Result<Vec<_>, _>>()?;
    insert_in_chat_messages(&mut messages, in_chat);
    messages.push(ProviderNeutralMessage {
        role: MessageRole::User,
        parts: vec![ProviderContextPart::Text {
            text: STAGED_LOREBOOK_COHERENCE_FINAL_INSTRUCTION.into(),
        }],
    });
    let input_bytes = messages
        .iter()
        .flat_map(|message| &message.parts)
        .filter_map(|part| match part {
            ProviderContextPart::Text { text } => Some(text.len()),
            _ => None,
        })
        .try_fold(0usize, usize::checked_add)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(StagedLorebookCoherenceExecutionError::ContextTooLarge)?;
    Ok(ProviderNeutralContext {
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
            selected_messages: 0,
            omitted_messages: 0,
            input_bytes,
            estimated_input_tokens: input_bytes.saturating_add(3) / 4,
            truncated: false,
        },
    })
}

fn strip_replay(mut calls: Vec<ProposedToolCall>) -> Vec<ProposedToolCall> {
    for call in &mut calls {
        call.provider_replay = None;
    }
    calls
}
