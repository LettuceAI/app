use std::collections::{BTreeMap, BTreeSet};

use lettuce_companions::{
    EmotionalState, GroupCompanionCheckpoint, GroupCompanionEffect, GroupCompanionMessageSource,
    GroupMemberRelationship,
};
use lettuce_types::{CharacterId, ConversationId, MessageId, PersonaId, TimestampMillis};
use serde::{Deserialize, Serialize};

pub const GROUP_COMPANION_BACKUP_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupCompanionBackup {
    pub version: u32,
    pub checkpoints: Vec<BackupGroupCompanionCheckpoint>,
    pub persona_applications: Vec<BackupGroupCompanionPersonaApplication>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupGroupCompanionCheckpoint {
    pub conversation_id: ConversationId,
    pub message_id: MessageId,
    pub character_id: CharacterId,
    pub source: GroupCompanionMessageSource,
    pub emotional_state: EmotionalState,
    pub member_relationships: Vec<GroupMemberRelationship>,
    pub effects: Vec<GroupCompanionEffect>,
    pub created_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupGroupCompanionPersonaApplication {
    pub conversation_id: ConversationId,
    pub message_id: MessageId,
    pub character_id: CharacterId,
    pub persona_key: String,
    pub applied_at: TimestampMillis,
}

impl BackupGroupCompanionCheckpoint {
    #[must_use]
    pub fn to_checkpoint(&self) -> GroupCompanionCheckpoint {
        GroupCompanionCheckpoint {
            message_id: self.message_id,
            character_id: self.character_id,
            source: self.source.clone(),
            emotional_state: self.emotional_state.clone(),
            member_relationships: self.member_relationships.clone(),
            effects: self.effects.clone(),
            created_at: self.created_at,
        }
    }
}

impl GroupCompanionBackup {
    pub fn canonicalize_and_validate(
        &mut self,
        authored: &crate::AuthoredProfileBackup,
        history: &crate::ConversationHistoryBackup,
    ) -> Result<(), GroupCompanionBackupError> {
        if self.version != GROUP_COMPANION_BACKUP_VERSION {
            return Err(GroupCompanionBackupError::InvalidData {
                reason: "backup version is not the supported version",
            });
        }
        self.checkpoints
            .sort_by_key(|checkpoint| (checkpoint.message_id, checkpoint.character_id));
        self.persona_applications.sort_by_key(|application| {
            (
                application.conversation_id,
                application.message_id,
                application.character_id,
            )
        });
        let messages = history
            .conversations
            .iter()
            .flat_map(|conversation| {
                conversation
                    .messages
                    .iter()
                    .map(|message| (message.message.id, conversation.aggregate.conversation.id))
            })
            .collect::<BTreeMap<_, _>>();
        let character_ids = authored
            .characters
            .iter()
            .map(|value| value.character.id)
            .collect::<BTreeSet<_>>();
        let mut checkpoint_keys = BTreeSet::new();
        for checkpoint in &self.checkpoints {
            if messages.get(&checkpoint.message_id) != Some(&checkpoint.conversation_id)
                || !character_ids.contains(&checkpoint.character_id)
                || !checkpoint_keys.insert((checkpoint.message_id, checkpoint.character_id))
                || checkpoint.to_checkpoint().validate().is_err()
            {
                return Err(GroupCompanionBackupError::InvalidData {
                    reason: "checkpoint references an unknown message or character, is duplicated or is invalid",
                });
            }
        }
        let mut application_keys = BTreeSet::new();
        for application in &self.persona_applications {
            if messages.get(&application.message_id) != Some(&application.conversation_id)
                || !character_ids.contains(&application.character_id)
                || (application.persona_key != "__default__"
                    && application.persona_key.parse::<PersonaId>().is_err())
                || !application_keys.insert((
                    application.message_id,
                    application.character_id,
                    application.persona_key.clone(),
                ))
            {
                return Err(GroupCompanionBackupError::InvalidData {
                    reason: "persona application references an unknown message or character, is duplicated or is invalid",
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum GroupCompanionBackupError {
    #[error("group companion backup is invalid: {reason}")]
    InvalidData { reason: &'static str },
}
