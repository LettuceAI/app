use std::collections::{BTreeMap, BTreeSet};

use lettuce_characters::{
    CharacterRepository, GroupStartingScene, LifecycleStatus, Scene, SceneOwner, SceneVariant,
};
use lettuce_conversations::{
    CharacterLaunchSnapshot, CharacterSnapshotBodyV1, ConversationKind,
    ConversationParticipantDraft, GroupMemberLaunchSnapshot, GroupParticipantPolicySnapshot,
    InteractionModeV1, MemoryPolicyV1, ParticipantRole, ParticipantSource, SnapshotArtifactDraft,
    SnapshotSelection,
};
use lettuce_transfer::{
    LegacyBackupGroupSession, LegacyDirectConversationMaterializationRequest,
    LegacyImportAdmission, LegacyImportPlan, LegacyImportRepository, LegacyImportRepositoryError,
    LegacyImportStageReceipt,
};
use lettuce_types::{
    CharacterId, ConversationParticipantId, GroupId, Revision, SceneId, SceneVariantId,
    SnapshotArtifactId, TimestampMillis,
};

use crate::launch::documents;
use crate::legacy::legacy_direct_conversation_import::{
    ImportContext, LegacyConversationSource, SessionSettingsSource, TimelineMessage,
    TimelineVariant, committed_stage, conversation_record, import_context, launch_key, legacy_user,
    memory_owner, parse, persona_selection, selected_model, session_settings,
};
use crate::{
    ConversationLaunchPlanner, DirectLaunchSources, GROUP_LAUNCH_REQUEST_FORMAT_V1,
    GroupConversationLaunchRequest, GroupLaunchSources,
};

type Error = LegacyImportRepositoryError;

#[derive(Debug)]
pub struct LegacyGroupConversationImportCoordinator<'a, S> {
    sources: &'a S,
}

