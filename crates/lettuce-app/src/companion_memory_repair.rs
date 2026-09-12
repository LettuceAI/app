use lettuce_context::PromptVariable as Variable;
use lettuce_conversations::{
    GenerationOperation, InferencePort, InferenceRequest, MessageRole, ProviderContextPart,
    ProviderNeutralContext, ProviderNeutralMessage, ProviderReplayArtifactPort,
};
use lettuce_jobs::handle::JobHandle;
use lettuce_memory::{
    DynamicMemoryInferenceRound, DynamicMemoryRun, DynamicMemoryRunRepository,
    DynamicMemoryRunRepositoryError, MemoryCategory, MemoryToolOutcome, MemoryToolSkipReason,
};
use lettuce_types::{
    DynamicMemoryAttemptId, GenerationAttemptId, GenerationTurnId, RequestId, TimestampMillis,
};
use lettuce_usage::JobUsageLedger;

use crate::companion_memory_inference::MemoryFallbackKind;
use crate::{
    CompanionMemoryInferenceError, cleanup_outcome_replays, plan_memory_round,
    run_memory_request_with_fallback,
};

/// A create call the manager left without a usable category, as legacy queued
/// it for repair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryRepairCandidate {
    pub text: String,
    pub important: bool,
}

#[derive(Debug)]
pub struct CompanionMemoryRepairCoordinator<'a, R: ?Sized, I: ?Sized> {
    repository: &'a R,
    inference: &'a I,
}

impl<'a, R: ?Sized, I: ?Sized> CompanionMemoryRepairCoordinator<'a, R, I> {
    #[must_use]
    pub const fn new(repository: &'a R, inference: &'a I) -> Self {
        Self {
            repository,
            inference,
        }
    }
}

impl<
    R: DynamicMemoryRunRepository
        + ProviderReplayArtifactPort
        + JobUsageLedger
        + crate::runtime_text::RuntimeTextSource
        + ?Sized,
    I: InferencePort + ?Sized,
