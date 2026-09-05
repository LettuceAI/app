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
    STAGED_LOREBOOK_REFINE_FINAL_INSTRUCTION, STAGED_LOREBOOK_WRITER_FINAL_INSTRUCTION,
    StagedLorebookDraftRevision, StagedLorebookPlanningRun, StagedLorebookRepository,
    StagedLorebookRepositoryError, StagedLorebookWriterAttempt, StagedLorebookWriterDecision,
    StagedLorebookWriterRun, StagedLorebookWriterRunRepository,
    StagedLorebookWriterRunRepositoryError, StagedLorebookWriterUsage,
    reduce_staged_lorebook_writer_calls, staged_lorebook_writer_tool_request,
};
use lettuce_jobs::handle::JobHandle;
use lettuce_types::{GenerationAttemptId, GenerationTurnId, RequestId, TimestampMillis};
use uuid::Uuid;

use crate::{
    cleanup_outcome_replays, condense_prompt_messages, insert_in_chat_messages, rendered_message,
};

#[derive(Debug, Clone, PartialEq)]
pub struct StagedLorebookWriterExecutionResult {
    pub project: StagedLorebookPlanningRun,
    pub replayed: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum StagedLorebookWriterExecutionError {
    #[error("staged lorebook writer execution ownership is invalid")]
    InvalidOwnership,
    #[error("staged lorebook writer prompt is invalid")]
    InvalidPrompt,
    #[error("staged lorebook writer inference was cancelled")]
    Cancelled,
    #[error("staged lorebook writer inference returned an invalid response")]
    InvalidResponse,
    #[error("staged lorebook writer request is too large")]
    ContextTooLarge,
    #[error("staged lorebook writer prompt rendering failed: {0}")]
    Prompt(#[from] PromptRenderError),
    #[error("staged lorebook writer provider failed: {0}")]
    Inference(PortError),
    #[error("staged lorebook writer replay cleanup failed")]
    ReplayCleanup,
    #[error("staged lorebook writer run persistence failed: {0}")]
    Run(StagedLorebookWriterRunRepositoryError),
    #[error("staged lorebook project persistence failed: {0}")]
    Project(StagedLorebookRepositoryError),
}

#[derive(Debug)]
pub struct StagedLorebookWriterExecutionCoordinator<'a, R: ?Sized, P: ?Sized, I: ?Sized> {
    runs: &'a R,
    projects: &'a P,
    inference: &'a I,
}

impl<'a, R: ?Sized, P: ?Sized, I: ?Sized> StagedLorebookWriterExecutionCoordinator<'a, R, P, I> {
    #[must_use]
    pub const fn new(runs: &'a R, projects: &'a P, inference: &'a I) -> Self {
        Self {
            runs,
            projects,
            inference,
        }
    }
}

impl<R, P, I> StagedLorebookWriterExecutionCoordinator<'_, R, P, I>
where
    R: StagedLorebookWriterRunRepository + ProviderReplayArtifactPort + ?Sized,
    P: StagedLorebookRepository + ?Sized,
    I: InferencePort + ?Sized,
{
    pub async fn run(
        &self,
        request_id: RequestId,
        prompt: &PromptDocument,
        handle: &JobHandle,
        stream_sink: Option<RequestId>,
        now: TimestampMillis,
    ) -> Result<StagedLorebookWriterExecutionResult, StagedLorebookWriterExecutionError> {
        let run = self
            .runs
            .load_staged_lorebook_writer_run(request_id)
            .map_err(StagedLorebookWriterExecutionError::Run)?;
        if run.job_id != handle.id() {
            return Err(StagedLorebookWriterExecutionError::InvalidOwnership);
        }
        let project = self
            .projects
            .load_staged_lorebook(run.project_request_id)
            .map_err(StagedLorebookWriterExecutionError::Project)?;
        if project.project.stage == lettuce_creation::StagedLorebookStage::Cancelled {
            return Err(StagedLorebookWriterExecutionError::Cancelled);
        }
        if let Some(attempt) = run.attempt.clone() {
            return settle(self.projects, &run, attempt, true);
        }
        let prompt = run.prompt_snapshot.as_ref().unwrap_or(prompt);
        validate_ownership(&run, prompt, handle)?;
        if handle.cancellation_token().is_cancelled() {
            return Err(StagedLorebookWriterExecutionError::Cancelled);
        }
        let outcome = self
            .inference
            .run(build_request(&run, prompt, handle, stream_sink)?)
            .await
            .map_err(|error| {
                if matches!(error, PortError::Cancelled) {
                    StagedLorebookWriterExecutionError::Cancelled
                } else {
                    StagedLorebookWriterExecutionError::Inference(error)
                }
            })?;
        if handle.cancellation_token().is_cancelled()
            || matches!(outcome.finish_reason, FinishReason::Cancelled)
        {
            cleanup_outcome_replays(self.runs, &outcome)
                .map_err(|_| StagedLorebookWriterExecutionError::ReplayCleanup)?;
            return Err(StagedLorebookWriterExecutionError::Cancelled);
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
            reduce_staged_lorebook_writer_calls(run.plan_id, &calls)
                .map(StagedLorebookWriterDecision::Draft)
                .unwrap_or(StagedLorebookWriterDecision::Invalid)
        } else {
            StagedLorebookWriterDecision::Invalid
        };
        let attempt = StagedLorebookWriterAttempt {
            calls: strip_replay(calls),
            decision,
            usage: outcome
                .usage
                .as_ref()
                .map(|usage| StagedLorebookWriterUsage {
                    input_tokens: usage.input_tokens,
                    cached_input_tokens: usage.cached_input_tokens,
                    reasoning_tokens: usage.reasoning_tokens,
                    output_tokens: usage.output_tokens,
                }),
            provider_finish_reason: outcome.provider_finish_reason.clone(),
            provider_request_id: outcome.provider_request_id.clone(),
            completed_at: now,
        };
        cleanup_outcome_replays(self.runs, &outcome)
            .map_err(|_| StagedLorebookWriterExecutionError::ReplayCleanup)?;
        if self
            .projects
            .load_staged_lorebook(run.project_request_id)
            .map_err(StagedLorebookWriterExecutionError::Project)?
            .project
            .stage
            == lettuce_creation::StagedLorebookStage::Cancelled
        {
            return Err(StagedLorebookWriterExecutionError::Cancelled);
        }
        let stored = self
            .runs
            .commit_staged_lorebook_writer_attempt(request_id, attempt)
            .map_err(StagedLorebookWriterExecutionError::Run)?;
        settle(
            self.projects,
            &stored,
            stored
                .attempt
                .clone()
                .ok_or(StagedLorebookWriterExecutionError::InvalidResponse)?,
            false,
        )
    }
}

fn settle<P: StagedLorebookRepository + ?Sized>(
    projects: &P,
    run: &StagedLorebookWriterRun,
    attempt: StagedLorebookWriterAttempt,
    replayed: bool,
) -> Result<StagedLorebookWriterExecutionResult, StagedLorebookWriterExecutionError> {
    let StagedLorebookWriterDecision::Draft(mut draft) = attempt.decision else {
        return Err(StagedLorebookWriterExecutionError::InvalidResponse);
    };
    let project = if let Some(refinement) = &run.refinement {
        draft.revisions = refinement.base_draft.revisions.clone();
        draft.revisions.push(StagedLorebookDraftRevision {
            feedback: refinement.feedback.clone(),
            content: draft.content.clone(),
            timestamp: attempt.completed_at,
        });
        projects.settle_staged_lorebook_refinement(
            run.project_request_id,
            run.project_revision,
            draft,
            attempt.completed_at,
        )
    } else {
        projects.settle_staged_lorebook_draft(
            run.project_request_id,
            run.project_revision,
            draft,
            attempt.completed_at,
        )
    }
    .map_err(StagedLorebookWriterExecutionError::Project)?;
    Ok(StagedLorebookWriterExecutionResult { project, replayed })
}

fn validate_ownership(
    run: &StagedLorebookWriterRun,
    prompt: &PromptDocument,
    handle: &JobHandle,
) -> Result<(), StagedLorebookWriterExecutionError> {
    if run.job_id != handle.id()
        || prompt.id != run.prompt_id
        || prompt.revision != run.prompt_revision
        || prompt.status != LifecycleStatus::Active
        || prompt.purpose
            != if run.refinement.is_some() {
                PromptPurpose::LorebookGeneratorRefine
            } else {
                PromptPurpose::LorebookGeneratorWriter
            }
    {
        return Err(StagedLorebookWriterExecutionError::InvalidOwnership);
    }
    Ok(())
}

fn build_request(
    run: &StagedLorebookWriterRun,
    prompt: &PromptDocument,
    handle: &JobHandle,
    stream_sink: Option<RequestId>,
) -> Result<InferenceRequest, StagedLorebookWriterExecutionError> {
    let mut profile = run.profile.clone();
    profile.tool_policy = ToolPolicy::Required;
    profile.output_policy = OutputPolicy::Plain;
    let operation_name = if run.refinement.is_some() {
        b"staged-lorebook-refine".as_slice()
    } else {
        b"staged-lorebook-writer".as_slice()
    };
    let request = InferenceRequest {
        turn_id: GenerationTurnId::from_uuid(Uuid::new_v5(&run.job_id.as_uuid(), operation_name)),
        attempt_id: GenerationAttemptId::from_uuid(Uuid::new_v5(&run.job_id.as_uuid(), b"native")),
        operation: GenerationOperation::Send,
        profile,
        context: render_context(run, prompt)?,
        cancellation: Some(handle.id()),
        stream_sink,
        media_grants: Vec::new(),
        tools: Some(staged_lorebook_writer_tool_request()),
    };
    request
        .validate()
        .map_err(|_| StagedLorebookWriterExecutionError::InvalidPrompt)?;
    Ok(request)
}

fn render_context(
    run: &StagedLorebookWriterRun,
    prompt: &PromptDocument,
) -> Result<ProviderNeutralContext, StagedLorebookWriterExecutionError> {
    let values = &run.prompt_values;
    let mut render_values = PromptRenderValues::default();
    for (variable, value) in [
        (PromptVariable::Brief, &values.brief),
        (PromptVariable::Outline, &values.outline),
        (PromptVariable::EntryTitle, &values.entry_title),
        (PromptVariable::EntryCategory, &values.entry_category),
        (
            PromptVariable::EntryProposedKeys,
            &values.entry_proposed_keys,
        ),
        (PromptVariable::EntryRationale, &values.entry_rationale),
        (PromptVariable::RelevantExcerpts, &values.relevant_excerpts),
        (PromptVariable::EntryKeywords, &values.entry_keywords),
        (
            PromptVariable::EntryAlwaysActive,
            &values.entry_always_active,
        ),
        (PromptVariable::EntryContent, &values.entry_content),
        (PromptVariable::UserFeedback, &values.user_feedback),
    ] {
        render_values.purpose_values.insert(variable, value.clone());
    }
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
                time_awareness_enabled: false,
                ..Default::default()
            },
            values: render_values,
        },
    )?;
    let mut messages = rendered
        .relative
        .iter()
        .map(rendered_message)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| StagedLorebookWriterExecutionError::InvalidPrompt)?;
    if prompt.condense {
        condense_prompt_messages(&mut messages);
    }
    let in_chat = rendered
        .in_chat
        .iter()
        .map(|entry| {
            rendered_message(entry)
                .map(|message| (entry.depth, message))
                .map_err(|_| StagedLorebookWriterExecutionError::InvalidPrompt)
        })
        .collect::<Result<Vec<_>, _>>()?;
    insert_in_chat_messages(&mut messages, in_chat);
    messages.push(ProviderNeutralMessage {
        role: MessageRole::User,
        parts: vec![ProviderContextPart::Text {
            text: if run.refinement.is_some() {
                STAGED_LOREBOOK_REFINE_FINAL_INSTRUCTION.into()
            } else {
                STAGED_LOREBOOK_WRITER_FINAL_INSTRUCTION.into()
            },
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
        .ok_or(StagedLorebookWriterExecutionError::ContextTooLarge)?;
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
