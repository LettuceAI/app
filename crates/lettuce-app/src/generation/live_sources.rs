//! The live sources a chat turn reads instead of its launch snapshots: the
//! conversation's persona, group and characters, re-read each turn.

use lettuce_characters::{
    CharacterRepository, GroupProfile, GroupRepository, GroupStartingScene, LifecycleStatus,
    Persona, PersonaRepository, RepositoryError, Selection,
};
use lettuce_conversations::{
    Conversation, ConversationKind, ConversationParticipant, GroupChatModeSnapshot,
    GroupSpeakerSelectionSnapshot, MemoryModeSnapshot, MemorySettingsSnapshot, ParticipantSource,
    SettingProvenance, SnapshotSelection,
};
use lettuce_types::{CharacterId, ModelProfileId};

/// A group conversation's live profile and the group settings a turn uses.
/// Each value is the conversation's own when it overrides it, else the
/// group's current one, else the launch value when the group no longer
/// exists.
pub(crate) struct LiveGroup {
    pub(crate) profile: Option<GroupProfile>,
    pub(crate) starting_scene: Option<GroupStartingScene>,
    pub(crate) chat_mode: GroupChatModeSnapshot,
    pub(crate) disable_character_lorebooks: bool,
    pub(crate) speaker_selection: GroupSpeakerSelectionSnapshot,
}

pub(crate) fn live_group<S: GroupRepository + ?Sized>(
    sources: &S,
    conversation: &Conversation,
) -> Result<Option<LiveGroup>, RepositoryError> {
    let ConversationKind::Group(details) = &conversation.kind else {
        return Ok(None);
    };
    let launch = &details.group;
    let (profile, starting_scene) = match sources.get(launch.source_id)? {
        Some(details) => (Some(details.group), details.starting_scene),
        None => (None, None),
    };
    let own = conversation.current_settings.as_ref();
    let chat_mode = own
        .and_then(|settings| settings.chat_mode)
        .or_else(|| {
            profile
                .as_ref()
                .map(|profile| crate::launch::policy::group_chat_mode(profile.chat_mode))
        })
        .unwrap_or(launch.chat_mode);
    let disable_character_lorebooks = own
        .and_then(|settings| settings.disable_character_lorebooks)
        .or_else(|| {
            profile
                .as_ref()
                .map(|profile| profile.disable_character_lorebooks)
        })
        .unwrap_or(launch.disable_character_lorebook);
    let speaker_selection = own
        .filter(|settings| {
            settings.speaker_selection_provenance == SettingProvenance::CurrentOverride
        })
        .and_then(|settings| settings.speaker_selection)
        .or_else(|| {
            profile.as_ref().map(|profile| {
                crate::launch::policy::group_speaker_selection(profile.speaker_selection)
            })
        })
        .unwrap_or(launch.speaker_selection);
    Ok(Some(LiveGroup {
        profile,
        starting_scene,
        chat_mode,
        disable_character_lorebooks,
        speaker_selection,
    }))
}

/// The participants a group turn uses (`lettuce_conversations::effective_participants`
/// over the group's current membership).
pub(crate) fn effective_participants(
    conversation: &Conversation,
    profile: Option<&GroupProfile>,
) -> Vec<ConversationParticipant> {
    lettuce_conversations::effective_participants(
        conversation,
        profile.map(group_membership).as_ref(),
    )
}

/// A group profile's members as the conversation domain sees them.
pub(crate) fn group_membership(profile: &GroupProfile) -> lettuce_conversations::GroupMembership {
    lettuce_conversations::GroupMembership {
        members: profile
            .members
            .iter()
            .map(|member| lettuce_conversations::GroupMembershipMember {
                character_id: member.character_id,
                muted: member.muted,
            })
            .collect(),
    }
}

