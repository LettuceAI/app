use lettuce_types::{
    CharacterId, ConversationId, OperationRecordId, PersonaId, Revision, TimestampMillis,
};
use serde::{Deserialize, Serialize};

use lettuce_conversations::{
    ConversationKind, ConversationRepositoryError, CreateConversationResult,
    PreparedConversationLaunch, SendConversation, SendConversationResult,
};

const DECAY_MINUTES: f64 = 45.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct EmotionVector {
    pub warmth: f64,
    pub trust: f64,
    pub calm: f64,
    pub vulnerability: f64,
    pub longing: f64,
    pub hurt: f64,
    pub tension: f64,
    pub irritation: f64,
    pub affection_intensity: f64,
    pub reassurance_need: f64,
}

impl Default for EmotionVector {
    fn default() -> Self {
        Self {
            warmth: 0.0,
            trust: 0.0,
            calm: 0.0,
            vulnerability: 0.0,
            longing: 0.0,
            hurt: 0.0,
            tension: 0.0,
            irritation: 0.0,
            affection_intensity: 0.0,
            reassurance_need: 0.0,
        }
    }
}

impl EmotionVector {
    #[must_use]
    pub fn is_unit(&self) -> bool {
        [
            self.warmth,
            self.trust,
            self.calm,
            self.vulnerability,
            self.longing,
            self.hurt,
            self.tension,
            self.irritation,
            self.affection_intensity,
            self.reassurance_need,
        ]
        .into_iter()
        .all(|value| value.is_finite() && (0.0..=1.0).contains(&value))
    }

    #[must_use]
    pub fn clamp(mut self) -> Self {
        self.warmth = clamp01(self.warmth);
        self.trust = clamp01(self.trust);
        self.calm = clamp01(self.calm);
        self.vulnerability = clamp01(self.vulnerability);
        self.longing = clamp01(self.longing);
        self.hurt = clamp01(self.hurt);
        self.tension = clamp01(self.tension);
        self.irritation = clamp01(self.irritation);
        self.affection_intensity = clamp01(self.affection_intensity);
        self.reassurance_need = clamp01(self.reassurance_need);
        self
    }

    #[must_use]
    pub fn scaled(&self, scale: f64) -> Self {
        Self {
            warmth: self.warmth * scale,
            trust: self.trust * scale,
            calm: self.calm * scale,
            vulnerability: self.vulnerability * scale,
            longing: self.longing * scale,
            hurt: self.hurt * scale,
            tension: self.tension * scale,
            irritation: self.irritation * scale,
            affection_intensity: self.affection_intensity * scale,
            reassurance_need: self.reassurance_need * scale,
        }
        .clamp_signed()
    }

    #[must_use]
    pub fn add(&self, other: &Self) -> Self {
        Self {
            warmth: self.warmth + other.warmth,
            trust: self.trust + other.trust,
            calm: self.calm + other.calm,
            vulnerability: self.vulnerability + other.vulnerability,
            longing: self.longing + other.longing,
            hurt: self.hurt + other.hurt,
            tension: self.tension + other.tension,
            irritation: self.irritation + other.irritation,
            affection_intensity: self.affection_intensity + other.affection_intensity,
            reassurance_need: self.reassurance_need + other.reassurance_need,
        }
        .clamp()
    }

    #[must_use]
    pub fn lerp(&self, target: &Self, weight: f64) -> Self {
        let w = clamp01(weight);
        Self {
            warmth: self.warmth * (1.0 - w) + target.warmth * w,
            trust: self.trust * (1.0 - w) + target.trust * w,
            calm: self.calm * (1.0 - w) + target.calm * w,
            vulnerability: self.vulnerability * (1.0 - w) + target.vulnerability * w,
            longing: self.longing * (1.0 - w) + target.longing * w,
            hurt: self.hurt * (1.0 - w) + target.hurt * w,
            tension: self.tension * (1.0 - w) + target.tension * w,
            irritation: self.irritation * (1.0 - w) + target.irritation * w,
            affection_intensity: self.affection_intensity * (1.0 - w)
                + target.affection_intensity * w,
            reassurance_need: self.reassurance_need * (1.0 - w) + target.reassurance_need * w,
        }
        .clamp_signed()
    }

