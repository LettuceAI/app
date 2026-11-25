use std::collections::BTreeMap;

use chrono::{Datelike, Local, LocalResult, TimeZone, Timelike};
use lettuce_context::{
    LifecycleStatus, PromptDocument, PromptEntryChatMode, PromptEntryInfoSource, PromptPurpose,
    PromptRenderContext, PromptRenderValues, PromptVariable, render_prompt,
};
use lettuce_conversations::{
    ContextAttributions, ContextBudgetReport, ConversationReader, FinishReason,
    GenerationOperation, InferenceOutcome, InferencePort, InferenceRequest, MessagePart,
    MessageRole, OutputPolicy, PortError, PromptAttribution, ProviderContextPart,
    ProviderNeutralContext, ProviderNeutralMessage, ProviderReplayArtifactPort, ToolChoice,
    ToolDefinition, ToolPolicy, ToolRequest,
};
use lettuce_jobs::handle::JobHandle;
use lettuce_memory::{
    DynamicMemoryRunRepository, DynamicMemorySummaryCheckpoint, DynamicMemorySummaryCommit,
    MemoryOrigin, MemoryRepository, MemorySummaryRepository, MemorySummaryUnit,
    MemorySummaryUnitKind, MemorySummaryUnitRepository, MemorySummaryUnitScope,
    NewMemorySummaryUnit, combined_time_range, plan_summary_adoption, render_summary_units,
};
use lettuce_types::{
    DynamicMemoryAttemptId, DynamicMemoryRunId, GenerationAttemptId, GenerationTurnId,
    MemorySummaryUnitId, RequestId, TimestampMillis,
};
use lettuce_usage::JobUsageLedger;
use serde_json::json;
use uuid::Uuid;

