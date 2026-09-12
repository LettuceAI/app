use std::time::Duration;

use lettuce_companions::{CompanionStateOwner, CompanionStateRepository};
use lettuce_context::{PromptDocument, PromptRepository};
use lettuce_conversations::{
    ConversationKind, ConversationReader, ConversationRepositoryError, GenerationOperation,
    InferencePort, MemoryModeSnapshot, OutputPolicy, ResolvedInferenceProfile, SafetyContext,
    SnapshotSelection, ToolPolicy, effective_memory,
};
use lettuce_jobs::{CancellationReason, JobStore, ResourceAvailability, WorkerId};
use lettuce_memory::{DynamicMemoryRunMode, DynamicMemoryStructuredFallbackFormat, MemoryPolicy};
use lettuce_models::{
    ChatParameterResolutionInput, ChatRequirements, ExpectedModelIdentity, ModelProfileRepository,
    ProviderAccountRepository, ProviderProtocol,
};
use lettuce_settings::{GlobalSettingsStore, MemoryRunMode, MemoryStructuredFallbackFormat};
use lettuce_types::{ConversationId, MemoryId, TimestampMillis};

use crate::{
    BuiltInPromptId, CompanionMemoryClaimedWork, CompanionMemoryDispatchCoordinator,
    CompanionMemoryDispatchError, CompanionMemoryJobRunError, CompanionMemoryJobRunner,
    CompanionMemorySettledWork, CompanionPostTurnMemoryAdmission, MAX_COMPANION_POST_TURN_EFFECTS,
    MemoryCreateSeed, MemoryEmbeddingEngine,
};

/// Everything the memory job runner needs that legacy read from live settings
/// and the session at cycle start.
#[derive(Debug, Clone)]
pub struct CompanionMemoryRuntimeInputs {
    pub profile: ResolvedInferenceProfile,
    pub time_awareness_enabled: bool,
    pub supersession_enabled: bool,
    pub structured_fallback_format: DynamicMemoryStructuredFallbackFormat,
    pub summary_prompt: PromptDocument,
    pub memory_prompt: PromptDocument,
    pub policy: MemoryPolicy,
    pub duplicate_threshold: lettuce_memory::Score,
}

/// Why a claimed cycle could not resolve its runtime inputs; the job is
/// rescheduled so a settings fix lets the same window run, as legacy retried
/// the cycle on the next turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CompanionMemoryRuntimeInputError {
    #[error("summarisation model not configured")]
    MissingModel,
    #[error("summarisation model is invalid")]
    InvalidModel,
    #[error("dynamic memory prompt is missing")]
    MissingPrompt,
    #[error("dynamic memory settings are invalid")]
    InvalidSettings,
    #[error("runtime input storage failed")]
    Storage,
}

#[derive(Debug, thiserror::Error)]
pub enum CompanionMemoryHostError {
    #[error("post-turn memory settings are unavailable: {0}")]
    Settings(lettuce_settings::GlobalSettingsStoreError),
    #[error("post-turn memory conversation lookup failed: {0}")]
    Conversation(ConversationRepositoryError),
    #[error("post-turn memory companion lookup failed: {0:?}")]
    Companion(lettuce_companions::CompanionStateRepositoryError),
    #[error("post-turn memory dispatch failed: {0}")]
    Dispatch(#[from] CompanionMemoryDispatchError),
    #[error("post-turn memory runtime inputs are unavailable: {0}")]
    RuntimeInputs(CompanionMemoryRuntimeInputError),
}

/// Every port the post-turn memory host reads; the composition root's
/// database is one.
pub trait CompanionMemoryHostSources:
    lettuce_memory::DynamicMemoryRunRepository
    + lettuce_memory::MemoryRepository
    + lettuce_memory::MemorySummaryRepository
    + lettuce_memory::DynamicMemoryApprovalRepository
    + lettuce_embeddings::MemoryEmbeddingRepository
    + lettuce_conversations::ProviderReplayArtifactPort
    + lettuce_usage::JobUsageLedger
    + lettuce_companions::CompanionTurnEffectRepository
    + CompanionStateRepository
    + GlobalSettingsStore
    + ConversationReader
    + ModelProfileRepository
    + ProviderAccountRepository
    + PromptRepository
    + JobStore
{
}

impl<T> CompanionMemoryHostSources for T where
    T: lettuce_memory::DynamicMemoryRunRepository
        + lettuce_memory::MemoryRepository
        + lettuce_memory::MemorySummaryRepository
        + lettuce_memory::DynamicMemoryApprovalRepository
        + lettuce_embeddings::MemoryEmbeddingRepository
        + lettuce_conversations::ProviderReplayArtifactPort
        + lettuce_usage::JobUsageLedger
        + lettuce_companions::CompanionTurnEffectRepository
        + CompanionStateRepository
        + GlobalSettingsStore
        + ConversationReader
        + ModelProfileRepository
        + ProviderAccountRepository
        + PromptRepository
        + JobStore
{
}

/// The host entry points legacy's `enqueue_post_turn_dynamic_memory` and its
/// scheduler loop provided: admit the cycle a finished turn earns, resolve the
/// live settings into runner inputs, run the claimed work and settle its job.
#[derive(Debug)]
pub struct CompanionMemoryHostCoordinator<'a, R: ?Sized, E: ?Sized, I: ?Sized> {
    repository: &'a R,
    engine: &'a E,
    inference: &'a I,
}