/// The group's current members a group conversation has no row for yet, in
/// cast order. Empty while the conversation owns its member list, for a
/// group that no longer exists and for a one-to-one chat.
pub(crate) fn missing_group_members(
    conversation: &Conversation,
    profile: Option<&GroupProfile>,
) -> Vec<CharacterId> {
    let (ConversationKind::Group(_), Some(profile)) = (&conversation.kind, profile) else {
        return Vec::new();
    };
    if conversation
        .current_settings
        .as_ref()
        .is_some_and(|settings| settings.members_overridden)
    {
        return Vec::new();
    }
    let mut members = profile.members.iter().collect::<Vec<_>>();
    members.sort_by_key(|member| member.ordinal);
    members
        .into_iter()
        .map(|member| member.character_id)
        .filter(|character_id| {
            !conversation.participants.iter().any(|participant| {
                participant.source == ParticipantSource::Character(*character_id)
            })
        })
        .collect()
}

/// Where a group speaker's model comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MemberModel {
    /// A model the conversation chose: its own model override, or a
    /// participant model it owns.
    Snapshot(lettuce_conversations::ModelSelectionSnapshot),
    /// The group's current model for the member.
    Profile(ModelProfileId),
    /// The character's current default model, then the app default.
    Live,
}

/// A group speaker's model: the conversation's own model override, then the
/// participant's own model when the conversation owns the members' models
/// (or the group no longer exists), else the group's current model for the
/// member; without either, the character's live default.
pub(crate) fn member_model(
    conversation: &Conversation,
    participant: &ConversationParticipant,
    profile: Option<&GroupProfile>,
) -> MemberModel {
    let own = conversation.current_settings.as_ref();
    if let Some(model) = own
        .filter(|settings| settings.model_provenance == SettingProvenance::CurrentOverride)
        .and_then(|settings| settings.model_override.clone())
    {
        return MemberModel::Snapshot(model);
    }
    let owned = own.is_some_and(|settings| settings.member_models_overridden);
    match (profile, owned) {
        (Some(profile), false) => {
            let ParticipantSource::Character(character_id) = participant.source else {
                return MemberModel::Live;
            };
            profile
                .members
                .iter()
                .find(|member| member.character_id == character_id)
                .and_then(|member| member.model_profile_override)
                .map_or(MemberModel::Live, MemberModel::Profile)
        }
        _ => match &participant.model_selection {
            SnapshotSelection::Explicit(model) => MemberModel::Snapshot(model.clone()),
            SnapshotSelection::Inherited(_) | SnapshotSelection::Disabled => MemberModel::Live,
        },
    }
}

/// The memory settings a conversation runs with now: its own setting, else
/// the live character's memory mode (one-to-one) or the group's current one,
/// else the launch value when the source no longer exists. A live mode equal
/// to the launch mode keeps the launch settings, frozen policy included; a
/// different one takes the current global policy, as a launch would.
pub(crate) fn live_memory<S: CharacterRepository + GroupRepository + ?Sized>(
    sources: &S,
    conversation: &Conversation,
    settings: &lettuce_settings::GlobalSettings,
) -> Result<Option<MemorySettingsSnapshot>, RepositoryError> {
    if let Some(own) = conversation.current_settings.as_ref() {
        match own.memory_provenance {
            SettingProvenance::CurrentOverride => return Ok(own.memory.clone()),
            SettingProvenance::Disabled => return Ok(None),
            SettingProvenance::LaunchInherited => {}
        }
    }
    let (launch, live_mode, policy) = match &conversation.kind {
        ConversationKind::Direct(details) => (
            selection_value(&details.memory),
            CharacterRepository::get(sources, details.character.source_id)?
                .map(|character| crate::launch::policy::memory_mode(&character.character.defaults)),
            &settings.dynamic_memory,
        ),
        ConversationKind::Group(details) => (
            selection_value(&details.group.memory),
            GroupRepository::get(sources, details.group.source_id)?
                .map(|group| crate::launch::policy::memory_mode_of(group.group.memory_policy)),
            settings.effective_group_dynamic_memory(),
        ),
    };
    Ok(match live_mode {
        Some(mode) if launch.as_ref().is_none_or(|launch| launch.mode != mode) => {
            Some(MemorySettingsSnapshot {
                policy_ref: None,
                mode,
                selected_revision_ids: Vec::new(),
                dynamic_policy: crate::launch::planner::dynamic_memory_policy_snapshot(
                    mode, policy,
                ),
            })
        }
        _ => launch,
    })
}

