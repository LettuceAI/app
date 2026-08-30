use lettuce_types::TimestampMillis;

use crate::{CompanionRuntimeState, CompanionSoulIdentity, EmotionVector, SoulCategory, SoulState};

#[derive(Debug)]
pub struct CompanionPromptStateInput<'a> {
    pub character_name: &'a str,
    pub partner_name: Option<&'a str>,
    pub soul: &'a CompanionSoulIdentity,
    pub soul_state: &'a SoulState,
    pub runtime_state: &'a CompanionRuntimeState,
    pub style_notes: &'a str,
    pub continuity_episode: u32,
    pub effective_at: TimestampMillis,
}

#[must_use]
pub fn render_prompt_state(input: &CompanionPromptStateInput<'_>) -> String {
    let state = input.runtime_state;
    let soul = input.soul;
    let regulation = soul.regulation_style.clone();

    let expressed = describe_top_dimensions(&state.emotional_state.expressed, 3);
    let blocked = describe_top_dimensions(&state.emotional_state.blocked, 2);
    let rel = &state.relationship_state;
    let partner_name = input
        .partner_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("the current conversation partner");

    let mut lines = vec![
        format!(
            "The following relationship and emotional state describes {}'s live relationship with {}, the person currently speaking in this chat.",
            input.character_name, partner_name,
        ),
        "Do not apply these metrics to third-party people mentioned in character definitions, persona descriptions, lore, or memories unless that relationship is explicitly stated.".to_string(),
        "Closeness, trust, and affection are bidirectional: they can run negative, meaning the character actively dislikes, distrusts, or wants distance from the partner, not merely feels neutral.".to_string(),
        "Treat these metrics as supporting signals, not as permission to contradict the chat history, memories, or established relationship events. Preserve established emotional breakthroughs as settled continuity; never reset or rediscover them merely because a metric band is lower.".to_string(),
        format!(
            "Relationship duration context: this session state has tracked {} user interaction{}.",
            rel.interaction_count,
            if rel.interaction_count == 1 { "" } else { "s" },
        ),
        format!(
            "Current {} <-> {} relationship stance: closeness {}, trust {}, affection {}; tension {:.0}%.",
            input.character_name,
            partner_name,
            closeness_band(rel.closeness),
            trust_band(rel.trust),
            affection_band(rel.affection),
            rel.tension * 100.0,
        ),
        format!(
            "Expressed tone right now: {}.",
            if expressed.is_empty() {
                "steady and low-intensity"
            } else {
                expressed.as_str()
            }
        ),
    ];

    if input.continuity_episode > 0 {
        lines.push(format!(
            "Continuity: this chat is episode {} of one continuous relationship. Treat earlier shared memories and settled milestones as prior episodes, not as events that need to be rediscovered.",
            input.continuity_episode,
        ));
    }

    push_soul_line(
        &mut lines,
        "Soul essence",
        &effective_soul_value(
            &soul.essence,
            SoulCategory::Essence,
            input.soul_state,
            input.effective_at,
        ),
    );
    push_soul_line(
        &mut lines,
        "Defining traits",
        &effective_soul_value(
            &soul.traits,
            SoulCategory::Traits,
            input.soul_state,
            input.effective_at,
        ),
    );
    push_soul_line(
        &mut lines,
        "Backstory",
        &effective_soul_value(
            &soul.backstory,
            SoulCategory::Backstory,
            input.soul_state,
            input.effective_at,
        ),
    );
    push_soul_line(
        &mut lines,
        "Appearance",
        &effective_soul_value(
            &soul.appearance,
            SoulCategory::Appearance,
            input.soul_state,
            input.effective_at,
        ),
    );
    push_soul_line(
        &mut lines,
        "Goals",
        &effective_soul_value(
            &soul.goals,
            SoulCategory::Goals,
            input.soul_state,
            input.effective_at,
        ),
    );
    push_soul_line(
        &mut lines,
        "Likes and favorites",
        &effective_soul_value(
            &soul.likes,
            SoulCategory::Likes,
            input.soul_state,
            input.effective_at,
        ),
    );
    push_soul_line(
        &mut lines,
        "Companion voice",
        &effective_soul_value(
            &soul.voice,
            SoulCategory::Voice,
            input.soul_state,
            input.effective_at,
        ),
    );
    push_soul_line(
        &mut lines,
        "Relational style",
        &effective_soul_value(
            &soul.relational_style,
            SoulCategory::RelationalStyle,
            input.soul_state,
            input.effective_at,
        ),
    );
    push_soul_line(
        &mut lines,
        "Vulnerabilities",
        &effective_soul_value(
            &soul.vulnerabilities,
            SoulCategory::Vulnerabilities,
            input.soul_state,
            input.effective_at,
        ),
    );
    push_soul_line(
        &mut lines,
        "Fears",
        &effective_soul_value(
            &soul.fears,
            SoulCategory::Fears,
            input.soul_state,
            input.effective_at,
        ),
    );
    push_soul_line(
        &mut lines,
        "Habits",
        &effective_soul_value(
            &soul.habits,
            SoulCategory::Habits,
            input.soul_state,
            input.effective_at,
        ),
    );
    push_soul_line(
        &mut lines,
        "Boundaries",
        &effective_soul_value(
            &soul.boundaries,
            SoulCategory::Boundaries,
            input.soul_state,
            input.effective_at,
        ),
    );
    push_soul_line(&mut lines, "Companion style notes", input.style_notes);

    if !blocked.is_empty() {
        lines.push(format!("More strongly felt than shown: {}.", blocked));
    }

    if !state.active_signals.is_empty() {
        lines.push(format!(
            "Recent drivers in {}'s interaction with {}: {}.",
            input.character_name,
            partner_name,
            state.active_signals.join(", ")
        ));
    }

    if regulation.suppression >= 0.6 {
        lines.push(
            "Regulation: tends to hide direct hurt and avoids blunt emotional disclosure."
                .to_string(),
        );
    } else if regulation.emotional_transparency >= 0.65 {
        lines.push("Regulation: relatively emotionally direct when trust is present.".to_string());
    }

    if regulation.reassurance_seeking >= 0.6 && regulation.pride < 0.45 {
        lines.push("When unsettled, may seek reassurance more openly.".to_string());
    } else if regulation.pride >= 0.55 {
        lines.push("When unsettled, may avoid asking directly for reassurance.".to_string());
    }

    lines.join("\n")
}

