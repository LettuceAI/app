use std::collections::BTreeMap;

use lettuce_conversations::{
    BranchStatus, Conversation, ConversationAggregate, ConversationBranch, ConversationKind,
    ConversationLifecycle, ConversationParticipant, ConversationParticipantDraft,
    GenerationAttempt, GenerationAttemptStatus, GenerationInput, GenerationOperation,
    GenerationTarget, GenerationTurn, GenerationTurnStatus, IdempotencyKey, InferenceUsage,
    InitialMessageDraft, InitialMessageOrigin, Message, MessageCandidate, MessagePart,
    MessageRenderSource, MessageRevision, MessageRole, MessageVisibility, ModelSelectionSnapshot,
    ParticipantRole, SnapshotArtifactDraft, SnapshotSelection, UsageCounters, UsageOutcome,
    UsageRecord, UsageUnavailableReason,
};
use lettuce_memory::{
    MAX_MEMORY_SUMMARY_SOURCE_MESSAGES, MemoryCategory, MemoryItem, MemoryShortId,
    MemorySpaceSnapshot, MemorySummary, Score,
};
use lettuce_transfer::{
    BackupConversation, BackupMemoryProjection, BackupMemoryProjectionState, BackupMemorySpace,
    BackupMessage, LEGACY_ID_NAMESPACE, LegacyBackupDirectSession,
    LegacyBackupMemoryEmbeddingOwner, LegacyBackupMemoryMaterialization,
    LegacyBackupMemoryOwnerKind, LegacyConversationRecord,
    LegacyDirectConversationMaterializationRequest, LegacyImportAdmission, LegacyImportAssignment,
    LegacyImportPlan, LegacyImportRepository, LegacyImportRepositoryError,
    LegacyImportStageReceipt,
};
use lettuce_types::{
    CharacterId, ConversationBranchId, ConversationId, ConversationParticipantId,
    GenerationAttemptId, GenerationTurnId, MemoryId, MemorySpaceId, MessageCandidateId, MessageId,
    MessageRevisionId, ModelProfileId, PersonaId, ProviderAccountId, Revision, SceneId,
    TimestampMillis, UsageEventId,
};
use lettuce_usage::UsageEvent;
use uuid::Uuid;

use crate::{
    ConversationLaunchPlanner, DIRECT_LAUNCH_REQUEST_FORMAT_V1, DirectConversationLaunchRequest,
    DirectLaunchSources, DirectUserParticipant, LaunchSelection,
};

type Error = LegacyImportRepositoryError;

#[derive(Debug)]
pub struct LegacyDirectConversationImportCoordinator<'a, S> {
    sources: &'a S,
}

pub(crate) struct ImportContext {
    models: BTreeMap<ModelProfileId, (ModelProfileId, ProviderAccountId)>,
    pub(crate) personas: BTreeMap<PersonaId, PersonaId>,
}

/// One legacy chat row in the shape both direct and group sessions share.
pub(crate) struct TimelineMessage<'a> {
    pub source_id: &'a str,
    pub role: &'a str,
    pub content: &'a str,
    pub created_at: u64,
    pub effective_at: Option<u64>,
    pub visible_in_chat: bool,
    pub pinned: bool,
    pub scene_edited: bool,
    pub author: Option<ConversationParticipantId>,
    pub model_source_id: Option<&'a str>,
    pub selected_variant_source_id: Option<&'a str>,
    pub reasoning: Option<&'a str>,
    pub variants: Vec<TimelineVariant<'a>>,
}

pub(crate) struct TimelineVariant<'a> {
    pub source_id: &'a str,
    pub content: &'a str,
    pub created_at: u64,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub reasoning: Option<&'a str>,
    pub author: Option<ConversationParticipantId>,
}

pub(crate) struct LegacyConversationSource<'a> {
    pub source_id: &'a str,
    pub title: String,
    pub kind: ConversationKind,
    pub participants: Vec<ConversationParticipantDraft>,
    pub initial_timeline: &'a [InitialMessageDraft],
    pub snapshots: Vec<SnapshotArtifactDraft>,
    pub model: Option<ModelSelectionSnapshot>,
    pub archived: bool,
    pub created_at: u64,
    pub updated_at: u64,
    pub messages: Vec<TimelineMessage<'a>>,
    pub memory: Option<&'a LegacyBackupMemoryEmbeddingOwner>,
    pub memory_summary: Option<&'a str>,
    pub memory_summary_token_count: u64,
}

