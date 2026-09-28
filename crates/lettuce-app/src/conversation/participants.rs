use lettuce_characters::{CharacterRepository, GroupProfile, LifecycleStatus};
use lettuce_conversations::{
    AddConversationParticipant, Conversation, ConversationKind, ConversationParticipant,
    ConversationReader, ConversationRepository, ConversationRepositoryError,
    GroupMemberLaunchSnapshot, ModelSelectionSnapshot, OperationToken, ParticipantOverrides,
    ParticipantPolicyChange, ParticipantRole, ParticipantSource, PreparedParticipantAdd,
    SnapshotSelection, UpdateParticipantPolicy,
};
use lettuce_database::Database;
use lettuce_models::{ModelKind, ModelProfileRepository, ProviderAccountRepository};
use lettuce_types::{
    CharacterId, ConversationId, ConversationParticipantId, ModelProfileId, TimestampMillis,
};

use super::{Change, ConversationEditError, edit_operation, snapshot_artifact_id};
use crate::generation::live_sources::{self, MemberModel};
use crate::launch::{documents, planner, policy};

/// How many times an edit rereads the conversation after another write
/// moved its revision on.
const REVISION_RETRIES: usize = 8;

/// A participant edit: enable or disable, mute or unmute, choose or reset
/// its model. `None` keeps a value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParticipantChange {
    pub enabled: Option<bool>,
    pub muted: Option<bool>,
    pub model: Option<Change<ModelProfileId>>,
}

/// The operation that adds a member the group gained after launch; every
/// path that needs its row uses the same key, so the row is created once.
pub fn member_operation(
    conversation_id: ConversationId,
    character_id: CharacterId,
) -> Result<OperationToken, ConversationEditError> {
    edit_operation(
        format!("group-member.{character_id}"),
        &[
            b"lettuce-group-member-v1",
            conversation_id.to_string().as_bytes(),
            character_id.to_string().as_bytes(),
        ],
    )
}

/// The participant id a character added to a group conversation gets.
#[must_use]
pub fn member_participant_id(
    conversation_id: ConversationId,
    character_id: CharacterId,
) -> ConversationParticipantId {
    ConversationParticipantId::from_uuid(uuid::Uuid::new_v5(
        &conversation_id.as_uuid(),
        format!("participant:{character_id}").as_bytes(),
    ))
}

fn group_profile(
    database: &Database,
    conversation: &Conversation,
) -> Result<Option<GroupProfile>, ConversationEditError> {
    Ok(live_sources::live_group(database, conversation)
        .map_err(|_| ConversationEditError::Source)?
        .and_then(|group| group.profile))
}

