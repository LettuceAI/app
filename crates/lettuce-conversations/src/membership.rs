//! Which participants a group conversation uses now: its own rows for what
//! it owns, its group's current membership for what it follows.

use lettuce_types::{CharacterId, ConversationParticipantId};

use crate::model::{Conversation, ConversationKind, ConversationParticipant, ParticipantSource};

/// One member of a group as it stands now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupMembershipMember {
    pub character_id: CharacterId,
    pub muted: bool,
}

/// A group's current members; a conversation whose group no longer exists
/// has none.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GroupMembership {
    pub members: Vec<GroupMembershipMember>,
}

/// The participants a group conversation uses. Each aspect the conversation
/// owns (its member list, its muted flags) comes from its rows; otherwise a
/// character is enabled while it is one of the group's current members and
/// muted as the group mutes it. Without a group, launch members keep their
/// launch values and later members their rows. A one-to-one chat's
/// participants are its rows.
#[must_use]
pub fn effective_participants(
    conversation: &Conversation,
    group: Option<&GroupMembership>,
) -> Vec<ConversationParticipant> {
    let ConversationKind::Group(details) = &conversation.kind else {
        return conversation.participants.clone();
    };
    let own = conversation.current_settings.as_ref();
    let members_owned = own.is_some_and(|settings| settings.members_overridden);
    let muted_owned = own.is_some_and(|settings| settings.muted_overridden);
    conversation
        .participants
        .iter()
        .map(|participant| {
            let mut participant = participant.clone();
            let ParticipantSource::Character(character_id) = participant.source else {
                return participant;
            };
            match group {
                Some(group) => {
                    let member = group
                        .members
                        .iter()
                        .find(|member| member.character_id == character_id);
                    if !members_owned {
                        participant.enabled = member.is_some();
                    }
                    if !muted_owned && let Some(member) = member {
                        participant.muted = member.muted;
                    }
                }
                None => {
                    if let Some(launch) = details
                        .initial_participant_policy
                        .members
                        .iter()
                        .find(|policy| policy.participant_id == participant.id)
                    {
                        if !members_owned {
                            participant.enabled = launch.enabled;
                        }
                        if !muted_owned {
                            participant.muted = launch.muted;
                        }
                    }
                }
            }
            participant
        })
        .collect()
}

/// Whether `participant_id` is an enabled character of the conversation now.
#[must_use]
pub fn is_effective_member(
    conversation: &Conversation,
    group: Option<&GroupMembership>,
    participant_id: ConversationParticipantId,
) -> bool {
    effective_participants(conversation, group)
        .iter()
        .any(|participant| {
            participant.id == participant_id
                && participant.role == crate::model::ParticipantRole::Character
                && participant.enabled
        })
}
