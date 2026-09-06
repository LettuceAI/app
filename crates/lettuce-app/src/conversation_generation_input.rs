use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use lettuce_characters::{CharacterRepository, PersonaRepository};
use lettuce_companions::{
    CompanionScheduledNoteRepository, CompanionStateRepository, SoulRepository,
};
use lettuce_conversations::{
    ContextAssembler, ContextAssemblyError, ContextRequest, ConversationKind, ConversationReader,
    ConversationRepository, ConversationRepositoryError, ConversationSnapshotMaterializer,
    DynamicMemoryPolicySnapshot, GenerationCheckpointEnvelope, GenerationCheckpointEvent,
    GenerationInput, GenerationTarget, GenerationTurnStatus, InferencePort, InferenceRequest,
    MemoryAttribution, MemoryContribution, MemoryModeSnapshot, MemoryRetrievalStrategySnapshot,
    MessageRole, OutputPolicy, PromptRuntimeFacts, PromptRuntimeValues, ProviderContextPart,
    ProviderNeutralContext, ProviderNeutralMessage, ResolveGroupSpeaker, ResolvedInferenceProfile,
    SafetyContext, SelectedSpeakerDecision, SpeakerDecisionMethod, SpeakerDecisionReference,
    SpeakerFallback, SpeakerInferenceBinding, SpeakerInferenceRepository, SpeakerParticipantState,
    SpeakerPolicyRequest, ToolChoice, ToolDefinition, ToolPolicy, ToolRequest,
    select_group_speaker,
};
use lettuce_embeddings::{EmbeddingDimensions, EmbeddingRequest, MemoryEmbeddingRepository};
use lettuce_jobs::{
    CancellationReason, Clock, ResourceAvailability, WorkerId, handle::CancellationToken,
};
use lettuce_memory::{
    DynamicMemoryPreparationRepository, DynamicMemoryRoundRepository, MemoryPolicy,
    MemoryRepository, MemoryRepositoryError, MemoryRetrievalAccess, MemoryRetrievalRepository,
    MemorySpaceSnapshot, MemorySummaryRepository, Score, dynamic_memory_tool_request,
    memory_revision_id,
};
use lettuce_models::{
    CapabilityStatus, ChatParameterResolutionInput, ChatProfileResolutionError, ChatRequirements,
    ModelProfileRepository, ModelRepositoryError, ProviderAccountRepository,
};
use lettuce_types::{PageLimit, PageRequest, RequestId, TimestampMillis, UsageEventId};
use lettuce_usage::JobUsageLedger;