fn push_soul_line(lines: &mut Vec<String>, label: &str, value: &str) {
    let trimmed = value.trim();
    if !trimmed.is_empty() {
        lines.push(format!("{}: {}.", label, trimmed));
    }
}

fn effective_soul_value(
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

fn describe_top_dimensions(vector: &EmotionVector, count: usize) -> String {
    let mut items = vec![
        ("warmth", vector.warmth),
        ("trust", vector.trust),
        ("calm", vector.calm),
        ("vulnerability", vector.vulnerability),
        ("longing", vector.longing),
        ("hurt", vector.hurt),
        ("tension", vector.tension),
        ("irritation", vector.irritation),
        ("affection", vector.affection_intensity),
        ("reassurance need", vector.reassurance_need),
    ];
    items.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let described = items
        .into_iter()
        .filter(|(_, value)| *value >= 0.08)
        .take(count)
        .map(|(label, value)| format!("{} ({:.0}%)", label, value * 100.0))
        .collect::<Vec<_>>();

    described.join(", ")
}

fn affection_band(value: f64) -> &'static str {
    if value < -0.5 {
        "hostile"
    } else if value < -0.15 {
        "cold/irritated"
    } else if value <= 0.15 {
        "neutral"
    } else if value <= 0.5 {
        "warm"
    } else {
        "deeply affectionate"
    }
}

fn trust_band(value: f64) -> &'static str {
    if value < -0.5 {
        "distrustful/guarded"
    } else if value < -0.15 {
        "wary"
    } else if value <= 0.15 {
        "neutral"
    } else if value <= 0.5 {
        "trusting"
    } else {
        "deeply trusting"
    }
}

