use std::cell::Cell;

use lettuce_companions::{
    CompanionPromptState, CompanionScheduledNote, EmotionDimension, EmotionReading, ReassuranceCue,
    RegulationCue, RelationshipBand, SoulCategory, scheduled_note_lines,
};
use lettuce_context::{PromptRenderValues, PromptVariable as Variable};

use crate::runtime_text::{RuntimeText, RuntimeTextError};

/// The live companion state block, one catalog line per fact the companion
/// crate decided on. A disabled or blank line is left out.
pub(crate) fn render_companion_state(
    text: &RuntimeText,
    character_name: &str,
    partner_name: Option<&str>,
    state: &CompanionPromptState,
) -> Result<String, RuntimeTextError> {
    let partner = match partner_name.map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) => name.to_owned(),
        None => text.render_with("companion_partner_fallback", [])?,
    };
    let names = PromptRenderValues {
        character_name: character_name.to_owned(),
        persona_name: partner,
        ..PromptRenderValues::default()
    };
    let mut lines = Vec::new();
    let mut line = |key: &str, variables: Vec<(Variable, String)>| {
        let mut values = names.clone();
        values.purpose_values.extend(variables);
        if let Some(rendered) = text.render(key, &values)?.filter(|line| !line.is_empty()) {
            lines.push(rendered);
        }
        Ok::<_, RuntimeTextError>(())
    };
    line("companion_state_intro", Vec::new())?;
    line("companion_state_scope", Vec::new())?;
    line("companion_state_bidirectional", Vec::new())?;
    line("companion_state_supporting_signals", Vec::new())?;
    line(
        if state.interaction_count == 1 {
            "companion_duration_one"
        } else {
            "companion_duration_many"
        },
        vec![(
            Variable::InteractionCount,
            state.interaction_count.to_string(),
        )],
    )?;
    line(
        "companion_stance",
        vec![
            (
                Variable::ClosenessBand,
                text.render_with(&band_key("closeness", state.closeness), [])?,
            ),
            (
                Variable::TrustBand,
                text.render_with(&band_key("trust", state.trust), [])?,
            ),
            (
                Variable::AffectionBand,
                text.render_with(&band_key("affection", state.affection), [])?,
            ),
            (Variable::TensionPercent, percent(state.tension)),
        ],
    )?;
    line(
        "companion_expressed_tone",
        vec![(
            Variable::EmotionList,
            if state.expressed.is_empty() {
                text.render_with("companion_expressed_tone_quiet", [])?
            } else {
                emotion_list(text, &state.expressed)?
            },
        )],
    )?;
    if let Some(episode) = state.continuity_episode {
        line(
            "companion_continuity",
            vec![(Variable::ContinuityEpisode, episode.to_string())],
        )?;
    }
    for (category, value) in &state.soul {
        line(
            soul_key(*category),
            vec![(Variable::SoulValue, value.clone())],
        )?;
    }
    if let Some(notes) = &state.style_notes {
        line(
            "companion_style_notes",
            vec![(Variable::SoulValue, notes.clone())],
        )?;
    }
    if !state.blocked.is_empty() {
        line(
            "companion_blocked_tone",
            vec![(Variable::EmotionList, emotion_list(text, &state.blocked)?)],
        )?;
    }
    if !state.active_signals.is_empty() {
        line(
            "companion_active_signals",
            vec![(Variable::ActiveSignals, state.active_signals.join(", "))],
        )?;
    }
    match state.regulation {
        Some(RegulationCue::Suppressed) => line("companion_regulation_suppressed", Vec::new())?,
        Some(RegulationCue::Transparent) => line("companion_regulation_transparent", Vec::new())?,
        None => {}
    }
    match state.reassurance {
        Some(ReassuranceCue::Open) => line("companion_reassurance_open", Vec::new())?,
        Some(ReassuranceCue::Avoidant) => line("companion_reassurance_avoidant", Vec::new())?,
        None => {}
    }
    Ok(lines.join("\n"))
}

