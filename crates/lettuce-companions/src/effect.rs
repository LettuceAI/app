use lettuce_types::{
    CompanionEffectId, ConversationId, GenerationTurnId, MemoryId, MessageId, TimestampMillis,
};

use crate::{CompanionTurnTransition, EmotionVector, RelationshipDelta};

const MAX_SUMMARY_BYTES: usize = 8 * 1024;
const MAX_SIGNAL_BYTES: usize = 256;
const MAX_SIGNAL_CHANGES: usize = 64;
const MAX_MEMORY_CHANGES: usize = 512;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CompanionEmotionDelta {
    pub felt: EmotionVector,
    pub expressed: EmotionVector,
    pub blocked: EmotionVector,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompanionSignalChanges {
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CompanionTurnEffectSeed {
    pub relationship_delta: RelationshipDelta,
    pub emotion_delta: CompanionEmotionDelta,
    pub signal_changes: CompanionSignalChanges,
}

impl CompanionTurnEffectSeed {
    #[must_use]
    pub fn from_transition(transition: &CompanionTurnTransition) -> Self {
        Self {
            relationship_delta: RelationshipDelta {
                closeness: transition.current.relationship_state.closeness
                    - transition.effect_baseline.relationship_state.closeness,
                trust: transition.current.relationship_state.trust
                    - transition.effect_baseline.relationship_state.trust,
                affection: transition.current.relationship_state.affection
                    - transition.effect_baseline.relationship_state.affection,
                tension: transition.current.relationship_state.tension
                    - transition.effect_baseline.relationship_state.tension,
                stability: transition.current.relationship_state.stability
                    - transition.effect_baseline.relationship_state.stability,
            },
            emotion_delta: CompanionEmotionDelta {
                felt: vector_delta(
                    &transition.effect_baseline.emotional_state.felt,
                    &transition.current.emotional_state.felt,
                ),
                expressed: vector_delta(
                    &transition.effect_baseline.emotional_state.expressed,
                    &transition.current.emotional_state.expressed,
                ),
                blocked: vector_delta(
                    &transition.effect_baseline.emotional_state.blocked,
                    &transition.current.emotional_state.blocked,
                ),
            },
            signal_changes: CompanionSignalChanges {
                added: transition
                    .current
                    .active_signals
                    .iter()
                    .filter(|signal| !transition.previous.active_signals.contains(signal))
                    .cloned()
                    .collect(),
                removed: transition
                    .previous
                    .active_signals
                    .iter()
                    .filter(|signal| !transition.current.active_signals.contains(signal))
                    .cloned()
                    .collect(),
            },
        }
    }

    pub fn validate(&self) -> Result<(), CompanionTurnEffectRepositoryError> {
        let relationship = &self.relationship_delta;
        let relationship_values = [
            relationship.closeness,
            relationship.trust,
            relationship.affection,
            relationship.tension,
            relationship.stability,
        ];
        if relationship_values
            .into_iter()
            .any(|value| !value.is_finite() || !(-2.0..=2.0).contains(&value))
            || [
                &self.emotion_delta.felt,
                &self.emotion_delta.expressed,
                &self.emotion_delta.blocked,
            ]
            .into_iter()
            .flat_map(emotion_values)
            .any(|value| !value.is_finite() || !(-1.0..=1.0).contains(&value))
            || self.signal_changes.added.len() + self.signal_changes.removed.len()
                > MAX_SIGNAL_CHANGES
            || self
                .signal_changes
                .added
                .iter()
                .chain(&self.signal_changes.removed)
                .any(|signal| signal.trim().is_empty() || signal.len() > MAX_SIGNAL_BYTES)
        {
            return Err(CompanionTurnEffectRepositoryError::Invalid);
        }
        let mut signals = std::collections::HashSet::new();
        if !self
            .signal_changes
            .added
            .iter()
            .chain(&self.signal_changes.removed)
            .all(|signal| signals.insert(signal))
        {
            return Err(CompanionTurnEffectRepositoryError::Invalid);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompanionTurnEffectStatus {
    Processing,
    Ready,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompanionMemoryChanges {
    pub added: Vec<MemoryId>,
    pub updated: Vec<MemoryId>,
    pub superseded: Vec<MemoryId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanionEffectSourceWindow {
    pub message_ids: Vec<MessageId>,
    pub enqueued_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompanionTurnEffect {
    pub id: CompanionEffectId,
    pub conversation_id: ConversationId,
    pub turn_id: GenerationTurnId,
    pub user_message_id: Option<MessageId>,
    pub assistant_message_id: MessageId,
    pub status: CompanionTurnEffectStatus,
    pub summary: Option<String>,
    pub seed: CompanionTurnEffectSeed,
    pub memory_changes: CompanionMemoryChanges,
    pub source_window: Option<CompanionEffectSourceWindow>,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompanionTurnEffectOutcome {
    Ready {
        summary: Option<String>,
        memory_changes: CompanionMemoryChanges,
        source_window: CompanionEffectSourceWindow,
    },
    Failed {
        summary: String,
    },
}

impl CompanionTurnEffectOutcome {
    pub fn validate(&self) -> Result<(), CompanionTurnEffectRepositoryError> {
        let summary = match self {
            Self::Ready { summary, .. } => summary.as_deref(),
            Self::Failed { summary } => Some(summary.as_str()),
        };
        if summary.is_some_and(|value| value.len() > MAX_SUMMARY_BYTES)
            || matches!(self, Self::Failed { summary } if summary.trim().is_empty())
        {
            return Err(CompanionTurnEffectRepositoryError::Invalid);
        }
        if let Self::Ready {
            memory_changes,
            source_window,
            ..
        } = self
        {
            let memories = memory_changes
                .added
                .iter()
                .chain(&memory_changes.updated)
                .chain(&memory_changes.superseded);
            let mut unique_memories = std::collections::HashSet::new();
            if memory_changes.added.len()
                + memory_changes.updated.len()
                + memory_changes.superseded.len()
                > MAX_MEMORY_CHANGES
                || !memories.into_iter().all(|id| unique_memories.insert(id))
                || source_window.message_ids.len() > 2
            {
                return Err(CompanionTurnEffectRepositoryError::Invalid);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompanionTurnEffectRepositoryError {
    NotFound,
    Conflict,
    Invalid,
    Corrupt,
    Failure,
}

pub trait CompanionTurnEffectRepository: Send + Sync {
    fn get_for_message(
        &self,
        conversation_id: ConversationId,
        assistant_message_id: MessageId,
    ) -> Result<Option<CompanionTurnEffect>, CompanionTurnEffectRepositoryError>;

    /// Returns durable pending work in stable conversation/time/effect order.
    /// Processing effects are the restart-safe queue authority; runtime job
    /// stores may be rebuilt from this list after process loss.
    fn list_processing(
        &self,
        limit: u16,
    ) -> Result<Vec<CompanionTurnEffect>, CompanionTurnEffectRepositoryError>;

    fn settle(
        &self,
        effect_id: CompanionEffectId,
        outcome: CompanionTurnEffectOutcome,
        now: TimestampMillis,
    ) -> Result<CompanionTurnEffect, CompanionTurnEffectRepositoryError>;
}

fn vector_delta(previous: &EmotionVector, current: &EmotionVector) -> EmotionVector {
    EmotionVector {
        warmth: current.warmth - previous.warmth,
        trust: current.trust - previous.trust,
        calm: current.calm - previous.calm,
        vulnerability: current.vulnerability - previous.vulnerability,
        longing: current.longing - previous.longing,
        hurt: current.hurt - previous.hurt,
        tension: current.tension - previous.tension,
        irritation: current.irritation - previous.irritation,
        affection_intensity: current.affection_intensity - previous.affection_intensity,
        reassurance_need: current.reassurance_need - previous.reassurance_need,
    }
}

fn emotion_values(value: &EmotionVector) -> [f64; 10] {
    [
        value.warmth,
        value.trust,
        value.calm,
        value.vulnerability,
        value.longing,
        value.hurt,
        value.tension,
        value.irritation,
        value.affection_intensity,
        value.reassurance_need,
    ]
}

#[cfg(test)]
mod tests {
    use lettuce_types::TimestampMillis;

    use super::*;
    use crate::{
        CompanionTurnInput, RegulationStyle, RelationshipDefaults, apply_turn,
        initial_runtime_state,
    };

    #[test]
    fn effect_seed_copies_legacy_baseline_deltas_and_signal_order() {
        let mut previous = initial_runtime_state(
            &EmotionVector {
                warmth: 0.4,
                calm: 0.5,
                ..EmotionVector::default()
            },
            &RegulationStyle::default(),
            &RelationshipDefaults::default(),
        );
        previous.active_signals = vec!["repair_needed".into(), "old".into()];
        let transition = apply_turn(
            &previous,
            &EmotionVector::default(),
            &RegulationStyle::default(),
            &RelationshipDefaults::default(),
            &CompanionTurnInput {
                signals: vec!["repair_needed".into(), "warmth".into()],
                emotion_delta: EmotionVector {
                    warmth: 0.2,
                    hurt: 0.1,
                    ..EmotionVector::default()
                },
                relationship_delta: RelationshipDelta {
                    closeness: 0.1,
                    tension: 0.2,
                    ..RelationshipDelta::default()
                },
                confidence: 0.8,
                now: TimestampMillis::new(60_000),
            },
        );
        let seed = CompanionTurnEffectSeed::from_transition(&transition);

        assert_eq!(seed.signal_changes.added, ["warmth"]);
        assert_eq!(seed.signal_changes.removed, ["old"]);
        assert_eq!(
            seed.relationship_delta.closeness,
            transition.current.relationship_state.closeness
                - transition.effect_baseline.relationship_state.closeness
        );
        assert_eq!(
            seed.emotion_delta.felt.warmth,
            transition.current.emotional_state.felt.warmth
                - transition.effect_baseline.emotional_state.felt.warmth
        );
        seed.validate().expect("valid copied seed");
    }
}