    #[must_use]
    pub fn subtract_positive(&self, other: &Self) -> Self {
        Self {
            warmth: (self.warmth - other.warmth).max(0.0),
            trust: (self.trust - other.trust).max(0.0),
            calm: (self.calm - other.calm).max(0.0),
            vulnerability: (self.vulnerability - other.vulnerability).max(0.0),
            longing: (self.longing - other.longing).max(0.0),
            hurt: (self.hurt - other.hurt).max(0.0),
            tension: (self.tension - other.tension).max(0.0),
            irritation: (self.irritation - other.irritation).max(0.0),
            affection_intensity: (self.affection_intensity - other.affection_intensity).max(0.0),
            reassurance_need: (self.reassurance_need - other.reassurance_need).max(0.0),
        }
        .clamp()
    }

    #[must_use]
    pub fn clamp_signed(mut self) -> Self {
        self.warmth = clamp_signed(self.warmth);
        self.trust = clamp_signed(self.trust);
        self.calm = clamp_signed(self.calm);
        self.vulnerability = clamp_signed(self.vulnerability);
        self.longing = clamp_signed(self.longing);
        self.hurt = clamp_signed(self.hurt);
        self.tension = clamp_signed(self.tension);
        self.irritation = clamp_signed(self.irritation);
        self.affection_intensity = clamp_signed(self.affection_intensity);
        self.reassurance_need = clamp_signed(self.reassurance_need);
        self
    }

