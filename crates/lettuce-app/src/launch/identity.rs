use lettuce_conversations::IdempotencyKey;
use lettuce_types::{
    ConversationId, ConversationParticipantId, MessageId, MessageRevisionId, SnapshotArtifactId,
};
use uuid::Uuid;

/// Fixed namespace for every application-derived launch identity. Changing it
/// would give already-created conversations new identities on replay.
const APP_NAMESPACE: Uuid = Uuid::from_u128(0x6d1f_4a27_9c83_5e14_b7a0_2f68_d345_91ceu128);

#[must_use]
pub fn launch_conversation_id(operation_key: &IdempotencyKey) -> ConversationId {
    ConversationId::from_uuid(Uuid::new_v5(
        &APP_NAMESPACE,
        operation_key.as_str().as_bytes(),
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LaunchIdentities {
    root: Uuid,
}

impl LaunchIdentities {
    pub(crate) const fn new(conversation_id: ConversationId) -> Self {
        Self {
            root: conversation_id.as_uuid(),
        }
    }

    fn derive(self, label: &str) -> Uuid {
        Uuid::new_v5(&self.root, label.as_bytes())
    }

    pub(crate) fn user_participant(self) -> ConversationParticipantId {
        ConversationParticipantId::from_uuid(self.derive("participant.user"))
    }

    pub(crate) fn character_participant(self) -> ConversationParticipantId {
        ConversationParticipantId::from_uuid(self.derive("participant.character"))
    }

    pub(crate) fn group_member_participant(self, ordinal: usize) -> ConversationParticipantId {
        ConversationParticipantId::from_uuid(
            self.derive(&format!("participant.group.member.{ordinal}")),
        )
    }

    pub(crate) fn artifact(self, slot: ArtifactSlot) -> SnapshotArtifactId {
        SnapshotArtifactId::from_uuid(self.derive(&slot.label()))
    }

    pub(crate) fn scene_message(self) -> MessageId {
        MessageId::from_uuid(self.derive("message.scene"))
    }

    pub(crate) fn scene_revision(self) -> MessageRevisionId {
        MessageRevisionId::from_uuid(self.derive("revision.scene"))
    }

    pub(crate) fn starter_message(self, ordinal: usize) -> MessageId {
        MessageId::from_uuid(self.derive(&format!("message.starter.{ordinal}")))
    }

    pub(crate) fn starter_revision(self, ordinal: usize) -> MessageRevisionId {
        MessageRevisionId::from_uuid(self.derive(&format!("revision.starter.{ordinal}")))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArtifactSlot {
    Character,
    Persona,
    Scene,
    Starter,
    Prompt,
    Model,
    Lorebook(usize),
    Group,
    GroupMember(usize),
    GroupModel(usize),
    GroupPrompt(usize),
}

impl ArtifactSlot {
    fn label(self) -> String {
        match self {
            Self::Character => "artifact.character".into(),
            Self::Persona => "artifact.persona".into(),
            Self::Scene => "artifact.scene".into(),
            Self::Starter => "artifact.starter".into(),
            Self::Prompt => "artifact.prompt".into(),
            Self::Model => "artifact.model".into(),
            Self::Lorebook(ordinal) => format!("artifact.lorebook.{ordinal}"),
            Self::Group => "artifact.group".into(),
            Self::GroupMember(ordinal) => format!("artifact.group.member.{ordinal}"),
            Self::GroupModel(ordinal) => format!("artifact.group.model.{ordinal}"),
            Self::GroupPrompt(ordinal) => format!("artifact.group.prompt.{ordinal}"),
        }
    }
}