> CompanionMemoryRepairCoordinator<'_, R, I>
{
    /// Legacy's post-cycle repair pass: every create the manager skipped for a
    /// missing or invalid category is re-tagged in one request and admitted as
    /// a final round of create calls.
    pub async fn repair_round(
        &self,
        run: &DynamicMemoryRun,
        attempt_id: DynamicMemoryAttemptId,
        handle: &JobHandle,
        stream_sink: Option<RequestId>,
        now: TimestampMillis,
    ) -> Result<Option<DynamicMemoryInferenceRound>, CompanionMemoryRepairError> {
        let rounds = self
            .repository
            .list_dynamic_memory_inference_rounds(run.id, attempt_id)?;
        let Some(last) = rounds.last() else {
            return Ok(None);
        };
        let candidates = self.candidates(run, attempt_id, &rounds)?;
        if candidates.is_empty() {
            return Ok(None);
        }
        let next_ordinal = last
            .ordinal
            .checked_add(1)
            .ok_or(CompanionMemoryRepairError::RoundLimit)?;
        let next_call = last
            .first_call_ordinal
            .checked_add(
                u16::try_from(last.calls.len())
                    .map_err(|_| CompanionMemoryRepairError::RoundLimit)?,
            )
            .ok_or(CompanionMemoryRepairError::RoundLimit)?;
        let text = crate::runtime_text::RuntimeText::load(
            self.repository,
            crate::BuiltInPromptId::MemoryRuntime,
        )
        .map_err(|_| CompanionMemoryRepairError::InvalidPrompt)?;
        let context = repair_context(&text, &candidates)?;
        let request = InferenceRequest {
            turn_id: GenerationTurnId::from_uuid(run.id.as_uuid()),
            attempt_id: GenerationAttemptId::from_uuid(attempt_id.as_uuid()),
            operation: GenerationOperation::Send,
            profile: run.profile.clone(),
            context: context.clone(),
            cancellation: Some(handle.id()),
            stream_sink,
            media_grants: Vec::new(),
            tools: Some(lettuce_memory::memory_repair_tool_request(&|key| {
                text.render_with(key, []).unwrap_or_default()
            })),
        };
        request
            .validate()
            .map_err(|_| CompanionMemoryRepairError::InvalidPrompt)?;
        let mut outcome = match run_memory_request_with_fallback(
            self.repository,
            self.inference,
            handle,
            request,
            run.structured_fallback_format,
            MemoryFallbackKind::Repairs,
            now,
        )
        .await
        {
            Ok(outcome) => Some(outcome),
            Err(CompanionMemoryInferenceError::Cancelled) => {
                return Err(CompanionMemoryRepairError::Cancelled);
            }
            Err(_) => None,
        };
        let repairs = outcome
            .as_ref()
            .map(repaired_categories)
            .unwrap_or_default();
        let resolved = resolve(&candidates, &repairs);
        if resolved.is_empty() {
            if let Some(outcome) = &outcome {
                cleanup_outcome_replays(self.repository, outcome)
                    .map_err(|_| CompanionMemoryRepairError::ReplayCleanup)?;
            }
            return Ok(None);
        }
        let outcome = match outcome.take() {
            Some(mut outcome) => {
                rewrite_as_creates(&mut outcome, &resolved);
                outcome
            }
            None => guessed_outcome(&resolved),
        };
        let planned = match plan_memory_round(run, next_ordinal, context, &outcome, now) {
            Ok(planned) => planned,
            Err(error) => {
                cleanup_outcome_replays(self.repository, &outcome)
                    .map_err(|_| CompanionMemoryRepairError::ReplayCleanup)?;
                return Err(CompanionMemoryRepairError::InvalidOutcome(error));
            }
        };
        let admitted = self.repository.admit_dynamic_memory_inference_round(
            run.id,
            attempt_id,
            next_ordinal,
            next_call,
            planned,
        )?;
        Ok(Some(admitted))
    }

    /// Creates skipped for a missing or invalid category, deduplicated by text
    /// and normalized as legacy queued them.
    fn candidates(
        &self,
        run: &DynamicMemoryRun,
        attempt_id: DynamicMemoryAttemptId,
        rounds: &[DynamicMemoryInferenceRound],
    ) -> Result<Vec<MemoryRepairCandidate>, CompanionMemoryRepairError> {
        let mut candidates: Vec<MemoryRepairCandidate> = Vec::new();
        for round in rounds {
            let Some(settlement) = self.repository.load_dynamic_memory_round_settlement(
                run.id,
                attempt_id,
                round.ordinal,
            )?
            else {
                continue;
            };
            for call in &round.calls {
                if call.call.name != "create_memory" {
                    continue;
                }
                if !settlement.results.iter().any(|result| {
                    result.execution_id == call.id
                        && matches!(
                            result.outcome,
                            MemoryToolOutcome::Skipped {
                                reason: MemoryToolSkipReason::MissingCategory
                                    | MemoryToolSkipReason::InvalidCategory
                            }
                        )
                }) {
                    continue;
                }
                let Some(text) = call
                    .call
                    .arguments
                    .get("text")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|text| lettuce_memory::normalize_memory_text(text).ok())
                else {
                    continue;
                };
                if candidates.iter().any(|candidate| candidate.text == text) {
                    continue;
                }
                candidates.push(MemoryRepairCandidate {
                    important: call
                        .call
                        .arguments
                        .get("important")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false),
                    text,
                });
            }
        }
        Ok(candidates)
    }
}

