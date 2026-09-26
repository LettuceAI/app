use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use lettuce_companions::{
    CompanionEffectSourceWindow, CompanionMemoryChanges, CompanionTurnEffect,
    CompanionTurnEffectOutcome, CompanionTurnEffectRepository, CompanionTurnEffectRepositoryError,
    CompanionTurnEffectStatus, EmotionVector,
};
use lettuce_memory::{MemoryItem, MemorySpaceSnapshot};
use lettuce_types::{MessageId, TimestampMillis};

#[derive(Debug, Clone, Copy)]
pub struct CompanionPostTurnEffect<'a> {
    pub effect: &'a CompanionTurnEffect,
    pub enqueued_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompanionPostTurnEffectError {
    #[error("companion post-turn memory snapshots do not share one valid history")]
    InvalidSnapshots,
    #[error("companion post-turn effect input is invalid")]
    InvalidEffect,
    #[error("companion post-turn effect repository failed: {0:?}")]
    Repository(CompanionTurnEffectRepositoryError),
}

#[derive(Debug)]
pub struct CompanionPostTurnEffectCoordinator<'a, R: ?Sized> {
    repository: &'a R,
}

impl<'a, R: CompanionTurnEffectRepository + ?Sized> CompanionPostTurnEffectCoordinator<'a, R> {
    #[must_use]
    pub const fn new(repository: &'a R) -> Self {
        Self { repository }
    }