impl<'a, S> LegacyDirectConversationImportCoordinator<'a, S>
where
    S: DirectLaunchSources + LegacyImportRepository,
{
    #[must_use]
    pub const fn new(sources: &'a S) -> Self {
        Self { sources }
    }

    /// Converts each legacy direct session into a finished conversation: launch
    /// snapshots come from the imported character, persona and models, and each
    /// assistant variant becomes a candidate of its own historical turn.
    pub fn execute(
        &self,
        admission: &LegacyImportAdmission,
        plan: &LegacyImportPlan,
        sessions: &[LegacyBackupDirectSession],
        memories: &[LegacyBackupMemoryEmbeddingOwner],
        completed_at: TimestampMillis,
    ) -> Result<LegacyImportStageReceipt, Error> {
        let plan_fingerprint = crate::legacy_import::plan_fingerprint(plan);
        if plan_fingerprint != admission.plan_fingerprint {
            return Err(Error::Conflict);
        }
        let source_fingerprint = plan.source_fingerprint.clone().ok_or(Error::InvalidInput)?;
        if let Some(receipt) = committed_stage(
            self.sources,
            admission,
            &source_fingerprint,
            lettuce_transfer::LegacyImportStage::DirectConversations,
            sessions.len(),
        )? {
            return Ok(receipt);
        }
        let context = import_context(admission, plan);
        let conversations = sessions
            .iter()
            .map(|session| {
                let memory = memory_owner(
                    memories,
                    LegacyBackupMemoryOwnerKind::DirectConversation,
                    &session.source_id,
                );
                self.map_session(session, memory, &context)
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.sources.materialize_direct_conversations(
            LegacyDirectConversationMaterializationRequest {
                run_id: admission.run_id,
                plan_fingerprint,
                source_fingerprint,
                conversations,
                completed_at,
            },
        )
    }

    pub fn execute_database_import(
        &self,
        admission: &LegacyImportAdmission,
        import: &crate::LegacyDatabaseImportPlan,
        completed_at: TimestampMillis,
    ) -> Result<LegacyImportStageReceipt, Error> {
        self.execute(
            admission,
            &import.plan,
            &import.compatibility.direct_sessions().sessions,
            &import.compatibility.memory_embeddings().owners,
            completed_at,
        )
    }

    fn map_session(
        &self,
        session: &LegacyBackupDirectSession,
        memory: Option<&LegacyBackupMemoryEmbeddingOwner>,
        context: &ImportContext,
    ) -> Result<LegacyConversationRecord, Error> {
        let mut rows = session.messages.iter().collect::<Vec<_>>();
        rows.sort_by_key(|message| message.ordinal);
        let opens_with_scene = opens_with_scene(
            rows.first()
                .map(|row| (row.role.as_str(), row.content.as_str())),
        );
        let persona = persona_selection(
            session.persona_disabled,
            session.persona_source_id.as_deref(),
            context,
        )?;
        let scene = match (&session.selected_scene_source_id, opens_with_scene) {
            (Some(id), true) => LaunchSelection::Explicit(parse::<SceneId>(id)?),
            _ => LaunchSelection::Disabled,
        };
        let character_id = parse::<CharacterId>(&session.character_source_id)?;
        let title = if session.title.trim().is_empty() {
            lettuce_characters::CharacterRepository::get(self.sources, character_id)
                .map_err(|_| Error::Storage)?
                .map(|details| crate::launch::policy::character_display_name(&details.character))
                .ok_or(Error::InvalidInput)?
        } else {
            session.title.clone()
        };
        let request = DirectConversationLaunchRequest {
            format_version: DIRECT_LAUNCH_REQUEST_FORMAT_V1,
            title,
            user: legacy_user(),
            character_id,
            scene,
            starter: LaunchSelection::Disabled,
            persona,
            operation_key: launch_key(&session.source_id)?,
        };
        let (plan, snapshots) = ConversationLaunchPlanner::new(self.sources)
            .prepare_direct(&request)
            .map_err(|_| Error::Conflict)?
            .into_parts();
        let character = plan
            .participants
            .iter()
            .find(|participant| participant.role == ParticipantRole::Character)
            .map(|participant| participant.id)
            .ok_or(Error::InvalidInput)?;
        let ConversationKind::Direct(details) = &plan.kind else {
            return Err(Error::InvalidInput);
        };
        let model = selected_model(&details.model);
        let messages = rows
            .iter()
            .map(|row| TimelineMessage {
                source_id: &row.source_id,
                role: &row.role,
                content: &row.content,
                created_at: row.created_at,
                effective_at: row.effective_at,
                visible_in_chat: row.visible_in_chat,
                pinned: row.pinned,
                scene_edited: row.scene_edited,
                author: (row.role == "assistant").then_some(character),
                model_source_id: row.model_source_id.as_deref(),
                selected_variant_source_id: row.selected_variant_source_id.as_deref(),
                reasoning: row.reasoning.as_deref(),
                variants: row
                    .variants
                    .iter()
                    .map(|variant| TimelineVariant {
                        source_id: &variant.source_id,
                        content: &variant.content,
                        created_at: variant.created_at,
                        prompt_tokens: variant.usage.prompt_tokens,
                        completion_tokens: variant.usage.completion_tokens,
                        reasoning: variant.reasoning.as_deref(),
                        author: Some(character),
                    })
                    .collect(),
            })
            .collect();
        conversation_record(
            LegacyConversationSource {
                source_id: &session.source_id,
                title: plan.title.clone(),
                kind: plan.kind.clone(),
                participants: plan.participants.clone(),
                initial_timeline: &plan.initial_timeline.entries,
                snapshots,
                model,
                archived: session.archived,
                created_at: session.created_at,
                updated_at: session.updated_at,
                messages,
                memory,
                memory_summary: session.memory_summary.as_deref(),
                memory_summary_token_count: session.memory_summary_token_count,
            },
            context,
        )
    }
}

pub(crate) fn memory_owner<'a>(
    owners: &'a [LegacyBackupMemoryEmbeddingOwner],
    kind: LegacyBackupMemoryOwnerKind,
    source_id: &str,
) -> Option<&'a LegacyBackupMemoryEmbeddingOwner> {
    owners
        .iter()
        .find(|owner| owner.kind == kind && owner.source_id == source_id)
}