/// Adds `character_id` to a group conversation, or enables the row it kept.
/// With `override_members` the conversation owns its member list from now
/// on; without it the member joins as one of the group's. A new row gets a
/// snapshot of the character like a launch member, and follows its model
/// live.
pub fn add_group_member(
    database: &Database,
    conversation_id: ConversationId,
    character_id: CharacterId,
    operation: OperationToken,
    override_members: bool,
    now: TimestampMillis,
) -> Result<Conversation, ConversationEditError> {
    let mut attempt = 0;
    loop {
        attempt += 1;
        let conversation = ConversationReader::get(database, conversation_id)?.conversation;
        if !matches!(conversation.kind, ConversationKind::Group(_)) {
            return Err(ConversationEditError::InvalidInput {
                field: "conversation_id",
            });
        }
        let profile = group_profile(database, &conversation)?;
        let existing = conversation
            .participants
            .iter()
            .find(|participant| participant.source == ParticipantSource::Character(character_id));
        let (participant_id, member, drafts, display_name) = match existing {
            Some(participant) => (
                participant.id,
                None,
                Vec::new(),
                participant.display_name.clone(),
            ),
            None => {
                let character = CharacterRepository::get(database, character_id)
                    .map_err(|_| ConversationEditError::Source)?
                    .filter(|details| details.character.status == LifecycleStatus::Active)
                    .ok_or(ConversationEditError::InvalidInput {
                        field: "character_id",
                    })?
                    .character;
                let draft = documents::draft(
                    snapshot_artifact_id(
                        conversation_id,
                        &format!("member:{}:{}", character.id, character.revision.get()),
                    ),
                    character.revision,
                    documents::character_body(&character),
                )
                .map_err(|_| ConversationEditError::Snapshot)?;
                let muted = profile
                    .as_ref()
                    .and_then(|profile| {
                        profile
                            .members
                            .iter()
                            .find(|member| member.character_id == character_id)
                    })
                    .is_some_and(|member| member.muted);
                let ordinal = u32::try_from(
                    conversation
                        .participants
                        .iter()
                        .filter(|participant| participant.role == ParticipantRole::Character)
                        .count(),
                )
                .unwrap_or(u32::MAX);
                let member = GroupMemberLaunchSnapshot {
                    character: planner::character_snapshot(&character, &draft),
                    ordinal,
                    enabled: true,
                    muted,
                    model_override: SnapshotSelection::Disabled,
                    lorebooks: SnapshotSelection::Disabled,
                    prompt: SnapshotSelection::Disabled,
                };
                (
                    member_participant_id(conversation_id, character_id),
                    Some(member),
                    vec![draft],
                    policy::character_display_name(&character),
                )
            }
        };
        let command = AddConversationParticipant {
            conversation_id,
            expected_revision: conversation.revision,
            operation: operation.clone(),
            participant_id,
            character_id,
            display_name,
            muted: member.as_ref().is_some_and(|member| member.muted),
            member,
            override_members,
        };
        let prepared = PreparedParticipantAdd::new(command, drafts)
            .map_err(|_| ConversationEditError::Snapshot)?;
        match ConversationRepository::add_participant(database, prepared, now) {
            Ok(added) => return Ok(added.value),
            Err(ConversationRepositoryError::StaleRevision { .. })
                if attempt < REVISION_RETRIES => {}
            Err(error) => return Err(error.into()),
        }
    }
}