/// Active scheduled notes under the legacy budget, wrapped in the catalog
/// block, or `None` when no line remains or the block is disabled.
pub(crate) fn render_scheduled_notes(
    text: &RuntimeText,
    notes: &[CompanionScheduledNote],
) -> Result<Option<String>, RuntimeTextError> {
    let truncated = text.render_with("scheduled_note_truncated", [])?;
    let failure = Cell::new(None);
    let mut lines = scheduled_note_lines(notes, &truncated, &|content| {
        text.render_with(
            "scheduled_note_line",
            [(Variable::NoteText, content.to_owned())],
        )
        .unwrap_or_else(|error| {
            failure.set(Some(error));
            String::new()
        })
    });
    if let Some(error) = failure.get() {
        return Err(error);
    }
    lines.retain(|line| !line.is_empty());
    if lines.is_empty() {
        return Ok(None);
    }
    let mut values = PromptRenderValues::default();
    values
        .purpose_values
        .insert(Variable::ScheduledNotes, lines.join("\n"));
    Ok(text
        .render("scheduled_notes_block", &values)?
        .filter(|block| !block.is_empty()))
}

fn percent(value: f64) -> String {
    format!("{:.0}", value * 100.0)
}

fn emotion_list(
    text: &RuntimeText,
    readings: &[EmotionReading],
) -> Result<String, RuntimeTextError> {
    Ok(readings
        .iter()
        .map(|reading| {
            text.render_with(
                "companion_emotion_item",
                [
                    (
                        Variable::EmotionLabel,
                        text.render_with(emotion_key(reading.dimension), [])?,
                    ),
                    (Variable::EmotionPercent, percent(reading.value)),
                ],
            )
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>()
        .join(", "))
}

fn band_key(metric: &str, band: RelationshipBand) -> String {
    let suffix = match band {
        RelationshipBand::Lowest => "lowest",
        RelationshipBand::Low => "low",
        RelationshipBand::Neutral => "neutral",
        RelationshipBand::High => "high",
        RelationshipBand::Highest => "highest",
    };
    format!("companion_{metric}_{suffix}")
}

const fn emotion_key(dimension: EmotionDimension) -> &'static str {
    match dimension {
        EmotionDimension::Warmth => "companion_emotion_warmth",
        EmotionDimension::Trust => "companion_emotion_trust",
        EmotionDimension::Calm => "companion_emotion_calm",
        EmotionDimension::Vulnerability => "companion_emotion_vulnerability",
        EmotionDimension::Longing => "companion_emotion_longing",
        EmotionDimension::Hurt => "companion_emotion_hurt",
        EmotionDimension::Tension => "companion_emotion_tension",
        EmotionDimension::Irritation => "companion_emotion_irritation",
        EmotionDimension::Affection => "companion_emotion_affection",
        EmotionDimension::ReassuranceNeed => "companion_emotion_reassurance_need",
    }
}

const fn soul_key(category: SoulCategory) -> &'static str {
    match category {
        SoulCategory::Essence => "companion_soul_essence",
        SoulCategory::Traits => "companion_soul_traits",
        SoulCategory::Backstory => "companion_soul_backstory",
        SoulCategory::Appearance => "companion_soul_appearance",
        SoulCategory::Goals => "companion_soul_goals",
        SoulCategory::Likes => "companion_soul_likes",
        SoulCategory::Voice => "companion_soul_voice",
        SoulCategory::RelationalStyle => "companion_soul_relational_style",
        SoulCategory::Vulnerabilities => "companion_soul_vulnerabilities",
        SoulCategory::Fears => "companion_soul_fears",
        SoulCategory::Habits => "companion_soul_habits",
        SoulCategory::Boundaries => "companion_soul_boundaries",
    }
}

#[cfg(test)]
mod tests {
    use lettuce_companions::{
        CompanionPromptState, CompanionScheduledNote, EmotionDimension, EmotionReading,
        ReassuranceCue, RegulationCue, RelationshipBand, ScheduledNoteRecurrence, SoulCategory,
    };
    use lettuce_types::{CharacterId, TimestampMillis};