fn repair_context(
    text: &crate::runtime_text::RuntimeText,
    candidates: &[MemoryRepairCandidate],
) -> Result<ProviderNeutralContext, CompanionMemoryRepairError> {
    let render = |key: &str, variables: Vec<(Variable, String)>| {
        text.render_with(key, variables)
            .map_err(|_| CompanionMemoryRepairError::InvalidPrompt)
    };
    let lines = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            render(
                "memory_repair_line",
                vec![
                    (Variable::ItemNumber, (index + 1).to_string()),
                    (Variable::MemoryText, candidate.text.clone()),
                ],
            )
        })
        .collect::<Result<Vec<_>, _>>()?
        .join("\n");
    let categories = lettuce_memory::MEMORY_CATEGORIES
        .map(MemoryCategory::as_str)
        .join(", ");
    let messages = vec![
        message(MessageRole::System, render("memory_repair_rules", vec![])?),
        message(
            MessageRole::User,
            render(
                "memory_repair_input",
                vec![
                    (Variable::MemoryCategories, categories),
                    (Variable::SelectedMemories, lines),
                ],
            )?,
        ),
    ];
    let input_bytes = messages
        .iter()
        .flat_map(|message| &message.parts)
        .map(|part| match part {
            ProviderContextPart::Text { text } => text.len(),
            _ => 0,
        })
        .try_fold(0_usize, usize::checked_add)
        .and_then(|size| u32::try_from(size).ok())
        .ok_or(CompanionMemoryRepairError::InvalidPrompt)?;
    Ok(ProviderNeutralContext {
        budget: lettuce_conversations::ContextBudgetReport {
            selected_messages: u32::try_from(messages.len())
                .map_err(|_| CompanionMemoryRepairError::InvalidPrompt)?,
            omitted_messages: 0,
            input_bytes,
            estimated_input_tokens: input_bytes.saturating_add(3) / 4,
            truncated: false,
        },
        messages,
        attributions: lettuce_conversations::ContextAttributions::default(),
    })
}

fn message(role: MessageRole, text: String) -> ProviderNeutralMessage {
    ProviderNeutralMessage {
        role,
        parts: vec![ProviderContextPart::Text { text }],
    }
}

fn repaired_categories(
    outcome: &lettuce_conversations::InferenceOutcome,
) -> Vec<(String, MemoryCategory)> {
    outcome
        .candidates
        .iter()
        .flat_map(|candidate| &candidate.tool_calls)
        .filter(|call| call.name == lettuce_memory::MEMORY_REPAIR_TOOL_NAME)
        .filter_map(|call| {
            let text = call.arguments.get("text")?.as_str()?.to_owned();
            let category = MemoryCategory::parse(call.arguments.get("category")?.as_str()?.trim())?;
            Some((text, category))
        })
        .collect()
}

/// Legacy kept only the answered candidates, and fell back to the keyword
/// guess for every candidate when the request answered with nothing.
fn resolve(
    candidates: &[MemoryRepairCandidate],
    repairs: &[(String, MemoryCategory)],
) -> Vec<(MemoryRepairCandidate, MemoryCategory)> {
    if repairs.is_empty() {
        return candidates
            .iter()
            .map(|candidate| {
                let category = lettuce_memory::guess_memory_category(&candidate.text);
                (candidate.clone(), category)
            })
            .collect();
    }
    candidates
        .iter()
        .filter_map(|candidate| {
            repairs
                .iter()
                .find(|(text, _)| text == &candidate.text)
                .map(|(_, category)| (candidate.clone(), *category))
        })
        .collect()
}

/// The admitted round records the creates the repair produced; the raw
/// response stays in the round's provider replay.
fn rewrite_as_creates(
    outcome: &mut lettuce_conversations::InferenceOutcome,
    resolved: &[(MemoryRepairCandidate, MemoryCategory)],
) {
    let Some(candidate) = outcome.candidates.first_mut() else {
        return;
    };
    let replay = candidate.provider_replay.clone();
    candidate.tool_calls = resolved
        .iter()
        .enumerate()
        .map(
            |(index, (repair, category))| lettuce_conversations::ProposedToolCall {
                provider_call_id: Some(format!("repair_create_{}", index + 1)),
                name: "create_memory".to_owned(),
                arguments: serde_json::json!({
                    "text": repair.text,
                    "category": category.as_str(),
                    "important": repair.important
                }),
                raw_arguments: None,
                provider_replay: replay.clone(),
            },
        )
        .collect();
}