/// Legacy session memories that fit the rewrite's memory item become the
/// conversation's memory space, their stored embeddings become ready
/// projections, and the rolling summary covers the latest imported messages.
/// Memories legacy stored in an incompatible shape stay in sealed evidence.
fn memory_space(
    conversation_id: ConversationId,
    source_id: &str,
    owner: Option<&LegacyBackupMemoryEmbeddingOwner>,
    summary: Option<&str>,
    summary_token_count: u64,
    messages: &[BackupMessage],
    updated_at: TimestampMillis,
) -> Result<(Option<BackupMemorySpace>, Vec<BackupMemoryProjection>), Error> {
    let space_id = MemorySpaceId::from_uuid(derived(source_id, "memory"));
    let mut items: Vec<MemoryItem> = Vec::new();
    let mut projections = Vec::new();
    for memory in owner.into_iter().flat_map(|owner| &owner.memories) {
        if memory.materialization == LegacyBackupMemoryMaterialization::RetainedEvidence {
            continue;
        }
        let id = parse::<MemoryId>(&memory.id)?;
        let short_id = MemoryShortId::allocate(id, |candidate| {
            items.iter().any(|item| item.short_id == candidate)
        });
        let score = |value: f32| {
            Score::from_ratio(if value.is_finite() {
                f64::from(value).clamp(0.0, 1.0)
            } else {
                0.0
            })
            .map_err(|_| Error::InvalidInput)
        };
        let created_at = timestamp(memory.created_at)?;
        let last_accessed_at = timestamp(memory.last_accessed_at)?.max(created_at);
        items.push(MemoryItem {
            id,
            short_id,
            text: memory.text.clone(),
            category: match memory.category.as_deref() {
                Some("character_trait") => MemoryCategory::CharacterTrait,
                Some("relationship") => MemoryCategory::Relationship,
                Some("plot_event") => MemoryCategory::PlotEvent,
                Some("world_detail") => MemoryCategory::WorldDetail,
                Some("preference") => MemoryCategory::Preference,
                _ => MemoryCategory::Other,
            },
            source_message_id: memory
                .source_message_id
                .as_deref()
                .map(|value| MessageId::from_uuid(legacy_uuid(value))),
            source_role: match memory.source_role.as_deref() {
                Some("user") => Some(MessageRole::User),
                Some("assistant") => Some(MessageRole::Assistant),
                _ => None,
            },
            observed_at: memory.observed_at.map(timestamp).transpose()?,
            observed_time_precision: memory.observed_time_precision.clone(),
            superseded_by: memory.superseded_by.as_deref().map(parse).transpose()?,
            superseded_at: memory.superseded_at.map(timestamp).transpose()?,
            supersedes: memory
                .supersedes
                .iter()
                .map(|value| parse(value))
                .collect::<Result<_, _>>()?,
            token_count: memory.token_count,
            is_cold: memory.is_cold && !memory.is_pinned,
            is_pinned: memory.is_pinned,
            importance: score(memory.importance_score)?,
            persistence_importance: score(memory.persistence_importance)?,
            prompt_importance: score(memory.prompt_importance)?,
            volatility: score(memory.volatility)?,
            access_count: memory.access_count,
            created_at,
            last_accessed_at,
        });
        let dimensions = memory.embedding.len();
        if memory.materialization == LegacyBackupMemoryMaterialization::InitialItemAndProjection
            && matches!(dimensions, 64 | 128 | 256 | 512 | 768)
            && let Some(source_revision) = &memory.embedding_source_version
        {
            projections.push(BackupMemoryProjection {
                space_id,
                memory_id: id,
                source_revision: source_revision.clone(),
                dimensions: u16::try_from(dimensions).map_err(|_| Error::InvalidInput)?,
                source_text: memory.text.clone(),
                state: BackupMemoryProjectionState::Ready {
                    vector_le_hex: memory
                        .embedding
                        .iter()
                        .flat_map(|value| value.to_le_bytes())
                        .map(|byte| format!("{byte:02x}"))
                        .collect(),
                },
                updated_at: last_accessed_at,
            });
        }
    }
    let summary = summary
        .filter(|text| !text.trim().is_empty() && !messages.is_empty())
        .map(|text| {
            let window = messages.len().min(MAX_MEMORY_SUMMARY_SOURCE_MESSAGES);
            let start = messages.len() - window;
            Ok::<_, Error>(MemorySummary {
                space_id,
                text: text.to_owned(),
                token_count: u32::try_from(summary_token_count).unwrap_or(u32::MAX),
                window_start: u64::try_from(start).map_err(|_| Error::InvalidInput)?,
                window_end: u64::try_from(messages.len()).map_err(|_| Error::InvalidInput)?,
                source_message_ids: messages[start..]
                    .iter()
                    .map(|message| message.message.id)
                    .collect(),
                updated_at,
            })
        })
        .transpose()?;
    if items.is_empty() && summary.is_none() {
        return Ok((None, Vec::new()));
    }
    Ok((
        Some(BackupMemorySpace {
            conversation_id,
            snapshot: MemorySpaceSnapshot {
                id: space_id,
                revision: Revision::INITIAL,
                items,
            },
            summary,
        }),
        projections,
    ))
}

