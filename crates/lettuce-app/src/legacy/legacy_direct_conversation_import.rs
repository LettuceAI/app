use std::collections::BTreeMap;

use lettuce_companions::{
    CompanionRuntimeState, CompanionStateOwner, EmotionVector, EmotionalState, RelationshipState,
};
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
    MemoryCategory, MemoryItem, MemoryShortId, MemorySpaceSnapshot, MemorySummary, Score,
};
use lettuce_transfer::{
    BackupConversation, BackupMemoryProjection, BackupMemoryProjectionState, BackupMemorySpace,
    BackupMessage, LegacyBackupDirectSession, LegacyBackupMemoryEmbeddingOwner,
    LegacyBackupMemoryMaterialization, LegacyBackupMemoryOwnerKind, LegacyConversationRecord,
    LegacyDirectConversationMaterializationRequest, LegacyIdScope, LegacyImportAdmission,
    LegacyImportAssignment, LegacyImportPlan, LegacyImportRepository, LegacyImportRepositoryError,
    LegacyImportStageReceipt,
};
use lettuce_transfer::{
    LegacyBackupCompanionMaterialization, LegacyBackupCompanionSharedMemory,
    LegacyBackupScheduledNote, LegacyCompanionConversation, LegacyCompanionEpisodeRecord,
};
use lettuce_types::{
    CharacterId, ConversationBranchId, ConversationId, ConversationParticipantId,
    GenerationAttemptId, GenerationTurnId, MemoryId, MemorySpaceId, MessageCandidateId, MessageId,
    MessageRevisionId, ModelProfileId, PersonaId, ProviderAccountId, Revision, SceneId,
    TimestampMillis, UsageEventId,
};
use lettuce_usage::UsageEvent;

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
    prompts: BTreeMap<String, lettuce_types::PromptDocumentId>,
    lorebooks: BTreeMap<lettuce_types::LorebookId, lettuce_types::LorebookId>,
    media: BTreeMap<String, lettuce_types::AssetId>,
    backgrounds: BTreeMap<(String, bool), lettuce_types::AssetId>,
    pub(crate) scope: LegacyIdScope,
    dynamic_memory_enabled: bool,
}

impl ImportContext {
    /// A context with no legacy references, for a file imported on its own.
    pub(crate) fn fresh(scope: LegacyIdScope) -> Self {
        Self {
            models: BTreeMap::new(),
            personas: BTreeMap::new(),
            prompts: BTreeMap::new(),
            lorebooks: BTreeMap::new(),
            media: BTreeMap::new(),
            backgrounds: BTreeMap::new(),
            scope,
            dynamic_memory_enabled: false,
        }
    }

    pub(crate) fn model_destination(&self, legacy: ModelProfileId) -> Option<ModelProfileId> {
        self.models.get(&legacy).map(|(model, _)| *model)
    }

    /// A session's own legacy background: `value` is what it set, `None` when
    /// it followed its scene, character or group. A background whose image
    /// did not import (recorded) shows none, as legacy did.
    pub(crate) fn background(
        &self,
        session_id: &str,
        group: bool,
        value: Option<&str>,
    ) -> Option<lettuce_conversations::ConversationBackground> {
        value?;
        Some(
            match self.backgrounds.get(&(session_id.to_owned(), group)) {
                Some(asset_id) => lettuce_conversations::ConversationBackground::Image {
                    asset_id: *asset_id,
                },
                None => lettuce_conversations::ConversationBackground::Hidden,
            },
        )
    }

    /// The imported media parts of legacy attachment columns, in legacy order
    /// and once per attachment id; attachments the media plan left out (and
    /// recorded) have no asset.
    fn attachment_parts(&self, columns: &[&str]) -> Vec<MessagePart> {
        let mut seen = std::collections::BTreeSet::new();
        columns
            .iter()
            .flat_map(|raw| lettuce_transfer::legacy_message_attachments(raw))
            .filter(|attachment| seen.insert(attachment.id.clone()))
            .filter_map(|attachment| {
                let asset_id = *self.media.get(attachment.stored_path()?)?;
                Some(MessagePart::MediaAsset {
                    asset_id,
                    role: lettuce_conversations::MediaAssetRole::Attachment,
                })
            })
            .collect()
    }
}

/// The per-session values legacy let a chat override on top of its launch
/// sources.
pub(crate) struct SessionSettingsSource<'a> {
    pub author_note: Option<&'a str>,
    pub prompt_source_id: Option<&'a str>,
    pub prompt_purposes: &'a [lettuce_context::PromptPurpose],
    pub prompt_snapshot_purpose: lettuce_conversations::PromptPurposeSnapshot,
    pub lorebook_source_ids: Option<&'a [String]>,
    pub speaker_selection: Option<lettuce_conversations::GroupSpeakerSelectionSnapshot>,
    pub model_settings: &'a lettuce_models::ModelSettingsLayer,
    pub background: Option<lettuce_conversations::ConversationBackground>,
    pub companion_clock: Option<lettuce_conversations::CompanionClockSettings>,
    /// Legacy `persona_disabled`: the chat turned its persona off.
    pub persona_disabled: bool,
    /// A group session's own `chatType` override.
    pub chat_mode: Option<lettuce_conversations::GroupChatModeSnapshot>,
    /// A group session's own `disableCharacterLorebooks` override.
    pub disable_character_lorebooks: Option<bool>,
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
    pub attachments_json: &'a str,
    /// The message row's speed stats, which are its selected variant's.
    pub speed: TimelineSpeed<'a>,
    pub variants: Vec<TimelineVariant<'a>>,
}

/// A row's generation speed columns (`first_token_ms`, `tokens_per_second`,
/// and `mtp_stats`, which holds MTP and DFlash stats alike).
#[derive(Clone, Copy, Default)]
pub(crate) struct TimelineSpeed<'a> {
    pub first_token_ms: Option<u64>,
    pub tokens_per_second: Option<f64>,
    pub mtp_stats_json: Option<&'a str>,
}

impl TimelineSpeed<'_> {
    fn is_empty(&self) -> bool {
        self.first_token_ms.is_none()
            && self.tokens_per_second.is_none()
            && self.mtp_stats_json.is_none()
    }

    /// The summary a local generation records for its message, with the
    /// runtime's keys; an `mtp_stats` value that is not JSON is left out.
    fn summary(&self, variant: &TimelineVariant<'_>) -> serde_json::Value {
        let mut summary = serde_json::Map::new();
        let mut put = |key: &str, value: Option<serde_json::Value>| {
            if let Some(value) = value {
                summary.insert(key.to_owned(), value);
            }
        };
        put("promptTokens", variant.prompt_tokens.map(Into::into));
        put(
            "completionTokens",
            variant.completion_tokens.map(Into::into),
        );
        put("totalTokens", variant.total_tokens.map(Into::into));
        put("ttftMs", self.first_token_ms.map(Into::into));
        put(
            "decodeTokensPerSecond",
            self.tokens_per_second
                .and_then(serde_json::Number::from_f64)
                .map(serde_json::Value::Number),
        );
        put(
            "mtpStats",
            self.mtp_stats_json
                .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
                .filter(|value| !value.is_null()),
        );
        serde_json::Value::Object(summary)
    }
}