use crate::{
    CompanionMemoryInferenceError, MemoryEmbeddingEngine, cleanup_outcome_replays,
    format_message_timestamp, insert_in_chat_messages, materialize_sources, rendered_message,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummaryMode {
    Cumulative,
    Chapters {
        token_budget: u32,
        chapter_size: u32,
    },
}

impl SummaryMode {
    #[must_use]
    pub const fn from_settings(settings: &lettuce_settings::DynamicMemorySettings) -> Self {
        if settings.chapter_summaries_enabled {
            Self::Chapters {
                token_budget: settings.summary_token_budget,
                chapter_size: settings.summary_chapter_size,
            }
        } else {
            Self::Cumulative
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanionMemorySummaryResult {
    pub checkpoint: DynamicMemorySummaryCheckpoint,
    pub replayed: bool,
}

#[derive(Debug)]
pub struct CompanionMemorySummaryCoordinator<'a, E: ?Sized, R: ?Sized, C: ?Sized, I: ?Sized> {
    engine: &'a E,
    repository: &'a R,
    conversations: &'a C,
    inference: &'a I,
}

impl<'a, E: ?Sized, R: ?Sized, C: ?Sized, I: ?Sized>
    CompanionMemorySummaryCoordinator<'a, E, R, C, I>
{
    #[must_use]
    pub const fn new(
        engine: &'a E,
        repository: &'a R,
        conversations: &'a C,
        inference: &'a I,
    ) -> Self {
        Self {
            engine,
            repository,
            conversations,
            inference,
        }
    }
}

impl<
    E: MemoryEmbeddingEngine + ?Sized,
    R: DynamicMemoryRunRepository
        + MemoryRepository
        + MemorySummaryRepository
        + MemorySummaryUnitRepository
        + ProviderReplayArtifactPort
        + JobUsageLedger
        + crate::generation::runtime_text::RuntimeTextSource
        + ?Sized,
    C: ConversationReader + ?Sized,
    I: InferencePort + ?Sized,
> CompanionMemorySummaryCoordinator<'_, E, R, C, I>
{
    #[allow(clippy::too_many_arguments)]
    pub async fn run(
        &self,
        run_id: DynamicMemoryRunId,
        attempt_id: DynamicMemoryAttemptId,
        window_selection: crate::CompanionMemoryWindowSelection,
        prompt: &PromptDocument,
        mode: SummaryMode,
        handle: &JobHandle,
        stream_sink: Option<RequestId>,
        now: TimestampMillis,
    ) -> Result<CompanionMemorySummaryResult, CompanionMemoryInferenceError> {
        let run = self
            .repository
            .load_dynamic_memory_run(run_id)
            .map_err(CompanionMemoryInferenceError::Run)?;
        let attempt = self
            .repository
            .load_dynamic_memory_attempt(attempt_id)
            .map_err(CompanionMemoryInferenceError::Run)?;
        if attempt.run_id != run.id {
            return Err(CompanionMemoryInferenceError::InvalidOwnership {
                reason: "attempt belongs to another run",
            });
        }
        if attempt.job_id != handle.id() {
            return Err(CompanionMemoryInferenceError::InvalidOwnership {
                reason: "attempt belongs to another job",
            });
        }
        if attempt.status != lettuce_memory::DynamicMemoryAttemptStatus::Processing {
            return Err(CompanionMemoryInferenceError::InvalidOwnership {
                reason: "attempt is not processing",
            });
        }
        if let Some(checkpoint) = self
            .repository
            .load_dynamic_memory_summary_checkpoint(run.id)
            .map_err(CompanionMemoryInferenceError::Run)?
        {
            return Ok(CompanionMemorySummaryResult {
                checkpoint,
                replayed: true,
            });
        }
        if handle.cancellation_token().is_cancelled() {
            return Err(CompanionMemoryInferenceError::Cancelled);
        }
        let aggregate = self
            .conversations
            .get(run.conversation_id)
            .map_err(CompanionMemoryInferenceError::Conversation)?;
        let persona_name =
            crate::companion::companion_memory_inference::transcript_persona_name(
                &aggregate.conversation,
            );
        let memory = self
            .repository
            .get(run.space_id)
            .map_err(CompanionMemoryInferenceError::Memory)?
            .ok_or(CompanionMemoryInferenceError::InvalidOwnership {
                reason: "memory space of the run does not exist",
            })?;
        let previous = self
            .repository
            .get_summary_for_branch(run.space_id, run.conversation_id, run.branch_id)
            .map_err(CompanionMemoryInferenceError::Memory)?;
        if window_already_summarized(
            self.repository
                .summary_cursor(run.space_id, run.conversation_id, run.branch_id)
                .map_err(CompanionMemoryInferenceError::Memory)?,
            run.summary_window,
            window_selection,
        ) {
            return Err(CompanionMemoryInferenceError::InvalidOwnership {
                reason: "the summary window is already summarized",
            });
        }
        let sources = materialize_sources(self.conversations, &run)?;
        let runtime_text = crate::generation::runtime_text::RuntimeText::load(
            self.repository,
            crate::BuiltInPromptId::MemoryRuntime,
        )
        .map_err(|error| CompanionMemoryInferenceError::InvalidPrompt {
            reason: format!("memory runtime prompt text could not be loaded: {error}"),
        })?;
        let is_group = aggregate.conversation.kind.is_group();
        if let SummaryMode::Chapters {
            token_budget,
            chapter_size,
        } = mode
        {
            return self
                .run_chapters(
                    &run,
                    &attempt,
                    &memory,
                    previous.as_ref(),
                    &sources,
                    &runtime_text,
                    persona_name.as_deref(),
                    is_group,
                    token_budget,
                    chapter_size,
                    handle,
                    stream_sink,
                    now,
                )
                .await;
        }
        let fallback_request = runtime_text
            .render_with("summary_fallback_request", [])
            .map_err(|error| CompanionMemoryInferenceError::InvalidPrompt {
                reason: format!("summary fallback request could not be rendered: {error}"),
            })?;
        let request = build_summary_request(
            &run,
            &attempt,
            prompt,
            &runtime_text,
            previous.as_ref().map(|summary| summary.text.as_str()),
            &sources,
            persona_name.as_deref(),
            is_group,
            aggregate.conversation.participants.len(),
            handle,
            stream_sink,
            now,
        )?;
        let (text, request_context, usage, provider_request_id) = self
            .infer_summary(request, fallback_request, handle, now)
            .await?;
        if handle.cancellation_token().is_cancelled() {
            return Err(CompanionMemoryInferenceError::Cancelled);
        }
        let token_count = self.count_tokens(&text)?;
        let checkpoint = self
            .repository
            .commit_dynamic_memory_summary(
                DynamicMemorySummaryCommit {
                    run_id: run.id,
                    attempt_id: attempt.id,
                    expected_memory_revision: memory.revision,
                    text,
                    token_count,
                    request_context,
                    usage,
                    provider_request_id,
                },
                now,
            )
            .map_err(CompanionMemoryInferenceError::Run)?;
        Ok(CompanionMemorySummaryResult {
            checkpoint,
            replayed: false,
        })
    }

    fn count_tokens(&self, text: &str) -> Result<u32, CompanionMemoryInferenceError> {
        self.engine.count_tokens(text).map_err(|error| match error {
            crate::EmbeddingGenerationError::Unavailable => {
                CompanionMemoryInferenceError::UnknownTokenCount
            }
            crate::EmbeddingGenerationError::Cancelled => CompanionMemoryInferenceError::Cancelled,
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_chapters(
        &self,
        run: &lettuce_memory::DynamicMemoryRun,
        attempt: &lettuce_memory::DynamicMemoryAttempt,
        memory: &lettuce_memory::MemorySpaceSnapshot,
        previous: Option<&lettuce_memory::MemorySummary>,
        sources: &[crate::companion::companion_memory_inference::MaterializedSource],
        runtime_text: &crate::generation::runtime_text::RuntimeText,
        persona_name: Option<&str>,
        is_group: bool,
        token_budget: u32,
        chapter_size: u32,
        handle: &JobHandle,
        stream_sink: Option<RequestId>,
        now: TimestampMillis,
    ) -> Result<CompanionMemorySummaryResult, CompanionMemoryInferenceError> {
        let scope = MemorySummaryUnitScope {
            space_id: run.space_id,
            conversation_id: run.conversation_id,
            branch_id: run.branch_id,
        };
        let mut active = self
            .repository
            .active_summary_units(scope, Some(run.id))
            .map_err(CompanionMemoryInferenceError::Memory)?;

        if !active.iter().any(|unit| unit.created_by_run_id == run.id) {
            if let Some(published) = previous {
                let parts: Vec<MemorySummaryUnit> = active
                    .iter()
                    .filter(|unit| unit.kind != MemorySummaryUnitKind::Reset)
                    .cloned()
                    .collect();
                if let Some(adoption) =
                    plan_summary_adoption(&parts, &published.text, run.summary_window.start)
                {
                    let token_count = self.count_tokens(&adoption.text)?;
                    let inputs: Vec<_> = if adoption.covers_all(parts.len()) {
                        active.iter().map(|unit| unit.id).collect()
                    } else {
                        parts[adoption.start..adoption.end]
                            .iter()
                            .map(|unit| unit.id)
                            .collect()
                    };
                    self.repository
                        .insert_summary_unit(
                            NewMemorySummaryUnit {
                                id: MemorySummaryUnitId::new(),
                                scope,
                                created_by_run_id: run.id,
                                kind: adoption.kind,
                                level: adoption.level,
                                origin: published.origin,
                                window_start: adoption.window_start,
                                window_end: adoption.window_end,
                                time_start: adoption.time_start,
                                time_end: adoption.time_end,
                                text: adoption.text,
                                token_count,
                                created_at: now,
                            },
                            &inputs,
                        )
                        .map_err(CompanionMemoryInferenceError::Memory)?;
                    active = self
                        .repository
                        .active_summary_units(scope, Some(run.id))
                        .map_err(CompanionMemoryInferenceError::Memory)?;
                }
            } else if active
                .iter()
                .any(|unit| unit.kind != MemorySummaryUnitKind::Reset)
            {
                let inputs: Vec<_> = active.iter().map(|unit| unit.id).collect();
                self.repository
                    .insert_summary_unit(
                        NewMemorySummaryUnit {
                            id: MemorySummaryUnitId::new(),
                            scope,
                            created_by_run_id: run.id,
                            kind: MemorySummaryUnitKind::Reset,
                            level: 0,
                            origin: MemoryOrigin::Model,
                            window_start: 0,
                            window_end: 0,
                            time_start: None,
                            time_end: None,
                            text: String::new(),
                            token_count: 0,
                            created_at: now,
                        },
                        &inputs,
                    )
                    .map_err(CompanionMemoryInferenceError::Memory)?;
                active = self
                    .repository
                    .active_summary_units(scope, Some(run.id))
                    .map_err(CompanionMemoryInferenceError::Memory)?;
            }
        }

        let has_cycle = active.iter().any(|unit| {
            unit.created_by_run_id == run.id
                && matches!(
                    unit.kind,
                    MemorySummaryUnitKind::Cycle | MemorySummaryUnitKind::Chapter
                )
                && unit.window_end >= run.summary_window.end
        });
        let earlier = render_summary_units(&active);
        let cycle_request = build_cycle_request(
            run,
            attempt,
            runtime_text,
            (!earlier.trim().is_empty()).then_some(earlier.as_str()),
            sources,
            persona_name,
            is_group,
            handle,
            stream_sink,
            now,
        )?;
        let (request_context, usage, provider_request_id) = if has_cycle {
            (cycle_request.context, None, None)
        } else {
            let fallback_request = runtime_text
                .render_with("summary_cycle_fallback_request", [])
                .map_err(|error| CompanionMemoryInferenceError::InvalidPrompt {
                    reason: format!(
                        "cycle summary fallback request could not be rendered: {error}"
                    ),
                })?;
            let (cycle_text, request_context, usage, provider_request_id) = self
                .infer_summary(cycle_request, fallback_request, handle, now)
                .await?;
            if handle.cancellation_token().is_cancelled() {
                return Err(CompanionMemoryInferenceError::Cancelled);
            }
            let cycle_text = validate_summary_text(&cycle_text).map_err(|reason| {
                CompanionMemoryInferenceError::NoToolCalls {
                    reason: reason.to_owned(),
                }
            })?;
            let token_count = self.count_tokens(&cycle_text)?;
            let time_start = run
                .source_messages
                .iter()
                .map(|message| message.effective_time)
                .min();
            let time_end = run
                .source_messages
                .iter()
                .map(|message| message.effective_time)
                .max();
            self.repository
                .insert_summary_unit(
                    NewMemorySummaryUnit {
                        id: MemorySummaryUnitId::new(),
                        scope,
                        created_by_run_id: run.id,
                        kind: MemorySummaryUnitKind::Cycle,
                        level: 0,
                        origin: MemoryOrigin::Model,
                        window_start: run.summary_window.start,
                        window_end: run.summary_window.end,
                        time_start,
                        time_end,
                        text: cycle_text,
                        token_count,
                        created_at: now,
                    },
                    &[],
                )
                .map_err(CompanionMemoryInferenceError::Memory)?;
            active = self
                .repository
                .active_summary_units(scope, Some(run.id))
                .map_err(CompanionMemoryInferenceError::Memory)?;
            (request_context, usage, provider_request_id)
        };

        loop {
            let active_tokens: u64 = active
                .iter()
                .filter(|unit| {
                    matches!(
                        unit.kind,
                        MemorySummaryUnitKind::Cycle | MemorySummaryUnitKind::Chapter
                    )
                })
                .map(|unit| u64::from(unit.token_count))
                .sum();
            if active_tokens <= u64::from(token_budget) {
                break;
            }
            let Some((level, group_indices)) = select_compaction_group(&active, chapter_size)
            else {
                break;
            };
            let group: Vec<&MemorySummaryUnit> =
                group_indices.iter().map(|&idx| &active[idx]).collect();
            let window_start = group[0].window_start;
            let window_end = group[group.len() - 1].window_end;
            let next_level = level.saturating_add(1);
            let group_tokens: u64 = group.iter().map(|unit| u64::from(unit.token_count)).sum();
            let word_limit = chapter_word_limit(group_tokens, token_budget);
            let chapter_fallback_request = runtime_text
                .render_with(
                    "summary_chapter_fallback_request",
                    [(PromptVariable::TargetCount, word_limit.to_string())],
                )
                .map_err(|error| CompanionMemoryInferenceError::InvalidPrompt {
                    reason: format!(
                        "chapter summary fallback request could not be rendered: {error}"
                    ),
                })?;
            let reference = group_indices[0]
                .checked_sub(1)
                .and_then(|idx| active.get(idx))
                .filter(|unit| unit.kind != MemorySummaryUnitKind::Reset)
                .map(|unit| unit.text.as_str());
            let chapter_request = match build_chapter_request(
                run,
                attempt,
                runtime_text,
                reference,
                &group,
                next_level,
                window_start,
                window_end,
                word_limit,
                is_group,
                handle,
                stream_sink,
                now,
            ) {
                Ok(request) => request,
                Err(error) => {
                    tracing::warn!(run_id = %run.id, ?error, "chapter summary request build failed");
                    break;
                }
            };
            let chapter_text = match self
                .infer_summary(chapter_request, chapter_fallback_request, handle, now)
                .await
            {
                Ok((text, _, _, _)) => text,
                Err(CompanionMemoryInferenceError::Cancelled) => {
                    return Err(CompanionMemoryInferenceError::Cancelled);
                }
                Err(error) => {
                    tracing::warn!(run_id = %run.id, ?error, "chapter summary inference failed");
                    break;
                }
            };
            if handle.cancellation_token().is_cancelled() {
                return Err(CompanionMemoryInferenceError::Cancelled);
            }
            let chapter_text = match validate_summary_text(&chapter_text) {
                Ok(text) => text,
                Err(reason) => {
                    tracing::warn!(run_id = %run.id, reason, "chapter summary validation failed");
                    break;
                }
            };
            let chapter_tokens = match self.count_tokens(&chapter_text) {
                Ok(tokens) => tokens,
                Err(CompanionMemoryInferenceError::Cancelled) => {
                    return Err(CompanionMemoryInferenceError::Cancelled);
                }
                Err(error) => {
                    tracing::warn!(run_id = %run.id, ?error, "chapter summary token count failed");
                    break;
                }
            };
            if u64::from(chapter_tokens) >= group_tokens {
                tracing::warn!(
                    run_id = %run.id,
                    chapter_tokens,
                    group_tokens,
                    "chapter summary was not shorter than its inputs"
                );
                break;
            }
            let input_ids: Vec<_> = group.iter().map(|unit| unit.id).collect();
            let (time_start, time_end) = combined_time_range(&group)
                .map_or((None, None), |(start, end)| (Some(start), Some(end)));
            self.repository
                .insert_summary_unit(
                    NewMemorySummaryUnit {
                        id: MemorySummaryUnitId::new(),
                        scope,
                        created_by_run_id: run.id,
                        kind: MemorySummaryUnitKind::Chapter,
                        level: next_level,
                        origin: MemoryOrigin::Model,
                        window_start,
                        window_end,
                        time_start,
                        time_end,
                        text: chapter_text,
                        token_count: chapter_tokens,
                        created_at: now,
                    },
                    &input_ids,
                )
                .map_err(CompanionMemoryInferenceError::Memory)?;
            active = self
                .repository
                .active_summary_units(scope, Some(run.id))
                .map_err(CompanionMemoryInferenceError::Memory)?;
        }

        let rendered = render_summary_units(&active);
        let token_count = self.count_tokens(&rendered)?;
        let checkpoint = self
            .repository
            .commit_dynamic_memory_summary(
                DynamicMemorySummaryCommit {
                    run_id: run.id,
                    attempt_id: attempt.id,
                    expected_memory_revision: memory.revision,
                    text: rendered,
                    token_count,
                    request_context,
                    usage,
                    provider_request_id,
                },
                now,
            )
            .map_err(CompanionMemoryInferenceError::Run)?;
        Ok(CompanionMemorySummaryResult {
            checkpoint,
            replayed: false,
        })
    }

    async fn infer_summary(
        &self,
        request: InferenceRequest,
        fallback_request: String,
        handle: &JobHandle,
        now: TimestampMillis,
    ) -> Result<
        (
            String,
            ProviderNeutralContext,
            Option<lettuce_conversations::InferenceUsage>,
            Option<String>,
        ),
        CompanionMemoryInferenceError,
    > {
        use crate::jobs::job_inference_usage::{JobInferenceError, run_job_inference};

        let primary_context = request.context.clone();
        let primary = match run_job_inference(
            self.repository,
            self.inference,
            handle.id(),
            request.clone(),
            now,
        )
        .await
        {
            Ok(outcome) => Some(outcome),
            Err(JobInferenceError::Provider(PortError::Cancelled)) => {
                return Err(CompanionMemoryInferenceError::Cancelled);
            }
            Err(JobInferenceError::Evidence { reason }) => {
                tracing::warn!(%reason, "memory summary inference usage evidence failed");
                return Err(CompanionMemoryInferenceError::Run(
                    lettuce_memory::DynamicMemoryRunRepositoryError::Storage {
                        reason: "summary inference usage evidence could not be recorded",
                    },
                ));
            }
            Err(JobInferenceError::Provider(_)) => None,
        };
        if let Some(outcome) = &primary {
            if outcome.finish_reason == FinishReason::Cancelled {
                cleanup_outcome_replays(self.repository, outcome)?;
                return Err(CompanionMemoryInferenceError::Cancelled);
            }
            if let Ok(text) = summary_from_outcome(outcome, true) {
                cleanup_outcome_replays(self.repository, outcome)?;
                return Ok((
                    text,
                    primary_context,
                    outcome.usage.clone(),
                    outcome.provider_request_id.clone(),
                ));
            }
        }
        let primary_usage = primary.as_ref().and_then(|outcome| outcome.usage.clone());
        if let Some(outcome) = &primary {
            cleanup_outcome_replays(self.repository, outcome)?;
        }
        if handle.cancellation_token().is_cancelled() {
            return Err(CompanionMemoryInferenceError::Cancelled);
        }
        let mut fallback = request;
        fallback.profile.tool_policy = ToolPolicy::Disabled;
        fallback.profile.output_policy = OutputPolicy::Plain;
        fallback.tools = None;
        fallback.context.messages.push(ProviderNeutralMessage {
            role: MessageRole::User,
            parts: vec![ProviderContextPart::Text {
                text: fallback_request,
            }],
        });
        fallback
            .validate()
            .map_err(|error| CompanionMemoryInferenceError::InvalidPrompt {
                reason: format!("summary fallback request failed validation: {error}"),
            })?;
        let fallback_context = fallback.context.clone();
        let outcome =
            run_job_inference(self.repository, self.inference, handle.id(), fallback, now)
                .await
                .map_err(|error| match error {
                    JobInferenceError::Provider(PortError::Cancelled) => {
                        CompanionMemoryInferenceError::Cancelled
                    }
                    JobInferenceError::Provider(error) => {
                        CompanionMemoryInferenceError::Inference(error)
                    }
                    JobInferenceError::Evidence { reason } => {
                        tracing::warn!(%reason, "memory summary fallback usage evidence failed");
                        CompanionMemoryInferenceError::Run(
                            lettuce_memory::DynamicMemoryRunRepositoryError::Storage {
                                reason: "summary fallback usage evidence could not be recorded",
                            },
                        )
                    }
                })?;
        let text = match summary_from_outcome(&outcome, false) {
            Ok(text) => text,
            Err(error) => {
                cleanup_outcome_replays(self.repository, &outcome)?;
                return Err(error);
            }
        };
        let usage = aggregate_usage(primary_usage, outcome.usage.clone());
        let provider_request_id = outcome.provider_request_id.clone();
        cleanup_outcome_replays(self.repository, &outcome)?;
        Ok((text, fallback_context, usage, provider_request_id))
    }
}

/// Whether a newer run already summarized past this run's window, so its
/// summary must not replace the newer one. An automatic window starts at the
/// cursor it was admitted at; a forced recent window may re-cover summarized
/// messages and is stale only once the cursor passed its end.
fn window_already_summarized(
    cursor: u64,
    window: lettuce_memory::DynamicMemorySummaryWindow,
    selection: crate::CompanionMemoryWindowSelection,
) -> bool {
    match selection {
        crate::CompanionMemoryWindowSelection::Automatic => cursor > window.start,
        crate::CompanionMemoryWindowSelection::Recent => cursor > window.end,
    }
}

fn summary_tool_request(description: String, parameter: String) -> ToolRequest {
    ToolRequest {
        definitions: vec![ToolDefinition {
            name: "write_summary".to_owned(),
            description: Some(description),
            parameters: json!({
                "type": "object",
                "properties": {
                    "summary": { "type": "string", "description": parameter }
                },
                "required": ["summary"]
            }),
            version: 1,
        }],
        choice: ToolChoice::Required,
    }
}

/// The speaker name for a summarizer transcript line: the active persona for
/// the user's own messages (so the summary names them instead of "the user"),
/// otherwise the structural user label or the character side.
fn summary_speaker_name<'a>(
    role: MessageRole,
    persona_name: Option<&'a str>,
    user_label: &'a str,
    character_label: &'a str,
) -> &'a str {
    match role {
        MessageRole::User => persona_name.unwrap_or(user_label),
        _ => character_label,
    }
}

/// The content line for a direct-chat summarizer message: the user's own
/// messages are prefixed with the active persona name so the summary names them
/// instead of "the user". Other roles pass through unchanged.
fn summary_direct_content(role: MessageRole, persona_name: Option<&str>, body: &str) -> String {
    match (role, persona_name) {
        (MessageRole::User, Some(name)) => format!("{name}: {body}"),
        _ => body.to_owned(),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_summary_request(
    run: &lettuce_memory::DynamicMemoryRun,
    attempt: &lettuce_memory::DynamicMemoryAttempt,
    prompt: &PromptDocument,
    text: &crate::generation::runtime_text::RuntimeText,
    previous_summary: Option<&str>,
    sources: &[crate::companion::companion_memory_inference::MaterializedSource],
    persona_name: Option<&str>,
    is_group: bool,
    participant_count: usize,
    handle: &JobHandle,
    stream_sink: Option<RequestId>,
    now: TimestampMillis,
) -> Result<InferenceRequest, CompanionMemoryInferenceError> {
    if prompt.status != LifecycleStatus::Active {
        return Err(CompanionMemoryInferenceError::InvalidPrompt {
            reason: "summarizer prompt is not active".to_owned(),
        });
    }
    if prompt.purpose != PromptPurpose::DynamicMemorySummarizer {
        return Err(CompanionMemoryInferenceError::InvalidPrompt {
            reason: "prompt purpose is not dynamic memory summarizer".to_owned(),
        });
    }
    if sources.len() != run.source_messages.len() {
        return Err(CompanionMemoryInferenceError::InvalidPrompt {
            reason: "materialized sources differ from the run source messages".to_owned(),
        });
    }
    let fragment = |key: &str, variables: Vec<(PromptVariable, String)>| {
        text.render_with(key, variables)
            .map_err(|error| CompanionMemoryInferenceError::InvalidPrompt {
                reason: format!("summary prompt fragment {key} could not be rendered: {error}"),
            })
    };
    let group_variant = |direct: &'static str, group: &'static str| {
        if is_group { group } else { direct }
    };
    let no_previous = fragment("summary_no_previous", Vec::new())?;
    let previous = previous_summary
        .filter(|summary| !summary.trim().is_empty())
        .unwrap_or(&no_previous);
    let transcript = sources
        .iter()
        .map(|source| {
            let role = role_name(source.role);
            if run.time_awareness_enabled {
                let timestamp = format_message_timestamp(source.effective_time);
                if source.text.is_empty() {
                    format!("[message:{}] {role}: {timestamp}", source.message_id)
                } else {
                    format!(
                        "[message:{}] {role}: {timestamp} {}",
                        source.message_id, source.text
                    )
                }
            } else {
                format!("{role}: {}", source.text)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut values = PromptRenderValues::default();
    values
        .purpose_values
        .insert(PromptVariable::PreviousSummary, previous.to_owned());
    if run.time_awareness_enabled {
        insert_time_values(&mut values, now);
    }
    let rendered = render_prompt(
        prompt,
        &PromptRenderContext {
            conditions: lettuce_context::PromptConditionContext {
                chat_mode: if is_group {
                    PromptEntryChatMode::Group
                } else {
                    PromptEntryChatMode::Direct
                },
                info_source: PromptEntryInfoSource::Messages,
                message_count: sources.len(),
                participant_count,
                recent_text: transcript,
                dynamic_memory_enabled: true,
                has_memory_summary: previous_summary.is_some_and(|value| !value.trim().is_empty()),
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
                companion_mode_enabled: run.supersession_enabled,
                time_awareness_enabled: run.time_awareness_enabled,
                ..Default::default()
            },
            values,
        },
    )
    .map_err(CompanionMemoryInferenceError::Prompt)?;
    let mut messages = rendered
        .relative
        .iter()
        .map(rendered_message)
        .collect::<Result<Vec<_>, _>>()?;
    let user_label = fragment("summary_group_user_label", Vec::new())?;
    let character_label = fragment("summary_group_character_label", Vec::new())?;
    for source in sources {
        let content = if is_group {
            fragment(
                "summary_group_line",
                vec![
                    (
                        PromptVariable::SpeakerName,
                        summary_speaker_name(
                            source.role,
                            persona_name,
                            &user_label,
                            &character_label,
                        )
                        .to_owned(),
                    ),
                    (PromptVariable::MessageText, source.text.clone()),
                ],
            )?
        } else if run.time_awareness_enabled {
            let timestamp = format_message_timestamp(source.effective_time);
            let body = if source.text.is_empty() {
                timestamp
            } else {
                format!("{timestamp} {}", source.text)
            };
            summary_direct_content(source.role, persona_name, &body)
        } else {
            summary_direct_content(source.role, persona_name, &source.text)
        };
        messages.push(ProviderNeutralMessage {
            role: source.role,
            parts: vec![ProviderContextPart::Text { text: content }],
        });
    }
    let mut in_chat = rendered
        .in_chat
        .iter()
        .map(|entry| rendered_message(entry).map(|message| (entry.depth, message)))
        .collect::<Result<Vec<_>, _>>()?;
    in_chat.push((
        0,
        ProviderNeutralMessage {
            role: MessageRole::User,
            parts: vec![ProviderContextPart::Text {
                text: fragment(
                    group_variant("summary_output_request", "summary_output_request_group"),
                    Vec::new(),
                )?,
            }],
        },
    ));
    insert_in_chat_messages(&mut messages, in_chat);
    let input_bytes = messages
        .iter()
        .flat_map(|message| &message.parts)
        .try_fold(0usize, |total, part| match part {
            ProviderContextPart::Text { text } => total.checked_add(text.len()),
            _ => None,
        })
        .and_then(|size| u32::try_from(size).ok())
        .ok_or(CompanionMemoryInferenceError::ContextTooLarge {
            reason: "prompt text exceeds the byte limit",
        })?;
    let mut profile = run.profile.clone();
    profile.tool_policy = ToolPolicy::Required;
    profile.output_policy = OutputPolicy::Plain;
    let request = InferenceRequest {
        turn_id: GenerationTurnId::from_uuid(Uuid::new_v5(&run.id.as_uuid(), b"summary")),
        attempt_id: GenerationAttemptId::from_uuid(Uuid::new_v5(&attempt.id.as_uuid(), b"summary")),
        operation: GenerationOperation::Send,
        profile,
        context: ProviderNeutralContext {
            messages,
            attributions: ContextAttributions {
                usage_snapshot: Some(lettuce_conversations::UsageRecordSnapshot {
                    operation_kind: Some("summary".into()),
                    ..Default::default()
                }),
                prompt: Some(PromptAttribution {
                    document_id: prompt.id,
                    name: prompt.name.clone(),
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
                selected_messages: u32::try_from(sources.len())
                    .map_err(|_| CompanionMemoryInferenceError::ContextTooLarge {
                        reason: "source message count does not fit in u32",
                    })?,
                omitted_messages: 0,
                input_bytes,
                estimated_input_tokens: input_bytes.saturating_add(3) / 4,
                truncated: false,
            },
        },
        cancellation: Some(handle.id()),
        stream_sink,
        media_grants: Vec::new(),
        tools: Some(summary_tool_request(
            fragment(
                group_variant("summary_tool", "summary_tool_group"),
                Vec::new(),
            )?,
            fragment(
                group_variant("summary_parameter", "summary_parameter_group"),
                Vec::new(),
            )?,
        )),
        prompt_cache_key: None,
    };
    request
        .validate()
        .map_err(|error| CompanionMemoryInferenceError::InvalidPrompt {
            reason: format!("summary request failed validation: {error}"),
        })?;
    Ok(request)
}

fn chapter_word_limit(group_tokens: u64, token_budget: u32) -> u64 {
    let tokens = (group_tokens / 2).min(u64::from(token_budget) / 2).max(1);
    (tokens * 3 / 4).max(1)
}

fn select_compaction_group(
    active: &[MemorySummaryUnit],
    chapter_size: u32,
) -> Option<(u32, Vec<usize>)> {
    let mut by_level: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for (index, unit) in active.iter().enumerate() {
        if matches!(
            unit.kind,
            MemorySummaryUnitKind::Cycle | MemorySummaryUnitKind::Chapter
        ) {
            by_level.entry(unit.level).or_default().push(index);
        }
    }
    let target = usize::try_from(chapter_size).unwrap_or(usize::MAX).max(2);
    if let Some((&level, indices)) = by_level.iter().find(|(_, indices)| indices.len() >= target) {
        return Some((level, indices[..target].to_vec()));
    }
    if let Some((&level, indices)) = by_level.iter().find(|(_, indices)| indices.len() >= 2) {
        return Some((level, indices[..indices.len().min(target)].to_vec()));
    }
    let mut compactable: Vec<usize> = by_level.into_values().flatten().collect();
    compactable.sort_unstable();
    if let [first, second, ..] = compactable[..] {
        return Some((
            active[first].level.max(active[second].level),
            vec![first, second],
        ));
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn build_cycle_request(
    run: &lettuce_memory::DynamicMemoryRun,
    attempt: &lettuce_memory::DynamicMemoryAttempt,
    text: &crate::generation::runtime_text::RuntimeText,
    earlier_summary: Option<&str>,
    sources: &[crate::companion::companion_memory_inference::MaterializedSource],
    persona_name: Option<&str>,
    is_group: bool,
    handle: &JobHandle,
    stream_sink: Option<RequestId>,
    now: TimestampMillis,
) -> Result<InferenceRequest, CompanionMemoryInferenceError> {
    if sources.len() != run.source_messages.len() {
        return Err(CompanionMemoryInferenceError::InvalidPrompt {
            reason: "materialized sources differ from the run source messages".to_owned(),
        });
    }
    let fragment = |key: &str, variables: Vec<(PromptVariable, String)>| {
        text.render_with(key, variables)
            .map_err(|error| CompanionMemoryInferenceError::InvalidPrompt {
                reason: format!("summary prompt fragment {key} could not be rendered: {error}"),
            })
    };
    let group_variant = |direct: &'static str, group: &'static str| {
        if is_group { group } else { direct }
    };

    let mut system = fragment("summary_cycle_system", Vec::new())?;
    if run.time_awareness_enabled {
        let mut time_values = PromptRenderValues::default();
        insert_time_values(&mut time_values, now);
        let time_rules = fragment(
            "summary_cycle_time_rules",
            time_values.purpose_values.into_iter().collect(),
        )?;
        system.push_str("\n\n");
        system.push_str(&time_rules);
    }

    let earlier_label = fragment("summary_cycle_earlier_label", Vec::new())?;
    let earlier_body = match earlier_summary.filter(|summary| !summary.trim().is_empty()) {
        Some(summary) => summary.to_owned(),
        None => fragment("summary_cycle_no_earlier", Vec::new())?,
    };
    let transcript_label = fragment("summary_cycle_transcript_label", Vec::new())?;
    let user_label = fragment("summary_group_user_label", Vec::new())?;
    let character_label = fragment("summary_group_character_label", Vec::new())?;
    let output_request = fragment("summary_cycle_output_request", Vec::new())?;

    let mut messages = vec![
        ProviderNeutralMessage {
            role: MessageRole::System,
            parts: vec![ProviderContextPart::Text { text: system }],
        },
        ProviderNeutralMessage {
            role: MessageRole::User,
            parts: vec![ProviderContextPart::Text {
                text: format!("{earlier_label}\n{earlier_body}"),
            }],
        },
        ProviderNeutralMessage {
            role: MessageRole::User,
            parts: vec![ProviderContextPart::Text {
                text: transcript_label,
            }],
        },
    ];
    for source in sources {
        let content = if is_group {
            fragment(
                "summary_group_line",
                vec![
                    (
                        PromptVariable::SpeakerName,
                        summary_speaker_name(
                            source.role,
                            persona_name,
                            &user_label,
                            &character_label,
                        )
                        .to_owned(),
                    ),
                    (PromptVariable::MessageText, source.text.clone()),
                ],
            )?
        } else if run.time_awareness_enabled {
            let timestamp = format_message_timestamp(source.effective_time);
            let body = if source.text.is_empty() {
                timestamp
            } else {
                format!("{timestamp} {}", source.text)
            };
            summary_direct_content(source.role, persona_name, &body)
        } else {
            summary_direct_content(source.role, persona_name, &source.text)
        };
        messages.push(ProviderNeutralMessage {
            role: source.role,
            parts: vec![ProviderContextPart::Text { text: content }],
        });
    }
    messages.push(ProviderNeutralMessage {
        role: MessageRole::User,
        parts: vec![ProviderContextPart::Text {
            text: output_request,
        }],
    });
    finalize_unit_request(
        run,
        messages,
        sources.len(),
        GenerationTurnId::from_uuid(Uuid::new_v5(&run.id.as_uuid(), b"summary")),
        GenerationAttemptId::from_uuid(Uuid::new_v5(&attempt.id.as_uuid(), b"summary")),
        fragment(
            group_variant("summary_tool", "summary_tool_group"),
            Vec::new(),
        )?,
        fragment(
            group_variant("summary_parameter", "summary_parameter_group"),
            Vec::new(),
        )?,
        handle,
        stream_sink,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_chapter_request(
    run: &lettuce_memory::DynamicMemoryRun,
    attempt: &lettuce_memory::DynamicMemoryAttempt,
    text: &crate::generation::runtime_text::RuntimeText,
    reference: Option<&str>,
    group: &[&MemorySummaryUnit],
    level: u32,
    window_start: u64,
    window_end: u64,
    word_limit: u64,
    is_group: bool,
    handle: &JobHandle,
    stream_sink: Option<RequestId>,
    now: TimestampMillis,
) -> Result<InferenceRequest, CompanionMemoryInferenceError> {
    let fragment = |key: &str| {
        text.render_with(key, [])
            .map_err(|error| CompanionMemoryInferenceError::InvalidPrompt {
                reason: format!("summary prompt fragment {key} could not be rendered: {error}"),
            })
    };
    let group_variant = |direct: &'static str, group_key: &'static str| {
        if is_group { group_key } else { direct }
    };

    let mut system = fragment("summary_chapter_system")?;
    if run.time_awareness_enabled {
        let mut time_values = PromptRenderValues::default();
        insert_time_values(&mut time_values, now);
        let time_rules = text
            .render_with(
                "summary_chapter_time_rules",
                time_values.purpose_values.into_iter().collect::<Vec<_>>(),
            )
            .map_err(|error| CompanionMemoryInferenceError::InvalidPrompt {
                reason: format!(
                    "summary prompt fragment summary_chapter_time_rules could not be rendered: {error}"
                ),
            })?;
        system.push_str("\n\n");
        system.push_str(&time_rules);
    }
    let reference_label = fragment("summary_chapter_reference_label")?;
    let reference_body = match reference.filter(|value| !value.trim().is_empty()) {
        Some(value) => value.to_owned(),
        None => fragment("summary_chapter_no_reference")?,
    };
    let entries_label = fragment("summary_chapter_entries_label")?;
    let entries_body = group
        .iter()
        .enumerate()
        .map(|(index, unit)| {
            if run.time_awareness_enabled
                && let (Some(start), Some(end)) = (unit.time_start, unit.time_end)
            {
                let start = format_message_timestamp(start);
                let end = format_message_timestamp(end);
                if start == end {
                    format!("[{}] {start} {}", index + 1, unit.text)
                } else {
                    format!("[{}] {start} to {end} {}", index + 1, unit.text)
                }
            } else {
                format!("[{}] {}", index + 1, unit.text)
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let output_request = text
        .render_with(
            "summary_chapter_output_request",
            [(PromptVariable::TargetCount, word_limit.to_string())],
        )
        .map_err(|error| CompanionMemoryInferenceError::InvalidPrompt {
            reason: format!(
                "summary prompt fragment summary_chapter_output_request could not be rendered: {error}"
            ),
        })?;

    let messages = vec![
        ProviderNeutralMessage {
            role: MessageRole::System,
            parts: vec![ProviderContextPart::Text { text: system }],
        },
        ProviderNeutralMessage {
            role: MessageRole::User,
            parts: vec![ProviderContextPart::Text {
                text: format!("{reference_label}\n{reference_body}"),
            }],
        },
        ProviderNeutralMessage {
            role: MessageRole::User,
            parts: vec![ProviderContextPart::Text {
                text: format!("{entries_label}\n{entries_body}"),
            }],
        },
        ProviderNeutralMessage {
            role: MessageRole::User,
            parts: vec![ProviderContextPart::Text {
                text: output_request,
            }],
        },
    ];
    let key = format!("summary-chapter:{level}:{window_start}:{window_end}");
    finalize_unit_request(
        run,
        messages,
        group.len(),
        GenerationTurnId::from_uuid(Uuid::new_v5(&run.id.as_uuid(), key.as_bytes())),
        GenerationAttemptId::from_uuid(Uuid::new_v5(&attempt.id.as_uuid(), key.as_bytes())),
        fragment(group_variant("summary_tool", "summary_tool_group"))?,
        fragment(group_variant("summary_parameter", "summary_parameter_group"))?,
        handle,
        stream_sink,
    )
}

#[allow(clippy::too_many_arguments)]
fn finalize_unit_request(
    run: &lettuce_memory::DynamicMemoryRun,
    messages: Vec<ProviderNeutralMessage>,
    selected_messages: usize,
    turn_id: GenerationTurnId,
    attempt_id: GenerationAttemptId,
    tool_description: String,
    tool_parameter: String,
    handle: &JobHandle,
    stream_sink: Option<RequestId>,
) -> Result<InferenceRequest, CompanionMemoryInferenceError> {
    let input_bytes = messages
        .iter()
        .flat_map(|message| &message.parts)
        .try_fold(0usize, |total, part| match part {
            ProviderContextPart::Text { text } => total.checked_add(text.len()),
            _ => None,
        })
        .and_then(|size| u32::try_from(size).ok())
        .ok_or(CompanionMemoryInferenceError::ContextTooLarge {
            reason: "prompt text exceeds the byte limit",
        })?;
    let mut profile = run.profile.clone();
    profile.tool_policy = ToolPolicy::Required;
    profile.output_policy = OutputPolicy::Plain;
    let request = InferenceRequest {
        turn_id,
        attempt_id,
        operation: GenerationOperation::Send,
        profile,
        context: ProviderNeutralContext {
            messages,
            attributions: ContextAttributions {
                usage_snapshot: Some(lettuce_conversations::UsageRecordSnapshot {
                    operation_kind: Some("summary".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            budget: ContextBudgetReport {
                selected_messages: u32::try_from(selected_messages).map_err(|_| {
                    CompanionMemoryInferenceError::ContextTooLarge {
                        reason: "selected message count does not fit in u32",
                    }
                })?,
                omitted_messages: 0,
                input_bytes,
                estimated_input_tokens: input_bytes.saturating_add(3) / 4,
                truncated: false,
            },
        },
        cancellation: Some(handle.id()),
        stream_sink,
        media_grants: Vec::new(),
        tools: Some(summary_tool_request(tool_description, tool_parameter)),
        prompt_cache_key: None,
    };
    request
        .validate()
        .map_err(|error| CompanionMemoryInferenceError::InvalidPrompt {
            reason: format!("summary request failed validation: {error}"),
        })?;
    Ok(request)
}

fn summary_from_outcome(
    outcome: &InferenceOutcome,
    allow_tool: bool,
) -> Result<String, CompanionMemoryInferenceError> {
    outcome
        .validate()
        .map_err(|error| CompanionMemoryInferenceError::NoToolCalls {
            reason: format!("summary outcome failed validation: {error}"),
        })?;
    if outcome.candidates.len() != 1 {
        return Err(CompanionMemoryInferenceError::MultipleCandidates {
            reason: format!(
                "summary outcome has {} candidates instead of one",
                outcome.candidates.len()
            ),
        });
    }
    if matches!(outcome.finish_reason, FinishReason::Cancelled) {
        return Err(CompanionMemoryInferenceError::Cancelled);
    }
    if matches!(outcome.finish_reason, FinishReason::Error) {
        return Err(CompanionMemoryInferenceError::Inference(
            PortError::Rejected,
        ));
    }
    let candidate = &outcome.candidates[0];
    if allow_tool {
        for call in &candidate.tool_calls {
            if call.name == "write_summary"
                && let Some(summary) = call
                    .arguments
                    .get("summary")
                    .and_then(|value| value.as_str())
                    && let Ok(summary) = validate_summary_text(summary) {
                        return Ok(summary);
                    }
        }
    } else if !candidate.tool_calls.is_empty() {
        return Err(CompanionMemoryInferenceError::NoToolCalls {
            reason: "summary candidate has tool calls where plain text was required".to_owned(),
        });
    }
    let text = candidate
        .parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<String>();
    validate_summary_text(&text).map_err(|reason| CompanionMemoryInferenceError::NoToolCalls {
        reason: reason.to_owned(),
    })
}

fn validate_summary_text(summary: &str) -> Result<String, &'static str> {
    let normalized =
        lettuce_memory::collapse_whitespace(&lettuce_memory::normalize_llm_output_text(summary));
    if normalized.is_empty() {
        return Err("summary text is empty");
    }
    let lower = normalized.to_ascii_lowercase();
    let refusal_prefixes = [
        "i'm sorry",
        "i am sorry",
        "sorry,",
        "sorry but",
        "i can't help",
        "i cannot help",
        "i can't assist",
        "i cannot assist",
        "i can't provide",
        "i cannot provide",
        "i'm unable to",
        "i am unable to",
        "cannot comply",
    ];
    if refusal_prefixes
        .iter()
        .any(|prefix| lower.starts_with(prefix))
        || lower.contains("write_summary")
        || lower.contains("create_memory(")
    {
        return Err("summary text is a refusal or mentions a memory tool");
    }
    Ok(normalized)
}

fn aggregate_usage(
    first: Option<lettuce_conversations::InferenceUsage>,
    second: Option<lettuce_conversations::InferenceUsage>,
) -> Option<lettuce_conversations::InferenceUsage> {
    match (first, second) {
        (Some(first), Some(second)) => Some(lettuce_conversations::InferenceUsage {
            provider_reported_cost: first
                .provider_reported_cost
                .zip(second.provider_reported_cost)
                .and_then(|(a, b)| a.checked_add(b)),
            cache_write_tokens: first
                .cache_write_tokens
                .zip(second.cache_write_tokens)
                .and_then(|(a, b)| a.checked_add(b)),
            web_search_requests: first
                .web_search_requests
                .zip(second.web_search_requests)
                .and_then(|(a, b)| a.checked_add(b)),
            cached_input_tokens: first
                .cached_input_tokens
                .zip(second.cached_input_tokens)
                .and_then(|(a, b)| a.checked_add(b)),
            reasoning_tokens: first
                .reasoning_tokens
                .zip(second.reasoning_tokens)
                .and_then(|(a, b)| a.checked_add(b)),
            image_tokens: first
                .image_tokens
                .zip(second.image_tokens)
                .and_then(|(a, b)| a.checked_add(b)),
            audio_tokens: first
                .audio_tokens
                .zip(second.audio_tokens)
                .and_then(|(a, b)| a.checked_add(b)),
            total_tokens: (first.total_tokens.is_some() || second.total_tokens.is_some()).then(
                || {
                    first
                        .effective_total_tokens()
                        .saturating_add(second.effective_total_tokens())
                },
            ),
            input_tokens: first.input_tokens.saturating_add(second.input_tokens),
            output_tokens: first.output_tokens.saturating_add(second.output_tokens),
        }),
        (Some(usage), None) | (None, Some(usage)) => Some(usage),
        (None, None) => None,
    }
}

pub(crate) fn insert_time_values(values: &mut PromptRenderValues, now: TimestampMillis) {
    let datetime = match Local.timestamp_millis_opt(now.get()) {
        LocalResult::Single(datetime) | LocalResult::Ambiguous(datetime, _) => datetime,
        LocalResult::None => Local::now(),
    };
    for (variable, value) in [
        (
            PromptVariable::DateFull,
            datetime.format("%A, %B %-d, %Y").to_string(),
        ),
        (
            PromptVariable::Time12HourFormat,
            datetime.format("%-I:%M %p").to_string(),
        ),
        (PromptVariable::DatetimeIso, datetime.to_rfc3339()),
        (
            PromptVariable::Date,
            datetime.format("%Y-%m-%d").to_string(),
        ),
        (PromptVariable::Weekday, datetime.weekday().to_string()),
        (PromptVariable::TimeHour, datetime.hour().to_string()),
        (PromptVariable::TimeMinute, datetime.minute().to_string()),
        (PromptVariable::TimeSecond, datetime.second().to_string()),
        (
            PromptVariable::TimeFull,
            datetime.format("%H:%M:%S").to_string(),
        ),
        (
            PromptVariable::TimeTimezone,
            datetime.format("%:z").to_string(),
        ),
        (
            PromptVariable::TimeTimezoneName,
            datetime.format("%Z").to_string(),
        ),
    ] {
        values.purpose_values.insert(variable, value);
    }
}

fn role_name(role: MessageRole) -> &'static str {
    match role {
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        _ => "invalid",
    }
}

#[cfg(test)]
mod tests {
    use lettuce_conversations::{
        FinishReason, InferenceCandidate, InferenceOutcome, MessagePart, ProposedToolCall,
    };
    use serde_json::json;

    use super::{
        chapter_word_limit, select_compaction_group, summary_direct_content, summary_from_outcome,
        summary_speaker_name, validate_summary_text,
    };

    fn unit(
        kind: lettuce_memory::MemorySummaryUnitKind,
        level: u32,
        window: (u64, u64),
    ) -> lettuce_memory::MemorySummaryUnit {
        lettuce_memory::MemorySummaryUnit {
            id: lettuce_types::MemorySummaryUnitId::new(),
            scope: lettuce_memory::MemorySummaryUnitScope {
                space_id: lettuce_types::MemorySpaceId::new(),
                conversation_id: lettuce_types::ConversationId::new(),
                branch_id: lettuce_types::ConversationBranchId::new(),
            },
            created_by_run_id: lettuce_types::DynamicMemoryRunId::new(),
            kind,
            level,
            origin: lettuce_memory::MemoryOrigin::Model,
            window_start: window.0,
            window_end: window.1,
            time_start: None,
            time_end: None,
            text: "Part.".into(),
            token_count: 100,
            created_at: lettuce_types::TimestampMillis::new(1),
        }
    }

    #[test]
    fn compaction_prefers_a_full_level_then_a_pair_then_the_two_oldest_parts() {
        use lettuce_memory::MemorySummaryUnitKind::{Chapter, Cycle, Seed};
        let full = vec![
            unit(Seed, 0, (0, 0)),
            unit(Chapter, 1, (0, 12)),
            unit(Chapter, 1, (12, 24)),
            unit(Cycle, 0, (24, 30)),
            unit(Cycle, 0, (30, 36)),
        ];
        assert_eq!(select_compaction_group(&full, 2), Some((0, vec![3, 4])));
        assert_eq!(select_compaction_group(&full, 4), Some((0, vec![3, 4])));

        let pair = vec![
            unit(Chapter, 1, (0, 12)),
            unit(Chapter, 1, (12, 24)),
            unit(Cycle, 0, (24, 30)),
        ];
        assert_eq!(select_compaction_group(&pair, 4), Some((1, vec![0, 1])));

        let distinct = vec![
            unit(Seed, 0, (0, 0)),
            unit(Chapter, 3, (0, 48)),
            unit(Chapter, 2, (48, 72)),
            unit(Chapter, 1, (72, 84)),
            unit(Cycle, 0, (84, 90)),
        ];
        assert_eq!(select_compaction_group(&distinct, 2), Some((3, vec![1, 2])));

        let single = vec![unit(Seed, 0, (0, 0)), unit(Chapter, 3, (0, 48))];
        assert_eq!(select_compaction_group(&single, 2), None);
    }

    #[test]
    fn chapter_word_limit_is_half_the_inputs_capped_by_half_the_budget() {
        assert_eq!(chapter_word_limit(400, 1536), 150);
        assert_eq!(chapter_word_limit(4000, 1536), 576);
        assert_eq!(chapter_word_limit(1, 1536), 1);
    }

    #[test]
    fn summary_speaker_name_uses_persona_for_the_user_side() {
        use lettuce_conversations::MessageRole;
        // The user's own messages are labeled with the active persona so the
        // summary says "Mia and Jake" instead of "Mia and the user".
        assert_eq!(
            summary_speaker_name(MessageRole::User, Some("Jake"), "User", "Character"),
            "Jake"
        );
        // No persona falls back to the structural label.
        assert_eq!(
            summary_speaker_name(MessageRole::User, None, "User", "Character"),
            "User"
        );
        // The character side is never renamed by the persona.
        assert_eq!(
            summary_speaker_name(MessageRole::Assistant, Some("Jake"), "User", "Character"),
            "Character"
        );
    }

    #[test]
    fn summary_direct_content_prefixes_the_user_persona() {
        use lettuce_conversations::MessageRole;
        // Direct chats have no speaker label, so the persona name is prefixed
        // onto the user's own line; everything else passes through unchanged.
        assert_eq!(
            summary_direct_content(MessageRole::User, Some("Jake"), "Hello"),
            "Jake: Hello"
        );
        assert_eq!(
            summary_direct_content(MessageRole::User, None, "Hello"),
            "Hello"
        );
        assert_eq!(
            summary_direct_content(MessageRole::Assistant, Some("Jake"), "Hi"),
            "Hi"
        );
    }

    #[test]
    fn fallback_usage_preserves_known_details_without_inventing_missing_counts() {
        let usage = |cached, reasoning| {
            Some(lettuce_conversations::InferenceUsage {
                image_tokens: None,
                audio_tokens: None,
                total_tokens: None,
                provider_reported_cost: None,
                cache_write_tokens: None,
                web_search_requests: None,
                input_tokens: 10,
                output_tokens: 5,
                cached_input_tokens: cached,
                reasoning_tokens: reasoning,
            })
        };
        let combined = super::aggregate_usage(usage(Some(0), Some(2)), usage(Some(3), Some(4)))
            .expect("combined usage");
        assert_eq!(combined.input_tokens, 20);
        assert_eq!(combined.cached_input_tokens, Some(3));
        assert_eq!(combined.reasoning_tokens, Some(6));
        let partial = super::aggregate_usage(usage(None, Some(u64::MAX)), usage(Some(3), Some(1)))
            .expect("partial usage");
        assert_eq!(partial.cached_input_tokens, None);
        assert_eq!(partial.reasoning_tokens, None);
    }

    #[test]
    fn a_covered_automatic_window_never_replaces_the_newer_summary() {
        let window = lettuce_memory::DynamicMemorySummaryWindow {
            message_interval: 2,
            start: 2,
            end: 4,
        };
        let automatic = crate::CompanionMemoryWindowSelection::Automatic;
        let recent = crate::CompanionMemoryWindowSelection::Recent;
        assert!(!super::window_already_summarized(2, window, automatic));
        assert!(super::window_already_summarized(4, window, automatic));
        assert!(!super::window_already_summarized(4, window, recent));
        assert!(super::window_already_summarized(6, window, recent));
    }

    #[test]
    fn legacy_summary_validation_strips_fences_thinking_and_whitespace() {
        assert_eq!(
            validate_summary_text("```text\n<think>draft</think> Mira   chose tea.\n```")
                .expect("summary"),
            "Mira chose tea."
        );
        assert!(validate_summary_text("I'm sorry, I cannot summarize that.").is_err());
        assert!(validate_summary_text("write_summary({ summary: 'x' })").is_err());
        assert!(validate_summary_text("  ").is_err());
    }

    #[test]
    fn summary_response_accepts_legacy_tool_or_plain_text() {
        let summary = "Mira reached the harbor. ".repeat(500).trim_end().to_owned();
        assert!(summary.len() > 6_000);
        let outcome = |parts, tool_calls| InferenceOutcome {
            provider_response_id: None,
            candidates: vec![InferenceCandidate {
                ordinal: 0,
                parts,
                tool_calls,
                provider_replay: None,
                media: Vec::new(),
            }],
            usage: None,
            finish_reason: FinishReason::Stop,
            provider_finish_reason: None,
            provider_request_id: None,
            warning_codes: Vec::new(),
        };
        assert_eq!(
            summary_from_outcome(
                &outcome(
                    Vec::new(),
                    vec![ProposedToolCall {
                        provider_call_id: Some("summary".into()),
                        name: "write_summary".into(),
                        arguments: json!({"summary": &summary}),
                        raw_arguments: None,
                        provider_replay: None,
                    }],
                ),
                true,
            )
            .expect("tool summary"),
            summary
        );
        assert_eq!(
            summary_from_outcome(
                &outcome(
                    vec![MessagePart::Text {
                        text: summary.clone(),
                    }],
                    Vec::new(),
                ),
                false,
            )
            .expect("text summary"),
            summary
        );
        assert!(
            summary_from_outcome(
                &outcome(
                    vec![MessagePart::Text {
                        text: "write_summary({ summary: 'invalid' })".into(),
                    }],
                    Vec::new(),
                ),
                false,
            )
            .is_err()
        );
    }

    mod chapters_tests {
        use std::collections::VecDeque;
        use std::sync::{Arc, Mutex};

        use async_trait::async_trait;
        use lettuce_context::{PromptDocument, PromptRenderValues, PromptRepository};
        use lettuce_conversations::{
            ContextAttributions, ContextBudgetReport, ConversationOverviewReader,
            ConversationReader, FinishReason, InferenceCandidate, InferenceOutcome, InferencePort,
            InferenceRequest, PortError, ProposedToolCall, ProviderContextPart,
            ProviderNeutralContext,
        };
        use lettuce_jobs::handle::JobHandle;
        use lettuce_jobs::{JobKind, JobSpec, JobStore, JobSubject, OutcomeRef, ResourceClass, SubjectKind};
        use lettuce_memory::{
            DynamicMemoryAttemptStatus, DynamicMemoryRunRepository, DynamicMemorySourceMessage,
            DynamicMemoryStructuredFallbackFormat, DynamicMemorySummaryCommit,
            DynamicMemorySummaryWindow, MemoryOrigin, MemoryRepository, MemorySummary,
            MemorySummaryChange, MemorySummaryRepository, MemorySummaryUnitKind,
            MemorySummaryUnitRepository, MemorySummaryUnitScope, NewDynamicMemoryRunAttempt,
            NewMemorySummaryUnit,
        };
        use lettuce_types::{
            ConversationBranchId, ConversationId, DynamicMemoryAttemptId, DynamicMemoryRunId,
            MemorySpaceId, MemorySummaryUnitId, MessageId, TimestampMillis,
        };
        use serde_json::json;

        use super::unit;
        use crate::api::conversations::turns_tests::{open, replied_chat};
        use crate::api::tests::{Harness as ApiHarness, Reply, harness as api_harness};
        use crate::companion::companion_memory_summary::build_summary_request;
        use crate::{
            BuiltInPromptId, BuiltInPromptService, CompanionMemoryInferenceError,
            CompanionMemorySummaryCoordinator, CompanionMemoryWindowSelection,
            EmbeddingGenerationError, MemoryEmbeddingEngine, SummaryMode, format_message_timestamp,
        };

        struct WordCountEngine;

        impl MemoryEmbeddingEngine for WordCountEngine {
            fn source_revision(&self) -> &str {
                "word-count-test"
            }

            fn dimensions(&self) -> lettuce_embeddings::EmbeddingDimensions {
                lettuce_embeddings::EmbeddingDimensions::D64
            }

            fn count_tokens(&self, text: &str) -> Result<u32, EmbeddingGenerationError> {
                Ok(u32::try_from(text.split_whitespace().count()).unwrap_or(u32::MAX))
            }

            fn embed_memory(
                &self,
                _: &lettuce_embeddings::EmbeddingRequest,
                _: &lettuce_jobs::handle::CancellationToken,
            ) -> Result<lettuce_embeddings::EmbeddingVector, EmbeddingGenerationError> {
                panic!("summary tests do not embed")
            }
        }

        enum ScriptedReply {
            Summary(String),
            Cancelled,
        }

        struct ScriptedInference {
            replies: Mutex<VecDeque<ScriptedReply>>,
            requests: Mutex<Vec<InferenceRequest>>,
        }

        impl ScriptedInference {
            fn new(replies: Vec<ScriptedReply>) -> Self {
                Self {
                    replies: Mutex::new(replies.into()),
                    requests: Mutex::new(Vec::new()),
                }
            }

            fn requests(&self) -> Vec<InferenceRequest> {
                self.requests.lock().expect("requests").clone()
            }
        }

        #[async_trait]
        impl InferencePort for ScriptedInference {
            async fn run(
                &self,
                request: InferenceRequest,
            ) -> Result<InferenceOutcome, PortError> {
                self.requests.lock().expect("requests").push(request);
                let reply = self
                    .replies
                    .lock()
                    .expect("replies")
                    .pop_front()
                    .expect("scripted reply");
                match reply {
                    ScriptedReply::Cancelled => Err(PortError::Cancelled),
                    ScriptedReply::Summary(text) => Ok(InferenceOutcome {
                        provider_response_id: None,
                        candidates: vec![InferenceCandidate {
                            ordinal: 0,
                            parts: Vec::new(),
                            tool_calls: vec![ProposedToolCall {
                                provider_call_id: Some("call-1".into()),
                                name: "write_summary".into(),
                                arguments: json!({ "summary": text }),
                                raw_arguments: None,
                                provider_replay: None,
                            }],
                            provider_replay: None,
                            media: Vec::new(),
                        }],
                        usage: None,
                        finish_reason: FinishReason::Stop,
                        provider_finish_reason: None,
                        provider_request_id: Some("req-1".into()),
                        warning_codes: Vec::new(),
                    }),
                }
            }
        }

        struct Harness {
            api: ApiHarness,
            chat: String,
            conversation_id: ConversationId,
            branch_id: ConversationBranchId,
            space_id: MemorySpaceId,
            summary_prompt: PromptDocument,
            sources: Vec<DynamicMemorySourceMessage>,
            time_awareness_enabled: bool,
            next_time: i64,
        }

        impl Harness {
            async fn new(tag: &str) -> Self {
                let api = api_harness(Reply::Text("assistant reply"));
                let (chat, _) = replied_chat(&api, tag).await;
                let database = api.context.backend().database();
                let ids = BuiltInPromptService::new(database)
                    .expect("prompt service")
                    .bootstrap(api.context.now())
                    .expect("bootstrap");
                let summary_prompt =
                    PromptRepository::get(database, ids.get(BuiltInPromptId::DynamicSummary))
                        .expect("prompt lookup")
                        .expect("summary prompt");
                let conversation_id: ConversationId = chat.parse().expect("conversation id");
                let branch_id = ConversationReader::get(database, conversation_id)
                    .expect("conversation")
                    .conversation
                    .active_branch_id;
                let space_id = MemoryRepository::get_for_branch(database, conversation_id, branch_id)
                    .expect("memory")
                    .expect("space")
                    .id;
                let view = open(&api, &chat).await;
                let mut sources = Vec::new();
                for message in view.messages.items.iter().rev() {
                    let item = ConversationOverviewReader::timeline_anchor(
                        database,
                        conversation_id,
                        branch_id,
                        message.id.parse().expect("message id"),
                    )
                    .expect("anchor")
                    .item;
                    sources.push(DynamicMemorySourceMessage {
                        message_id: item.message.id,
                        role: item.message.role,
                        render_source: item.message.active_render_source,
                        effective_time: item.message.effective_time,
                    });
                }
                Self {
                    api,
                    chat,
                    conversation_id,
                    branch_id,
                    space_id,
                    summary_prompt,
                    sources,
                    time_awareness_enabled: false,
                    next_time: 10_000,
                }
            }

            fn database(&self) -> &lettuce_database::Database {
                self.api.context.backend().database()
            }

            fn scope(&self) -> MemorySummaryUnitScope {
                MemorySummaryUnitScope {
                    space_id: self.space_id,
                    conversation_id: self.conversation_id,
                    branch_id: self.branch_id,
                }
            }

            fn tick(&mut self) -> TimestampMillis {
                self.next_time += 100;
                TimestampMillis::new(self.next_time)
            }

            fn open_run(
                &mut self,
                start: u64,
                end: u64,
            ) -> (DynamicMemoryRunId, DynamicMemoryAttemptId, JobHandle) {
                let at = self.tick();
                let database = self.database();
                let job = JobStore::create_or_get(
                    database,
                    JobSpec::new(
                        JobKind::MemoryExtraction,
                        JobSubject::new(
                            SubjectKind::Conversation,
                            format!("{}-{}", self.chat, at.get()),
                        )
                        .expect("subject"),
                        OutcomeRef::Conversation(self.conversation_id),
                    )
                    .with_resources(vec![ResourceClass::Cpu]),
                )
                .expect("job")
                .job;
                let handle = JobHandle::new(job.id);
                let memory = MemoryRepository::get(database, self.space_id)
                    .expect("memory")
                    .expect("space");
                let run_id = DynamicMemoryRunId::new();
                let attempt_id = DynamicMemoryAttemptId::new();
                let admitted = database
                    .admit_dynamic_memory_run_attempt(NewDynamicMemoryRunAttempt {
                        run_id,
                        attempt_id,
                        conversation_id: self.conversation_id,
                        branch_id: self.branch_id,
                        space_id: self.space_id,
                        cycle_start_change: None,
                        starting_memory: memory,
                        source_messages: self.sources.clone(),
                        profile: crate::companion::companion_memory_run::tests::profile(),
                        time_awareness_enabled: self.time_awareness_enabled,
                        supersession_enabled: false,
                        group_companion_members: Vec::new(),
                        structured_fallback_format: DynamicMemoryStructuredFallbackFormat::Xml,
                        summary_window: DynamicMemorySummaryWindow {
                            message_interval: 2,
                            start,
                            end,
                        },
                        tool_request: lettuce_memory::dynamic_memory_tool_request_for_run(
                            lettuce_memory::DynamicMemoryToolOptions {
                                group: false,
                                supersession_enabled: false,
                                require_source_message_id: self.time_awareness_enabled,
                                group_companion: None,
                            },
                            &|key| key.to_owned(),
                        ),
                        job_id: job.id,
                        job_attempt: None,
                        now: at,
                    })
                    .expect("admit run");
                database
                    .transition_dynamic_memory_attempt(
                        attempt_id,
                        admitted.attempt.revision,
                        DynamicMemoryAttemptStatus::Processing,
                        None,
                        at,
                    )
                    .expect("transition processing");
                (run_id, attempt_id, handle)
            }

            fn publish_checkpoint(&mut self, summary: MemorySummary) {
                let database = self.database();
                let memory = MemoryRepository::get(database, self.space_id)
                    .expect("memory")
                    .expect("space");
                MemorySummaryRepository::compare_and_apply_summary(
                    database,
                    MemorySummaryChange {
                        expected_revision: memory.revision,
                        summary,
                    },
                )
                .expect("publish checkpoint summary");
            }

            fn publish_summary(&mut self, origin: MemoryOrigin, text: &str) {
                let at = self.tick();
                let database = self.database();
                let memory = MemoryRepository::get(database, self.space_id)
                    .expect("memory")
                    .expect("space");
                let source_ids: Vec<MessageId> = self
                    .sources
                    .iter()
                    .map(|source| source.message_id)
                    .collect();
                let (window_start, window_end) = if origin == MemoryOrigin::User {
                    (0, 0)
                } else {
                    (0, u64::try_from(source_ids.len()).expect("len"))
                };
                MemorySummaryRepository::compare_and_apply_summary(
                    database,
                    MemorySummaryChange {
                        expected_revision: memory.revision,
                        summary: MemorySummary {
                            origin,
                            space_id: self.space_id,
                            branch_id: self.branch_id,
                            text: text.to_owned(),
                            token_count: Some(WordCountEngine.count_tokens(text).expect("count")),
                            window_start,
                            window_end,
                            source_message_ids: if origin == MemoryOrigin::User {
                                Vec::new()
                            } else {
                                source_ids
                            },
                            updated_at: at,
                        },
                    },
                )
                .expect("publish summary");
            }
        }

        fn request_texts(request: &InferenceRequest) -> Vec<String> {
            request
                .context
                .messages
                .iter()
                .map(|message| {
                    message
                        .parts
                        .iter()
                        .filter_map(|part| match part {
                            ProviderContextPart::Text { text } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<String>()
                })
                .collect()
        }

        #[tokio::test]
        async fn cumulative_mode_builds_same_request_as_before() {
            let mut harness = Harness::new("cumulative").await;
            let count = u64::try_from(harness.sources.len()).expect("len");
            let (run_id, attempt_id, handle) = harness.open_run(0, count);
            let now = harness.tick();
            let database = harness.database();

            let run = database.load_dynamic_memory_run(run_id).expect("run");
            let attempt = database
                .load_dynamic_memory_attempt(attempt_id)
                .expect("attempt");
            let sources = crate::materialize_sources(database, &run).expect("sources");
            let runtime_text = crate::generation::runtime_text::RuntimeText::load(
                database,
                BuiltInPromptId::MemoryRuntime,
            )
            .expect("runtime text");
            let aggregate = ConversationReader::get(database, harness.conversation_id)
                .expect("conversation");
            let persona_name =
                crate::companion::companion_memory_inference::transcript_persona_name(
                    &aggregate.conversation,
                );
            let expected = build_summary_request(
                &run,
                &attempt,
                &harness.summary_prompt,
                &runtime_text,
                None,
                &sources,
                persona_name.as_deref(),
                aggregate.conversation.kind.is_group(),
                aggregate.conversation.participants.len(),
                &handle,
                None,
                now,
            )
            .expect("expected request");

            let inference = Arc::new(ScriptedInference::new(vec![ScriptedReply::Summary(
                "They greeted each other.".into(),
            )]));
            CompanionMemorySummaryCoordinator::new(
                &WordCountEngine,
                database,
                database,
                inference.as_ref(),
            )
            .run(
                run_id,
                attempt_id,
                CompanionMemoryWindowSelection::Automatic,
                &harness.summary_prompt,
                SummaryMode::Cumulative,
                &handle,
                None,
                now,
            )
            .await
            .expect("cumulative run");

            let requests = inference.requests();
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0], expected);
        }

        #[tokio::test]
        async fn chapters_adopts_legacy_published_summary_as_seed() {
            let mut harness = Harness::new("legacy-seed").await;
            let count = u64::try_from(harness.sources.len()).expect("len");
            harness.publish_summary(MemoryOrigin::Model, "Met at the dock at dawn.");

            let (run_id, attempt_id, handle) = harness.open_run(count, count + count);
            let now = harness.tick();
            let inference = Arc::new(ScriptedInference::new(vec![ScriptedReply::Summary(
                "They decided to buy coffee.".into(),
            )]));

            let result = CompanionMemorySummaryCoordinator::new(
                &WordCountEngine,
                harness.database(),
                harness.database(),
                inference.as_ref(),
            )
            .run(
                run_id,
                attempt_id,
                CompanionMemoryWindowSelection::Automatic,
                &harness.summary_prompt,
                SummaryMode::Chapters {
                    token_budget: 100,
                    chapter_size: 2,
                },
                &handle,
                None,
                now,
            )
            .await
            .expect("chapter run");

            assert_eq!(
                result.checkpoint.summary.text,
                "Met at the dock at dawn.\n\nThey decided to buy coffee."
            );
            let active = harness
                .database()
                .active_summary_units(harness.scope(), Some(run_id))
                .expect("active");
            assert_eq!(active.len(), 2);
            assert_eq!(active[0].kind, MemorySummaryUnitKind::Seed);
            assert_eq!(active[1].kind, MemorySummaryUnitKind::Cycle);

            let requests = inference.requests();
            let cycle = &requests[0].context.messages;
            let texts = request_texts(&requests[0]);
            assert_eq!(cycle.len(), harness.sources.len() + 4);
            assert!(texts[1].contains("Met at the dock at dawn."));
            for (message, source) in cycle[3..cycle.len() - 1].iter().zip(&harness.sources) {
                assert_eq!(message.role, source.role);
            }
            assert!(texts.iter().all(|text| !text.contains("[message:")));
        }

        #[tokio::test]
        async fn chapters_seed_does_not_count_against_the_budget() {
            let mut harness = Harness::new("seed-budget").await;
            let count = u64::try_from(harness.sources.len()).expect("len");
            harness.publish_summary(
                MemoryOrigin::Model,
                "A long legacy summary that alone exceeds the whole budget.",
            );
            let mode = SummaryMode::Chapters {
                token_budget: 8,
                chapter_size: 2,
            };
            let inference = Arc::new(ScriptedInference::new(vec![
                ScriptedReply::Summary("Cycle one words.".into()),
                ScriptedReply::Summary("Cycle two words.".into()),
            ]));

            let (run1, att1, h1) = harness.open_run(count, count * 2);
            let now1 = harness.tick();
            let res1 = CompanionMemorySummaryCoordinator::new(
                &WordCountEngine,
                harness.database(),
                harness.database(),
                inference.as_ref(),
            )
            .run(
                run1,
                att1,
                CompanionMemoryWindowSelection::Automatic,
                &harness.summary_prompt,
                mode,
                &h1,
                None,
                now1,
            )
            .await
            .expect("run 1");
            harness.publish_checkpoint(res1.checkpoint.summary);

            let (run2, att2, h2) = harness.open_run(count * 2, count * 3);
            let now2 = harness.tick();
            let res = CompanionMemorySummaryCoordinator::new(
                &WordCountEngine,
                harness.database(),
                harness.database(),
                inference.as_ref(),
            )
            .run(
                run2,
                att2,
                CompanionMemoryWindowSelection::Automatic,
                &harness.summary_prompt,
                mode,
                &h2,
                None,
                now2,
            )
            .await
            .expect("run 2");

            assert_eq!(inference.requests().len(), 2);
            assert_eq!(
                res.checkpoint.summary.text,
                "A long legacy summary that alone exceeds the whole budget.\n\nCycle one words.\n\nCycle two words."
            );
        }

        #[tokio::test]
        async fn chapters_under_budget_makes_no_chapter_request() {
            let mut harness = Harness::new("under-budget").await;
            let count = u64::try_from(harness.sources.len()).expect("len");
            let (run1, att1, handle1) = harness.open_run(0, count);
            let inference = Arc::new(ScriptedInference::new(vec![
                ScriptedReply::Summary("First cycle summary.".into()),
                ScriptedReply::Summary("Second cycle summary.".into()),
            ]));
            let now1 = harness.tick();
            let res1 = CompanionMemorySummaryCoordinator::new(
                &WordCountEngine,
                harness.database(),
                harness.database(),
                inference.as_ref(),
            )
            .run(
                run1,
                att1,
                CompanionMemoryWindowSelection::Automatic,
                &harness.summary_prompt,
                SummaryMode::Chapters {
                    token_budget: 50,
                    chapter_size: 2,
                },
                &handle1,
                None,
                now1,
            )
            .await
            .expect("run 1");
            harness.publish_checkpoint(res1.checkpoint.summary);

            let (run2, att2, handle2) = harness.open_run(count, count * 2);
            let now2 = harness.tick();
            let result = CompanionMemorySummaryCoordinator::new(
                &WordCountEngine,
                harness.database(),
                harness.database(),
                inference.as_ref(),
            )
            .run(
                run2,
                att2,
                CompanionMemoryWindowSelection::Automatic,
                &harness.summary_prompt,
                SummaryMode::Chapters {
                    token_budget: 50,
                    chapter_size: 2,
                },
                &handle2,
                None,
                now2,
            )
            .await
            .expect("run 2");

            assert_eq!(inference.requests().len(), 2);
            assert_eq!(
                result.checkpoint.summary.text,
                "First cycle summary.\n\nSecond cycle summary."
            );
        }

        #[tokio::test]
        async fn chapters_over_budget_folds_oldest_cycles_into_level_1_chapter() {
            let mut harness = Harness::new("over-budget").await;
            let count = u64::try_from(harness.sources.len()).expect("len");
            let mode = SummaryMode::Chapters {
                token_budget: 10,
                chapter_size: 2,
            };
            let inference = Arc::new(ScriptedInference::new(vec![
                ScriptedReply::Summary("One two three four.".into()),
                ScriptedReply::Summary("Five six seven eight.".into()),
                ScriptedReply::Summary("Nine ten eleven twelve.".into()),
                ScriptedReply::Summary("Combined chapter one.".into()),
            ]));

            for step in 0..3 {
                let start = count * step;
                let end = start + count;
                let (run_id, attempt_id, handle) = harness.open_run(start, end);
                let now = harness.tick();
                let res = CompanionMemorySummaryCoordinator::new(
                    &WordCountEngine,
                    harness.database(),
                    harness.database(),
                    inference.as_ref(),
                )
                .run(
                    run_id,
                    attempt_id,
                    CompanionMemoryWindowSelection::Automatic,
                    &harness.summary_prompt,
                    mode,
                    &handle,
                    None,
                    now,
                )
                .await
                .expect("run");

                if step == 2 {
                    assert_eq!(
                        res.checkpoint.summary.text,
                        "Combined chapter one.\n\nNine ten eleven twelve."
                    );
                    let active = harness
                        .database()
                        .active_summary_units(harness.scope(), Some(run_id))
                        .expect("active");
                    assert_eq!(active.len(), 2);
                    assert_eq!(active[0].kind, MemorySummaryUnitKind::Chapter);
                    assert_eq!(active[0].level, 1);
                    assert_eq!((active[0].window_start, active[0].window_end), (0, count * 2));
                    assert_eq!(active[1].kind, MemorySummaryUnitKind::Cycle);
                    assert_eq!(
                        (active[1].window_start, active[1].window_end),
                        (count * 2, count * 3)
                    );
                } else {
                    harness.publish_checkpoint(res.checkpoint.summary);
                }
            }
            assert_eq!(inference.requests().len(), 4);
        }

        #[tokio::test]
        async fn chapters_rejects_non_shorter_chapter_and_publishes_over_budget() {
            let mut harness = Harness::new("reject-longer").await;
            let count = u64::try_from(harness.sources.len()).expect("len");
            let mode = SummaryMode::Chapters {
                token_budget: 5,
                chapter_size: 2,
            };
            let inference = Arc::new(ScriptedInference::new(vec![
                ScriptedReply::Summary("One two three.".into()),
                ScriptedReply::Summary("Four five six.".into()),
                ScriptedReply::Summary(
                    "This chapter output is way longer than six words total.".into(),
                ),
            ]));

            let (run1, att1, h1) = harness.open_run(0, count);
            let now1 = harness.tick();
            let res1 = CompanionMemorySummaryCoordinator::new(
                &WordCountEngine,
                harness.database(),
                harness.database(),
                inference.as_ref(),
            )
            .run(
                run1,
                att1,
                CompanionMemoryWindowSelection::Automatic,
                &harness.summary_prompt,
                mode,
                &h1,
                None,
                now1,
            )
            .await
            .expect("run 1");
            harness.publish_checkpoint(res1.checkpoint.summary);

            let (run2, att2, h2) = harness.open_run(count, count * 2);
            let now2 = harness.tick();
            let res = CompanionMemorySummaryCoordinator::new(
                &WordCountEngine,
                harness.database(),
                harness.database(),
                inference.as_ref(),
            )
            .run(
                run2,
                att2,
                CompanionMemoryWindowSelection::Automatic,
                &harness.summary_prompt,
                mode,
                &h2,
                None,
                now2,
            )
            .await
            .expect("run 2 publishes over budget");

            assert_eq!(
                res.checkpoint.summary.text,
                "One two three.\n\nFour five six."
            );
        }

        #[tokio::test]
        async fn chapters_retried_attempt_reuses_stored_cycle_unit() {
            let mut harness = Harness::new("retry-reuse").await;
            let count = u64::try_from(harness.sources.len()).expect("len");
            let mode = SummaryMode::Chapters {
                token_budget: 5,
                chapter_size: 2,
            };
            let inference = Arc::new(ScriptedInference::new(vec![
                ScriptedReply::Summary("One two three.".into()),
                ScriptedReply::Summary("Four five six.".into()),
                ScriptedReply::Cancelled,
                ScriptedReply::Summary("Short chapter.".into()),
            ]));

            let (run1, att1, h1) = harness.open_run(0, count);
            let now1 = harness.tick();
            let res1 = CompanionMemorySummaryCoordinator::new(
                &WordCountEngine,
                harness.database(),
                harness.database(),
                inference.as_ref(),
            )
            .run(
                run1,
                att1,
                CompanionMemoryWindowSelection::Automatic,
                &harness.summary_prompt,
                mode,
                &h1,
                None,
                now1,
            )
            .await
            .expect("run 1");
            harness.publish_checkpoint(res1.checkpoint.summary);

            let (run2, att2, h2) = harness.open_run(count, count * 2);
            let now2 = harness.tick();
            let err = CompanionMemorySummaryCoordinator::new(
                &WordCountEngine,
                harness.database(),
                harness.database(),
                inference.as_ref(),
            )
            .run(
                run2,
                att2,
                CompanionMemoryWindowSelection::Automatic,
                &harness.summary_prompt,
                mode,
                &h2,
                None,
                now2,
            )
            .await
            .expect_err("cancelled during chapter");
            assert!(matches!(err, CompanionMemoryInferenceError::Cancelled));
            assert_eq!(inference.requests().len(), 3);

            let now3 = harness.tick();
            let res = CompanionMemorySummaryCoordinator::new(
                &WordCountEngine,
                harness.database(),
                harness.database(),
                inference.as_ref(),
            )
            .run(
                run2,
                att2,
                CompanionMemoryWindowSelection::Automatic,
                &harness.summary_prompt,
                mode,
                &h2,
                None,
                now3,
            )
            .await
            .expect("retry succeeds");

            assert_eq!(inference.requests().len(), 4);
            assert_eq!(res.checkpoint.summary.text, "Short chapter.");
        }

        #[tokio::test]
        async fn chapters_user_edit_of_one_part_replaces_only_that_part() {
            let mut harness = Harness::new("part-edit").await;
            let count = u64::try_from(harness.sources.len()).expect("len");
            let mode = SummaryMode::Chapters {
                token_budget: 50,
                chapter_size: 2,
            };
            let inference = Arc::new(ScriptedInference::new(vec![
                ScriptedReply::Summary("First cycle summary.".into()),
                ScriptedReply::Summary("Second cycle summary.".into()),
                ScriptedReply::Summary("Third cycle summary.".into()),
            ]));
            for (index, (start, end)) in [(0, count), (count, count * 2)].into_iter().enumerate() {
                let (run, attempt, handle) = harness.open_run(start, end);
                let now = harness.tick();
                let result = CompanionMemorySummaryCoordinator::new(
                    &WordCountEngine,
                    harness.database(),
                    harness.database(),
                    inference.as_ref(),
                )
                .run(
                    run,
                    attempt,
                    CompanionMemoryWindowSelection::Automatic,
                    &harness.summary_prompt,
                    mode,
                    &handle,
                    None,
                    now,
                )
                .await
                .unwrap_or_else(|error| panic!("run {index}: {error:?}"));
                harness.publish_checkpoint(result.checkpoint.summary);
            }
            harness.publish_summary(
                MemoryOrigin::User,
                "First cycle summary, corrected.\n\nSecond cycle summary.",
            );

            let (run3, att3, handle3) = harness.open_run(count * 2, count * 3);
            let now3 = harness.tick();
            let result = CompanionMemorySummaryCoordinator::new(
                &WordCountEngine,
                harness.database(),
                harness.database(),
                inference.as_ref(),
            )
            .run(
                run3,
                att3,
                CompanionMemoryWindowSelection::Automatic,
                &harness.summary_prompt,
                mode,
                &handle3,
                None,
                now3,
            )
            .await
            .expect("run 3");

            assert_eq!(
                result.checkpoint.summary.text,
                "First cycle summary, corrected.\n\nSecond cycle summary.\n\nThird cycle summary."
            );
            let active = harness
                .database()
                .active_summary_units(harness.scope(), Some(run3))
                .expect("active");
            let shape: Vec<_> = active
                .iter()
                .map(|unit| (unit.kind, unit.origin, unit.window_start, unit.window_end))
                .collect();
            assert_eq!(
                shape,
                vec![
                    (MemorySummaryUnitKind::Cycle, MemoryOrigin::User, 0, count),
                    (
                        MemorySummaryUnitKind::Cycle,
                        MemoryOrigin::Model,
                        count,
                        count * 2
                    ),
                    (
                        MemorySummaryUnitKind::Cycle,
                        MemoryOrigin::Model,
                        count * 2,
                        count * 3
                    ),
                ]
            );
        }

        #[tokio::test]
        async fn chapters_user_edited_summary_is_adopted_as_user_seed_and_never_compacted() {
            let mut harness = Harness::new("user-seed").await;
            let count = u64::try_from(harness.sources.len()).expect("len");
            harness.publish_summary(MemoryOrigin::User, "User authored foundation text.");
            let mode = SummaryMode::Chapters {
                token_budget: 5,
                chapter_size: 2,
            };
            let inference = Arc::new(ScriptedInference::new(vec![
                ScriptedReply::Summary("Cycle one words.".into()),
                ScriptedReply::Summary("Cycle two words.".into()),
                ScriptedReply::Summary("Compact chapter.".into()),
            ]));

            let (run1, att1, h1) = harness.open_run(0, count);
            let now1 = harness.tick();
            let res1 = CompanionMemorySummaryCoordinator::new(
                &WordCountEngine,
                harness.database(),
                harness.database(),
                inference.as_ref(),
            )
            .run(
                run1,
                att1,
                CompanionMemoryWindowSelection::Automatic,
                &harness.summary_prompt,
                mode,
                &h1,
                None,
                now1,
            )
            .await
            .expect("run 1");
            harness.publish_checkpoint(res1.checkpoint.summary);

            let (run2, att2, h2) = harness.open_run(count, count * 2);
            let now2 = harness.tick();
            let res = CompanionMemorySummaryCoordinator::new(
                &WordCountEngine,
                harness.database(),
                harness.database(),
                inference.as_ref(),
            )
            .run(
                run2,
                att2,
                CompanionMemoryWindowSelection::Automatic,
                &harness.summary_prompt,
                mode,
                &h2,
                None,
                now2,
            )
            .await
            .expect("run 2");

            assert_eq!(
                res.checkpoint.summary.text,
                "User authored foundation text.\n\nCompact chapter."
            );
            let active = harness
                .database()
                .active_summary_units(harness.scope(), Some(run2))
                .expect("active");
            assert_eq!(active[0].kind, MemorySummaryUnitKind::Seed);
            assert_eq!(active[0].origin, MemoryOrigin::User);
            assert_eq!(active[1].kind, MemorySummaryUnitKind::Chapter);

            let requests = inference.requests();
            assert_eq!(requests.len(), 3);
            let chapter_msgs = request_texts(&requests[2]);
            assert!(chapter_msgs[1].contains("User authored foundation text."));
            assert!(!chapter_msgs[2].contains("User authored foundation text."));
            assert!(chapter_msgs[2].contains("[1] Cycle one words."));
            assert!(chapter_msgs[2].contains("[2] Cycle two words."));
            assert!(chapter_msgs[3].contains("at most 1 words"));
        }

        #[tokio::test]
        async fn chapters_cancellation_during_chapter_request_returns_cancelled() {
            let mut harness = Harness::new("chapter-cancel").await;
            let count = u64::try_from(harness.sources.len()).expect("len");
            let mode = SummaryMode::Chapters {
                token_budget: 4,
                chapter_size: 2,
            };
            let inference = Arc::new(ScriptedInference::new(vec![
                ScriptedReply::Summary("Alpha beta gamma.".into()),
                ScriptedReply::Summary("Delta epsilon zeta.".into()),
                ScriptedReply::Cancelled,
            ]));

            let (run1, att1, h1) = harness.open_run(0, count);
            let now1 = harness.tick();
            let res1 = CompanionMemorySummaryCoordinator::new(
                &WordCountEngine,
                harness.database(),
                harness.database(),
                inference.as_ref(),
            )
            .run(
                run1,
                att1,
                CompanionMemoryWindowSelection::Automatic,
                &harness.summary_prompt,
                mode,
                &h1,
                None,
                now1,
            )
            .await
            .expect("run 1");
            harness.publish_checkpoint(res1.checkpoint.summary);

            let (run2, att2, h2) = harness.open_run(count, count * 2);
            let now2 = harness.tick();
            let err = CompanionMemorySummaryCoordinator::new(
                &WordCountEngine,
                harness.database(),
                harness.database(),
                inference.as_ref(),
            )
            .run(
                run2,
                att2,
                CompanionMemoryWindowSelection::Automatic,
                &harness.summary_prompt,
                mode,
                &h2,
                None,
                now2,
            )
            .await
            .expect_err("must cancel");
            assert!(matches!(err, CompanionMemoryInferenceError::Cancelled));
        }

        #[tokio::test]
        async fn chapters_cycle_unit_stores_source_message_time_range() {
            let mut harness = Harness::new("cycle-range").await;
            harness.time_awareness_enabled = false;
            for (index, source) in harness.sources.iter_mut().enumerate() {
                source.effective_time = TimestampMillis::new(
                    1_700_000_000_000 + i64::try_from(index).expect("index") * 60_000,
                );
            }
            let expected_start = harness
                .sources
                .iter()
                .map(|source| source.effective_time)
                .min();
            let expected_end = harness
                .sources
                .iter()
                .map(|source| source.effective_time)
                .max();
            let count = u64::try_from(harness.sources.len()).expect("len");
            let (run_id, attempt_id, handle) = harness.open_run(0, count);
            let now = harness.tick();
            let inference = Arc::new(ScriptedInference::new(vec![ScriptedReply::Summary(
                "Cycle with range.".into(),
            )]));

            CompanionMemorySummaryCoordinator::new(
                &WordCountEngine,
                harness.database(),
                harness.database(),
                inference.as_ref(),
            )
            .run(
                run_id,
                attempt_id,
                CompanionMemoryWindowSelection::Automatic,
                &harness.summary_prompt,
                SummaryMode::Chapters {
                    token_budget: 100,
                    chapter_size: 2,
                },
                &handle,
                None,
                now,
            )
            .await
            .expect("cycle run");

            let active = harness
                .database()
                .active_summary_units(harness.scope(), Some(run_id))
                .expect("active");
            assert_eq!(active.len(), 1);
            assert_eq!(active[0].kind, MemorySummaryUnitKind::Cycle);
            assert_eq!(active[0].time_start, expected_start);
            assert_eq!(active[0].time_end, expected_end);
        }

        #[tokio::test]
        async fn chapters_chapter_unit_spans_its_inputs_time_range() {
            let mut harness = Harness::new("chapter-range").await;
            let count = u64::try_from(harness.sources.len()).expect("len");
            let mode = SummaryMode::Chapters {
                token_budget: 5,
                chapter_size: 2,
            };
            let inference = Arc::new(ScriptedInference::new(vec![
                ScriptedReply::Summary("Cycle one words here.".into()),
                ScriptedReply::Summary("Cycle two words here.".into()),
                ScriptedReply::Summary("Compact chapter.".into()),
            ]));

            let t1_start = TimestampMillis::new(1_700_000_000_000);
            let t1_end = TimestampMillis::new(1_700_000_300_000);
            for (index, source) in harness.sources.iter_mut().enumerate() {
                source.effective_time = if index == 0 { t1_start } else { t1_end };
            }
            let (run1, att1, h1) = harness.open_run(0, count);
            let now1 = harness.tick();
            let res1 = CompanionMemorySummaryCoordinator::new(
                &WordCountEngine,
                harness.database(),
                harness.database(),
                inference.as_ref(),
            )
            .run(
                run1,
                att1,
                CompanionMemoryWindowSelection::Automatic,
                &harness.summary_prompt,
                mode,
                &h1,
                None,
                now1,
            )
            .await
            .expect("run 1");
            harness.publish_checkpoint(res1.checkpoint.summary);

            let t2_start = TimestampMillis::new(1_700_010_000_000);
            let t2_end = TimestampMillis::new(1_700_010_600_000);
            for (index, source) in harness.sources.iter_mut().enumerate() {
                source.effective_time = if index == 0 { t2_start } else { t2_end };
            }
            let (run2, att2, h2) = harness.open_run(count, count * 2);
            let now2 = harness.tick();
            CompanionMemorySummaryCoordinator::new(
                &WordCountEngine,
                harness.database(),
                harness.database(),
                inference.as_ref(),
            )
            .run(
                run2,
                att2,
                CompanionMemoryWindowSelection::Automatic,
                &harness.summary_prompt,
                mode,
                &h2,
                None,
                now2,
            )
            .await
            .expect("run 2");

            let active = harness
                .database()
                .active_summary_units(harness.scope(), Some(run2))
                .expect("active");
            assert_eq!(active.len(), 1);
            assert_eq!(active[0].kind, MemorySummaryUnitKind::Chapter);
            assert_eq!(active[0].time_start, Some(t1_start));
            assert_eq!(active[0].time_end, Some(t2_end));
        }

        #[tokio::test]
        async fn chapters_chapter_range_is_none_when_any_input_has_none() {
            let mut harness = Harness::new("chapter-mixed-range").await;
            let count = u64::try_from(harness.sources.len()).expect("len");
            let mode = SummaryMode::Chapters {
                token_budget: 5,
                chapter_size: 2,
            };
            let inference = Arc::new(ScriptedInference::new(vec![
                ScriptedReply::Summary("Cycle two words here.".into()),
                ScriptedReply::Summary("Compact chapter.".into()),
            ]));

            let (run1, att1, _h1) = harness.open_run(0, count);
            let now1 = harness.tick();
            harness
                .database()
                .insert_summary_unit(
                    NewMemorySummaryUnit {
                        id: MemorySummaryUnitId::new(),
                        scope: harness.scope(),
                        created_by_run_id: run1,
                        kind: MemorySummaryUnitKind::Cycle,
                        level: 0,
                        origin: MemoryOrigin::Model,
                        window_start: 0,
                        window_end: count,
                        time_start: None,
                        time_end: None,
                        text: "Legacy cycle words here.".into(),
                        token_count: 4,
                        created_at: now1,
                    },
                    &[],
                )
                .expect("legacy cycle");
            let memory = MemoryRepository::get(harness.database(), harness.space_id)
                .expect("memory")
                .expect("space");
            let checkpoint = harness
                .database()
                .commit_dynamic_memory_summary(
                    DynamicMemorySummaryCommit {
                        run_id: run1,
                        attempt_id: att1,
                        expected_memory_revision: memory.revision,
                        text: "Legacy cycle words here.".into(),
                        token_count: 4,
                        request_context: ProviderNeutralContext {
                            messages: Vec::new(),
                            budget: ContextBudgetReport {
                                selected_messages: 0,
                                omitted_messages: 0,
                                input_bytes: 0,
                                estimated_input_tokens: 1,
                                truncated: false,
                            },
                            attributions: ContextAttributions::default(),
                        },
                        usage: None,
                        provider_request_id: None,
                    },
                    now1,
                )
                .expect("commit legacy checkpoint");
            harness.publish_checkpoint(checkpoint.summary);

            let (run2, att2, h2) = harness.open_run(count, count * 2);
            let now2 = harness.tick();
            CompanionMemorySummaryCoordinator::new(
                &WordCountEngine,
                harness.database(),
                harness.database(),
                inference.as_ref(),
            )
            .run(
                run2,
                att2,
                CompanionMemoryWindowSelection::Automatic,
                &harness.summary_prompt,
                mode,
                &h2,
                None,
                now2,
            )
            .await
            .expect("run 2");

            let active = harness
                .database()
                .active_summary_units(harness.scope(), Some(run2))
                .expect("active");
            assert_eq!(active.len(), 1);
            assert_eq!(active[0].kind, MemorySummaryUnitKind::Chapter);
            assert_eq!(active[0].time_start, None);
            assert_eq!(active[0].time_end, None);
        }

        #[tokio::test]
        async fn chapters_time_aware_chapter_request_shows_entry_ranges() {
            let mut harness = Harness::new("chapter-time-aware").await;
            harness.time_awareness_enabled = true;
            let count = u64::try_from(harness.sources.len()).expect("len");
            let (run_id, attempt_id, handle) = harness.open_run(0, count);
            let now = TimestampMillis::new(1_700_020_000_000);
            let database = harness.database();
            let run = database.load_dynamic_memory_run(run_id).expect("run");
            let attempt = database
                .load_dynamic_memory_attempt(attempt_id)
                .expect("attempt");
            let runtime_text = crate::generation::runtime_text::RuntimeText::load(
                database,
                BuiltInPromptId::MemoryRuntime,
            )
            .expect("runtime text");

            let t_a = TimestampMillis::new(1_700_000_000_000);
            let t_b = TimestampMillis::new(1_700_000_600_000);
            let t_same_a = TimestampMillis::new(1_700_010_000_000);
            let t_same_b = TimestampMillis::new(1_700_010_005_000);

            let mut u1 = unit(MemorySummaryUnitKind::Cycle, 0, (0, 6));
            u1.text = "First stretch.".into();
            u1.time_start = Some(t_a);
            u1.time_end = Some(t_b);

            let mut u2 = unit(MemorySummaryUnitKind::Cycle, 0, (6, 12));
            u2.text = "Same minute stretch.".into();
            u2.time_start = Some(t_same_a);
            u2.time_end = Some(t_same_b);

            let mut u3 = unit(MemorySummaryUnitKind::Cycle, 0, (12, 18));
            u3.text = "Legacy stretch.".into();
            u3.time_start = None;
            u3.time_end = None;

            let request = super::super::build_chapter_request(
                &run,
                &attempt,
                &runtime_text,
                None,
                &[&u1, &u2, &u3],
                1,
                0,
                18,
                120,
                false,
                &handle,
                None,
                now,
            )
            .expect("chapter request");

            let texts = request_texts(&request);
            let expected_system = {
                let mut system = runtime_text
                    .render_with("summary_chapter_system", [])
                    .expect("system");
                let mut time_values = PromptRenderValues::default();
                super::super::insert_time_values(&mut time_values, now);
                let time_rules = runtime_text
                    .render_with(
                        "summary_chapter_time_rules",
                        time_values.purpose_values.into_iter().collect::<Vec<_>>(),
                    )
                    .expect("time rules");
                system.push_str("\n\n");
                system.push_str(&time_rules);
                system
            };
            assert_eq!(texts[0], expected_system);

            let entries_label = runtime_text
                .render_with("summary_chapter_entries_label", [])
                .expect("entries label");
            let expected_entries = format!(
                "{entries_label}\n[1] {} to {} First stretch.\n\n[2] {} Same minute stretch.\n\n[3] Legacy stretch.",
                format_message_timestamp(t_a),
                format_message_timestamp(t_b),
                format_message_timestamp(t_same_a),
            );
            assert_eq!(texts[2], expected_entries);
        }

        #[tokio::test]
        async fn chapters_chapter_request_without_time_awareness_is_unchanged() {
            let mut harness = Harness::new("chapter-no-time-aware").await;
            harness.time_awareness_enabled = false;
            let count = u64::try_from(harness.sources.len()).expect("len");
            let (run_id, attempt_id, handle) = harness.open_run(0, count);
            let now = TimestampMillis::new(1_700_020_000_000);
            let database = harness.database();
            let run = database.load_dynamic_memory_run(run_id).expect("run");
            let attempt = database
                .load_dynamic_memory_attempt(attempt_id)
                .expect("attempt");
            let runtime_text = crate::generation::runtime_text::RuntimeText::load(
                database,
                BuiltInPromptId::MemoryRuntime,
            )
            .expect("runtime text");

            let mut u1 = unit(MemorySummaryUnitKind::Cycle, 0, (0, 6));
            u1.text = "First stretch.".into();
            u1.time_start = Some(TimestampMillis::new(1_700_000_000_000));
            u1.time_end = Some(TimestampMillis::new(1_700_000_600_000));

            let mut u2 = unit(MemorySummaryUnitKind::Cycle, 0, (6, 12));
            u2.text = "Second stretch.".into();
            u2.time_start = Some(TimestampMillis::new(1_700_010_000_000));
            u2.time_end = Some(TimestampMillis::new(1_700_010_600_000));

            let request = super::super::build_chapter_request(
                &run,
                &attempt,
                &runtime_text,
                None,
                &[&u1, &u2],
                1,
                0,
                12,
                120,
                false,
                &handle,
                None,
                now,
            )
            .expect("chapter request");

            let texts = request_texts(&request);
            let expected_system = runtime_text
                .render_with("summary_chapter_system", [])
                .expect("system");
            let entries_label = runtime_text
                .render_with("summary_chapter_entries_label", [])
                .expect("entries label");
            assert_eq!(texts[0], expected_system);
            assert_eq!(
                texts[2],
                format!("{entries_label}\n[1] First stretch.\n\n[2] Second stretch.")
            );
        }
    }
}
