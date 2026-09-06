use lettuce_companions::{
    CompanionSoulWriterRoundCheckpoint, CompanionSoulWriterRun, CompanionSoulWriterRunRepository,
    CompanionSoulWriterRunRepositoryError, SoulWriterProfileTarget,
    parse_soul_writer_fallback_calls, reduce_soul_writer_calls, soul_writer_fact_fallback_prompt,
    soul_writer_fallback_prompt, soul_writer_tool_request,
};
use lettuce_context::{
    LifecycleStatus, PromptDocument, PromptEntryChatMode, PromptEntryInfoSource, PromptPurpose,
    PromptRenderContext, PromptRenderError, PromptRenderValues, PromptVariable, render_prompt,
};
use lettuce_conversations::{
    ContextAttributions, ContextBudgetReport, FinishReason, GenerationOperation, InferenceOutcome,
    InferencePort, InferenceRequest, MessagePart, MessageRole, OutputPolicy, PortError,
    PromptAttribution, ProposedToolCall, ProviderContextPart, ProviderNeutralContext,
    ProviderNeutralMessage, ProviderReplayArtifactPort, ResolvedInferenceProfile, ToolOutput,
    ToolPolicy, TranscriptToolCall, TranscriptToolResult,
};
use lettuce_jobs::handle::JobHandle;
use lettuce_types::{
    GenerationAttemptId, GenerationTurnId, RequestId, TimestampMillis, ToolExecutionId,
};
use serde_json::Value;
use uuid::Uuid;

use crate::{cleanup_outcome_replays, insert_in_chat_messages, rendered_message};

const MAX_SOUL_WRITER_ROUNDS: usize = 8;