use crate::{
    ConversationContextAssembler, ConversationGenerationClaimContext,
    ConversationGenerationClaimedWork, ConversationGenerationDispatchCoordinator,
    ConversationGenerationDispatchError, ConversationGenerationInput,
    ConversationGenerationJobRunner, ConversationGenerationMemoryInput,
    ConversationGenerationOperation, ConversationGenerationRunError,
    ConversationGenerationRunResult, ConversationGenerationSettledWork, EmbeddingGenerationError,
    MemoryCreateSeed, MemoryEmbeddingEngine, operation_token,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConversationGenerationRuntimeInput {
    pub stream_sink: Option<RequestId>,
    pub prompt_values: PromptRuntimeValues,
}

#[derive(Debug, Clone)]
pub struct ConversationGenerationExecutionRequest {
    pub conversation_id: lettuce_types::ConversationId,
    pub turn_id: lettuce_types::GenerationTurnId,
    pub attempt_id: lettuce_types::GenerationAttemptId,
    pub worker_id: WorkerId,
    pub lease_for: Duration,
    pub resources: ResourceAvailability,
    pub runtime: ConversationGenerationRuntimeInput,
    pub cancellation: CancellationToken,
    pub cancellation_reason: CancellationReason,
}

#[derive(Debug)]
pub enum ConversationGenerationExecutionOutcome {
    Settled(ConversationGenerationSettledWork),
    Replayed {
        result: Box<ConversationGenerationRunResult>,
        job: lettuce_jobs::JobSnapshot,
    },
    Terminal(crate::ConversationGenerationAdmission),
    NotClaimed(crate::ConversationGenerationAdmission),
}

#[derive(Debug, thiserror::Error)]
pub enum ConversationGenerationExecutionError {
    #[error("conversation generation dispatch failed: {0}")]
    Dispatch(#[from] ConversationGenerationDispatchError),
    #[error("conversation generation replay failed: {0}")]
    Replay(#[from] ConversationGenerationRunError),
}

#[derive(Debug)]
enum ConversationGenerationInputError {
    Repository(ConversationRepositoryError),
    ModelRepository(ModelRepositoryError),
    MissingModel,
    Profile(ChatProfileResolutionError),
    Context(ContextAssemblyError),
    Memory(MemoryRepositoryError),
    Embedding,
    Cancelled,
    MemoryInputUnavailable,
    SpeakerUnavailable,
    SpeakerPending(UsageEventId),
    InvalidTurn,
}

#[derive(Debug)]
pub struct PreparedConversationGenerationJobRunner<'a, E: ?Sized, R: ?Sized, I: ?Sized> {
    embedding: &'a E,
    repository: &'a R,
    inference: &'a I,
}

impl<'a, E: ?Sized, R: ?Sized, I: ?Sized> PreparedConversationGenerationJobRunner<'a, E, R, I> {
    pub fn new(embedding: &'a E, repository: &'a R, inference: &'a I) -> Self {
        Self {
            embedding,
            repository,
            inference,
        }
    }
}

impl<E, R, I> PreparedConversationGenerationJobRunner<'_, E, R, I>
where
    E: MemoryEmbeddingEngine + ?Sized,
    R: ConversationRepository
        + ConversationSnapshotMaterializer
        + CharacterRepository
        + PersonaRepository
        + SoulRepository
        + CompanionStateRepository
        + CompanionScheduledNoteRepository
        + ModelProfileRepository
        + ProviderAccountRepository
        + lettuce_conversations::InitialInferenceRepository
        + SpeakerInferenceRepository
        + lettuce_conversations::ToolExecutionRepository
        + lettuce_conversations::ProviderReplayArtifactPort
        + JobUsageLedger
        + lettuce_conversations::UsagePort
        + DynamicMemoryRoundRepository
        + DynamicMemoryPreparationRepository
        + MemoryEmbeddingRepository
        + MemoryRepository
        + MemoryRetrievalRepository
        + MemorySummaryRepository,
    I: InferencePort + ?Sized,
{
    pub async fn execute<F, C>(
        &self,
        request: ConversationGenerationExecutionRequest,
        clock: &C,
        seeds_for_round: F,
    ) -> Result<ConversationGenerationExecutionOutcome, ConversationGenerationExecutionError>
    where
        F: FnMut(&[lettuce_conversations::ToolExecution]) -> Vec<MemoryCreateSeed>,
        C: Clock + ?Sized,
        R: lettuce_jobs::JobStore + lettuce_usage::UsageLedger,
    {
        let dispatcher =
            ConversationGenerationDispatchCoordinator::new(self.repository, self.repository);
        let admission = dispatcher.admit(
            request.conversation_id,
            request.turn_id,
            request.attempt_id,
            clock.now(),
        )?;
        if admission.job.state == lettuce_jobs::JobState::Succeeded {
            let result = ConversationGenerationJobRunner::new(
                self.embedding,
                self.repository,
                self.inference,
            )
            .replay_succeeded_attempt(
                request.conversation_id,
                request.turn_id,
                request.attempt_id,
                admission.job.id,
            )?;
            return Ok(ConversationGenerationExecutionOutcome::Replayed {
                result: Box::new(result),
                job: admission.job,
            });
        }
        if admission.job.state.is_terminal() {
            return Ok(ConversationGenerationExecutionOutcome::Terminal(admission));
        }
        let Some(work) = dispatcher.claim_with_cancellation(
            request.turn_id,
            request.attempt_id,
            ConversationGenerationClaimContext {
                worker_id: request.worker_id,
                cancellation: request.cancellation,
            },
            clock.now(),
            request.lease_for,
            &request.resources,
        )?
        else {
            return Ok(ConversationGenerationExecutionOutcome::NotClaimed(
                admission,
            ));
        };
        let result = self
            .run(&work, request.runtime, clock.now(), seeds_for_round)
            .await;
        let settled = dispatcher.settle(work, result, request.cancellation_reason, clock.now())?;
        Ok(ConversationGenerationExecutionOutcome::Settled(settled))
    }

    pub async fn run<F>(
        &self,
        work: &ConversationGenerationClaimedWork,
        runtime: ConversationGenerationRuntimeInput,
        now: TimestampMillis,
        seeds_for_round: F,
    ) -> Result<ConversationGenerationRunResult, ConversationGenerationRunError>
    where
        F: FnMut(&[lettuce_conversations::ToolExecution]) -> Vec<MemoryCreateSeed>,
    {
        let runner =
            ConversationGenerationJobRunner::new(self.embedding, self.repository, self.inference);
        if let Some(replay) = runner.replay_terminal(work)? {
            return Ok(replay);
        }
        self.resolve_automatic_speaker(work, now)
            .await
            .map_err(ConversationGenerationInputError::into_run_error)?;
        let input = self
            .build_input(work, runtime, now)
            .await
            .map_err(ConversationGenerationInputError::into_run_error)?;
        runner.run(work, input, now, seeds_for_round).await
    }

    async fn resolve_automatic_speaker(
        &self,
        work: &ConversationGenerationClaimedWork,
        now: TimestampMillis,
    ) -> Result<(), ConversationGenerationInputError> {
        let aggregate = ConversationReader::get(self.repository, work.conversation_id)
            .map_err(ConversationGenerationInputError::Repository)?;
        let ConversationKind::Group(details) = &aggregate.conversation.kind else {
            return Ok(());
        };
        let mut turn = ConversationReader::get_turn(self.repository, work.turn_id)
            .map_err(ConversationGenerationInputError::Repository)?;
        if turn.selected_speaker.is_some()
            || turn.forced_speaker.is_some()
            || matches!(turn.target, GenerationTarget::ExistingCandidate { .. })
        {
            return Ok(());
        }
        if matches!(
            details.group.speaker_selection,
            lettuce_conversations::GroupSpeakerSelectionSnapshot::Director
                | lettuce_conversations::GroupSpeakerSelectionSnapshot::DirectorAction
        ) {
            return Ok(());
        }
        let stages = match turn.status {
            GenerationTurnStatus::Created => vec![
                (
                    GenerationTurnStatus::Preparing,
                    ConversationGenerationOperation::StagePreparing,
                ),
                (
                    GenerationTurnStatus::SelectingSpeaker,
                    ConversationGenerationOperation::StageSelectingSpeaker,
                ),
            ],
            GenerationTurnStatus::Preparing => vec![(
                GenerationTurnStatus::SelectingSpeaker,
                ConversationGenerationOperation::StageSelectingSpeaker,
            )],
            GenerationTurnStatus::SelectingSpeaker => Vec::new(),
            _ => return Err(ConversationGenerationInputError::InvalidTurn),
        };
        for (status, operation) in stages {
            let sequence = self
                .repository
                .latest_checkpoint_sequence(work.turn_id, work.attempt_id)
                .map_err(ConversationGenerationInputError::Repository)?
                .unwrap_or(0)
                .checked_add(1)
                .ok_or(ConversationGenerationInputError::InvalidTurn)?;
            turn = self
                .repository
                .append_event(
                    work.turn_id,
                    turn.revision,
                    &operation_token(
                        work.conversation_id,
                        work.turn_id,
                        work.attempt_id,
                        work.handle.id(),
                        operation,
                    ),
                    GenerationCheckpointEnvelope {
                        turn_id: work.turn_id,
                        attempt_id: work.attempt_id,
                        job_id: Some(work.handle.id()),
                        correlation_id: None,
                        sequence,
                        event: GenerationCheckpointEvent::Stage { status },
                    },
                    now,
                )
                .map_err(ConversationGenerationInputError::Repository)?
                .value;
        }
        let source_message_id = match turn.input {
            GenerationInput::UserMessage { message_id } => message_id,
            GenerationInput::ExistingHead { head_message_id } => head_message_id,
            GenerationInput::ExistingCandidate { message_id, .. } => message_id,
        };
        let mut timeline = self.timeline(work.conversation_id, turn.branch_id)?;
        retain_source_ancestry(&mut timeline.items, source_message_id)?;
        let prior_speaker = timeline.items.iter().rev().find_map(|item| {
            (item.message.role == MessageRole::Assistant)
                .then_some(item.message.author_participant_id)
                .flatten()
        });
        let participants = aggregate
            .conversation
            .participants
            .iter()
            .filter(|participant| {
                participant.role == lettuce_conversations::ParticipantRole::Character
            })
            .map(|participant| SpeakerParticipantState {
                id: participant.id,
                eligible: participant.enabled,
                muted: participant.muted,
                speak_count: u32::try_from(
                    timeline
                        .items
                        .iter()
                        .filter(|item| {
                            item.message.role == MessageRole::Assistant
                                && item.message.author_participant_id == Some(participant.id)
                        })
                        .count(),
                )
                .unwrap_or(u32::MAX),
                last_spoke_turn: None,
                last_spoke_at: None,
            })
            .collect();
        let policy_request = SpeakerPolicyRequest {
            conversation_id: work.conversation_id,
            branch_id: turn.branch_id,
            operation: turn.operation,
            forced_speaker: None,
            mention_source: None,
            participants,
            prior_speaker,
            timeline: timeline.items,
        };
        let selected_speaker = if details.group.speaker_selection
            == lettuce_conversations::GroupSpeakerSelectionSnapshot::Llm
        {
            self.select_speaker_via_llm(
                work,
                &aggregate.conversation,
                &turn,
                &policy_request,
                details.group.speaker_selection_model.as_ref(),
                now,
            )
            .await?
        } else {
            select_group_speaker(&policy_request, details.group.speaker_selection)
                .map_err(|_| ConversationGenerationInputError::SpeakerUnavailable)?
        };
        self.repository
            .resolve_group_speaker(
                &ResolveGroupSpeaker {
                    conversation_id: work.conversation_id,
                    turn_id: work.turn_id,
                    expected_turn_revision: turn.revision,
                    operation: operation_token(
                        work.conversation_id,
                        work.turn_id,
                        work.attempt_id,
                        work.handle.id(),
                        ConversationGenerationOperation::ResolveSpeaker,
                    ),
                    selected_speaker,
                },
                now,
            )
            .map_err(ConversationGenerationInputError::Repository)?;
        Ok(())
    }

    async fn select_speaker_via_llm(
        &self,
        work: &ConversationGenerationClaimedWork,
        conversation: &lettuce_conversations::Conversation,
        turn: &lettuce_conversations::GenerationTurn,
        policy: &SpeakerPolicyRequest,
        selection_model: Option<&lettuce_conversations::ModelSelectionSnapshot>,
        now: TimestampMillis,
    ) -> Result<SelectedSpeakerDecision, ConversationGenerationInputError> {
        let available = conversation
            .participants
            .iter()
            .filter(|participant| {
                participant.role == lettuce_conversations::ParticipantRole::Character
                    && participant.enabled
                    && !participant.muted
            })
            .collect::<Vec<_>>();
        if available.is_empty() {
            return Err(ConversationGenerationInputError::SpeakerUnavailable);
        }
        let Some(selection_model) = selection_model else {
            return heuristic_fallback(policy, None, None);
        };
        let Some(model) = ModelProfileRepository::get(self.repository, selection_model.source_id)
            .map_err(ConversationGenerationInputError::ModelRepository)?
        else {
            return heuristic_fallback(policy, None, None);
        };
        let Some(account) =
            ProviderAccountRepository::get(self.repository, model.provider_account_id)
                .map_err(ConversationGenerationInputError::ModelRepository)?
        else {
            return heuristic_fallback(policy, None, None);
        };
        let profile = match lettuce_models::resolve_chat_profile(
            &selection_model.expected_chat_identity(),
            &model,
            &account,
            &ChatParameterResolutionInput::default(),
            &ChatRequirements {
                require_tools: true,
                ..Default::default()
            },
        ) {
            Ok(profile) => profile,
            Err(_) => return heuristic_fallback(policy, None, None),
        };
        let prompt = speaker_selection_prompt(conversation, policy, &available);
        let tools = speaker_selection_tools(&available);
        let request = InferenceRequest {
            turn_id: turn.id,
            attempt_id: work.attempt_id,
            operation: turn.operation,
            profile: ResolvedInferenceProfile {
                chat_profile: profile,
                tool_policy: ToolPolicy::Allowed,
                output_policy: OutputPolicy::Plain,
                safety_policy: SafetyContext::Standard,
                correlation_id: None,
            },
            context: ProviderNeutralContext {
                messages: vec![ProviderNeutralMessage {
                    role: MessageRole::User,
                    parts: vec![ProviderContextPart::Text { text: prompt }],
                }],
                attributions: Default::default(),
                budget: Default::default(),
            },
            cancellation: Some(work.handle.id()),
            stream_sink: None,
            media_grants: Vec::new(),
            tools: Some(tools),
        };
        let binding = SpeakerInferenceBinding::from_request(work.conversation_id, &request)
            .map_err(|_| ConversationGenerationInputError::InvalidTurn)?;
        if let Some(record) = self
            .repository
            .speaker_inference(&binding)
            .map_err(ConversationGenerationInputError::Repository)?
        {
            return record
                .decision
                .ok_or(ConversationGenerationInputError::SpeakerPending(
                    record.usage_event_id,
                ));
        }
        let admission = self
            .repository
            .admit_speaker_inference(work.conversation_id, &request, now)
            .map_err(ConversationGenerationInputError::Repository)?;
        if !admission.created {
            return admission.record.decision.ok_or(
                ConversationGenerationInputError::SpeakerPending(admission.record.usage_event_id),
            );
        }
        let outcome = crate::job_inference_usage::run_job_inference_with_id(
            self.repository,
            self.inference,
            work.handle.id(),
            request,
            now,
            admission.record.usage_event_id,
        )
        .await;
        let mut decision = match outcome {
            Ok(ref outcome) => llm_speaker_decision(
                outcome,
                &available,
                selection_model,
                admission.record.usage_event_id,
            )
            .unwrap_or_else(|| {
                heuristic_fallback(
                    policy,
                    Some(selection_model),
                    Some(admission.record.usage_event_id),
                )
                .expect("available speaker has a heuristic fallback")
            }),
            Err(crate::job_inference_usage::JobInferenceError::Provider(
                lettuce_conversations::PortError::Cancelled,
            )) => return Err(ConversationGenerationInputError::Cancelled),
            Err(crate::job_inference_usage::JobInferenceError::Provider(_)) => heuristic_fallback(
                policy,
                Some(selection_model),
                Some(admission.record.usage_event_id),
            )?,
            Err(crate::job_inference_usage::JobInferenceError::Evidence) => {
                return Err(ConversationGenerationInputError::Repository(
                    ConversationRepositoryError::Storage,
                ));
            }
        };
        if let Ok(outcome) = outcome {
            crate::cleanup_outcome_replays(self.repository, &outcome).map_err(|_| {
                ConversationGenerationInputError::Repository(ConversationRepositoryError::Storage)
            })?;
        }
        decision.usage_event_id = Some(admission.record.usage_event_id);
        self.repository
            .settle_speaker_inference(&binding, &decision, now)
            .map_err(ConversationGenerationInputError::Repository)?;
        Ok(decision)
    }

    async fn build_input(
        &self,
        work: &ConversationGenerationClaimedWork,
        runtime: ConversationGenerationRuntimeInput,
        now: TimestampMillis,
    ) -> Result<ConversationGenerationInput, ConversationGenerationInputError> {
        let aggregate = ConversationReader::get(self.repository, work.conversation_id)
            .map_err(ConversationGenerationInputError::Repository)?;
        let turn = ConversationReader::get_turn(self.repository, work.turn_id)
            .map_err(ConversationGenerationInputError::Repository)?;
        if turn.conversation_id != work.conversation_id
            || turn.branch_id != aggregate.conversation.active_branch_id
        {
            return Err(ConversationGenerationInputError::InvalidTurn);
        }
        let selected_speaker = self.generation_speaker(&aggregate.conversation, &turn)?;
        let settings = lettuce_conversations::resolve_effective_settings(
            &aggregate.conversation,
            selected_speaker
                .as_ref()
                .map(|speaker| speaker.participant_id),
        )
        .map_err(|_| ConversationGenerationInputError::InvalidTurn)?;
        let memory_settings = settings.memory.as_ref();
        if memory_settings.is_some_and(|memory| !memory.selected_revision_ids.is_empty()) {
            return Err(ConversationGenerationInputError::MemoryInputUnavailable);
        }
        let memory_mode = memory_settings
            .map(|memory| memory.mode)
            .unwrap_or(MemoryModeSnapshot::Disabled);
        let dynamic_memory = memory_mode == MemoryModeSnapshot::Dynamic;
        let model = turn
            .resolved_model
            .clone()
            .or(turn.requested_model_override.clone())
            .or(settings.model)
            .ok_or(ConversationGenerationInputError::MissingModel)?;
        let stored_model = ModelProfileRepository::get(self.repository, model.source_id)
            .map_err(ConversationGenerationInputError::ModelRepository)?
            .ok_or(ConversationGenerationInputError::MissingModel)?;
        let account = ProviderAccountRepository::get(self.repository, model.provider_account_id)
            .map_err(ConversationGenerationInputError::ModelRepository)?
            .ok_or(ConversationGenerationInputError::MissingModel)?;
        let profile = lettuce_models::resolve_chat_profile(
            &model.expected_chat_identity(),
            &stored_model,
            &account,
            &ChatParameterResolutionInput::default(),
            &ChatRequirements {
                require_streaming: runtime.stream_sink.is_some(),
                require_tools: dynamic_memory,
                ..Default::default()
            },
        )
        .map_err(ConversationGenerationInputError::Profile)?;
        let source_message_id = match turn.input {
            GenerationInput::UserMessage { message_id } => message_id,
            GenerationInput::ExistingHead { head_message_id } => head_message_id,
            GenerationInput::ExistingCandidate { message_id, .. } => message_id,
        };
        let mut timeline = self.timeline(work.conversation_id, turn.branch_id)?;
        retain_source_ancestry(&mut timeline.items, source_message_id)?;
        let (memory_contribution, memory_input) = match memory_mode {
            MemoryModeSnapshot::Dynamic => {
                let policy = memory_settings
                    .and_then(|memory| memory.dynamic_policy.as_ref())
                    .ok_or(ConversationGenerationInputError::MemoryInputUnavailable)?;
                let prepared = self
                    .dynamic_memory_input(work, &timeline.items, policy, now)
                    .await?;
                (prepared.0, Some(prepared.1))
            }
            MemoryModeSnapshot::Manual => (self.manual_memory_input(work.conversation_id)?, None),
            MemoryModeSnapshot::Disabled => (None, None),
        };
        let prompt_runtime = PromptRuntimeFacts {
            provider_id: Some(account.provider_kind),
            provider_label: Some(account.label),
            input_scopes: modality_scopes(profile.capabilities.input_modalities),
            output_scopes: modality_scopes(profile.capabilities.output_modalities),
            dynamic_memory_enabled: dynamic_memory,
            ..Default::default()
        };
        let context = ConversationContextAssembler::new(self.repository)
            .assemble(ContextRequest {
                conversation_id: work.conversation_id,
                branch_id: turn.branch_id,
                branch_path: timeline
                    .branch_path
                    .iter()
                    .map(|branch| branch.id)
                    .collect(),
                source_message_id,
                operation: turn.operation,
                swap_roles: turn.swap_roles,
                guidance: turn.guidance.clone(),
                window: Default::default(),
                selected_speaker,
                capabilities: profile.capabilities.clone(),
                safety: SafetyContext::Standard,
                prompt_runtime,
                prompt_values: runtime.prompt_values,
                memory: memory_contribution,
                timeline: timeline.items,
            })
            .await
            .map_err(ConversationGenerationInputError::Context)?;
        let mut seen_media = HashSet::<lettuce_types::AssetId>::new();
        let media_grants = context
            .messages
            .iter()
            .flat_map(|message| &message.parts)
            .filter_map(|part| match part {
                ProviderContextPart::MediaAsset { asset_id, .. }
                    if seen_media.insert(*asset_id) =>
                {
                    Some(*asset_id)
                }
                _ => None,
            })
            .collect();
        let attributions = context.attributions.clone();
        Ok(ConversationGenerationInput {
            model,
            attributions,
            profile: ResolvedInferenceProfile {
                chat_profile: profile,
                tool_policy: if dynamic_memory {
                    ToolPolicy::Allowed
                } else {
                    ToolPolicy::Disabled
                },
                output_policy: OutputPolicy::Plain,
                safety_policy: SafetyContext::Standard,
                correlation_id: turn.correlation_id,
            },
            context,
            tools: dynamic_memory.then(dynamic_memory_tool_request),
            media_grants,
            stream_sink: runtime.stream_sink,
            memory: memory_input,
        })
    }

    fn generation_speaker(
        &self,
        conversation: &lettuce_conversations::Conversation,
        turn: &lettuce_conversations::GenerationTurn,
    ) -> Result<Option<SelectedSpeakerDecision>, ConversationGenerationInputError> {
        let ConversationKind::Group(details) = &conversation.kind else {
            return Ok(None);
        };
        if let Some(decision) = &turn.selected_speaker {
            return Ok(Some(decision.clone()));
        }
        let (participant_id, method, reference) = if let Some(participant_id) = turn.forced_speaker
        {
            let method = match details.group.speaker_selection {
                lettuce_conversations::GroupSpeakerSelectionSnapshot::Director => {
                    SpeakerDecisionMethod::Director
                }
                lettuce_conversations::GroupSpeakerSelectionSnapshot::DirectorAction => {
                    SpeakerDecisionMethod::DirectorAction
                }
                _ => SpeakerDecisionMethod::Explicit,
            };
            (participant_id, method, None)
        } else if let GenerationTarget::ExistingCandidate {
            message_id,
            prior_candidate_id,
        } = turn.target
        {
            let candidate = self
                .repository
                .get_candidate(prior_candidate_id)
                .map_err(ConversationGenerationInputError::Repository)?;
            if candidate.message_id != message_id {
                return Err(ConversationGenerationInputError::SpeakerUnavailable);
            }
            (
                candidate.author_participant_id,
                SpeakerDecisionMethod::Explicit,
                Some(SpeakerDecisionReference::Message(message_id)),
            )
        } else {
            return Err(ConversationGenerationInputError::SpeakerUnavailable);
        };
        Ok(Some(SelectedSpeakerDecision {
            participant_id,
            method,
            fallback: SpeakerFallback::None,
            reference,
            rationale_summary: None,
            decision_model: None,
            usage_event_id: None,
        }))
    }

    async fn dynamic_memory_input(
        &self,
        work: &ConversationGenerationClaimedWork,
        timeline: &[lettuce_conversations::TimelineItem],
        settings: &DynamicMemoryPolicySnapshot,
        now: TimestampMillis,
    ) -> Result<
        (
            Option<MemoryContribution>,
            ConversationGenerationMemoryInput,
        ),
        ConversationGenerationInputError,
    > {
        let (policy, duplicate_threshold) = dynamic_memory_policy(settings)?;
        let memory = MemoryRepository::get_for_conversation(self.repository, work.conversation_id)
            .map_err(ConversationGenerationInputError::Memory)?
            .ok_or(ConversationGenerationInputError::MemoryInputUnavailable)?;
        let summary = MemorySummaryRepository::get_summary(self.repository, memory.id)
            .map_err(ConversationGenerationInputError::Memory)?
            .map(|summary| summary.text);
        let prior_access = MemoryRetrievalRepository::get_retrieval_access(
            self.repository,
            work.conversation_id,
            work.turn_id,
            work.attempt_id,
        )
        .map_err(ConversationGenerationInputError::Memory)?;
        let (selected, revision) = if let Some(receipt) = prior_access {
            if receipt.access.space_id != memory.id || receipt.resulting_revision != memory.revision
            {
                return Err(ConversationGenerationInputError::MemoryInputUnavailable);
            }
            let selected = receipt
                .access
                .selected_memory_ids
                .iter()
                .map(|id| {
                    memory
                        .items
                        .iter()
                        .find(|item| item.id == *id && item.superseded_by.is_none())
                        .cloned()
                        .ok_or(ConversationGenerationInputError::MemoryInputUnavailable)
                })
                .collect::<Result<Vec<_>, _>>()?;
            (selected, receipt.resulting_revision)
        } else {
            let selected = self
                .retrieve_memories(work, timeline, &memory, settings)
                .await?;
            let revision = if selected.is_empty() {
                memory.revision
            } else {
                MemoryRetrievalRepository::apply_retrieval_access(
                    self.repository,
                    MemoryRetrievalAccess {
                        conversation_id: work.conversation_id,
                        turn_id: work.turn_id,
                        attempt_id: work.attempt_id,
                        space_id: memory.id,
                        expected_revision: memory.revision,
                        selected_memory_ids: selected.iter().map(|item| item.id).collect(),
                        accessed_at: now,
                    },
                )
                .map_err(ConversationGenerationInputError::Memory)?
                .resulting_revision
            };
            (selected, revision)
        };
        let key_memories = selected
            .into_iter()
            .map(|item| format!("- {}", item.text.trim()))
            .collect::<Vec<_>>();
        let contribution =
            (summary.is_some() || !key_memories.is_empty()).then(|| MemoryContribution {
                attribution: MemoryAttribution {
                    revision_id: memory_revision_id(memory.id, revision),
                },
                summary,
                key_memories,
            });
        Ok((
            contribution,
            ConversationGenerationMemoryInput {
                space_id: memory.id,
                policy,
                duplicate_threshold,
            },
        ))
    }

    fn manual_memory_input(
        &self,
        conversation_id: lettuce_types::ConversationId,
    ) -> Result<Option<MemoryContribution>, ConversationGenerationInputError> {
        let memory = MemoryRepository::get_for_conversation(self.repository, conversation_id)
            .map_err(ConversationGenerationInputError::Memory)?
            .ok_or(ConversationGenerationInputError::MemoryInputUnavailable)?;
        let key_memories = memory
            .items
            .iter()
            .filter(|item| item.superseded_by.is_none())
            .map(|item| format!("- {}", item.text.trim()))
            .collect::<Vec<_>>();
        Ok((!key_memories.is_empty()).then(|| MemoryContribution {
            attribution: MemoryAttribution {
                revision_id: memory_revision_id(memory.id, memory.revision),
            },
            summary: None,
            key_memories,
        }))
    }

    async fn retrieve_memories(
        &self,
        work: &ConversationGenerationClaimedWork,
        timeline: &[lettuce_conversations::TimelineItem],
        memory: &MemorySpaceSnapshot,
        settings: &DynamicMemoryPolicySnapshot,
    ) -> Result<Vec<lettuce_memory::MemoryItem>, ConversationGenerationInputError> {
        let active = memory
            .items
            .iter()
            .filter(|item| item.superseded_by.is_none())
            .map(|item| (item.id, item))
            .collect::<HashMap<_, _>>();
        if active.is_empty() {
            return Ok(Vec::new());
        }
        let query = memory_query(timeline, settings.context_enrichment_enabled);
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let query_embedding = match self.embedding.embed_memory(
            &EmbeddingRequest {
                text: query.clone(),
                dimensions: EmbeddingDimensions::D128,
            },
            &work.handle.cancellation_token(),
        ) {
            Ok(vector) => vector,
            Err(EmbeddingGenerationError::Cancelled) => {
                return Err(ConversationGenerationInputError::Cancelled);
            }
            Err(EmbeddingGenerationError::Unavailable) => {
                tracing::warn!("dynamic-memory retrieval embedding is unavailable");
                return Ok(Vec::new());
            }
        };
        let projections = self
            .repository
            .list_ready(
                memory.id,
                self.embedding.source_revision(),
                EmbeddingDimensions::D128,
            )
            .map_err(|_| ConversationGenerationInputError::Embedding)?;
        let limit = usize::from(settings.retrieval_limit);
        let threshold = f32::from(settings.min_similarity_basis_points) / 10_000.0;
        let selected = select_memories(
            &query,
            &query_embedding,
            &projections,
            &active,
            limit,
            threshold,
            settings.retrieval_strategy,
        );
        Ok(selected.into_iter().cloned().collect())
    }

    fn timeline(
        &self,
        conversation_id: lettuce_types::ConversationId,
        branch_id: lettuce_types::ConversationBranchId,
    ) -> Result<lettuce_conversations::TimelinePage, ConversationGenerationInputError> {
        let mut request = PageRequest {
            cursor: None,
            limit: PageLimit::new(200),
        };
        let mut complete = self
            .repository
            .timeline_page(conversation_id, branch_id, &request)
            .map_err(ConversationGenerationInputError::Repository)?;
        while let Some(cursor) = complete.next_cursor.take() {
            request.cursor = Some(cursor);
            let page = self
                .repository
                .timeline_page(conversation_id, branch_id, &request)
                .map_err(ConversationGenerationInputError::Repository)?;
            if page.branch_path != complete.branch_path
                || complete.items.len().saturating_add(page.items.len()) > 512
            {
                return Err(ConversationGenerationInputError::Context(
                    ContextAssemblyError::SizeLimit,
                ));
            }
            complete.items.extend(page.items);
            complete.boundary_parent_id = page.boundary_parent_id;
            complete.next_cursor = page.next_cursor;
        }
        Ok(complete)
    }
}

fn modality_scopes(capabilities: lettuce_models::ModalityCapabilities) -> Vec<String> {
    [
        ("text", capabilities.text),
        ("image", capabilities.image),
        ("audio", capabilities.audio),
    ]
    .into_iter()
    .filter(|(_, status)| *status == CapabilityStatus::Supported)
    .map(|(name, _)| name.into())
    .collect()
}

fn retain_source_ancestry(
    timeline: &mut Vec<lettuce_conversations::TimelineItem>,
    source_message_id: lettuce_types::MessageId,
) -> Result<(), ConversationGenerationInputError> {
    let parents = timeline
        .iter()
        .map(|item| (item.message.id, item.message.parent_message_id))
        .collect::<std::collections::HashMap<_, _>>();
    let mut retained = HashSet::new();
    let mut current = Some(source_message_id);
    while let Some(message_id) = current {
        if !retained.insert(message_id) {
            return Err(ConversationGenerationInputError::InvalidTurn);
        }
        current = *parents
            .get(&message_id)
            .ok_or(ConversationGenerationInputError::InvalidTurn)?;
    }
    timeline.retain(|item| retained.contains(&item.message.id));
    Ok(())
}

impl ConversationGenerationInputError {
    fn into_run_error(self) -> ConversationGenerationRunError {
        match self {
            Self::Repository(error) => ConversationGenerationRunError::Repository(error),
            Self::ModelRepository(ModelRepositoryError::Storage) => {
                ConversationGenerationRunError::Repository(ConversationRepositoryError::Storage)
            }
            Self::Context(error) => {
                tracing::warn!(?error, "conversation context preparation failed");
                ConversationGenerationRunError::PreparationFailed {
                    code: lettuce_conversations::GenerationFailureCode::ContextUnavailable,
                }
            }
            Self::Memory(error) => {
                tracing::warn!(?error, "dynamic-memory state preparation failed");
                ConversationGenerationRunError::PreparationFailed {
                    code: lettuce_conversations::GenerationFailureCode::ContextUnavailable,
                }
            }
            Self::Embedding => {
                tracing::warn!("dynamic-memory projection preparation failed");
                ConversationGenerationRunError::PreparationFailed {
                    code: lettuce_conversations::GenerationFailureCode::ContextUnavailable,
                }
            }
            Self::Cancelled => ConversationGenerationRunError::Cancelled {
                evidence: crate::GenerationUsageEvidence::None,
            },
            Self::Profile(error) => {
                tracing::warn!(?error, "conversation model profile resolution failed");
                ConversationGenerationRunError::PreparationFailed {
                    code: lettuce_conversations::GenerationFailureCode::MissingModel,
                }
            }
            Self::MissingModel | Self::ModelRepository(_) => {
                ConversationGenerationRunError::PreparationFailed {
                    code: lettuce_conversations::GenerationFailureCode::MissingModel,
                }
            }
            Self::SpeakerUnavailable => ConversationGenerationRunError::PreparationFailed {
                code: lettuce_conversations::GenerationFailureCode::SpeakerUnavailable,
            },
            Self::SpeakerPending(usage_event_id) => ConversationGenerationRunError::Pending {
                evidence: crate::GenerationUsageEvidence::Dispatch(usage_event_id),
            },
            Self::MemoryInputUnavailable | Self::InvalidTurn => {
                ConversationGenerationRunError::InvalidInput
            }
        }
    }
}

fn dynamic_memory_policy(
    settings: &DynamicMemoryPolicySnapshot,
) -> Result<(MemoryPolicy, Score), ConversationGenerationInputError> {
    let score = |value| {
        Score::from_basis_points(value)
            .ok_or(ConversationGenerationInputError::MemoryInputUnavailable)
    };
    if settings.retrieval_limit == 0 || settings.retrieval_limit > 256 {
        return Err(ConversationGenerationInputError::MemoryInputUnavailable);
    }
    let policy = MemoryPolicy {
        max_entries: usize::try_from(settings.max_entries)
            .map_err(|_| ConversationGenerationInputError::MemoryInputUnavailable)?,
        hot_token_budget: settings.hot_memory_token_budget,
        cold_threshold: score(settings.cold_threshold_basis_points)?,
        delete_confidence_default: score(settings.delete_confidence_basis_points)?,
        max_hard_delete_ratio_per_cycle: score(settings.max_hard_delete_ratio_basis_points)?,
    };
    policy
        .validate()
        .map_err(|_| ConversationGenerationInputError::MemoryInputUnavailable)?;
    Ok((policy, score(settings.duplicate_threshold_basis_points)?))
}

fn memory_query(timeline: &[lettuce_conversations::TimelineItem], enriched: bool) -> String {
    let count = if enriched { 2 } else { 1 };
    let mut messages = timeline
        .iter()
        .rev()
        .filter(|item| {
            matches!(
                item.message.role,
                MessageRole::User | MessageRole::Assistant
            )
        })
        .filter_map(|item| {
            let parts = item
                .active_revision
                .as_ref()
                .map(|revision| revision.parts.as_slice())
                .or_else(|| {
                    item.active_candidate
                        .as_ref()
                        .map(|candidate| candidate.parts.as_slice())
                })?;
            let text = parts
                .iter()
                .filter_map(|part| match part {
                    lettuce_conversations::MessagePart::Text { text } => Some(text.trim()),
                    _ => None,
                })
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            (!text.is_empty()).then_some(text)
        })
        .take(count)
        .collect::<Vec<_>>();
    messages.reverse();
    messages.join("\n")
}

fn speaker_selection_prompt(
    conversation: &lettuce_conversations::Conversation,
    policy: &SpeakerPolicyRequest,
    available: &[&lettuce_conversations::ConversationParticipant],
) -> String {
    let total = policy
        .participants
        .iter()
        .map(|participant| u64::from(participant.speak_count))
        .sum::<u64>();
    let mut prompt = String::from(
        "You are selecting which character should respond next in a group conversation.\n\n## Participants\n",
    );
    for participant in available {
        let count = policy
            .participants
            .iter()
            .find(|state| state.id == participant.id)
            .map_or(0, |state| state.speak_count);
        prompt.push_str(&format!(
            "\n- Name: {}\n  ID: {}\n  Participation: {count} of {total} assistant messages\n",
            participant.display_name, participant.id
        ));
        if let Some(description) = &participant.authored_description {
            let excerpt = description.chars().take(1_000).collect::<String>();
            if !excerpt.trim().is_empty() {
                prompt.push_str(&format!("  Description: {}\n", excerpt.trim()));
            }
        }
    }
    let new_user_message = (policy.operation == lettuce_conversations::GenerationOperation::Send)
        .then(|| policy.timeline.last())
        .flatten()
        .filter(|item| item.message.role == MessageRole::User);
    let recent_end = policy.timeline.len() - usize::from(new_user_message.is_some());
    prompt.push_str("\n## Recent Conversation\n");
    for item in policy.timeline[..recent_end].iter().rev().take(10).rev() {
        let speaker = if item.message.role == MessageRole::User {
            "User"
        } else {
            item.message
                .author_participant_id
                .and_then(|id| {
                    conversation
                        .participants
                        .iter()
                        .find(|participant| participant.id == id)
                        .map(|participant| participant.display_name.as_str())
                })
                .unwrap_or("System")
        };
        let text = timeline_item_text(item);
        if !text.is_empty() {
            prompt.push_str(&format!(
                "\n- {speaker}: {}",
                text.chars().take(1_000).collect::<String>()
            ));
        }
    }
    if let Some(item) = new_user_message {
        let text = timeline_item_text(item);
        if !text.is_empty() {
            prompt.push_str("\n\n## New Message from User\n\n");
            prompt.push_str(&text.chars().take(1_000).collect::<String>());
        }
    }
    prompt.push_str(
        "\n\nChoose for relevance, expertise, participation balance, and natural flow. Use the select_next_speaker tool.",
    );
    prompt
}

fn timeline_item_text(item: &lettuce_conversations::TimelineItem) -> String {
    item.active_revision
        .as_ref()
        .map(|revision| revision.parts.as_slice())
        .or_else(|| {
            item.active_candidate
                .as_ref()
                .map(|candidate| candidate.parts.as_slice())
        })
        .unwrap_or_default()
        .iter()
        .filter_map(|part| match part {
            lettuce_conversations::MessagePart::Text { text } => Some(text.trim()),
            _ => None,
        })
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn speaker_selection_tools(
    available: &[&lettuce_conversations::ConversationParticipant],
) -> ToolRequest {
    ToolRequest {
        definitions: vec![ToolDefinition {
            name: "select_next_speaker".into(),
            description: Some(
                "Select which character should respond next in the group conversation.".into(),
            ),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "character_id": {
                        "type": "string",
                        "description": "ID of the character who should respond",
                        "enum": available.iter().map(|participant| participant.id.to_string()).collect::<Vec<_>>()
                    },
                    "reasoning": {
                        "type": "string",
                        "description": "Brief explanation of why this character should speak"
                    }
                },
                "required": ["character_id"]
            }),
            version: 1,
        }],
        choice: ToolChoice::Required,
    }
}

