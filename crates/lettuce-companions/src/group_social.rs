use std::collections::{BTreeMap, BTreeSet, HashSet};

use lettuce_types::{CharacterId, MessageId};
use serde::{Deserialize, Serialize};

use crate::{
    CompanionRuntimeState, CompanionSoulConfig, CompanionTurnEffectSeed, CompanionTurnInput,
    EmotionVector, GroupCompanionCheckpoint, GroupCompanionEffect, GroupCompanionEffectReason,
    GroupCompanionMember, GroupCompanionRelationshipChange, GroupCompanionTarget,
    GroupMemberRelationship, RelationshipAxis, RelationshipDelta, RelationshipState,
    apply_bipolar_delta, apply_turn, initial_group_relationship,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupSocialEventKind {
    pub kind: String,
    pub emotion: EmotionVector,
    pub relationship: RelationshipDelta,
    pub witness_emotion: Option<EmotionVector>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroupSocialEventCatalog {
    kinds: BTreeMap<String, GroupSocialEventKind>,
}

#[derive(Debug, thiserror::Error)]
pub enum GroupSocialCatalogError {
    #[error("invalid social event catalog JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported social event catalog version {0}")]
    UnsupportedVersion(u32),
    #[error("duplicate social event kind {0}")]
    DuplicateKind(String),
    #[error("social event kind {kind} has an invalid {field} value")]
    InvalidValue { kind: String, field: &'static str },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupSocialEventCatalogResource {
    version: u32,
    kinds: Vec<GroupSocialEventKind>,
}

impl GroupSocialEventCatalog {
    /// Parses and validates the bundled catalog: version 1, unique kinds, every value finite and within [-1, 1].
    pub fn from_json(source: &str) -> Result<Self, GroupSocialCatalogError> {
        let resource: GroupSocialEventCatalogResource = serde_json::from_str(source)?;
        if resource.version != 1 {
            return Err(GroupSocialCatalogError::UnsupportedVersion(
                resource.version,
            ));
        }
        let mut kinds = BTreeMap::new();
        for kind in resource.kinds {
            validate_social_event_kind(&kind)?;
            let name = kind.kind.clone();
            if kinds.insert(name.clone(), kind).is_some() {
                return Err(GroupSocialCatalogError::DuplicateKind(name));
            }
        }
        Ok(Self { kinds })
    }

    pub fn kind(&self, kind: &str) -> Option<&GroupSocialEventKind> {
        self.kinds.get(kind)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GroupSocialEvent {
    pub kind: String,
    /// `None` when the actor is the user's persona.
    pub from: Option<CharacterId>,
    pub to: CharacterId,
    pub source_message_id: MessageId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupSocialDropReason {
    UnknownKind,
    UnknownMember,
    SelfTarget,
    NotOnPath,
    TargetNotPresent,
    AlreadyApplied,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GroupSocialOutcome {
    /// The full checkpoint set of every message whose checkpoints changed, in path order.
    pub checkpoints: Vec<(MessageId, Vec<GroupCompanionCheckpoint>)>,
    /// Persona relationships changed by persona-authored events.
    pub persona_relationships: Vec<(CharacterId, RelationshipState)>,
    pub applied: Vec<GroupSocialEvent>,
    pub dropped: Vec<(GroupSocialEvent, GroupSocialDropReason)>,
}

#[allow(clippy::too_many_arguments)]
pub fn apply_group_social_events(
    members: &[GroupCompanionMember<'_>],
    path: &[MessageId],
    checkpoints: &[GroupCompanionCheckpoint],
    audience: &BTreeMap<MessageId, BTreeSet<CharacterId>>,
    persona_relationships: &BTreeMap<CharacterId, RelationshipState>,
    already_applied: &BTreeSet<GroupSocialEvent>,
    events: &[GroupSocialEvent],
    catalog: &GroupSocialEventCatalog,
) -> GroupSocialOutcome {
    let member_by_id: BTreeMap<CharacterId, GroupCompanionMember<'_>> = members
        .iter()
        .map(|member| (member.character_id, *member))
        .collect();
    let member_ids: HashSet<CharacterId> =
        members.iter().map(|member| member.character_id).collect();
    let path_position: BTreeMap<MessageId, usize> = path
        .iter()
        .enumerate()
        .map(|(position, message_id)| (*message_id, position))
        .collect();

    let mut outcome = GroupSocialOutcome::default();
    let mut accepted: Vec<(usize, usize, &GroupSocialEvent, &GroupSocialEventKind)> = Vec::new();
    let mut seen = HashSet::new();
    for (index, event) in events.iter().enumerate() {
        let Some(kind) = catalog.kind(&event.kind) else {
            outcome
                .dropped
                .push((event.clone(), GroupSocialDropReason::UnknownKind));
            continue;
        };
        if !member_ids.contains(&event.to)
            || event.from.is_some_and(|from| !member_ids.contains(&from))
        {
            outcome
                .dropped
                .push((event.clone(), GroupSocialDropReason::UnknownMember));
            continue;
        }
        if event.from == Some(event.to) {
            outcome
                .dropped
                .push((event.clone(), GroupSocialDropReason::SelfTarget));
            continue;
        }
        let Some(position) = path_position.get(&event.source_message_id).copied() else {
            outcome
                .dropped
                .push((event.clone(), GroupSocialDropReason::NotOnPath));
            continue;
        };
        if !audience
            .get(&event.source_message_id)
            .is_some_and(|members| members.contains(&event.to))
        {
            outcome
                .dropped
                .push((event.clone(), GroupSocialDropReason::TargetNotPresent));
            continue;
        }
        if already_applied.contains(event) || !seen.insert(event.clone()) {
            outcome
                .dropped
                .push((event.clone(), GroupSocialDropReason::AlreadyApplied));
            continue;
        }
        accepted.push((position, index, event, kind));
    }
    accepted.sort_by_key(|(position, index, _, _)| (*position, *index));

    let mut working = checkpoints.to_vec();
    let mut persona_working = persona_relationships.clone();
    for (source_position, _, event, kind) in accepted {
        let target_config = member_by_id[&event.to].config;
        let baseline = initial_group_relationship(target_config);
        let (target, relationship_delta) = if let Some(actor) = event.from {
            let mut source_delta = RelationshipDelta::default();
            for checkpoint in &mut working {
                if checkpoint.character_id != event.to {
                    continue;
                }
                let Some(position) = path_position.get(&checkpoint.message_id).copied() else {
                    continue;
                };
                if position < source_position {
                    continue;
                }
                let before = member_relationship_toward(&checkpoint.member_relationships, actor)
                    .unwrap_or_else(|| baseline.clone());
                let after = apply_relationship_delta(&before, &kind.relationship, &baseline);
                upsert_member_relationship(
                    &mut checkpoint.member_relationships,
                    actor,
                    after.clone(),
                );
                if checkpoint.message_id == event.source_message_id {
                    source_delta = relationship_delta_between(&before, &after);
                }
            }
            (
                GroupCompanionTarget::Character {
                    character_id: actor,
                },
                source_delta,
            )
        } else {
            let before = persona_working
                .get(&event.to)
                .cloned()
                .unwrap_or_else(|| baseline.clone());
            let after = apply_relationship_delta(&before, &kind.relationship, &baseline);
            persona_working.insert(event.to, after.clone());
            outcome
                .persona_relationships
                .push((event.to, after.clone()));
            (
                GroupCompanionTarget::Persona { persona_id: None },
                relationship_delta_between(&before, &after),
            )
        };

        let target_seed =
            latest_checkpoint_index(&working, &path_position, event.to).map(|index| {
                apply_social_emotion(
                    &mut working[index],
                    target_config,
                    &kind.emotion,
                    &event.kind,
                )
            });
        if let (Some(seed), Some(index)) = (
            target_seed,
            checkpoint_index(&working, event.source_message_id, event.to),
        ) {
            working[index].effects.push(GroupCompanionEffect {
                reason: GroupCompanionEffectReason::SocialEvent,
                social_event: Some(event.kind.clone()),
                emotion_delta: seed.emotion_delta,
                relationship_change: Some(GroupCompanionRelationshipChange {
                    target,
                    delta: relationship_delta,
                }),
                signal_changes: seed.signal_changes,
                by_user: false,
            });
        }

        if let Some(witness_emotion) = &kind.witness_emotion {
            for member in members {
                let witness = member.character_id;
                if witness == event.to || event.from == Some(witness) {
                    continue;
                }
                if !audience
                    .get(&event.source_message_id)
                    .is_some_and(|members| members.contains(&witness))
                {
                    continue;
                }
                let Some(latest) = latest_checkpoint_index(&working, &path_position, witness)
                else {
                    continue;
                };
                let seed = apply_social_emotion(
                    &mut working[latest],
                    member.config,
                    witness_emotion,
                    &event.kind,
                );
                if let Some(index) = checkpoint_index(&working, event.source_message_id, witness) {
                    working[index].effects.push(GroupCompanionEffect {
                        reason: GroupCompanionEffectReason::SocialEvent,
                        social_event: Some(event.kind.clone()),
                        emotion_delta: seed.emotion_delta,
                        relationship_change: None,
                        signal_changes: seed.signal_changes,
                        by_user: false,
                    });
                }
            }
        }

        outcome.applied.push(event.clone());
    }

    let changed_messages: BTreeSet<MessageId> = working
        .iter()
        .zip(checkpoints)
        .filter(|(current, previous)| current != previous)
        .map(|(current, _)| current.message_id)
        .collect();
    let mut by_message: BTreeMap<MessageId, Vec<GroupCompanionCheckpoint>> = BTreeMap::new();
    for checkpoint in &working {
        if changed_messages.contains(&checkpoint.message_id) {
            by_message
                .entry(checkpoint.message_id)
                .or_default()
                .push(checkpoint.clone());
        }
    }
    for message_id in path {
        if let Some(checkpoints) = by_message.remove(message_id) {
            outcome.checkpoints.push((*message_id, checkpoints));
        }
    }
    outcome
}

fn validate_social_event_kind(kind: &GroupSocialEventKind) -> Result<(), GroupSocialCatalogError> {
    if !emotion_in_range(&kind.emotion) {
        return Err(GroupSocialCatalogError::InvalidValue {
            kind: kind.kind.clone(),
            field: "emotion",
        });
    }
    if !relationship_in_range(&kind.relationship) {
        return Err(GroupSocialCatalogError::InvalidValue {
            kind: kind.kind.clone(),
            field: "relationship",
        });
    }
    if kind
        .witness_emotion
        .as_ref()
        .is_some_and(|emotion| !emotion_in_range(emotion))
    {
        return Err(GroupSocialCatalogError::InvalidValue {
            kind: kind.kind.clone(),
            field: "witness_emotion",
        });
    }
    Ok(())
}

fn emotion_in_range(emotion: &EmotionVector) -> bool {
    [
        emotion.warmth,
        emotion.trust,
        emotion.calm,
        emotion.vulnerability,
        emotion.longing,
        emotion.hurt,
        emotion.tension,
        emotion.irritation,
        emotion.affection_intensity,
        emotion.reassurance_need,
    ]
    .into_iter()
    .all(signed)
}

fn relationship_in_range(relationship: &RelationshipDelta) -> bool {
    [
        relationship.closeness,
        relationship.trust,
        relationship.affection,
        relationship.tension,
        relationship.stability,
    ]
    .into_iter()
    .all(signed)
}

fn signed(value: f64) -> bool {
    value.is_finite() && (-1.0..=1.0).contains(&value)
}

fn checkpoint_index(
    checkpoints: &[GroupCompanionCheckpoint],
    message_id: MessageId,
    character_id: CharacterId,
) -> Option<usize> {
    checkpoints.iter().position(|checkpoint| {
        checkpoint.message_id == message_id && checkpoint.character_id == character_id
    })
}

fn latest_checkpoint_index(
    checkpoints: &[GroupCompanionCheckpoint],
    path_position: &BTreeMap<MessageId, usize>,
    character_id: CharacterId,
) -> Option<usize> {
    checkpoints
        .iter()
        .enumerate()
        .filter(|(_, checkpoint)| checkpoint.character_id == character_id)
        .filter_map(|(index, checkpoint)| {
            path_position
                .get(&checkpoint.message_id)
                .map(|position| (index, *position))
        })
        .max_by_key(|(_, position)| *position)
        .map(|(index, _)| index)
}

fn member_relationship_toward(
    relationships: &[GroupMemberRelationship],
    target_character_id: CharacterId,
) -> Option<RelationshipState> {
    relationships
        .iter()
        .find(|relationship| relationship.target_character_id == target_character_id)
        .map(|relationship| relationship.state.clone())
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

fn apply_relationship_delta(
    state: &RelationshipState,
    delta: &RelationshipDelta,
    baseline: &RelationshipState,
) -> RelationshipState {
    let mut next = state.clone();
    next.closeness = apply_bipolar_delta(
        RelationshipAxis::Closeness,
        state.closeness,
        delta.closeness,
        baseline.closeness,
    );
    next.trust = apply_bipolar_delta(
        RelationshipAxis::Trust,
        state.trust,
        delta.trust,
        baseline.trust,
    );
    next.affection = apply_bipolar_delta(
        RelationshipAxis::Affection,
        state.affection,
        delta.affection,
        baseline.affection,
    );
    next.tension = (state.tension + delta.tension).clamp(0.0, 1.0);
    next.stability = (state.stability + delta.stability).clamp(0.0, 1.0);
    next
}

fn relationship_delta_between(
    before: &RelationshipState,
    after: &RelationshipState,
) -> RelationshipDelta {
    RelationshipDelta {
        closeness: after.closeness - before.closeness,
        trust: after.trust - before.trust,
        affection: after.affection - before.affection,
        tension: after.tension - before.tension,
        stability: after.stability - before.stability,
    }
}

fn apply_social_emotion(
    checkpoint: &mut GroupCompanionCheckpoint,
    config: &CompanionSoulConfig,
    delta: &EmotionVector,
    kind: &str,
) -> CompanionTurnEffectSeed {
    let now = checkpoint.created_at;
    let runtime = CompanionRuntimeState {
        emotional_state: checkpoint.emotional_state.clone(),
        relationship_state: initial_group_relationship(config),
        active_signals: checkpoint.emotional_state.active_drivers.clone(),
        updated_at: now,
    };
    let transition = apply_turn(
        &runtime,
        &config.soul.baseline_affect,
        &config.soul.regulation_style,
        &config.relationship_defaults,
        &CompanionTurnInput {
            signals: vec![format!("social:{kind}")],
            emotion_delta: delta.clone(),
            relationship_delta: RelationshipDelta::default(),
            confidence: checkpoint.emotional_state.confidence,
            now,
        },
    );
    checkpoint.emotional_state = transition.current.emotional_state.clone();
    CompanionTurnEffectSeed::from_transition(&transition)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GroupCompanionMessageSource, initial_group_member_state};
    use lettuce_types::TimestampMillis;

    fn config() -> CompanionSoulConfig {
        CompanionSoulConfig::default()
    }

    fn member(character_id: CharacterId, config: &CompanionSoulConfig) -> GroupCompanionMember<'_> {
        GroupCompanionMember {
            character_id,
            config,
        }
    }

    fn checkpoint(
        message_id: MessageId,
        character_id: CharacterId,
        created_at: TimestampMillis,
    ) -> GroupCompanionCheckpoint {
        let config = config();
        GroupCompanionCheckpoint {
            message_id,
            character_id,
            source: GroupCompanionMessageSource {
                revision_id: None,
                candidate_id: None,
            },
            emotional_state: initial_group_member_state(&config).emotional_state,
            member_relationships: Vec::new(),
            effects: Vec::new(),
            created_at,
        }
    }

    fn catalog() -> GroupSocialEventCatalog {
        GroupSocialEventCatalog::from_json(
            r#"{
                "version": 1,
                "kinds": [
                    {
                        "kind": "apology",
                        "emotion": { "warmth": 0.1, "hurt": -0.1, "tension": -0.1 },
                        "relationship": { "closeness": 0.06, "trust": 0.1, "affection": 0.05, "tension": -0.1, "stability": 0.05 },
                        "witness_emotion": { "warmth": 0.05, "tension": -0.05 }
                    },
                    {
                        "kind": "support",
                        "emotion": { "warmth": 0.1, "hurt": -0.06, "tension": -0.06 },
                        "relationship": { "closeness": 0.05, "trust": 0.07, "affection": 0.06, "tension": -0.07, "stability": 0.04 },
                        "witness_emotion": { "warmth": 0.04 }
                    },
                    {
                        "kind": "teasing",
                        "emotion": { "warmth": 0.05, "tension": 0.02 },
                        "relationship": { "closeness": 0.03, "trust": 0.01, "affection": 0.04, "tension": 0.02, "stability": 0.01 }
                    }
                ]
            }"#,
        )
        .expect("catalog")
    }

    fn event(
        kind: &str,
        from: Option<CharacterId>,
        to: CharacterId,
        source_message_id: MessageId,
    ) -> GroupSocialEvent {
        GroupSocialEvent {
            kind: kind.into(),
            from,
            to,
            source_message_id,
        }
    }

    fn find_checkpoint(
        checkpoints: &[GroupCompanionCheckpoint],
        message_id: MessageId,
        character_id: CharacterId,
    ) -> &GroupCompanionCheckpoint {
        checkpoints
            .iter()
            .find(|checkpoint| {
                checkpoint.message_id == message_id && checkpoint.character_id == character_id
            })
            .expect("checkpoint")
    }

    fn audience(
        checkpoints: &[GroupCompanionCheckpoint],
    ) -> BTreeMap<MessageId, BTreeSet<CharacterId>> {
        let mut audience: BTreeMap<MessageId, BTreeSet<CharacterId>> = BTreeMap::new();
        for checkpoint in checkpoints {
            audience
                .entry(checkpoint.message_id)
                .or_default()
                .insert(checkpoint.character_id);
        }
        audience
    }

    fn find_stance(
        checkpoint: &GroupCompanionCheckpoint,
        target_character_id: CharacterId,
    ) -> Option<&RelationshipState> {
        checkpoint
            .member_relationships
            .iter()
            .find(|relationship| relationship.target_character_id == target_character_id)
            .map(|relationship| &relationship.state)
    }

    #[test]
    fn apology_changes_target_stance_on_source_and_later_checkpoints_only() {
        let config = config();
        let bea = CharacterId::new();
        let ada = CharacterId::new();
        let members = [member(bea, &config), member(ada, &config)];
        let path = [MessageId::new(), MessageId::new(), MessageId::new()];
        let original = vec![
            checkpoint(path[0], bea, TimestampMillis::new(1_000)),
            checkpoint(path[0], ada, TimestampMillis::new(1_000)),
            checkpoint(path[1], bea, TimestampMillis::new(2_000)),
            checkpoint(path[1], ada, TimestampMillis::new(2_000)),
            checkpoint(path[2], bea, TimestampMillis::new(3_000)),
            checkpoint(path[2], ada, TimestampMillis::new(3_000)),
        ];
        let event = event("apology", Some(bea), ada, path[1]);
        let outcome = apply_group_social_events(
            &members,
            &path,
            &original,
            &audience(&original),
            &BTreeMap::new(),
            &BTreeSet::new(),
            std::slice::from_ref(&event),
            &catalog(),
        );

        assert_eq!(outcome.applied, [event]);
        assert!(outcome.dropped.is_empty());
        assert_eq!(outcome.checkpoints.len(), 2);
        assert_eq!(outcome.checkpoints[0].0, path[1]);
        assert_eq!(outcome.checkpoints[1].0, path[2]);

        let baseline = initial_group_relationship(&config);
        let expected = apply_relationship_delta(
            &baseline,
            &catalog().kind("apology").expect("apology").relationship,
            &baseline,
        );
        let source = &outcome.checkpoints[0].1;
        let source_ada = find_checkpoint(source, path[1], ada);
        assert_eq!(find_stance(source_ada, bea), Some(&expected));
        let later = &outcome.checkpoints[1].1;
        let later_ada = find_checkpoint(later, path[2], ada);
        assert_eq!(find_stance(later_ada, bea), Some(&expected));

        let original_source_ada = find_checkpoint(&original, path[1], ada);
        assert_eq!(
            source_ada.emotional_state,
            original_source_ada.emotional_state
        );
        let original_later_ada = find_checkpoint(&original, path[2], ada);
        assert_ne!(
            later_ada.emotional_state,
            original_later_ada.emotional_state
        );

        for (message_id, checkpoints) in &outcome.checkpoints {
            let bea_checkpoint = find_checkpoint(checkpoints, *message_id, bea);
            let original_bea = find_checkpoint(&original, *message_id, bea);
            assert_eq!(bea_checkpoint, original_bea);
        }
        assert!(
            outcome
                .checkpoints
                .iter()
                .all(|(message_id, _)| *message_id != path[0])
        );
    }

    #[test]
    fn witness_receives_only_the_witness_emotion_and_an_effect_without_relationship_change() {
        let config = config();
        let bea = CharacterId::new();
        let ada = CharacterId::new();
        let cy = CharacterId::new();
        let members = [
            member(bea, &config),
            member(ada, &config),
            member(cy, &config),
        ];
        let path = [MessageId::new(), MessageId::new()];
        let original = vec![
            checkpoint(path[0], bea, TimestampMillis::new(1_000)),
            checkpoint(path[0], ada, TimestampMillis::new(1_000)),
            checkpoint(path[0], cy, TimestampMillis::new(1_000)),
            checkpoint(path[1], cy, TimestampMillis::new(2_000)),
        ];
        let event = event("apology", Some(bea), ada, path[0]);
        let outcome = apply_group_social_events(
            &members,
            &path,
            &original,
            &audience(&original),
            &BTreeMap::new(),
            &BTreeSet::new(),
            &[event],
            &catalog(),
        );

        let source = &outcome.checkpoints[0].1;
        let witness_source = find_checkpoint(source, path[0], cy);
        let original_witness_source = find_checkpoint(&original, path[0], cy);
        assert_eq!(
            witness_source.emotional_state,
            original_witness_source.emotional_state
        );
        assert_eq!(witness_source.effects.len(), 1);
        assert_eq!(
            witness_source.effects[0].reason,
            GroupCompanionEffectReason::SocialEvent
        );
        assert!(witness_source.effects[0].relationship_change.is_none());
        assert!(witness_source.member_relationships.is_empty());

        let latest = &outcome.checkpoints[1].1;
        let witness_latest = find_checkpoint(latest, path[1], cy);
        let original_witness_latest = find_checkpoint(&original, path[1], cy);
        assert_ne!(
            witness_latest.emotional_state,
            original_witness_latest.emotional_state
        );
        assert!(witness_latest.effects.is_empty());
    }

    #[test]
    fn each_drop_reason_is_reported() {
        let config = config();
        let bea = CharacterId::new();
        let ada = CharacterId::new();
        let cleo = CharacterId::new();
        let stranger = CharacterId::new();
        let members = [
            member(bea, &config),
            member(ada, &config),
            member(cleo, &config),
        ];
        let path = [MessageId::new(), MessageId::new()];
        let checkpoints = vec![
            checkpoint(path[0], bea, TimestampMillis::new(1_000)),
            checkpoint(path[0], ada, TimestampMillis::new(1_000)),
            checkpoint(path[1], cleo, TimestampMillis::new(2_000)),
        ];
        let already = event("apology", Some(bea), ada, path[0]);
        let events = [
            event("unknown", Some(bea), ada, path[0]),
            event("apology", Some(bea), stranger, path[0]),
            event("apology", Some(bea), bea, path[0]),
            event("apology", Some(bea), ada, MessageId::new()),
            event("apology", Some(bea), cleo, path[0]),
            already.clone(),
        ];
        let already_applied = BTreeSet::from([already]);
        let outcome = apply_group_social_events(
            &members,
            &path,
            &checkpoints,
            &audience(&checkpoints),
            &BTreeMap::new(),
            &already_applied,
            &events,
            &catalog(),
        );

        assert!(outcome.applied.is_empty());
        assert_eq!(
            outcome
                .dropped
                .iter()
                .map(|(_, reason)| *reason)
                .collect::<Vec<_>>(),
            [
                GroupSocialDropReason::UnknownKind,
                GroupSocialDropReason::UnknownMember,
                GroupSocialDropReason::SelfTarget,
                GroupSocialDropReason::NotOnPath,
                GroupSocialDropReason::TargetNotPresent,
                GroupSocialDropReason::AlreadyApplied,
            ]
        );
    }

    #[test]
    fn already_applied_and_duplicate_events_are_applied_once() {
        let config = config();
        let bea = CharacterId::new();
        let ada = CharacterId::new();
        let members = [member(bea, &config), member(ada, &config)];
        let path = [MessageId::new()];
        let checkpoints = vec![
            checkpoint(path[0], bea, TimestampMillis::new(1_000)),
            checkpoint(path[0], ada, TimestampMillis::new(1_000)),
        ];
        let event = event("apology", Some(bea), ada, path[0]);

        let already = apply_group_social_events(
            &members,
            &path,
            &checkpoints,
            &audience(&checkpoints),
            &BTreeMap::new(),
            &BTreeSet::from([event.clone()]),
            &[event.clone(), event.clone()],
            &catalog(),
        );
        assert!(already.applied.is_empty());
        assert_eq!(already.dropped.len(), 2);
        assert!(
            already
                .dropped
                .iter()
                .all(|(_, reason)| *reason == GroupSocialDropReason::AlreadyApplied)
        );

        let duplicated = apply_group_social_events(
            &members,
            &path,
            &checkpoints,
            &audience(&checkpoints),
            &BTreeMap::new(),
            &BTreeSet::new(),
            &[event.clone(), event.clone()],
            &catalog(),
        );
        assert_eq!(duplicated.applied, [event]);
        assert_eq!(
            duplicated.dropped,
            [(
                duplicated.dropped[0].0.clone(),
                GroupSocialDropReason::AlreadyApplied
            )]
        );
    }

    #[test]
    fn persona_authored_event_changes_only_the_persona_relationship() {
        let config = config();
        let bea = CharacterId::new();
        let ada = CharacterId::new();
        let members = [member(bea, &config), member(ada, &config)];
        let path = [MessageId::new()];
        let checkpoints = vec![
            checkpoint(path[0], bea, TimestampMillis::new(1_000)),
            checkpoint(path[0], ada, TimestampMillis::new(1_000)),
        ];
        let event = event("apology", None, ada, path[0]);
        let outcome = apply_group_social_events(
            &members,
            &path,
            &checkpoints,
            &audience(&checkpoints),
            &BTreeMap::new(),
            &BTreeSet::new(),
            &[event],
            &catalog(),
        );

        let baseline = initial_group_relationship(&config);
        let expected = apply_relationship_delta(
            &baseline,
            &catalog().kind("apology").expect("apology").relationship,
            &baseline,
        );
        assert_eq!(outcome.persona_relationships.len(), 1);
        assert_eq!(outcome.persona_relationships[0].0, ada);
        assert_eq!(outcome.persona_relationships[0].1, expected);

        let ada_checkpoint = find_checkpoint(&outcome.checkpoints[0].1, path[0], ada);
        assert!(ada_checkpoint.member_relationships.is_empty());
        let effect = &ada_checkpoint.effects[0];
        assert_eq!(
            effect
                .relationship_change
                .as_ref()
                .expect("relationship change")
                .target,
            GroupCompanionTarget::Persona { persona_id: None }
        );
    }

    #[test]
    fn two_events_on_the_same_target_compose_in_path_order() {
        let config = config();
        let bea = CharacterId::new();
        let ada = CharacterId::new();
        let members = [member(bea, &config), member(ada, &config)];
        let path = [MessageId::new(), MessageId::new(), MessageId::new()];
        let checkpoints = vec![
            checkpoint(path[0], bea, TimestampMillis::new(1_000)),
            checkpoint(path[0], ada, TimestampMillis::new(1_000)),
            checkpoint(path[1], bea, TimestampMillis::new(2_000)),
            checkpoint(path[1], ada, TimestampMillis::new(2_000)),
            checkpoint(path[2], bea, TimestampMillis::new(3_000)),
            checkpoint(path[2], ada, TimestampMillis::new(3_000)),
        ];
        let first = event("apology", Some(bea), ada, path[0]);
        let second = event("support", Some(bea), ada, path[1]);
        let outcome = apply_group_social_events(
            &members,
            &path,
            &checkpoints,
            &audience(&checkpoints),
            &BTreeMap::new(),
            &BTreeSet::new(),
            &[second.clone(), first.clone()],
            &catalog(),
        );

        assert_eq!(outcome.applied, [first.clone(), second]);
        let baseline = initial_group_relationship(&config);
        let catalog = catalog();
        let once = apply_relationship_delta(
            &baseline,
            &catalog.kind("apology").expect("apology").relationship,
            &baseline,
        );
        let twice = apply_relationship_delta(
            &once,
            &catalog.kind("support").expect("support").relationship,
            &baseline,
        );
        let first_message = find_checkpoint(&outcome.checkpoints[0].1, path[0], ada);
        assert_eq!(find_stance(first_message, bea), Some(&once));
        let second_message = find_checkpoint(&outcome.checkpoints[1].1, path[1], ada);
        assert_eq!(find_stance(second_message, bea), Some(&twice));
        let third_message = find_checkpoint(&outcome.checkpoints[2].1, path[2], ada);
        assert_eq!(find_stance(third_message, bea), Some(&twice));
    }

    #[test]
    fn catalog_rejects_unknown_fields_duplicate_kinds_and_invalid_values() {
        let unknown_field = r#"{
            "version": 1,
            "kinds": [{
                "kind": "apology",
                "emotion": {},
                "relationship": { "closeness": 0.0, "trust": 0.0, "affection": 0.0, "tension": 0.0, "stability": 0.0 },
                "extra": 1.0
            }]
        }"#;
        assert!(GroupSocialEventCatalog::from_json(unknown_field).is_err());

        let duplicate = r#"{
            "version": 1,
            "kinds": [
                { "kind": "apology", "emotion": {}, "relationship": { "closeness": 0.0, "trust": 0.0, "affection": 0.0, "tension": 0.0, "stability": 0.0 } },
                { "kind": "apology", "emotion": {}, "relationship": { "closeness": 0.0, "trust": 0.0, "affection": 0.0, "tension": 0.0, "stability": 0.0 } }
            ]
        }"#;
        assert!(matches!(
            GroupSocialEventCatalog::from_json(duplicate),
            Err(GroupSocialCatalogError::DuplicateKind(kind)) if kind == "apology"
        ));

        let non_finite = r#"{
            "version": 1,
            "kinds": [{
                "kind": "apology",
                "emotion": { "warmth": 1e400 },
                "relationship": { "closeness": 0.0, "trust": 0.0, "affection": 0.0, "tension": 0.0, "stability": 0.0 }
            }]
        }"#;
        assert!(GroupSocialEventCatalog::from_json(non_finite).is_err());

        let out_of_range = r#"{
            "version": 1,
            "kinds": [{
                "kind": "apology",
                "emotion": { "warmth": 1.5 },
                "relationship": { "closeness": 0.0, "trust": 0.0, "affection": 0.0, "tension": 0.0, "stability": 0.0 }
            }]
        }"#;
        assert!(matches!(
            GroupSocialEventCatalog::from_json(out_of_range),
            Err(GroupSocialCatalogError::InvalidValue {
                field: "emotion",
                ..
            })
        ));
    }
}
