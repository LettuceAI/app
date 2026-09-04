use lettuce_context::{
    LifecycleStatus, PromptDocument, PromptEntryChatMode, PromptEntryInfoSource, PromptPurpose,
    PromptRenderContext, PromptRenderError, PromptRenderValues, PromptVariable, render_prompt,
};
use lettuce_conversations::{
    ContextAttributions, ContextBudgetReport, FinishReason, GenerationOperation, InferenceOutcome,
    InferencePort, InferenceRequest, MessagePart, MessageRole, OutputPolicy, PortError,
    PromptAttribution, ProposedToolCall, ProviderContextPart, ProviderNeutralContext,
    ProviderNeutralMessage, ProviderReplayArtifactPort, ToolPolicy,
};
use lettuce_creation::{
    LorebookEntryAttemptCheckpoint, LorebookEntryAttemptDecision, LorebookEntryAttemptKind,
    LorebookEntryAttemptUsage, LorebookEntryGenerationResult, LorebookEntryGenerationRun,
    LorebookEntryRunRepository, LorebookEntryRunRepositoryError, LorebookEntrySource,
    lorebook_entry_fallback_prompt, lorebook_entry_final_instruction, lorebook_entry_tool_request,
    parse_lorebook_entry_fallback, reduce_lorebook_entry_calls,
};
use lettuce_jobs::handle::JobHandle;
use lettuce_types::{GenerationAttemptId, GenerationTurnId, RequestId, TimestampMillis};
use uuid::Uuid;

