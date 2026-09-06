use std::collections::{HashMap, HashSet};

use lettuce_characters::{CharacterRepository, PersonaRepository};
use lettuce_companions::{
    CompanionScheduledNoteRepository, CompanionStateRepository, SoulRepository,
};
use lettuce_conversations::{
    ContextAssembler, ContextAssemblyError, ContextRequest, ConversationKind, ConversationReader,
    ConversationRepository, ConversationRepositoryError, ConversationSnapshotMaterializer,
    GenerationInput, InferencePort, MemoryAttribution, MemoryContribution, MemoryModeSnapshot,
    MessageRole, OutputPolicy, PromptRuntimeFacts, PromptRuntimeValues, ProviderContextPart,
    ResolvedInferenceProfile, SafetyContext, ToolPolicy,
};
use lettuce_embeddings::{EmbeddingDimensions, EmbeddingRequest, MemoryEmbeddingRepository};
use lettuce_memory::{
    DynamicMemoryPreparationRepository, DynamicMemoryRoundRepository, MemoryPolicy,
    MemoryRepository, MemoryRepositoryError, MemorySpaceSnapshot, MemorySummaryRepository, Score,
    dynamic_memory_tool_request, memory_revision_id,
};
use lettuce_models::{
    CapabilityStatus, ChatParameterResolutionInput, ChatProfileResolutionError, ChatRequirements,
    ModelProfileRepository, ModelRepositoryError, ProviderAccountRepository,
};
use lettuce_settings::{
    DynamicMemorySettings, GlobalSettingsStore, GlobalSettingsStoreError, MemoryRetrievalStrategy,
};
use lettuce_types::{PageLimit, PageRequest, RequestId, TimestampMillis};
use lettuce_usage::JobUsageLedger;

use crate::{
    ConversationContextAssembler, ConversationGenerationClaimedWork, ConversationGenerationInput,
    ConversationGenerationJobRunner, ConversationGenerationMemoryInput,
    ConversationGenerationRunError, ConversationGenerationRunResult, EmbeddingGenerationError,
    MemoryCreateSeed, MemoryEmbeddingEngine,
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
    Settings(GlobalSettingsStoreError),
    Memory(MemoryRepositoryError),
    Embedding,
    Cancelled,
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
        + MemoryRepository
        + MemorySummaryRepository
        + GlobalSettingsStore,
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
        if settings
            .memory
            .as_ref()
            .is_some_and(|memory| !memory.selected_revision_ids.is_empty())
        {
            return Err(ConversationGenerationInputError::MemoryInputUnavailable);
        }
        let dynamic_memory = settings
            .memory
            .as_ref()
            .is_some_and(|memory| memory.mode == MemoryModeSnapshot::Dynamic);
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
        let (memory_contribution, memory_input) = if dynamic_memory {
            let prepared = self.dynamic_memory_input(work, &timeline.items).await?;
            (prepared.0, Some(prepared.1))
        } else {
            (None, None)
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
                selected_speaker: turn.selected_speaker.clone(),
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

    async fn dynamic_memory_input(
        &self,
        work: &ConversationGenerationClaimedWork,
        timeline: &[lettuce_conversations::TimelineItem],
    ) -> Result<
        (
            Option<MemoryContribution>,
            ConversationGenerationMemoryInput,
        ),
        ConversationGenerationInputError,
    > {
        let settings = GlobalSettingsStore::load(self.repository)
            .map_err(ConversationGenerationInputError::Settings)?
            .settings
            .dynamic_memory;
        let (policy, duplicate_threshold) = dynamic_memory_policy(&settings)?;
        let memory = MemoryRepository::get_for_conversation(self.repository, work.conversation_id)
            .map_err(ConversationGenerationInputError::Memory)?
            .ok_or(ConversationGenerationInputError::MemoryInputUnavailable)?;
        let summary = MemorySummaryRepository::get_summary(self.repository, memory.id)
            .map_err(ConversationGenerationInputError::Memory)?
            .map(|summary| summary.text);
        let key_memories = self
            .retrieve_memories(work, timeline, &memory, &settings)
            .await?;
        let contribution =
            (summary.is_some() || !key_memories.is_empty()).then(|| MemoryContribution {
                attribution: MemoryAttribution {
                    revision_id: memory_revision_id(memory.id, memory.revision),
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

    async fn retrieve_memories(
        &self,
        work: &ConversationGenerationClaimedWork,
        timeline: &[lettuce_conversations::TimelineItem],
        memory: &MemorySpaceSnapshot,
        settings: &DynamicMemorySettings,
    ) -> Result<Vec<String>, ConversationGenerationInputError> {
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
        Ok(selected
            .into_iter()
            .map(|item| format!("- {}", item.text.trim()))
            .collect())
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
            Self::Settings(error) => {
                tracing::warn!(?error, "dynamic-memory settings preparation failed");
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
            Self::MemoryInputUnavailable | Self::GroupUnsupported | Self::InvalidTurn => {
                ConversationGenerationRunError::InvalidInput
            }
        }
    }
}

fn dynamic_memory_policy(
    settings: &DynamicMemorySettings,
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

fn select_memories<'a>(
    query_text: &str,
    query: &lettuce_embeddings::EmbeddingVector,
    projections: &[lettuce_embeddings::MemoryEmbeddingProjection],
    active: &HashMap<lettuce_types::MemoryId, &'a lettuce_memory::MemoryItem>,
    limit: usize,
    threshold: f32,
    strategy: MemoryRetrievalStrategy,
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
    if strategy == MemoryRetrievalStrategy::Smart {
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
    if strategy == MemoryRetrievalStrategy::Smart && selected.len() < limit {
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
    if strategy == MemoryRetrievalStrategy::Smart && selected.is_empty() {
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