    #[must_use]
    pub fn decay_toward(&self, baseline: &Self, elapsed_minutes: f64, recovery_speed: f64) -> Self {
        let decay_strength = (elapsed_minutes / DECAY_MINUTES) * (0.35 + recovery_speed * 0.85);
        let factor = (-decay_strength).exp();
        Self {
            warmth: baseline.warmth + (self.warmth - baseline.warmth) * factor,
            trust: baseline.trust + (self.trust - baseline.trust) * factor,
            calm: baseline.calm + (self.calm - baseline.calm) * factor,
            vulnerability: baseline.vulnerability
                + (self.vulnerability - baseline.vulnerability) * factor,
            longing: baseline.longing + (self.longing - baseline.longing) * factor,
            hurt: baseline.hurt + (self.hurt - baseline.hurt) * factor,
            tension: baseline.tension + (self.tension - baseline.tension) * factor,
            irritation: baseline.irritation + (self.irritation - baseline.irritation) * factor,
            affection_intensity: baseline.affection_intensity
                + (self.affection_intensity - baseline.affection_intensity) * factor,
            reassurance_need: baseline.reassurance_need
                + (self.reassurance_need - baseline.reassurance_need) * factor,
        }
        .clamp()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct RegulationStyle {
    pub suppression: f64,
    pub volatility: f64,
    pub recovery_speed: f64,
    pub conflict_avoidance: f64,
    pub reassurance_seeking: f64,
    pub protest_behavior: f64,
    pub emotional_transparency: f64,
    pub attachment_activation: f64,
    pub pride: f64,
}

impl Default for RegulationStyle {
    fn default() -> Self {
        Self {
            suppression: 0.35,
            volatility: 0.25,
            recovery_speed: 0.55,
            conflict_avoidance: 0.45,
            reassurance_seeking: 0.4,
            protest_behavior: 0.2,
            emotional_transparency: 0.55,
            attachment_activation: 0.45,
            pride: 0.3,
        }
    }
}

impl RegulationStyle {
    #[must_use]
    pub fn is_unit(&self) -> bool {
        [
            self.suppression,
            self.volatility,
            self.recovery_speed,
            self.conflict_avoidance,
            self.reassurance_seeking,
            self.protest_behavior,
            self.emotional_transparency,
            self.attachment_activation,
            self.pride,
        ]
        .into_iter()
        .all(|value| value.is_finite() && (0.0..=1.0).contains(&value))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct RelationshipDefaults {
    pub closeness: f64,
    pub trust: f64,
    pub affection: f64,
    pub tension: f64,
}

impl Default for RelationshipDefaults {
    fn default() -> Self {
        Self {
            closeness: 0.1,
            trust: 0.1,
            affection: 0.05,
            tension: 0.0,
        }
    }
}

impl RelationshipDefaults {
    #[must_use]
    pub fn is_valid(&self) -> bool {
        [self.closeness, self.trust, self.affection]
            .into_iter()
            .all(|value| value.is_finite() && (-1.0..=1.0).contains(&value))
            && self.tension.is_finite()
            && (0.0..=1.0).contains(&self.tension)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EmotionalState {
    pub felt: EmotionVector,
    pub expressed: EmotionVector,
    pub blocked: EmotionVector,
    pub momentum: EmotionVector,
    pub active_drivers: Vec<String>,
    pub confidence: f64,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RelationshipState {
    pub closeness: f64,
    pub trust: f64,
    pub affection: f64,
    pub tension: f64,
    pub stability: f64,
    pub interaction_count: u32,
    pub last_interaction_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompanionRuntimeState {
    pub emotional_state: EmotionalState,
    pub relationship_state: RelationshipState,
    pub active_signals: Vec<String>,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RelationshipDelta {
    pub closeness: f64,
    pub trust: f64,
    pub affection: f64,
    pub tension: f64,
    pub stability: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompanionTurnInput {
    pub signals: Vec<String>,
    pub emotion_delta: EmotionVector,
    pub relationship_delta: RelationshipDelta,
    pub confidence: f64,
    pub now: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompanionTurnTransition {
    pub previous: CompanionRuntimeState,
    pub effect_baseline: CompanionRuntimeState,
    pub current: CompanionRuntimeState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompanionStateOwner {
    pub conversation_id: ConversationId,
    pub character_id: CharacterId,
    pub persona_id: Option<PersonaId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompanionStateSnapshot {
    pub owner: CompanionStateOwner,
    pub session_revision: Revision,
    pub relationship_revision: Revision,
    pub state: CompanionRuntimeState,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompanionStateReplacement {
    pub expected_session_revision: Revision,
    pub expected_relationship_revision: Revision,
    pub state: CompanionRuntimeState,
    pub applied_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanionStateApplyReceipt {
    pub operation_id: OperationRecordId,
    pub owner: CompanionStateOwner,
    pub expected_session_revision: Revision,
    pub resulting_session_revision: Revision,
    pub expected_relationship_revision: Revision,
    pub resulting_relationship_revision: Revision,
    pub applied_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompanionStateRepositoryError {
    NotFound,
    AlreadyExists,
    Conflict,
    Invalid,
    OperationMismatch,
    Corrupt,
    Failure,
}

pub trait CompanionStateRepository: Send + Sync {
    fn create(
        &self,
        owner: CompanionStateOwner,
        initial: CompanionRuntimeState,
        now: TimestampMillis,
    ) -> Result<CompanionStateSnapshot, CompanionStateRepositoryError>;

    fn get(
        &self,
        owner: CompanionStateOwner,
    ) -> Result<Option<CompanionStateSnapshot>, CompanionStateRepositoryError>;

    fn replace(
        &self,
        owner: CompanionStateOwner,
        operation_id: OperationRecordId,
        replacement: CompanionStateReplacement,
    ) -> Result<CompanionStateApplyReceipt, CompanionStateRepositoryError>;
}

#[derive(Debug)]
pub struct PreparedCompanionLaunch {
    conversation: PreparedConversationLaunch,
    owner: CompanionStateOwner,
    initial: CompanionRuntimeState,
}

impl PreparedCompanionLaunch {
    pub fn new(
        conversation: PreparedConversationLaunch,
        owner: CompanionStateOwner,
        initial: CompanionRuntimeState,
    ) -> Result<Self, CompanionLaunchRepositoryError> {
        let plan = conversation.plan();
        let character_matches = matches!(&plan.kind, ConversationKind::Direct(details)
            if details.character.source_id == owner.character_id)
            && plan.conversation_id == owner.conversation_id;
        if !character_matches {
            return Err(CompanionLaunchRepositoryError::Invalid);
        }
        validate_runtime_state(&initial).map_err(|_| CompanionLaunchRepositoryError::Invalid)?;
        Ok(Self {
            conversation,
            owner,
            initial,
        })
    }

    #[must_use]
    pub fn conversation(&self) -> &PreparedConversationLaunch {
        &self.conversation
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        PreparedConversationLaunch,
        CompanionStateOwner,
        CompanionRuntimeState,
    ) {
        (self.conversation, self.owner, self.initial)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompanionLaunchRepositoryError {
    Conversation(ConversationRepositoryError),
    State(CompanionStateRepositoryError),
    Invalid,
}

pub trait CompanionConversationCreator: Send + Sync {
    fn create_companion_conversation(
        &self,
        launch: PreparedCompanionLaunch,
        now: TimestampMillis,
    ) -> Result<CreateConversationResult, CompanionLaunchRepositoryError>;
}

#[derive(Debug)]
pub struct PreparedCompanionSend {
    command: SendConversation,
    owner: CompanionStateOwner,
    replacement: CompanionStateReplacement,
}

impl PreparedCompanionSend {
    pub fn new(
        command: SendConversation,
        owner: CompanionStateOwner,
        replacement: CompanionStateReplacement,
    ) -> Result<Self, CompanionSendRepositoryError> {
        command
            .validate()
            .map_err(|_| CompanionSendRepositoryError::Invalid)?;
        if command.conversation_id != owner.conversation_id
            || validate_runtime_state(&replacement.state).is_err()
        {
            return Err(CompanionSendRepositoryError::Invalid);
        }
        Ok(Self {
            command,
            owner,
            replacement,
        })
    }

    #[must_use]
    pub fn command(&self) -> &SendConversation {
        &self.command
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        SendConversation,
        CompanionStateOwner,
        CompanionStateReplacement,
    ) {
        (self.command, self.owner, self.replacement)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompanionSendRepositoryError {
    Conversation(ConversationRepositoryError),
    Invalid,
}

pub trait CompanionConversationSender: Send + Sync {
    fn begin_companion_send(
        &self,
        prepared: PreparedCompanionSend,
        now: TimestampMillis,
    ) -> Result<SendConversationResult, CompanionSendRepositoryError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationshipAxis {
    Closeness,
    Trust,
    Affection,
}

#[derive(Debug, Clone, Copy)]
struct AxisDynamics {
    neg_mult: f64,
    recovery_rate: f64,
}

const TRUST_DYN: AxisDynamics = AxisDynamics {
    neg_mult: 1.6,
    recovery_rate: 0.012,
};
const AFFECTION_DYN: AxisDynamics = AxisDynamics {
    neg_mult: 1.4,
    recovery_rate: 0.018,
};
const CLOSENESS_DYN: AxisDynamics = AxisDynamics {
    neg_mult: 1.3,
    recovery_rate: 0.02,
};

#[must_use]
pub fn regulate_expressed(felt: &EmotionVector, regulation: &RegulationStyle) -> EmotionVector {
    EmotionVector {
        warmth: felt.warmth * (0.6 + regulation.attachment_activation * 0.35),
        trust: felt.trust * 0.85,
        calm: felt.calm * (0.55 + regulation.recovery_speed * 0.4),
        vulnerability: felt.vulnerability
            * (1.0 - regulation.suppression)
            * regulation.emotional_transparency,
        longing: felt.longing
            * (0.35 + regulation.attachment_activation * 0.65)
            * (1.0 - regulation.suppression * 0.25),
        hurt: felt.hurt * (1.0 - regulation.suppression) * regulation.emotional_transparency,
        tension: felt.tension * (1.0 - regulation.recovery_speed * 0.15),
        irritation: felt.irritation * (1.0 - regulation.conflict_avoidance * 0.35),
        affection_intensity: felt.affection_intensity
            * (0.65 + regulation.emotional_transparency * 0.3),
        reassurance_need: felt.reassurance_need
            * regulation.reassurance_seeking
            * (1.0 - regulation.pride * 0.4),
    }
    .clamp()
}

#[must_use]
pub fn apply_bipolar_delta(
    axis: RelationshipAxis,
    current: f64,
    raw_delta: f64,
    baseline: f64,
) -> f64 {
    let cfg = match axis {
        RelationshipAxis::Closeness => CLOSENESS_DYN,
        RelationshipAxis::Trust => TRUST_DYN,
        RelationshipAxis::Affection => AFFECTION_DYN,
    };
    let d = if raw_delta < 0.0 {
        raw_delta * cfg.neg_mult
    } else {
        raw_delta
    };
    let headroom = if d >= 0.0 {
        1.0 - current
    } else {
        1.0 + current
    };
    let v = current + d * headroom.max(0.0);
    let gap = v - baseline;
    let out = if gap < 0.0 {
        v - cfg.recovery_rate * gap
    } else {
        v
    };
    clamp_signed(out)
}

#[must_use]
pub fn initial_runtime_state(
    baseline: &EmotionVector,
    regulation: &RegulationStyle,
    relationship: &RelationshipDefaults,
) -> CompanionRuntimeState {
    CompanionRuntimeState {
        emotional_state: EmotionalState {
            felt: baseline.clone(),
            expressed: regulate_expressed(baseline, regulation),
            blocked: EmotionVector::default(),
            momentum: EmotionVector::default(),
            active_drivers: Vec::new(),
            confidence: 0.5,
            updated_at: TimestampMillis::UNIX_EPOCH,
        },
        relationship_state: RelationshipState {
            closeness: relationship.closeness,
            trust: relationship.trust,
            affection: relationship.affection,
            tension: relationship.tension,
            stability: 0.5,
            interaction_count: 0,
            last_interaction_at: TimestampMillis::UNIX_EPOCH,
        },
        active_signals: Vec::new(),
        updated_at: TimestampMillis::UNIX_EPOCH,
    }
}

#[must_use]
pub fn elapsed_minutes(previous: TimestampMillis, now: TimestampMillis) -> f64 {
    if previous == TimestampMillis::UNIX_EPOCH || now <= previous {
        0.0
    } else {
        (now.get() - previous.get()) as f64 / 60_000.0
    }
}

#[must_use]
pub fn apply_passive_decay(
    state: &CompanionRuntimeState,
    baseline: &EmotionVector,
    regulation: &RegulationStyle,
    now: TimestampMillis,
) -> CompanionRuntimeState {
    let mut state = state.clone();
    let elapsed_minutes = elapsed_minutes(state.updated_at, now);
    state.emotional_state.felt = state.emotional_state.felt.decay_toward(
        baseline,
        elapsed_minutes,
        regulation.recovery_speed,
    );
    state.emotional_state.expressed = state.emotional_state.expressed.decay_toward(
        baseline,
        elapsed_minutes,
        regulation.recovery_speed,
    );
    state.emotional_state.blocked = state.emotional_state.blocked.decay_toward(
        &EmotionVector::default(),
        elapsed_minutes,
        regulation.recovery_speed,
    );
    state.relationship_state.tension = clamp01(
        state.relationship_state.tension * (-elapsed_minutes / (DECAY_MINUTES * 2.0)).exp(),
    );
    state.relationship_state.stability = clamp01(
        state.relationship_state.stability
            + ((0.55 - state.relationship_state.tension) * 0.02)
            + (elapsed_minutes / 180.0).min(0.05),
    );
    state
}

#[must_use]
pub fn apply_turn(
    state: &CompanionRuntimeState,
    baseline: &EmotionVector,
    regulation: &RegulationStyle,
    relationship_defaults: &RelationshipDefaults,
    input: &CompanionTurnInput,
) -> CompanionTurnTransition {
    let previous = state.clone();
    let mut state = apply_passive_decay(state, baseline, regulation, input.now);
    let mut effect_baseline = state.clone();
    effect_baseline.emotional_state.expressed =
        regulate_expressed(&effect_baseline.emotional_state.felt, regulation);
    effect_baseline.emotional_state.blocked = effect_baseline
        .emotional_state
        .felt
        .subtract_positive(&effect_baseline.emotional_state.expressed);
    effect_baseline.relationship_state.closeness = apply_bipolar_delta(
        RelationshipAxis::Closeness,
        effect_baseline.relationship_state.closeness,
        0.0,
        relationship_defaults.closeness,
    );
    effect_baseline.relationship_state.trust = apply_bipolar_delta(
        RelationshipAxis::Trust,
        effect_baseline.relationship_state.trust,
        0.0,
        relationship_defaults.trust,
    );
    effect_baseline.relationship_state.affection = apply_bipolar_delta(
        RelationshipAxis::Affection,
        effect_baseline.relationship_state.affection,
        0.0,
        relationship_defaults.affection,
    );

    let volatility = 0.75 + regulation.volatility * 0.9;
    let delta = input.emotion_delta.scaled(volatility);
    let felt = state.emotional_state.felt.add(&delta).clamp();
    let expressed = regulate_expressed(&felt, regulation);
    let blocked = felt.subtract_positive(&expressed);

    state.emotional_state.momentum = state.emotional_state.momentum.lerp(&delta, 0.45);
    state.emotional_state.felt = felt;
    state.emotional_state.expressed = expressed;
    state.emotional_state.blocked = blocked;
    state.emotional_state.active_drivers = input.signals.clone();
    state.emotional_state.confidence = input.confidence;
    state.emotional_state.updated_at = input.now;

    state.relationship_state.closeness = apply_bipolar_delta(
        RelationshipAxis::Closeness,
        state.relationship_state.closeness,
        input.relationship_delta.closeness,
        relationship_defaults.closeness,
    );
    state.relationship_state.trust = apply_bipolar_delta(
        RelationshipAxis::Trust,
        state.relationship_state.trust,
        input.relationship_delta.trust,
        relationship_defaults.trust,
    );
    state.relationship_state.affection = apply_bipolar_delta(
        RelationshipAxis::Affection,
        state.relationship_state.affection,
        input.relationship_delta.affection,
        relationship_defaults.affection,
    );
    state.relationship_state.tension =
        clamp01(state.relationship_state.tension + input.relationship_delta.tension);
    state.relationship_state.stability =
        clamp01(state.relationship_state.stability + input.relationship_delta.stability);
    state.relationship_state.interaction_count += 1;
    state.relationship_state.last_interaction_at = input.now;
    state.active_signals = input.signals.clone();
    state.updated_at = input.now;

    CompanionTurnTransition {
        previous,
        effect_baseline,
        current: state,
    }
}

pub fn validate_runtime_state(
    state: &CompanionRuntimeState,
) -> Result<(), CompanionStateRepositoryError> {
    let emotional = &state.emotional_state;
    let relationship = &state.relationship_state;
    if !emotional.felt.is_unit()
        || !emotional.expressed.is_unit()
        || !emotional.blocked.is_unit()
        || !is_signed_vector(&emotional.momentum)
        || !unit(emotional.confidence)
        || !signed(relationship.closeness)
        || !signed(relationship.trust)
        || !signed(relationship.affection)
        || !unit(relationship.tension)
        || !unit(relationship.stability)
        || emotional.updated_at > state.updated_at
        || emotional
            .active_drivers
            .iter()
            .any(|value| value.trim().is_empty())
        || state
            .active_signals
            .iter()
            .any(|value| value.trim().is_empty())
    {
        return Err(CompanionStateRepositoryError::Invalid);
    }
    Ok(())
}

fn is_signed_vector(value: &EmotionVector) -> bool {
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
    .into_iter()
    .all(signed)
}

fn unit(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn signed(value: f64) -> bool {
    value.is_finite() && (-1.0..=1.0).contains(&value)
}

fn clamp01(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

fn clamp_signed(value: f64) -> f64 {
    value.clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn baseline() -> EmotionVector {
        EmotionVector {
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
        }
    }

    #[test]
    fn default_initial_state_copies_legacy_values_and_expression_formula() {
        let baseline = baseline();
        let state = initial_runtime_state(
            &baseline,
            &RegulationStyle::default(),
            &RelationshipDefaults::default(),
        );
        assert_eq!(state.emotional_state.felt, baseline);
        assert!((state.emotional_state.expressed.warmth - 0.340875).abs() < 1e-12);
        assert!((state.emotional_state.expressed.vulnerability - 0.0715).abs() < 1e-12);
        assert!((state.emotional_state.expressed.reassurance_need - 0.0528).abs() < 1e-12);
        assert_eq!(state.emotional_state.blocked, EmotionVector::default());
        assert_eq!(state.relationship_state.closeness, 0.1);
        assert_eq!(state.relationship_state.trust, 0.1);
        assert_eq!(state.relationship_state.affection, 0.05);
        assert_eq!(state.relationship_state.stability, 0.5);
    }

    #[test]
    fn vector_operations_copy_legacy_signed_and_unit_clamps() {
        let delta = EmotionVector {
            warmth: 0.8,
            hurt: -0.8,
            ..EmotionVector::default()
        };
        let scaled = delta.scaled(2.0);
        assert_eq!(scaled.warmth, 1.0);
        assert_eq!(scaled.hurt, -1.0);

        let added = baseline().add(&scaled);
        assert_eq!(added.warmth, 1.0);
        assert_eq!(added.hurt, 0.0);

        let lerped = EmotionVector::default().lerp(&scaled, 2.0);
        assert_eq!(lerped, scaled);
    }

    #[test]
    fn regulation_copies_legacy_suppression_and_transparency_math() {
        let felt = EmotionVector {
            vulnerability: 0.8,
            hurt: 0.6,
            reassurance_need: 0.5,
            ..EmotionVector::default()
        };
        let regulation = RegulationStyle {
            suppression: 1.0,
            emotional_transparency: 1.0,
            reassurance_seeking: 1.0,
            pride: 1.0,
            ..RegulationStyle::default()
        };
        let expressed = regulate_expressed(&felt, &regulation);
        assert_eq!(expressed.vulnerability, 0.0);
        assert_eq!(expressed.hurt, 0.0);
        assert!((expressed.reassurance_need - 0.3).abs() < 1e-12);
    }

    #[test]
    fn bipolar_axes_copy_legacy_damage_recovery_and_saturation() {
        let trust_down = apply_bipolar_delta(RelationshipAxis::Trust, 0.3, -0.045, 0.3);
        let trust_up = apply_bipolar_delta(RelationshipAxis::Trust, 0.3, 0.045, 0.3);
        assert!(0.3 - trust_down > trust_up - 0.3);
        let recovered = apply_bipolar_delta(RelationshipAxis::Trust, -0.1, 0.0, 0.3);
        assert!((recovered - (-0.0952)).abs() < 1e-12);
        let low_gain = apply_bipolar_delta(RelationshipAxis::Affection, 0.0, 0.1, 0.15);
        let high_gain = apply_bipolar_delta(RelationshipAxis::Affection, 0.9, 0.1, 0.15) - 0.9;
        assert!(high_gain < low_gain);
    }

    #[test]
    fn passive_decay_copies_legacy_exponential_and_stability_math() {
        let baseline = baseline();
        let mut state = initial_runtime_state(
            &baseline,
            &RegulationStyle::default(),
            &RelationshipDefaults::default(),
        );
        state.updated_at = TimestampMillis::new(60_000);
        state.emotional_state.felt.warmth = 1.0;
        state.relationship_state.tension = 0.8;
        let decayed = apply_passive_decay(
            &state,
            &baseline,
            &RegulationStyle::default(),
            TimestampMillis::new(2_760_000),
        );
        let factor = (-(45.0_f64 / 45.0) * (0.35 + 0.55 * 0.85)).exp();
        assert!((decayed.emotional_state.felt.warmth - (0.45 + 0.55 * factor)).abs() < 1e-12);
        assert!((decayed.relationship_state.tension - 0.8 * (-0.5_f64).exp()).abs() < 1e-12);
    }

    #[test]
    fn pure_turn_transition_copies_legacy_volatility_momentum_and_axes() {
        let baseline = baseline();
        let regulation = RegulationStyle::default();
        let relationship = RelationshipDefaults::default();
        let state = initial_runtime_state(&baseline, &regulation, &relationship);
        let transition = apply_turn(
            &state,
            &baseline,
            &regulation,
            &relationship,
            &CompanionTurnInput {
                signals: vec!["emotion:love".into()],
                emotion_delta: EmotionVector {
                    warmth: 0.1,
                    trust: 0.04,
                    affection_intensity: 0.15,
                    ..EmotionVector::default()
                },
                relationship_delta: RelationshipDelta {
                    closeness: 0.035,
                    affection: 0.055,
                    ..RelationshipDelta::default()
                },
                confidence: 0.82,
                now: TimestampMillis::new(1_000),
            },
        );
        let volatility = 0.75 + 0.25 * 0.9;
        assert!(
            (transition.current.emotional_state.felt.warmth - (0.45 + 0.1 * volatility)).abs()
                < 1e-12
        );
        assert!(
            (transition.current.emotional_state.momentum.warmth - (0.1 * volatility * 0.45)).abs()
                < 1e-12
        );
        assert_eq!(transition.current.emotional_state.confidence, 0.82);
        assert_eq!(transition.current.relationship_state.interaction_count, 1);
        assert_eq!(transition.current.active_signals, ["emotion:love"]);
        assert_eq!(
            transition.effect_baseline.updated_at,
            TimestampMillis::UNIX_EPOCH
        );
    }

    #[test]
    fn runtime_validation_distinguishes_signed_momentum_from_unit_state() {
        let mut state = initial_runtime_state(
            &baseline(),
            &RegulationStyle::default(),
            &RelationshipDefaults::default(),
        );
        state.emotional_state.momentum.hurt = -0.4;
        assert_eq!(validate_runtime_state(&state), Ok(()));
        state.emotional_state.felt.hurt = -0.1;
        assert_eq!(
            validate_runtime_state(&state),
            Err(CompanionStateRepositoryError::Invalid)
        );
    }
}
