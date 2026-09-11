use lettuce_types::TimestampMillis;

use crate::{CompanionRuntimeState, CompanionSoulIdentity, EmotionVector, SoulCategory, SoulState};

#[derive(Debug)]
pub struct CompanionPromptStateInput<'a> {
    pub soul: &'a CompanionSoulIdentity,
    pub soul_state: &'a SoulState,
    pub runtime_state: &'a CompanionRuntimeState,
    pub style_notes: &'a str,
    pub continuity_episode: u32,
    pub effective_at: TimestampMillis,
}

/// The legacy five-step band shared by closeness, trust and affection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationshipBand {
    Lowest,
    Low,
    Neutral,
    High,
    Highest,
}

impl RelationshipBand {
    #[must_use]
    pub fn of(value: f64) -> Self {
        if value < -0.5 {
            Self::Lowest
        } else if value < -0.15 {
            Self::Low
        } else if value <= 0.15 {
            Self::Neutral
        } else if value <= 0.5 {
            Self::High
        } else {
            Self::Highest
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmotionDimension {
    Warmth,
    Trust,
    Calm,
    Vulnerability,
    Longing,
    Hurt,
    Tension,
    Irritation,
    Affection,
    ReassuranceNeed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EmotionReading {
    pub dimension: EmotionDimension,
    pub value: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegulationCue {
    Suppressed,
    Transparent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReassuranceCue {
    Open,
    Avoidant,
}

/// Soul lines in the order the prompt lists them.
pub const SOUL_PROMPT_ORDER: [SoulCategory; 12] = [
    SoulCategory::Essence,
    SoulCategory::Traits,
    SoulCategory::Backstory,
    SoulCategory::Appearance,
    SoulCategory::Goals,
    SoulCategory::Likes,
    SoulCategory::Voice,
    SoulCategory::RelationalStyle,
    SoulCategory::Vulnerabilities,
    SoulCategory::Fears,
    SoulCategory::Habits,
    SoulCategory::Boundaries,
];

/// What the companion state block says, decided with the legacy thresholds;
/// the application renders each part from the prompt catalog.
#[derive(Debug, Clone, PartialEq)]
pub struct CompanionPromptState {
    pub interaction_count: u32,
    pub closeness: RelationshipBand,
    pub trust: RelationshipBand,
    pub affection: RelationshipBand,
    pub tension: f64,
    pub expressed: Vec<EmotionReading>,
    pub continuity_episode: Option<u32>,
    pub soul: Vec<(SoulCategory, String)>,
    pub style_notes: Option<String>,
    pub blocked: Vec<EmotionReading>,
    pub active_signals: Vec<String>,
    pub regulation: Option<RegulationCue>,
    pub reassurance: Option<ReassuranceCue>,
}

#[must_use]
pub fn prompt_state(input: &CompanionPromptStateInput<'_>) -> CompanionPromptState {
    let state = input.runtime_state;
    let regulation = &input.soul.regulation_style;
    let rel = &state.relationship_state;
    CompanionPromptState {
        interaction_count: rel.interaction_count,
        closeness: RelationshipBand::of(rel.closeness),
        trust: RelationshipBand::of(rel.trust),
        affection: RelationshipBand::of(rel.affection),
        tension: rel.tension,
        expressed: top_dimensions(&state.emotional_state.expressed, 3),
        continuity_episode: (input.continuity_episode > 0).then_some(input.continuity_episode),
        soul: SOUL_PROMPT_ORDER
            .into_iter()
            .filter_map(|category| {
                let value = effective_soul_value(
                    soul_base(input.soul, category),
                    category,
                    input.soul_state,
                    input.effective_at,
                );
                let trimmed = value.trim();
                (!trimmed.is_empty()).then(|| (category, trimmed.to_owned()))
            })
            .collect(),
        style_notes: Some(input.style_notes.trim())
            .filter(|value| !value.is_empty())
            .map(str::to_owned),
        blocked: top_dimensions(&state.emotional_state.blocked, 2),
        active_signals: state.active_signals.clone(),
        regulation: if regulation.suppression >= 0.6 {
            Some(RegulationCue::Suppressed)
        } else if regulation.emotional_transparency >= 0.65 {
            Some(RegulationCue::Transparent)
        } else {
            None
        },
        reassurance: if regulation.reassurance_seeking >= 0.6 && regulation.pride < 0.45 {
            Some(ReassuranceCue::Open)
        } else if regulation.pride >= 0.55 {
            Some(ReassuranceCue::Avoidant)
        } else {
            None
        },
    }
}

fn soul_base(soul: &CompanionSoulIdentity, category: SoulCategory) -> &str {
    match category {
        SoulCategory::Essence => &soul.essence,
        SoulCategory::Traits => &soul.traits,
        SoulCategory::Backstory => &soul.backstory,
        SoulCategory::Appearance => &soul.appearance,
        SoulCategory::Goals => &soul.goals,
        SoulCategory::Likes => &soul.likes,
        SoulCategory::Voice => &soul.voice,
        SoulCategory::RelationalStyle => &soul.relational_style,
        SoulCategory::Vulnerabilities => &soul.vulnerabilities,
        SoulCategory::Fears => &soul.fears,
        SoulCategory::Habits => &soul.habits,
        SoulCategory::Boundaries => &soul.boundaries,
    }
}

pub fn effective_soul_value(
    base: &str,
    category: SoulCategory,
    state: &SoulState,
    effective_at: TimestampMillis,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    let trimmed = base.trim();
    if !trimmed.is_empty() {
        parts.push(trimmed.to_string());
    }
    let mut entries = state
        .facts
        .iter()
        .filter(|entry| entry.category == category && entry.is_effective_at(effective_at))
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        let left_score = left.weight.clamp(0.0, 1.0) * left.confidence.clamp(0.0, 1.0);
        let right_score = right.weight.clamp(0.0, 1.0) * right.confidence.clamp(0.0, 1.0);
        right_score.total_cmp(&left_score)
    });
    for entry in entries {
        let value = entry.value.trim();
        if !value.is_empty() {
            parts.push(value.to_string());
        }
    }
    parts.join(" ")
}

fn top_dimensions(vector: &EmotionVector, count: usize) -> Vec<EmotionReading> {
    let mut items = vec![
        (EmotionDimension::Warmth, vector.warmth),
        (EmotionDimension::Trust, vector.trust),
        (EmotionDimension::Calm, vector.calm),
        (EmotionDimension::Vulnerability, vector.vulnerability),
        (EmotionDimension::Longing, vector.longing),
        (EmotionDimension::Hurt, vector.hurt),
        (EmotionDimension::Tension, vector.tension),
        (EmotionDimension::Irritation, vector.irritation),
        (EmotionDimension::Affection, vector.affection_intensity),
        (EmotionDimension::ReassuranceNeed, vector.reassurance_need),
    ];
    items.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    items
        .into_iter()
        .filter(|(_, value)| *value >= 0.08)
        .take(count)
        .map(|(dimension, value)| EmotionReading { dimension, value })
        .collect()
}

#[cfg(test)]
mod tests {
    use lettuce_types::Revision;

    use super::*;
    use crate::{
        CompanionSoulIdentity, RegulationStyle, RelationshipDefaults, SoulFact, SoulFactKind,
        SoulFactPolicy, initial_runtime_state,
    };

    fn state() -> CompanionRuntimeState {
        let soul = CompanionSoulIdentity::default();
        initial_runtime_state(
            &soul.baseline_affect,
            &soul.regulation_style,
            &RelationshipDefaults::default(),
        )
    }

    fn soul_state(facts: Vec<SoulFact>) -> SoulState {
        SoulState {
            revision: Revision::INITIAL,
            facts,
        }
    }

    fn fact(id: &str, value: &str, weight: f64, confidence: f64) -> SoulFact {
        SoulFact {
            id: id.into(),
            category: SoulCategory::Likes,
            value: value.into(),
            kind: SoulFactKind::Add,
            policy: SoulFactPolicy::Adaptive,
            slot: id.into(),
            confidence,
            evidence_count: 1,
            weight,
            valid_from: TimestampMillis::new(10),
            valid_until: None,
            locked: false,
            source_memory_ids: Vec::new(),
            created_at: TimestampMillis::new(10),
            supersedes: Vec::new(),
            superseded_by: None,
            superseded_at: None,
        }
    }

    fn reading(dimension: EmotionDimension, value: f64) -> EmotionReading {
        EmotionReading { dimension, value }
    }

    #[test]
    fn default_state_matches_the_legacy_defaults() {
        let soul = CompanionSoulIdentity::default();
        let soul_state = soul_state(Vec::new());
        let mut state = prompt_state(&CompanionPromptStateInput {
            soul: &soul,
            soul_state: &soul_state,
            runtime_state: &state(),
            style_notes: "  ",
            continuity_episode: 0,
            effective_at: TimestampMillis::new(20),
        });
        assert_eq!(
            std::mem::take(&mut state.expressed)
                .into_iter()
                .map(|reading| (reading.dimension, format!("{:.0}", reading.value * 100.0)))
                .collect::<Vec<_>>(),
            vec![
                (EmotionDimension::Calm, "50".to_owned()),
                (EmotionDimension::Warmth, "34".to_owned()),
                (EmotionDimension::Trust, "30".to_owned()),
            ]
        );
        assert_eq!(
            state,
            CompanionPromptState {
                interaction_count: 0,
                closeness: RelationshipBand::Neutral,
                trust: RelationshipBand::Neutral,
                affection: RelationshipBand::Neutral,
                tension: 0.0,
                expressed: Vec::new(),
                continuity_episode: None,
                soul: Vec::new(),
                style_notes: None,
                blocked: Vec::new(),
                active_signals: Vec::new(),
                regulation: None,
                reassurance: None,
            }
        );
    }

    #[test]
    fn effective_soul_facts_copy_legacy_score_order_and_validity() {
        let soul = CompanionSoulIdentity {
            likes: " Tea ".into(),
            boundaries: "No lies".into(),
            essence: "Curious".into(),
            ..CompanionSoulIdentity::default()
        };
        let mut future = fact("future", "Future", 1.0, 1.0);
        future.valid_from = TimestampMillis::new(30);
        let mut ended = fact("ended", "Ended", 1.0, 1.0);
        ended.valid_until = Some(TimestampMillis::new(20));
        let soul_state = soul_state(vec![
            fact("lower", "Rain", 0.5, 0.5),
            fact("first-equal", "Harbors", 0.5, 1.0),
            fact("second-equal", "Letters", 1.0, 0.5),
            future,
            ended,
        ]);
        let state = prompt_state(&CompanionPromptStateInput {
            soul: &soul,
            soul_state: &soul_state,
            runtime_state: &state(),
            style_notes: " restrained ",
            continuity_episode: 2,
            effective_at: TimestampMillis::new(20),
        });
        assert_eq!(
            state.soul,
            vec![
                (SoulCategory::Essence, "Curious".to_owned()),
                (SoulCategory::Likes, "Tea Harbors Letters Rain".to_owned()),
                (SoulCategory::Boundaries, "No lies".to_owned()),
            ]
        );
        assert_eq!(state.style_notes.as_deref(), Some("restrained"));
        assert_eq!(state.continuity_episode, Some(2));
    }

    #[test]
    fn blocked_signals_and_regulation_copy_legacy_thresholds() {
        let soul = CompanionSoulIdentity {
            regulation_style: RegulationStyle {
                suppression: 0.6,
                reassurance_seeking: 0.6,
                pride: 0.44,
                ..RegulationStyle::default()
            },
            ..CompanionSoulIdentity::default()
        };
        let mut runtime = state();
        runtime.relationship_state.closeness = -0.51;
        runtime.relationship_state.trust = -0.5;
        runtime.relationship_state.affection = 0.51;
        runtime.relationship_state.interaction_count = 1;
        runtime.emotional_state.blocked.hurt = 0.08;
        runtime.active_signals = vec!["emotion:conflict".into()];
        let soul_state = soul_state(Vec::new());
        let state = prompt_state(&CompanionPromptStateInput {
            soul: &soul,
            soul_state: &soul_state,
            runtime_state: &runtime,
            style_notes: "",
            continuity_episode: 0,
            effective_at: TimestampMillis::new(20),
        });
        assert_eq!(state.closeness, RelationshipBand::Lowest);
        assert_eq!(state.trust, RelationshipBand::Low);
        assert_eq!(state.affection, RelationshipBand::Highest);
        assert_eq!(state.blocked, vec![reading(EmotionDimension::Hurt, 0.08)]);
        assert_eq!(state.active_signals, vec!["emotion:conflict".to_owned()]);
        assert_eq!(state.regulation, Some(RegulationCue::Suppressed));
        assert_eq!(state.reassurance, Some(ReassuranceCue::Open));

        let soul = CompanionSoulIdentity {
            regulation_style: RegulationStyle {
                suppression: 0.59,
                emotional_transparency: 0.65,
                reassurance_seeking: 0.59,
                pride: 0.55,
                ..RegulationStyle::default()
            },
            ..CompanionSoulIdentity::default()
        };
        let state = prompt_state(&CompanionPromptStateInput {
            soul: &soul,
            soul_state: &soul_state,
            runtime_state: &runtime,
            style_notes: "",
            continuity_episode: 0,
            effective_at: TimestampMillis::new(20),
        });
        assert_eq!(state.regulation, Some(RegulationCue::Transparent));
        assert_eq!(state.reassurance, Some(ReassuranceCue::Avoidant));
    }

    #[test]
    fn every_relationship_band_boundary_is_exact() {
        for (value, band) in [
            (-0.51, RelationshipBand::Lowest),
            (-0.5, RelationshipBand::Low),
            (-0.15, RelationshipBand::Neutral),
            (0.15, RelationshipBand::Neutral),
            (0.5, RelationshipBand::High),
            (0.51, RelationshipBand::Highest),
            (f64::NAN, RelationshipBand::Highest),
        ] {
            assert_eq!(RelationshipBand::of(value), band, "{value}");
        }
    }

    #[test]
    fn top_dimensions_keep_the_legacy_floor_and_order() {
        let vector = EmotionVector {
            warmth: 0.079,
            trust: 0.08,
            calm: 0.081,
            ..EmotionVector::default()
        };
        assert_eq!(
            top_dimensions(&vector, 3),
            vec![
                reading(EmotionDimension::Calm, 0.081),
                reading(EmotionDimension::Trust, 0.08),
            ]
        );
    }
}