pub(crate) fn committed_stage<S: LegacyImportRepository>(
    sources: &S,
    admission: &LegacyImportAdmission,
    source_fingerprint: &lettuce_types::ContentHash,
    stage: lettuce_transfer::LegacyImportStage,
    record_count: usize,
) -> Result<Option<LegacyImportStageReceipt>, Error> {
    let Some(receipt) = sources.stage_receipt(
        admission.run_id,
        stage,
        (&admission.plan_fingerprint, source_fingerprint),
    )?
    else {
        return Ok(None);
    };
    if receipt.record_count != u64::try_from(record_count).map_err(|_| Error::InvalidInput)? {
        return Err(Error::Conflict);
    }
    Ok(Some(receipt))
}

pub(crate) fn launch_key(source_id: &str) -> Result<IdempotencyKey, Error> {
    IdempotencyKey::new(format!("legacy-import.{}", legacy_uuid(source_id)))
        .map_err(|_| Error::InvalidInput)
}

pub(crate) fn opens_with_scene(first: Option<(&str, &str)>) -> bool {
    first.is_some_and(|(role, content)| role == "scene" && !content.trim().is_empty())
}

pub(crate) fn legacy_user() -> DirectUserParticipant {
    DirectUserParticipant {
        display_name: "User".to_owned(),
        authored_description: None,
    }
}

