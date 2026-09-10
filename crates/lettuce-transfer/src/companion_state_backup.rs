use std::collections::{BTreeMap, BTreeSet};

use lettuce_companions::{
    CompanionContinuityEpisode, CompanionStateApplyReceipt, CompanionStateOwner, EmotionalState,
    RelationshipState, validate_runtime_state,
};
use lettuce_conversations::{ConversationKind, SnapshotSelection};
use lettuce_types::{CharacterId, ContentHash, PersonaId, Revision, TimestampMillis};
use serde::{Deserialize, Serialize};

pub const COMPANION_STATE_BACKUP_VERSION: u32 = 1;
pub const MAX_BACKUP_COMPANION_RELATIONSHIPS: usize = 100_000;
pub const MAX_BACKUP_COMPANION_SESSIONS: usize = 100_000;
pub const MAX_BACKUP_COMPANION_RECEIPTS: usize = 1_000_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompanionStateBackup {
    pub version: u32,
    pub relationships: Vec<BackupCompanionRelationship>,
    pub sessions: Vec<BackupCompanionSession>,
    pub episodes: Vec<CompanionContinuityEpisode>,
    pub receipts: Vec<BackupCompanionStateReceipt>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupCompanionRelationship {
    pub character_id: CharacterId,
    pub persona_id: Option<PersonaId>,
    pub state: RelationshipState,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupCompanionSession {
    pub owner: CompanionStateOwner,
    pub initial_state_hash: ContentHash,
    pub emotional_state: EmotionalState,
    pub active_signals: Vec<String>,
    pub state_updated_at: TimestampMillis,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupCompanionStateReceipt {
    pub receipt: CompanionStateApplyReceipt,
    pub change_hash: ContentHash,
}

impl CompanionStateBackup {
    pub fn canonicalize_and_validate(
        &mut self,
        authored: &crate::AuthoredProfileBackup,
        history: &crate::ConversationHistoryBackup,
    ) -> Result<(), CompanionStateBackupError> {
        if self.version != COMPANION_STATE_BACKUP_VERSION
            || self.relationships.len() > MAX_BACKUP_COMPANION_RELATIONSHIPS
            || self.sessions.len() > MAX_BACKUP_COMPANION_SESSIONS
            || self.episodes.len() > MAX_BACKUP_COMPANION_SESSIONS
            || self.receipts.len() > MAX_BACKUP_COMPANION_RECEIPTS
        {
            return Err(CompanionStateBackupError::InvalidData);
        }
        self.relationships
            .sort_by_key(|value| (value.character_id, value.persona_id));
        self.sessions
            .sort_by_key(|value| value.owner.conversation_id);
        self.episodes.sort_by_key(|value| value.conversation_id);
        self.receipts
            .sort_by_key(|value| (value.receipt.applied_at, value.receipt.operation_id));
        let character_ids = authored
            .characters
            .iter()
            .map(|value| value.character.id)
            .collect::<BTreeSet<_>>();
        let persona_ids = authored
            .personas
            .iter()
            .map(|value| value.id)
            .collect::<BTreeSet<_>>();
        let conversations = history
            .conversations
            .iter()
            .map(|value| {
                (
                    value.aggregate.conversation.id,
                    &value.aggregate.conversation.kind,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut relationships = BTreeMap::new();
        for relationship in &self.relationships {
            let key = (relationship.character_id, relationship.persona_id);
            if !character_ids.contains(&relationship.character_id)
                || relationship
                    .persona_id
                    .is_some_and(|id| !persona_ids.contains(&id))
                || relationship.revision.get() == 0
                || relationship.created_at > relationship.updated_at
                || !valid_relationship(&relationship.state)
                || relationships.insert(key, relationship).is_some()
            {
                return Err(CompanionStateBackupError::InvalidData);
            }
        }
        let mut sessions = BTreeMap::new();
        for session in &self.sessions {
            let owner = session.owner;
            let relationship = relationships
                .get(&(owner.character_id, owner.persona_id))
                .ok_or(CompanionStateBackupError::InvalidData)?;
            let kind = conversations
                .get(&owner.conversation_id)
                .ok_or(CompanionStateBackupError::InvalidData)?;
            let runtime = lettuce_companions::CompanionRuntimeState {
                emotional_state: session.emotional_state.clone(),
                relationship_state: relationship.state.clone(),
                active_signals: session.active_signals.clone(),
                updated_at: session.state_updated_at,
            };
            if !conversation_matches_owner(kind, owner)
                || session.revision.get() == 0
                || session.created_at > session.updated_at
                || session.emotional_state.updated_at > session.state_updated_at
                || validate_runtime_state(&runtime).is_err()
                || sessions.insert(owner.conversation_id, owner).is_some()
            {
                return Err(CompanionStateBackupError::InvalidData);
            }
        }
        if self.episodes.len() != self.sessions.len() {
            return Err(CompanionStateBackupError::InvalidData);
        }
        let mut episode_keys = BTreeSet::new();
        let mut episode_conversations = BTreeSet::new();
        for episode in &self.episodes {
            let owner = sessions
                .get(&episode.conversation_id)
                .ok_or(CompanionStateBackupError::InvalidData)?;
            if owner.character_id != episode.character_id
                || owner.persona_id != episode.persona_id
                || episode.episode_index == 0
                || episode.started_at > episode.updated_at
                || episode
                    .ended_at
                    .is_some_and(|at| at < episode.started_at || at > episode.updated_at)
                || episode
                    .previous_conversation_id
                    .is_some_and(|id| !sessions.contains_key(&id))
                || !episode_keys.insert((
                    episode.character_id,
                    episode.persona_id,
                    episode.episode_index,
                ))
                || !episode_conversations.insert(episode.conversation_id)
            {
                return Err(CompanionStateBackupError::InvalidData);
            }
        }
        let mut receipt_ids = BTreeSet::new();
        for value in &self.receipts {
            let receipt = &value.receipt;
            if sessions.get(&receipt.owner.conversation_id) != Some(&receipt.owner)
                || receipt.resulting_session_revision
                    != receipt
                        .expected_session_revision
                        .next()
                        .map_err(|_| CompanionStateBackupError::InvalidData)?
                || receipt.resulting_relationship_revision
                    != receipt
                        .expected_relationship_revision
                        .next()
                        .map_err(|_| CompanionStateBackupError::InvalidData)?
                || !receipt_ids.insert(receipt.operation_id)
            {
                return Err(CompanionStateBackupError::InvalidData);
            }
        }
        Ok(())
    }
}

fn valid_relationship(state: &RelationshipState) -> bool {
    [state.closeness, state.trust, state.affection]
        .into_iter()
        .all(|value| value.is_finite() && (-1.0..=1.0).contains(&value))
        && [state.tension, state.stability]
            .into_iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(&value))
}

fn conversation_matches_owner(kind: &ConversationKind, owner: CompanionStateOwner) -> bool {
    let ConversationKind::Direct(details) = kind else {
        return false;
    };
    if details.character.source_id != owner.character_id {
        return false;
    }
    let persona_id = match &details.persona {
        SnapshotSelection::Inherited(persona) | SnapshotSelection::Explicit(persona) => {
            Some(persona.source_id)
        }
        SnapshotSelection::Disabled => None,
    };
    persona_id == owner.persona_id
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CompanionStateBackupError {
    #[error("companion state backup is invalid")]
    InvalidData,
}