fn closeness_band(value: f64) -> &'static str {
    if value < -0.5 {
        "withdrawing/wants distance"
    } else if value < -0.15 {
        "distant"
    } else if value <= 0.15 {
        "acquainted"
    } else if value <= 0.5 {
        "close"
    } else {
        "intimate"
    }
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

    #[test]
    fn default_prompt_is_byte_exact_and_uses_partner_fallback() {
        let soul = CompanionSoulIdentity::default();
        let soul_state = soul_state(Vec::new());
        let rendered = render_prompt_state(&CompanionPromptStateInput {
            character_name: "Mira",
            partner_name: Some("  "),
            soul: &soul,
            soul_state: &soul_state,
            runtime_state: &state(),
            style_notes: "",
            continuity_episode: 0,
            effective_at: TimestampMillis::new(20),
        });
        assert_eq!(
            rendered,
            "The following relationship and emotional state describes Mira's live relationship with the current conversation partner, the person currently speaking in this chat.\nDo not apply these metrics to third-party people mentioned in character definitions, persona descriptions, lore, or memories unless that relationship is explicitly stated.\nCloseness, trust, and affection are bidirectional: they can run negative, meaning the character actively dislikes, distrusts, or wants distance from the partner, not merely feels neutral.\nTreat these metrics as supporting signals, not as permission to contradict the chat history, memories, or established relationship events. Preserve established emotional breakthroughs as settled continuity; never reset or rediscover them merely because a metric band is lower.\nRelationship duration context: this session state has tracked 0 user interactions.\nCurrent Mira <-> the current conversation partner relationship stance: closeness acquainted, trust neutral, affection neutral; tension 0%.\nExpressed tone right now: calm (50%), warmth (34%), trust (30%)."
        );
    }

    #[test]
    fn effective_soul_facts_copy_legacy_score_order_and_validity() {
        let soul = CompanionSoulIdentity {
            likes: " Tea ".into(),
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
        let rendered = render_prompt_state(&CompanionPromptStateInput {
            character_name: "Mira",
            partner_name: Some("Ari"),
            soul: &soul,
            soul_state: &soul_state,
            runtime_state: &state(),
            style_notes: "",
            continuity_episode: 0,
            effective_at: TimestampMillis::new(20),
        });
        assert!(rendered.contains("Likes and favorites: Tea Harbors Letters Rain."));
        assert!(!rendered.contains("Future"));
        assert!(!rendered.contains("Ended"));
    }

    #[test]
    fn bands_blocked_signals_continuity_and_regulation_copy_boundaries() {
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
        runtime.relationship_state.tension = 0.456;
        runtime.relationship_state.interaction_count = 1;
        runtime.emotional_state.blocked.hurt = 0.08;
        runtime.active_signals = vec!["emotion:conflict".into()];
        let soul_state = soul_state(Vec::new());
        let rendered = render_prompt_state(&CompanionPromptStateInput {
            character_name: "Mira",
            partner_name: Some("Ari"),
            soul: &soul,
            soul_state: &soul_state,
            runtime_state: &runtime,
            style_notes: " restrained ",
            continuity_episode: 2,
            effective_at: TimestampMillis::new(20),
        });
        assert!(rendered.contains("tracked 1 user interaction."));
        assert!(rendered.contains("closeness withdrawing/wants distance, trust wary, affection deeply affectionate; tension 46%."));
        assert!(
            rendered.contains("Continuity: this chat is episode 2 of one continuous relationship.")
        );
        assert!(rendered.contains("Companion style notes: restrained."));
        assert!(rendered.contains("More strongly felt than shown: hurt (8%)."));
        assert!(
            rendered.contains("Recent drivers in Mira's interaction with Ari: emotion:conflict.")
        );
        assert!(rendered.contains(
            "Regulation: tends to hide direct hurt and avoids blunt emotional disclosure."
        ));
        assert!(rendered.contains("When unsettled, may seek reassurance more openly."));
    }

    #[test]
    fn every_relationship_band_boundary_is_exact() {
        assert_eq!(closeness_band(-0.51), "withdrawing/wants distance");
        assert_eq!(closeness_band(-0.5), "distant");
        assert_eq!(closeness_band(-0.15), "acquainted");
        assert_eq!(closeness_band(0.15), "acquainted");
        assert_eq!(closeness_band(0.5), "close");
        assert_eq!(closeness_band(0.51), "intimate");

        assert_eq!(trust_band(-0.51), "distrustful/guarded");
        assert_eq!(trust_band(-0.5), "wary");
        assert_eq!(trust_band(-0.15), "neutral");
        assert_eq!(trust_band(0.15), "neutral");
        assert_eq!(trust_band(0.5), "trusting");
        assert_eq!(trust_band(0.51), "deeply trusting");

        assert_eq!(affection_band(-0.51), "hostile");
        assert_eq!(affection_band(-0.5), "cold/irritated");
        assert_eq!(affection_band(-0.15), "neutral");
        assert_eq!(affection_band(0.15), "neutral");
        assert_eq!(affection_band(0.5), "warm");
        assert_eq!(affection_band(0.51), "deeply affectionate");
    }

    #[test]
    fn top_dimension_and_alternate_regulation_thresholds_are_exact() {
        let vector = EmotionVector {
            warmth: 0.079,
            trust: 0.08,
            calm: 0.081,
            ..EmotionVector::default()
        };
        assert_eq!(describe_top_dimensions(&vector, 3), "calm (8%), trust (8%)");

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
        let soul_state = soul_state(Vec::new());
        let rendered = render_prompt_state(&CompanionPromptStateInput {
            character_name: "Mira",
            partner_name: Some("Ari"),
            soul: &soul,
            soul_state: &soul_state,
            runtime_state: &state(),
            style_notes: "",
            continuity_episode: 0,
            effective_at: TimestampMillis::new(20),
        });
        assert!(
            rendered.contains("Regulation: relatively emotionally direct when trust is present.")
        );
        assert!(rendered.contains("When unsettled, may avoid asking directly for reassurance."));
    }
}