pub(crate) fn persona_selection(
    disabled: bool,
    persona_source_id: Option<&str>,
    context: &ImportContext,
) -> Result<LaunchSelection<PersonaId>, Error> {
    if disabled {
        return Ok(LaunchSelection::Disabled);
    }
    persona_source_id.map_or(Ok(LaunchSelection::Inherit), |id| {
        context
            .personas
            .get(&parse(id)?)
            .copied()
            .map(LaunchSelection::Explicit)
            .ok_or(Error::InvalidInput)
    })
}

pub(crate) fn selected_model(
    selection: &SnapshotSelection<ModelSelectionSnapshot>,
) -> Option<ModelSelectionSnapshot> {
    match selection {
        SnapshotSelection::Inherited(model) | SnapshotSelection::Explicit(model) => {
            Some(model.clone())
        }
        _ => None,
    }
}

pub(crate) fn conversation_record(
    source: LegacyConversationSource<'_>,
    context: &ImportContext,
) -> Result<LegacyConversationRecord, Error> {
    let conversation_id = ConversationId::from_uuid(legacy_uuid(source.source_id));
    let branch_id = ConversationBranchId::from_uuid(derived(source.source_id, "branch"));
    let created_at = timestamp(source.created_at)?;
    let updated_at = timestamp(source.updated_at)?.max(created_at);
    let user = source
        .participants
        .iter()
        .find(|participant| participant.role == ParticipantRole::User)
        .map(|participant| participant.id)
        .ok_or(Error::InvalidInput)?;
    let scene_origin = source
        .initial_timeline
        .iter()
        .find_map(|entry| match &entry.origin {
            origin @ InitialMessageOrigin::SelectedScene { .. } => Some(origin.clone()),
            InitialMessageOrigin::StarterMessage { .. } => None,
        });
    let opens = opens_with_scene(
        source
            .messages
            .first()
            .map(|message| (message.role, message.content)),
    );
    let mut writer = SessionWriter {
        conversation_id,
        branch_id,
        user,
        model: source.model,
        context,
        messages: Vec::new(),
        turns: Vec::new(),
        usage: Vec::new(),
    };
    let mut parent: Option<(MessageId, MessageRole)> = None;
    for (index, message) in source.messages.iter().enumerate() {
        let origin = (index == 0 && opens)
            .then(|| scene_origin.clone())
            .flatten();
        parent = Some(writer.push(message, parent, origin)?);
    }
    let conversation = Conversation {
        id: conversation_id,
        lifecycle: if source.archived {
            ConversationLifecycle::Archived
        } else {
            ConversationLifecycle::Active
        },
        title: source.title,
        kind: source.kind,
        active_branch_id: branch_id,
        participants: source
            .participants
            .iter()
            .map(|draft| ConversationParticipant {
                id: draft.id,
                role: draft.role,
                ordinal: draft.ordinal,
                enabled: draft.enabled,
                muted: draft.muted,
                source: draft.source,
                display_name: draft.display_name.clone(),
                authored_description: draft.authored_description.clone(),
                model_selection: draft.model_selection.clone(),
                revision: Revision::INITIAL,
                created_at,
                updated_at: created_at,
            })
            .collect(),
        current_settings: None,
        revision: Revision::INITIAL,
        created_at,
        updated_at,
    };
    let branch = ConversationBranch {
        id: branch_id,
        conversation_id,
        parent_branch_id: None,
        fork_message_id: None,
        head_message_id: parent.map(|(id, _)| id),
        status: BranchStatus::Active,
        revision: Revision::INITIAL,
        created_at,
        updated_at,
    };
    let (memory, memory_projections) = memory_space(
        conversation_id,
        source.source_id,
        source.memory,
        source.memory_summary,
        source.memory_summary_token_count,
        &writer.messages,
        updated_at,
    )?;
    Ok(LegacyConversationRecord {
        history: BackupConversation {
            aggregate: ConversationAggregate {
                conversation,
                branches: vec![branch],
            },
            messages: writer.messages,
        },
        turns: writer.turns,
        usage: writer.usage,
        snapshots: source.snapshots,
        memory,
        memory_projections,
    })
}

struct SessionWriter<'a> {
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
    user: ConversationParticipantId,
    model: Option<ModelSelectionSnapshot>,
    context: &'a ImportContext,
    messages: Vec<BackupMessage>,
    turns: Vec<GenerationTurn>,
    usage: Vec<UsageEvent>,
}

