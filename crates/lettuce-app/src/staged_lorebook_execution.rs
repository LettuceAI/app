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
    STAGED_LOREBOOK_PLANNER_FINAL_INSTRUCTION, StagedLorebookPlannerAttempt,
    StagedLorebookPlannerDecision, StagedLorebookPlannerUsage, StagedLorebookPlanningRun,
    StagedLorebookRepository, StagedLorebookRepositoryError, StagedLorebookStage,
    reduce_staged_lorebook_planner_calls, staged_lorebook_planner_tool_request,
};
use lettuce_jobs::handle::JobHandle;
use lettuce_types::{GenerationAttemptId, GenerationTurnId, RequestId, TimestampMillis};
use uuid::Uuid;

use crate::{
    cleanup_outcome_replays, condense_prompt_messages, insert_in_chat_messages, rendered_message,
};

#[derive(Debug, Clone, PartialEq)]
pub struct StagedLorebookPlannerExecutionResult {
    pub run: StagedLorebookPlanningRun,
    pub replayed: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum StagedLorebookPlannerExecutionError {
    #[error("staged lorebook planner execution ownership is invalid")]
    InvalidOwnership,
    #[error("staged lorebook planner prompt is invalid")]
    InvalidPrompt,
    #[error("staged lorebook planner inference was cancelled")]
    Cancelled,
    #[error("staged lorebook planner inference returned an invalid response")]
    InvalidResponse,
    #[error("staged lorebook planner request is too large")]
    ContextTooLarge,
    #[error("staged lorebook planner prompt rendering failed: {0}")]
    Prompt(#[from] PromptRenderError),
    #[error("staged lorebook planner provider failed: {0}")]
    Inference(PortError),
    #[error("staged lorebook planner replay cleanup failed")]
    ReplayCleanup,
    #[error("staged lorebook planner persistence failed: {0}")]
    Repository(StagedLorebookRepositoryError),
}

#[derive(Debug)]
pub struct StagedLorebookPlannerExecutionCoordinator<'a, R: ?Sized, I: ?Sized> {
    repository: &'a R,
    inference: &'a I,
}

impl<'a, R: ?Sized, I: ?Sized> StagedLorebookPlannerExecutionCoordinator<'a, R, I> {
    #[must_use]
    pub const fn new(repository: &'a R, inference: &'a I) -> Self {
        Self {
            repository,
            inference,
        }
    }
}

impl<R, I> StagedLorebookPlannerExecutionCoordinator<'_, R, I>
where
    R: StagedLorebookRepository
        + ProviderReplayArtifactPort
        + lettuce_usage::JobUsageLedger
        + ?Sized,
    I: InferencePort + ?Sized,
{
    pub async fn run(
        &self,
        request_id: RequestId,
        prompt: &PromptDocument,
        handle: &JobHandle,
        stream_sink: Option<RequestId>,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlannerExecutionResult, StagedLorebookPlannerExecutionError> {
        let run = self
            .repository
            .load_staged_lorebook(request_id)
            .map_err(StagedLorebookPlannerExecutionError::Repository)?;
        if run.job_id != handle.id() {
            return Err(StagedLorebookPlannerExecutionError::InvalidOwnership);
        }
        if run.project.stage == StagedLorebookStage::Cancelled {
            return Err(StagedLorebookPlannerExecutionError::Cancelled);
        }
        if let Some(attempt) = run.planner_attempt.clone() {
            return settle_checkpoint(self.repository, run, attempt, true);
        }
        let prompt = run.planner_prompt_snapshot.as_ref().unwrap_or(prompt);
        validate_ownership(&run, prompt, handle)?;
        if run.project.stage != StagedLorebookStage::Planning {
            return Err(StagedLorebookPlannerExecutionError::InvalidOwnership);
        }
        if handle.cancellation_token().is_cancelled() {
            return Err(StagedLorebookPlannerExecutionError::Cancelled);
        }
        let request = build_request(&run, prompt, handle, stream_sink)?;
        let outcome = crate::job_inference_usage::run_job_inference(
            self.repository,
            self.inference,
            run.job_id,
            request,
            now,
        )
        .await
        .map_err(|error| {
            if matches!(error, PortError::Cancelled) {
                StagedLorebookPlannerExecutionError::Cancelled
            } else {
                StagedLorebookPlannerExecutionError::Inference(error)
            }
        })?;
        if handle.cancellation_token().is_cancelled()
            || matches!(outcome.finish_reason, FinishReason::Cancelled)
        {
            cleanup_outcome_replays(self.repository, &outcome)
                .map_err(|_| StagedLorebookPlannerExecutionError::ReplayCleanup)?;
            return Err(StagedLorebookPlannerExecutionError::Cancelled);
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
            reduce_staged_lorebook_planner_calls(&run.project, &calls)
                .map(StagedLorebookPlannerDecision::Outline)
                .unwrap_or(StagedLorebookPlannerDecision::Invalid)
        } else {
            StagedLorebookPlannerDecision::Invalid
        };
        let attempt = StagedLorebookPlannerAttempt {
            project_revision: run.project.revision,
            calls: strip_replay(calls),
            decision,
            usage: outcome
                .usage
                .as_ref()
                .map(|usage| StagedLorebookPlannerUsage {
                    input_tokens: usage.input_tokens,
                    cached_input_tokens: usage.cached_input_tokens,
                    reasoning_tokens: usage.reasoning_tokens,
                    cache_write_tokens: usage.cache_write_tokens,
                    web_search_requests: usage.web_search_requests,
                    provider_reported_cost: usage.provider_reported_cost,
                    output_tokens: usage.output_tokens,
                }),
            provider_finish_reason: outcome.provider_finish_reason.clone(),
            provider_request_id: outcome.provider_request_id.clone(),
            completed_at: now,
        };
        cleanup_outcome_replays(self.repository, &outcome)
            .map_err(|_| StagedLorebookPlannerExecutionError::ReplayCleanup)?;
        if self
            .repository
            .load_staged_lorebook(request_id)
            .map_err(StagedLorebookPlannerExecutionError::Repository)?
            .project
            .stage
            == StagedLorebookStage::Cancelled
        {
            return Err(StagedLorebookPlannerExecutionError::Cancelled);
        }
        let stored = self
            .repository
            .commit_staged_lorebook_planner_attempt(request_id, attempt)
            .map_err(StagedLorebookPlannerExecutionError::Repository)?;
        settle_checkpoint(
            self.repository,
            stored.clone(),
            stored
                .planner_attempt
                .clone()
                .ok_or(StagedLorebookPlannerExecutionError::InvalidResponse)?,
            false,
        )
    }
}

fn settle_checkpoint<R: StagedLorebookRepository + ?Sized>(
    repository: &R,
    run: StagedLorebookPlanningRun,
    attempt: StagedLorebookPlannerAttempt,
    replayed: bool,
) -> Result<StagedLorebookPlannerExecutionResult, StagedLorebookPlannerExecutionError> {
    let StagedLorebookPlannerDecision::Outline(outline) = attempt.decision else {
        return Err(StagedLorebookPlannerExecutionError::InvalidResponse);
    };
    let run = repository
        .submit_staged_lorebook_outline(
            run.request_id,
            attempt.project_revision,
            outline,
            attempt.completed_at,
        )
        .map_err(StagedLorebookPlannerExecutionError::Repository)?;
    Ok(StagedLorebookPlannerExecutionResult { run, replayed })
}

fn validate_ownership(
    run: &StagedLorebookPlanningRun,
    prompt: &PromptDocument,
    handle: &JobHandle,
) -> Result<(), StagedLorebookPlannerExecutionError> {
    if run.job_id != handle.id()
        || prompt.id != run.planner_prompt_id
        || prompt.revision != run.planner_prompt_revision
        || prompt.status != LifecycleStatus::Active
        || prompt.purpose != PromptPurpose::LorebookGeneratorPlanner
    {
        return Err(StagedLorebookPlannerExecutionError::InvalidOwnership);
    }
    Ok(())
}

fn build_request(
    run: &StagedLorebookPlanningRun,
    prompt: &PromptDocument,
    handle: &JobHandle,
    stream_sink: Option<RequestId>,
) -> Result<InferenceRequest, StagedLorebookPlannerExecutionError> {
    let mut profile = run.planner_profile.clone();
    profile.tool_policy = ToolPolicy::Required;
    profile.output_policy = OutputPolicy::Plain;
    let request = InferenceRequest {
        turn_id: GenerationTurnId::from_uuid(Uuid::new_v5(
            &run.job_id.as_uuid(),
            b"staged-lorebook-planner",
        )),
        attempt_id: GenerationAttemptId::from_uuid(Uuid::new_v5(&run.job_id.as_uuid(), b"native")),
        operation: GenerationOperation::Send,
        profile,
        context: render_context(run, prompt)?,
        cancellation: Some(handle.id()),
        stream_sink,
        media_grants: Vec::new(),
        tools: Some(staged_lorebook_planner_tool_request()),
    };
    request
        .validate()
        .map_err(|_| StagedLorebookPlannerExecutionError::InvalidPrompt)?;
    Ok(request)
}

fn render_context(
    run: &StagedLorebookPlanningRun,
    prompt: &PromptDocument,
) -> Result<ProviderNeutralContext, StagedLorebookPlannerExecutionError> {
    let mut values = PromptRenderValues::default();
    values
        .purpose_values
        .insert(PromptVariable::Brief, run.project.brief.clone());
    values.purpose_values.insert(
        PromptVariable::TargetCount,
        run.project.target_count.to_string(),
    );
    values.purpose_values.insert(
        PromptVariable::SourceExcerpts,
        format_excerpts(&run.project.excerpts),
    );
    let rendered = render_prompt(
        prompt,
        &PromptRenderContext {
            conditions: lettuce_context::PromptConditionContext {
                chat_mode: PromptEntryChatMode::Direct,
                info_source: PromptEntryInfoSource::Messages,
                has_persona: false,
                message_count: 0,
                participant_count: 1,
                recent_text: String::new(),
                has_memory_summary: false,
                has_key_memories: false,
                has_lorebook_content: false,
                input_scopes: vec!["text".to_owned()],
                output_scopes: vec!["text".to_owned()],
                provider_id: Some(run.planner_profile.chat_profile.provider_kind.clone()),
                reasoning_enabled: run.planner_profile.chat_profile.parameters.reasoning_mode
                    == Some(lettuce_models::ReasoningMode::Enabled)
                    || run
                        .planner_profile
                        .chat_profile
                        .parameters
                        .reasoning_effort
                        .is_some()
                    || run
                        .planner_profile
                        .chat_profile
                        .parameters
                        .reasoning_budget_tokens
                        .is_some(),
                time_awareness_enabled: false,
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
        .map_err(|_| StagedLorebookPlannerExecutionError::InvalidPrompt)?;
    if prompt.condense {
        condense_prompt_messages(&mut messages);
    }
    let in_chat = rendered
        .in_chat
        .iter()
        .map(|entry| {
            rendered_message(entry)
                .map(|message| (entry.depth, message))
                .map_err(|_| StagedLorebookPlannerExecutionError::InvalidPrompt)
        })
        .collect::<Result<Vec<_>, _>>()?;
    insert_in_chat_messages(&mut messages, in_chat);
    messages.push(ProviderNeutralMessage {
        role: MessageRole::User,
        parts: vec![ProviderContextPart::Text {
            text: STAGED_LOREBOOK_PLANNER_FINAL_INSTRUCTION.to_owned(),
        }],
    });
    let input_bytes = text_bytes(&messages)?;
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

fn format_excerpts(excerpts: &[lettuce_creation::StagedLorebookSourceExcerpt]) -> String {
    if excerpts.is_empty() {
        return "(none)".to_owned();
    }
    excerpts
        .iter()
        .map(|excerpt| {
            format!(
                "[{}] {}\n{}",
                excerpt.source_id, excerpt.label, excerpt.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n---\n\n")
}

fn strip_replay(mut calls: Vec<ProposedToolCall>) -> Vec<ProposedToolCall> {
    for call in &mut calls {
        call.provider_replay = None;
    }
    calls
}

fn text_bytes(
    messages: &[ProviderNeutralMessage],
) -> Result<u32, StagedLorebookPlannerExecutionError> {
    messages
        .iter()
        .flat_map(|message| &message.parts)
        .filter_map(|part| match part {
            ProviderContextPart::Text { text } => Some(text.len()),
            _ => None,
        })
        .try_fold(0usize, usize::checked_add)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(StagedLorebookPlannerExecutionError::ContextTooLarge)
}
