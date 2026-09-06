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
    LOREBOOK_KEYWORD_FINAL_INSTRUCTION, LorebookKeywordAttemptCheckpoint,
    LorebookKeywordAttemptDecision, LorebookKeywordAttemptKind, LorebookKeywordAttemptUsage,
    LorebookKeywordDraft, LorebookKeywordGenerationRun, LorebookKeywordRunRepository,
    LorebookKeywordRunRepositoryError, lorebook_keyword_fallback_prompt,
    lorebook_keyword_tool_request, parse_lorebook_keyword_fallback, reduce_lorebook_keyword_calls,
};
use lettuce_jobs::handle::JobHandle;
use lettuce_types::{GenerationAttemptId, GenerationTurnId, RequestId, TimestampMillis};
use uuid::Uuid;

use crate::{
    cleanup_outcome_replays, condense_prompt_messages, insert_in_chat_messages, rendered_message,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LorebookKeywordExecutionResult {
    pub result: LorebookKeywordDraft,
    pub attempts: u8,
    pub replayed: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum LorebookKeywordExecutionError {
    #[error("lorebook keyword generation execution ownership is invalid")]
    InvalidOwnership,
    #[error("lorebook keyword generation prompt is invalid")]
    InvalidPrompt,
    #[error("lorebook keyword generation inference was cancelled")]
    Cancelled,
    #[error("lorebook keyword generation inference returned an invalid response")]
    InvalidResponse,
    #[error("lorebook keyword generation request is too large")]
    ContextTooLarge,
    #[error("lorebook keyword generation prompt rendering failed: {0}")]
    Prompt(#[from] PromptRenderError),
    #[error("lorebook keyword generation provider failed: {0}")]
    Inference(PortError),
    #[error("lorebook keyword generation replay cleanup failed")]
    ReplayCleanup,
    #[error("lorebook keyword generation run persistence failed: {0}")]
    Run(LorebookKeywordRunRepositoryError),
}

#[derive(Debug)]
pub struct LorebookKeywordExecutionCoordinator<'a, R: ?Sized, I: ?Sized> {
    repository: &'a R,
    inference: &'a I,
}

impl<'a, R: ?Sized, I: ?Sized> LorebookKeywordExecutionCoordinator<'a, R, I> {
    #[must_use]
    pub const fn new(repository: &'a R, inference: &'a I) -> Self {
        Self {
            repository,
            inference,
        }
    }
}

impl<R, I> LorebookKeywordExecutionCoordinator<'_, R, I>
where
    R: LorebookKeywordRunRepository + ProviderReplayArtifactPort + ?Sized,
    I: InferencePort + ?Sized,
{
    pub async fn run(
        &self,
        request_id: RequestId,
        prompt: &PromptDocument,
        handle: &JobHandle,
        stream_sink: Option<RequestId>,
        now: TimestampMillis,
    ) -> Result<LorebookKeywordExecutionResult, LorebookKeywordExecutionError> {
        let run = self
            .repository
            .load_lorebook_keyword_run(request_id)
            .map_err(LorebookKeywordExecutionError::Run)?;
        validate_ownership(&run, prompt, handle)?;
        let mut attempts = self
            .repository
            .load_lorebook_keyword_attempts(request_id)
            .map_err(LorebookKeywordExecutionError::Run)?;
        let replayed = !attempts.is_empty();
        if let Some(result) = completed_result(&attempts, replayed)? {
            return Ok(result);
        }
        if attempts.is_empty() {
            if handle.cancellation_token().is_cancelled() {
                return Err(LorebookKeywordExecutionError::Cancelled);
            }
            let request = build_request(&run, prompt, handle, stream_sink, false)?;
            match self.inference.run(request).await {
                Ok(outcome) => {
                    if handle.cancellation_token().is_cancelled()
                        || matches!(outcome.finish_reason, FinishReason::Cancelled)
                    {
                        cleanup(self.repository, &outcome)?;
                        return Err(LorebookKeywordExecutionError::Cancelled);
                    }
                    let checkpoint = native_checkpoint(&run, &outcome, now);
                    cleanup(self.repository, &outcome)?;
                    attempts = self
                        .repository
                        .commit_lorebook_keyword_attempt(request_id, checkpoint)
                        .map_err(LorebookKeywordExecutionError::Run)?;
                }
                Err(PortError::Cancelled) => return Err(LorebookKeywordExecutionError::Cancelled),
                Err(_) => {
                    attempts = self
                        .repository
                        .commit_lorebook_keyword_attempt(
                            request_id,
                            checkpoint(
                                0,
                                LorebookKeywordAttemptKind::Native,
                                Vec::new(),
                                LorebookKeywordAttemptDecision::StructuredFallback,
                                None,
                                None,
                                None,
                                now,
                            ),
                        )
                        .map_err(LorebookKeywordExecutionError::Run)?;
                }
            }
            if let Some(result) = completed_result(&attempts, replayed)? {
                return Ok(result);
            }
        }
        if !matches!(
            attempts.last().map(|attempt| &attempt.decision),
            Some(LorebookKeywordAttemptDecision::StructuredFallback)
        ) {
            return Err(LorebookKeywordExecutionError::InvalidResponse);
        }
        tracing::warn!(
            request_id = %request_id,
            "native lorebook keyword result was unusable; using structured fallback"
        );
        if handle.cancellation_token().is_cancelled() {
            return Err(LorebookKeywordExecutionError::Cancelled);
        }
        let request = build_request(&run, prompt, handle, stream_sink, true)?;
        let outcome = self.inference.run(request).await.map_err(|error| {
            if matches!(error, PortError::Cancelled) {
                LorebookKeywordExecutionError::Cancelled
            } else {
                LorebookKeywordExecutionError::Inference(error)
            }
        })?;
        if handle.cancellation_token().is_cancelled()
            || matches!(outcome.finish_reason, FinishReason::Cancelled)
        {
            cleanup(self.repository, &outcome)?;
            return Err(LorebookKeywordExecutionError::Cancelled);
        }
        let checkpoint = fallback_checkpoint(&run, &outcome, now);
        cleanup(self.repository, &outcome)?;
        attempts = self
            .repository
            .commit_lorebook_keyword_attempt(request_id, checkpoint)
            .map_err(LorebookKeywordExecutionError::Run)?;
        completed_result(&attempts, replayed)?.ok_or(LorebookKeywordExecutionError::InvalidResponse)
    }
}

fn validate_ownership(
    run: &LorebookKeywordGenerationRun,
    prompt: &PromptDocument,
    handle: &JobHandle,
) -> Result<(), LorebookKeywordExecutionError> {
    if run.job_id != handle.id()
        || prompt.id != run.prompt_id
        || prompt.revision != run.prompt_revision
        || prompt.status != LifecycleStatus::Active
        || prompt.purpose != PromptPurpose::LorebookKeywordGenerator
    {
        return Err(LorebookKeywordExecutionError::InvalidOwnership);
    }
    Ok(())
}

fn native_checkpoint(
    _run: &LorebookKeywordGenerationRun,
    outcome: &InferenceOutcome,
    now: TimestampMillis,
) -> LorebookKeywordAttemptCheckpoint {
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
        LorebookKeywordAttemptDecision::StructuredFallback
    } else {
        match reduce_lorebook_keyword_calls(&calls) {
            Some(result) => LorebookKeywordAttemptDecision::Result(result),
            None => LorebookKeywordAttemptDecision::StructuredFallback,
        }
    };
    checkpoint(
        0,
        LorebookKeywordAttemptKind::Native,
        strip_replay(calls),
        decision,
        outcome
            .usage
            .as_ref()
            .map(|usage| LorebookKeywordAttemptUsage {
                input_tokens: usage.input_tokens,
                cached_input_tokens: usage.cached_input_tokens,
                reasoning_tokens: usage.reasoning_tokens,
                cache_write_tokens: usage.cache_write_tokens,
                web_search_requests: usage.web_search_requests,
                output_tokens: usage.output_tokens,
            }),
        outcome.provider_finish_reason.clone(),
        outcome.provider_request_id.clone(),
        now,
    )
}

fn fallback_checkpoint(
    run: &LorebookKeywordGenerationRun,
    outcome: &InferenceOutcome,
    now: TimestampMillis,
) -> LorebookKeywordAttemptCheckpoint {
    let decision = if outcome.validate().is_err()
        || matches!(outcome.finish_reason, FinishReason::Error)
        || outcome.candidates.len() != 1
    {
        LorebookKeywordAttemptDecision::Invalid
    } else {
        let text = outcome.candidates[0]
            .parts
            .iter()
            .filter_map(|part| match part {
                MessagePart::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<String>();
        match parse_lorebook_keyword_fallback(&text, run.fallback_format) {
            Ok(result) => LorebookKeywordAttemptDecision::Result(result),
            Err(_) => LorebookKeywordAttemptDecision::Invalid,
        }
    };
    checkpoint(
        1,
        LorebookKeywordAttemptKind::StructuredFallback,
        Vec::new(),
        decision,
        outcome
            .usage
            .as_ref()
            .map(|usage| LorebookKeywordAttemptUsage {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                cached_input_tokens: usage.cached_input_tokens,
                reasoning_tokens: usage.reasoning_tokens,
                cache_write_tokens: usage.cache_write_tokens,
                web_search_requests: usage.web_search_requests,
            }),
        outcome.provider_finish_reason.clone(),
        outcome.provider_request_id.clone(),
        now,
    )
}

#[allow(clippy::too_many_arguments)]
fn checkpoint(
    ordinal: u8,
    attempt_kind: LorebookKeywordAttemptKind,
    calls: Vec<ProposedToolCall>,
    decision: LorebookKeywordAttemptDecision,
    usage: Option<LorebookKeywordAttemptUsage>,
    provider_finish_reason: Option<String>,
    provider_request_id: Option<String>,
    completed_at: TimestampMillis,
) -> LorebookKeywordAttemptCheckpoint {
    LorebookKeywordAttemptCheckpoint {
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
    attempts: &[LorebookKeywordAttemptCheckpoint],
    replayed: bool,
) -> Result<Option<LorebookKeywordExecutionResult>, LorebookKeywordExecutionError> {
    let Some(attempt) = attempts.last() else {
        return Ok(None);
    };
    match &attempt.decision {
        LorebookKeywordAttemptDecision::Result(result) => {
            Ok(Some(LorebookKeywordExecutionResult {
                result: result.clone(),
                attempts: u8::try_from(attempts.len())
                    .map_err(|_| LorebookKeywordExecutionError::InvalidResponse)?,
                replayed,
            }))
        }
        LorebookKeywordAttemptDecision::Invalid => {
            Err(LorebookKeywordExecutionError::InvalidResponse)
        }
        LorebookKeywordAttemptDecision::StructuredFallback => Ok(None),
    }
}

fn build_request(
    run: &LorebookKeywordGenerationRun,
    prompt: &PromptDocument,
    handle: &JobHandle,
    stream_sink: Option<RequestId>,
    structured_fallback: bool,
) -> Result<InferenceRequest, LorebookKeywordExecutionError> {
    let mut context = render_context(run, prompt)?;
    if structured_fallback {
        context.messages.push(ProviderNeutralMessage {
            role: MessageRole::User,
            parts: vec![ProviderContextPart::Text {
                text: lorebook_keyword_fallback_prompt(run.fallback_format).to_owned(),
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
            b"lorebook-keyword-generator",
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
        tools: (!structured_fallback).then(lorebook_keyword_tool_request),
    };
    request
        .validate()
        .map_err(|_| LorebookKeywordExecutionError::InvalidPrompt)?;
    Ok(request)
}

fn render_context(
    run: &LorebookKeywordGenerationRun,
    prompt: &PromptDocument,
) -> Result<ProviderNeutralContext, LorebookKeywordExecutionError> {
    let values = &run.prompt_values;
    let mut render_values = PromptRenderValues::default();
    for (variable, value) in [
        (PromptVariable::EntryTitle, &values.entry_title),
        (PromptVariable::EntryContent, &values.entry_content),
        (PromptVariable::ExistingKeywords, &values.existing_keywords),
        (PromptVariable::DirectionPrompt, &values.direction_prompt),
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
                recent_text: [
                    values.entry_title.as_str(),
                    values.entry_content.as_str(),
                    values.existing_keywords.as_str(),
                    values.direction_prompt.as_str(),
                ]
                .join("\n"),
                has_memory_summary: false,
                has_key_memories: false,
                has_lorebook_content: !values.entry_content.trim().is_empty(),
                input_scopes: vec!["text".to_owned()],
                output_scopes: vec!["text".to_owned()],
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
        .map_err(|_| LorebookKeywordExecutionError::InvalidPrompt)?;
    if prompt.condense {
        condense_prompt_messages(&mut messages);
    }
    let in_chat = rendered
        .in_chat
        .iter()
        .map(|entry| {
            rendered_message(entry)
                .map(|message| (entry.depth, message))
                .map_err(|_| LorebookKeywordExecutionError::InvalidPrompt)
        })
        .collect::<Result<Vec<_>, _>>()?;
    insert_in_chat_messages(&mut messages, in_chat);
    messages.push(ProviderNeutralMessage {
        role: MessageRole::User,
        parts: vec![ProviderContextPart::Text {
            text: LOREBOOK_KEYWORD_FINAL_INSTRUCTION.to_owned(),
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

fn text_bytes(messages: &[ProviderNeutralMessage]) -> Result<u32, LorebookKeywordExecutionError> {
    messages
        .iter()
        .flat_map(|message| &message.parts)
        .filter_map(|part| match part {
            ProviderContextPart::Text { text } => Some(text.len()),
            _ => None,
        })
        .try_fold(0usize, usize::checked_add)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(LorebookKeywordExecutionError::ContextTooLarge)
}

fn cleanup<R: ProviderReplayArtifactPort + ?Sized>(
    repository: &R,
    outcome: &InferenceOutcome,
) -> Result<(), LorebookKeywordExecutionError> {
    cleanup_outcome_replays(repository, outcome)
        .map_err(|_| LorebookKeywordExecutionError::ReplayCleanup)
}