#[derive(Debug, Clone, PartialEq)]
pub struct CompanionSoulWriterExecutionResult {
    pub draft: Value,
    pub rounds: u32,
    pub replayed: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum CompanionSoulWriterExecutionError {
    #[error("companion Soul-writer execution ownership is invalid")]
    InvalidOwnership,
    #[error("companion Soul-writer prompt is invalid")]
    InvalidPrompt,
    #[error("companion Soul-writer inference was cancelled")]
    Cancelled,
    #[error("companion Soul-writer inference returned an invalid response")]
    InvalidResponse,
    #[error("companion Soul-writer stopped before done")]
    RoundLimit,
    #[error("companion Soul-writer request is too large")]
    ContextTooLarge,
    #[error("companion Soul-writer prompt rendering failed: {0}")]
    Prompt(#[from] PromptRenderError),
    #[error("companion Soul-writer provider failed: {0}")]
    Inference(PortError),
    #[error("companion Soul-writer replay cleanup failed")]
    ReplayCleanup,
    #[error("companion Soul-writer run persistence failed: {0}")]
    Run(CompanionSoulWriterRunRepositoryError),
}

#[derive(Debug)]
pub struct CompanionSoulWriterExecutionCoordinator<'a, R: ?Sized, I: ?Sized> {
    repository: &'a R,
    inference: &'a I,
}

impl<'a, R: ?Sized, I: ?Sized> CompanionSoulWriterExecutionCoordinator<'a, R, I> {
    #[must_use]
    pub const fn new(repository: &'a R, inference: &'a I) -> Self {
        Self {
            repository,
            inference,
        }
    }
}

impl<
    R: CompanionSoulWriterRunRepository
        + ProviderReplayArtifactPort
        + lettuce_usage::JobUsageLedger
        + ?Sized,
    I: InferencePort + ?Sized,
> CompanionSoulWriterExecutionCoordinator<'_, R, I>
{
    pub async fn run(
        &self,
        request_id: RequestId,
        prompt: &PromptDocument,
        handle: &JobHandle,
        stream_sink: Option<RequestId>,
        now: TimestampMillis,
    ) -> Result<CompanionSoulWriterExecutionResult, CompanionSoulWriterExecutionError> {
        let mut run = self
            .repository
            .load_companion_soul_writer_run(request_id)
            .map_err(CompanionSoulWriterExecutionError::Run)?;
        if run.job_id != handle.id()
            || prompt.id != run.prompt_id
            || prompt.revision != run.prompt_revision
            || prompt.status != LifecycleStatus::Active
            || prompt.purpose != PromptPurpose::CompanionSoulWriter
        {
            return Err(CompanionSoulWriterExecutionError::InvalidOwnership);
        }
        let replayed = !run.rounds.is_empty();
        if run.rounds.last().is_some_and(|round| round.completed) {
            return completed_result(&run, replayed);
        }
        if run.rounds.len() >= MAX_SOUL_WRITER_ROUNDS {
            return Err(CompanionSoulWriterExecutionError::RoundLimit);
        }
        let target = run
            .rounds
            .last()
            .map_or(SoulWriterProfileTarget::Primary, |round| {
                round.profile_target
            });
        match self
            .run_target(&mut run, prompt, handle, stream_sink, now, target, replayed)
            .await
        {
            Ok(result) => Ok(result),
            Err(CompanionSoulWriterExecutionError::Cancelled) => {
                Err(CompanionSoulWriterExecutionError::Cancelled)
            }
            Err(
                error @ (CompanionSoulWriterExecutionError::Run(_)
                | CompanionSoulWriterExecutionError::ReplayCleanup),
            ) => Err(error),
            Err(_primary_error)
                if target == SoulWriterProfileTarget::Primary
                    && run.fallback_profile.as_ref().is_some_and(|fallback| {
                        fallback.chat_profile.model_profile_id
                            != run.primary_profile.chat_profile.model_profile_id
                    })
                    && run.rounds.len() < MAX_SOUL_WRITER_ROUNDS =>
            {
                self.run_target(
                    &mut run,
                    prompt,
                    handle,
                    stream_sink,
                    now,
                    SoulWriterProfileTarget::Fallback,
                    replayed,
                )
                .await
            }
            Err(error) => Err(error),
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_target(
        &self,
        run: &mut CompanionSoulWriterRun,
        prompt: &PromptDocument,
        handle: &JobHandle,
        stream_sink: Option<RequestId>,
        now: TimestampMillis,
        target: SoulWriterProfileTarget,
        replayed: bool,
    ) -> Result<CompanionSoulWriterExecutionResult, CompanionSoulWriterExecutionError> {
        loop {
            if handle.cancellation_token().is_cancelled() {
                return Err(CompanionSoulWriterExecutionError::Cancelled);
            }
            if run.rounds.len() >= MAX_SOUL_WRITER_ROUNDS {
                return Err(CompanionSoulWriterExecutionError::RoundLimit);
            }
            let request = build_request(run, prompt, handle, stream_sink, target, false)?;
            let outcome = match crate::job_inference_usage::run_job_inference(
                self.repository,
                self.inference,
                handle.id(),
                request,
                now,
            )
            .await
            {
                Ok(outcome) => outcome,
                Err(crate::job_inference_usage::JobInferenceError::Evidence) => {
                    return Err(CompanionSoulWriterExecutionError::Run(
                        CompanionSoulWriterRunRepositoryError::Failure,
                    ));
                }
                Err(crate::job_inference_usage::JobInferenceError::Provider(
                    PortError::Cancelled,
                )) => {
                    return Err(CompanionSoulWriterExecutionError::Cancelled);
                }
                Err(crate::job_inference_usage::JobInferenceError::Provider(error)) => {
                    return Err(CompanionSoulWriterExecutionError::Inference(error));
                }
            };
            if handle.cancellation_token().is_cancelled()
                || matches!(outcome.finish_reason, FinishReason::Cancelled)
            {
                cleanup(self.repository, &outcome)?;
                return Err(CompanionSoulWriterExecutionError::Cancelled);
            }
            let calls = match usable_calls(&outcome) {
                Ok(calls) => calls,
                Err(error) => {
                    cleanup(self.repository, &outcome)?;
                    return Err(error);
                }
            };
            if calls.is_empty() {
                cleanup(self.repository, &outcome)?;
                let fallback = build_request(run, prompt, handle, stream_sink, target, true)?;
                let fallback_outcome = crate::job_inference_usage::run_job_inference(
                    self.repository,
                    self.inference,
                    handle.id(),
                    fallback,
                    now,
                )
                .await
                .map_err(|error| {
                    let error = match error {
                        crate::job_inference_usage::JobInferenceError::Evidence => {
                            return CompanionSoulWriterExecutionError::Run(
                                CompanionSoulWriterRunRepositoryError::Failure,
                            );
                        }
                        crate::job_inference_usage::JobInferenceError::Provider(error) => error,
                    };
                    if matches!(error, PortError::Cancelled) {
                        CompanionSoulWriterExecutionError::Cancelled
                    } else {
                        CompanionSoulWriterExecutionError::Inference(error)
                    }
                })?;
                if handle.cancellation_token().is_cancelled()
                    || matches!(fallback_outcome.finish_reason, FinishReason::Cancelled)
                {
                    cleanup(self.repository, &fallback_outcome)?;
                    return Err(CompanionSoulWriterExecutionError::Cancelled);
                }
                let calls = match fallback_calls(&fallback_outcome, run) {
                    Ok(calls) => calls,
                    Err(error) => {
                        cleanup(self.repository, &fallback_outcome)?;
                        return Err(error);
                    }
                };
                cleanup(self.repository, &fallback_outcome)?;
                *run = commit_round(
                    self.repository,
                    run,
                    target,
                    calls,
                    now,
                    outcome.usage.clone(),
                    fallback_outcome.usage.clone(),
                )?;
                if run.rounds.last().is_some_and(|round| round.completed) {
                    return completed_result(run, replayed);
                }
                return Err(CompanionSoulWriterExecutionError::InvalidResponse);
            }
            *run = commit_round(
                self.repository,
                run,
                target,
                calls,
                now,
                outcome.usage.clone(),
                None,
            )?;
            if run.rounds.last().is_some_and(|round| round.completed) {
                return completed_result(run, replayed);
            }
        }
    }
}

fn usable_calls(
    outcome: &InferenceOutcome,
) -> Result<Vec<ProposedToolCall>, CompanionSoulWriterExecutionError> {
    let candidate = valid_candidate(outcome)?;
    let request = soul_writer_tool_request();
    let mut calls = Vec::new();
    for call in &candidate.tool_calls {
        if request
            .definitions
            .iter()
            .any(|definition| definition.name == call.name)
            && call.validate().is_ok()
        {
            calls.push(call.clone());
            if call.name == lettuce_companions::SOUL_WRITER_DONE_TOOL_NAME {
                break;
            }
        }
    }
    Ok(calls)
}

fn fallback_calls(
    outcome: &InferenceOutcome,
    run: &CompanionSoulWriterRun,
) -> Result<Vec<ProposedToolCall>, CompanionSoulWriterExecutionError> {
    let candidate = valid_candidate(outcome)?;
    let text = candidate
        .parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<String>();
    let mut calls = parse_soul_writer_fallback_calls(&text, run.fallback_format)
        .map_err(|_| CompanionSoulWriterExecutionError::InvalidResponse)?;
    if calls.is_empty() {
        return Err(CompanionSoulWriterExecutionError::InvalidResponse);
    }
    if let Some(done) = calls
        .iter()
        .position(|call| call.name == lettuce_companions::SOUL_WRITER_DONE_TOOL_NAME)
    {
        calls.truncate(done + 1);
    }
    Ok(calls)
}

fn valid_candidate(
    outcome: &InferenceOutcome,
) -> Result<&lettuce_conversations::InferenceCandidate, CompanionSoulWriterExecutionError> {
    if outcome.validate().is_err()
        || outcome.candidates.len() != 1
        || matches!(outcome.finish_reason, FinishReason::Error)
    {
        return Err(CompanionSoulWriterExecutionError::InvalidResponse);
    }
    if matches!(outcome.finish_reason, FinishReason::Cancelled) {
        return Err(CompanionSoulWriterExecutionError::Cancelled);
    }
    Ok(&outcome.candidates[0])
}

fn commit_round<R: CompanionSoulWriterRunRepository + ?Sized>(
    repository: &R,
    run: &CompanionSoulWriterRun,
    profile_target: SoulWriterProfileTarget,
    calls: Vec<ProposedToolCall>,
    now: TimestampMillis,
    usage: Option<lettuce_conversations::InferenceUsage>,
    fallback_usage: Option<lettuce_conversations::InferenceUsage>,
) -> Result<CompanionSoulWriterRun, CompanionSoulWriterExecutionError> {
    let current = run
        .rounds
        .last()
        .map_or(&run.starting_draft, |round| &round.resulting_draft);
    let reduction = reduce_soul_writer_calls(Some(current), &calls, now);
    repository
        .commit_companion_soul_writer_round(
            run.request_id,
            CompanionSoulWriterRoundCheckpoint {
                usage,
                fallback_usage,
                ordinal: u32::try_from(run.rounds.len())
                    .map_err(|_| CompanionSoulWriterExecutionError::RoundLimit)?,
                profile_target,
                calls,
                resulting_draft: reduction.draft,
                completed: reduction.completed,
                reduced_at: now,
            },
        )
        .map_err(CompanionSoulWriterExecutionError::Run)
}

fn completed_result(
    run: &CompanionSoulWriterRun,
    replayed: bool,
) -> Result<CompanionSoulWriterExecutionResult, CompanionSoulWriterExecutionError> {
    let round = run
        .rounds
        .last()
        .filter(|round| round.completed)
        .ok_or(CompanionSoulWriterExecutionError::InvalidResponse)?;
    Ok(CompanionSoulWriterExecutionResult {
        draft: round.resulting_draft.clone(),
        rounds: u32::try_from(run.rounds.len())
            .map_err(|_| CompanionSoulWriterExecutionError::RoundLimit)?,
        replayed,
    })
}

fn build_request(
    run: &CompanionSoulWriterRun,
    prompt: &PromptDocument,
    handle: &JobHandle,
    stream_sink: Option<RequestId>,
    target: SoulWriterProfileTarget,
    structured_fallback: bool,
) -> Result<InferenceRequest, CompanionSoulWriterExecutionError> {
    let mut context = render_context(run, prompt, target)?;
    let profile = profile_for(run, target)?;
    if !structured_fallback {
        replay_rounds(run, target, &mut context)?;
    } else {
        context.messages.push(ProviderNeutralMessage {
            role: MessageRole::User,
            parts: vec![ProviderContextPart::Text {
                text: format!(
                    "{}\n\n{}",
                    soul_writer_fallback_prompt(run.fallback_format),
                    soul_writer_fact_fallback_prompt(run.fallback_format)
                ),
            }],
        });
    }
    let suffix = match (target, structured_fallback) {
        (SoulWriterProfileTarget::Primary, false) => "primary",
        (SoulWriterProfileTarget::Primary, true) => "primary-structured",
        (SoulWriterProfileTarget::Fallback, false) => "fallback",
        (SoulWriterProfileTarget::Fallback, true) => "fallback-structured",
    };
    let mut profile = profile.clone();
    profile.tool_policy = if structured_fallback {
        ToolPolicy::Disabled
    } else {
        ToolPolicy::Required
    };
    profile.output_policy = OutputPolicy::Plain;
    let request = InferenceRequest {
        turn_id: GenerationTurnId::from_uuid(Uuid::new_v5(
            &run.job_id.as_uuid(),
            b"companion-soul-writer",
        )),
        attempt_id: GenerationAttemptId::from_uuid(Uuid::new_v5(
            &run.job_id.as_uuid(),
            suffix.as_bytes(),
        )),
        operation: GenerationOperation::Send,
        profile,
        context,
        cancellation: Some(handle.id()),
        stream_sink,
        media_grants: Vec::new(),
        tools: (!structured_fallback).then(soul_writer_tool_request),
    };
    request
        .validate()
        .map_err(|_| CompanionSoulWriterExecutionError::InvalidPrompt)?;
    Ok(request)
}

fn profile_for(
    run: &CompanionSoulWriterRun,
    target: SoulWriterProfileTarget,
) -> Result<&ResolvedInferenceProfile, CompanionSoulWriterExecutionError> {
    match target {
        SoulWriterProfileTarget::Primary => Ok(&run.primary_profile),
        SoulWriterProfileTarget::Fallback => run
            .fallback_profile
            .as_ref()
            .ok_or(CompanionSoulWriterExecutionError::InvalidOwnership),
    }
}

fn render_context(
    run: &CompanionSoulWriterRun,
    prompt: &PromptDocument,
    target: SoulWriterProfileTarget,
) -> Result<ProviderNeutralContext, CompanionSoulWriterExecutionError> {
    let values = &run.prompt_values;
    let mut render_values = PromptRenderValues {
        character_name: values.character_name.clone(),
        character_description: values.character_description.clone(),
        ..PromptRenderValues::default()
    };
    for (variable, value) in [
        (
            PromptVariable::CharacterDefinition,
            &values.character_definition,
        ),
        (
            PromptVariable::CharacterDescription,
            &values.character_description,
        ),
        (PromptVariable::OpeningContext, &values.opening_context),
        (PromptVariable::CurrentSoul, &values.current_soul),
        (PromptVariable::UserNotes, &values.user_notes),
    ] {
        render_values.purpose_values.insert(variable, value.clone());
    }
    let rendered = render_prompt(
        prompt,
        &PromptRenderContext {
            conditions: lettuce_context::PromptConditionContext {
                chat_mode: PromptEntryChatMode::Direct,
                info_source: PromptEntryInfoSource::Messages,
                message_count: 0,
                participant_count: 2,
                companion_mode_enabled: true,
                provider_id: Some(profile_for(run, target)?.chat_profile.provider_kind.clone()),
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
        .map_err(|_| CompanionSoulWriterExecutionError::InvalidPrompt)?;
    let in_chat = rendered
        .in_chat
        .iter()
        .map(|entry| {
            rendered_message(entry)
                .map(|message| (entry.depth, message))
                .map_err(|_| CompanionSoulWriterExecutionError::InvalidPrompt)
        })
        .collect::<Result<Vec<_>, _>>()?;
    insert_in_chat_messages(&mut messages, in_chat);
    messages.push(ProviderNeutralMessage {
        role: MessageRole::User,
        parts: vec![ProviderContextPart::Text {
            text: values.final_instruction.clone(),
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

fn replay_rounds(
    run: &CompanionSoulWriterRun,
    target: SoulWriterProfileTarget,
    context: &mut ProviderNeutralContext,
) -> Result<(), CompanionSoulWriterExecutionError> {
    let mut draft = run.starting_draft.clone();
    for round in &run.rounds {
        let reduction = reduce_soul_writer_calls(Some(&draft), &round.calls, round.reduced_at);
        let executions = round
            .calls
            .iter()
            .enumerate()
            .zip(reduction.results)
            .map(|((index, call), result)| {
                let id = ToolExecutionId::from_uuid(Uuid::new_v5(
                    &run.job_id.as_uuid(),
                    format!("soul-writer-{}-{index}", round.ordinal).as_bytes(),
                ));
                let provider_replay = (round.profile_target == target)
                    .then(|| call.provider_replay.clone())
                    .flatten();
                (
                    ProviderContextPart::ToolCall(TranscriptToolCall {
                        execution_id: id,
                        provider_call_id: call.provider_call_id.clone(),
                        name: call.name.clone(),
                        arguments: call.arguments.clone(),
                        raw_arguments: call.raw_arguments.clone(),
                        provider_replay,
                    }),
                    ProviderContextPart::ToolResult(TranscriptToolResult {
                        execution_id: id,
                        provider_call_id: call.provider_call_id.clone(),
                        name: call.name.clone(),
                        output: ToolOutput {
                            value: result,
                            is_error: false,
                        },
                    }),
                )
            })
            .collect::<Vec<_>>();
        if !executions.is_empty() {
            context.messages.push(ProviderNeutralMessage {
                role: MessageRole::Assistant,
                parts: executions.iter().map(|(call, _)| call.clone()).collect(),
            });
            context.messages.push(ProviderNeutralMessage {
                role: MessageRole::User,
                parts: executions.into_iter().map(|(_, result)| result).collect(),
            });
        }
        draft = reduction.draft;
    }
    Ok(())
}

fn text_bytes(
    messages: &[ProviderNeutralMessage],
) -> Result<u32, CompanionSoulWriterExecutionError> {
    messages
        .iter()
        .flat_map(|message| &message.parts)
        .filter_map(|part| match part {
            ProviderContextPart::Text { text } => Some(text.len()),
            _ => None,
        })
        .try_fold(0usize, usize::checked_add)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(CompanionSoulWriterExecutionError::ContextTooLarge)
}

fn cleanup<R: ProviderReplayArtifactPort + ?Sized>(
    repository: &R,
    outcome: &InferenceOutcome,
) -> Result<(), CompanionSoulWriterExecutionError> {
    cleanup_outcome_replays(repository, outcome)
        .map_err(|_| CompanionSoulWriterExecutionError::ReplayCleanup)
}