/// Creates the row of every current group member a conversation that
/// follows its group has none for yet, so the member can speak. A member
/// whose character is gone or archived gets none.
pub fn ensure_group_members(
    database: &Database,
    conversation_id: ConversationId,
    now: TimestampMillis,
) -> Result<(), ConversationEditError> {
    let conversation = ConversationReader::get(database, conversation_id)?.conversation;
    let profile = group_profile(database, &conversation)?;
    for character_id in live_sources::missing_group_members(&conversation, profile.as_ref()) {
        match add_group_member(
            database,
            conversation_id,
            character_id,
            member_operation(conversation_id, character_id)?,
            false,
            now,
        ) {
            Ok(_)
            | Err(ConversationEditError::InvalidInput {
                field: "character_id",
            }) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

/// Changes one participant of a group conversation. Each aspect it touches
/// (member list, muted flags, models) becomes the conversation's own; the
/// other participants keep what they followed from the group, written into
/// their rows in the same change. The conversation command refuses a change
/// that leaves no enabled, unmuted character.
pub fn update_group_participant(
    database: &Database,
    conversation_id: ConversationId,
    participant_id: ConversationParticipantId,
    change: &ParticipantChange,
    operation: OperationToken,
    now: TimestampMillis,
) -> Result<Conversation, ConversationEditError> {
    ensure_group_members(database, conversation_id, now)?;
    let mut attempt = 0;
    loop {
        attempt += 1;
        let conversation = ConversationReader::get(database, conversation_id)?.conversation;
        if !matches!(conversation.kind, ConversationKind::Group(_)) {
            return Err(ConversationEditError::InvalidInput {
                field: "conversation_id",
            });
        }
        if !conversation.participants.iter().any(|participant| {
            participant.id == participant_id && participant.role == ParticipantRole::Character
        }) {
            return Err(ConversationEditError::InvalidInput {
                field: "participant_id",
            });
        }
        let profile = group_profile(database, &conversation)?;
        let effective = live_sources::effective_participants(&conversation, profile.as_ref());
        let own = conversation.current_settings.as_ref();
        let owned = ParticipantOverrides {
            members: own.is_some_and(|settings| settings.members_overridden),
            muted: own.is_some_and(|settings| settings.muted_overridden),
            member_models: own.is_some_and(|settings| settings.member_models_overridden),
        };
        let overrides = ParticipantOverrides {
            members: change.enabled.is_some(),
            muted: change.muted.is_some(),
            member_models: change.model.is_some(),
        };
        let mut materialize = Vec::new();
        for participant in &conversation.participants {
            if participant.id == participant_id || participant.role != ParticipantRole::Character {
                continue;
            }
            let current = effective
                .iter()
                .find(|value| value.id == participant.id)
                .unwrap_or(participant);
            let values = ParticipantPolicyChange {
                participant_id: participant.id,
                enabled: (overrides.members && !owned.members).then_some(current.enabled),
                muted: (overrides.muted && !owned.muted).then_some(current.muted),
                model_override: if overrides.member_models && !owned.member_models {
                    Some(group_member_model(
                        database,
                        &conversation,
                        participant,
                        profile.as_ref(),
                    )?)
                } else {
                    None
                },
            };
            if values.enabled.is_some() || values.muted.is_some() || values.model_override.is_some()
            {
                materialize.push(values);
            }
        }
        let model_override = match &change.model {
            Some(Change::Set(id)) => Some(SnapshotSelection::Explicit(conversation_model(
                database,
                conversation_id,
                *id,
            )?)),
            Some(Change::Reset) => Some(SnapshotSelection::Disabled),
            None => None,
        };
        let command = UpdateParticipantPolicy {
            conversation_id,
            participant_id,
            expected_revision: conversation.revision,
            operation: operation.clone(),
            enabled: change.enabled,
            muted: change.muted,
            model_override,
            materialize,
            overrides,
        };
        match ConversationRepository::update_participant_policy(database, &command, now) {
            Ok(updated) => return Ok(updated.value),
            Err(ConversationRepositoryError::StaleRevision { .. })
                if attempt < REVISION_RETRIES => {}
            Err(error) => return Err(error.into()),
        }
    }
}

/// The model a participant follows from its group, as the row value it keeps
/// once the conversation owns its members' models: the group's model for
/// the member, else none (the character's live default).
fn group_member_model(
    database: &Database,
    conversation: &Conversation,
    participant: &ConversationParticipant,
    profile: Option<&GroupProfile>,
) -> Result<SnapshotSelection<ModelSelectionSnapshot>, ConversationEditError> {
    let mut without_own = conversation.clone();
    if let Some(settings) = without_own.current_settings.as_mut() {
        settings.model_override = None;
        settings.model_provenance = lettuce_conversations::SettingProvenance::LaunchInherited;
    }
    Ok(
        match live_sources::member_model(&without_own, participant, profile) {
            MemberModel::Profile(id) => {
                SnapshotSelection::Explicit(conversation_model(database, conversation.id, id)?)
            }
            MemberModel::Snapshot(model) => SnapshotSelection::Explicit(model),
            MemberModel::Live => SnapshotSelection::Disabled,
        },
    )
}

/// A snapshot of a chat model, stored and attached to the conversation.
pub(crate) fn conversation_model(
    database: &Database,
    conversation_id: ConversationId,
    model_profile_id: ModelProfileId,
) -> Result<ModelSelectionSnapshot, ConversationEditError> {
    let invalid = ConversationEditError::InvalidInput {
        field: "model_profile_id",
    };
    let profile = ModelProfileRepository::get(database, model_profile_id)
        .map_err(|_| ConversationEditError::Source)?
        .filter(|profile| profile.kind == ModelKind::Chat)
        .ok_or(invalid.clone())?;
    let account = ProviderAccountRepository::get(database, profile.provider_account_id)
        .map_err(|_| ConversationEditError::Source)?
        .filter(|account| account.enabled)
        .ok_or(invalid)?;
    let draft = documents::draft(
        snapshot_artifact_id(
            conversation_id,
            &format!(
                "member-model:{}:{}:{}:{}",
                profile.id,
                profile.revision.get(),
                account.id,
                account.revision.get()
            ),
        ),
        profile.revision,
        documents::model_body(&profile, &account),
    )
    .map_err(|_| ConversationEditError::Snapshot)?;
    let snapshot = planner::model_snapshot(&profile, &account, &draft);
    ConversationRepository::artifact_store(database)
        .attach_snapshot(conversation_id, draft)
        .map_err(|_| ConversationEditError::Snapshot)?;
    Ok(snapshot)
}