    use super::{emotion_key, render_companion_state, render_scheduled_notes, soul_key};
    use crate::BuiltInPromptId;
    use crate::runtime_text::RuntimeText;

    fn quiet_state() -> CompanionPromptState {
        CompanionPromptState {
            interaction_count: 0,
            closeness: RelationshipBand::Neutral,
            trust: RelationshipBand::Neutral,
            affection: RelationshipBand::Neutral,
            tension: 0.0,
            expressed: vec![
                EmotionReading {
                    dimension: EmotionDimension::Calm,
                    value: 0.5,
                },
                EmotionReading {
                    dimension: EmotionDimension::Warmth,
                    value: 0.34,
                },
                EmotionReading {
                    dimension: EmotionDimension::Trust,
                    value: 0.3,
                },
            ],
            continuity_episode: None,
            soul: Vec::new(),
            style_notes: None,
            blocked: Vec::new(),
            active_signals: Vec::new(),
            regulation: None,
            reassurance: None,
        }
    }

    #[test]
    fn default_state_is_byte_exact_with_the_partner_fallback() {
        let text = RuntimeText::from_seed(BuiltInPromptId::CompanionRuntime);
        assert_eq!(
            render_companion_state(&text, "Mira", Some("  "), &quiet_state()).expect("render"),
            "The following relationship and emotional state describes Mira's live relationship with the current conversation partner, the person currently speaking in this chat.\nDo not apply these metrics to third-party people mentioned in character definitions, persona descriptions, lore, or memories unless that relationship is explicitly stated.\nCloseness, trust, and affection are bidirectional: they can run negative, meaning the character actively dislikes, distrusts, or wants distance from the partner, not merely feels neutral.\nTreat these metrics as supporting signals, not as permission to contradict the chat history, memories, or established relationship events. Preserve established emotional breakthroughs as settled continuity; never reset or rediscover them merely because a metric band is lower.\nRelationship duration context: this session state has tracked 0 user interactions.\nCurrent Mira <-> the current conversation partner relationship stance: closeness acquainted, trust neutral, affection neutral; tension 0%.\nExpressed tone right now: calm (50%), warmth (34%), trust (30%)."
        );
    }

    #[test]
    fn every_optional_line_renders_legacy_wording_in_legacy_order() {
        let text = RuntimeText::from_seed(BuiltInPromptId::CompanionRuntime);
        let state = CompanionPromptState {
            interaction_count: 1,
            closeness: RelationshipBand::Lowest,
            trust: RelationshipBand::Low,
            affection: RelationshipBand::Highest,
            tension: 0.456,
            expressed: Vec::new(),
            continuity_episode: Some(2),
            soul: vec![
                (SoulCategory::Essence, "Curious".into()),
                (SoulCategory::Likes, "Tea Harbors".into()),
            ],
            style_notes: Some("restrained".into()),
            blocked: vec![EmotionReading {
                dimension: EmotionDimension::Hurt,
                value: 0.08,
            }],
            active_signals: vec!["emotion:conflict".into(), "time:gap".into()],
            regulation: Some(RegulationCue::Suppressed),
            reassurance: Some(ReassuranceCue::Open),
        };
        let rendered =
            render_companion_state(&text, "Mira", Some(" Ari "), &state).expect("render");
        let tail = rendered.lines().skip(4).collect::<Vec<_>>();
        assert_eq!(
            tail,
            vec![
                "Relationship duration context: this session state has tracked 1 user interaction.",
                "Current Mira <-> Ari relationship stance: closeness withdrawing/wants distance, trust wary, affection deeply affectionate; tension 46%.",
                "Expressed tone right now: steady and low-intensity.",
                "Continuity: this chat is episode 2 of one continuous relationship. Treat earlier shared memories and settled milestones as prior episodes, not as events that need to be rediscovered.",
                "Soul essence: Curious.",
                "Likes and favorites: Tea Harbors.",
                "Companion style notes: restrained.",
                "More strongly felt than shown: hurt (8%).",
                "Recent drivers in Mira's interaction with Ari: emotion:conflict, time:gap.",
                "Regulation: tends to hide direct hurt and avoids blunt emotional disclosure.",
                "When unsettled, may seek reassurance more openly.",
            ]
        );
        let state = CompanionPromptState {
            closeness: RelationshipBand::Highest,
            trust: RelationshipBand::High,
            affection: RelationshipBand::Low,
            regulation: Some(RegulationCue::Transparent),
            reassurance: Some(ReassuranceCue::Avoidant),
            ..quiet_state()
        };
        let rendered = render_companion_state(&text, "Mira", Some("Ari"), &state).expect("render");
        assert!(
            rendered.contains(
                "closeness intimate, trust trusting, affection cold/irritated; tension 0%."
            )
        );
        assert!(rendered.ends_with(
            "Regulation: relatively emotionally direct when trust is present.\nWhen unsettled, may avoid asking directly for reassurance."
        ));
    }

