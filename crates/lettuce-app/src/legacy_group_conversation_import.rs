use std::collections::{BTreeMap, BTreeSet};

use lettuce_characters::{CharacterRepository, LifecycleStatus};
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
    CharacterId, ConversationParticipantId, GroupId, Revision, SnapshotArtifactId, TimestampMillis,
};

use crate::launch::documents;
use crate::legacy_direct_conversation_import::{
    ImportContext, LegacyConversationSource, TimelineMessage, TimelineVariant, committed_stage,
    conversation_record, derived, import_context, launch_key, legacy_user, parse,
    persona_selection, selected_model,
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
            lettuce_transfer::LegacyImportStage::GroupConversations,
            sessions.len(),
        )? {
            return Ok(receipt);
        }
        let context = import_context(admission, plan);
        let conversations = sessions
            .iter()
            .map(|session| self.map_session(session, &context, completed_at))
            .collect::<Result<Vec<_>, _>>()?;
        self.sources.materialize_group_conversations(
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
            &import.compatibility.group_sessions().sessions,
            completed_at,
        )
    }

    fn map_session(
        &self,
        session: &LegacyBackupGroupSession,
        context: &ImportContext,
        now: TimestampMillis,
    ) -> Result<lettuce_transfer::LegacyConversationRecord, Error> {
        let group_id = parse::<GroupId>(
            session
                .group_source_id
                .as_deref()
                .ok_or(Error::InvalidInput)?,
        )?;
        let request = GroupConversationLaunchRequest {
            format_version: GROUP_LAUNCH_REQUEST_FORMAT_V1,
            title: session.name.clone(),
            user: legacy_user(),
            group_id,
            persona: persona_selection(false, session.persona_source_id.as_deref(), context)?,
            operation_key: launch_key(&session.source_id)?,
        };
        let (mut plan, mut snapshots) = ConversationLaunchPlanner::new(self.sources)
            .prepare_group(&request, now)
            .map_err(|_| Error::Conflict)?
            .into_parts();
        let authors = session_cast(
            self.sources,
            session,
            &mut plan.kind,
            &mut plan.participants,
            &mut snapshots,
        )?;
        let ConversationKind::Group(details) = &plan.kind else {
            return Err(Error::InvalidInput);
        };
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
                    .get(&parse::<CharacterId>(id)?)
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
                    reasoning: variant.reasoning.as_deref(),
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
        .map(|id| parse_character(id))
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
        let id = parse_character(id)?;
        if !wanted.contains(&id) {
            wanted.push(id);
        }
    }
    let listed = session
        .member_source_ids
        .iter()
        .map(|id| parse_character(id))
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
            None => new_member(sources, session, character_id, snapshots)?,
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
    if members.iter().all(|member| member.muted || !member.enabled) {
        return Err(Error::InvalidInput);
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

const UNKNOWN_NAME: &str = "Unknown";

fn new_member<S: DirectLaunchSources>(
    sources: &S,
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
        SnapshotArtifactId::from_uuid(derived(
            &session.source_id,
            &format!("member:{character_id}"),
        )),
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
        id: ConversationParticipantId::from_uuid(derived(
            &session.source_id,
            &format!("participant:{character_id}"),
        )),
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

fn parse_character(value: &str) -> Result<CharacterId, Error> {
    parse(value)
}