impl<'a, R: ?Sized, E: ?Sized, I: ?Sized> CompanionMemoryHostCoordinator<'a, R, E, I> {
    #[must_use]
    pub const fn new(repository: &'a R, engine: &'a E, inference: &'a I) -> Self {
        Self {
            repository,
            engine,
            inference,
        }
    }
}

impl<R, E, I> CompanionMemoryHostCoordinator<'_, R, E, I>
where
    R: CompanionMemoryHostSources + ?Sized,
    E: MemoryEmbeddingEngine + ?Sized,
    I: InferencePort + ?Sized,
{
    /// Legacy enqueued the post-turn cycle after a send or continue (never a
    /// regenerate) when dynamic memory was active: global `enabled` and a
    /// dynamic session for direct chats, only a dynamic session for groups.
    /// Companion conversations discover their processing turn effects; plain
    /// conversations admit the next interval-sized window.
    pub fn after_turn(
        &self,
        conversation_id: ConversationId,
        operation: GenerationOperation,
        worker_id: WorkerId,
        now: TimestampMillis,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Vec<CompanionMemoryClaimedWork>, CompanionMemoryHostError> {
        if operation == GenerationOperation::Regenerate {
            return Ok(Vec::new());
        }
        let Some(active) = self.active_cycle(conversation_id)? else {
            return Ok(Vec::new());
        };
        let run_mode = run_mode(active.settings.run_mode);
        let dispatcher = CompanionMemoryDispatchCoordinator::new(self.repository, self.repository);
        if active.companion {
            Ok(dispatcher.admit_companion_after_turn_and_claim(
                conversation_id,
                MAX_COMPANION_POST_TURN_EFFECTS,
                active.settings.summary_message_interval,
                run_mode,
                worker_id,
                now,
                lease_for,
                allowed,
            )?)
        } else {
            Ok(dispatcher.admit_plain_after_turn_and_claim(
                conversation_id,
                active.settings.summary_message_interval,
                run_mode,
                worker_id,
                now,
                lease_for,
                allowed,
            )?)
        }
    }

    /// Legacy `trigger_dynamic_memory` (and `retry_dynamic_memory` with a
    /// model override): a forced cycle over the most recent window, which also
    /// answers an `ask_first` approval. The same gate as `after_turn` applies,
    /// as legacy's cycle checked it before running.
    #[allow(clippy::too_many_arguments)]
    pub fn trigger(
        &self,
        conversation_id: ConversationId,
        model_profile_id: Option<lettuce_types::ModelProfileId>,
        update_default_on_success: bool,
        worker_id: WorkerId,
        now: TimestampMillis,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Vec<CompanionMemoryClaimedWork>, CompanionMemoryHostError> {
        let Some(active) = self.active_cycle(conversation_id)? else {
            return Ok(Vec::new());
        };
        let dispatcher = CompanionMemoryDispatchCoordinator::new(self.repository, self.repository);
        let interval = active.settings.summary_message_interval;
        if active.companion {
            match model_profile_id {
                Some(model_profile_id) => Ok(dispatcher.retry_direct_with_model_and_claim(
                    conversation_id,
                    MAX_COMPANION_POST_TURN_EFFECTS,
                    interval,
                    model_profile_id,
                    update_default_on_success,
                    worker_id,
                    now,
                    lease_for,
                    allowed,
                )?),
                None => Ok(dispatcher.trigger_and_claim(
                    conversation_id,
                    MAX_COMPANION_POST_TURN_EFFECTS,
                    interval,
                    crate::CompanionMemoryWindowSelection::Recent,
                    worker_id,
                    now,
                    lease_for,
                    allowed,
                )?),
            }
        } else {
            Ok(dispatcher.trigger_plain_and_claim(
                conversation_id,
                interval,
                model_profile_id,
                update_default_on_success,
                worker_id,
                now,
                lease_for,
                allowed,
            )?)
        }
    }

    /// Legacy `skip_dynamic_memory_cycle`: the pending `ask_first` approval is
    /// marked skipped.
    pub fn skip(
        &self,
        conversation_id: ConversationId,
        now: TimestampMillis,
    ) -> Result<Option<lettuce_memory::DynamicMemoryPendingApproval>, CompanionMemoryHostError>
    {
        Ok(
            CompanionMemoryDispatchCoordinator::new(self.repository, self.repository)
                .skip_pending_approval(conversation_id, now)?,
        )
    }

    /// Legacy `dynamic_memory_pending_approval`.
    pub fn pending_approval_count(
        &self,
        conversation_id: ConversationId,
    ) -> Result<Option<u64>, CompanionMemoryHostError> {
        Ok(
            CompanionMemoryDispatchCoordinator::new(self.repository, self.repository)
                .pending_approval_count(conversation_id)?,
        )
    }

    fn active_cycle(
        &self,
        conversation_id: ConversationId,
    ) -> Result<Option<ActiveMemoryCycle>, CompanionMemoryHostError> {
        let aggregate = ConversationReader::get(self.repository, conversation_id)
            .map_err(CompanionMemoryHostError::Conversation)?;
        let settings = GlobalSettingsStore::load(self.repository)
            .map_err(CompanionMemoryHostError::Settings)?
            .settings;
        let group = matches!(aggregate.conversation.kind, ConversationKind::Group(_));
        let dynamic = if group {
            settings.effective_group_dynamic_memory()
        } else {
            &settings.dynamic_memory
        };
        let dynamic_session = effective_memory(&aggregate.conversation)
            .is_some_and(|memory| memory.mode == MemoryModeSnapshot::Dynamic);
        if !dynamic_session || (!group && !dynamic.enabled) {
            return Ok(None);
        }
        Ok(Some(ActiveMemoryCycle {
            settings: dynamic.clone(),
            companion: self.is_companion(&aggregate.conversation)?,
        }))
    }

    /// Legacy resolved the summarisation model as override, then
    /// `summarisationModelId`, then the app default model; the manager prompt
    /// is the local variant for llama.cpp models; the policy comes from the
    /// direct or group dynamic-memory settings.
    pub fn resolve_runtime_inputs(
        &self,
        admission: &CompanionPostTurnMemoryAdmission,
    ) -> Result<CompanionMemoryRuntimeInputs, CompanionMemoryHostError> {
        let aggregate = ConversationReader::get(self.repository, admission.batch.conversation_id)
            .map_err(CompanionMemoryHostError::Conversation)?;
        let stored = GlobalSettingsStore::load(self.repository)
            .map_err(CompanionMemoryHostError::Settings)?;
        let group = matches!(aggregate.conversation.kind, ConversationKind::Group(_));
        let dynamic = if group {
            stored.settings.effective_group_dynamic_memory()
        } else {
            &stored.settings.dynamic_memory
        };
        let model_id = admission
            .batch
            .selected_model_profile_id
            .or(stored.dynamic_memory_model_profile_id)
            .or(stored.default_model_profile_id)
            .ok_or(CompanionMemoryHostError::RuntimeInputs(
                CompanionMemoryRuntimeInputError::MissingModel,
            ))?;
        let model = ModelProfileRepository::get(self.repository, model_id)
            .map_err(|_| storage())?
            .ok_or(CompanionMemoryHostError::RuntimeInputs(
                CompanionMemoryRuntimeInputError::MissingModel,
            ))?;
        let account = ProviderAccountRepository::get(self.repository, model.provider_account_id)
            .map_err(|_| storage())?
            .ok_or(CompanionMemoryHostError::RuntimeInputs(
                CompanionMemoryRuntimeInputError::MissingModel,
            ))?;
        let chat_profile = lettuce_models::resolve_chat_profile(
            &ExpectedModelIdentity {
                model_profile_id: model.id,
                model_revision: model.revision,
                provider_account_id: account.id,
                provider_account_revision: account.revision,
                external_model_id: model.external_model_id.clone(),
                display_name: model.display_name.clone(),
                provider_protocol: account.protocol,
                model_kind: model.kind,
            },
            &model,
            &account,
            &ChatParameterResolutionInput::default(),
            &ChatRequirements::default(),
        )
        .map_err(|_| {
            CompanionMemoryHostError::RuntimeInputs(CompanionMemoryRuntimeInputError::InvalidModel)
        })?;
        let manager_prompt = if account.protocol == ProviderProtocol::LlamaCpp {
            BuiltInPromptId::DynamicMemoryLocal
        } else {
            BuiltInPromptId::DynamicMemory
        };
        let summary_prompt = self.built_in_prompt(BuiltInPromptId::DynamicSummary)?;
        let memory_prompt = self.built_in_prompt(manager_prompt)?;
        let score = |basis_points| {
            lettuce_memory::Score::from_basis_points(basis_points).ok_or(
                CompanionMemoryHostError::RuntimeInputs(
                    CompanionMemoryRuntimeInputError::InvalidSettings,
                ),
            )
        };
        let policy = MemoryPolicy {
            max_entries: usize::try_from(dynamic.max_entries.max(1)).map_err(|_| {
                CompanionMemoryHostError::RuntimeInputs(
                    CompanionMemoryRuntimeInputError::InvalidSettings,
                )
            })?,
            hot_token_budget: dynamic.hot_memory_token_budget,
            cold_threshold: score(dynamic.cold_threshold_basis_points)?,
            delete_confidence_default: score(dynamic.delete_confidence_basis_points)?,
            max_hard_delete_ratio_per_cycle: score(dynamic.max_hard_delete_ratio_basis_points)?,
            decay_rate: score(dynamic.decay_rate_basis_points)?,
        };
        policy.validate().map_err(|_| {
            CompanionMemoryHostError::RuntimeInputs(
                CompanionMemoryRuntimeInputError::InvalidSettings,
            )
        })?;
        let companion = self.is_companion(&aggregate.conversation)?;
        Ok(CompanionMemoryRuntimeInputs {
            profile: ResolvedInferenceProfile {
                chat_profile,
                tool_policy: ToolPolicy::Required,
                output_policy: OutputPolicy::Plain,
                safety_policy: SafetyContext::Standard,
                correlation_id: None,
            },
            time_awareness_enabled: false,
            supersession_enabled: companion,
            structured_fallback_format: match dynamic.structured_fallback_format {
                MemoryStructuredFallbackFormat::Json => DynamicMemoryStructuredFallbackFormat::Json,
                MemoryStructuredFallbackFormat::Xml => DynamicMemoryStructuredFallbackFormat::Xml,
            },
            summary_prompt,
            memory_prompt,
            policy,
            duplicate_threshold: score(dynamic.duplicate_threshold_basis_points)?,
        })
    }

    /// Runs one claimed cycle with live inputs and settles its job. Missing
    /// runtime inputs reschedule the job without starting a run, as legacy
    /// skipped the cycle when no summarisation model was configured and tried
    /// again on the next turn.
    pub async fn run_claimed(
        &self,
        work: CompanionMemoryClaimedWork,
        cancellation_reason: CancellationReason,
        now: TimestampMillis,
    ) -> Result<CompanionMemorySettledWork, CompanionMemoryHostError> {
        let dispatcher = CompanionMemoryDispatchCoordinator::new(self.repository, self.repository);
        let inputs = match self.resolve_runtime_inputs(&work.admission) {
            Ok(inputs) => inputs,
            Err(CompanionMemoryHostError::RuntimeInputs(error)) => {
                return Ok(dispatcher.settle_run(
                    work,
                    Err(CompanionMemoryJobRunError::RuntimeInputs(error)),
                    cancellation_reason,
                    now,
                )?);
            }
            Err(error) => return Err(error),
        };
        let engine = self.engine;
        let result = CompanionMemoryJobRunner::new(
            self.engine,
            self.repository,
            self.repository,
            self.inference,
        )
        .run(
            &work.admission,
            inputs.profile,
            inputs.time_awareness_enabled,
            inputs.supersession_enabled,
            inputs.structured_fallback_format,
            &inputs.summary_prompt,
            &inputs.memory_prompt,
            &inputs.policy,
            inputs.duplicate_threshold,
            &work.claim,
            &work.handle,
            None,
            now,
            |round| create_seeds(engine, round, now),
        )
        .await;
        Ok(dispatcher.settle_run(work, result, cancellation_reason, now)?)
    }

    fn is_companion(
        &self,
        conversation: &lettuce_conversations::Conversation,
    ) -> Result<bool, CompanionMemoryHostError> {
        let ConversationKind::Direct(details) = &conversation.kind else {
            return Ok(false);
        };
        let owner = CompanionStateOwner {
            conversation_id: conversation.id,
            character_id: details.character.source_id,
            persona_id: match &details.persona {
                SnapshotSelection::Inherited(persona) | SnapshotSelection::Explicit(persona) => {
                    Some(persona.source_id)
                }
                SnapshotSelection::Disabled => None,
            },
        };
        CompanionStateRepository::get(self.repository, owner)
            .map(|state| state.is_some())
            .map_err(CompanionMemoryHostError::Companion)
    }

    fn built_in_prompt(
        &self,
        id: BuiltInPromptId,
    ) -> Result<PromptDocument, CompanionMemoryHostError> {
        crate::built_in_prompts::active_built_in_prompt(self.repository, id)
            .map_err(|_| storage())?
            .ok_or(CompanionMemoryHostError::RuntimeInputs(
                CompanionMemoryRuntimeInputError::MissingPrompt,
            ))
    }
}

struct ActiveMemoryCycle {
    settings: lettuce_settings::DynamicMemorySettings,
    companion: bool,
}

const fn run_mode(mode: MemoryRunMode) -> DynamicMemoryRunMode {
    match mode {
        MemoryRunMode::Auto => DynamicMemoryRunMode::Auto,
        MemoryRunMode::AskFirst => DynamicMemoryRunMode::AskFirst,
        MemoryRunMode::Manual => DynamicMemoryRunMode::Manual,
    }
}

const fn storage() -> CompanionMemoryHostError {
    CompanionMemoryHostError::RuntimeInputs(CompanionMemoryRuntimeInputError::Storage)
}

/// Legacy counted the validated memory text's tokens with the embedding
/// tokenizer and stored zero when counting failed.
fn create_seeds<E: MemoryEmbeddingEngine + ?Sized>(
    engine: &E,
    round: &lettuce_memory::DynamicMemoryInferenceRound,
    now: TimestampMillis,
) -> Vec<MemoryCreateSeed> {
    round
        .calls
        .iter()
        .filter(|call| call.call.name == "create_memory")
        .map(|call| {
            let token_count = call
                .call
                .arguments
                .get("text")
                .and_then(serde_json::Value::as_str)
                .map(|text| {
                    let counted = lettuce_memory::normalize_memory_text(text)
                        .unwrap_or_else(|_| text.to_owned());
                    engine.count_tokens(&counted).unwrap_or(0)
                })
                .unwrap_or(0);
            MemoryCreateSeed {
                execution_id: call.id,
                id: MemoryId::new(),
                token_count,
                created_at: now,
            }
        })
        .collect()
}
