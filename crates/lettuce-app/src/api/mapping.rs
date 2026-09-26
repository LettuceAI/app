//! Domain values to contract DTOs.

use std::collections::HashMap;

use lettuce_characters::{Character, CharacterMediaSlot, CharacterRepository};
use lettuce_contracts as dto;
use lettuce_conversations::{
    ConversationKind, ConversationKindTag, ConversationParticipant, MediaAssetRole, MessagePart,
    MessageRenderSource, MessageRole, ParticipantRole, ParticipantSource, TimelineItem,
};
use lettuce_types::{AssetId, CharacterId, MessageId};

pub(crate) fn asset_ref(asset_id: AssetId) -> dto::AssetRef {
    dto::AssetRef {
        asset_id: asset_id.to_string(),
    }
}

pub(crate) fn character_avatar(character: &Character) -> Option<AssetId> {
    character
        .media
        .links
        .iter()
        .find(|link| link.slot == CharacterMediaSlot::AvatarOriginal)
        .map(|link| link.asset_id)
}

/// Character avatars read once per request.
#[derive(Debug, Default)]
pub(crate) struct AvatarLookup {
    avatars: HashMap<CharacterId, Option<AssetId>>,
}

impl AvatarLookup {
    pub(crate) fn avatar<R: CharacterRepository + ?Sized>(
        &mut self,
        repository: &R,
        character_id: CharacterId,
    ) -> Result<Option<AssetId>, lettuce_characters::RepositoryError> {
        if let Some(avatar) = self.avatars.get(&character_id) {
            return Ok(*avatar);
        }
        let avatar = CharacterRepository::get(repository, character_id)?
            .and_then(|details| character_avatar(&details.character));
        self.avatars.insert(character_id, avatar);
        Ok(avatar)
    }

    pub(crate) fn participant<R: CharacterRepository + ?Sized>(
        &mut self,
        repository: &R,
        participant: &ConversationParticipant,
    ) -> Result<dto::ParticipantView, lettuce_characters::RepositoryError> {
        let character_id = match participant.source {
            ParticipantSource::Character(id) => Some(id),
            ParticipantSource::User | ParticipantSource::System => None,
        };
        let avatar = match character_id {
            Some(id) => self.avatar(repository, id)?,
            None => None,
        };
        Ok(dto::ParticipantView {
            id: participant.id.to_string(),
            role: participant_role(participant.role),
            name: participant.display_name.clone(),
            character_id: character_id.map(|id| id.to_string()),
            avatar: avatar.map(asset_ref),
        })
    }
}

pub(crate) const fn participant_role(role: ParticipantRole) -> dto::ParticipantRole {
    match role {
        ParticipantRole::User => dto::ParticipantRole::User,
        ParticipantRole::Character => dto::ParticipantRole::Character,
        ParticipantRole::System => dto::ParticipantRole::System,
    }
}

pub(crate) const fn conversation_kind_tag(kind: ConversationKindTag) -> dto::ConversationKind {
    match kind {
        ConversationKindTag::Direct => dto::ConversationKind::Direct,
        ConversationKindTag::Group => dto::ConversationKind::Group,
    }
}

pub(crate) const fn conversation_kind(kind: &ConversationKind) -> dto::ConversationKind {
    match kind {
        ConversationKind::Direct(_) => dto::ConversationKind::Direct,
        ConversationKind::Group(_) => dto::ConversationKind::Group,
    }
}

const fn message_role(role: MessageRole) -> dto::MessageRole {
    match role {
        MessageRole::User => dto::MessageRole::User,
        MessageRole::Assistant => dto::MessageRole::Assistant,
        MessageRole::System => dto::MessageRole::System,
        MessageRole::Scene => dto::MessageRole::Scene,
    }
}

const fn media_role(role: MediaAssetRole) -> dto::MediaRole {
    match role {
        MediaAssetRole::Inline => dto::MediaRole::Inline,
        MediaAssetRole::Attachment => dto::MediaRole::Attachment,
        MediaAssetRole::Avatar => dto::MediaRole::Avatar,
        MediaAssetRole::Scene => dto::MediaRole::Scene,
        MediaAssetRole::Reference => dto::MediaRole::Reference,
    }
}

/// The parts the message currently shows: its active revision or its
/// selected candidate.
pub(crate) fn shown_parts(item: &TimelineItem) -> &[MessagePart] {
    let parts = match item.message.active_render_source {
        MessageRenderSource::Revision(_) => item.active_revision.as_ref().map(|value| &value.parts),
        MessageRenderSource::Candidate(_) => {
            item.active_candidate.as_ref().map(|value| &value.parts)
        }
    };
    parts.map_or(&[], Vec::as_slice)
}

pub(crate) fn shown_text(item: &TimelineItem) -> Option<String> {
    let text = shown_parts(item)
        .iter()
        .filter_map(|part| match part {
            MessagePart::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    (!text.trim().is_empty()).then_some(text)
}

pub(crate) fn timeline_message(
    item: &TimelineItem,
    candidate_counts: &HashMap<MessageId, u32>,
) -> dto::TimelineMessage {
    let mut parts = Vec::new();
    let mut reasoning = Vec::new();
    for part in shown_parts(item) {
        match part {
            MessagePart::Text { text } => {
                parts.push(dto::MessagePartView::Text { text: text.clone() })
            }
            MessagePart::MediaAsset { asset_id, role } => parts.push(dto::MessagePartView::Media {
                asset: asset_ref(*asset_id),
                role: media_role(*role),
            }),
            MessagePart::ReasoningSummary { text } => reasoning.push(text.as_str()),
            MessagePart::ToolCall { .. }
            | MessagePart::ToolResult { .. }
            | MessagePart::Annotation { .. } => {}
        }
    }
    dto::TimelineMessage {
        id: item.message.id.to_string(),
        role: message_role(item.message.role),
        author_participant_id: item.message.author_participant_id.map(|id| id.to_string()),
        parts,
        reasoning: (!reasoning.is_empty()).then(|| reasoning.join("\n")),
        created_at: item.message.created_at.get(),
        candidate_index: item
            .active_candidate
            .as_ref()
            .map(|candidate| candidate.ordinal),
        candidate_count: candidate_counts
            .get(&item.message.id)
            .copied()
            .unwrap_or_default(),
    }
}
