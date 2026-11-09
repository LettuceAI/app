use std::collections::{BTreeMap, BTreeSet};

use lettuce_types::{CharacterId, MessageId};

use crate::{GroupCompanionCheckpoint, GroupCompanionEffectReason};

#[derive(Debug, Clone, PartialEq)]
pub struct GroupBranchAudience {
    pub path: Vec<MessageId>,
    pub authors: BTreeMap<MessageId, CharacterId>,
    pub checkpoints: Vec<GroupCompanionCheckpoint>,
    pub audience: BTreeMap<MessageId, BTreeSet<CharacterId>>,
    pub away: BTreeSet<CharacterId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupMessageAudienceMember {
    pub character_id: CharacterId,
    pub heard: bool,
    pub away_after: bool,
}

/// Who heard each message of `path`, walking it in order. A member is in a
/// message's audience when it has a checkpoint there and is not away, or when
/// the message corrects that: `LeftAudience` makes the member away from that
/// message on and `JoinedAudience` ends it; `Unheard` and `Overheard` change
/// only that message. The message's author always heard it and is no longer away.
pub fn group_message_audience(
    path: &[MessageId],
    authors: &BTreeMap<MessageId, CharacterId>,
    checkpoints: &[GroupCompanionCheckpoint],
) -> BTreeMap<MessageId, BTreeSet<CharacterId>> {
    walk(path, authors, checkpoints).0
}

/// Members away after the last message of `path`.
pub fn group_audience_away(
    path: &[MessageId],
    authors: &BTreeMap<MessageId, CharacterId>,
    checkpoints: &[GroupCompanionCheckpoint],
) -> BTreeSet<CharacterId> {
    walk(path, authors, checkpoints).1
}

fn walk(
    path: &[MessageId],
    authors: &BTreeMap<MessageId, CharacterId>,
    checkpoints: &[GroupCompanionCheckpoint],
) -> (
    BTreeMap<MessageId, BTreeSet<CharacterId>>,
    BTreeSet<CharacterId>,
) {
    let mut by_message =
        BTreeMap::<MessageId, BTreeMap<CharacterId, &GroupCompanionCheckpoint>>::new();
    for checkpoint in checkpoints {
        by_message
            .entry(checkpoint.message_id)
            .or_default()
            .insert(checkpoint.character_id, checkpoint);
    }
    let empty = BTreeMap::new();
    let mut audience_by_message = BTreeMap::new();
    let mut away = BTreeSet::new();
    for message_id in path {
        let author = authors.get(message_id).copied();
        let at_message = by_message.get(message_id).unwrap_or(&empty);
        let mut members = at_message.keys().copied().collect::<BTreeSet<_>>();
        members.extend(away.iter().copied());
        if let Some(author) = author {
            members.insert(author);
            away.remove(&author);
        }
        let mut audience = BTreeSet::new();
        for member in members {
            if Some(member) == author {
                away.remove(&member);
                audience.insert(member);
                continue;
            }
            let checkpoint = at_message.get(&member).copied();
            let mut heard = checkpoint.is_some() && !away.contains(&member);
            if let Some(checkpoint) = checkpoint {
                for effect in &checkpoint.effects {
                    match effect.reason {
                        GroupCompanionEffectReason::Unheard => heard = false,
                        GroupCompanionEffectReason::Overheard => heard = true,
                        GroupCompanionEffectReason::LeftAudience => {
                            heard = false;
                            away.insert(member);
                        }
                        GroupCompanionEffectReason::JoinedAudience => {
                            heard = true;
                            away.remove(&member);
                        }
                        GroupCompanionEffectReason::Witnessed
                        | GroupCompanionEffectReason::Addressed
                        | GroupCompanionEffectReason::SocialEvent => {}
                    }
                }
            }
            if heard {
                audience.insert(member);
            }
        }
        audience_by_message.insert(*message_id, audience);
    }
    (audience_by_message, away)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CompanionEmotionDelta, CompanionSignalChanges, EmotionVector, EmotionalState,
        GroupCompanionEffect, GroupCompanionMessageSource,
    };
    use lettuce_types::TimestampMillis;

    fn checkpoint(
        message_id: MessageId,
        character_id: CharacterId,
        effects: Vec<GroupCompanionEffectReason>,
    ) -> GroupCompanionCheckpoint {
        GroupCompanionCheckpoint {
            message_id,
            character_id,
            source: GroupCompanionMessageSource {
                revision_id: None,
                candidate_id: None,
            },
            emotional_state: EmotionalState {
                felt: EmotionVector::default(),
                expressed: EmotionVector::default(),
                blocked: EmotionVector::default(),
                momentum: EmotionVector::default(),
                active_drivers: Vec::new(),
                confidence: 0.5,
                updated_at: TimestampMillis::UNIX_EPOCH,
            },
            member_relationships: Vec::new(),
            effects: effects
                .into_iter()
                .map(|reason| GroupCompanionEffect {
                    reason,
                    social_event: None,
                    emotion_delta: CompanionEmotionDelta::default(),
                    relationship_change: None,
                    signal_changes: CompanionSignalChanges::default(),
                    by_user: false,
                })
                .collect(),
            created_at: TimestampMillis::UNIX_EPOCH,
        }
    }