impl SessionWriter<'_> {
    fn push(
        &mut self,
        legacy: &TimelineMessage<'_>,
        parent: Option<(MessageId, MessageRole)>,
        origin: Option<InitialMessageOrigin>,
    ) -> Result<(MessageId, MessageRole), Error> {
        let message_id = MessageId::from_uuid(legacy_uuid(legacy.source_id));
        let created_at = timestamp(legacy.created_at)?;
        let effective_time = legacy
            .effective_at
            .map(timestamp)
            .transpose()?
            .unwrap_or(created_at);
        let (role, author, visibility) = match (legacy.role, origin.is_some()) {
            ("scene", true) => (MessageRole::Scene, None, MessageVisibility::Visible),
            ("scene", false) => (MessageRole::System, None, MessageVisibility::Visible),
            ("user", _) => (
                MessageRole::User,
                Some(self.user),
                MessageVisibility::Visible,
            ),
            ("assistant", _) => (
                MessageRole::Assistant,
                Some(legacy.author.ok_or(Error::InvalidInput)?),
                MessageVisibility::Visible,
            ),
            ("system", _) => (
                MessageRole::System,
                None,
                if legacy.visible_in_chat {
                    MessageVisibility::Visible
                } else {
                    MessageVisibility::Hidden
                },
            ),
            _ => return Err(Error::InvalidInput),
        };
        let mut revisions = Vec::new();
        let mut candidates = Vec::new();
        let variants_as_candidates = role == MessageRole::Assistant
            && !legacy.variants.is_empty()
            && parent.is_some()
            && self.model.is_some();
        let active_render_source = if variants_as_candidates {
            self.push_candidates(legacy, message_id, parent, &mut candidates)?
        } else if legacy.variants.is_empty() {
            let revision = MessageRevision {
                id: MessageRevisionId::from_uuid(derived(legacy.source_id, "revision")),
                message_id,
                sequence: Revision::INITIAL,
                parts: parts(legacy.content, legacy.reasoning),
                authored_at: created_at,
                source_turn_id: None,
                provider_replay: None,
            };
            let active = MessageRenderSource::Revision(revision.id);
            revisions.push(revision);
            active
        } else {
            let mut active = None;
            for (index, variant) in legacy.variants.iter().enumerate() {
                let revision = MessageRevision {
                    id: MessageRevisionId::from_uuid(derived(variant.source_id, "revision")),
                    message_id,
                    sequence: Revision::new(
                        u64::try_from(index + 1).map_err(|_| Error::InvalidInput)?,
                    ),
                    parts: parts(
                        variant.content,
                        variant.reasoning.or((index == active_variant_index(legacy))
                            .then_some(legacy.reasoning)
                            .flatten()),
                    ),
                    authored_at: timestamp(variant.created_at)?,
                    source_turn_id: None,
                    provider_replay: None,
                };
                if index == active_variant_index(legacy) {
                    active = Some(revision.id);
                }
                revisions.push(revision);
            }
            MessageRenderSource::Revision(active.ok_or(Error::InvalidInput)?)
        };
        let author = match active_render_source {
            MessageRenderSource::Candidate(active) => candidates
                .iter()
                .find(|candidate| candidate.id == active)
                .map(|candidate| candidate.author_participant_id)
                .or(author),
            MessageRenderSource::Revision(_) => author,
        };
        let timeline_ordinal =
            u64::try_from(self.messages.len() + 1).map_err(|_| Error::InvalidInput)?;
        self.messages.push(BackupMessage {
            message: Message {
                id: message_id,
                conversation_id: self.conversation_id,
                branch_id: self.branch_id,
                parent_message_id: parent.map(|(id, _)| id),
                author_participant_id: author,
                role,
                logical_time: created_at,
                effective_time,
                visibility,
                pinned: legacy.pinned,
                scene_edited: legacy.scene_edited && role == MessageRole::Scene,
                active_render_source,
                revision: Revision::INITIAL,
                created_at,
                updated_at: created_at,
            },
            timeline_ordinal,
            initial_origin: origin,
            revisions,
            candidates,
        });
        Ok((message_id, role))
    }

    fn push_candidates(
        &mut self,
        legacy: &TimelineMessage<'_>,
        message_id: MessageId,
        parent: Option<(MessageId, MessageRole)>,
        candidates: &mut Vec<MessageCandidate>,
    ) -> Result<MessageRenderSource, Error> {
        let (parent_id, parent_role) = parent.ok_or(Error::InvalidInput)?;
        let model = self.model.clone().ok_or(Error::InvalidInput)?;
        let (usage_model, usage_provider, usage_model_revision, usage_provider_revision) = legacy
            .model_source_id
            .and_then(|id| id.parse::<ModelProfileId>().ok())
            .and_then(|id| self.context.models.get(&id))
            .map_or(
                (
                    model.source_id,
                    model.provider_account_id,
                    model.source_revision,
                    model.provider_account_revision,
                ),
                |(model_id, provider_id)| {
                    (
                        *model_id,
                        *provider_id,
                        Revision::INITIAL,
                        Revision::INITIAL,
                    )
                },
            );
        let mut previous: Option<MessageCandidateId> = None;
        let mut active = None;
        let active_index = active_variant_index(legacy);
        for (index, variant) in legacy.variants.iter().enumerate() {
            let candidate_id = MessageCandidateId::from_uuid(legacy_uuid(variant.source_id));
            let turn_id = GenerationTurnId::from_uuid(derived(variant.source_id, "turn"));
            let attempt_id = GenerationAttemptId::from_uuid(derived(variant.source_id, "attempt"));
            let usage_event_id = UsageEventId::from_uuid(derived(variant.source_id, "usage"));
            let at = timestamp(variant.created_at)?;
            let (operation, input, target) = match previous {
                None if parent_role == MessageRole::User => (
                    GenerationOperation::Send,
                    GenerationInput::UserMessage {
                        message_id: parent_id,
                    },
                    GenerationTarget::NewAssistant {
                        message_id,
                        parent_message_id: Some(parent_id),
                    },
                ),
                None => (
                    GenerationOperation::Continue,
                    GenerationInput::ExistingHead {
                        head_message_id: parent_id,
                    },
                    GenerationTarget::NewAssistant {
                        message_id,
                        parent_message_id: Some(parent_id),
                    },
                ),
                Some(prior) => (
                    GenerationOperation::Regenerate,
                    GenerationInput::ExistingCandidate {
                        message_id,
                        candidate_id: prior,
                    },
                    GenerationTarget::ExistingCandidate {
                        message_id,
                        prior_candidate_id: prior,
                    },
                ),
            };
            self.turns.push(GenerationTurn {
                id: turn_id,
                conversation_id: self.conversation_id,
                branch_id: self.branch_id,
                operation,
                input,
                target,
                swap_roles: false,
                retry_of_turn_id: None,
                idempotency_key: IdempotencyKey::new(format!("legacy-import.turn.{turn_id}"))
                    .map_err(|_| Error::InvalidInput)?,
                correlation_id: None,
                status: GenerationTurnStatus::Succeeded,
                selected_speaker: None,
                guidance: None,
                requested_model_override: None,
                forced_speaker: None,
                resolved_model: Some(model.clone()),
                prompt: None,
                lorebooks: Vec::new(),
                memory: None,
                candidate_ids: vec![candidate_id],
                selected_candidate_id: Some(candidate_id),
                attempts: vec![GenerationAttempt {
                    id: attempt_id,
                    turn_id,
                    ordinal: 0,
                    parent_attempt_id: None,
                    status: GenerationAttemptStatus::Succeeded,
                    job_idempotency_key: IdempotencyKey::new(format!(
                        "generation.{turn_id}.{attempt_id}"
                    ))
                    .map_err(|_| Error::InvalidInput)?,
                    job_id: None,
                    started_at: Some(at),
                    finished_at: Some(at),
                    candidate_ids: vec![candidate_id],
                    usage_event_id: Some(usage_event_id),
                    failure: None,
                }],
                failure: None,
                revision: Revision::INITIAL,
                created_at: at,
                updated_at: at,
            });
            let counters = match (variant.prompt_tokens, variant.completion_tokens) {
                (Some(input_tokens), Some(output_tokens)) => UsageCounters::Known(InferenceUsage {
                    provider_reported_cost: None,
                    cache_write_tokens: None,
                    web_search_requests: None,
                    cached_input_tokens: None,
                    reasoning_tokens: None,
                    input_tokens,
                    output_tokens,
                }),
                _ => UsageCounters::Unavailable(UsageUnavailableReason::ProviderOmitted),
            };
            self.usage.push(UsageEvent {
                id: usage_event_id,
                record: UsageRecord {
                    turn_id,
                    attempt_id,
                    outcome: UsageOutcome::Succeeded,
                    usage: counters,
                    model_profile_id: Some(usage_model),
                    model_revision: Some(usage_model_revision),
                    provider_account_id: Some(usage_provider),
                    provider_account_revision: Some(usage_provider_revision),
                    recorded_at: at,
                },
            });
            candidates.push(MessageCandidate {
                id: candidate_id,
                message_id,
                turn_id,
                attempt_id,
                author_participant_id: variant
                    .author
                    .or(legacy.author)
                    .ok_or(Error::InvalidInput)?,
                ordinal: u16::try_from(index).map_err(|_| Error::InvalidInput)?,
                parts: parts(
                    variant.content,
                    variant.reasoning.or((index == active_index)
                        .then_some(legacy.reasoning)
                        .flatten()),
                ),
                model: model.clone(),
                created_at: at,
                provider_replay: None,
            });
            if index == active_index {
                active = Some(candidate_id);
            }
            previous = Some(candidate_id);
        }
        Ok(MessageRenderSource::Candidate(
            active.or(previous).ok_or(Error::InvalidInput)?,
        ))
    }
}

