use std::collections::BTreeMap;

use lettuce_types::{CharacterId, MessageId, PersonaId, TimestampMillis};

use crate::{
    CompanionRuntimeState, CompanionSignalBundle, CompanionSoulConfig, CompanionTurnEffectSeed,
    CompanionTurnInput, EmotionalState, GroupCompanionCheckpoint, GroupCompanionEffect,
    GroupCompanionEffectReason, GroupCompanionMessageSource, GroupCompanionRelationshipChange,
    GroupCompanionTarget, GroupMemberRelationship, RelationshipAxis, RelationshipDelta,
    RelationshipReading, RelationshipState, TensionReading, apply_passive_decay, apply_turn,
    initial_runtime_state,
};

#[derive(Debug, Clone, Copy)]
pub struct GroupCompanionMember<'a> {
    pub character_id: CharacterId,
    pub config: &'a CompanionSoulConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupCompanionAuthor {
    Persona { persona_id: Option<PersonaId> },
    Member { character_id: CharacterId },
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroupCompanionEvent {
    pub message_id: MessageId,
    pub source: GroupCompanionMessageSource,
    pub author: GroupCompanionAuthor,
    /// The members that witnessed the event.
    pub present: Vec<CharacterId>,
    /// The present members the event was directed at.
    pub addressed: Vec<CharacterId>,
    pub signals: CompanionSignalBundle,
    pub at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroupCompanionMemberState {
    pub emotional_state: EmotionalState,
    pub member_relationships: Vec<GroupMemberRelationship>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct GroupCompanionStep {
    pub checkpoints: Vec<GroupCompanionCheckpoint>,
    /// The updated persona relationship of each addressed member, in member order.
    pub persona_relationships: Vec<(CharacterId, RelationshipState)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroupMemberStance {
    pub target_character_id: CharacterId,
    pub closeness: RelationshipReading,
    pub trust: RelationshipReading,
    pub affection: RelationshipReading,
    pub tension: TensionReading,
}

#[must_use]
pub fn initial_group_member_state(config: &CompanionSoulConfig) -> GroupCompanionMemberState {
    GroupCompanionMemberState {
        emotional_state: initial_runtime_state(
            &config.soul.baseline_affect,
            &config.soul.regulation_style,
            &config.relationship_defaults,
        )
        .emotional_state,
        member_relationships: Vec::new(),
    }
}

#[must_use]
pub fn initial_group_relationship(config: &CompanionSoulConfig) -> RelationshipState {
    initial_runtime_state(
        &config.soul.baseline_affect,
        &config.soul.regulation_style,
        &config.relationship_defaults,
    )
    .relationship_state
}

pub fn reduce_group_event(
    members: &[GroupCompanionMember<'_>],
    states: &mut BTreeMap<CharacterId, GroupCompanionMemberState>,
    persona_relationships: &mut BTreeMap<CharacterId, RelationshipState>,
    event: &GroupCompanionEvent,
) -> GroupCompanionStep {
    let mut step = GroupCompanionStep::default();
    for member in members {
        let character_id = member.character_id;
        if !event.present.contains(&character_id) {
            continue;
        }
        let config = member.config;
        let state = states
            .entry(character_id)
            .or_insert_with(|| initial_group_member_state(config));
        if matches!(
            event.author,
            GroupCompanionAuthor::Member { character_id: author } if author == character_id
        ) {
            step.checkpoints.push(GroupCompanionCheckpoint {
                message_id: event.message_id,
                character_id,
                source: event.source.clone(),
                emotional_state: state.emotional_state.clone(),
                member_relationships: state.member_relationships.clone(),
                effects: Vec::new(),
                created_at: event.at,
            });
            continue;
        }
        let addressed = event.addressed.contains(&character_id);
        let baseline = match &event.author {
            GroupCompanionAuthor::Persona { .. } => persona_relationships
                .get(&character_id)
                .cloned()
                .unwrap_or_else(|| initial_group_relationship(config)),
            GroupCompanionAuthor::Member {
                character_id: author,
            } => state
                .member_relationships
                .iter()
                .find(|relationship| relationship.target_character_id == *author)
                .map(|relationship| relationship.state.clone())
                .unwrap_or_else(|| initial_group_relationship(config)),
        };
        let runtime = CompanionRuntimeState {
            emotional_state: state.emotional_state.clone(),
            relationship_state: baseline,
            active_signals: state.emotional_state.active_drivers.clone(),
            updated_at: state.emotional_state.updated_at,
        };
        let input = CompanionTurnInput {
            signals: event.signals.signals.clone(),
            emotion_delta: event.signals.emotion_delta.clone(),
            relationship_delta: if addressed {
                event.signals.relationship_delta.clone()
            } else {
                RelationshipDelta::default()
            },
            confidence: event.signals.confidence,
            now: event.at,
        };
        let transition = apply_turn(
            &runtime,
            &config.soul.baseline_affect,
            &config.soul.regulation_style,
            &config.relationship_defaults,
            &input,
        );
        state.emotional_state = transition.current.emotional_state.clone();
        if addressed {
            let relationship = transition.current.relationship_state.clone();
            match &event.author {
                GroupCompanionAuthor::Persona { .. } => {
                    persona_relationships.insert(character_id, relationship.clone());
                    step.persona_relationships
                        .push((character_id, relationship));
                }
                GroupCompanionAuthor::Member {
                    character_id: author,
                } => {
                    upsert_member_relationship(
                        &mut state.member_relationships,
                        *author,
                        relationship,
                    );
                }
            }
        }
        let seed = CompanionTurnEffectSeed::from_transition(&transition);
        let target = match &event.author {
            GroupCompanionAuthor::Persona { persona_id } => GroupCompanionTarget::Persona {
                persona_id: *persona_id,
            },
            GroupCompanionAuthor::Member { character_id } => GroupCompanionTarget::Character {
                character_id: *character_id,
            },
        };
        let effect = GroupCompanionEffect {
            reason: if addressed {
                GroupCompanionEffectReason::Addressed
            } else {
                GroupCompanionEffectReason::Witnessed
            },
            social_event: None,
            emotion_delta: seed.emotion_delta,
            relationship_change: addressed.then(|| GroupCompanionRelationshipChange {
                target,
                delta: seed.relationship_delta,
            }),
            signal_changes: seed.signal_changes,
            by_user: false,
        };
        step.checkpoints.push(GroupCompanionCheckpoint {
            message_id: event.message_id,
            character_id,
            source: event.source.clone(),
            emotional_state: state.emotional_state.clone(),
            member_relationships: state.member_relationships.clone(),
            effects: vec![effect],
            created_at: event.at,
        });
    }
    step
}

#[must_use]
pub fn group_speaker_runtime_state(
    state: &GroupCompanionMemberState,
    persona_relationship: &RelationshipState,
    config: &CompanionSoulConfig,
    now: TimestampMillis,
) -> CompanionRuntimeState {
    let runtime = CompanionRuntimeState {
        emotional_state: state.emotional_state.clone(),
        relationship_state: persona_relationship.clone(),
        active_signals: state.emotional_state.active_drivers.clone(),
        updated_at: state.emotional_state.updated_at,
    };
    apply_passive_decay(
        &runtime,
        &config.soul.baseline_affect,
        &config.soul.regulation_style,
        now,
    )
}

#[must_use]
pub fn group_member_stances(state: &GroupCompanionMemberState) -> Vec<GroupMemberStance> {
    state
        .member_relationships
        .iter()
        .map(|relationship| GroupMemberStance {
            target_character_id: relationship.target_character_id,
            closeness: RelationshipReading::of(
                RelationshipAxis::Closeness,
                relationship.state.closeness,
            ),
            trust: RelationshipReading::of(RelationshipAxis::Trust, relationship.state.trust),
            affection: RelationshipReading::of(
                RelationshipAxis::Affection,
                relationship.state.affection,
            ),
            tension: TensionReading::of(relationship.state.tension),
        })
        .collect()
}

fn upsert_member_relationship(
    relationships: &mut Vec<GroupMemberRelationship>,
    target_character_id: CharacterId,
    state: RelationshipState,
) {
    match relationships.binary_search_by_key(&target_character_id, |relationship| {
        relationship.target_character_id
    }) {
        Ok(index) => relationships[index].state = state,
        Err(index) => relationships.insert(
            index,
            GroupMemberRelationship {
                target_character_id,
                state,
            },
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EmotionClassification, EmotionLabelScore, RelationshipBand, TensionBand,
        signals_from_classification,
    };

    fn config() -> CompanionSoulConfig {
        CompanionSoulConfig::default()
    }

    fn member(character_id: CharacterId, config: &CompanionSoulConfig) -> GroupCompanionMember<'_> {
        GroupCompanionMember {
            character_id,
            config,
        }
    }

    fn bundle(label: &str, score: f32) -> CompanionSignalBundle {
        signals_from_classification(&EmotionClassification {
            labels: vec![EmotionLabelScore {
                label: label.into(),
                score,
                threshold: 0.0,
            }],
            confidence: 1.0,
        })
    }

    fn event(
        author: GroupCompanionAuthor,
        present: Vec<CharacterId>,
        addressed: Vec<CharacterId>,
        signals: CompanionSignalBundle,
        at: TimestampMillis,
    ) -> GroupCompanionEvent {
        GroupCompanionEvent {
            message_id: MessageId::new(),
            source: GroupCompanionMessageSource {
                revision_id: None,
                candidate_id: None,
            },
            author,
            present,
            addressed,
            signals,
            at,
        }
    }

    #[test]
    fn addressed_persona_message_updates_only_the_addressee_relationship() {
        let config = config();
        let a = CharacterId::new();
        let b = CharacterId::new();
        let members = [member(a, &config), member(b, &config)];
        let mut states = BTreeMap::new();
        let mut persona_relationships = BTreeMap::new();
        let event = event(
            GroupCompanionAuthor::Persona { persona_id: None },
            vec![a, b],
            vec![a],
            bundle("love", 1.0),
            TimestampMillis::new(60_000),
        );
        let step = reduce_group_event(&members, &mut states, &mut persona_relationships, &event);

        assert_eq!(step.checkpoints.len(), 2);
        let a_checkpoint = &step.checkpoints[0];
        assert_eq!(a_checkpoint.character_id, a);
        assert_eq!(
            a_checkpoint.effects[0].reason,
            GroupCompanionEffectReason::Addressed
        );
        assert!(a_checkpoint.effects[0].relationship_change.is_some());
        assert_eq!(step.persona_relationships.len(), 1);
        assert_eq!(step.persona_relationships[0].0, a);
        assert_eq!(
            persona_relationships.get(&a),
            Some(&step.persona_relationships[0].1)
        );

        let b_checkpoint = &step.checkpoints[1];
        assert_eq!(b_checkpoint.character_id, b);
        assert_eq!(
            b_checkpoint.effects[0].reason,
            GroupCompanionEffectReason::Witnessed
        );
        assert!(b_checkpoint.effects[0].relationship_change.is_none());
        assert!(persona_relationships.get(&b).is_none());
        assert_ne!(
            b_checkpoint.emotional_state.felt,
            initial_group_member_state(&config).emotional_state.felt
        );
    }

    #[test]
    fn member_message_updates_the_addressee_member_relationship_only() {
        let config = config();
        let a = CharacterId::new();
        let b = CharacterId::new();
        let c = CharacterId::new();
        let members = [member(a, &config), member(b, &config), member(c, &config)];
        let mut states = BTreeMap::new();
        let mut persona_relationships = BTreeMap::new();
        let event = event(
            GroupCompanionAuthor::Member { character_id: a },
            vec![a, b, c],
            vec![b],
            bundle("caring", 1.0),
            TimestampMillis::new(60_000),
        );
        let step = reduce_group_event(&members, &mut states, &mut persona_relationships, &event);

        assert_eq!(step.checkpoints.len(), 3);
        let a_checkpoint = &step.checkpoints[0];
        assert_eq!(a_checkpoint.character_id, a);
        assert!(a_checkpoint.effects.is_empty());
        assert_eq!(
            a_checkpoint.emotional_state,
            initial_group_member_state(&config).emotional_state
        );

        let b_state = &states[&b];
        assert_eq!(b_state.member_relationships.len(), 1);
        assert_eq!(b_state.member_relationships[0].target_character_id, a);

        let c_state = &states[&c];
        assert!(c_state.member_relationships.is_empty());

        let b_checkpoint = step
            .checkpoints
            .iter()
            .find(|checkpoint| checkpoint.character_id == b)
            .expect("addressee checkpoint");
        assert_eq!(
            b_checkpoint.effects[0].reason,
            GroupCompanionEffectReason::Addressed
        );
        assert_eq!(
            b_checkpoint.effects[0]
                .relationship_change
                .as_ref()
                .expect("relationship change")
                .target,
            GroupCompanionTarget::Character { character_id: a }
        );
    }

    #[test]
    fn absent_members_keep_their_state_and_get_no_checkpoint() {
        let config = config();
        let a = CharacterId::new();
        let absent = CharacterId::new();
        let members = [member(a, &config), member(absent, &config)];
        let mut states = BTreeMap::new();
        let mut original = initial_group_member_state(&config);
        original.emotional_state.felt.warmth = 0.9;
        states.insert(absent, original.clone());
        let mut persona_relationships = BTreeMap::new();
        let event = event(
            GroupCompanionAuthor::Persona { persona_id: None },
            vec![a],
            vec![a],
            bundle("love", 1.0),
            TimestampMillis::new(60_000),
        );
        let step = reduce_group_event(&members, &mut states, &mut persona_relationships, &event);

        assert_eq!(step.checkpoints.len(), 1);
        assert_eq!(step.checkpoints[0].character_id, a);
        assert_eq!(states.get(&absent), Some(&original));
    }

    #[test]
    fn consecutive_events_carry_state_and_grow_the_interaction_count() {
        let config = config();
        let a = CharacterId::new();
        let members = [member(a, &config)];
        let mut states = BTreeMap::new();
        let mut persona_relationships = BTreeMap::new();
        let first = event(
            GroupCompanionAuthor::Persona { persona_id: None },
            vec![a],
            vec![a],
            bundle("love", 1.0),
            TimestampMillis::new(60_000),
        );
        let second = event(
            GroupCompanionAuthor::Persona { persona_id: None },
            vec![a],
            vec![a],
            bundle("love", 1.0),
            TimestampMillis::new(120_000),
        );
        reduce_group_event(&members, &mut states, &mut persona_relationships, &first);
        let warmth_after_first = states[&a].emotional_state.felt.warmth;
        reduce_group_event(&members, &mut states, &mut persona_relationships, &second);

        assert!(states[&a].emotional_state.felt.warmth > warmth_after_first);
        assert_eq!(persona_relationships[&a].interaction_count, 2);
    }

    #[test]
    fn reduction_is_deterministic_and_keeps_member_relationships_sorted() {
        let config = config();
        let a = CharacterId::new();
        let b = CharacterId::new();
        let c = CharacterId::new();
        let members = [member(a, &config), member(b, &config), member(c, &config)];
        let first = event(
            GroupCompanionAuthor::Member { character_id: a },
            vec![a, b],
            vec![b],
            bundle("caring", 1.0),
            TimestampMillis::new(60_000),
        );
        let second = event(
            GroupCompanionAuthor::Member { character_id: c },
            vec![c, b],
            vec![b],
            bundle("caring", 1.0),
            TimestampMillis::new(120_000),
        );
        let run = || {
            let mut states = BTreeMap::new();
            let mut persona_relationships = BTreeMap::new();
            let first_step =
                reduce_group_event(&members, &mut states, &mut persona_relationships, &first);
            let second_step =
                reduce_group_event(&members, &mut states, &mut persona_relationships, &second);
            (states, persona_relationships, first_step, second_step)
        };
        let (states_one, persona_one, first_step_one, second_step_one) = run();
        let (states_two, persona_two, first_step_two, second_step_two) = run();

        assert_eq!(states_one, states_two);
        assert_eq!(persona_one, persona_two);
        assert_eq!(first_step_one, first_step_two);
        assert_eq!(second_step_one, second_step_two);
        assert!(
            states_one[&b]
                .member_relationships
                .windows(2)
                .all(|pair| pair[0].target_character_id <= pair[1].target_character_id)
        );
    }

    #[test]
    fn every_checkpoint_validates() {
        let config = config();
        let a = CharacterId::new();
        let b = CharacterId::new();
        let members = [member(a, &config), member(b, &config)];
        let mut states = BTreeMap::new();
        let mut persona_relationships = BTreeMap::new();
        let events = [
            event(
                GroupCompanionAuthor::Persona {
                    persona_id: Some(PersonaId::new()),
                },
                vec![a, b],
                vec![a],
                bundle("love", 1.0),
                TimestampMillis::new(60_000),
            ),
            event(
                GroupCompanionAuthor::Member { character_id: a },
                vec![a, b],
                vec![b],
                bundle("anger", 1.0),
                TimestampMillis::new(120_000),
            ),
        ];
        for event in &events {
            let step = reduce_group_event(&members, &mut states, &mut persona_relationships, event);
            for checkpoint in step.checkpoints {
                assert_eq!(checkpoint.validate(), Ok(()));
            }
        }
    }

    #[test]
    fn speaker_runtime_state_decays_toward_baseline() {
        let config = config();
        let mut state = initial_group_member_state(&config);
        state.emotional_state.felt.warmth = 1.0;
        state.emotional_state.felt.hurt = 0.8;
        state.emotional_state.updated_at = TimestampMillis::new(60_000);
        let persona_relationship = initial_group_relationship(&config);
        let decayed = group_speaker_runtime_state(
            &state,
            &persona_relationship,
            &config,
            TimestampMillis::new(60_000 + 100_000_000),
        );

        assert!(
            (decayed.emotional_state.felt.warmth - config.soul.baseline_affect.warmth).abs() < 1e-9
        );
        assert!(
            (decayed.emotional_state.felt.hurt - config.soul.baseline_affect.hurt).abs() < 1e-9
        );
    }

    #[test]
    fn stances_map_relationship_bands() {
        let config = config();
        let a = CharacterId::new();
        let b = CharacterId::new();
        let state = GroupCompanionMemberState {
            emotional_state: initial_group_member_state(&config).emotional_state,
            member_relationships: vec![
                GroupMemberRelationship {
                    target_character_id: a,
                    state: RelationshipState {
                        closeness: 0.8,
                        trust: -0.6,
                        affection: 0.3,
                        tension: 0.7,
                        stability: 0.5,
                        interaction_count: 1,
                        last_interaction_at: TimestampMillis::new(1),
                    },
                },
                GroupMemberRelationship {
                    target_character_id: b,
                    state: RelationshipState {
                        closeness: -0.2,
                        trust: 0.1,
                        affection: -0.3,
                        tension: 0.1,
                        stability: 0.5,
                        interaction_count: 1,
                        last_interaction_at: TimestampMillis::new(1),
                    },
                },
            ],
        };
        let stances = group_member_stances(&state);

        assert_eq!(stances.len(), 2);
        assert_eq!(stances[0].target_character_id, a);
        assert_eq!(
            stances[0].closeness,
            RelationshipReading::of(RelationshipAxis::Closeness, 0.8)
        );
        assert_eq!(stances[0].closeness.band, RelationshipBand::Highest);
        assert_eq!(stances[0].trust.band, RelationshipBand::Lower);
        assert_eq!(stances[0].affection.band, RelationshipBand::High);
        assert_eq!(stances[0].tension.band, TensionBand::Strained);

        assert_eq!(stances[1].target_character_id, b);
        assert_eq!(stances[1].closeness.band, RelationshipBand::Low);
        assert_eq!(stances[1].trust.band, RelationshipBand::Neutral);
        assert_eq!(stances[1].affection.band, RelationshipBand::Low);
        assert_eq!(stances[1].tension.band, TensionBand::Calm);
    }
}
