use std::collections::HashSet;

use lettuce_characters::{CharacterRepository, PersonaRepository};
use lettuce_companions::{
    CompanionScheduledNoteRepository, CompanionStateRepository, SoulRepository,
};
use lettuce_conversations::{
    ContextAssembler, ContextAssemblyError, ContextRequest, ConversationKind, ConversationReader,
    ConversationRepository, ConversationRepositoryError, ConversationSnapshotMaterializer,
    GenerationInput, InferencePort, MemoryModeSnapshot, OutputPolicy, PromptRuntimeFacts,
    PromptRuntimeValues, ProviderContextPart, ResolvedInferenceProfile, SafetyContext, ToolPolicy,
};
use lettuce_embeddings::MemoryEmbeddingRepository;
use lettuce_memory::{
    DynamicMemoryPreparationRepository, DynamicMemoryRoundRepository, MemoryRepository,
};
use lettuce_models::{
    CapabilityStatus, ChatParameterResolutionInput, ChatProfileResolutionError, ChatRequirements,
    ModelProfileRepository, ModelRepositoryError, ProviderAccountRepository,
};
use lettuce_types::{PageLimit, PageRequest, RequestId, TimestampMillis};
use lettuce_usage::JobUsageLedger;

use crate::{
    ConversationContextAssembler, ConversationGenerationClaimedWork, ConversationGenerationInput,
    ConversationGenerationJobRunner, ConversationGenerationRunError,
    ConversationGenerationRunResult, MemoryCreateSeed, MemoryEmbeddingEngine,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConversationGenerationRuntimeInput {
    pub stream_sink: Option<RequestId>,
    pub prompt_values: PromptRuntimeValues,
}

#[derive(Debug)]
enum ConversationGenerationInputError {
    Repository(ConversationRepositoryError),
    ModelRepository(ModelRepositoryError),
    MissingModel,
    Profile(ChatProfileResolutionError),
    Context(ContextAssemblyError),
    MemoryInputUnavailable,
    GroupUnsupported,
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
        + lettuce_conversations::ToolExecutionRepository
        + lettuce_conversations::ProviderReplayArtifactPort
        + JobUsageLedger
        + lettuce_conversations::UsagePort
        + DynamicMemoryRoundRepository
        + DynamicMemoryPreparationRepository
        + MemoryEmbeddingRepository
        + MemoryRepository,
    I: InferencePort + ?Sized,
{
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
        let input = self
            .build_input(work, runtime)
            .await
            .map_err(ConversationGenerationInputError::into_run_error)?;
        ConversationGenerationJobRunner::new(self.embedding, self.repository, self.inference)
            .run(work, input, now, seeds_for_round)
            .await
    }

    async fn build_input(
        &self,
        work: &ConversationGenerationClaimedWork,
        runtime: ConversationGenerationRuntimeInput,
    ) -> Result<ConversationGenerationInput, ConversationGenerationInputError> {
        let aggregate = ConversationReader::get(self.repository, work.conversation_id)
            .map_err(ConversationGenerationInputError::Repository)?;
        if !matches!(aggregate.conversation.kind, ConversationKind::Direct(_)) {
            return Err(ConversationGenerationInputError::GroupUnsupported);
        }
        let turn = ConversationReader::get_turn(self.repository, work.turn_id)
            .map_err(ConversationGenerationInputError::Repository)?;
        if turn.conversation_id != work.conversation_id
            || turn.branch_id != aggregate.conversation.active_branch_id
        {
            return Err(ConversationGenerationInputError::InvalidTurn);
        }
        let settings = lettuce_conversations::resolve_effective_settings(
            &aggregate.conversation,
            turn.selected_speaker
                .as_ref()
                .map(|speaker| speaker.participant_id),
        )
        .map_err(|_| ConversationGenerationInputError::InvalidTurn)?;
        if settings.memory.as_ref().is_some_and(|memory| {
            memory.mode == MemoryModeSnapshot::Dynamic || !memory.selected_revision_ids.is_empty()
        }) {
            return Err(ConversationGenerationInputError::MemoryInputUnavailable);
        }
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
        let prompt_runtime = PromptRuntimeFacts {
            provider_id: Some(account.provider_kind),
            provider_label: Some(account.label),
            input_scopes: modality_scopes(profile.capabilities.input_modalities),
            output_scopes: modality_scopes(profile.capabilities.output_modalities),
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
                selected_speaker: turn.selected_speaker.clone(),
                capabilities: profile.capabilities.clone(),
                safety: SafetyContext::Standard,
                prompt_runtime,
                prompt_values: runtime.prompt_values,
                memory: None,
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
                tool_policy: ToolPolicy::Disabled,
                output_policy: OutputPolicy::Plain,
                safety_policy: SafetyContext::Standard,
                correlation_id: turn.correlation_id,
            },
            context,
            tools: None,
            media_grants,
            stream_sink: runtime.stream_sink,
            memory: None,
        })
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
            Self::MemoryInputUnavailable | Self::GroupUnsupported | Self::InvalidTurn => {
                ConversationGenerationRunError::InvalidInput
            }
        }
    }
}
