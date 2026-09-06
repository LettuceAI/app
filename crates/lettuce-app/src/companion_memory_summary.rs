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
    MemoryRepository, MemorySummaryRepository,
};
use lettuce_types::{
    DynamicMemoryAttemptId, DynamicMemoryRunId, GenerationAttemptId, GenerationTurnId, RequestId,
    TimestampMillis,
};
use lettuce_usage::JobUsageLedger;
use serde_json::json;
use uuid::Uuid;

use crate::{
    CompanionMemoryInferenceError, MemoryEmbeddingEngine, cleanup_outcome_replays,
    format_message_timestamp, insert_in_chat_messages, materialize_sources, rendered_message,
};

const SUMMARY_OUTPUT_REQUEST: &str = "Return only the concise summary for the above conversation window. Use the write_summary tool.";
const SUMMARY_FALLBACK_REQUEST: &str = "Return only the final merged summary as plain text. No tools, no JSON, no markdown, no commentary.";

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
        + ProviderReplayArtifactPort
        + JobUsageLedger
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
        prompt: &PromptDocument,
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
        if attempt.run_id != run.id
            || attempt.job_id != handle.id()
            || attempt.status != lettuce_memory::DynamicMemoryAttemptStatus::Processing
        {
            return Err(CompanionMemoryInferenceError::InvalidOwnership);
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
        let memory = self
            .repository
            .get(run.space_id)
            .map_err(CompanionMemoryInferenceError::Memory)?
            .ok_or(CompanionMemoryInferenceError::InvalidOwnership)?;
        let previous = self
            .repository
            .get_summary(run.space_id)
            .map_err(CompanionMemoryInferenceError::Memory)?;
        if previous
            .as_ref()
            .is_some_and(|summary| summary.window_end > run.summary_window.start)
        {
            return Err(CompanionMemoryInferenceError::InvalidOwnership);
        }
        let sources = materialize_sources(self.conversations, &run)?;
        let request = build_summary_request(
            &run,
            &attempt,
            prompt,
            previous.as_ref().map(|summary| summary.text.as_str()),
            &sources,
            aggregate.conversation.kind.is_group(),
            aggregate.conversation.participants.len(),
            handle,
            stream_sink,
            now,
        )?;
        let (text, request_context, usage, provider_request_id) =
            self.infer_summary(request, handle, now).await?;
        if handle.cancellation_token().is_cancelled() {
            return Err(CompanionMemoryInferenceError::Cancelled);
        }
        let token_count = self.engine.count_tokens(&text).unwrap_or(0);
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

    async fn infer_summary(
        &self,
        request: InferenceRequest,
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
        use crate::job_inference_usage::{JobInferenceError, run_job_inference};

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
            Err(JobInferenceError::Evidence) => {
                return Err(CompanionMemoryInferenceError::Run(
                    lettuce_memory::DynamicMemoryRunRepositoryError::Storage,
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
                text: SUMMARY_FALLBACK_REQUEST.to_owned(),
            }],
        });
        fallback
            .validate()
            .map_err(|_| CompanionMemoryInferenceError::InvalidPrompt)?;
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
                    JobInferenceError::Evidence => CompanionMemoryInferenceError::Run(
                        lettuce_memory::DynamicMemoryRunRepositoryError::Storage,
                    ),
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

fn summary_tool_request() -> ToolRequest {
    ToolRequest {
        definitions: vec![ToolDefinition {
            name: "write_summary".to_owned(),
            description: Some(
                "Return a concise summary of the provided conversation window.".to_owned(),
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "summary": { "type": "string", "description": "Concise summary text" }
                },
                "required": ["summary"]
            }),
            version: 1,
        }],
        choice: ToolChoice::Required,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_summary_request(
    run: &lettuce_memory::DynamicMemoryRun,
    attempt: &lettuce_memory::DynamicMemoryAttempt,
    prompt: &PromptDocument,
    previous_summary: Option<&str>,
    sources: &[crate::companion_memory_inference::MaterializedSource],
    is_group: bool,
    participant_count: usize,
    handle: &JobHandle,
    stream_sink: Option<RequestId>,
    now: TimestampMillis,
) -> Result<InferenceRequest, CompanionMemoryInferenceError> {
    if prompt.status != LifecycleStatus::Active
        || prompt.purpose != PromptPurpose::DynamicMemorySummarizer
        || sources.len() != run.source_messages.len()
    {
        return Err(CompanionMemoryInferenceError::InvalidPrompt);
    }
    let previous = previous_summary
        .filter(|summary| !summary.trim().is_empty())
        .unwrap_or("No previous summary provided.");
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
                companion_mode_enabled: !is_group,
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
    messages.extend(sources.iter().map(|source| ProviderNeutralMessage {
        role: source.role,
        parts: vec![ProviderContextPart::Text {
            text: if run.time_awareness_enabled {
                let timestamp = format_message_timestamp(source.effective_time);
                if source.text.is_empty() {
                    timestamp
                } else {
                    format!("{timestamp} {}", source.text)
                }
            } else {
                source.text.clone()
            },
        }],
    }));
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
                text: SUMMARY_OUTPUT_REQUEST.to_owned(),
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
        .ok_or(CompanionMemoryInferenceError::ContextTooLarge)?;
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
                selected_messages: u32::try_from(sources.len())
                    .map_err(|_| CompanionMemoryInferenceError::ContextTooLarge)?,
                omitted_messages: 0,
                input_bytes,
                estimated_input_tokens: input_bytes.saturating_add(3) / 4,
                truncated: false,
            },
        },
        cancellation: Some(handle.id()),
        stream_sink,
        media_grants: Vec::new(),
        tools: Some(summary_tool_request()),
    };
    request
        .validate()
        .map_err(|_| CompanionMemoryInferenceError::InvalidPrompt)?;
    Ok(request)
}