    #[test]
    fn every_catalog_key_the_renderer_uses_exists() {
        let seed = crate::BuiltInPromptCatalog::bundled()
            .expect("catalog")
            .seed(BuiltInPromptId::CompanionRuntime)
            .entries
            .iter()
            .filter_map(|entry| entry.built_in_entry_key.clone())
            .collect::<Vec<_>>();
        let mut keys = [
            "companion_partner_fallback",
            "companion_state_intro",
            "companion_state_scope",
            "companion_state_bidirectional",
            "companion_state_supporting_signals",
            "companion_duration_one",
            "companion_duration_many",
            "companion_stance",
            "companion_expressed_tone",
            "companion_expressed_tone_quiet",
            "companion_emotion_item",
            "companion_continuity",
            "companion_style_notes",
            "companion_blocked_tone",
            "companion_active_signals",
            "companion_regulation_suppressed",
            "companion_regulation_transparent",
            "companion_reassurance_open",
            "companion_reassurance_avoidant",
            "scheduled_notes_block",
            "scheduled_note_line",
            "scheduled_note_truncated",
        ]
        .map(str::to_owned)
        .to_vec();
        for metric in ["closeness", "trust", "affection"] {
            for band in [
                RelationshipBand::Lowest,
                RelationshipBand::Low,
                RelationshipBand::Neutral,
                RelationshipBand::High,
                RelationshipBand::Highest,
            ] {
                keys.push(super::band_key(metric, band));
            }
        }
        keys.extend(
            lettuce_companions::SOUL_PROMPT_ORDER.map(|category| soul_key(category).to_owned()),
        );
        keys.extend(
            [
                EmotionDimension::Warmth,
                EmotionDimension::Trust,
                EmotionDimension::Calm,
                EmotionDimension::Vulnerability,
                EmotionDimension::Longing,
                EmotionDimension::Hurt,
                EmotionDimension::Tension,
                EmotionDimension::Irritation,
                EmotionDimension::Affection,
                EmotionDimension::ReassuranceNeed,
            ]
            .map(|dimension| emotion_key(dimension).to_owned()),
        );
        for key in &keys {
            assert!(seed.contains(key), "{key}");
        }
        assert_eq!(keys.len(), seed.len());
    }

    #[test]
    fn scheduled_notes_render_the_legacy_block_and_honor_disabled_entries() {
        let mut text = RuntimeText::from_seed(BuiltInPromptId::CompanionRuntime);
        let note = CompanionScheduledNote {
            id: uuid::Uuid::from_u128(1),
            character_id: CharacterId::new(),
            label: "reminder".into(),
            content: " remember this ".into(),
            available_at: TimestampMillis::new(1),
            expires_at: None,
            recurrence: ScheduledNoteRecurrence::None,
            recurrence_window_ms: None,
            enabled: true,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        };
        assert_eq!(
            render_scheduled_notes(&text, std::slice::from_ref(&note)).expect("render"),
            Some("[Background context you currently hold in mind]\n- remember this".into())
        );
        assert_eq!(render_scheduled_notes(&text, &[]).expect("empty"), None);
        text.disable_for_test("scheduled_note_line");
        assert_eq!(
            render_scheduled_notes(&text, std::slice::from_ref(&note)).expect("disabled line"),
            None
        );
    }
}