fn llm_speaker_decision(
    outcome: &lettuce_conversations::InferenceOutcome,
    available: &[&lettuce_conversations::ConversationParticipant],
    selection_model: &lettuce_conversations::ModelSelectionSnapshot,
    usage_event_id: UsageEventId,
) -> Option<SelectedSpeakerDecision> {
    for call in outcome
        .candidates
        .iter()
        .flat_map(|candidate| &candidate.tool_calls)
    {
        if call.name != "select_next_speaker" {
            continue;
        }
        let Some(participant_id) = call
            .arguments
            .get("character_id")
            .and_then(|value| value.as_str())
            .and_then(|value| value.parse().ok())
        else {
            continue;
        };
        if !available
            .iter()
            .any(|participant| participant.id == participant_id)
        {
            continue;
        }
        let rationale_summary = call
            .arguments
            .get("reasoning")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty() && value.len() <= 4_096)
            .map(str::to_owned);
        return Some(SelectedSpeakerDecision {
            participant_id,
            method: SpeakerDecisionMethod::Llm,
            fallback: SpeakerFallback::None,
            reference: None,
            rationale_summary,
            decision_model: Some(selection_model.clone()),
            usage_event_id: Some(usage_event_id),
        });
    }
    None
}

fn heuristic_fallback(
    policy: &SpeakerPolicyRequest,
    selection_model: Option<&lettuce_conversations::ModelSelectionSnapshot>,
    usage_event_id: Option<UsageEventId>,
) -> Result<SelectedSpeakerDecision, ConversationGenerationInputError> {
    let mut decision = select_group_speaker(
        policy,
        lettuce_conversations::GroupSpeakerSelectionSnapshot::Heuristic,
    )
    .map_err(|_| ConversationGenerationInputError::SpeakerUnavailable)?;
    decision.method = SpeakerDecisionMethod::Llm;
    decision.fallback = SpeakerFallback::Heuristic;
    decision.decision_model = selection_model.cloned();
    decision.usage_event_id = usage_event_id;
    Ok(decision)
}