    pub fn settle_ready(
        &self,
        effects: &[CompanionPostTurnEffect<'_>],
        before: &MemorySpaceSnapshot,
        after: &MemorySpaceSnapshot,
        settled_at: TimestampMillis,
    ) -> Result<Vec<CompanionTurnEffect>, CompanionPostTurnEffectError> {
        if before.id != after.id || before.revision.get() > after.revision.get() {
            return Err(CompanionPostTurnEffectError::InvalidSnapshots);
        }
        before
            .validate()
            .map_err(|_| CompanionPostTurnEffectError::InvalidSnapshots)?;
        after
            .validate()
            .map_err(|_| CompanionPostTurnEffectError::InvalidSnapshots)?;
        let before_by_id = before
            .items
            .iter()
            .map(|memory| (memory.id, memory))
            .collect::<HashMap<_, _>>();
        let mut unique_effects = HashSet::with_capacity(effects.len());
        let mut settled = Vec::with_capacity(effects.len());

        for input in effects {
            let effect = input.effect;
            if !unique_effects.insert(effect.id)
                || input.enqueued_at > settled_at
                || !matches!(
                    effect.status,
                    CompanionTurnEffectStatus::Processing | CompanionTurnEffectStatus::Ready
                )
            {
                return Err(CompanionPostTurnEffectError::InvalidEffect);
            }
            let message_ids = effect_message_ids(effect);
            let memory_changes = memory_changes_for_turn(&message_ids, &before_by_id, after);
            let summary = summarize_turn_effect(effect, &memory_changes);
            let outcome = CompanionTurnEffectOutcome::Ready {
                summary,
                memory_changes,
                source_window: CompanionEffectSourceWindow {
                    message_ids,
                    enqueued_at: input.enqueued_at,
                },
            };
            settled.push(
                self.repository
                    .settle(effect.id, outcome, settled_at)
                    .map_err(CompanionPostTurnEffectError::Repository)?,
            );
        }
        Ok(settled)
    }
}

fn effect_message_ids(effect: &CompanionTurnEffect) -> Vec<MessageId> {
    effect
        .user_message_id
        .into_iter()
        .chain(std::iter::once(effect.assistant_message_id))
        .collect()
}

fn memory_changes_for_turn(
    message_ids: &[MessageId],
    before_by_id: &HashMap<lettuce_types::MemoryId, &MemoryItem>,
    after: &MemorySpaceSnapshot,
) -> CompanionMemoryChanges {
    let message_ids = message_ids.iter().copied().collect::<HashSet<_>>();
    let from_turn = |memory: &MemoryItem| {
        memory
            .source_message_id
            .is_some_and(|id| message_ids.contains(&id))
    };
    let mut changes = CompanionMemoryChanges::default();
    for memory in &after.items {
        let source_matches = from_turn(memory);
        match before_by_id.get(&memory.id) {
            None if source_matches => changes.added.push(memory.id),
            None => {}
            Some(previous) => {
                if source_matches && legacy_effect_fields_changed(previous, memory) {
                    changes.updated.push(memory.id);
                }
                if previous.superseded_at.is_none()
                    && memory.superseded_at.is_some()
                    && memory.superseded_by.is_some_and(|replacement| {
                        after
                            .items
                            .iter()
                            .find(|item| item.id == replacement)
                            .is_some_and(from_turn)
                    })
                {
                    changes.superseded.push(memory.id);
                }
            }
        }
    }
    changes
}

fn legacy_effect_fields_changed(previous: &MemoryItem, current: &MemoryItem) -> bool {
    previous.text != current.text
        || previous.category != current.category
        || previous.importance != current.importance
        || previous.prompt_importance != current.prompt_importance
        || previous.persistence_importance != current.persistence_importance
}

fn summarize_turn_effect(
    effect: &CompanionTurnEffect,
    memory_changes: &CompanionMemoryChanges,
) -> Option<String> {
    let mut parts = Vec::new();
    if let Some((key, value)) = largest_relationship_delta(effect) {
        parts.push(format!("{} {}", humanize_key(key), format_delta(value)));
    }
    if let Some((key, value)) = largest_emotion_delta(effect) {
        parts.push(format!("{} {}", humanize_key(&key), format_delta(value)));
    }
    let added_signals = effect.seed.signal_changes.added.len();
    if added_signals > 0 {
        parts.push(format!(
            "{} signal{}",
            added_signals,
            plural_suffix(added_signals)
        ));
    }
    if !memory_changes.added.is_empty() {
        parts.push(format!(
            "{} memory{} added",
            memory_changes.added.len(),
            plural_suffix(memory_changes.added.len())
        ));
    }
    if !memory_changes.superseded.is_empty() {
        parts.push(format!(
            "{} memory{} superseded",
            memory_changes.superseded.len(),
            plural_suffix(memory_changes.superseded.len())
        ));
    }
    (!parts.is_empty()).then(|| parts.into_iter().take(3).collect::<Vec<_>>().join(", "))
}

/// The deltas are compared in alphabetical key order, and the last of equal
/// largest values is kept.
fn largest_relationship_delta(effect: &CompanionTurnEffect) -> Option<(&'static str, f64)> {
    let delta = &effect.seed.relationship_delta;
    [
        ("affection", delta.affection),
        ("closeness", delta.closeness),
        ("stability", delta.stability),
        ("tension", delta.tension),
        ("trust", delta.trust),
    ]
    .into_iter()
    .max_by(compare_absolute_delta)
}

fn largest_emotion_delta(effect: &CompanionTurnEffect) -> Option<(String, f64)> {
    [
        ("blocked", &effect.seed.emotion_delta.blocked),
        ("expressed", &effect.seed.emotion_delta.expressed),
        ("felt", &effect.seed.emotion_delta.felt),
    ]
    .into_iter()
    .flat_map(|(group, vector)| {
        emotion_values(vector).map(move |(key, value)| (format!("{group}.{key}"), value))
    })
    .max_by(compare_absolute_delta)
}

/// The emotion dimensions under their camelCase keys, alphabetically.
fn emotion_values(value: &EmotionVector) -> std::array::IntoIter<(&'static str, f64), 10> {
    [
        ("affectionIntensity", value.affection_intensity),
        ("calm", value.calm),
        ("hurt", value.hurt),
        ("irritation", value.irritation),
        ("longing", value.longing),
        ("reassuranceNeed", value.reassurance_need),
        ("tension", value.tension),
        ("trust", value.trust),
        ("vulnerability", value.vulnerability),
        ("warmth", value.warmth),
    ]
    .into_iter()
}

fn compare_absolute_delta<T>((_, left): &(T, f64), (_, right): &(T, f64)) -> Ordering {
    left.abs()
        .partial_cmp(&right.abs())
        .unwrap_or(Ordering::Equal)
}

fn format_delta(value: f64) -> String {
    #[allow(clippy::cast_possible_truncation)]
    let percent = (value * 100.0).round() as i64;
    if percent >= 0 {
        format!("+{percent}%")
    } else {
        format!("{percent}%")
    }
}

fn humanize_key(key: &str) -> String {
    key.replace(['_', '.'], " ")
}

const fn plural_suffix(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use lettuce_companions::{
        CompanionEmotionDelta, CompanionSignalChanges, CompanionTurnEffectSeed, RelationshipDelta,
    };
    use lettuce_memory::{MemoryCategory, Score};
    use lettuce_types::{
        CompanionEffectId, ConversationId, GenerationTurnId, MemoryId, MemorySpaceId, Revision,
    };

    use super::*;

    #[derive(Debug, Default)]
    struct EffectRepository {
        effects: Mutex<HashMap<CompanionEffectId, CompanionTurnEffect>>,
    }

    impl EffectRepository {
        fn insert(&self, effect: CompanionTurnEffect) {
            self.effects
                .lock()
                .expect("effects")
                .insert(effect.id, effect);
        }
    }

    impl CompanionTurnEffectRepository for EffectRepository {
        fn get_for_message(
            &self,
            conversation_id: lettuce_types::ConversationId,
            assistant_message_id: MessageId,
        ) -> Result<Option<CompanionTurnEffect>, CompanionTurnEffectRepositoryError> {
            Ok(self
                .effects
                .lock()
                .expect("effects")
                .values()
                .find(|effect| {
                    effect.conversation_id == conversation_id
                        && effect.assistant_message_id == assistant_message_id
                })
                .cloned())
        }

        fn list_processing(
            &self,
            limit: u16,
        ) -> Result<Vec<CompanionTurnEffect>, CompanionTurnEffectRepositoryError> {
            Ok(self
                .effects
                .lock()
                .expect("effects")
                .values()
                .filter(|effect| effect.status == CompanionTurnEffectStatus::Processing)
                .take(usize::from(limit))
                .cloned()
                .collect())
        }

        fn settle(
            &self,
            effect_id: CompanionEffectId,
            outcome: CompanionTurnEffectOutcome,
            now: TimestampMillis,
        ) -> Result<CompanionTurnEffect, CompanionTurnEffectRepositoryError> {
            let mut effects = self.effects.lock().expect("effects");
            let effect = effects
                .get_mut(&effect_id)
                .ok_or(CompanionTurnEffectRepositoryError::NotFound)?;
            let exact = match (&effect.status, &outcome) {
                (
                    CompanionTurnEffectStatus::Ready,
                    CompanionTurnEffectOutcome::Ready {
                        summary,
                        memory_changes,
                        source_window,
                    },
                ) => {
                    effect.summary == *summary
                        && effect.memory_changes == *memory_changes
                        && effect.source_window.as_ref() == Some(source_window)
                }
                (
                    CompanionTurnEffectStatus::Failed,
                    CompanionTurnEffectOutcome::Failed { summary },
                ) => effect.summary.as_ref() == Some(summary),
                (CompanionTurnEffectStatus::Processing, _) => true,
                _ => false,
            };
            if !exact {
                return Err(CompanionTurnEffectRepositoryError::Conflict);
            }
            if effect.status == CompanionTurnEffectStatus::Processing {
                match outcome {
                    CompanionTurnEffectOutcome::Ready {
                        summary,
                        memory_changes,
                        source_window,
                    } => {
                        effect.status = CompanionTurnEffectStatus::Ready;
                        effect.summary = summary;
                        effect.memory_changes = memory_changes;
                        effect.source_window = Some(source_window);
                    }
                    CompanionTurnEffectOutcome::Failed { summary } => {
                        effect.status = CompanionTurnEffectStatus::Failed;
                        effect.summary = Some(summary);
                    }
                }
                effect.updated_at = now;
            }
            Ok(effect.clone())
        }
    }

    fn effect(
        conversation_id: ConversationId,
        user_message_id: Option<MessageId>,
        assistant_message_id: MessageId,
        seed: CompanionTurnEffectSeed,
    ) -> CompanionTurnEffect {
        CompanionTurnEffect {
            id: CompanionEffectId::new(),
            conversation_id,
            turn_id: GenerationTurnId::new(),
            user_message_id,
            assistant_message_id,
            status: CompanionTurnEffectStatus::Processing,
            summary: None,
            seed,
            memory_changes: CompanionMemoryChanges::default(),
            source_window: None,
            created_at: TimestampMillis::new(10),
            updated_at: TimestampMillis::new(10),
        }
    }

    fn memory(id: MemoryId, text: &str, source_message_id: Option<MessageId>) -> MemoryItem {
        MemoryItem {
            id,
            short_id: lettuce_memory::MemoryShortId::derived(id),
            text: text.to_owned(),
            category: MemoryCategory::Other,
            source_message_id,
            source_role: None,
            observed_at: None,
            observed_time_precision: None,
            superseded_by: None,
            superseded_at: None,
            supersedes: Vec::new(),
            token_count: 3,
            is_cold: false,
            is_pinned: false,
            importance: Score::FULL,
            persistence_importance: Score::FULL,
            prompt_importance: Score::FULL,
            volatility: Score::LEGACY_VOLATILITY,
            access_count: 0,
            created_at: TimestampMillis::new(10),
            last_accessed_at: TimestampMillis::new(10),
        }
    }

    fn snapshot(
        id: MemorySpaceId,
        revision: Revision,
        items: Vec<MemoryItem>,
    ) -> MemorySpaceSnapshot {
        MemorySpaceSnapshot {
            id,
            revision,
            items,
        }
    }

    #[test]
    fn coalesced_ready_settlement_uses_each_effect_source_window_and_legacy_summary() {
        let repository = EffectRepository::default();
        let coordinator = CompanionPostTurnEffectCoordinator::new(&repository);
        let conversation_id = ConversationId::new();
        let first_user = MessageId::new();
        let first_assistant = MessageId::new();
        let second_assistant = MessageId::new();
        let first = effect(
            conversation_id,
            Some(first_user),
            first_assistant,
            CompanionTurnEffectSeed {
                relationship_delta: RelationshipDelta {
                    trust: 0.12,
                    ..RelationshipDelta::default()
                },
                emotion_delta: CompanionEmotionDelta {
                    felt: EmotionVector {
                        hurt: -0.2,
                        ..EmotionVector::default()
                    },
                    ..CompanionEmotionDelta::default()
                },
                signal_changes: CompanionSignalChanges {
                    added: vec!["promise".to_owned()],
                    removed: vec![],
                },
            },
        );
        let second = effect(
            conversation_id,
            None,
            second_assistant,
            CompanionTurnEffectSeed::default(),
        );
        repository.insert(first.clone());
        repository.insert(second.clone());
        let space_id = MemorySpaceId::new();
        let updated_id = MemoryId::new();
        let first_added_id = MemoryId::new();
        let second_added_id = MemoryId::new();
        let unrelated_id = MemoryId::new();
        let replaced_id = MemoryId::new();
        let replaced = memory(replaced_id, "replaced", Some(MessageId::new()));
        let before = snapshot(
            space_id,
            Revision::INITIAL,
            vec![
                memory(updated_id, "before", Some(first_user)),
                replaced.clone(),
            ],
        );
        let after = snapshot(
            space_id,
            Revision::new(2),
            vec![
                memory(updated_id, "after", Some(first_user)),
                MemoryItem {
                    superseded_by: Some(second_added_id),
                    superseded_at: Some(TimestampMillis::new(25)),
                    ..replaced
                },
                memory(first_added_id, "first", Some(first_assistant)),
                memory(second_added_id, "second", Some(second_assistant)),
                memory(unrelated_id, "unrelated", Some(MessageId::new())),
            ],
        );
        let inputs = [
            CompanionPostTurnEffect {
                effect: &first,
                enqueued_at: TimestampMillis::new(20),
            },
            CompanionPostTurnEffect {
                effect: &second,
                enqueued_at: TimestampMillis::new(21),
            },
        ];

        let settled = coordinator
            .settle_ready(&inputs, &before, &after, TimestampMillis::new(30))
            .expect("settle coalesced effects");
        assert_eq!(settled[0].memory_changes.added, [first_added_id]);
        assert_eq!(settled[0].memory_changes.updated, [updated_id]);
        assert_eq!(
            settled[0].summary.as_deref(),
            Some("trust +12%, felt hurt -20%, 1 signal")
        );
        assert_eq!(
            settled[0]
                .source_window
                .as_ref()
                .map(|window| window.message_ids.as_slice()),
            Some([first_user, first_assistant].as_slice())
        );
        assert_eq!(settled[1].memory_changes.added, [second_added_id]);
        assert!(settled[0].memory_changes.superseded.is_empty());
        assert_eq!(settled[1].memory_changes.superseded, [replaced_id]);
        assert_eq!(
            settled[1].summary.as_deref(),
            Some("trust +0%, felt warmth +0%, 1 memory added")
        );

        let replay = coordinator
            .settle_ready(&inputs, &before, &after, TimestampMillis::new(30))
            .expect("exact replay");
        assert_eq!(replay, settled);
    }

    #[test]
    fn summary_reads_deltas_in_legacy_json_key_order_and_names() {
        let tie = effect(
            ConversationId::new(),
            None,
            MessageId::new(),
            CompanionTurnEffectSeed {
                relationship_delta: RelationshipDelta {
                    affection: 0.1,
                    closeness: -0.1,
                    ..RelationshipDelta::default()
                },
                emotion_delta: CompanionEmotionDelta {
                    blocked: EmotionVector {
                        affection_intensity: 0.3,
                        ..EmotionVector::default()
                    },
                    felt: EmotionVector {
                        warmth: -0.3,
                        ..EmotionVector::default()
                    },
                    ..CompanionEmotionDelta::default()
                },
                signal_changes: CompanionSignalChanges::default(),
            },
        );
        assert_eq!(
            summarize_turn_effect(&tie, &CompanionMemoryChanges::default()).as_deref(),
            Some("closeness -10%, felt warmth -30%")
        );
        let camel = effect(
            ConversationId::new(),
            None,
            MessageId::new(),
            CompanionTurnEffectSeed {
                emotion_delta: CompanionEmotionDelta {
                    blocked: EmotionVector {
                        affection_intensity: 0.3,
                        ..EmotionVector::default()
                    },
                    ..CompanionEmotionDelta::default()
                },
                ..CompanionTurnEffectSeed::default()
            },
        );
        assert_eq!(
            summarize_turn_effect(&camel, &CompanionMemoryChanges::default()).as_deref(),
            Some("trust +0%, blocked affectionIntensity +30%")
        );
    }

    #[test]
    fn no_op_effect_settles_ready() {
        let repository = EffectRepository::default();
        let coordinator = CompanionPostTurnEffectCoordinator::new(&repository);
        let ready = effect(
            ConversationId::new(),
            None,
            MessageId::new(),
            CompanionTurnEffectSeed::default(),
        );
        repository.insert(ready.clone());
        let space_id = MemorySpaceId::new();
        let snapshot = snapshot(space_id, Revision::INITIAL, vec![]);
        let settled = coordinator
            .settle_ready(
                &[CompanionPostTurnEffect {
                    effect: &ready,
                    enqueued_at: TimestampMillis::new(20),
                }],
                &snapshot,
                &snapshot,
                TimestampMillis::new(30),
            )
            .expect("settle no-op");
        assert_eq!(settled[0].status, CompanionTurnEffectStatus::Ready);
        assert_eq!(
            settled[0].summary.as_deref(),
            Some("trust +0%, felt warmth +0%")
        );
    }
}