/// Whether the conversation's memory mode is dynamic now (before the global
/// switch one-to-one chats also need).
pub(crate) fn live_memory_is_dynamic<S: CharacterRepository + GroupRepository + ?Sized>(
    sources: &S,
    conversation: &Conversation,
    settings: &lettuce_settings::GlobalSettings,
) -> Result<bool, RepositoryError> {
    Ok(live_memory(sources, conversation, settings)?
        .is_some_and(|memory| memory.mode == MemoryModeSnapshot::Dynamic))
}

fn selection_value<T: Clone>(selection: &SnapshotSelection<T>) -> Option<T> {
    match selection {
        SnapshotSelection::Inherited(value) | SnapshotSelection::Explicit(value) => {
            Some(value.clone())
        }
        SnapshotSelection::Disabled => None,
    }
}

/// The persona a turn speaks to, read live. A persona the conversation turned
/// off (its own disabled setting) is none. A one-to-one chat uses its chosen
/// persona, or the current default persona when it chose none, including a
/// launch that found no default then, or when its persona no longer exists
/// or is archived. A group chat uses its own persona, else the persona its
/// launch chose explicitly, else the group's current selection (the default
/// persona when the group inherits it), and a persona that no longer exists
/// is none.
pub(crate) fn live_persona<S: PersonaRepository + ?Sized>(
    sources: &S,
    conversation: &Conversation,
    group: Option<&GroupProfile>,
) -> Result<Option<Persona>, RepositoryError> {
    let own = conversation
        .current_settings
        .as_ref()
        .map(|settings| (settings.persona_provenance, settings.persona.as_ref()));
    let chosen = match own {
        Some((SettingProvenance::Disabled, _)) => return Ok(None),
        Some((SettingProvenance::CurrentOverride, persona)) => {
            Chosen::Explicit(persona.map(|persona| persona.source_id))
        }
        _ => match &conversation.kind {
            ConversationKind::Direct(details) => match &details.persona {
                SnapshotSelection::Explicit(persona) => Chosen::Explicit(Some(persona.source_id)),
                SnapshotSelection::Inherited(_) | SnapshotSelection::Disabled => Chosen::Default,
            },
            ConversationKind::Group(details) => match (&details.group.persona, group) {
                (SnapshotSelection::Disabled, None) => return Ok(None),
                (SnapshotSelection::Explicit(persona), _)
                | (SnapshotSelection::Inherited(persona), None) => {
                    Chosen::Explicit(Some(persona.source_id))
                }
                (SnapshotSelection::Inherited(_) | SnapshotSelection::Disabled, Some(profile)) => {
                    match profile.persona {
                        Selection::Disabled => return Ok(None),
                        Selection::Inherit => Chosen::Default,
                        Selection::Explicit(id) => Chosen::Explicit(Some(id)),
                    }
                }
            },
        },
    };
    let group_chat = matches!(conversation.kind, ConversationKind::Group(_));
    match chosen {
        Chosen::Explicit(Some(id)) => {
            let persona = sources.get(id)?;
            match persona {
                Some(persona) if group_chat || persona.status == LifecycleStatus::Active => {
                    Ok(Some(persona))
                }
                _ if group_chat => Ok(None),
                _ => default_persona(sources),
            }
        }
        Chosen::Explicit(None) => Ok(None),
        Chosen::Default => default_persona(sources),
    }
}

enum Chosen {
    Explicit(Option<lettuce_types::PersonaId>),
    Default,
}

fn default_persona<S: PersonaRepository + ?Sized>(
    sources: &S,
) -> Result<Option<Persona>, RepositoryError> {
    Ok(sources
        .get_default_snapshot()?
        .persona
        .filter(|persona| persona.status == LifecycleStatus::Active))
}