/// Legacy still applied the keyword guess when both the repair request and its
/// structured fallback failed, so the round is planned without any response.
fn guessed_outcome(
    resolved: &[(MemoryRepairCandidate, MemoryCategory)],
) -> lettuce_conversations::InferenceOutcome {
    let mut outcome = lettuce_conversations::InferenceOutcome {
        provider_response_id: None,
        candidates: vec![lettuce_conversations::InferenceCandidate {
            ordinal: 0,
            parts: Vec::new(),
            tool_calls: Vec::new(),
            provider_replay: None,
        }],
        usage: None,
        finish_reason: lettuce_conversations::FinishReason::Stop,
        provider_finish_reason: None,
        provider_request_id: None,
        warning_codes: Vec::new(),
    };
    rewrite_as_creates(&mut outcome, resolved);
    outcome
}

#[derive(Debug, thiserror::Error)]
pub enum CompanionMemoryRepairError {
    #[error("background memory run persistence failed: {0}")]
    Run(#[from] DynamicMemoryRunRepositoryError),
    #[error("memory repair prompt is unavailable")]
    InvalidPrompt,
    #[error("memory repair response is invalid: {0}")]
    InvalidOutcome(CompanionMemoryInferenceError),
    #[error("memory repair round limit reached")]
    RoundLimit,
    #[error("memory repair replay cleanup failed")]
    ReplayCleanup,
    #[error("memory repair was cancelled")]
    Cancelled,
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, sync::Mutex};

    use lettuce_conversations::{
        FinishReason, InferenceCandidate, InferenceOutcome, PortError, ProposedToolCall,
        ReplayArtifactRef, ToolPolicy,
    };
    use lettuce_memory::{
        DynamicMemoryBackgroundRoundSettlement, DynamicMemoryRoundFinishReason,
        DynamicMemorySummaryWindow, DynamicMemoryToolCallEvidence, MemorySpaceSnapshot,
        MemoryToolResult, NewDynamicMemoryInferenceRound,
    };
    use lettuce_models::{
        CapabilityStatus, ChatParameterResolutionInput, ChatRequirements, ExpectedModelIdentity,
        ModelCapabilities, ModelKind, ModelProfile, ModelProfileConfig, ProviderAccount,
        ProviderConfig, ProviderProtocol,
    };
    use lettuce_settings::SecretOwnerId;
    use lettuce_types::{
        ConversationId, DynamicMemoryRunId, JobId, MemorySpaceId, ModelProfileId,
        ProviderAccountId, Revision, ToolExecutionId,
    };

    use super::*;

    struct ScriptedInference {
        outcomes: Mutex<VecDeque<Result<InferenceOutcome, PortError>>>,
        requests: Mutex<Vec<InferenceRequest>>,
    }

    #[async_trait::async_trait]
    impl InferencePort for ScriptedInference {
        async fn run(&self, request: InferenceRequest) -> Result<InferenceOutcome, PortError> {
            self.requests.lock().expect("requests").push(request);
            self.outcomes
                .lock()
                .expect("outcomes")
                .pop_front()
                .expect("scripted outcome")
        }
    }

    #[derive(Debug, Default)]
    struct Repository {
        rounds: Mutex<Vec<DynamicMemoryInferenceRound>>,
        settlements: Mutex<Vec<DynamicMemoryBackgroundRoundSettlement>>,
    }

    impl crate::runtime_text::RuntimeTextSource for Repository {
        fn runtime_text_document(
            &self,
            id: crate::BuiltInPromptId,
        ) -> Result<Option<lettuce_context::PromptDocument>, lettuce_context::PromptRepositoryError>
        {
            Ok(Some(crate::built_in_prompts::seed_document(id)))
        }
    }

    impl JobUsageLedger for Repository {
        fn admit_job_usage(
            &self,
            _record: lettuce_usage::JobInferenceUsage,
        ) -> Result<(), lettuce_usage::UsageLedgerError> {
            Ok(())
        }
        fn settle_job_usage(
            &self,
            _id: lettuce_types::UsageEventId,
            _result: lettuce_usage::JobInferenceUsageResult,
        ) -> Result<(), lettuce_usage::UsageLedgerError> {
            Ok(())
        }
        fn job_usage(
            &self,
            _job_id: JobId,
        ) -> Result<Vec<lettuce_usage::JobInferenceUsage>, lettuce_usage::UsageLedgerError>
        {
            Ok(Vec::new())
        }
    }

