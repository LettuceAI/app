use std::collections::{HashMap, HashSet};

use lettuce_types::{CharacterId, OperationRecordId, Revision, TimestampMillis};

pub const CONSOLIDATION_THRESHOLD: usize = 12;
pub const MAX_SUPERSEDED_HISTORY: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoulCategory {
    Essence,
    Traits,
    Backstory,
    Appearance,
    Goals,
    Likes,
    Voice,
    RelationalStyle,
    Vulnerabilities,
    Fears,
    Habits,
    Boundaries,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoulMutability {
    Immutable,
    VerySlow,
    Slow,
    Fast,
}

impl SoulCategory {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Essence => "essence",
            Self::Traits => "traits",
            Self::Backstory => "backstory",
            Self::Appearance => "appearance",
            Self::Goals => "goals",
            Self::Likes => "likes",
            Self::Voice => "voice",
            Self::RelationalStyle => "relationalStyle",
            Self::Vulnerabilities => "vulnerabilities",
            Self::Fears => "fears",
            Self::Habits => "habits",
            Self::Boundaries => "boundaries",
        }
    }

    #[must_use]
    pub const fn mutability(self) -> SoulMutability {
        match self {
            Self::Backstory => SoulMutability::Immutable,
            Self::Essence | Self::Traits => SoulMutability::VerySlow,
            Self::Likes => SoulMutability::Fast,
            Self::Appearance
            | Self::Goals
            | Self::Voice
            | Self::RelationalStyle
            | Self::Vulnerabilities
            | Self::Fears
            | Self::Habits
            | Self::Boundaries => SoulMutability::Slow,
        }
    }

    #[must_use]
    pub const fn is_changeable(self) -> bool {
        matches!(
            self.mutability(),
            SoulMutability::Fast | SoulMutability::Slow
        )
    }

    #[must_use]
    pub const fn is_consolidatable(self) -> bool {
        !matches!(self.mutability(), SoulMutability::Immutable)
    }

    #[must_use]
    pub const fn minimum_confidence(self) -> f64 {
        match self.mutability() {
            SoulMutability::Fast => 0.55,
            SoulMutability::Slow => 0.70,
            SoulMutability::VerySlow => 0.85,
            SoulMutability::Immutable => 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoulFactPolicy {
    Current,
    Adaptive,
    Historical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoulFactKind {
    Add,
    Adjust,
    Authored,
    Consolidated,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SoulFact {
    pub id: String,
    pub category: SoulCategory,
    pub value: String,
    pub kind: SoulFactKind,
    pub policy: SoulFactPolicy,
    pub slot: String,
    pub confidence: f64,
    pub evidence_count: u32,
    pub weight: f64,
    pub valid_from: TimestampMillis,
    pub valid_until: Option<TimestampMillis>,
    pub locked: bool,
    pub source_memory_ids: Vec<String>,
    pub created_at: TimestampMillis,
    pub supersedes: Vec<String>,
    pub superseded_by: Option<String>,
    pub superseded_at: Option<TimestampMillis>,
}

impl SoulFact {
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.superseded_by.is_none()
    }

    #[must_use]
    pub fn is_effective_at(&self, now: TimestampMillis) -> bool {
        self.is_active()
            && self.valid_from.get() <= now.get()
            && self.valid_until.is_none_or(|until| until.get() > now.get())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SoulState {
    pub revision: Revision,
    pub facts: Vec<SoulFact>,
}

/// Durable Soul continuity follows the legacy character-wide ownership rule.
/// It is intentionally not session- or persona-scoped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoulOwner {
    Character(CharacterId),
}

impl SoulOwner {
    #[must_use]
    pub const fn character_id(self) -> CharacterId {
        match self {
            Self::Character(id) => id,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProposedSoulFact {
    pub id: String,
    pub category: SoulCategory,
    pub value: String,
    pub kind: SoulFactKind,
    pub policy: SoulFactPolicy,
    pub slot: String,
    pub confidence: f64,
    pub weight: f64,
    pub valid_until: Option<TimestampMillis>,
    pub locked: bool,
    pub source_memory_ids: Vec<String>,
    pub supersedes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoulSupersession {
    pub fact_id: String,
    pub superseded_by: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SoulChangeSet {
    pub expected_revision: Revision,
    pub resulting_revision: Revision,
    pub additions: Vec<SoulFact>,
    pub supersessions: Vec<SoulSupersession>,
    pub applied_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoulApplyReceipt {
    pub operation_id: OperationRecordId,
    pub owner: SoulOwner,
    pub expected_revision: Revision,
    pub resulting_revision: Revision,
    pub applied_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SoulRepositoryError {
    NotFound,
    AlreadyExists,
    Conflict,
    Invalid(SoulPolicyError),
    OperationMismatch,
    Corrupt,
    Failure,
}

pub trait SoulRepository: Send + Sync {
    fn create(
        &self,
        owner: SoulOwner,
        state: SoulState,
        now: TimestampMillis,
    ) -> Result<SoulState, SoulRepositoryError>;

    fn get(&self, owner: SoulOwner) -> Result<Option<SoulState>, SoulRepositoryError>;

    fn apply(
        &self,
        owner: SoulOwner,
        operation_id: OperationRecordId,
        change_set: SoulChangeSet,
    ) -> Result<SoulApplyReceipt, SoulRepositoryError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SoulPolicyError {
    StaleRevision,
    InvalidFact,
    DuplicateIdentity,
    InvalidSupersession,
    LockedFact,
    ConsolidationNotReady,
}

pub fn validate_state(state: &SoulState) -> Result<(), SoulPolicyError> {
    if state.revision.get() == 0 {
        return Err(SoulPolicyError::InvalidFact);
    }
    let mut ids = HashSet::new();
    for fact in &state.facts {
        if fact.id.trim().is_empty()
            || fact.value.trim().is_empty()
            || fact.slot.trim().is_empty()
            || !fact.confidence.is_finite()
            || !(0.0..=1.0).contains(&fact.confidence)
            || !fact.weight.is_finite()
            || !(0.0..=1.0).contains(&fact.weight)
            || fact
                .valid_until
                .is_some_and(|until| until.get() <= fact.valid_from.get())
            || !ids.insert(fact.id.as_str())
            || fact.source_memory_ids.iter().any(|id| id.trim().is_empty())
            || fact.supersedes.iter().any(|id| id.trim().is_empty())
            || fact.superseded_by.is_some() != fact.superseded_at.is_some()
            || fact
                .superseded_by
                .as_deref()
                .is_some_and(|id| id.trim().is_empty())
        {
            return Err(SoulPolicyError::InvalidFact);
        }
    }
    Ok(())
}

pub fn normalize_authored_fact(
    mut proposal: ProposedSoulFact,
    now: TimestampMillis,
) -> Result<SoulFact, SoulPolicyError> {
    proposal.confidence = proposal.confidence.clamp(0.0, 1.0);
    proposal.weight = proposal.weight.clamp(0.0, 1.0);
    if proposal.slot.trim().is_empty()
        || proposal.id.trim().is_empty()
        || proposal.value.trim().is_empty()
        || !proposal.confidence.is_finite()
        || proposal.confidence < 0.7
        || !proposal.weight.is_finite()
    {
        return Err(SoulPolicyError::InvalidFact);
    }
    Ok(SoulFact {
        id: proposal.id,
        category: proposal.category,
        value: proposal.value.trim().to_owned(),
        kind: SoulFactKind::Authored,
        policy: proposal.policy,
        slot: proposal.slot.trim().to_owned(),
        confidence: proposal.confidence,
        evidence_count: 1,
        weight: proposal.weight,
        valid_from: now,
        valid_until: None,
        locked: proposal.locked || proposal.policy == SoulFactPolicy::Historical,
        source_memory_ids: Vec::new(),
        created_at: now,
        supersedes: Vec::new(),
        superseded_by: None,
        superseded_at: None,
    })
}

fn normalize_proposal(
    mut proposal: ProposedSoulFact,
    now: TimestampMillis,
) -> Result<ProposedSoulFact, SoulPolicyError> {
    proposal.confidence = proposal.confidence.clamp(0.0, 1.0);
    proposal.weight = proposal.weight.clamp(0.0, 1.0);
    if proposal.slot.trim().is_empty() {
        proposal.slot = proposal.category.as_str().to_owned();
    }
    if proposal.id.trim().is_empty()
        || proposal.value.trim().is_empty()
        || !proposal.confidence.is_finite()
        || proposal.confidence < proposal.category.minimum_confidence()
        || !proposal.weight.is_finite()
        || proposal.weight == 0.0
        || proposal
            .source_memory_ids
            .iter()
            .any(|id| id.trim().is_empty())
        || proposal
            .valid_until
            .is_some_and(|until| until.get() <= now.get())
    {
        return Err(SoulPolicyError::InvalidFact);
    }
    Ok(proposal)
}

pub fn prepare_growth_change_set(
    state: &SoulState,
    expected_revision: Revision,
    proposals: Vec<ProposedSoulFact>,
    now: TimestampMillis,
) -> Result<SoulChangeSet, SoulPolicyError> {
    validate_state(state)?;
    if state.revision != expected_revision {
        return Err(SoulPolicyError::StaleRevision);
    }
    prepare_change_set(state, expected_revision, proposals, Vec::new(), now, false)
}

pub fn prepare_consolidation_change_set(
    state: &SoulState,
    expected_revision: Revision,
    proposals: Vec<ProposedSoulFact>,
    retire_ids: Vec<String>,
    now: TimestampMillis,
) -> Result<SoulChangeSet, SoulPolicyError> {
    validate_state(state)?;
    if state.revision != expected_revision {
        return Err(SoulPolicyError::StaleRevision);
    }
    let active_changeable = state
        .facts
        .iter()
        .filter(|fact| fact.is_effective_at(now) && fact.category.is_changeable())
        .count();
    if active_changeable < CONSOLIDATION_THRESHOLD {
        return Err(SoulPolicyError::ConsolidationNotReady);
    }
    let proposals = proposals
        .into_iter()
        .filter(|proposal| {
            matches!(
                proposal.category,
                SoulCategory::Essence | SoulCategory::Traits
            )
        })
        .collect();
    prepare_change_set(state, expected_revision, proposals, retire_ids, now, true)
}

fn prepare_change_set(
    state: &SoulState,
    expected_revision: Revision,
    proposals: Vec<ProposedSoulFact>,
    retire_ids: Vec<String>,
    now: TimestampMillis,
    consolidation: bool,
) -> Result<SoulChangeSet, SoulPolicyError> {
    let mut state_ids = HashSet::new();
    if state
        .facts
        .iter()
        .any(|fact| fact.id.trim().is_empty() || !state_ids.insert(fact.id.as_str()))
    {
        return Err(SoulPolicyError::DuplicateIdentity);
    }
    let existing: HashMap<_, _> = state
        .facts
        .iter()
        .map(|fact| (fact.id.as_str(), fact))
        .collect();
    let mut identities: HashSet<String> = existing.keys().map(|id| (*id).to_owned()).collect();
    let mut superseded = HashSet::new();
    let mut current_slots = HashSet::new();
    let mut supersessions = Vec::new();
    let mut additions = Vec::new();
    for proposal in proposals {
        let proposal = normalize_proposal(proposal, now)?;
        if !consolidation && !proposal.category.is_changeable() {
            return Err(SoulPolicyError::InvalidFact);
        }
        if proposal.policy == SoulFactPolicy::Current
            && !current_slots.insert((proposal.category, proposal.slot.clone()))
        {
            return Err(SoulPolicyError::InvalidSupersession);
        }
        if !identities.insert(proposal.id.clone()) {
            return Err(SoulPolicyError::DuplicateIdentity);
        }
        let mut targets = proposal.supersedes.clone();
        if proposal.policy == SoulFactPolicy::Current {
            for fact in &state.facts {
                if fact.is_active()
                    && fact.category == proposal.category
                    && (fact.slot == proposal.slot
                        || (fact.slot.is_empty() && proposal.slot == proposal.category.as_str()))
                {
                    if fact.locked {
                        return Err(SoulPolicyError::LockedFact);
                    }
                    targets.push(fact.id.clone());
                }
            }
        }
        targets.sort();
        targets.dedup();
        for target in &targets {
            let fact = existing
                .get(target.as_str())
                .ok_or(SoulPolicyError::InvalidSupersession)?;
            if !fact.is_active() || fact.category != proposal.category {
                return Err(SoulPolicyError::InvalidSupersession);
            }
            if fact.locked {
                return Err(SoulPolicyError::LockedFact);
            }
            if !superseded.insert(target.clone()) {
                return Err(SoulPolicyError::InvalidSupersession);
            }
            supersessions.push(SoulSupersession {
                fact_id: target.clone(),
                superseded_by: proposal.id.clone(),
            });
        }
        additions.push(SoulFact {
            id: proposal.id,
            category: proposal.category,
            value: proposal.value.trim().to_owned(),
            kind: if consolidation {
                SoulFactKind::Consolidated
            } else {
                proposal.kind
            },
            policy: proposal.policy,
            slot: proposal.slot.trim().to_owned(),
            confidence: proposal.confidence,
            evidence_count: u32::try_from(proposal.source_memory_ids.len())
                .map_err(|_| SoulPolicyError::InvalidFact)?,
            weight: proposal.weight,
            valid_from: now,
            valid_until: proposal.valid_until,
            locked: proposal.locked || proposal.policy == SoulFactPolicy::Historical,
            source_memory_ids: proposal.source_memory_ids,
            created_at: now,
            supersedes: targets,
            superseded_by: None,
            superseded_at: None,
        });
    }
    let mut retire_ids = retire_ids;
    retire_ids.sort();
    retire_ids.dedup();
    for id in retire_ids {
        if let Some(fact) = existing.get(id.as_str()) {
            if fact.is_active() && !fact.locked && superseded.insert(id.clone()) {
                supersessions.push(SoulSupersession {
                    fact_id: id,
                    superseded_by: "consolidation".into(),
                });
            }
        }
    }
    supersessions.sort_by(|left, right| left.fact_id.cmp(&right.fact_id));
    Ok(SoulChangeSet {
        expected_revision,
        resulting_revision: expected_revision
            .next()
            .map_err(|_| SoulPolicyError::InvalidFact)?,
        additions,
        supersessions,
        applied_at: now,
    })
}

pub fn apply_change_set(
    state: &SoulState,
    change_set: &SoulChangeSet,
) -> Result<SoulState, SoulPolicyError> {
    validate_state(state)?;
    if state.revision != change_set.expected_revision {
        return Err(SoulPolicyError::StaleRevision);
    }
    if change_set.resulting_revision
        != change_set
            .expected_revision
            .next()
            .map_err(|_| SoulPolicyError::InvalidFact)?
        || change_set
            .additions
            .iter()
            .any(|fact| fact.created_at != change_set.applied_at)
    {
        return Err(SoulPolicyError::InvalidFact);
    }
    let mut facts = state.facts.clone();
    for supersession in &change_set.supersessions {
        let fact = facts
            .iter_mut()
            .find(|fact| fact.id == supersession.fact_id && fact.is_active())
            .ok_or(SoulPolicyError::InvalidSupersession)?;
        if fact.locked {
            return Err(SoulPolicyError::LockedFact);
        }
        fact.superseded_by = Some(supersession.superseded_by.clone());
        fact.superseded_at = Some(change_set.applied_at);
    }
    facts.extend(change_set.additions.clone());

    let superseded_count = facts.iter().filter(|entry| !entry.is_active()).count();
    if superseded_count > MAX_SUPERSEDED_HISTORY {
        let mut to_drop = superseded_count - MAX_SUPERSEDED_HISTORY;
        facts.retain(|entry| {
            if to_drop > 0 && !entry.is_active() {
                to_drop -= 1;
                false
            } else {
                true
            }
        });
    }
    let state = SoulState {
        revision: change_set.resulting_revision,
        facts,
    };
    validate_state(&state)?;
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proposed(id: &str, category: SoulCategory, policy: SoulFactPolicy) -> ProposedSoulFact {
        ProposedSoulFact {
            id: id.into(),
            category,
            value: format!("value-{id}"),
            kind: SoulFactKind::Add,
            policy,
            slot: "slot".into(),
            confidence: 1.0,
            weight: 1.0,
            valid_until: None,
            locked: false,
            source_memory_ids: vec!["memory-1".into()],
            supersedes: Vec::new(),
        }
    }

    fn fact(id: &str, category: SoulCategory, slot: &str, locked: bool) -> SoulFact {
        let mut value = normalize_authored_fact(
            ProposedSoulFact {
                slot: slot.into(),
                locked,
                ..proposed(id, category, SoulFactPolicy::Adaptive)
            },
            TimestampMillis::new(1),
        )
        .expect("fact");
        value.locked = locked;
        value
    }

    #[test]
    fn category_policy_copies_legacy_thresholds() {
        assert_eq!(SoulCategory::Likes.minimum_confidence(), 0.55);
        assert_eq!(SoulCategory::Fears.minimum_confidence(), 0.70);
        assert_eq!(SoulCategory::Traits.minimum_confidence(), 0.85);
        assert_eq!(SoulCategory::Backstory.minimum_confidence(), 1.0);
        assert!(SoulCategory::Likes.is_changeable());
        assert!(!SoulCategory::Traits.is_changeable());
        assert!(SoulCategory::Traits.is_consolidatable());
        assert!(!SoulCategory::Backstory.is_consolidatable());
    }

    #[test]
    fn growth_clamps_like_legacy_then_enforces_category_weight_and_validity() {
        let state = SoulState {
            revision: Revision::INITIAL,
            facts: Vec::new(),
        };
        let mut accepted = proposed("new", SoulCategory::Likes, SoulFactPolicy::Current);
        accepted.confidence = 2.0;
        accepted.weight = 4.0;
        accepted.slot.clear();
        let change = prepare_growth_change_set(
            &state,
            Revision::INITIAL,
            vec![accepted],
            TimestampMillis::new(10),
        )
        .expect("accepted");
        assert_eq!(change.additions[0].confidence, 1.0);
        assert_eq!(change.additions[0].weight, 1.0);
        assert_eq!(change.additions[0].slot, "likes");

        let mut low = proposed("low", SoulCategory::Fears, SoulFactPolicy::Adaptive);
        low.confidence = 0.69;
        assert_eq!(
            prepare_growth_change_set(
                &state,
                Revision::INITIAL,
                vec![low],
                TimestampMillis::new(10)
            ),
            Err(SoulPolicyError::InvalidFact)
        );
        let mut zero = proposed("zero", SoulCategory::Likes, SoulFactPolicy::Adaptive);
        zero.weight = -1.0;
        assert_eq!(
            prepare_growth_change_set(
                &state,
                Revision::INITIAL,
                vec![zero],
                TimestampMillis::new(10)
            ),
            Err(SoulPolicyError::InvalidFact)
        );
    }

    #[test]
    fn current_same_slot_supersedes_unlocked_and_rejects_locked_atomically() {
        let state = SoulState {
            revision: Revision::INITIAL,
            facts: vec![fact("old", SoulCategory::Likes, "food", false)],
        };
        let change = prepare_growth_change_set(
            &state,
            Revision::INITIAL,
            vec![ProposedSoulFact {
                slot: "food".into(),
                ..proposed("new", SoulCategory::Likes, SoulFactPolicy::Current)
            }],
            TimestampMillis::new(2),
        )
        .expect("change");
        assert_eq!(change.supersessions[0].fact_id, "old");
        let applied = apply_change_set(&state, &change).expect("apply");
        assert_eq!(applied.facts[0].superseded_by.as_deref(), Some("new"));

        let locked = SoulState {
            facts: vec![fact("locked", SoulCategory::Likes, "food", true)],
            ..state
        };
        assert_eq!(
            prepare_growth_change_set(
                &locked,
                Revision::INITIAL,
                vec![ProposedSoulFact {
                    slot: "food".into(),
                    ..proposed("blocked", SoulCategory::Likes, SoulFactPolicy::Current)
                }],
                TimestampMillis::new(2)
            ),
            Err(SoulPolicyError::LockedFact)
        );
    }

    #[test]
    fn consolidation_threshold_core_filter_and_locked_retirement_match_legacy() {
        let mut facts = (0..12)
            .map(|index| {
                fact(
                    &format!("growth-{index}"),
                    SoulCategory::Habits,
                    "habit",
                    false,
                )
            })
            .collect::<Vec<_>>();
        facts.push(fact("locked", SoulCategory::Traits, "core", true));
        let state = SoulState {
            revision: Revision::INITIAL,
            facts,
        };
        let change = prepare_consolidation_change_set(
            &state,
            Revision::INITIAL,
            vec![
                proposed("core", SoulCategory::Traits, SoulFactPolicy::Adaptive),
                proposed("ignored", SoulCategory::Likes, SoulFactPolicy::Adaptive),
            ],
            vec!["growth-0".into(), "locked".into(), "missing".into()],
            TimestampMillis::new(2),
        )
        .expect("consolidation");
        assert_eq!(change.additions.len(), 1);
        assert_eq!(change.additions[0].category, SoulCategory::Traits);
        assert_eq!(change.supersessions.len(), 1);
        assert_eq!(change.supersessions[0].fact_id, "growth-0");

        let not_ready = SoulState {
            facts: state.facts[..11].to_vec(),
            ..state
        };
        assert_eq!(
            prepare_consolidation_change_set(
                &not_ready,
                Revision::INITIAL,
                Vec::new(),
                Vec::new(),
                TimestampMillis::new(2)
            ),
            Err(SoulPolicyError::ConsolidationNotReady)
        );
    }

    #[test]
    fn authored_historical_lock_and_point_seven_gate_are_exact() {
        let mut historical = proposed(
            "history",
            SoulCategory::Backstory,
            SoulFactPolicy::Historical,
        );
        historical.confidence = 0.7;
        historical.source_memory_ids.clear();
        assert!(
            normalize_authored_fact(historical, TimestampMillis::new(1))
                .expect("fact")
                .locked
        );
        let mut low = proposed("low", SoulCategory::Traits, SoulFactPolicy::Adaptive);
        low.confidence = 0.699;
        assert_eq!(
            normalize_authored_fact(low, TimestampMillis::new(1)),
            Err(SoulPolicyError::InvalidFact)
        );
    }

    #[test]
    fn invalid_batches_and_stale_revisions_fail_without_partial_state() {
        let state = SoulState {
            revision: Revision::INITIAL,
            facts: vec![fact("existing", SoulCategory::Likes, "food", false)],
        };
        let mut invalid = proposed(
            "invalid",
            SoulCategory::Backstory,
            SoulFactPolicy::Historical,
        );
        invalid.valid_until = Some(TimestampMillis::new(1));
        assert_eq!(
            prepare_growth_change_set(
                &state,
                Revision::INITIAL,
                vec![
                    proposed("valid", SoulCategory::Likes, SoulFactPolicy::Adaptive),
                    invalid
                ],
                TimestampMillis::new(2)
            ),
            Err(SoulPolicyError::InvalidFact)
        );
        assert_eq!(state.facts.len(), 1);
        assert_eq!(
            prepare_growth_change_set(
                &state,
                Revision::new(2),
                Vec::new(),
                TimestampMillis::new(2)
            ),
            Err(SoulPolicyError::StaleRevision)
        );
        assert_eq!(
            prepare_growth_change_set(
                &state,
                Revision::INITIAL,
                vec![
                    proposed("same", SoulCategory::Likes, SoulFactPolicy::Adaptive),
                    proposed("same", SoulCategory::Likes, SoulFactPolicy::Adaptive),
                ],
                TimestampMillis::new(2)
            ),
            Err(SoulPolicyError::DuplicateIdentity)
        );
    }

    #[test]
    fn applied_state_keeps_exactly_forty_old_superseded_facts() {
        let facts = (0..41)
            .map(|index| {
                let mut value = fact(&format!("old-{index}"), SoulCategory::Likes, "food", false);
                value.superseded_by = Some("prior".into());
                value.superseded_at = Some(TimestampMillis::new(2));
                value
            })
            .collect::<Vec<_>>();
        let state = SoulState {
            revision: Revision::INITIAL,
            facts,
        };
        let applied = apply_change_set(
            &state,
            &SoulChangeSet {
                expected_revision: Revision::INITIAL,
                resulting_revision: Revision::new(2),
                additions: Vec::new(),
                supersessions: Vec::new(),
                applied_at: TimestampMillis::new(2),
            },
        )
        .expect("apply");
        assert_eq!(applied.facts.len(), MAX_SUPERSEDED_HISTORY);
        assert_eq!(applied.facts[0].id, "old-1");
    }
}