fn summary_from_outcome(
    outcome: &InferenceOutcome,
    allow_tool: bool,
) -> Result<String, CompanionMemoryInferenceError> {
    outcome
        .validate()
        .map_err(|_| CompanionMemoryInferenceError::NoToolCalls)?;
    if outcome.candidates.len() != 1 {
        return Err(CompanionMemoryInferenceError::MultipleCandidates);
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
            if call.name == "write_summary" {
                if let Some(summary) = call
                    .arguments
                    .get("summary")
                    .and_then(|value| value.as_str())
                {
                    if let Ok(summary) = validate_summary_text(summary) {
                        return Ok(summary);
                    }
                }
            }
        }
    } else if !candidate.tool_calls.is_empty() {
        return Err(CompanionMemoryInferenceError::NoToolCalls);
    }
    let text = candidate
        .parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<String>();
    validate_summary_text(&text).map_err(|_| CompanionMemoryInferenceError::NoToolCalls)
}

fn validate_summary_text(summary: &str) -> Result<String, ()> {
    let normalized = collapse_whitespace(&normalize_llm_output_text(summary));
    if normalized.is_empty() || normalized.len() > 6_000 {
        return Err(());
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
        return Err(());
    }
    Ok(normalized)
}

fn normalize_llm_output_text(raw: &str) -> String {
    let trimmed = raw.trim();
    let without_fences = if trimmed.starts_with("```") {
        let mut lines = trimmed.lines();
        let _ = lines.next();
        let mut body = lines.collect::<Vec<_>>();
        if body.last().is_some_and(|line| line.trim() == "```") {
            body.pop();
        }
        body.join("\n").trim().to_owned()
    } else {
        trimmed.to_owned()
    };
    strip_thinking_tags(&without_fences).trim().to_owned()
}

fn strip_thinking_tags(text: &str) -> String {
    const TAGS: [(&str, &str); 6] = [
        ("<think>", "</think>"),
        ("<thinking>", "</thinking>"),
        ("<reason>", "</reason>"),
        ("<reasoning>", "</reasoning>"),
        ("<|channel>thought", "<channel|>"),
        ("<|channel>", "<channel|>"),
    ];
    let mut content = text.to_owned();
    for (open, close) in TAGS {
        loop {
            let lower = content.to_ascii_lowercase();
            let Some(start) = lower.find(open) else {
                break;
            };
            let tail = start + open.len();
            let end = lower[tail..]
                .find(close)
                .map_or(content.len(), |offset| tail + offset + close.len());
            content.replace_range(start..end, "");
        }
    }
    content
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
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
            input_tokens: first.input_tokens.saturating_add(second.input_tokens),
            output_tokens: first.output_tokens.saturating_add(second.output_tokens),
        }),
        (Some(usage), None) | (None, Some(usage)) => Some(usage),
        (None, None) => None,
    }
}

fn insert_time_values(values: &mut PromptRenderValues, now: TimestampMillis) {
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

    use super::{summary_from_outcome, validate_summary_text};

    #[test]
    fn fallback_usage_preserves_known_details_without_inventing_missing_counts() {
        let usage = |cached, reasoning| {
            Some(lettuce_conversations::InferenceUsage {
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
    fn legacy_summary_validation_strips_fences_thinking_and_whitespace() {
        assert_eq!(
            validate_summary_text("```text\n<think>draft</think> Mira   chose tea.\n```")
                .expect("summary"),
            "Mira chose tea."
        );
        assert!(validate_summary_text("I'm sorry, I cannot summarize that.").is_err());
        assert!(validate_summary_text("write_summary({ summary: 'x' })").is_err());
    }

    #[test]
    fn summary_response_accepts_legacy_tool_or_plain_text() {
        let outcome = |parts, tool_calls| InferenceOutcome {
            provider_response_id: None,
            candidates: vec![InferenceCandidate {
                ordinal: 0,
                parts,
                tool_calls,
                provider_replay: None,
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
                        arguments: json!({"summary":"Mira chose tea."}),
                        raw_arguments: None,
                        provider_replay: None,
                    }],
                ),
                true,
            )
            .expect("tool summary"),
            "Mira chose tea."
        );
        assert_eq!(
            summary_from_outcome(
                &outcome(
                    vec![MessagePart::Text {
                        text: "Mira chose coffee.".into(),
                    }],
                    Vec::new(),
                ),
                true,
            )
            .expect("text summary"),
            "Mira chose coffee."
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
}