use crate::{
    cleanup_outcome_replays, condense_prompt_messages, insert_in_chat_messages, rendered_message,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LorebookEntryExecutionResult {
    pub result: LorebookEntryGenerationResult,
    pub attempts: u8,
    pub replayed: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum LorebookEntryExecutionError {
    #[error("lorebook entry generation execution ownership is invalid")]
    InvalidOwnership,
    #[error("lorebook entry generation prompt is invalid")]
    InvalidPrompt,
    #[error("lorebook entry generation inference was cancelled")]
    Cancelled,
    #[error("lorebook entry generation inference returned an invalid response")]
    InvalidResponse,
    #[error("lorebook entry generation request is too large")]
    ContextTooLarge,
    #[error("lorebook entry generation prompt rendering failed: {0}")]
    Prompt(#[from] PromptRenderError),
    #[error("lorebook entry generation provider failed: {0}")]
    Inference(PortError),
    #[error("lorebook entry generation replay cleanup failed")]
    ReplayCleanup,
    #[error("lorebook entry generation run persistence failed: {0}")]
    Run(LorebookEntryRunRepositoryError),
}

#[derive(Debug)]
pub struct LorebookEntryExecutionCoordinator<'a, R: ?Sized, I: ?Sized> {
    repository: &'a R,
    inference: &'a I,
}

impl<'a, R: ?Sized, I: ?Sized> LorebookEntryExecutionCoordinator<'a, R, I> {
    #[must_use]
    pub const fn new(repository: &'a R, inference: &'a I) -> Self {
        Self {
            repository,
            inference,
        }
    }
}

impl<R, I> LorebookEntryExecutionCoordinator<'_, R, I>
where
    R: LorebookEntryRunRepository + ProviderReplayArtifactPort + ?Sized,
    I: InferencePort + ?Sized,
{
    pub async fn run(
        &self,
        request_id: RequestId,
        prompt: &PromptDocument,
        handle: &JobHandle,
        stream_sink: Option<RequestId>,
        now: TimestampMillis,
    ) -> Result<LorebookEntryExecutionResult, LorebookEntryExecutionError> {
        let run = self
            .repository
            .load_lorebook_entry_run(request_id)
            .map_err(LorebookEntryExecutionError::Run)?;
        validate_ownership(&run, prompt, handle)?;
        let mut attempts = self
            .repository
            .load_lorebook_entry_attempts(request_id)
            .map_err(LorebookEntryExecutionError::Run)?;
        let replayed = !attempts.is_empty();
        if let Some(result) = completed_result(&attempts, replayed)? {
            return Ok(result);
        }
        if attempts.is_empty() {
            if handle.cancellation_token().is_cancelled() {
                return Err(LorebookEntryExecutionError::Cancelled);
            }
            let request = build_request(&run, prompt, handle, stream_sink, false)?;
            match self.inference.run(request).await {
                Ok(outcome) => {
                    if handle.cancellation_token().is_cancelled()
                        || matches!(outcome.finish_reason, FinishReason::Cancelled)
                    {
                        cleanup(self.repository, &outcome)?;
                        return Err(LorebookEntryExecutionError::Cancelled);
                    }
                    let checkpoint = native_checkpoint(&run, &outcome, now);
                    cleanup(self.repository, &outcome)?;
                    attempts = self
                        .repository
                        .commit_lorebook_entry_attempt(request_id, checkpoint)
                        .map_err(LorebookEntryExecutionError::Run)?;
                }
                Err(PortError::Cancelled) => return Err(LorebookEntryExecutionError::Cancelled),
                Err(_) => {
                    attempts = self
                        .repository
                        .commit_lorebook_entry_attempt(
                            request_id,
                            checkpoint(
                                0,
                                LorebookEntryAttemptKind::Native,
                                Vec::new(),
                                LorebookEntryAttemptDecision::StructuredFallback,
                                None,
                                None,
                                None,
                                now,
                            ),
                        )
                        .map_err(LorebookEntryExecutionError::Run)?;
                }
            }
            if let Some(result) = completed_result(&attempts, replayed)? {
                return Ok(result);
            }
        }
        if !matches!(
            attempts.last().map(|attempt| &attempt.decision),
            Some(LorebookEntryAttemptDecision::StructuredFallback)
        ) {
            return Err(LorebookEntryExecutionError::InvalidResponse);
        }
        tracing::warn!(
            request_id = %request_id,
            "native lorebook entry result was unusable; using structured fallback"
        );
        if handle.cancellation_token().is_cancelled() {
            return Err(LorebookEntryExecutionError::Cancelled);
        }
        let request = build_request(&run, prompt, handle, stream_sink, true)?;
        let outcome = self.inference.run(request).await.map_err(|error| {
            if matches!(error, PortError::Cancelled) {
                LorebookEntryExecutionError::Cancelled
            } else {
                LorebookEntryExecutionError::Inference(error)
            }
        })?;
        if handle.cancellation_token().is_cancelled()
            || matches!(outcome.finish_reason, FinishReason::Cancelled)
        {
            cleanup(self.repository, &outcome)?;
            return Err(LorebookEntryExecutionError::Cancelled);
        }
        let checkpoint = fallback_checkpoint(&run, &outcome, now);
        cleanup(self.repository, &outcome)?;
        attempts = self
            .repository
            .commit_lorebook_entry_attempt(request_id, checkpoint)
            .map_err(LorebookEntryExecutionError::Run)?;
        completed_result(&attempts, replayed)?.ok_or(LorebookEntryExecutionError::InvalidResponse)
    }
}

fn validate_ownership(
    run: &LorebookEntryGenerationRun,
    prompt: &PromptDocument,
    handle: &JobHandle,
) -> Result<(), LorebookEntryExecutionError> {
    if run.job_id != handle.id()
        || prompt.id != run.prompt_id
        || prompt.revision != run.prompt_revision
        || prompt.status != LifecycleStatus::Active
        || prompt.purpose != PromptPurpose::LorebookEntryWriter
    {
        return Err(LorebookEntryExecutionError::InvalidOwnership);
    }
    Ok(())
}

fn native_checkpoint(
    run: &LorebookEntryGenerationRun,
    outcome: &InferenceOutcome,
    now: TimestampMillis,
) -> LorebookEntryAttemptCheckpoint {
    let valid_outcome = outcome.validate().is_ok()
        && !matches!(outcome.finish_reason, FinishReason::Error)
        && outcome.candidates.len() == 1;
    let calls = outcome
        .candidates
        .first()
        .filter(|_| valid_outcome)
        .map(|candidate| candidate.tool_calls.clone())
        .unwrap_or_default();
    let decision = if !valid_outcome {
        LorebookEntryAttemptDecision::StructuredFallback
    } else {
        match reduce_lorebook_entry_calls(&calls, run.force) {
            Ok(Some(result)) => LorebookEntryAttemptDecision::Result(result),
            Ok(None) => LorebookEntryAttemptDecision::StructuredFallback,
            Err(_) => LorebookEntryAttemptDecision::Invalid,
        }
    };
    checkpoint(
        0,
        LorebookEntryAttemptKind::Native,
        strip_replay(calls),
        decision,
        outcome
            .usage
            .as_ref()
            .map(|usage| LorebookEntryAttemptUsage {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
            }),
        outcome.provider_finish_reason.clone(),
        outcome.provider_request_id.clone(),
        now,
    )
}

fn fallback_checkpoint(
    run: &LorebookEntryGenerationRun,
    outcome: &InferenceOutcome,
    now: TimestampMillis,
) -> LorebookEntryAttemptCheckpoint {
    let decision = if outcome.validate().is_err()
        || matches!(outcome.finish_reason, FinishReason::Error)
        || outcome.candidates.len() != 1
    {
        LorebookEntryAttemptDecision::Invalid
    } else {
        let text = outcome.candidates[0]
            .parts
            .iter()
            .filter_map(|part| match part {
                MessagePart::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<String>();
        match parse_lorebook_entry_fallback(&text, run.fallback_format, run.force) {
            Ok(result) => LorebookEntryAttemptDecision::Result(result),
            Err(_) => LorebookEntryAttemptDecision::Invalid,
        }
    };
    checkpoint(
        1,
        LorebookEntryAttemptKind::StructuredFallback,
        Vec::new(),
        decision,
        outcome
            .usage
            .as_ref()
            .map(|usage| LorebookEntryAttemptUsage {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
            }),
        outcome.provider_finish_reason.clone(),
        outcome.provider_request_id.clone(),
        now,
    )
}

#[allow(clippy::too_many_arguments)]
fn checkpoint(
    ordinal: u8,
    attempt_kind: LorebookEntryAttemptKind,
    calls: Vec<ProposedToolCall>,
    decision: LorebookEntryAttemptDecision,
    usage: Option<LorebookEntryAttemptUsage>,
    provider_finish_reason: Option<String>,
    provider_request_id: Option<String>,
    completed_at: TimestampMillis,
) -> LorebookEntryAttemptCheckpoint {
    LorebookEntryAttemptCheckpoint {
        ordinal,
        attempt_kind,
        calls,
        decision,
        usage,
        provider_finish_reason,
        provider_request_id,
        completed_at,
    }
}

fn strip_replay(mut calls: Vec<ProposedToolCall>) -> Vec<ProposedToolCall> {
    for call in &mut calls {
        call.provider_replay = None;
    }
    calls
}

fn completed_result(
    attempts: &[LorebookEntryAttemptCheckpoint],
    replayed: bool,
) -> Result<Option<LorebookEntryExecutionResult>, LorebookEntryExecutionError> {
    let Some(attempt) = attempts.last() else {
        return Ok(None);
    };
    match &attempt.decision {
        LorebookEntryAttemptDecision::Result(result) => Ok(Some(LorebookEntryExecutionResult {
            result: result.clone(),
            attempts: u8::try_from(attempts.len())
                .map_err(|_| LorebookEntryExecutionError::InvalidResponse)?,
            replayed,
        })),
        LorebookEntryAttemptDecision::Invalid => Err(LorebookEntryExecutionError::InvalidResponse),
        LorebookEntryAttemptDecision::StructuredFallback => Ok(None),
    }
}

fn build_request(
    run: &LorebookEntryGenerationRun,
    prompt: &PromptDocument,
    handle: &JobHandle,
    stream_sink: Option<RequestId>,
    structured_fallback: bool,
) -> Result<InferenceRequest, LorebookEntryExecutionError> {
    let mut context = render_context(run, prompt)?;
    if structured_fallback {
        context.messages.push(ProviderNeutralMessage {
            role: MessageRole::User,
            parts: vec![ProviderContextPart::Text {
                text: lorebook_entry_fallback_prompt(run.fallback_format, run.force).to_owned(),
            }],
        });
    }
    let mut profile = run.profile.clone();
    profile.tool_policy = if structured_fallback {
        ToolPolicy::Disabled
    } else {
        ToolPolicy::Required
    };
    profile.output_policy = OutputPolicy::Plain;
    let request = InferenceRequest {
        turn_id: GenerationTurnId::from_uuid(Uuid::new_v5(
            &run.job_id.as_uuid(),
            b"lorebook-entry-generator",
        )),
        attempt_id: GenerationAttemptId::from_uuid(Uuid::new_v5(
            &run.job_id.as_uuid(),
            if structured_fallback {
                b"structured-fallback"
            } else {
                b"native"
            },
        )),
        operation: GenerationOperation::Send,
        profile,
        context,
        cancellation: Some(handle.id()),
        stream_sink,
        media_grants: Vec::new(),
        tools: (!structured_fallback).then(|| lorebook_entry_tool_request(run.force)),
    };
    request
        .validate()
        .map_err(|_| LorebookEntryExecutionError::InvalidPrompt)?;
    Ok(request)
}

fn render_context(
    run: &LorebookEntryGenerationRun,
    prompt: &PromptDocument,
) -> Result<ProviderNeutralContext, LorebookEntryExecutionError> {
    let values = &run.prompt_values;
    let mut render_values = PromptRenderValues {
        character_name: values.character_name.clone(),
        ..PromptRenderValues::default()
    };
    for (variable, value) in [
        (PromptVariable::LorebookName, &values.lorebook_name),
        (PromptVariable::CharacterName, &values.character_name),
        (PromptVariable::SessionTitle, &values.session_title),
        (PromptVariable::ExistingEntries, &values.existing_entries),
        (PromptVariable::DirectionPrompt, &values.direction_prompt),
        (PromptVariable::SelectedMessages, &values.selected_messages),
        (PromptVariable::MemorySummary, &values.memory_summary),
        (PromptVariable::SelectedMemories, &values.selected_memories),
    ] {
        render_values.purpose_values.insert(variable, value.clone());
    }
    let info_source = match run.source {
        LorebookEntrySource::Messages => PromptEntryInfoSource::Messages,
        LorebookEntrySource::Memory => PromptEntryInfoSource::Memory,
        LorebookEntrySource::Mixed => PromptEntryInfoSource::Mixed,
    };
    let rendered = render_prompt(
        prompt,
        &PromptRenderContext {
            conditions: lettuce_context::PromptConditionContext {
                chat_mode: PromptEntryChatMode::Direct,
                info_source,
                has_persona: run.persona_id.is_some(),
                message_count: run.selected_message_ids.len(),
                participant_count: 2,
                recent_text: [
                    values.direction_prompt.as_str(),
                    values.selected_messages.as_str(),
                    values.memory_summary.as_str(),
                    values.selected_memories.as_str(),
                ]
                .join("\n"),
                has_memory_summary: values.memory_summary.trim() != "(none)",
                has_key_memories: values.selected_memories.trim() != "(none)",
                has_lorebook_content: !values.existing_entries.trim().is_empty(),
                input_scopes: vec!["text".to_owned()],
                output_scopes: vec!["text".to_owned()],
                provider_id: Some(run.profile.chat_profile.provider_kind.clone()),
                reasoning_enabled: run.profile.chat_profile.parameters.reasoning_mode.is_some()
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
        .map_err(|_| LorebookEntryExecutionError::InvalidPrompt)?;
    if prompt.condense {
        condense_prompt_messages(&mut messages);
    }
    let in_chat = rendered
        .in_chat
        .iter()
        .map(|entry| {
            rendered_message(entry)
                .map(|message| (entry.depth, message))
                .map_err(|_| LorebookEntryExecutionError::InvalidPrompt)
        })
        .collect::<Result<Vec<_>, _>>()?;
    insert_in_chat_messages(&mut messages, in_chat);
    messages.push(ProviderNeutralMessage {
        role: MessageRole::User,
        parts: vec![ProviderContextPart::Text {
            text: lorebook_entry_final_instruction(run.source, run.force).to_owned(),
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

fn text_bytes(messages: &[ProviderNeutralMessage]) -> Result<u32, LorebookEntryExecutionError> {
    messages
        .iter()
        .flat_map(|message| &message.parts)
        .filter_map(|part| match part {
            ProviderContextPart::Text { text } => Some(text.len()),
            _ => None,
        })
        .try_fold(0usize, usize::checked_add)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(LorebookEntryExecutionError::ContextTooLarge)
}

fn cleanup<R: ProviderReplayArtifactPort + ?Sized>(
    repository: &R,
    outcome: &InferenceOutcome,
) -> Result<(), LorebookEntryExecutionError> {
    cleanup_outcome_replays(repository, outcome)
        .map_err(|_| LorebookEntryExecutionError::ReplayCleanup)
}