fn select_memories<'a>(
    query_text: &str,
    query: &lettuce_embeddings::EmbeddingVector,
    projections: &[lettuce_embeddings::MemoryEmbeddingProjection],
    active: &HashMap<lettuce_types::MemoryId, &'a lettuce_memory::MemoryItem>,
    limit: usize,
    threshold: f32,
    strategy: MemoryRetrievalStrategySnapshot,
) -> Vec<&'a lettuce_memory::MemoryItem> {
    let mut scored = projections
        .iter()
        .filter_map(|projection| {
            let item = active.get(&projection.memory_id).copied()?;
            let raw = query.cosine_similarity(&projection.vector)?;
            let score = if item.is_cold && !item.is_pinned {
                raw * 0.7
            } else {
                raw
            };
            (score >= threshold).then_some((score, item))
        })
        .collect::<Vec<_>>();
    scored.sort_by(|(left_score, left), (right_score, right)| {
        right_score
            .total_cmp(left_score)
            .then_with(|| left.id.cmp(&right.id))
    });
    let mut selected = Vec::new();
    if strategy == MemoryRetrievalStrategySnapshot::Smart {
        let mut categories = HashMap::new();
        for (_, item) in &scored {
            let count = categories.entry(item.category).or_insert(0usize);
            if *count < 2 && selected.len() < limit {
                *count += 1;
                selected.push(*item);
            }
        }
    } else {
        selected.extend(scored.iter().take(limit).map(|(_, item)| *item));
    }
    if strategy == MemoryRetrievalStrategySnapshot::Smart && selected.len() < limit {
        for (_, item) in &scored {
            if selected.len() == limit {
                break;
            }
            if !selected.iter().any(|selected| selected.id == item.id) {
                selected.push(*item);
            }
        }
        for item in [
            active
                .values()
                .copied()
                .filter(|item| !item.is_cold)
                .max_by_key(|item| item.created_at),
            active
                .values()
                .copied()
                .filter(|item| !item.is_cold && item.access_count > 0)
                .max_by_key(|item| item.access_count),
        ]
        .into_iter()
        .flatten()
        {
            if selected.len() == limit {
                break;
            }
            if !selected.iter().any(|selected| selected.id == item.id) {
                selected.push(item);
            }
        }
    }
    if strategy == MemoryRetrievalStrategySnapshot::Smart && selected.is_empty() {
        let keywords = keywords(query_text);
        let mut cold = active
            .values()
            .copied()
            .filter(|item| item.is_cold)
            .filter_map(|item| {
                let normalized = normalize_memory_text(&item.text);
                let matches = keywords
                    .iter()
                    .filter(|keyword| normalized.contains(keyword.as_str()))
                    .count();
                (matches > 0).then_some((matches, item))
            })
            .collect::<Vec<_>>();
        cold.sort_by(|(left_count, left), (right_count, right)| {
            right_count
                .cmp(left_count)
                .then_with(|| left.id.cmp(&right.id))
        });
        selected.extend(cold.into_iter().take(limit).map(|(_, item)| item));
    }
    selected
}

fn normalize_memory_text(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut last_space = false;
    for character in value.chars() {
        if character.is_alphanumeric() {
            normalized.extend(character.to_lowercase());
            last_space = false;
        } else if !last_space {
            normalized.push(' ');
            last_space = true;
        }
    }
    normalized.trim().to_owned()
}

fn keywords(value: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    normalize_memory_text(value)
        .split_whitespace()
        .filter(|word| word.len() >= 3)
        .filter(|word| seen.insert((*word).to_owned()))
        .map(str::to_owned)
        .collect()
}
