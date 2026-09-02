use chrono::{Local, LocalResult, TimeZone};
use lettuce_context::{
    LifecycleStatus, PromptDocument, PromptEntryChatMode, PromptEntryInfoSource, PromptEntryRole,
    PromptPurpose, PromptRenderContext, PromptRenderError, PromptRenderValues, PromptVariable,
    render_prompt,
};
use lettuce_conversations::{
    ContextAttributions, ContextBudgetReport, ConversationReader, ConversationRepositoryError,
    FinishReason, GenerationOperation, InferenceOutcome, InferencePort, InferenceRequest,
    MessagePart, MessageRenderSource, MessageRole, PortError, PromptAttribution,
    ProviderContextPart, ProviderNeutralContext, ProviderNeutralMessage,
    ProviderReplayArtifactPort,
};
use lettuce_jobs::handle::JobHandle;
use lettuce_memory::{
    DynamicMemoryAttempt, DynamicMemoryAttemptStatus, DynamicMemoryInferenceRound,
    DynamicMemoryRoundFinishReason, DynamicMemoryRun, DynamicMemoryRunRepository,
    DynamicMemoryRunRepositoryError, MemoryPolicy, MemoryRepository, MemoryRepositoryError,
    MemorySpaceSnapshot, NewDynamicMemoryInferenceRound, NewDynamicMemoryToolCall,
};
use lettuce_types::{
    DynamicMemoryAttemptId, DynamicMemoryRunId, GenerationAttemptId, GenerationTurnId, RequestId,
    TimestampMillis, ToolExecutionId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanionMemoryFirstRoundResult {
    pub round: DynamicMemoryInferenceRound,
    pub replayed: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum CompanionMemoryInferenceError {
    #[error("background memory ownership is invalid")]
    InvalidOwnership,
    #[error("background memory prompt is invalid")]
    InvalidPrompt,
    #[error("background memory source content is invalid")]
    InvalidSource,
    #[error("background memory request is too large")]
    ContextTooLarge,
    #[error("background memory inference was cancelled")]
    Cancelled,
    #[error("background memory inference returned multiple candidates")]
    MultipleCandidates,
    #[error("background memory inference returned no tool calls")]
    NoToolCalls,
    #[error("background memory inference returned invalid content with tool calls")]
    MixedToolAndContent,
    #[error("background memory inference returned inconsistent protected replay material")]
    InvalidSignedReplay,
    #[error("background memory inference called an undeclared tool")]
    UndeclaredTool,
    #[error("background memory conversation read failed: {0}")]
    Conversation(ConversationRepositoryError),
    #[error("background memory read failed: {0}")]
    Memory(MemoryRepositoryError),
    #[error("background memory run failed: {0}")]
    Run(DynamicMemoryRunRepositoryError),
    #[error("background memory prompt rendering failed: {0}")]
    Prompt(PromptRenderError),
    #[error("background memory provider failed: {0}")]
    Inference(PortError),
    #[error("background memory provider replay cleanup failed")]
    ReplayCleanup,
}

#[derive(Debug)]
pub struct CompanionMemoryInferenceCoordinator<'a, R: ?Sized, C: ?Sized, I: ?Sized> {
    repository: &'a R,
    conversations: &'a C,
    inference: &'a I,
}

impl<
    'a,
    R: DynamicMemoryRunRepository + MemoryRepository + ProviderReplayArtifactPort + ?Sized,
    C: ConversationReader + ?Sized,
    I: InferencePort + ?Sized,
> CompanionMemoryInferenceCoordinator<'a, R, C, I>
{
    #[must_use]
    pub const fn new(repository: &'a R, conversations: &'a C, inference: &'a I) -> Self {
        Self {
            repository,
            conversations,
            inference,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn run_first_round(
        &self,
        run_id: DynamicMemoryRunId,
        attempt_id: DynamicMemoryAttemptId,
        prompt: &PromptDocument,
        previous_summary: &str,
        policy: &MemoryPolicy,
        handle: &JobHandle,
        stream_sink: Option<RequestId>,
        now: TimestampMillis,
    ) -> Result<CompanionMemoryFirstRoundResult, CompanionMemoryInferenceError> {
        let run = self
            .repository
            .load_dynamic_memory_run(run_id)
            .map_err(CompanionMemoryInferenceError::Run)?;
        let attempt = self
            .repository
            .load_dynamic_memory_attempt(attempt_id)
            .map_err(CompanionMemoryInferenceError::Run)?;
        validate_ownership(&run, &attempt, handle)?;

        let rounds = self
            .repository
            .list_dynamic_memory_inference_rounds(run.id, attempt.id)
            .map_err(CompanionMemoryInferenceError::Run)?;
        if let Some(first) = rounds.first() {
            if first.run_id != run.id || first.attempt_id != attempt.id || first.ordinal != 0 {
                return Err(CompanionMemoryInferenceError::InvalidOwnership);
            }
            return Ok(CompanionMemoryFirstRoundResult {
                round: first.clone(),
                replayed: true,
            });
        }

        if handle.cancellation_token().is_cancelled() {
            self.cancel_attempt(&attempt, now)?;
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
        if memory.id != run.space_id {
            return Err(CompanionMemoryInferenceError::InvalidOwnership);
        }
        let sources = materialize_sources(self.conversations, &run)?;
        let request = build_first_request(
            &run,
            &attempt,
            prompt,
            previous_summary,
            policy,
            &memory,
            &sources,
            aggregate.conversation.kind.is_group(),
            aggregate.conversation.participants.len(),
            handle,
            stream_sink,
        )?;
        let request_context = request.context.clone();
        let outcome = match self.inference.run(request).await {
            Ok(outcome) => outcome,
            Err(PortError::Cancelled) => {
                self.cancel_attempt(&attempt, now)?;
                return Err(CompanionMemoryInferenceError::Cancelled);
            }
            Err(error) => return Err(CompanionMemoryInferenceError::Inference(error)),
        };
        if handle.cancellation_token().is_cancelled() {
            cleanup_outcome_replays(self.repository, &outcome)?;
            self.cancel_attempt(&attempt, now)?;
            return Err(CompanionMemoryInferenceError::Cancelled);
        }
        let planned = match plan_memory_round(&run, 0, request_context, &outcome, now) {
            Ok(round) => round,
            Err(CompanionMemoryInferenceError::Cancelled) => {
                cleanup_outcome_replays(self.repository, &outcome)?;
                self.cancel_attempt(&attempt, now)?;
                return Err(CompanionMemoryInferenceError::Cancelled);
            }
            Err(error) => {
                cleanup_outcome_replays(self.repository, &outcome)?;
                return Err(error);
            }
        };
        let round = match self
            .repository
            .admit_dynamic_memory_inference_round(run.id, attempt.id, 0, 0, planned)
        {
            Ok(round) => round,
            Err(error) => {
                cleanup_outcome_replays(self.repository, &outcome)?;
                return Err(CompanionMemoryInferenceError::Run(error));
            }
        };
        Ok(CompanionMemoryFirstRoundResult {
            round,
            replayed: false,
        })
    }

    fn cancel_attempt(
        &self,
        attempt: &DynamicMemoryAttempt,
        now: TimestampMillis,
    ) -> Result<(), CompanionMemoryInferenceError> {
        self.repository
            .transition_dynamic_memory_attempt(
                attempt.id,
                attempt.revision,
                DynamicMemoryAttemptStatus::Cancelled,
                None,
                now,
            )
            .map_err(CompanionMemoryInferenceError::Run)?;
        Ok(())
    }
}

fn validate_ownership(
    run: &DynamicMemoryRun,
    attempt: &DynamicMemoryAttempt,
    handle: &JobHandle,
) -> Result<(), CompanionMemoryInferenceError> {
    if attempt.run_id != run.id
        || attempt.job_id != handle.id()
        || attempt.status != DynamicMemoryAttemptStatus::Processing
    {
        return Err(CompanionMemoryInferenceError::InvalidOwnership);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MaterializedSource {
    message_id: lettuce_types::MessageId,
    role: MessageRole,
    effective_time: TimestampMillis,
    text: String,
}

fn materialize_sources<C: ConversationReader + ?Sized>(
    conversations: &C,
    run: &DynamicMemoryRun,
) -> Result<Vec<MaterializedSource>, CompanionMemoryInferenceError> {
    run.source_messages
        .iter()
        .map(|source| {
            let parts = match source.render_source {
                MessageRenderSource::Revision(id) => {
                    let revision = conversations
                        .get_message_revision(id)
                        .map_err(CompanionMemoryInferenceError::Conversation)?;
                    if revision.message_id != source.message_id {
                        return Err(CompanionMemoryInferenceError::InvalidSource);
                    }
                    revision.parts
                }
                MessageRenderSource::Candidate(id) => {
                    let candidate = conversations
                        .get_candidate(id)
                        .map_err(CompanionMemoryInferenceError::Conversation)?;
                    if candidate.message_id != source.message_id
                        || source.role != MessageRole::Assistant
                    {
                        return Err(CompanionMemoryInferenceError::InvalidSource);
                    }
                    candidate.parts
                }
            };
            let text = parts
                .into_iter()
                .filter_map(|part| match part {
                    MessagePart::Text { text } => Some(text),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            Ok(MaterializedSource {
                message_id: source.message_id,
                role: source.role,
                effective_time: source.effective_time,
                text,
            })
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn build_first_request(
    run: &DynamicMemoryRun,
    attempt: &DynamicMemoryAttempt,
    prompt: &PromptDocument,
    previous_summary: &str,
    policy: &MemoryPolicy,
    memory: &MemorySpaceSnapshot,
    sources: &[MaterializedSource],
    is_group: bool,
    participant_count: usize,
    handle: &JobHandle,
    stream_sink: Option<RequestId>,
) -> Result<InferenceRequest, CompanionMemoryInferenceError> {
    if prompt.status != LifecycleStatus::Active
        || prompt.purpose != PromptPurpose::DynamicMemoryManager
        || memory.id != run.space_id
        || sources.len() != run.source_messages.len()
    {
        return Err(CompanionMemoryInferenceError::InvalidPrompt);
    }
    policy
        .validate()
        .map_err(|_| CompanionMemoryInferenceError::InvalidPrompt)?;
    let current_tokens = memory
        .items
        .iter()
        .filter(|item| !item.is_cold)
        .try_fold(0u32, |total, item| total.checked_add(item.token_count))
        .ok_or(CompanionMemoryInferenceError::ContextTooLarge)?;
    if sources
        .iter()
        .any(|source| !matches!(source.role, MessageRole::User | MessageRole::Assistant))
    {
        return Err(CompanionMemoryInferenceError::InvalidSource);
    }
    let transcript = sources
        .iter()
        .map(|source| {
            let role = match source.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                _ => unreachable!("source roles were validated"),
            };
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
    let memory_lines = memory
        .items
        .iter()
        .filter(|item| item.superseded_by.is_none())
        .map(|item| format!("[{}] {}", item.id, item.text))
        .collect::<Vec<_>>();
    let mut values = PromptRenderValues::default();
    values
        .purpose_values
        .insert(PromptVariable::MaxEntries, policy.max_entries.to_string());
    values.purpose_values.insert(
        PromptVariable::CurrentMemoryTokens,
        current_tokens.to_string(),
    );
    values.purpose_values.insert(
        PromptVariable::HotTokenBudget,
        policy.hot_token_budget.to_string(),
    );
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
                recent_text: transcript.clone(),
                dynamic_memory_enabled: true,
                has_memory_summary: !previous_summary.trim().is_empty(),
                has_key_memories: memory.items.iter().any(|item| item.superseded_by.is_none()),
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
    let runtime_input = format!(
        "Conversation transcript summary:\n{}\n\nRecent transcript lines:\n{}\n\nCurrent memories (with IDs):\n{}",
        previous_summary,
        transcript,
        if memory_lines.is_empty() {
            "none".to_owned()
        } else {
            memory_lines.join("\n")
        }
    );
    let mut messages = rendered
        .relative
        .iter()
        .map(rendered_message)
        .collect::<Result<Vec<_>, _>>()?;
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
                text: runtime_input,
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
    let selected_entry_ids = rendered
        .relative
        .iter()
        .chain(&rendered.in_chat)
        .map(|entry| entry.entry_id)
        .collect();
    let request = InferenceRequest {
        turn_id: GenerationTurnId::from_uuid(run.id.as_uuid()),
        attempt_id: GenerationAttemptId::from_uuid(attempt.id.as_uuid()),
        operation: GenerationOperation::Send,
        profile: run.profile.clone(),
        context: ProviderNeutralContext {
            messages,
            attributions: ContextAttributions {
                prompt: Some(PromptAttribution {
                    document_id: prompt.id,
                    revision: prompt.revision,
                    selected_entry_ids,
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
        tools: Some(run.tool_request.clone()),
    };
    request
        .validate()
        .map_err(|_| CompanionMemoryInferenceError::InvalidPrompt)?;
    Ok(request)
}

fn format_message_timestamp(effective_time: TimestampMillis) -> String {
    let datetime = match Local.timestamp_millis_opt(effective_time.get()) {
        LocalResult::Single(datetime) | LocalResult::Ambiguous(datetime, _) => datetime,
        LocalResult::None => Local::now(),
    };
    format!("<time>{}</time>", datetime.format("%Y-%m-%d %H:%M"))
}

fn rendered_message(
    entry: &lettuce_context::RenderedPromptMessage,
) -> Result<ProviderNeutralMessage, CompanionMemoryInferenceError> {
    if entry.payload.is_some() {
        return Err(CompanionMemoryInferenceError::InvalidPrompt);
    }
    Ok(ProviderNeutralMessage {
        role: match entry.role {
            PromptEntryRole::System => MessageRole::System,
            PromptEntryRole::User => MessageRole::User,
            PromptEntryRole::Assistant => MessageRole::Assistant,
        },
        parts: vec![ProviderContextPart::Text {
            text: entry.content.trim().to_owned(),
        }],
    })
}

fn insert_in_chat_messages(
    messages: &mut Vec<ProviderNeutralMessage>,
    in_chat: Vec<(u32, ProviderNeutralMessage)>,
) {
    let base_len = messages.len();
    let mut inserts = in_chat
        .into_iter()
        .enumerate()
        .map(|(index, (depth, message))| (base_len.saturating_sub(depth as usize), index, message))
        .collect::<Vec<_>>();
    inserts.sort_by_key(|(position, index, _)| (*position, *index));
    for (offset, (position, _, message)) in inserts.into_iter().enumerate() {
        messages.insert((position + offset).min(messages.len()), message);
    }
}

pub(crate) fn plan_memory_round(
    run: &DynamicMemoryRun,
    ordinal: u8,
    request_context: ProviderNeutralContext,
    outcome: &InferenceOutcome,
    now: TimestampMillis,
) -> Result<NewDynamicMemoryInferenceRound, CompanionMemoryInferenceError> {
    outcome
        .validate()
        .map_err(|_| CompanionMemoryInferenceError::NoToolCalls)?;
    if outcome.candidates.len() != 1 {
        return Err(CompanionMemoryInferenceError::MultipleCandidates);
    }
    let candidate = &outcome.candidates[0];
    let finish_reason = match outcome.finish_reason {
        FinishReason::Stop => DynamicMemoryRoundFinishReason::Stop,
        FinishReason::Length => DynamicMemoryRoundFinishReason::Length,
        FinishReason::Cancelled => return Err(CompanionMemoryInferenceError::Cancelled),
        FinishReason::Error => {
            return Err(CompanionMemoryInferenceError::Inference(
                PortError::Rejected,
            ));
        }
    };
    if candidate.tool_calls.is_empty() {
        return Err(CompanionMemoryInferenceError::NoToolCalls);
    }
    if !candidate.parts.is_empty()
        && (candidate.provider_replay.is_none()
            || candidate
                .parts
                .iter()
                .any(|part| !matches!(part, MessagePart::ReasoningSummary { .. })))
    {
        return Err(CompanionMemoryInferenceError::MixedToolAndContent);
    }
    if candidate
        .tool_calls
        .iter()
        .any(|call| call.provider_replay.as_ref() != candidate.provider_replay.as_ref())
    {
        return Err(CompanionMemoryInferenceError::InvalidSignedReplay);
    }
    let calls = candidate
        .tool_calls
        .iter()
        .map(|call| {
            let definition = run
                .tool_request
                .definitions
                .iter()
                .find(|definition| definition.name == call.name)
                .ok_or(CompanionMemoryInferenceError::UndeclaredTool)?;
            Ok(NewDynamicMemoryToolCall {
                id: ToolExecutionId::new(),
                definition_version: definition.version,
                call: call.clone(),
            })
        })
        .collect::<Result<Vec<_>, CompanionMemoryInferenceError>>()?;
    let round = NewDynamicMemoryInferenceRound {
        ordinal,
        request_context,
        parts: candidate.parts.clone(),
        provider_replay: candidate.provider_replay.clone(),
        usage: outcome.usage.clone(),
        finish_reason,
        provider_request_id: outcome.provider_request_id.clone(),
        calls,
        admitted_at: now,
    };
    round
        .validate()
        .map_err(|_| CompanionMemoryInferenceError::NoToolCalls)?;
    Ok(round)
}

pub(crate) fn cleanup_outcome_replays<R: ProviderReplayArtifactPort + ?Sized>(
    repository: &R,
    outcome: &InferenceOutcome,
) -> Result<(), CompanionMemoryInferenceError> {
    let mut ids = std::collections::BTreeSet::new();
    for candidate in &outcome.candidates {
        if let Some(replay) = &candidate.provider_replay {
            ids.insert(replay.artifact_id);
        }
        ids.extend(
            candidate
                .tool_calls
                .iter()
                .filter_map(|call| call.provider_replay.as_ref())
                .map(|replay| replay.artifact_id),
        );
    }
    for id in ids {
        repository
            .cleanup_orphan_provider_replay(id)
            .map_err(|_| CompanionMemoryInferenceError::ReplayCleanup)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use lettuce_context::PromptRepository;
    use lettuce_conversations::{
        InferenceCandidate, InferenceWarningCode, OutputPolicy, ProposedToolCall, SafetyContext,
        ToolPolicy,
    };
    use lettuce_memory::{MemoryCategory, MemoryItem, Score, dynamic_memory_tool_request};
    use lettuce_models::{
        CapabilityStatus, ChatParameterResolutionInput, ChatRequirements, ExpectedModelIdentity,
        ModelCapabilities, ModelKind, ModelProfile, ModelProfileConfig, ProviderAccount,
        ProviderConfig, ProviderProtocol,
    };
    use lettuce_settings::SecretOwnerId;
    use lettuce_types::{
        JobId, MemoryId, MemorySpaceId, MessageCandidateId, MessageId, MessageRevisionId,
        ModelProfileId, ProviderAccountId, Revision,
    };

    use super::*;
    use crate::{AppBackend, BuiltInPromptId};

    fn profile() -> lettuce_conversations::ResolvedInferenceProfile {
        let account_id = ProviderAccountId::new();
        let profile_id = ModelProfileId::new();
        let account = ProviderAccount {
            id: account_id,
            secret_owner_id: SecretOwnerId::new(),
            provider_kind: "ollama".into(),
            protocol: ProviderProtocol::Ollama,
            label: "Ollama".into(),
            endpoint: Some("http://127.0.0.1:11434".into()),
            enabled: true,
            streaming_enabled: false,
            allow_invalid_tls: false,
            api_key_ref: None,
            secret_headers: Vec::new(),
            config: ProviderConfig::Standard,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        };
        let model = ModelProfile {
            id: profile_id,
            provider_account_id: account_id,
            external_model_id: "memory-model".into(),
            display_name: "Memory model".into(),
            kind: ModelKind::Chat,
            config: ModelProfileConfig {
                chat_parameters: Default::default(),
                capabilities: ModelCapabilities {
                    input_modalities: lettuce_models::ModalityCapabilities {
                        text: CapabilityStatus::Supported,
                        ..Default::default()
                    },
                    output_modalities: lettuce_models::ModalityCapabilities {
                        text: CapabilityStatus::Supported,
                        ..Default::default()
                    },
                    tools: CapabilityStatus::Supported,
                    ..Default::default()
                },
            },
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        };
        let expected = ExpectedModelIdentity {
            model_profile_id: profile_id,
            model_revision: model.revision,
            provider_account_id: account_id,
            provider_account_revision: account.revision,
            external_model_id: model.external_model_id.clone(),
            display_name: model.display_name.clone(),
            provider_protocol: account.protocol,
            model_kind: ModelKind::Chat,
        };
        lettuce_conversations::ResolvedInferenceProfile {
            chat_profile: lettuce_models::resolve_chat_profile(
                &expected,
                &model,
                &account,
                &ChatParameterResolutionInput::default(),
                &ChatRequirements {
                    require_tools: true,
                    ..Default::default()
                },
            )
            .expect("profile"),
            tool_policy: ToolPolicy::Required,
            output_policy: OutputPolicy::Plain,
            safety_policy: SafetyContext::Standard,
            correlation_id: None,
        }
    }

    fn run_and_attempt(job_id: JobId) -> (DynamicMemoryRun, DynamicMemoryAttempt) {
        let run_id = DynamicMemoryRunId::new();
        let attempt_id = DynamicMemoryAttemptId::new();
        let space_id = MemorySpaceId::new();
        let now = TimestampMillis::new(1);
        (
            DynamicMemoryRun {
                id: run_id,
                conversation_id: lettuce_types::ConversationId::new(),
                space_id,
                starting_memory: MemorySpaceSnapshot {
                    id: space_id,
                    revision: lettuce_types::Revision::INITIAL,
                    items: Vec::new(),
                },
                source_messages: vec![
                    lettuce_memory::DynamicMemorySourceMessage {
                        message_id: MessageId::new(),
                        role: MessageRole::User,
                        render_source: MessageRenderSource::Revision(MessageRevisionId::new()),
                        effective_time: now,
                    },
                    lettuce_memory::DynamicMemorySourceMessage {
                        message_id: MessageId::new(),
                        role: MessageRole::Assistant,
                        render_source: MessageRenderSource::Candidate(MessageCandidateId::new()),
                        effective_time: now,
                    },
                ],
                profile: profile(),
                time_awareness_enabled: false,
                supersession_enabled: false,
                tool_request: dynamic_memory_tool_request(),
                created_at: now,
            },
            DynamicMemoryAttempt {
                id: attempt_id,
                run_id,
                ordinal: 0,
                retry_parent_id: None,
                job_id,
                status: DynamicMemoryAttemptStatus::Processing,
                failure: None,
                revision: Revision::new(2),
                created_at: now,
                started_at: Some(now),
                finished_at: None,
                updated_at: now,
            },
        )
    }

    #[test]
    fn request_copies_legacy_companion_prompt_and_runtime_input() {
        let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let prompt = PromptRepository::get(
            backend.database(),
            backend
                .built_in_prompt_ids()
                .get(BuiltInPromptId::DynamicMemory),
        )
        .expect("prompt read")
        .expect("prompt");
        let handle = JobHandle::new(JobId::new());
        let (run, attempt) = run_and_attempt(handle.id());
        let memory_id = MemoryId::new();
        let memory = MemorySpaceSnapshot {
            id: run.space_id,
            revision: Revision::INITIAL,
            items: vec![MemoryItem {
                id: memory_id,
                text: "The user prefers tea.".into(),
                category: MemoryCategory::Preference,
                source_message_id: Some(run.source_messages[0].message_id),
                source_role: None,
                observed_at: None,
                observed_time_precision: None,
                superseded_by: None,
                superseded_at: None,
                supersedes: Vec::new(),
                token_count: 7,
                is_cold: false,
                is_pinned: false,
                importance: Score::FULL,
                persistence_importance: Score::FULL,
                prompt_importance: Score::FULL,
                volatility: Score::LEGACY_VOLATILITY,
                access_count: 0,
                created_at: TimestampMillis::new(1),
                last_accessed_at: TimestampMillis::new(1),
            }],
        };
        let policy = MemoryPolicy {
            max_entries: 100,
            hot_token_budget: 128,
            cold_threshold: Score::ZERO,
            delete_confidence_default: Score::HARD_DELETE_THRESHOLD,
            max_hard_delete_ratio_per_cycle: Score::FULL,
        };
        let sources = [
            MaterializedSource {
                message_id: run.source_messages[0].message_id,
                role: MessageRole::User,
                effective_time: run.source_messages[0].effective_time,
                text: "Hello".into(),
            },
            MaterializedSource {
                message_id: run.source_messages[1].message_id,
                role: MessageRole::Assistant,
                effective_time: run.source_messages[1].effective_time,
                text: "Hi".into(),
            },
        ];
        let request = build_first_request(
            &run,
            &attempt,
            &prompt,
            "Prior summary.",
            &policy,
            &memory,
            &sources,
            false,
            2,
            &handle,
            None,
        )
        .expect("request");
        let text_messages = request
            .context
            .messages
            .iter()
            .map(|message| match &message.parts[..] {
                [ProviderContextPart::Text { text }] => (message.role, text.as_str()),
                _ => panic!("text-only memory prompt"),
            })
            .collect::<Vec<_>>();
        assert!(text_messages.iter().any(|(role, text)| {
            *role == MessageRole::System
                && *text
                    == "You maintain a long-term memory index for a conversation transcript. Extract durable facts, reconcile them against existing memories, and update the list without commentary."
        }));
        assert!(
            text_messages
                .iter()
                .any(|(_, text)| { text.contains("Current hot memory usage: 7/128 tokens") })
        );
        assert!(
            text_messages
                .iter()
                .any(|(_, text)| { text.starts_with("Companion memory rules:\n") })
        );
        assert!(
            !text_messages
                .iter()
                .any(|(_, text)| { text.starts_with("Companion temporal memory rules:\n") })
        );
        let (last_role, last_text) = text_messages.last().expect("runtime input");
        assert_eq!(*last_role, MessageRole::User);
        assert_eq!(
            *last_text,
            format!(
                "Conversation transcript summary:\nPrior summary.\n\nRecent transcript lines:\nuser: Hello\nassistant: Hi\n\nCurrent memories (with IDs):\n[{memory_id}] The user prefers tea."
            )
        );
        assert_eq!(request.tools, Some(dynamic_memory_tool_request()));
        assert_eq!(request.cancellation, Some(handle.id()));

        let group_request = build_first_request(
            &run,
            &attempt,
            &prompt,
            "Prior summary.",
            &policy,
            &memory,
            &sources,
            true,
            2,
            &handle,
            None,
        )
        .expect("group request");
        let group_runtime = match &group_request
            .context
            .messages
            .last()
            .expect("group runtime input")
            .parts[..]
        {
            [ProviderContextPart::Text { text }] => text,
            _ => panic!("text-only group runtime input"),
        };
        assert!(group_runtime.contains("Recent transcript lines:\nuser: Hello\nassistant: Hi"));
        assert!(!group_runtime.contains("[message:"));

        let mut time_aware_run = run.clone();
        time_aware_run.time_awareness_enabled = true;
        time_aware_run.supersession_enabled = true;
        time_aware_run.tool_request =
            lettuce_memory::dynamic_memory_tool_request_for_run(true, true);
        let sources = [
            MaterializedSource {
                message_id: time_aware_run.source_messages[0].message_id,
                role: MessageRole::User,
                effective_time: time_aware_run.source_messages[0].effective_time,
                text: "Hello".into(),
            },
            MaterializedSource {
                message_id: time_aware_run.source_messages[1].message_id,
                role: MessageRole::Assistant,
                effective_time: time_aware_run.source_messages[1].effective_time,
                text: String::new(),
            },
        ];
        let request = build_first_request(
            &time_aware_run,
            &attempt,
            &prompt,
            "Prior summary.",
            &policy,
            &memory,
            &sources,
            false,
            2,
            &handle,
            None,
        )
        .expect("time-aware request");
        let runtime_input = match &request
            .context
            .messages
            .last()
            .expect("runtime input")
            .parts[..]
        {
            [ProviderContextPart::Text { text }] => text,
            _ => panic!("text-only runtime input"),
        };
        for source in &sources {
            let prefix = format!(
                "[message:{}] {}: {}",
                source.message_id,
                match source.role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    _ => unreachable!(),
                },
                format_message_timestamp(source.effective_time)
            );
            let expected = if source.text.is_empty() {
                prefix
            } else {
                format!("{prefix} {}", source.text)
            };
            assert!(runtime_input.contains(&expected));
        }
        assert_eq!(
            request.tools,
            Some(lettuce_memory::dynamic_memory_tool_request_for_run(
                true, true
            ))
        );
    }

    #[test]
    fn first_round_requires_one_tool_candidate_and_preserves_calls() {
        let (run, _) = run_and_attempt(JobId::new());
        let request_context = ProviderNeutralContext {
            messages: Vec::new(),
            attributions: Default::default(),
            budget: Default::default(),
        };
        let empty = InferenceOutcome {
            candidates: Vec::new(),
            usage: None,
            finish_reason: FinishReason::Stop,
            provider_finish_reason: None,
            provider_request_id: None,
            warning_codes: Vec::new(),
        };
        assert!(matches!(
            plan_memory_round(
                &run,
                0,
                request_context.clone(),
                &empty,
                TimestampMillis::new(2)
            ),
            Err(CompanionMemoryInferenceError::NoToolCalls)
        ));

        let outcome = InferenceOutcome {
            candidates: vec![InferenceCandidate {
                ordinal: 0,
                parts: Vec::new(),
                tool_calls: vec![ProposedToolCall {
                    provider_call_id: Some("call-1".into()),
                    name: "done".into(),
                    arguments: serde_json::json!({}),
                    raw_arguments: Some("{}".into()),
                    provider_replay: None,
                }],
                provider_replay: None,
            }],
            usage: Some(lettuce_conversations::InferenceUsage {
                input_tokens: 10,
                output_tokens: 2,
            }),
            finish_reason: FinishReason::Stop,
            provider_finish_reason: Some("stop".into()),
            provider_request_id: Some("request-1".into()),
            warning_codes: Vec::<InferenceWarningCode>::new(),
        };
        let round = plan_memory_round(&run, 0, request_context, &outcome, TimestampMillis::new(2))
            .expect("round");
        assert_eq!(round.calls.len(), 1);
        assert_eq!(round.calls[0].call, outcome.candidates[0].tool_calls[0]);
        assert_eq!(round.provider_request_id.as_deref(), Some("request-1"));
    }
}