/// Legacy rendered the selected variant, and without a selection the message
/// content, which edits kept equal to one variant; the last one is the
/// fallback when neither matches.
fn active_variant_index(legacy: &TimelineMessage<'_>) -> usize {
    legacy
        .selected_variant_source_id
        .and_then(|selected| {
            legacy
                .variants
                .iter()
                .position(|variant| variant.source_id == selected)
        })
        .or_else(|| {
            legacy
                .variants
                .iter()
                .rposition(|variant| variant.content == legacy.content)
        })
        .unwrap_or_else(|| legacy.variants.len().saturating_sub(1))
}

pub(crate) fn import_context(
    admission: &LegacyImportAdmission,
    plan: &LegacyImportPlan,
) -> ImportContext {
    let mut providers = BTreeMap::new();
    let mut models = BTreeMap::new();
    let mut personas = BTreeMap::new();
    for assignment in &admission.assignments {
        match assignment {
            LegacyImportAssignment::ProviderAccount {
                legacy_id,
                destination_id,
                ..
            } => {
                providers.insert(*legacy_id, *destination_id);
            }
            LegacyImportAssignment::ModelProfile {
                legacy_id,
                destination_id,
            } => {
                models.insert(*legacy_id, *destination_id);
            }
            LegacyImportAssignment::Persona {
                legacy_id,
                destination_id,
            } => {
                personas.insert(*legacy_id, *destination_id);
            }
            _ => {}
        }
    }
    let models = plan
        .provider_models
        .model_profiles
        .iter()
        .filter_map(|model| {
            Some((
                model.id,
                (
                    *models.get(&model.id)?,
                    *providers.get(&model.provider_account_id)?,
                ),
            ))
        })
        .collect();
    ImportContext { models, personas }
}

pub(crate) fn legacy_uuid(value: &str) -> Uuid {
    Uuid::parse_str(value).unwrap_or_else(|_| Uuid::new_v5(&LEGACY_ID_NAMESPACE, value.as_bytes()))
}

pub(crate) fn derived(value: &str, suffix: &str) -> Uuid {
    Uuid::new_v5(&LEGACY_ID_NAMESPACE, format!("{value}:{suffix}").as_bytes())
}

pub(crate) fn parse<T: std::str::FromStr>(value: &str) -> Result<T, Error> {
    value.parse().map_err(|_| Error::InvalidInput)
}

fn timestamp(value: u64) -> Result<TimestampMillis, Error> {
    i64::try_from(value)
        .map(TimestampMillis::new)
        .map_err(|_| Error::InvalidInput)
}

fn parts(content: &str, reasoning: Option<&str>) -> Vec<MessagePart> {
    let mut parts = Vec::new();
    if let Some(reasoning) = reasoning.filter(|value| !value.trim().is_empty()) {
        parts.push(MessagePart::ReasoningSummary {
            text: reasoning.to_owned(),
        });
    }
    if !content.is_empty() {
        parts.push(MessagePart::Text {
            text: content.to_owned(),
        });
    }
    parts
}