    #[test]
    fn left_audience_hides_later_messages_until_the_member_speaks() {
        let path = [MessageId::new(), MessageId::new(), MessageId::new()];
        let left = CharacterId::new();
        let other = CharacterId::new();
        let checkpoints = vec![
            checkpoint(path[0], left, Vec::new()),
            checkpoint(path[0], other, Vec::new()),
            checkpoint(
                path[1],
                left,
                vec![GroupCompanionEffectReason::LeftAudience],
            ),
            checkpoint(path[1], other, Vec::new()),
            checkpoint(path[2], other, Vec::new()),
        ];
        let audience = group_message_audience(&path, &BTreeMap::new(), &checkpoints);
        assert!(audience[&path[0]].contains(&left));
        assert!(!audience[&path[1]].contains(&left));
        assert!(!audience[&path[2]].contains(&left));
        assert!(audience[&path[2]].contains(&other));

        let mut authors = BTreeMap::new();
        authors.insert(path[2], left);
        let audience = group_message_audience(&path, &authors, &checkpoints);
        assert!(audience[&path[2]].contains(&left));
        assert!(!group_audience_away(&path, &authors, &checkpoints).contains(&left));
    }

    #[test]
    fn joined_audience_returns_a_member_without_speaking() {
        let path = [MessageId::new(), MessageId::new(), MessageId::new()];
        let member = CharacterId::new();
        let checkpoints = vec![
            checkpoint(
                path[0],
                member,
                vec![GroupCompanionEffectReason::LeftAudience],
            ),
            checkpoint(
                path[2],
                member,
                vec![GroupCompanionEffectReason::JoinedAudience],
            ),
        ];
        let audience = group_message_audience(&path, &BTreeMap::new(), &checkpoints);
        assert!(!audience[&path[0]].contains(&member));
        assert!(!audience[&path[1]].contains(&member));
        assert!(audience[&path[2]].contains(&member));
        assert!(!group_audience_away(&path, &BTreeMap::new(), &checkpoints).contains(&member));
    }

    #[test]
    fn unheard_and_overheard_change_only_their_message() {
        let path = [MessageId::new(), MessageId::new(), MessageId::new()];
        let member = CharacterId::new();
        let checkpoints = vec![
            checkpoint(path[0], member, Vec::new()),
            checkpoint(path[1], member, vec![GroupCompanionEffectReason::Unheard]),
            checkpoint(path[2], member, Vec::new()),
        ];
        let audience = group_message_audience(&path, &BTreeMap::new(), &checkpoints);
        assert!(audience[&path[0]].contains(&member));
        assert!(!audience[&path[1]].contains(&member));
        assert!(audience[&path[2]].contains(&member));

        let checkpoints = vec![
            checkpoint(
                path[0],
                member,
                vec![GroupCompanionEffectReason::LeftAudience],
            ),
            checkpoint(path[1], member, vec![GroupCompanionEffectReason::Overheard]),
            checkpoint(path[2], member, Vec::new()),
        ];
        let audience = group_message_audience(&path, &BTreeMap::new(), &checkpoints);
        assert!(!audience[&path[0]].contains(&member));
        assert!(audience[&path[1]].contains(&member));
        assert!(!audience[&path[2]].contains(&member));
        assert!(group_audience_away(&path, &BTreeMap::new(), &checkpoints).contains(&member));
    }

    #[test]
    fn a_member_without_checkpoints_is_never_in_the_audience() {
        let path = [MessageId::new(), MessageId::new()];
        let member = CharacterId::new();
        let other = CharacterId::new();
        let checkpoints = vec![checkpoint(path[0], other, Vec::new())];
        let audience = group_message_audience(&path, &BTreeMap::new(), &checkpoints);
        assert!(!audience[&path[0]].contains(&member));
        assert!(!audience[&path[1]].contains(&member));
        assert!(audience[&path[0]].contains(&other));
        assert!(!audience[&path[1]].contains(&other));
    }

    #[test]
    fn the_author_outranks_a_correction_at_the_same_message() {
        let path = [MessageId::new()];
        let member = CharacterId::new();
        let checkpoints = vec![checkpoint(
            path[0],
            member,
            vec![GroupCompanionEffectReason::LeftAudience],
        )];
        let mut authors = BTreeMap::new();
        authors.insert(path[0], member);
        let audience = group_message_audience(&path, &authors, &checkpoints);
        assert!(audience[&path[0]].contains(&member));
        assert!(!group_audience_away(&path, &authors, &checkpoints).contains(&member));
    }
}
