use std::collections::{HashMap, HashSet};

use lettuce_types::{
    CharacterId, ConversationId, OperationRecordId, PromptDocumentId, Revision, TimestampMillis,
};
use serde::{Deserialize, Serialize};

use crate::state::{EmotionVector, RegulationStyle, RelationshipDefaults};

pub const CONSOLIDATION_THRESHOLD: usize = 12;
pub const MAX_SUPERSEDED_HISTORY: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoulFactPolicy {
    Current,
    Adaptive,
    Historical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoulFactKind {
    Add,
    Adjust,
    Authored,
    Consolidated,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SoulFact {
    pub id: String,
    pub category: SoulCategory,
    pub value: String,
    #[serde(default = "authored_fact_kind")]
    pub kind: SoulFactKind,
    pub policy: SoulFactPolicy,
    pub slot: String,
    #[serde(default = "full_strength")]
    pub confidence: f64,
    #[serde(default = "one_evidence")]
    pub evidence_count: u32,
    #[serde(default = "full_strength")]
    pub weight: f64,
    #[serde(default = "epoch")]
    pub valid_from: TimestampMillis,
    #[serde(default)]
    pub valid_until: Option<TimestampMillis>,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub source_memory_ids: Vec<String>,
    #[serde(default = "epoch")]
    pub created_at: TimestampMillis,
    #[serde(default)]
    pub supersedes: Vec<String>,
    #[serde(default)]
    pub superseded_by: Option<String>,
    #[serde(default)]
    pub superseded_at: Option<TimestampMillis>,
}

const fn authored_fact_kind() -> SoulFactKind {
    SoulFactKind::Authored
}

const fn full_strength() -> f64 {
    1.0
}

const fn one_evidence() -> u32 {
    1
}

const fn epoch() -> TimestampMillis {
    TimestampMillis::UNIX_EPOCH
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct CompanionSoulIdentity {
    pub essence: String,
    pub traits: String,
    pub backstory: String,
    pub appearance: String,
    pub goals: String,
    pub likes: String,
    pub voice: String,
    pub relational_style: String,
    pub vulnerabilities: String,
    pub fears: String,
    pub habits: String,
    pub boundaries: String,
    pub baseline_affect: EmotionVector,
    pub regulation_style: RegulationStyle,
}

impl Default for CompanionSoulIdentity {
    fn default() -> Self {
        Self {
            essence: String::new(),
            traits: String::new(),
            backstory: String::new(),
            appearance: String::new(),
            goals: String::new(),
            likes: String::new(),
            voice: String::new(),
            relational_style: String::new(),
            vulnerabilities: String::new(),
            fears: String::new(),
            habits: String::new(),
            boundaries: String::new(),
            baseline_affect: EmotionVector {
                warmth: 0.45,
                trust: 0.35,
                calm: 0.65,
                vulnerability: 0.2,
                longing: 0.15,
                hurt: 0.05,
                tension: 0.1,
                irritation: 0.05,
                affection_intensity: 0.25,
                reassurance_need: 0.15,
            },
            regulation_style: RegulationStyle::default(),
        }
    }
}

impl CompanionSoulIdentity {
    pub fn values(&self) -> impl Iterator<Item = &str> {
        [
            self.essence.as_str(),
            self.traits.as_str(),
            self.backstory.as_str(),
            self.appearance.as_str(),
            self.goals.as_str(),
            self.likes.as_str(),
            self.voice.as_str(),
            self.relational_style.as_str(),
            self.vulnerabilities.as_str(),
            self.fears.as_str(),
            self.habits.as_str(),
            self.boundaries.as_str(),
        ]
        .into_iter()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompanionPromptingConfig {
    #[serde(default)]
    pub prompt_template_id: Option<PromptDocumentId>,
    #[serde(default)]
    pub style_notes: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompanionSoulConfig {
    #[serde(default)]
    pub soul: CompanionSoulIdentity,
    #[serde(default)]
    pub authored_facts: Vec<SoulFact>,
    #[serde(default)]
    pub relationship_defaults: RelationshipDefaults,
    #[serde(default)]
    pub prompting: CompanionPromptingConfig,
    /// Whether this companion's conversations are time aware unless a
    /// conversation sets its own clock (legacy `timeAwareness`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub time_awareness: bool,
    /// Whether the companion's conversations use one memory pool (legacy
    /// `memory.sharedAcrossSessions`); off, each uses its own memory.
    #[serde(default = "shared", skip_serializing_if = "is_shared")]
    pub share_memory_across_chats: bool,
    /// Whether the companion's conversations grow one Soul; off, each grows
    /// its own.
    #[serde(default = "shared", skip_serializing_if = "is_shared")]
    pub share_soul_growth_across_chats: bool,
}

const fn shared() -> bool {
    true
}

#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_shared(value: &bool) -> bool {
    *value
}

impl Default for CompanionSoulConfig {
    fn default() -> Self {
        Self {
            soul: CompanionSoulIdentity::default(),
            authored_facts: Vec::new(),
            relationship_defaults: RelationshipDefaults::default(),
            prompting: CompanionPromptingConfig::default(),
            time_awareness: false,
            share_memory_across_chats: true,
            share_soul_growth_across_chats: true,
        }
    }
}

pub fn initial_soul_state(
    config: Option<&CompanionSoulConfig>,
    now: TimestampMillis,
) -> Result<SoulState, SoulPolicyError> {
    let mut facts = config
        .map(|config| config.authored_facts.clone())
        .unwrap_or_default();
    for fact in &mut facts {
        if fact.id.trim().is_empty() {
            fact.id = uuid::Uuid::new_v4().to_string();
        }
        fact.confidence = fact.confidence.clamp(0.0, 1.0);
        fact.weight = fact.weight.clamp(0.0, 1.0);
        if fact.slot.trim().is_empty() {
            fact.slot = fact.category.as_str().to_owned();
        }
        if fact.evidence_count == 0 {
            fact.evidence_count = u32::try_from(fact.source_memory_ids.len())
                .map_err(|_| SoulPolicyError::InvalidFact)?;
        }
        if fact.created_at == TimestampMillis::UNIX_EPOCH {
            fact.created_at = now;
        }
        if fact.valid_from == TimestampMillis::UNIX_EPOCH {
            fact.valid_from = fact.created_at;
        }
        if fact.policy == SoulFactPolicy::Historical {
            fact.locked = true;
        }
    }
    let state = SoulState {
        revision: Revision::INITIAL,
        facts,
    };
    validate_state(&state)?;
    Ok(state)
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoulState {
    pub revision: Revision,
    pub facts: Vec<SoulFact>,
}

/// Whose Soul grows: the character's, shared by all of its conversations
/// (legacy), or one conversation's own while the character does not share
/// Soul growth across chats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoulOwner {
    Character(CharacterId),
    Conversation {
        character_id: CharacterId,
        conversation_id: ConversationId,
    },
}

impl SoulOwner {
    /// The Soul a conversation grows given its character's toggle.
    #[must_use]
    pub const fn for_conversation(
        character_id: CharacterId,
        conversation_id: ConversationId,
        shared: bool,
    ) -> Self {
        if shared {
            Self::Character(character_id)
        } else {
            Self::Conversation {
                character_id,
                conversation_id,
            }
        }
    }

    #[must_use]
    pub const fn character_id(self) -> CharacterId {
        match self {
            Self::Character(id)
            | Self::Conversation {
                character_id: id, ..
            } => id,
        }
    }

    #[must_use]
    pub const fn conversation_id(self) -> Option<ConversationId> {
        match self {
            Self::Character(_) => None,
            Self::Conversation {
                conversation_id, ..
            } => Some(conversation_id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// A user's direct edit of Soul growth (legacy `companion_clear_soul_growth`,
/// `companion_remove_soul_growth` and `companion_set_soul_growth_lock`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SoulUserEdit {
    ClearAll,
    Remove { fact_id: String },
    SetLocked { fact_id: String, locked: bool },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SoulChangeSet {
    pub expected_revision: Revision,
    pub resulting_revision: Revision,
    pub additions: Vec<SoulFact>,
    pub supersessions: Vec<SoulSupersession>,
    pub user_edits: Vec<SoulUserEdit>,
    /// The Soul's own time for the change: the timestamp of added facts and
    /// supersessions, which follows the conversation's companion clock.
    pub applied_at: TimestampMillis,
    /// Wall time the change is written at; it orders Souls by recency and is
    /// not part of the change's identity.
    pub recorded_at: TimestampMillis,
}

/// The change set for one user edit, or `None` when it changes nothing: legacy
/// answered clearing an empty Soul with 0, removing an unknown entry with
/// false and setting a lock to its current value with true, all without a
/// write.
pub fn prepare_user_edit(
    state: &SoulState,
    edit: SoulUserEdit,
    now: TimestampMillis,
) -> Result<Option<SoulChangeSet>, SoulPolicyError> {
    let changes = match &edit {
        SoulUserEdit::ClearAll => !state.facts.is_empty(),
        SoulUserEdit::Remove { fact_id } => state.facts.iter().any(|fact| &fact.id == fact_id),
        SoulUserEdit::SetLocked { fact_id, locked } => state
            .facts
            .iter()
            .any(|fact| &fact.id == fact_id && fact.locked != *locked),
    };
    if !changes {
        return Ok(None);
    }
    Ok(Some(SoulChangeSet {
        expected_revision: state.revision,
        resulting_revision: state
            .revision
            .next()
            .map_err(|_| SoulPolicyError::InvalidFact)?,
        additions: Vec::new(),
        supersessions: Vec::new(),
        user_edits: vec![edit],
        applied_at: now,
        recorded_at: now,
    }))
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
            || fact.valid_from.get() < 0
            || fact.created_at.get() < 0
            || fact
                .valid_until
                .is_some_and(|until| until.get() < 0 || until.get() <= fact.valid_from.get())
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

/// Legacy `append_soul_growth_gated`: each proposal is judged on its own and
/// a bad one (below the category's confidence, zero weight, a locked current
/// slot, an unusable validity window, a reused id) is skipped while the rest
/// apply. A current proposal supersedes the unlocked active facts of its slot
/// and an explicit `supersedes` id only an unlocked active fact of its
/// category; other ids are ignored. Facts added earlier in the same batch take
/// part in both, so a later current proposal supersedes an earlier one.
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
    let mut supersessions = Vec::new();
    let mut additions: Vec<SoulFact> = Vec::new();
    for proposal in proposals {
        let Ok(proposal) = normalize_proposal(proposal, now) else {
            continue;
        };
        if (!consolidation && !proposal.category.is_changeable())
            || identities.contains(&proposal.id)
        {
            continue;
        }
        let same_slot = |category: SoulCategory, slot: &str| {
            category == proposal.category
                && (slot == proposal.slot
                    || (slot.is_empty() && proposal.slot == proposal.category.as_str()))
        };
        let current = proposal.policy == SoulFactPolicy::Current;
        if current
            && (state.facts.iter().any(|fact| {
                fact.is_active()
                    && !superseded.contains(&fact.id)
                    && fact.locked
                    && same_slot(fact.category, &fact.slot)
            }) || additions.iter().any(|fact| {
                fact.is_active() && fact.locked && same_slot(fact.category, &fact.slot)
            }))
        {
            continue;
        }
        let targeted = |category: SoulCategory, slot: &str, id: &str| {
            (current && same_slot(category, slot))
                || (category == proposal.category
                    && proposal.supersedes.iter().any(|target| target == id))
        };
        let mut targets = Vec::new();
        for fact in &state.facts {
            if fact.is_active()
                && !fact.locked
                && !superseded.contains(&fact.id)
                && targeted(fact.category, &fact.slot, &fact.id)
            {
                superseded.insert(fact.id.clone());
                targets.push(fact.id.clone());
                supersessions.push(SoulSupersession {
                    fact_id: fact.id.clone(),
                    superseded_by: proposal.id.clone(),
                });
            }
        }
        for fact in &mut additions {
            if fact.is_active() && !fact.locked && targeted(fact.category, &fact.slot, &fact.id) {
                targets.push(fact.id.clone());
                fact.superseded_by = Some(proposal.id.clone());
                fact.superseded_at = Some(now);
            }
        }
        targets.sort();
        targets.dedup();
        identities.insert(proposal.id.clone());
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
        user_edits: Vec::new(),
        applied_at: now,
        recorded_at: now,
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
    for edit in &change_set.user_edits {
        match edit {
            SoulUserEdit::ClearAll => facts.clear(),
            SoulUserEdit::Remove { fact_id } => {
                let before = facts.len();
                facts.retain(|fact| &fact.id != fact_id);
                if facts.len() == before {
                    return Err(SoulPolicyError::InvalidFact);
                }
            }
            SoulUserEdit::SetLocked { fact_id, locked } => {
                facts
                    .iter_mut()
                    .find(|fact| &fact.id == fact_id)
                    .ok_or(SoulPolicyError::InvalidFact)?
                    .locked = *locked;
            }
        }
    }
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
    if change_set.user_edits.is_empty() && superseded_count > MAX_SUPERSEDED_HISTORY {
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
    fn initial_state_copies_legacy_authored_fact_normalization() {
        let config = CompanionSoulConfig {
            soul: CompanionSoulIdentity {
                essence: "Steady".into(),
                fears: "Being forgotten".into(),
                ..CompanionSoulIdentity::default()
            },
            authored_facts: vec![SoulFact {
                id: String::new(),
                category: SoulCategory::Backstory,
                value: "Moved to the coast".into(),
                kind: SoulFactKind::Authored,
                policy: SoulFactPolicy::Historical,
                slot: String::new(),
                confidence: 2.0,
                evidence_count: 0,
                weight: -1.0,
                valid_from: TimestampMillis::UNIX_EPOCH,
                valid_until: None,
                locked: false,
                source_memory_ids: vec!["memory-a".into()],
                created_at: TimestampMillis::UNIX_EPOCH,
                supersedes: Vec::new(),
                superseded_by: None,
                superseded_at: None,
            }],
            relationship_defaults: RelationshipDefaults::default(),
            prompting: CompanionPromptingConfig::default(),
            time_awareness: false,
            share_memory_across_chats: true,
            share_soul_growth_across_chats: true,
        };
        let state = initial_soul_state(Some(&config), TimestampMillis::new(42)).expect("state");
        assert_eq!(state.revision, Revision::INITIAL);
        assert_eq!(state.facts.len(), 1);
        let fact = &state.facts[0];
        assert!(!fact.id.is_empty());
        assert_eq!(fact.slot, "backstory");
        assert_eq!(fact.confidence, 1.0);
        assert_eq!(fact.weight, 0.0);
        assert_eq!(fact.evidence_count, 1);
        assert_eq!(fact.created_at, TimestampMillis::new(42));
        assert_eq!(fact.valid_from, TimestampMillis::new(42));
        assert!(fact.locked);
    }

    #[test]
    fn initial_state_without_authored_config_is_empty() {
        assert_eq!(
            initial_soul_state(None, TimestampMillis::new(42)),
            Ok(SoulState {
                revision: Revision::INITIAL,
                facts: Vec::new(),
            })
        );
    }

    #[test]
    fn authored_config_uses_legacy_camel_case_field_names() {
        let config = CompanionSoulConfig {
            soul: CompanionSoulIdentity {
                relational_style: "Slow trust".into(),
                ..CompanionSoulIdentity::default()
            },
            authored_facts: vec![fact(
                "authored",
                SoulCategory::RelationalStyle,
                "trust",
                false,
            )],
            relationship_defaults: RelationshipDefaults::default(),
            prompting: CompanionPromptingConfig {
                prompt_template_id: Some(PromptDocumentId::new()),
                style_notes: " restrained ".into(),
            },
            time_awareness: false,
            share_memory_across_chats: true,
            share_soul_growth_across_chats: true,
        };
        let value = serde_json::to_value(&config).expect("serialize");
        assert!(value.get("shareMemoryAcrossChats").is_none());
        let private: CompanionSoulConfig =
            serde_json::from_value(serde_json::json!({"shareMemoryAcrossChats": false}))
                .expect("private memory");
        assert!(!private.share_memory_across_chats);
        assert!(private.share_soul_growth_across_chats);
        assert_eq!(
            serde_json::to_value(&private).expect("serialize")["shareMemoryAcrossChats"],
            false
        );
        assert_eq!(value["soul"]["relationalStyle"], "Slow trust");
        assert!(value.get("authoredFacts").is_some());
        assert_eq!(
            value["authoredFacts"][0]["sourceMemoryIds"],
            serde_json::json!([])
        );
        assert_eq!(value["authoredFacts"][0]["category"], "relationalStyle");
        assert!(value["prompting"]["promptTemplateId"].is_string());
        assert_eq!(value["prompting"]["styleNotes"], " restrained ");

        let decoded: CompanionSoulConfig = serde_json::from_value(serde_json::json!({
            "soul": { "relationalStyle": "Slow trust" },
            "authoredFacts": [{
                "id": "legacy",
                "category": "backstory",
                "value": "Moved to the coast",
                "policy": "historical",
                "slot": "coast-move"
            }],
            "prompting": {
                "styleNotes": "warm but reserved"
            }
        }))
        .expect("legacy-shaped config");
        assert_eq!(decoded.soul.relational_style, "Slow trust");
        assert_eq!(decoded.authored_facts[0].kind, SoulFactKind::Authored);
        assert_eq!(decoded.authored_facts[0].confidence, 1.0);
        assert_eq!(decoded.authored_facts[0].evidence_count, 1);
        assert_eq!(decoded.authored_facts[0].weight, 1.0);
        assert_eq!(decoded.prompting.style_notes, "warm but reserved");
        assert_eq!(decoded.prompting.prompt_template_id, None);
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
        let mut zero = proposed("zero", SoulCategory::Likes, SoulFactPolicy::Adaptive);
        zero.weight = -1.0;
        let change = prepare_growth_change_set(
            &state,
            Revision::INITIAL,
            vec![low, zero],
            TimestampMillis::new(10),
        )
        .expect("bad proposals are skipped");
        assert!(change.additions.is_empty());
    }

    #[test]
    fn current_same_slot_supersedes_unlocked_and_skips_a_locked_slot() {
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
        let change = prepare_growth_change_set(
            &locked,
            Revision::INITIAL,
            vec![
                ProposedSoulFact {
                    slot: "food".into(),
                    ..proposed("blocked", SoulCategory::Likes, SoulFactPolicy::Current)
                },
                proposed("kept", SoulCategory::Habits, SoulFactPolicy::Adaptive),
            ],
            TimestampMillis::new(2),
        )
        .expect("change");
        assert_eq!(
            change
                .additions
                .iter()
                .map(|fact| fact.id.as_str())
                .collect::<Vec<_>>(),
            ["kept"]
        );
        assert!(change.supersessions.is_empty());
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
    fn invalid_items_are_skipped_and_stale_revisions_fail() {
        let state = SoulState {
            revision: Revision::INITIAL,
            facts: vec![fact("existing", SoulCategory::Likes, "food", false)],
        };
        let mut invalid = proposed("invalid", SoulCategory::Likes, SoulFactPolicy::Adaptive);
        invalid.valid_until = Some(TimestampMillis::new(1));
        let change = prepare_growth_change_set(
            &state,
            Revision::INITIAL,
            vec![
                proposed("valid", SoulCategory::Likes, SoulFactPolicy::Adaptive),
                invalid,
                proposed("valid", SoulCategory::Likes, SoulFactPolicy::Adaptive),
                proposed("existing", SoulCategory::Likes, SoulFactPolicy::Adaptive),
            ],
            TimestampMillis::new(2),
        )
        .expect("change");
        assert_eq!(
            change
                .additions
                .iter()
                .map(|fact| fact.id.as_str())
                .collect::<Vec<_>>(),
            ["valid"]
        );
        assert_eq!(
            prepare_growth_change_set(
                &state,
                Revision::new(2),
                Vec::new(),
                TimestampMillis::new(2)
            ),
            Err(SoulPolicyError::StaleRevision)
        );
    }

    /// Legacy `append_soul_growth_gated` (companion/mod.rs 1182-1246): a
    /// low-confidence item, an unknown or cross-category `supersedes` id and a
    /// second current fact in one slot never discard the rest of the batch.
    #[test]
    fn growth_batch_skips_bad_items_and_ignores_unusable_supersedes_like_legacy() {
        let state = SoulState {
            revision: Revision::INITIAL,
            facts: vec![
                fact("likes-old", SoulCategory::Likes, "food", false),
                fact("fear-old", SoulCategory::Fears, "fears", false),
            ],
        };
        let mut low = proposed("low", SoulCategory::Fears, SoulFactPolicy::Adaptive);
        low.confidence = 0.6;
        let mut hallucinated = proposed("adjust", SoulCategory::Habits, SoulFactPolicy::Adaptive);
        hallucinated.supersedes = vec!["missing".into(), "likes-old".into()];
        let change = prepare_growth_change_set(
            &state,
            Revision::INITIAL,
            vec![
                ProposedSoulFact {
                    slot: "food".into(),
                    ..proposed("first", SoulCategory::Likes, SoulFactPolicy::Current)
                },
                low,
                hallucinated,
                ProposedSoulFact {
                    slot: "food".into(),
                    ..proposed("second", SoulCategory::Likes, SoulFactPolicy::Current)
                },
            ],
            TimestampMillis::new(5),
        )
        .expect("change");
        assert_eq!(
            change
                .additions
                .iter()
                .map(|fact| (fact.id.as_str(), fact.superseded_by.as_deref()))
                .collect::<Vec<_>>(),
            [
                ("first", Some("second")),
                ("adjust", None),
                ("second", None)
            ]
        );
        assert_eq!(
            change.supersessions,
            vec![SoulSupersession {
                fact_id: "likes-old".into(),
                superseded_by: "first".into(),
            }]
        );
        let applied = apply_change_set(&state, &change).expect("apply");
        assert_eq!(
            applied
                .facts
                .iter()
                .filter(|fact| fact.is_active())
                .map(|fact| fact.id.as_str())
                .collect::<Vec<_>>(),
            ["fear-old", "adjust", "second"]
        );
    }

    /// Legacy consolidation (companion_consolidation.rs 135-138) retires even
    /// when its core adjustment is ignored.
    #[test]
    fn consolidation_retires_even_when_the_core_item_is_skipped() {
        let facts = (0..12)
            .map(|index| {
                fact(
                    &format!("growth-{index}"),
                    SoulCategory::Habits,
                    "habit",
                    false,
                )
            })
            .collect::<Vec<_>>();
        let state = SoulState {
            revision: Revision::INITIAL,
            facts,
        };
        let mut weak = proposed("core", SoulCategory::Traits, SoulFactPolicy::Adaptive);
        weak.confidence = 0.8;
        weak.supersedes = vec!["growth-1".into()];
        let change = prepare_consolidation_change_set(
            &state,
            Revision::INITIAL,
            vec![weak],
            vec!["growth-0".into()],
            TimestampMillis::new(2),
        )
        .expect("consolidation");
        assert!(change.additions.is_empty());
        assert_eq!(change.supersessions.len(), 1);
        assert_eq!(change.supersessions[0].fact_id, "growth-0");
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
                user_edits: Vec::new(),
                applied_at: TimestampMillis::new(2),
                recorded_at: TimestampMillis::new(2),
            },
        )
        .expect("apply");
        assert_eq!(applied.facts.len(), MAX_SUPERSEDED_HISTORY);
        assert_eq!(applied.facts[0].id, "old-1");
    }
}