    impl ProviderReplayArtifactPort for Repository {
        fn stage_provider_replay(
            &self,
            _draft: lettuce_conversations::ReplayArtifactDraft,
        ) -> Result<ReplayArtifactRef, lettuce_conversations::ArtifactError> {
            unimplemented!()
        }
        fn materialize_provider_replay(
            &self,
            _reference: &ReplayArtifactRef,
        ) -> Result<
            lettuce_conversations::ProtectedArtifactBytes,
            lettuce_conversations::ArtifactError,
        > {
            unimplemented!()
        }
        fn cleanup_orphan_provider_replay(
            &self,
            _id: lettuce_types::ReplayArtifactId,
        ) -> Result<(), lettuce_conversations::ArtifactError> {
            Ok(())
        }
    }

    impl DynamicMemoryRunRepository for Repository {
        fn admit_dynamic_memory_run_attempt(
            &self,
            _admission: lettuce_memory::NewDynamicMemoryRunAttempt,
        ) -> Result<lettuce_memory::DynamicMemoryRunAttemptAdmission, DynamicMemoryRunRepositoryError>
        {
            unimplemented!()
        }
        fn load_dynamic_memory_run(
            &self,
            _id: DynamicMemoryRunId,
        ) -> Result<DynamicMemoryRun, DynamicMemoryRunRepositoryError> {
            unimplemented!()
        }
        fn load_dynamic_memory_attempt(
            &self,
            _id: DynamicMemoryAttemptId,
        ) -> Result<lettuce_memory::DynamicMemoryAttempt, DynamicMemoryRunRepositoryError> {
            unimplemented!()
        }
        fn load_latest_dynamic_memory_attempt(
            &self,
            _run_id: DynamicMemoryRunId,
        ) -> Result<lettuce_memory::DynamicMemoryAttempt, DynamicMemoryRunRepositoryError> {
            unimplemented!()
        }
        fn transition_dynamic_memory_attempt(
            &self,
            _id: DynamicMemoryAttemptId,
            _expected_revision: Revision,
            _next: lettuce_memory::DynamicMemoryAttemptStatus,
            _failure: Option<lettuce_memory::DynamicMemoryAttemptFailureCode>,
            _at: TimestampMillis,
        ) -> Result<lettuce_memory::DynamicMemoryAttempt, DynamicMemoryRunRepositoryError> {
            unimplemented!()
        }
        fn recover_dynamic_memory_attempt(
            &self,
            _recovery: lettuce_memory::NewDynamicMemoryAttemptRecovery,
        ) -> Result<lettuce_memory::DynamicMemoryAttemptRecovery, DynamicMemoryRunRepositoryError>
        {
            unimplemented!()
        }
        fn admit_dynamic_memory_inference_round(
            &self,
            run_id: DynamicMemoryRunId,
            attempt_id: DynamicMemoryAttemptId,
            expected_round_ordinal: u8,
            expected_next_call_ordinal: u16,
            round: NewDynamicMemoryInferenceRound,
        ) -> Result<DynamicMemoryInferenceRound, DynamicMemoryRunRepositoryError> {
            round
                .validate()
                .map_err(|_| DynamicMemoryRunRepositoryError::Invalid)?;
            let admitted = DynamicMemoryInferenceRound {
                run_id,
                attempt_id,
                ordinal: expected_round_ordinal,
                first_call_ordinal: expected_next_call_ordinal,
                request_context: round.request_context,
                parts: round.parts,
                provider_replay: round.provider_replay,
                usage: round.usage,
                finish_reason: round.finish_reason,
                provider_request_id: round.provider_request_id,
                calls: round
                    .calls
                    .into_iter()
                    .enumerate()
                    .map(|(index, call)| DynamicMemoryToolCallEvidence {
                        id: call.id,
                        run_id,
                        attempt_id,
                        round_ordinal: expected_round_ordinal,
                        ordinal: expected_next_call_ordinal
                            + u16::try_from(index).expect("call ordinal"),
                        definition_version: call.definition_version,
                        call: call.call,
                        admitted_at: round.admitted_at,
                    })
                    .collect(),
                admitted_at: round.admitted_at,
            };
            self.rounds.lock().expect("rounds").push(admitted.clone());
            Ok(admitted)
        }
        fn list_dynamic_memory_inference_rounds(
            &self,
            _run_id: DynamicMemoryRunId,
            _attempt_id: DynamicMemoryAttemptId,
        ) -> Result<Vec<DynamicMemoryInferenceRound>, DynamicMemoryRunRepositoryError> {
            Ok(self.rounds.lock().expect("rounds").clone())
        }
        fn list_dynamic_memory_tool_calls(
            &self,
            _run_id: DynamicMemoryRunId,
            _attempt_id: DynamicMemoryAttemptId,
        ) -> Result<Vec<DynamicMemoryToolCallEvidence>, DynamicMemoryRunRepositoryError> {
            Ok(Vec::new())
        }
        fn load_dynamic_memory_round_settlement(
            &self,
            _run_id: DynamicMemoryRunId,
            _attempt_id: DynamicMemoryAttemptId,
            round_ordinal: u8,
        ) -> Result<Option<DynamicMemoryBackgroundRoundSettlement>, DynamicMemoryRunRepositoryError>
        {
            Ok(self
                .settlements
                .lock()
                .expect("settlements")
                .iter()
                .find(|settlement| settlement.round_ordinal == round_ordinal)
                .cloned())
        }
    }

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
                lorebook_generator_parameters: Default::default(),
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
            output_policy: lettuce_conversations::OutputPolicy::Plain,
            safety_policy: lettuce_conversations::SafetyContext::Standard,
            correlation_id: None,
        }
    }

    fn run() -> DynamicMemoryRun {
        let space_id = MemorySpaceId::new();
        DynamicMemoryRun {
            id: DynamicMemoryRunId::new(),
            conversation_id: ConversationId::new(),
            space_id,
            starting_memory: MemorySpaceSnapshot {
                id: space_id,
                revision: Revision::INITIAL,
                items: Vec::new(),
            },
            source_messages: Vec::new(),
            profile: profile(),
            time_awareness_enabled: false,
            supersession_enabled: false,
            structured_fallback_format: lettuce_memory::DynamicMemoryStructuredFallbackFormat::Json,
            summary_window: DynamicMemorySummaryWindow {
                message_interval: 4,
                start: 0,
                end: 2,
            },
            tool_request: crate::companion_memory_run::test_memory_tool_request(false, false),
            created_at: TimestampMillis::new(1),
        }
    }

    fn create_call(id: ToolExecutionId, ordinal: u16, text: &str) -> DynamicMemoryToolCallEvidence {
        DynamicMemoryToolCallEvidence {
            id,
            run_id: DynamicMemoryRunId::new(),
            attempt_id: DynamicMemoryAttemptId::new(),
            round_ordinal: 0,
            ordinal,
            definition_version: 1,
            call: ProposedToolCall {
                provider_call_id: Some(format!("call_{ordinal}")),
                name: "create_memory".to_owned(),
                arguments: serde_json::json!({"text": text, "important": true}),
                raw_arguments: None,
                provider_replay: None,
            },
            admitted_at: TimestampMillis::new(1),
        }
    }

    fn skipped(
        execution_id: ToolExecutionId,
        reason: MemoryToolSkipReason,
    ) -> lettuce_memory::MemoryToolResult {
        MemoryToolResult {
            execution_id,
            outcome: MemoryToolOutcome::Skipped { reason },
        }
    }

    fn repair_outcome(calls: Vec<ProposedToolCall>) -> InferenceOutcome {
        InferenceOutcome {
            provider_response_id: Some("repair".to_owned()),
            candidates: vec![InferenceCandidate {
                ordinal: 0,
                parts: Vec::new(),
                tool_calls: calls,
                provider_replay: None,
            }],
            usage: None,
            finish_reason: FinishReason::Stop,
            provider_finish_reason: Some("stop".to_owned()),
            provider_request_id: None,
            warning_codes: Vec::new(),
        }
    }

    fn retag(text: &str, category: &str) -> ProposedToolCall {
        ProposedToolCall {
            provider_call_id: Some(format!("retag_{category}")),
            name: lettuce_memory::MEMORY_REPAIR_TOOL_NAME.to_owned(),
            arguments: serde_json::json!({"text": text, "category": category}),
            raw_arguments: None,
            provider_replay: None,
        }
    }

    fn fixture(
        run: &DynamicMemoryRun,
        attempt_id: DynamicMemoryAttemptId,
        results: Vec<MemoryToolResult>,
        calls: Vec<DynamicMemoryToolCallEvidence>,
    ) -> Repository {
        let repository = Repository::default();
        repository
            .rounds
            .lock()
            .expect("rounds")
            .push(DynamicMemoryInferenceRound {
                run_id: run.id,
                attempt_id,
                ordinal: 0,
                first_call_ordinal: 0,
                request_context: ProviderNeutralContext {
                    budget: lettuce_conversations::ContextBudgetReport::default(),
                    messages: vec![message(MessageRole::User, "memory input".to_owned())],
                    attributions: lettuce_conversations::ContextAttributions::default(),
                },
                parts: Vec::new(),
                provider_replay: None,
                usage: None,
                finish_reason: DynamicMemoryRoundFinishReason::Stop,
                provider_request_id: None,
                calls,
                admitted_at: TimestampMillis::new(1),
            });
        repository.settlements.lock().expect("settlements").push(
            DynamicMemoryBackgroundRoundSettlement {
                run_id: run.id,
                attempt_id,
                round_ordinal: 0,
                space_id: run.space_id,
                expected_memory_revision: Revision::INITIAL,
                resulting_memory_revision: Revision::INITIAL,
                results,
                settled_at: TimestampMillis::new(2),
            },
        );
        repository
    }

    #[tokio::test]
    async fn answered_repairs_become_creates_in_a_final_round() {
        let run = run();
        let attempt_id = DynamicMemoryAttemptId::new();
        let (first, second) = (ToolExecutionId::new(), ToolExecutionId::new());
        let repository = fixture(
            &run,
            attempt_id,
            vec![
                skipped(first, MemoryToolSkipReason::MissingCategory),
                skipped(second, MemoryToolSkipReason::InvalidCategory),
            ],
            vec![
                create_call(first, 0, "Mira trusts the captain"),
                create_call(second, 1, "The village burned down"),
            ],
        );
        let inference = ScriptedInference {
            outcomes: Mutex::new(VecDeque::from([Ok(repair_outcome(vec![
                retag("Mira trusts the captain", "relationship"),
                retag("The village burned down", "plot_event"),
            ]))])),
            requests: Mutex::new(Vec::new()),
        };
        let round = CompanionMemoryRepairCoordinator::new(&repository, &inference)
            .repair_round(
                &run,
                attempt_id,
                &JobHandle::new(JobId::new()),
                None,
                TimestampMillis::new(3),
            )
            .await
            .expect("repair round")
            .expect("repaired round");
        assert_eq!((round.ordinal, round.first_call_ordinal), (1, 2));
        assert_eq!(round.calls.len(), 2);
        for (index, (text, category)) in [
            ("Mira trusts the captain", "relationship"),
            ("The village burned down", "plot_event"),
        ]
        .into_iter()
        .enumerate()
        {
            let call = &round.calls[index].call;
            assert_eq!(call.name, "create_memory");
            assert_eq!(
                call.provider_call_id.as_deref(),
                Some(format!("repair_create_{}", index + 1).as_str())
            );
            assert_eq!(
                call.arguments,
                serde_json::json!({"text": text, "category": category, "important": true})
            );
        }
        let requests = inference.requests.lock().expect("requests");
        assert_eq!(requests.len(), 1);
        let tools = requests[0].tools.as_ref().expect("repair tools");
        assert_eq!(tools.definitions[0].name, "retag_memory");
        assert!(matches!(
            requests[0].context.messages[1].parts.as_slice(),
            [ProviderContextPart::Text { text }]
                if text.contains("1. Mira trusts the captain")
                    && text.contains("2. The village burned down")
                    && text.contains("character_trait, relationship")
        ));
    }

    #[tokio::test]
    async fn an_unanswered_repair_falls_back_to_the_keyword_guess() {
        let run = run();
        let attempt_id = DynamicMemoryAttemptId::new();
        let execution_id = ToolExecutionId::new();
        let repository = fixture(
            &run,
            attempt_id,
            vec![skipped(execution_id, MemoryToolSkipReason::MissingCategory)],
            vec![create_call(execution_id, 0, "She decided to stay")],
        );
        let inference = ScriptedInference {
            outcomes: Mutex::new(VecDeque::from([Ok(repair_outcome(vec![retag(
                "She decided to stay",
                "milestone",
            )]))])),
            requests: Mutex::new(Vec::new()),
        };
        let round = CompanionMemoryRepairCoordinator::new(&repository, &inference)
            .repair_round(
                &run,
                attempt_id,
                &JobHandle::new(JobId::new()),
                None,
                TimestampMillis::new(3),
            )
            .await
            .expect("repair round")
            .expect("repaired round");
        assert_eq!(round.calls.len(), 1);
        assert_eq!(
            round.calls[0].call.arguments,
            serde_json::json!({
                "text": "She decided to stay",
                "category": "plot_event",
                "important": true
            })
        );
    }

    #[tokio::test]
    async fn a_failed_repair_request_still_applies_the_keyword_guess() {
        let run = run();
        let attempt_id = DynamicMemoryAttemptId::new();
        let execution_id = ToolExecutionId::new();
        let repository = fixture(
            &run,
            attempt_id,
            vec![skipped(execution_id, MemoryToolSkipReason::InvalidCategory)],
            vec![create_call(execution_id, 0, "Mira prefers tea")],
        );
        let inference = ScriptedInference {
            outcomes: Mutex::new(VecDeque::from([
                Err(PortError::Unavailable),
                Err(PortError::Unavailable),
            ])),
            requests: Mutex::new(Vec::new()),
        };
        let round = CompanionMemoryRepairCoordinator::new(&repository, &inference)
            .repair_round(
                &run,
                attempt_id,
                &JobHandle::new(JobId::new()),
                None,
                TimestampMillis::new(3),
            )
            .await
            .expect("repair round")
            .expect("repaired round");
        assert_eq!(round.calls.len(), 1);
        assert_eq!(
            round.calls[0].call.arguments,
            serde_json::json!({
                "text": "Mira prefers tea",
                "category": "preference",
                "important": true
            })
        );
        assert_eq!(inference.requests.lock().expect("requests").len(), 2);
    }

    #[tokio::test]
    async fn rounds_without_skipped_categories_are_left_alone() {
        let run = run();
        let attempt_id = DynamicMemoryAttemptId::new();
        let execution_id = ToolExecutionId::new();
        let repository = fixture(
            &run,
            attempt_id,
            vec![skipped(execution_id, MemoryToolSkipReason::EmptyText)],
            vec![create_call(execution_id, 0, "Mira trusts the captain")],
        );
        let inference = ScriptedInference {
            outcomes: Mutex::new(VecDeque::new()),
            requests: Mutex::new(Vec::new()),
        };
        assert!(
            CompanionMemoryRepairCoordinator::new(&repository, &inference)
                .repair_round(
                    &run,
                    attempt_id,
                    &JobHandle::new(JobId::new()),
                    None,
                    TimestampMillis::new(3),
                )
                .await
                .expect("repair round")
                .is_none()
        );
        assert!(inference.requests.lock().expect("requests").is_empty());
    }
}
