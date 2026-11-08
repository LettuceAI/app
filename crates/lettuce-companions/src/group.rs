use std::collections::{BTreeMap, HashSet};

use lettuce_conversations::{
    ContinueConversation, ContinueConversationResult, ConversationRepositoryError,
    SendConversation, SendConversationResult,
};
use lettuce_types::{
    CharacterId, ConversationBranchId, ConversationId, MemorySpaceId, MessageCandidateId,
    MessageId, MessageRevisionId, PersonaId, Revision, TimestampMillis,
};
use serde::{Deserialize, Serialize};

use crate::{
    CompanionEmotionDelta, CompanionSignalBundle, CompanionSignalChanges, CompanionSoulConfig,
    EmotionalState, GroupCompanionMemberState, GroupCompanionStep, RelationshipDelta,
    RelationshipState,
};

const MAX_SIGNAL_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupMemberRelationship {
    pub target_character_id: CharacterId,
    pub state: RelationshipState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "snake_case")]
pub enum GroupCompanionTarget {
    Persona { persona_id: Option<PersonaId> },
    Character { character_id: CharacterId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupCompanionEffectReason {
    Witnessed,
    Addressed,
    SocialEvent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupCompanionRelationshipChange {
    pub target: GroupCompanionTarget,
    pub delta: RelationshipDelta,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupCompanionEffect {
    pub reason: GroupCompanionEffectReason,
    pub social_event: Option<String>,
    pub emotion_delta: CompanionEmotionDelta,
    pub relationship_change: Option<GroupCompanionRelationshipChange>,
    pub signal_changes: CompanionSignalChanges,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupCompanionMessageSource {
    pub revision_id: Option<MessageRevisionId>,
    pub candidate_id: Option<MessageCandidateId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroupCompanionCheckpoint {
    pub message_id: MessageId,
    pub character_id: CharacterId,
    pub source: GroupCompanionMessageSource,
    pub emotional_state: EmotionalState,
    pub member_relationships: Vec<GroupMemberRelationship>,
    pub effects: Vec<GroupCompanionEffect>,
    pub created_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupCompanionRepositoryError {
    Conflict,
    Invalid { reason: String },
    Corrupt { reason: String },
    Failure { reason: String },
}

pub trait GroupCompanionRepository: Send + Sync {
    /// `path` lists the active branch's message ids oldest first.
    fn latest_checkpoints(
        &self,
        conversation_id: ConversationId,
        path: &[MessageId],
    ) -> Result<Vec<GroupCompanionCheckpoint>, GroupCompanionRepositoryError>;

    fn checkpoints_for_message(
        &self,
        conversation_id: ConversationId,
        message_id: MessageId,
    ) -> Result<Vec<GroupCompanionCheckpoint>, GroupCompanionRepositoryError>;

    /// The message ids among `path` that have checkpoints, with the message source those checkpoints were computed from.
    fn path_checkpoint_sources(
        &self,
        conversation_id: ConversationId,
        path: &[MessageId],
    ) -> Result<Vec<(MessageId, GroupCompanionMessageSource)>, GroupCompanionRepositoryError>;

    fn persona_relationship(
        &self,
        character_id: CharacterId,
        persona_id: Option<PersonaId>,
    ) -> Result<Option<(Revision, RelationshipState)>, GroupCompanionRepositoryError>;

    fn recorded_persona_applications(
        &self,
        conversation_id: ConversationId,
        path: &[MessageId],
        persona_id: Option<PersonaId>,
    ) -> Result<Vec<(MessageId, CharacterId)>, GroupCompanionRepositoryError>;

    /// The member's perspective space on this branch, if one was created.
    fn perspective_space(
        &self,
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
        character_id: CharacterId,
    ) -> Result<Option<MemorySpaceId>, GroupCompanionRepositoryError>;

    /// The member's perspective space on this branch, created empty when missing.
    fn ensure_perspective_space(
        &self,
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
        character_id: CharacterId,
    ) -> Result<MemorySpaceId, GroupCompanionRepositoryError>;

    /// The character's direct-chat memory pool, only when it exists and the character shares memory across chats.
    fn companion_pool_space(
        &self,
        character_id: CharacterId,
    ) -> Result<Option<MemorySpaceId>, GroupCompanionRepositoryError>;

    /// Every (message, member) pair on `path` that has a checkpoint, i.e. who was present for which message.
    fn message_presence(
        &self,
        conversation_id: ConversationId,
        path: &[MessageId],
    ) -> Result<Vec<(MessageId, CharacterId)>, GroupCompanionRepositoryError>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroupCompanionUserEvent {
    pub present: Vec<CharacterId>,
    pub addressed: Vec<CharacterId>,
    pub signals: CompanionSignalBundle,
    pub at: TimestampMillis,
}

#[derive(Debug, Clone)]
pub struct PreparedGroupCompanionTurn {
    pub conversation_id: ConversationId,
    pub persona_id: Option<PersonaId>,
    pub members: Vec<(CharacterId, CompanionSoulConfig)>,
    pub applied: Vec<(MessageId, GroupCompanionStep)>,
    pub states: BTreeMap<CharacterId, GroupCompanionMemberState>,
    pub persona_relationships: BTreeMap<CharacterId, RelationshipState>,
    pub persona_revisions: BTreeMap<CharacterId, Option<Revision>>,
    pub user_event: Option<GroupCompanionUserEvent>,
}

pub trait GroupCompanionTurnWriter: Send + Sync {
    fn begin_group_companion_send(
        &self,
        command: &SendConversation,
        prepared: PreparedGroupCompanionTurn,
        now: TimestampMillis,
    ) -> Result<SendConversationResult, ConversationRepositoryError>;
    fn begin_group_companion_continue(
        &self,
        command: &ContinueConversation,
        prepared: PreparedGroupCompanionTurn,
        now: TimestampMillis,
    ) -> Result<ContinueConversationResult, ConversationRepositoryError>;
}

impl GroupCompanionCheckpoint {
    pub fn validate(&self) -> Result<(), GroupCompanionRepositoryError> {
        let emotional = &self.emotional_state;
        let momentum = &emotional.momentum;
        if !emotional.felt.is_unit()
            || !emotional.expressed.is_unit()
            || !emotional.blocked.is_unit()
            || [
                momentum.warmth,
                momentum.trust,
                momentum.calm,
                momentum.vulnerability,
                momentum.longing,
                momentum.hurt,
                momentum.tension,
                momentum.irritation,
                momentum.affection_intensity,
                momentum.reassurance_need,
            ]
            .into_iter()
            .any(|value| !value.is_finite() || !(-1.0..=1.0).contains(&value))
            || !emotional.confidence.is_finite()
            || !(0.0..=1.0).contains(&emotional.confidence)
        {
            return Err(invalid("the emotional state has an out-of-range value"));
        }
        if emotional
            .active_drivers
            .iter()
            .chain(self.effects.iter().flat_map(|effect| {
                effect
                    .signal_changes
                    .added
                    .iter()
                    .chain(&effect.signal_changes.removed)
            }))
            .any(|signal| signal.trim().is_empty() || signal.len() > MAX_SIGNAL_BYTES)
        {
            return Err(invalid("a signal is blank or longer than the limit"));
        }
        let mut targets = HashSet::new();
        if self.member_relationships.iter().any(|relationship| {
            let state = &relationship.state;
            relationship.target_character_id == self.character_id
                || !targets.insert(relationship.target_character_id)
                || !signed(state.closeness)
                || !signed(state.trust)
                || !signed(state.affection)
                || !unit(state.tension)
                || !unit(state.stability)
        }) {
            return Err(invalid(
                "a member relationship repeats a target, targets the owner or has an out-of-range value",
            ));
        }
        for effect in &self.effects {
            let social_event = effect.social_event.as_deref();
            let is_social = effect.reason == GroupCompanionEffectReason::SocialEvent;
            let valid_social = match social_event {
                Some(value) => !value.trim().is_empty(),
                None => false,
            };
            if is_social != social_event.is_some() || (is_social && !valid_social) {
                return Err(invalid(
                    "an effect's social event does not match its reason",
                ));
            }
            if effect.relationship_change.as_ref().is_some_and(|change| {
                matches!(
                    change.target,
                    GroupCompanionTarget::Character { character_id }
                        if character_id == self.character_id
                )
            }) {
                return Err(invalid("an effect targets its owner character"));
            }
        }
        Ok(())
    }
}

fn invalid(reason: &str) -> GroupCompanionRepositoryError {
    GroupCompanionRepositoryError::Invalid {
        reason: reason.to_owned(),
    }
}

fn signed(value: f64) -> bool {
    value.is_finite() && (-1.0..=1.0).contains(&value)
}

fn unit(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EmotionVector, RelationshipDelta, RelationshipState};

    fn relationship(target_character_id: CharacterId) -> GroupMemberRelationship {
        GroupMemberRelationship {
            target_character_id,
            state: RelationshipState {
                closeness: 0.2,
                trust: 0.4,
                affection: 0.1,
                tension: 0.05,
                stability: 0.8,
                interaction_count: 3,
                last_interaction_at: TimestampMillis::new(5),
            },
        }
    }

    fn effect(reason: GroupCompanionEffectReason) -> GroupCompanionEffect {
        GroupCompanionEffect {
            reason,
            social_event: (reason == GroupCompanionEffectReason::SocialEvent)
                .then(|| "party".into()),
            emotion_delta: CompanionEmotionDelta::default(),
            relationship_change: Some(GroupCompanionRelationshipChange {
                target: GroupCompanionTarget::Persona { persona_id: None },
                delta: RelationshipDelta::default(),
            }),
            signal_changes: CompanionSignalChanges {
                added: vec!["emotion:warmth".into()],
                removed: vec!["emotion:hurt".into()],
            },
        }
    }

    fn checkpoint() -> GroupCompanionCheckpoint {
        let character_id = CharacterId::new();
        GroupCompanionCheckpoint {
            message_id: MessageId::new(),
            character_id,
            source: GroupCompanionMessageSource {
                revision_id: Some(MessageRevisionId::new()),
                candidate_id: None,
            },
            emotional_state: EmotionalState {
                felt: EmotionVector {
                    warmth: 0.5,
                    ..EmotionVector::default()
                },
                expressed: EmotionVector::default(),
                blocked: EmotionVector::default(),
                momentum: EmotionVector {
                    hurt: -0.5,
                    ..EmotionVector::default()
                },
                active_drivers: vec!["emotion:warmth".into()],
                confidence: 0.7,
                updated_at: TimestampMillis::new(5),
            },
            member_relationships: vec![relationship(CharacterId::new())],
            effects: vec![
                effect(GroupCompanionEffectReason::Witnessed),
                effect(GroupCompanionEffectReason::SocialEvent),
            ],
            created_at: TimestampMillis::new(5),
        }
    }

    #[test]
    fn valid_checkpoint_passes() {
        assert_eq!(checkpoint().validate(), Ok(()));
    }

    #[test]
    fn emotional_state_must_be_in_range() {
        let mut value = checkpoint();
        value.emotional_state.felt.hurt = -0.1;
        assert!(matches!(
            value.validate(),
            Err(GroupCompanionRepositoryError::Invalid { .. })
        ));

        let mut value = checkpoint();
        value.emotional_state.momentum.hurt = 1.1;
        assert!(matches!(
            value.validate(),
            Err(GroupCompanionRepositoryError::Invalid { .. })
        ));

        let mut value = checkpoint();
        value.emotional_state.confidence = f64::NAN;
        assert!(matches!(
            value.validate(),
            Err(GroupCompanionRepositoryError::Invalid { .. })
        ));
    }

    #[test]
    fn signals_must_be_non_blank_and_bounded() {
        let mut value = checkpoint();
        value.emotional_state.active_drivers = vec!["  ".into()];
        assert!(matches!(
            value.validate(),
            Err(GroupCompanionRepositoryError::Invalid { .. })
        ));

        let mut value = checkpoint();
        value.effects[0].signal_changes.added = vec!["x".repeat(257)];
        assert!(matches!(
            value.validate(),
            Err(GroupCompanionRepositoryError::Invalid { .. })
        ));
    }

    #[test]
    fn member_relationships_must_be_unique_and_not_self() {
        let mut value = checkpoint();
        value.member_relationships[0].target_character_id = value.character_id;
        assert!(matches!(
            value.validate(),
            Err(GroupCompanionRepositoryError::Invalid { .. })
        ));

        let mut value = checkpoint();
        let duplicate = value.member_relationships[0].clone();
        value.member_relationships.push(duplicate);
        assert!(matches!(
            value.validate(),
            Err(GroupCompanionRepositoryError::Invalid { .. })
        ));

        let mut value = checkpoint();
        value.member_relationships[0].state.trust = 1.2;
        assert!(matches!(
            value.validate(),
            Err(GroupCompanionRepositoryError::Invalid { .. })
        ));
    }

    #[test]
    fn social_events_and_character_targets_must_match() {
        let mut value = checkpoint();
        value.effects[0].social_event = Some("party".into());
        assert!(matches!(
            value.validate(),
            Err(GroupCompanionRepositoryError::Invalid { .. })
        ));

        let mut value = checkpoint();
        value.effects[1].social_event = None;
        assert!(matches!(
            value.validate(),
            Err(GroupCompanionRepositoryError::Invalid { .. })
        ));

        let mut value = checkpoint();
        value.effects[0].relationship_change = Some(GroupCompanionRelationshipChange {
            target: GroupCompanionTarget::Character {
                character_id: value.character_id,
            },
            delta: RelationshipDelta::default(),
        });
        assert!(matches!(
            value.validate(),
            Err(GroupCompanionRepositoryError::Invalid { .. })
        ));
    }
}