impl<'a, S> LegacyGroupConversationImportCoordinator<'a, S>
where
    S: GroupLaunchSources + LegacyImportRepository,
{
    #[must_use]
    pub const fn new(sources: &'a S) -> Self {
        Self { sources }
    }

    /// Converts each legacy group session into a finished group conversation
    /// launched from its imported group profile, with the session's own cast.
    pub fn execute(
        &self,
        admission: &LegacyImportAdmission,
        plan: &LegacyImportPlan,
        sessions: &[LegacyBackupGroupSession],
        memories: &[lettuce_transfer::LegacyBackupMemoryEmbeddingOwner],
        completed_at: TimestampMillis,
    ) -> Result<LegacyImportStageReceipt, Error> {
        let plan_fingerprint = crate::legacy::legacy_import::plan_fingerprint(plan);
        if plan_fingerprint != admission.plan_fingerprint {
            return Err(Error::Conflict);
        }
        let source_fingerprint = plan.source_fingerprint.clone().ok_or(Error::InvalidInput)?;
        let sessions = sessions
            .iter()
            .filter(|session| session.group_source_id.is_some())
            .collect::<Vec<_>>();
        if let Some(receipt) = committed_stage(
            self.sources,
            admission,
            &source_fingerprint,
            lettuce_transfer::LegacyImportStage::GroupConversations,
            sessions.len(),
        )? {
            return Ok(receipt);
        }
        let context = import_context(admission, plan, &source_fingerprint);
        let conversations = sessions
            .iter()
            .map(|session| {
                let memory = memory_owner(
                    memories,
                    lettuce_transfer::LegacyBackupMemoryOwnerKind::GroupConversation,
                    &session.source_id,
                );
                self.map_session(session, memory, &context, completed_at)
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.sources.materialize_group_conversations(
            LegacyDirectConversationMaterializationRequest {
                run_id: admission.run_id,
                plan_fingerprint,
                source_fingerprint,
                conversations,
                companion_souls: Vec::new(),
                scheduled_notes: Vec::new(),
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
            &import.compatibility.group_sessions().sessions,
            &import.compatibility.memory_embeddings().owners,
            completed_at,
        )
    }

    fn map_session(
        &self,
        session: &LegacyBackupGroupSession,
        memory: Option<&lettuce_transfer::LegacyBackupMemoryEmbeddingOwner>,
        context: &ImportContext,
        now: TimestampMillis,
    ) -> Result<lettuce_transfer::LegacyConversationRecord, Error> {
        let legacy_group = parse::<GroupId>(
            session
                .group_source_id
                .as_deref()
                .ok_or(Error::InvalidInput)?,
        )?;
        let group_id = GroupId::from_uuid(context.scope.uuid(legacy_group.as_uuid()));
        let request = GroupConversationLaunchRequest {
            format_version: GROUP_LAUNCH_REQUEST_FORMAT_V1,
            title: session.name.clone(),
            user: legacy_user(),
            group_id,
            persona: persona_selection(false, session.persona_source_id.as_deref(), context)?,
            operation_key: launch_key(context.scope, &session.source_id)?,
        };
        let overrides = crate::launch::GroupLaunchOverrides {
            chat_mode: Some(match session.chat_mode.as_str() {
                "roleplay" => lettuce_characters::ChatMode::Roleplay,
                _ => lettuce_characters::ChatMode::Conversation,
            }),
            memory_policy: Some(match session.memory_policy.as_str() {
                "dynamic" => lettuce_characters::MemoryPolicy::Dynamic,
                _ => lettuce_characters::MemoryPolicy::Manual,
            }),
            disable_character_lorebooks: Some(session.disable_character_lorebooks),
            member_models: Some(
                session
                    .character_model_overrides
                    .iter()
                    .filter_map(|(character, model)| {
                        let character = parse_character(context.scope, character).ok()?;
                        let model = model.parse::<lettuce_types::ModelProfileId>().ok()?;
                        Some((character, Some(context.model_destination(model)?)))
                    })
                    .collect(),
            ),
            starting_scene: session.starting_scene_override.as_ref().map(|scene| {
                scene
                    .as_ref()
                    .map(|scene| session_scene(context.scope, session, group_id, scene))
            }),
        };
        let (mut plan, mut snapshots) = ConversationLaunchPlanner::new(self.sources)
            .prepare_group_with(&request, &overrides, now)
            .map_err(|_| Error::Conflict)?
            .into_parts();
        let authors = session_cast(
            self.sources,
            context.scope,
            session,
            &mut plan.kind,
            &mut plan.participants,
            &mut snapshots,
        )?;
        let ConversationKind::Group(details) = &plan.kind else {
            return Err(Error::InvalidInput);
        };
        let (prompt_source_id, prompt_purpose, prompt_snapshot_purpose) =
            match details.group.chat_mode {
                lettuce_conversations::GroupChatModeSnapshot::Conversation => (
                    session.group_conversation_prompt_source_id.as_deref(),
                    lettuce_context::PromptPurpose::GroupChatConversational,
                    lettuce_conversations::PromptPurposeSnapshot::GroupConversational,
                ),
                lettuce_conversations::GroupChatModeSnapshot::Roleplay => (
                    session.group_roleplay_prompt_source_id.as_deref(),
                    lettuce_context::PromptPurpose::GroupChatRoleplay,
                    lettuce_conversations::PromptPurposeSnapshot::GroupRoleplay,
                ),
            };
        let speaker_selection = match session.speaker_selection.as_str() {
            "llm" => lettuce_conversations::GroupSpeakerSelectionSnapshot::Llm,
            "heuristic" => lettuce_conversations::GroupSpeakerSelectionSnapshot::Heuristic,
            "round_robin" => lettuce_conversations::GroupSpeakerSelectionSnapshot::RoundRobin,
            "director" => lettuce_conversations::GroupSpeakerSelectionSnapshot::Director,
            "director_action" => {
                lettuce_conversations::GroupSpeakerSelectionSnapshot::DirectorAction
            }
            _ => return Err(Error::InvalidInput),
        };
        let (settings, settings_snapshots) = session_settings(
            self.sources,
            &session.source_id,
            context,
            SessionSettingsSource {
                author_note: session.author_note.as_deref(),
                prompt_source_id,
                prompt_purposes: &[prompt_purpose],
                prompt_snapshot_purpose,
                lorebook_source_ids: session
                    .lorebooks_overridden
                    .then_some(session.lorebook_source_ids.as_slice()),
                speaker_selection: (speaker_selection != details.group.speaker_selection)
                    .then_some(speaker_selection),
                model_settings: &lettuce_models::ModelSettingsLayer::default(),
                background: context.background(
                    &session.source_id,
                    true,
                    lettuce_transfer::legacy_group_session_background(
                        &session.config_overrides_json,
                    )
                    .as_deref(),
                ),
                companion_clock: None,
            },
        )?;
        snapshots.extend(settings_snapshots);
        let model = selected_model(&details.group.model).or_else(|| {
            details
                .group
                .members
                .iter()
                .find_map(|member| selected_model(&member.model_override))
        });
        let author = |id: Option<&String>| -> Result<Option<ConversationParticipantId>, Error> {
            id.map(|id| {
                authors
                    .get(&parse_character(context.scope, id)?)
                    .copied()
                    .ok_or(Error::InvalidInput)
            })
            .transpose()
        };
        let mut rows = session.messages.iter().collect::<Vec<_>>();
        rows.sort_by_key(|message| message.ordinal);
        let mut messages = Vec::with_capacity(rows.len());
        for row in rows {
            let mut variants = Vec::with_capacity(row.variants.len());
            for variant in &row.variants {
                variants.push(TimelineVariant {
                    source_id: &variant.source_id,
                    content: &variant.content,
                    created_at: variant.created_at,
                    prompt_tokens: variant.usage.prompt_tokens,
                    completion_tokens: variant.usage.completion_tokens,
                    total_tokens: variant.usage.total_tokens,
                    reasoning: variant.reasoning.as_deref(),
                    attachments_json: Some(&variant.attachments_json),
                    author: author(variant.speaker_character_source_id.as_ref())?,
                });
            }
            messages.push(TimelineMessage {
                source_id: &row.source_id,
                role: &row.role,
                content: &row.content,
                created_at: row.created_at,
                effective_at: None,
                visible_in_chat: true,
                pinned: row.pinned,
                scene_edited: false,
                author: author(row.speaker_character_source_id.as_ref())?,
                model_source_id: row.model_source_id.as_deref(),
                selected_variant_source_id: row.selected_variant_source_id.as_deref(),
                reasoning: row.reasoning.as_deref(),
                attachments_json: &row.attachments_json,
                variants,
            });
        }
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
                memory_texts: crate::legacy::legacy_direct_conversation_import::shown_memory_texts(
                    session.memory_policy == "dynamic",
                    memory,
                    &session.memories_json,
                ),
                memory_summary: Some(session.memory_summary.as_str()),
                memory_summary_token_count: session.memory_summary_token_count,
                memory_tool_events: Some(session.memory_tool_events_json.as_str()),
                settings,
            },
            context,
        )
    }
}

/// A launch plan's group cast rewritten to the members of one legacy session:
/// members the session kept reuse the planner's snapshots, members the group
/// profile no longer lists get a fresh character snapshot, and speakers whose
/// character was deleted become disabled, muted "Unknown" members so their
/// messages keep an author without ever being selected again.
fn session_cast<S: DirectLaunchSources>(
    sources: &S,
    scope: lettuce_transfer::LegacyIdScope,
    session: &LegacyBackupGroupSession,
    kind: &mut ConversationKind,
    participants: &mut Vec<ConversationParticipantDraft>,
    snapshots: &mut Vec<SnapshotArtifactDraft>,
) -> Result<BTreeMap<CharacterId, ConversationParticipantId>, Error> {
    let ConversationKind::Group(details) = kind else {
        return Err(Error::InvalidInput);
    };
    let muted = session
        .muted_member_source_ids
        .iter()
        .map(|id| parse_character(scope, id))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let mut wanted = Vec::new();
    for id in session
        .member_source_ids
        .iter()
        .chain(session.messages.iter().flat_map(|message| {
            message.speaker_character_source_id.iter().chain(
                message
                    .variants
                    .iter()
                    .filter_map(|variant| variant.speaker_character_source_id.as_ref()),
            )
        }))
    {
        let id = parse_character(scope, id)?;
        if !wanted.contains(&id) {
            wanted.push(id);
        }
    }
    let listed = session
        .member_source_ids
        .iter()
        .map(|id| parse_character(scope, id))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let user = participants
        .iter()
        .find(|participant| participant.role == ParticipantRole::User)
        .cloned()
        .ok_or(Error::InvalidInput)?;
    let mut planned = details
        .group
        .members
        .drain(..)
        .filter_map(|member| {
            let participant = participants.iter().find(|participant| {
                participant.source == ParticipantSource::Character(member.character.source_id)
            })?;
            Some((member.character.source_id, (member, participant.clone())))
        })
        .collect::<BTreeMap<_, _>>();
    let mut members = Vec::with_capacity(wanted.len());
    let mut cast = vec![user];
    let mut authors = BTreeMap::new();
    for (ordinal, character_id) in wanted.into_iter().enumerate() {
        let ordinal = u32::try_from(ordinal).map_err(|_| Error::InvalidInput)?;
        let (mut member, mut participant) = match planned.remove(&character_id) {
            Some(value) => value,
            None => new_member(sources, scope, session, character_id, snapshots)?,
        };
        let known = participant.display_name != UNKNOWN_NAME;
        member.ordinal = ordinal;
        member.enabled = known && listed.contains(&character_id);
        member.muted = !known || muted.contains(&character_id);
        participant.ordinal = ordinal + 1;
        participant.enabled = member.enabled;
        participant.muted = member.muted;
        authors.insert(character_id, participant.id);
        members.push(member);
        cast.push(participant);
    }
    if members.is_empty() {
        return Err(Error::InvalidInput);
    }
    if members.iter().all(|member| member.muted) {
        let index = members
            .iter()
            .position(|member| member.enabled)
            .unwrap_or(0);
        members[index].muted = false;
        cast[index + 1].muted = false;
    }
    let fallback_model =
        crate::legacy::legacy_direct_conversation_import::selected_model(&details.group.model)
            .or_else(|| {
                members.iter().find_map(|member| {
                    crate::legacy::legacy_direct_conversation_import::selected_model(
                        &member.model_override,
                    )
                })
            });
    if !details.group.model.is_resolved()
        && let Some(model) = fallback_model
    {
        for (index, member) in members.iter_mut().enumerate() {
            if !member.model_override.is_resolved() {
                member.model_override = SnapshotSelection::Inherited(model.clone());
                cast[index + 1].model_selection = member.model_override.clone();
            }
        }
    }
    details.group.members = members;
    details.initial_participant_policy.members = cast
        .iter()
        .filter(|participant| participant.role == ParticipantRole::Character)
        .map(|participant| GroupParticipantPolicySnapshot {
            participant_id: participant.id,
            enabled: participant.enabled,
            muted: participant.muted,
            model_override: participant.model_selection.clone(),
        })
        .collect();
    *participants = cast;
    let referenced = referenced_artifacts(kind, participants);
    snapshots.retain(|draft| referenced.contains(&draft.artifact_id));
    Ok(authors)
}

fn session_scene(
    scope: lettuce_transfer::LegacyIdScope,
    session: &LegacyBackupGroupSession,
    group_id: GroupId,
    scene: &lettuce_transfer::LegacyBackupSceneCandidate,
) -> GroupStartingScene {
    let scene_id = SceneId::from_uuid(scope.derived(&session.source_id, "starting_scene"));
    let variant_id = |id: SceneVariantId| {
        SceneVariantId::from_uuid(
            scope.derived(&session.source_id, &format!("starting_scene_variant:{id}")),
        )
    };
    GroupStartingScene {
        scene: Scene {
            id: scene_id,
            owner: SceneOwner::Group(group_id),
            status: LifecycleStatus::Active,
            ordinal: 0,
            content: scene.content.clone(),
            direction: scene.direction.clone(),
            selected_variant_id: scene.selected_variant_id.map(variant_id),
            assets: Vec::new(),
            revision: Revision::INITIAL,
            created_at: scene.created_at,
            updated_at: scene.created_at,
        },
        variants: scene
            .variants
            .iter()
            .map(|variant| SceneVariant {
                id: variant_id(variant.id),
                scene_id,
                ordinal: variant.ordinal,
                content: variant.content.clone(),
                direction: variant.direction.clone(),
                revision: Revision::INITIAL,
                created_at: variant.created_at,
                updated_at: variant.created_at,
            })
            .collect(),
    }
}

const UNKNOWN_NAME: &str = "Unknown";

fn new_member<S: DirectLaunchSources>(
    sources: &S,
    scope: lettuce_transfer::LegacyIdScope,
    session: &LegacyBackupGroupSession,
    character_id: CharacterId,
    snapshots: &mut Vec<SnapshotArtifactDraft>,
) -> Result<(GroupMemberLaunchSnapshot, ConversationParticipantDraft), Error> {
    let character = CharacterRepository::get(sources, character_id)
        .map_err(|_| Error::Storage)?
        .filter(|details| details.character.status != LifecycleStatus::Archived)
        .map(|details| details.character);
    let (revision, name, nickname, body) = match &character {
        Some(character) => (
            character.revision,
            crate::launch::policy::character_display_name(character),
            character.profile.nickname.clone(),
            documents::character_body(character),
        ),
        None => (
            Revision::INITIAL,
            UNKNOWN_NAME.to_owned(),
            None,
            unknown_character(character_id),
        ),
    };
    let draft = documents::draft(
        SnapshotArtifactId::from_uuid(
            scope.derived(&session.source_id, &format!("member:{character_id}")),
        ),
        revision,
        body,
    )
    .map_err(|_| Error::InvalidInput)?;
    let member = GroupMemberLaunchSnapshot {
        character: CharacterLaunchSnapshot {
            snapshot_ref: draft.reference(),
            source_id: character_id,
            source_revision: revision,
            name: name.clone(),
            nickname,
        },
        ordinal: 0,
        enabled: true,
        muted: false,
        model_override: SnapshotSelection::Disabled,
        lorebooks: SnapshotSelection::Disabled,
        prompt: SnapshotSelection::Disabled,
    };
    let participant = ConversationParticipantDraft {
        id: ConversationParticipantId::from_uuid(
            scope.derived(&session.source_id, &format!("participant:{character_id}")),
        ),
        role: ParticipantRole::Character,
        ordinal: 0,
        source: ParticipantSource::Character(character_id),
        enabled: true,
        muted: false,
        display_name: name,
        authored_description: None,
        model_selection: SnapshotSelection::Disabled,
    };
    snapshots.push(draft);
    Ok((member, participant))
}

fn unknown_character(character_id: CharacterId) -> CharacterSnapshotBodyV1 {
    CharacterSnapshotBodyV1 {
        character_id,
        name: UNKNOWN_NAME.to_owned(),
        nickname: None,
        description: None,
        definition: None,
        design_description: None,
        interaction_mode: InteractionModeV1::Roleplay,
        memory_policy: MemoryPolicyV1::Manual,
        model_profile_id: None,
        default_scene_id: None,
        default_starter_id: None,
        direct_prompt_id: None,
        group_conversation_prompt_id: None,
        group_roleplay_prompt_id: None,
        voice: None,
        voice_autoplay: false,
        image_recommendation: None,
        media: Vec::new(),
        presentation_asset_ids: Vec::new(),
    }
}

fn referenced_artifacts(
    kind: &ConversationKind,
    participants: &[ConversationParticipantDraft],
) -> BTreeSet<SnapshotArtifactId> {
    let mut references = lettuce_conversations::conversation_snapshot_references(kind)
        .into_iter()
        .map(|reference| reference.artifact_id)
        .collect::<BTreeSet<_>>();
    for participant in participants {
        if let SnapshotSelection::Inherited(model) | SnapshotSelection::Explicit(model) =
            &participant.model_selection
        {
            references.insert(model.snapshot_ref.artifact_id);
        }
    }
    references
}

fn parse_character(
    scope: lettuce_transfer::LegacyIdScope,
    value: &str,
) -> Result<CharacterId, Error> {
    parse::<CharacterId>(value).map(|legacy| CharacterId::from_uuid(scope.uuid(legacy.as_uuid())))
}