pub(crate) struct TimelineVariant<'a> {
    pub source_id: &'a str,
    pub content: &'a str,
    pub created_at: u64,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub reasoning: Option<&'a str>,
    /// Legacy group variants kept their own attachments; direct ones had none.
    pub attachments_json: Option<&'a str>,
    pub author: Option<ConversationParticipantId>,
    pub speed: TimelineSpeed<'a>,
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
    pub memory_texts: Option<&'a str>,
    pub memory_summary: Option<&'a str>,
    pub memory_summary_token_count: u64,
    pub memory_tool_events: Option<&'a str>,
    pub settings: Option<lettuce_conversations::CurrentConversationSettings>,
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
    #[allow(clippy::too_many_arguments)]
    pub fn execute(
        &self,
        admission: &LegacyImportAdmission,
        plan: &LegacyImportPlan,
        sessions: &[LegacyBackupDirectSession],
        memories: &[LegacyBackupMemoryEmbeddingOwner],
        companions: &[LegacyBackupCompanionSharedMemory],
        scheduled_notes: &[LegacyBackupScheduledNote],
        completed_at: TimestampMillis,
    ) -> Result<LegacyImportStageReceipt, Error> {
        self.execute_with_effects(
            admission,
            plan,
            sessions,
            memories,
            companions,
            scheduled_notes,
            &[],
            completed_at,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_with_effects(
        &self,
        admission: &LegacyImportAdmission,
        plan: &LegacyImportPlan,
        sessions: &[LegacyBackupDirectSession],
        memories: &[LegacyBackupMemoryEmbeddingOwner],
        companions: &[LegacyBackupCompanionSharedMemory],
        scheduled_notes: &[LegacyBackupScheduledNote],
        preserved: &[lettuce_transfer::LegacyPreservedRow],
        completed_at: TimestampMillis,
    ) -> Result<LegacyImportStageReceipt, Error> {
        let plan_fingerprint = crate::legacy::legacy_import::plan_fingerprint(plan);
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
        let mut context = import_context(admission, plan, &source_fingerprint);
        context.dynamic_memory_enabled = lettuce_settings::GlobalSettingsStore::load(self.sources)
            .map_err(|_| Error::Storage)?
            .settings
            .dynamic_memory
            .enabled;
        let mut mapped = Vec::with_capacity(sessions.len());
        for session in sessions {
            let memory = memory_owner(
                memories,
                LegacyBackupMemoryOwnerKind::DirectConversation,
                &session.source_id,
            );
            mapped.push(self.map_session(session, memory, companions, &context)?);
        }
        let mut dynamic_companions = std::collections::BTreeSet::new();
        for (_, pending) in &mapped {
            if let Some(pending) = pending
                && self.dynamic_memory_active(pending.owner.character_id, &context)?
            {
                dynamic_companions.insert(pending.owner.character_id);
            }
        }
        attach_companion_pools(
            &mut mapped,
            sessions,
            memories,
            companions,
            &dynamic_companions,
            context.scope,
        )?;
        let mut conversations = assign_companion_episodes(mapped)?;
        attach_companion_effects(&mut conversations, sessions, preserved, context.scope);
        conversations.sort_by_key(|record| {
            let episode = record
                .companion
                .as_ref()
                .map(|companion| &companion.episode);
            (
                episode.map_or(
                    record.history.aggregate.conversation.created_at,
                    |episode| episode.started_at,
                ),
                episode.map_or(0, |episode| episode.episode_index),
                record.history.aggregate.conversation.id,
            )
        });
        let companion_characters = conversations
            .iter()
            .filter_map(|record| record.companion.as_ref())
            .map(|companion| companion.owner.character_id)
            .collect::<std::collections::BTreeSet<_>>();
        let mut companion_souls = Vec::new();
        let character = |id: CharacterId| CharacterId::from_uuid(context.scope.uuid(id.as_uuid()));
        for state in companions {
            if state.soul_materialization
                == LegacyBackupCompanionMaterialization::ExactInitialSnapshot
                && self.is_companion(character(state.character_id), &companion_characters)?
                && let Some(facts) = state.soul_facts.clone()
            {
                let pool = format!("companion-pool:{}", state.character_id);
                let facts = facts
                    .into_iter()
                    .map(|fact| lettuce_companions::SoulFact {
                        source_memory_ids: fact
                            .source_memory_ids
                            .iter()
                            .map(|id| memory_item_id(context.scope, &pool, id).to_string())
                            .collect(),
                        ..fact
                    })
                    .collect();
                companion_souls.push((character(state.character_id), facts));
            }
        }
        let mut notes = Vec::new();
        for note in scheduled_notes {
            let note = lettuce_companions::CompanionScheduledNote {
                id: context.scope.uuid(note.note.id),
                character_id: character(note.note.character_id),
                ..note.note.clone()
            };
            if note.clone().normalize().is_ok()
                && self.is_companion(note.character_id, &companion_characters)?
            {
                notes.push(note);
            }
        }
        let scheduled_notes = notes;
        self.sources.materialize_direct_conversations(
            LegacyDirectConversationMaterializationRequest {
                run_id: admission.run_id,
                plan_fingerprint,
                source_fingerprint,
                conversations,
                companion_souls,
                scheduled_notes,
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
        self.execute_with_effects(
            admission,
            &import.plan,
            &import.compatibility.direct_sessions().sessions,
            &import.compatibility.memory_embeddings().owners,
            &import.compatibility.memory_embeddings().source.states,
            &import.compatibility.memory_embeddings().source.source.notes,
            &import.preserved,
            completed_at,
        )
    }

    /// Whether legacy ran this character's chats on dynamic memory: the
    /// global switch and the character's dynamic memory type.
    fn dynamic_memory_active(
        &self,
        character_id: CharacterId,
        context: &ImportContext,
    ) -> Result<bool, Error> {
        Ok(context.dynamic_memory_enabled
            && lettuce_characters::CharacterRepository::get(self.sources, character_id)
                .map_err(|_| Error::Storage)?
                .is_some_and(|details| {
                    details.character.defaults.memory_policy
                        == lettuce_characters::MemoryPolicy::Dynamic
                }))
    }

    /// Soul facts and scheduled notes belong to companion characters the
    /// import wrote; a character imported in another interaction mode keeps none.
    fn is_companion(
        &self,
        character_id: CharacterId,
        imported_companions: &std::collections::BTreeSet<CharacterId>,
    ) -> Result<bool, Error> {
        if imported_companions.contains(&character_id) {
            return Ok(true);
        }
        Ok(
            lettuce_characters::CharacterRepository::get(self.sources, character_id)
                .map_err(|_| Error::Storage)?
                .is_some_and(|details| {
                    crate::launch::policy::is_companion(&details.character.defaults)
                }),
        )
    }

    /// A session imported as a companion chat (its character is a companion,
    /// so the import seeds companion state and every turn runs on the
    /// companion prompt chain) imports no prompt override, since legacy never
    /// consulted a companion session's prompt template; the value stays only
    /// in the legacy source. A session whose own mode alone was companion
    /// runs as a direct chat and keeps its override. Any other session whose
    /// template was empty or did not import pins no launch prompt, so it
    /// follows the live character like legacy.
    fn map_session(
        &self,
        session: &LegacyBackupDirectSession,
        memory: Option<&LegacyBackupMemoryEmbeddingOwner>,
        companions: &[LegacyBackupCompanionSharedMemory],
        context: &ImportContext,
    ) -> Result<(LegacyConversationRecord, Option<PendingCompanion>), Error> {
        let mut rows = session.messages.iter().collect::<Vec<_>>();
        rows.sort_by_key(|message| message.ordinal);
        let persona = persona_selection(
            session.persona_disabled,
            session.persona_source_id.as_deref(),
            context,
        )?;
        let scene = match &session.selected_scene_source_id {
            Some(id) => LaunchSelection::Explicit(SceneId::from_uuid(
                context.scope.uuid(parse::<SceneId>(id)?.as_uuid()),
            )),
            None => LaunchSelection::Disabled,
        };
        let legacy_character = parse::<CharacterId>(&session.character_source_id)?;
        let character_id = CharacterId::from_uuid(context.scope.uuid(legacy_character.as_uuid()));
        let title = if session.title.trim().is_empty() {
            lettuce_characters::CharacterRepository::get(self.sources, character_id)
                .map_err(|_| Error::Storage)?
                .map(|details| crate::launch::policy::character_display_name(&details.character))
                .ok_or(Error::InvalidInput)?
        } else {
            session.title.clone()
        };
        let memory_texts = shown_memory_texts(
            self.dynamic_memory_active(character_id, context)?,
            memory,
            &session.memories_json,
        );
        let request = DirectConversationLaunchRequest {
            format_version: DIRECT_LAUNCH_REQUEST_FORMAT_V1,
            title,
            user: legacy_user(),
            character_id,
            scene,
            starter: LaunchSelection::Disabled,
            persona,
            operation_key: launch_key(context.scope, &session.source_id)?,
        };
        let (prepared, launch_companion) = ConversationLaunchPlanner::new(self.sources)
            .prepare_direct_parts(&request)
            .map_err(|_| Error::Conflict)?;
        let (plan, mut snapshots) = prepared.into_parts();
        let conversation_id = ConversationId::from_uuid(context.scope.source(&session.source_id));
        let companion_time_awareness = launch_companion
            .as_ref()
            .is_some_and(|(_, _, time_awareness)| *time_awareness);
        let companion_session = launch_companion.is_some();
        let companion = launch_companion.map(|(owner, initial, _)| {
            let owner = CompanionStateOwner {
                conversation_id,
                ..owner
            };
            let shared = companions
                .iter()
                .find(|state| state.character_id == legacy_character);
            let mut initial = session
                .companion_state_json
                .as_deref()
                .and_then(|json| legacy_companion_state(json, &initial))
                .unwrap_or(initial);
            if let Some(relationship) = shared.and_then(|state| {
                state.relationship_states.iter().find(|relationship| {
                    relationship.persona_id.map(|id| id.to_string()).as_deref()
                        == session.persona_source_id.as_deref()
                        && relationship.materialization
                            == LegacyBackupCompanionMaterialization::ExactInitialSnapshot
                })
            }) {
                initial.relationship_state = relationship.state.clone();
            }
            let legacy_episode = shared
                .and_then(|state| {
                    state
                        .episodes
                        .iter()
                        .find(|episode| episode.conversation_source_id == session.source_id)
                })
                .map(|episode| {
                    Ok::<_, Error>(LegacyCompanionEpisodeRecord {
                        episode_index: episode.episode_index,
                        previous_conversation_id: None,
                        started_at: timestamp(episode.started_at)?,
                        ended_at: episode.ended_at.map(timestamp).transpose()?,
                        updated_at: timestamp(episode.updated_at)?,
                    })
                })
                .transpose()?;
            Ok::<_, Error>(PendingCompanion {
                owner,
                initial,
                legacy_episode,
                legacy_character_id: legacy_character,
                created_at: timestamp(session.created_at)?,
            })
        });
        let companion = companion.transpose()?;
        let character = plan
            .participants
            .iter()
            .find(|participant| participant.role == ParticipantRole::Character)
            .map(|participant| participant.id)
            .ok_or(Error::InvalidInput)?;
        let ConversationKind::Direct(details) = &plan.kind else {
            return Err(Error::InvalidInput);
        };
        let (settings, settings_snapshots) = session_settings(
            self.sources,
            &session.source_id,
            context,
            SessionSettingsSource {
                author_note: session.author_note.as_deref(),
                prompt_source_id: session
                    .prompt_source_id
                    .as_deref()
                    .filter(|_| !companion_session),
                prompt_purposes: &crate::launch::policy::DIRECT_SELECTION_PURPOSES,
                prompt_snapshot_purpose: lettuce_conversations::PromptPurposeSnapshot::Direct,
                lorebook_source_ids: session.lorebook_source_ids_override.as_deref(),
                speaker_selection: None,
                persona_disabled: session.persona_disabled,
                chat_mode: None,
                disable_character_lorebooks: None,
                model_settings: &session.generation_settings.model_settings,
                background: context.background(
                    &session.source_id,
                    false,
                    lettuce_transfer::legacy_direct_session_background(
                        session.background_image_locator.as_deref(),
                    )
                    .as_deref(),
                ),
                companion_clock: session
                    .companion_state_json
                    .as_deref()
                    .and_then(legacy_companion_clock)
                    .or_else(|| {
                        companion_time_awareness.then(|| {
                            lettuce_conversations::CompanionClockSettings {
                                time_awareness_enabled: true,
                                ..Default::default()
                            }
                        })
                    }),
            },
        )?;
        snapshots.extend(settings_snapshots);
        let mut kind = plan.kind.clone();
        if !companion_session
            && settings
                .as_ref()
                .is_none_or(|settings| settings.prompt.is_none())
        {
            unpin_launch_prompt(&mut kind, &mut snapshots);
        }
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
                attachments_json: &row.attachments_json,
                speed: TimelineSpeed {
                    first_token_ms: row.usage.first_token_ms,
                    tokens_per_second: row.usage.tokens_per_second,
                    mtp_stats_json: row.usage.mtp_stats_json.as_deref(),
                },
                variants: row
                    .variants
                    .iter()
                    .map(|variant| TimelineVariant {
                        source_id: &variant.source_id,
                        content: &variant.content,
                        created_at: variant.created_at,
                        prompt_tokens: variant.usage.prompt_tokens,
                        completion_tokens: variant.usage.completion_tokens,
                        total_tokens: variant.usage.total_tokens,
                        reasoning: variant.reasoning.as_deref(),
                        attachments_json: None,
                        author: Some(character),
                        speed: TimelineSpeed {
                            first_token_ms: variant.usage.first_token_ms,
                            tokens_per_second: variant.usage.tokens_per_second,
                            mtp_stats_json: variant.usage.mtp_stats_json.as_deref(),
                        },
                    })
                    .collect(),
            })
            .collect();
        conversation_record(
            LegacyConversationSource {
                source_id: &session.source_id,
                title: plan.title.clone(),
                kind,
                participants: plan.participants.clone(),
                initial_timeline: &plan.initial_timeline.entries,
                snapshots,
                model,
                archived: session.archived,
                created_at: session.created_at,
                updated_at: session.updated_at,
                messages,
                memory,
                memory_texts,
                memory_summary: session.memory_summary.as_deref(),
                memory_summary_token_count: session.memory_summary_token_count,
                memory_tool_events: Some(session.memory_tool_events_json.as_str()),
                settings,
            },
            context,
        )
        .map(|record| (record, companion))
    }
}

/// Drops the launch prompt of an imported direct session that carries no
/// session template of its own, so its turns follow the live character like
/// legacy's null `session.prompt_template_id` instead of reading the launch
/// prompt as a pin; the launch prompt's snapshot draft goes with it.
fn unpin_launch_prompt(kind: &mut ConversationKind, snapshots: &mut Vec<SnapshotArtifactDraft>) {
    let ConversationKind::Direct(details) = kind else {
        return;
    };
    if let SnapshotSelection::Inherited(prompt) | SnapshotSelection::Explicit(prompt) =
        &details.prompt
    {
        let artifact_id = prompt.snapshot_ref.artifact_id;
        snapshots.retain(|draft| draft.artifact_id != artifact_id);
    }
    details.prompt = SnapshotSelection::Disabled;
}

/// A companion session's state before its continuity episode is placed in the
/// character's chain.
struct PendingCompanion {
    owner: CompanionStateOwner,
    initial: CompanionRuntimeState,
    legacy_episode: Option<LegacyCompanionEpisodeRecord>,
    legacy_character_id: CharacterId,
    created_at: TimestampMillis,
}

/// Places every companion conversation in its character and persona chain by
/// start time. Legacy created episodes lazily and keyed sessions without a
/// persona apart from the default persona they launch with, so indexes are
/// renumbered along the merged chain; a legacy episode keeps its timestamps, a
/// session legacy never recorded starts at its creation and ends where the next
/// one starts. The latest conversation's relationship is the chain's.
fn assign_companion_episodes(
    mapped: Vec<(LegacyConversationRecord, Option<PendingCompanion>)>,
) -> Result<Vec<LegacyConversationRecord>, Error> {
    let start = |pending: &PendingCompanion| {
        pending
            .legacy_episode
            .as_ref()
            .map_or(pending.created_at, |episode| episode.started_at)
    };
    type Chains<'a> =
        BTreeMap<(CharacterId, Option<PersonaId>), Vec<(usize, &'a PendingCompanion)>>;
    let mut chains = Chains::new();
    for (index, (_, pending)) in mapped.iter().enumerate() {
        if let Some(pending) = pending {
            chains
                .entry((pending.owner.character_id, pending.owner.persona_id))
                .or_default()
                .push((index, pending));
        }
    }
    let mut assigned = BTreeMap::new();
    for chain in chains.values_mut() {
        chain.sort_by_key(|(_, pending)| {
            (
                start(pending),
                pending
                    .legacy_episode
                    .as_ref()
                    .map_or(u32::MAX, |episode| episode.episode_index),
                pending.owner.conversation_id,
            )
        });
        let relationship = chain
            .last()
            .map(|(_, pending)| pending.initial.relationship_state.clone());
        for (position, (index, pending)) in chain.iter().enumerate() {
            let episode_index = u32::try_from(position + 1).map_err(|_| Error::InvalidInput)?;
            let previous_conversation_id = position
                .checked_sub(1)
                .map(|previous| chain[previous].1.owner.conversation_id);
            let episode = match &pending.legacy_episode {
                Some(legacy) => LegacyCompanionEpisodeRecord {
                    episode_index,
                    previous_conversation_id,
                    ..legacy.clone()
                },
                None => {
                    let ended_at = chain.get(position + 1).map(|(_, next)| start(next));
                    LegacyCompanionEpisodeRecord {
                        episode_index,
                        previous_conversation_id,
                        started_at: pending.created_at,
                        ended_at,
                        updated_at: ended_at.unwrap_or(pending.created_at),
                    }
                }
            };
            let mut initial = pending.initial.clone();
            if let Some(relationship) = &relationship {
                initial.relationship_state = relationship.clone();
            }
            assigned.insert(
                *index,
                LegacyCompanionConversation {
                    owner: pending.owner,
                    initial,
                    episode,
                },
            );
        }
    }
    Ok(mapped
        .into_iter()
        .enumerate()
        .map(|(index, (mut record, _))| {
            record.companion = assigned.remove(&index);
            record
        })
        .collect())
}

/// A companion character's conversations belong to one memory pool and keep
/// their own memories beside it; the share-memory toggle picks which one they
/// use, as legacy's live owner resolution did. The pool takes the legacy
/// shared memory when legacy kept one, otherwise a copy of the memories of
/// the character's most recently updated session that has any; its summary
/// window belongs to that session.
fn attach_companion_pools(
    mapped: &mut [(LegacyConversationRecord, Option<PendingCompanion>)],
    sessions: &[LegacyBackupDirectSession],
    memories: &[LegacyBackupMemoryEmbeddingOwner],
    companions: &[LegacyBackupCompanionSharedMemory],
    dynamic_companions: &std::collections::BTreeSet<CharacterId>,
    scope: LegacyIdScope,
) -> Result<(), Error> {
    let character_of = |pending: &Option<PendingCompanion>| {
        pending.as_ref().map(|pending| pending.owner.character_id)
    };
    let characters = mapped
        .iter()
        .filter_map(|(_, pending)| {
            pending
                .as_ref()
                .map(|pending| (pending.owner.character_id, pending.legacy_character_id))
        })
        .collect::<std::collections::BTreeSet<_>>();
    for (character_id, legacy_character_id) in characters {
        let mut candidates = mapped
            .iter()
            .enumerate()
            .filter(|(_, (_, pending))| character_of(pending) == Some(character_id))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        candidates.sort_by_key(|index| std::cmp::Reverse((sessions[*index].updated_at, *index)));
        let shared_state = companions
            .iter()
            .find(|state| state.character_id == legacy_character_id);
        let shared_owner = memory_owner(
            memories,
            LegacyBackupMemoryOwnerKind::CompanionShared,
            &legacy_character_id.to_string(),
        );
        let dynamic = dynamic_companions.contains(&character_id);
        let shared_texts = shared_state
            .and_then(|state| shown_memory_texts(dynamic, shared_owner, &state.memories_json));
        let shared = shared_owner.is_some_and(|owner| {
            owner.memories.iter().any(|memory| {
                memory.materialization != LegacyBackupMemoryMaterialization::RetainedEvidence
            })
        }) || !unembedded_memory_texts(shared_owner, shared_texts).is_empty();
        let mut pool = None;
        for carrier in candidates {
            let session = &sessions[carrier];
            let (owner, texts, summary, summary_token_count, tool_events) = if shared {
                (
                    shared_owner,
                    shared_texts,
                    shared_state.and_then(|state| state.memory_summary.as_deref()),
                    shared_state.map_or(0, |state| state.memory_summary_token_count),
                    shared_state.map(|state| state.memory_tool_events_json.as_str()),
                )
            } else {
                let owner = memory_owner(
                    memories,
                    LegacyBackupMemoryOwnerKind::DirectConversation,
                    &session.source_id,
                );
                (
                    owner,
                    shown_memory_texts(dynamic, owner, &session.memories_json),
                    session.memory_summary.as_deref(),
                    session.memory_summary_token_count,
                    Some(session.memory_tool_events_json.as_str()),
                )
            };
            let record = &mapped[carrier].0;
            let (space, projections) = memory_space(
                scope,
                record.history.aggregate.conversation.id,
                &format!("companion-pool:{legacy_character_id}"),
                owner,
                texts,
                summary,
                summary_token_count,
                tool_events,
                &record.history.messages,
                record.history.aggregate.conversation.updated_at,
            )?;
            if let Some(space) = space {
                pool = Some((space, projections));
                break;
            }
        }
        let Some((mut space, projections)) = pool else {
            continue;
        };
        space.shared_conversation_ids = mapped
            .iter()
            .filter(|(record, pending)| {
                character_of(pending) == Some(character_id)
                    && record.history.aggregate.conversation.id != space.conversation_id
            })
            .map(|(record, _)| record.history.aggregate.conversation.id)
            .collect();
        for (record, pending) in mapped.iter_mut() {
            if character_of(pending) == Some(character_id) {
                record.pool = Some(space.clone());
                record
                    .memory_projections
                    .extend(projections.iter().cloned());
            }
        }
    }
    Ok(())
}

/// Legacy `companion_time_awareness_enabled` / `companion_effective_now` over
/// the saved `companionState`: its `preferences` switch (off when absent) and
/// time override, where an override missing its anchor ran on real time. A
/// session without saved state takes the companion's default, which legacy
/// seeded on its next save.
fn legacy_companion_clock(json: &str) -> Option<lettuce_conversations::CompanionClockSettings> {
    use lettuce_conversations::{CompanionClockSettings, CompanionTimeOverride};
    let state = serde_json::from_str::<serde_json::Value>(json).ok()?;
    if state.is_null() {
        return None;
    }
    let preferences = state.get("preferences");
    let time_awareness_enabled = preferences
        .and_then(|value| {
            value
                .get("timeAwarenessEnabled")
                .or_else(|| value.get("time_awareness_enabled"))
        })
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let time_override = preferences.and_then(|value| value.get("timeOverride"));
    let millis = |key: &str| {
        time_override
            .and_then(|value| value.get(key))
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| i64::try_from(value).ok())
            .map(TimestampMillis::new)
    };
    let time_override = match time_override
        .and_then(|value| value.get("mode"))
        .and_then(serde_json::Value::as_str)
    {
        Some("frozen") => millis("anchorMs").map_or(CompanionTimeOverride::Live, |anchor_at| {
            CompanionTimeOverride::Frozen { anchor_at }
        }),
        Some("ticking") => match (millis("anchorMs"), millis("setAtMs")) {
            (Some(anchor_at), Some(set_at)) => CompanionTimeOverride::Ticking { anchor_at, set_at },
            _ => CompanionTimeOverride::Live,
        },
        _ => CompanionTimeOverride::Live,
    };
    Some(CompanionClockSettings {
        time_awareness_enabled,
        time_override,
    })
}

/// Legacy stored a companion session's runtime state as camelCase JSON; values
/// are clamped into the rewrite's ranges and an unreadable state keeps the
/// launch initial state.
fn legacy_companion_state(
    json: &str,
    initial: &CompanionRuntimeState,
) -> Option<CompanionRuntimeState> {
    #[derive(serde::Deserialize, Default)]
    #[serde(rename_all = "camelCase", default)]
    struct Vector {
        warmth: f64,
        trust: f64,
        calm: f64,
        vulnerability: f64,
        longing: f64,
        hurt: f64,
        tension: f64,
        irritation: f64,
        affection_intensity: f64,
        reassurance_need: f64,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Emotional {
        #[serde(default)]
        felt: Vector,
        #[serde(default)]
        expressed: Vector,
        #[serde(default)]
        blocked: Vector,
        #[serde(default)]
        momentum: Vector,
        #[serde(default)]
        active_drivers: Vec<String>,
        confidence: f64,
        #[serde(default)]
        updated_at: u64,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Relationship {
        closeness: f64,
        trust: f64,
        affection: f64,
        tension: f64,
        stability: f64,
        #[serde(default)]
        interaction_count: u32,
        #[serde(default)]
        last_interaction_at: u64,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct State {
        emotional_state: Emotional,
        relationship_state: Relationship,
        #[serde(default)]
        active_signals: Vec<String>,
        #[serde(default)]
        updated_at: u64,
    }
    let vector = |value: &Vector| EmotionVector {
        warmth: value.warmth,
        trust: value.trust,
        calm: value.calm,
        vulnerability: value.vulnerability,
        longing: value.longing,
        hurt: value.hurt,
        tension: value.tension,
        irritation: value.irritation,
        affection_intensity: value.affection_intensity,
        reassurance_need: value.reassurance_need,
    };
    let finite = |value: f64, low: f64, high: f64, fallback: f64| {
        if value.is_finite() {
            value.clamp(low, high)
        } else {
            fallback
        }
    };
    let legacy = serde_json::from_str::<State>(json).ok()?;
    let updated_at = i64::try_from(legacy.updated_at).ok()?;
    let emotional_updated_at = i64::try_from(legacy.emotional_state.updated_at)
        .ok()?
        .min(updated_at);
    let text = |values: Vec<String>| {
        values
            .into_iter()
            .filter(|value| !value.trim().is_empty())
            .collect::<Vec<_>>()
    };
    Some(CompanionRuntimeState {
        emotional_state: EmotionalState {
            felt: vector(&legacy.emotional_state.felt).clamp(),
            expressed: vector(&legacy.emotional_state.expressed).clamp(),
            blocked: vector(&legacy.emotional_state.blocked).clamp(),
            momentum: vector(&legacy.emotional_state.momentum).clamp_signed(),
            active_drivers: text(legacy.emotional_state.active_drivers),
            confidence: finite(legacy.emotional_state.confidence, 0.0, 1.0, 0.5),
            updated_at: TimestampMillis::new(emotional_updated_at),
        },
        relationship_state: RelationshipState {
            closeness: finite(
                legacy.relationship_state.closeness,
                -1.0,
                1.0,
                initial.relationship_state.closeness,
            ),
            trust: finite(
                legacy.relationship_state.trust,
                -1.0,
                1.0,
                initial.relationship_state.trust,
            ),
            affection: finite(
                legacy.relationship_state.affection,
                -1.0,
                1.0,
                initial.relationship_state.affection,
            ),
            tension: finite(
                legacy.relationship_state.tension,
                0.0,
                1.0,
                initial.relationship_state.tension,
            ),
            stability: finite(
                legacy.relationship_state.stability,
                0.0,
                1.0,
                initial.relationship_state.stability,
            ),
            interaction_count: legacy.relationship_state.interaction_count,
            last_interaction_at: TimestampMillis::new(
                i64::try_from(legacy.relationship_state.last_interaction_at).ok()?,
            ),
        },
        active_signals: text(legacy.active_signals),
        updated_at: TimestampMillis::new(updated_at),
    })
}

/// A legacy session's author note, prompt and lorebook overrides and group
/// speaker selection as current conversation settings, snapshotting the
/// imported prompt and lorebooks they name. A prompt that is missing, archived
/// or of another purpose keeps the launch prompt, like legacy's fallback.
pub(crate) fn session_settings<S: DirectLaunchSources>(
    sources: &S,
    source_id: &str,
    context: &ImportContext,
    input: SessionSettingsSource<'_>,
) -> Result<
    (
        Option<lettuce_conversations::CurrentConversationSettings>,
        Vec<SnapshotArtifactDraft>,
    ),
    Error,
> {
    use lettuce_context::{LorebookRepository, PromptLookupResult, PromptRepository};
    use lettuce_conversations::{
        CurrentConversationSettings, LorebookLaunchSnapshot, PromptLaunchSnapshot,
        SettingProvenance,
    };
    use lettuce_types::SnapshotArtifactId;

    let provenance = |present: bool| {
        if present {
            SettingProvenance::CurrentOverride
        } else {
            SettingProvenance::LaunchInherited
        }
    };
    let mut drafts = Vec::new();
    let author_note = input
        .author_note
        .filter(|note| !note.trim().is_empty())
        .map(str::to_owned);
    let mut prompt = None;
    if let Some(destination) = input
        .prompt_source_id
        .and_then(|id| context.prompts.get(id))
    {
        for purpose in input.prompt_purposes {
            if let PromptLookupResult::Available { document } =
                PromptRepository::lookup_exact(sources, *destination, *purpose)
                    .map_err(|_| Error::Storage)?
            {
                let draft = crate::launch::documents::draft(
                    SnapshotArtifactId::from_uuid(
                        context.scope.derived(source_id, "settings:prompt"),
                    ),
                    document.revision,
                    crate::launch::documents::prompt_body(&document),
                )
                .map_err(|_| Error::InvalidInput)?;
                prompt = Some(PromptLaunchSnapshot {
                    snapshot_ref: draft.reference(),
                    source_id: document.id,
                    source_revision: document.revision,
                    title: document.name.clone(),
                    purpose: input.prompt_snapshot_purpose,
                });
                drafts.push(draft);
                break;
            }
        }
    }
    let lorebooks = input
        .lorebook_source_ids
        .map(|ids| {
            let mut books = Vec::new();
            for id in ids {
                let Some(destination) = id
                    .parse::<lettuce_types::LorebookId>()
                    .ok()
                    .and_then(|id| context.lorebooks.get(&id))
                else {
                    continue;
                };
                let Some(details) = LorebookRepository::get(sources, *destination)
                    .map_err(|_| Error::Storage)?
                    .filter(|details| {
                        details.book.status != lettuce_context::LifecycleStatus::Archived
                    })
                else {
                    continue;
                };
                if books
                    .iter()
                    .any(|book: &LorebookLaunchSnapshot| book.source_id == details.book.id)
                {
                    continue;
                }
                let draft = crate::launch::documents::draft(
                    SnapshotArtifactId::from_uuid(
                        context
                            .scope
                            .derived(source_id, &format!("settings:lorebook:{}", details.book.id)),
                    ),
                    details.book.revision,
                    crate::launch::documents::lorebook_body(&details),
                )
                .map_err(|_| Error::InvalidInput)?;
                books.push(LorebookLaunchSnapshot {
                    snapshot_ref: draft.reference(),
                    source_id: details.book.id,
                    source_revision: details.book.revision,
                    name: details.book.name.clone(),
                });
                drafts.push(draft);
            }
            Ok::<_, Error>(books)
        })
        .transpose()?;
    let lorebooks_provenance = match &lorebooks {
        Some(books) if books.is_empty() => SettingProvenance::Disabled,
        Some(_) => SettingProvenance::CurrentOverride,
        None => SettingProvenance::LaunchInherited,
    };
    let lorebooks = lorebooks.filter(|books| !books.is_empty());
    if author_note.is_none()
        && prompt.is_none()
        && lorebooks_provenance == SettingProvenance::LaunchInherited
        && input.speaker_selection.is_none()
        && input.model_settings.is_empty()
        && input.background.is_none()
        && input.companion_clock.is_none()
        && !input.persona_disabled
        && input.chat_mode.is_none()
        && input.disable_character_lorebooks.is_none()
    {
        return Ok((None, drafts));
    }
    Ok((
        Some(CurrentConversationSettings {
            companion_clock: input.companion_clock,
            background: input.background,
            model_settings: input.model_settings.clone(),
            revision: Revision::INITIAL,
            author_note_provenance: provenance(author_note.is_some()),
            author_note,
            memory: None,
            memory_provenance: SettingProvenance::LaunchInherited,
            model_override: None,
            model_provenance: SettingProvenance::LaunchInherited,
            voice: None,
            voice_provenance: SettingProvenance::LaunchInherited,
            prompt_provenance: provenance(prompt.is_some()),
            prompt,
            lorebooks_provenance,
            lorebooks,
            persona: None,
            persona_provenance: if input.persona_disabled {
                SettingProvenance::Disabled
            } else {
                SettingProvenance::LaunchInherited
            },
            scene: None,
            scene_provenance: SettingProvenance::LaunchInherited,
            speaker_selection_provenance: provenance(input.speaker_selection.is_some()),
            speaker_selection: input.speaker_selection,
            chat_mode: input.chat_mode,
            disable_character_lorebooks: input.disable_character_lorebooks,
        }),
        drafts,
    ))
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
#[allow(clippy::too_many_arguments)]
fn memory_space(
    scope: LegacyIdScope,
    conversation_id: ConversationId,
    source_id: &str,
    owner: Option<&LegacyBackupMemoryEmbeddingOwner>,
    texts: Option<&str>,
    summary: Option<&str>,
    summary_token_count: u64,
    tool_events: Option<&str>,
    messages: &[BackupMessage],
    updated_at: TimestampMillis,
) -> Result<(Option<BackupMemorySpace>, Vec<BackupMemoryProjection>), Error> {
    let space_id = MemorySpaceId::from_uuid(scope.derived(source_id, "memory"));
    let mut items: Vec<MemoryItem> = Vec::new();
    let mut projections = Vec::new();
    for memory in owner.into_iter().flat_map(|owner| &owner.memories) {
        if memory.materialization == LegacyBackupMemoryMaterialization::RetainedEvidence {
            continue;
        }
        let id = memory_item_id(scope, source_id, &memory.id);
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
                .map(|value| MessageId::from_uuid(scope.source(value))),
            source_role: match memory.source_role.as_deref() {
                Some("user") => Some(MessageRole::User),
                Some("assistant") => Some(MessageRole::Assistant),
                _ => None,
            },
            observed_at: memory.observed_at.map(timestamp).transpose()?,
            observed_time_precision: memory.observed_time_precision.clone(),
            superseded_by: memory
                .superseded_by
                .as_deref()
                .map(|value| memory_item_id(scope, source_id, value)),
            superseded_at: memory.superseded_at.map(timestamp).transpose()?,
            supersedes: memory
                .supersedes
                .iter()
                .map(|value| memory_item_id(scope, source_id, value))
                .collect(),
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
    for (index, text) in unembedded_memory_texts(owner, texts) {
        let id = memory_item_id(scope, source_id, &format!("text:{index}"));
        let short_id = MemoryShortId::allocate(id, |candidate| {
            items.iter().any(|item| item.short_id == candidate)
        });
        items.push(MemoryItem::written(id, short_id, text, updated_at));
    }
    let in_dialogue = |message: &&BackupMessage| {
        message.message.visibility == MessageVisibility::Visible
            && matches!(
                message.message.role,
                MessageRole::User | MessageRole::Assistant
            )
    };
    let dialogue = messages
        .iter()
        .filter(in_dialogue)
        .map(|message| message.message.id)
        .collect::<Vec<_>>();
    let summary = summary
        .filter(|text| !text.trim().is_empty() && !dialogue.is_empty())
        .map(|text| {
            let anchor = legacy_summary_anchor(scope, tool_events, messages);
            let end = messages
                .iter()
                .position(|message| Some(message.message.id) == anchor)
                .map_or(0, |anchor| {
                    messages[..=anchor].iter().filter(in_dialogue).count()
                })
                .clamp(1, dialogue.len());
            Ok::<_, Error>(MemorySummary {
                space_id,
                text: text.to_owned(),
                token_count: u32::try_from(summary_token_count).unwrap_or(u32::MAX),
                window_start: 0,
                window_end: u64::try_from(end).map_err(|_| Error::InvalidInput)?,
                source_message_ids: dialogue[..end].to_vec(),
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
            shared_conversation_ids: Vec::new(),
        }),
        projections,
    ))
}

/// Legacy `resolve_last_valid_window_end`: the last message of the newest
/// memory cycle that still advances the cursor (not reverted, not an error or
/// user edit) and whose anchor is a user or assistant message of this
/// conversation.
fn legacy_summary_anchor(
    scope: LegacyIdScope,
    tool_events: Option<&str>,
    messages: &[BackupMessage],
) -> Option<MessageId> {
    use serde_json::Value;
    let events = serde_json::from_str::<Vec<Value>>(tool_events?).ok()?;
    events.iter().rev().find_map(|event| {
        if event.get("revertedAt").and_then(Value::as_u64).is_some()
            || matches!(
                event.get("status").and_then(Value::as_str),
                Some("error" | "user_edit")
            )
        {
            return None;
        }
        let anchor = MessageId::from_uuid(
            scope.source(
                event
                    .get("windowMessageIds")?
                    .as_array()?
                    .last()?
                    .as_str()?,
            ),
        );
        messages
            .iter()
            .any(|message| {
                message.message.id == anchor
                    && matches!(
                        message.message.role,
                        MessageRole::User | MessageRole::Assistant
                    )
            })
            .then_some(anchor)
    })
}

/// The memory texts a chat showed: all of them on manual memory, and on
/// dynamic memory only while it had no embeddings.
pub(crate) fn shown_memory_texts<'a>(
    dynamic: bool,
    owner: Option<&LegacyBackupMemoryEmbeddingOwner>,
    texts: &'a str,
) -> Option<&'a str> {
    (!dynamic || owner.is_none_or(|owner| owner.memories.is_empty())).then_some(texts)
}

/// The owner's memory texts that none of its embeddings carries, with their
/// index in the text list.
fn unembedded_memory_texts(
    owner: Option<&LegacyBackupMemoryEmbeddingOwner>,
    texts: Option<&str>,
) -> Vec<(usize, String)> {
    let mut embedded = owner
        .into_iter()
        .flat_map(|owner| &owner.memories)
        .map(|memory| memory.text.trim())
        .collect::<Vec<_>>();
    let texts = texts
        .and_then(|texts| serde_json::from_str::<Vec<String>>(texts).ok())
        .unwrap_or_default();
    let mut unembedded = Vec::new();
    for (index, text) in texts.iter().enumerate() {
        let text = text.trim();
        if text.is_empty() || text.len() > lettuce_memory::MAX_MEMORY_TEXT_BYTES {
            continue;
        }
        match embedded.iter().position(|embedded| *embedded == text) {
            Some(position) => {
                embedded.swap_remove(position);
            }
            None => unembedded.push((index, text.to_owned())),
        }
    }
    unembedded
}

/// Turns legacy `companion_turn_effects` rows into effect records on the
/// imported conversations. A row becomes a record only when its assistant
/// message was imported with a generation turn (the newest one, which is the
/// generation the row last described) and its deltas are valid; memory
/// changes keep the memories the import wrote. Every row stays verbatim in
/// the run's provenance either way.
fn attach_companion_effects(
    conversations: &mut [LegacyConversationRecord],
    sessions: &[LegacyBackupDirectSession],
    preserved: &[lettuce_transfer::LegacyPreservedRow],
    scope: LegacyIdScope,
) {
    for row in preserved
        .iter()
        .filter(|row| row.source_table == "companion_turn_effects")
    {
        let Ok(serde_json::Value::Object(row)) = serde_json::from_str(&row.row_json) else {
            continue;
        };
        let text = |column: &str| row.get(column).and_then(serde_json::Value::as_str);
        let Some(session) = text("session_id")
            .and_then(|id| sessions.iter().find(|session| session.source_id == id))
        else {
            continue;
        };
        let conversation_id = ConversationId::from_uuid(scope.source(&session.source_id));
        let Some(record) = conversations
            .iter_mut()
            .find(|record| record.history.aggregate.conversation.id == conversation_id)
        else {
            continue;
        };
        if let Some(effect) = legacy_companion_effect(&row, record, session, scope) {
            record.companion_effects.push(effect);
        }
    }
}

fn legacy_companion_effect(
    row: &serde_json::Map<String, serde_json::Value>,
    record: &LegacyConversationRecord,
    session: &LegacyBackupDirectSession,
    scope: LegacyIdScope,
) -> Option<lettuce_companions::CompanionTurnEffect> {
    use lettuce_companions::{
        CompanionEffectSourceWindow, CompanionEmotionDelta, CompanionMemoryChanges,
        CompanionSignalChanges, CompanionTurnEffect, CompanionTurnEffectSeed,
        CompanionTurnEffectStatus, RelationshipDelta,
    };
    let text = |column: &str| row.get(column).and_then(serde_json::Value::as_str);
    let json = |column: &str| {
        text(column).and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
    };
    let integer = |column: &str| row.get(column).and_then(serde_json::Value::as_i64);
    let conversation_id = record.history.aggregate.conversation.id;
    let imported = |legacy: &str| {
        let id = MessageId::from_uuid(scope.source(legacy));
        record
            .history
            .messages
            .iter()
            .any(|message| message.message.id == id)
            .then_some(id)
    };
    let assistant_message_id = imported(text("assistant_message_id")?)?;
    let turn = record
        .turns
        .iter()
        .filter(|turn| match turn.target {
            GenerationTarget::NewAssistant { message_id, .. }
            | GenerationTarget::ExistingCandidate { message_id, .. } => {
                message_id == assistant_message_id
            }
        })
        .max_by_key(|turn| (turn.created_at, turn.id))?;
    let status = match text("status")? {
        "processing" => CompanionTurnEffectStatus::Processing,
        "ready" => CompanionTurnEffectStatus::Ready,
        "failed" => CompanionTurnEffectStatus::Failed,
        _ => return None,
    };
    let seed = CompanionTurnEffectSeed {
        relationship_delta: serde_json::from_value::<RelationshipDelta>(json(
            "relationship_delta",
        )?)
        .ok()?,
        emotion_delta: serde_json::from_value::<CompanionEmotionDelta>(json("emotion_delta")?)
            .ok()?,
        signal_changes: serde_json::from_value::<CompanionSignalChanges>(json("signal_changes")?)
            .ok()?,
    };
    seed.validate().ok()?;
    let memory_ids = record
        .memory
        .iter()
        .chain(&record.pool)
        .flat_map(|space| space.snapshot.items.iter().map(|item| item.id))
        .collect::<std::collections::BTreeSet<_>>();
    let pool = format!("companion-pool:{}", session.character_source_id);
    let memory_changes = json("memory_changes")?;
    let changed = |kind: &str| {
        memory_changes
            .get(kind)
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|item| item.get("memoryId").and_then(serde_json::Value::as_str))
            .filter_map(|legacy| {
                [session.source_id.as_str(), pool.as_str()]
                    .into_iter()
                    .map(|source| memory_item_id(scope, source, legacy))
                    .find(|id| memory_ids.contains(id))
            })
            .collect::<Vec<_>>()
    };
    let window = json("source_window")?;
    let source_window = window
        .get("enqueuedAt")
        .and_then(serde_json::Value::as_i64)
        .map(|enqueued_at| CompanionEffectSourceWindow {
            message_ids: window
                .get("messageIds")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str)
                .filter_map(imported)
                .collect(),
            enqueued_at: TimestampMillis::new(enqueued_at),
        });
    if status == CompanionTurnEffectStatus::Ready && source_window.is_none() {
        return None;
    }
    let summary = text("summary").map(str::to_owned);
    if summary.as_ref().is_some_and(|value| value.len() > 8 * 1024) {
        return None;
    }
    let created_at = integer("created_at")?;
    let updated_at = integer("updated_at")?.max(created_at);
    Some(CompanionTurnEffect {
        id: lettuce_types::CompanionEffectId::from_uuid(scope.source(text("id")?)),
        conversation_id,
        turn_id: turn.id,
        user_message_id: text("user_message_id").and_then(imported),
        assistant_message_id,
        status,
        summary,
        seed,
        memory_changes: CompanionMemoryChanges {
            added: changed("added"),
            updated: changed("updated"),
            superseded: changed("superseded"),
        },
        source_window,
        created_at: TimestampMillis::new(created_at),
        updated_at: TimestampMillis::new(updated_at),
    })
}

/// Legacy branch sessions copied their parent's memories with the same ids,
/// and memory ids are unique across spaces, so each owner derives its own.
fn memory_item_id(scope: LegacyIdScope, source_id: &str, legacy_id: &str) -> MemoryId {
    MemoryId::from_uuid(scope.derived(source_id, &format!("memory-item:{legacy_id}")))
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

pub(crate) fn launch_key(scope: LegacyIdScope, source_id: &str) -> Result<IdempotencyKey, Error> {
    IdempotencyKey::new(format!("legacy-import.{}", scope.source(source_id)))
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
    let conversation_id = ConversationId::from_uuid(context.scope.source(source.source_id));
    let branch_id =
        ConversationBranchId::from_uuid(context.scope.derived(source.source_id, "branch"));
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
        generation_stats: Vec::new(),
        unlinked_stats: 0,
    };
    let mut parent: Option<(MessageId, MessageRole)> = None;
    for (index, message) in source.messages.iter().enumerate() {
        let origin = (index == 0 && opens)
            .then(|| scene_origin.clone())
            .flatten();
        parent = Some(writer.push(message, parent, origin)?);
    }
    if writer.unlinked_stats > 0 {
        tracing::warn!(
            conversation_id = %conversation_id,
            skipped = writer.unlinked_stats,
            "legacy speed stats of replies imported without a generation were not kept"
        );
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
        current_settings: source.settings,
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
        context.scope,
        conversation_id,
        source.source_id,
        source.memory,
        source.memory_texts,
        source.memory_summary,
        source.memory_summary_token_count,
        source.memory_tool_events,
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
        generation_stats: writer.generation_stats,
        snapshots: source.snapshots,
        memory,
        pool: None,
        memory_projections,
        companion: None,
        companion_effects: Vec::new(),
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
    generation_stats: Vec<lettuce_transfer::LegacyGenerationStats>,
    unlinked_stats: usize,
}

impl SessionWriter<'_> {
    /// A variant's own attachments (group chats) plus, for the variant legacy
    /// rendered, the message's attachments.
    fn variant_attachments(
        &self,
        legacy: &TimelineMessage<'_>,
        variant: &TimelineVariant<'_>,
        active: bool,
    ) -> Vec<MessagePart> {
        let mut columns = Vec::with_capacity(2);
        columns.extend(variant.attachments_json);
        if active {
            columns.push(legacy.attachments_json);
        }
        self.context.attachment_parts(&columns)
    }

    fn push(
        &mut self,
        legacy: &TimelineMessage<'_>,
        parent: Option<(MessageId, MessageRole)>,
        origin: Option<InitialMessageOrigin>,
    ) -> Result<(MessageId, MessageRole), Error> {
        let message_id = MessageId::from_uuid(self.context.scope.source(legacy.source_id));
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
        if !variants_as_candidates {
            self.unlinked_stats += legacy
                .variants
                .iter()
                .enumerate()
                .filter(|(index, variant)| !variant_speed(legacy, variant, *index).is_empty())
                .count()
                .max(usize::from(
                    legacy.variants.is_empty() && !legacy.speed.is_empty(),
                ));
        }
        let active_render_source = if variants_as_candidates {
            self.push_candidates(legacy, message_id, parent, &mut candidates)?
        } else if legacy.variants.is_empty() {
            let revision = MessageRevision {
                id: MessageRevisionId::from_uuid(
                    self.context.scope.derived(legacy.source_id, "revision"),
                ),
                message_id,
                sequence: Revision::INITIAL,
                parts: parts(
                    legacy.content,
                    legacy.reasoning,
                    self.context.attachment_parts(&[legacy.attachments_json]),
                ),
                authored_at: created_at,
                source_turn_id: None,
                provider_replay: None,
                supersedes_candidate_id: None,
            };
            let active = MessageRenderSource::Revision(revision.id);
            revisions.push(revision);
            active
        } else {
            let mut active = None;
            for (index, variant) in legacy.variants.iter().enumerate() {
                let revision = MessageRevision {
                    id: MessageRevisionId::from_uuid(
                        self.context.scope.derived(variant.source_id, "revision"),
                    ),
                    message_id,
                    sequence: Revision::new(
                        u64::try_from(index + 1).map_err(|_| Error::InvalidInput)?,
                    ),
                    parts: parts(
                        variant.content,
                        variant.reasoning.or((index == active_variant_index(legacy))
                            .then_some(legacy.reasoning)
                            .flatten()),
                        self.variant_attachments(
                            legacy,
                            variant,
                            index == active_variant_index(legacy),
                        ),
                    ),
                    authored_at: timestamp(variant.created_at)?,
                    source_turn_id: None,
                    provider_replay: None,
                    supersedes_candidate_id: None,
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
            historical_media_revision_ids: revisions
                .iter()
                .filter(|revision| {
                    active_render_source != MessageRenderSource::Revision(revision.id)
                        && has_media(&revision.parts)
                })
                .map(|revision| revision.id)
                .collect(),
            historical_media_candidate_ids: candidates
                .iter()
                .filter(|candidate| {
                    active_render_source != MessageRenderSource::Candidate(candidate.id)
                        && has_media(&candidate.parts)
                })
                .map(|candidate| candidate.id)
                .collect(),
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
            let candidate_id =
                MessageCandidateId::from_uuid(self.context.scope.source(variant.source_id));
            let turn_id =
                GenerationTurnId::from_uuid(self.context.scope.derived(variant.source_id, "turn"));
            let attempt_id = GenerationAttemptId::from_uuid(
                self.context.scope.derived(variant.source_id, "attempt"),
            );
            let usage_event_id =
                UsageEventId::from_uuid(self.context.scope.derived(variant.source_id, "usage"));
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
                    image_tokens: None,
                    audio_tokens: None,
                    total_tokens: variant.total_tokens,
                    input_tokens,
                    output_tokens,
                }),
                _ => UsageCounters::Unavailable(UsageUnavailableReason::ProviderOmitted),
            };
            let speed = variant_speed(legacy, variant, index);
            if !speed.is_empty() {
                self.generation_stats
                    .push(lettuce_transfer::LegacyGenerationStats {
                        attempt_id,
                        created_at: at,
                        summary: speed.summary(variant),
                    });
            }
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
                    self.variant_attachments(legacy, variant, index == active_index),
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

/// A variant's own speed stats; the rendered variant without any falls back
/// to the message row's, which are the rendered variant's.
fn variant_speed<'a>(
    legacy: &TimelineMessage<'a>,
    variant: &TimelineVariant<'a>,
    index: usize,
) -> TimelineSpeed<'a> {
    if variant.speed.is_empty() && index == active_variant_index(legacy) {
        legacy.speed
    } else {
        variant.speed
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
    source_fingerprint: &lettuce_types::ContentHash,
) -> ImportContext {
    let mut providers = BTreeMap::new();
    let mut models = BTreeMap::new();
    let mut personas = BTreeMap::new();
    let mut prompts = BTreeMap::new();
    let mut lorebooks = BTreeMap::new();
    let mut media = BTreeMap::new();
    for assignment in &admission.assignments {
        match assignment {
            LegacyImportAssignment::Prompt {
                legacy_id,
                destination_id,
            } => {
                prompts.insert(legacy_id.clone(), *destination_id);
            }
            LegacyImportAssignment::Lorebook {
                legacy_id,
                destination_id,
            } => {
                lorebooks.insert(*legacy_id, *destination_id);
            }
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
            LegacyImportAssignment::Media {
                relative_path,
                destination_id,
                ..
            } => {
                media.insert(relative_path.clone(), *destination_id);
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
    let backgrounds = plan
        .media
        .media
        .iter()
        .flat_map(|candidate| {
            candidate
                .uses
                .iter()
                .filter_map(|media_use| match media_use {
                    lettuce_transfer::LegacyMediaUse::ConversationBackground {
                        session_id,
                        group,
                    } => Some((
                        (session_id.clone(), *group),
                        *media.get(&candidate.relative_path)?,
                    )),
                    _ => None,
                })
        })
        .collect();
    ImportContext {
        models,
        personas,
        prompts,
        lorebooks,
        media,
        backgrounds,
        scope: LegacyIdScope::new(source_fingerprint),
        dynamic_memory_enabled: false,
    }
}

pub(crate) fn parse<T: std::str::FromStr>(value: &str) -> Result<T, Error> {
    value.parse().map_err(|_| Error::InvalidInput)
}

fn timestamp(value: u64) -> Result<TimestampMillis, Error> {
    i64::try_from(value)
        .map(TimestampMillis::new)
        .map_err(|_| Error::InvalidInput)
}

fn has_media(parts: &[MessagePart]) -> bool {
    parts
        .iter()
        .any(|part| matches!(part, MessagePart::MediaAsset { .. }))
}

/// Legacy chat parts: reasoning, the text, then its attachments.
fn parts(
    content: &str,
    reasoning: Option<&str>,
    attachments: Vec<MessagePart>,
) -> Vec<MessagePart> {
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
    parts.extend(attachments);
    parts
}

#[cfg(test)]
mod tests {
    use lettuce_conversations::{CompanionClockSettings, CompanionTimeOverride};

    use super::*;

    #[test]
    fn saved_companion_state_decides_the_clock_like_legacy_temporal_reads() {
        assert_eq!(legacy_companion_clock("null"), None);
        assert_eq!(
            legacy_companion_clock("{}"),
            Some(CompanionClockSettings::default())
        );
        assert_eq!(
            legacy_companion_clock(
                r#"{"preferences":{"time_awareness_enabled":true,"timeOverride":{"mode":"frozen"}}}"#
            ),
            Some(CompanionClockSettings {
                time_awareness_enabled: true,
                time_override: CompanionTimeOverride::Live,
            })
        );
        assert_eq!(
            legacy_companion_clock(
                r#"{"preferences":{"timeAwarenessEnabled":true,"timeOverride":{"mode":"ticking","anchorMs":10,"setAtMs":4}}}"#
            ),
            Some(CompanionClockSettings {
                time_awareness_enabled: true,
                time_override: CompanionTimeOverride::Ticking {
                    anchor_at: TimestampMillis::new(10),
                    set_at: TimestampMillis::new(4),
                },
            })
        );
    }
}
