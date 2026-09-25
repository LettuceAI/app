//! The live sources a chat turn reads instead of its launch snapshots, the
//! way legacy re-read the session's persona, group and characters each turn.

use lettuce_characters::{
    GroupProfile, GroupRepository, LifecycleStatus, Persona, PersonaRepository, RepositoryError,
    Selection,
};
use lettuce_conversations::{
    Conversation, ConversationKind, GroupChatModeSnapshot, SettingProvenance, SnapshotSelection,
};

/// A group conversation's live profile and the group settings a turn uses.
/// Legacy resolved the chat mode and the character-lorebook switch from the
/// group on every turn unless the session overrode them
/// (`group_sessions.rs` 509-600). A launch applies such an override to its
/// snapshot, so while the group is unchanged since launch the snapshot's value
/// is the conversation's; once the group changes, its current value is used.
pub(crate) struct LiveGroup {
    pub(crate) profile: Option<GroupProfile>,
    pub(crate) chat_mode: GroupChatModeSnapshot,
    pub(crate) disable_character_lorebooks: bool,
}

pub(crate) fn live_group<S: GroupRepository + ?Sized>(
    sources: &S,
    conversation: &Conversation,
) -> Result<Option<LiveGroup>, RepositoryError> {
    let ConversationKind::Group(details) = &conversation.kind else {
        return Ok(None);
    };
    let launch = &details.group;
    let profile = sources.get(launch.source_id)?.map(|details| details.group);
    let (chat_mode, disable_character_lorebooks) = match &profile {
        Some(profile) if profile.revision != launch.source_revision => (
            crate::launch::policy::group_chat_mode(profile.chat_mode),
            profile.disable_character_lorebooks,
        ),
        _ => (launch.chat_mode, launch.disable_character_lorebook),
    };
    Ok(Some(LiveGroup {
        profile,
        chat_mode,
        disable_character_lorebooks,
    }))
}

/// The persona a turn speaks to, read live (legacy `choose_persona`,
/// `storage.rs` 509-521). A one-to-one chat uses its chosen persona, or the
/// current default persona when it chose none or its persona no longer exists
/// or is archived; a disabled persona is none. A group chat uses its own
/// persona, else the persona its launch chose, else the group's current
/// selection (the default persona when the group inherits it), and a persona that no longer exists is none, as legacy
/// `load_persona` found none.
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
                SnapshotSelection::Disabled => return Ok(None),
                SnapshotSelection::Explicit(persona) => Chosen::Explicit(Some(persona.source_id)),
                SnapshotSelection::Inherited(_) => Chosen::Default,
            },
            ConversationKind::Group(details) => match (&details.group.persona, group) {
                (SnapshotSelection::Disabled, _) => return Ok(None),
                (SnapshotSelection::Explicit(persona), _)
                | (SnapshotSelection::Inherited(persona), None) => {
                    Chosen::Explicit(Some(persona.source_id))
                }
                (SnapshotSelection::Inherited(_), Some(profile)) => match profile.persona {
                    Selection::Disabled => return Ok(None),
                    Selection::Inherit => Chosen::Default,
                    Selection::Explicit(id) => Chosen::Explicit(Some(id)),
                },
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
