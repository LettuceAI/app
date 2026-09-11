use chrono::{Local, LocalResult, TimeZone};
use lettuce_context::PromptVariable as Variable;
use lettuce_conversations::{ElapsedUnit, MemoryObservation, MemoryPromptLine, RelativeTime};
use lettuce_memory::MemoryItem;
use lettuce_types::TimestampMillis;

use crate::runtime_text::{RuntimeText, RuntimeTextError};

pub(crate) fn memory_prompt_line(
    memory: &MemoryItem,
    effective_now: TimestampMillis,
) -> MemoryPromptLine {
    MemoryPromptLine {
        text: memory.text.clone(),
        observed: memory.observed_at.map(|observed_at| {
            let observed = match Local.timestamp_millis_opt(observed_at.get()) {
                LocalResult::Single(datetime) | LocalResult::Ambiguous(datetime, _) => datetime,
                LocalResult::None => Local::now(),
            };
            MemoryObservation {
                local_time: observed.format("%Y-%m-%d %H:%M").to_string(),
                relative: relative_time(effective_now.get() - observed_at.get()),
            }
        }),
    }
}

pub(crate) fn relative_time(delta_ms: i64) -> RelativeTime {
    let future = delta_ms < 0;
    let seconds = delta_ms.unsigned_abs() / 1000;
    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;
    const WEEK: u64 = 7 * DAY;
    const MONTH: u64 = 30 * DAY;
    const YEAR: u64 = 365 * DAY;

    if seconds < 45 {
        return RelativeTime::JustNow;
    }
    let (count, unit) = if seconds < HOUR {
        (seconds / MINUTE, ElapsedUnit::Minute)
    } else if seconds < DAY {
        (seconds / HOUR, ElapsedUnit::Hour)
    } else if seconds < WEEK {
        (seconds / DAY, ElapsedUnit::Day)
    } else if seconds < MONTH {
        (seconds / WEEK, ElapsedUnit::Week)
    } else if seconds < YEAR {
        (seconds / MONTH, ElapsedUnit::Month)
    } else {
        (seconds / YEAR, ElapsedUnit::Year)
    };
    let count = count.max(1);
    if unit == ElapsedUnit::Day && count == 1 {
        return if future {
            RelativeTime::Tomorrow
        } else {
            RelativeTime::Yesterday
        };
    }
    if future {
        RelativeTime::In { count, unit }
    } else {
        RelativeTime::Ago { count, unit }
    }
}

/// The catalog observation note (`observed DATE, RELATIVE`) of a line.
pub(crate) fn render_observation(
    text: &RuntimeText,
    observation: &MemoryObservation,
) -> Result<String, RuntimeTextError> {
    let relative = match observation.relative {
        RelativeTime::JustNow => text.render_with("relative_just_now", [])?,
        RelativeTime::Yesterday => text.render_with("relative_yesterday", [])?,
        RelativeTime::Tomorrow => text.render_with("relative_tomorrow", [])?,
        RelativeTime::Ago { count, unit } => elapsed(text, "relative_ago", count, unit)?,
        RelativeTime::In { count, unit } => elapsed(text, "relative_in", count, unit)?,
    };
    text.render_with(
        "memory_observed",
        [
            (Variable::ObservedTime, observation.local_time.clone()),
            (Variable::ObservedRelative, relative),
        ],
    )
}

/// One memory bullet from the catalog, with its observation note when
/// `observed` is set and the memory has one.
pub(crate) fn render_memory_line(
    text: &RuntimeText,
    line: &MemoryPromptLine,
    observed: bool,
) -> Result<String, RuntimeTextError> {
    let note = match line.observed.as_ref().filter(|_| observed) {
        Some(observation) => render_observation(text, observation)?,
        None => String::new(),
    };
    text.render_with(
        "memory_line",
        [
            (Variable::MemoryText, line.text.clone()),
            (Variable::MemoryObserved, note),
        ],
    )
}

fn elapsed(
    text: &RuntimeText,
    key: &str,
    count: u64,
    unit: ElapsedUnit,
) -> Result<String, RuntimeTextError> {
    let unit_key = match (unit, count == 1) {
        (ElapsedUnit::Minute, true) => "elapsed_minute",
        (ElapsedUnit::Minute, false) => "elapsed_minutes",
        (ElapsedUnit::Hour, true) => "elapsed_hour",
        (ElapsedUnit::Hour, false) => "elapsed_hours",
        (ElapsedUnit::Day, true) => "elapsed_day",
        (ElapsedUnit::Day, false) => "elapsed_days",
        (ElapsedUnit::Week, true) => "elapsed_week",
        (ElapsedUnit::Week, false) => "elapsed_weeks",
        (ElapsedUnit::Month, true) => "elapsed_month",
        (ElapsedUnit::Month, false) => "elapsed_months",
        (ElapsedUnit::Year, true) => "elapsed_year",
        (ElapsedUnit::Year, false) => "elapsed_years",
    };
    text.render_with(
        key,
        [
            (Variable::ElapsedCount, count.to_string()),
            (Variable::ElapsedUnit, text.render_with(unit_key, [])?),
        ],
    )
}

#[cfg(test)]
mod tests {
    use lettuce_conversations::{ElapsedUnit, RelativeTime};
    use lettuce_memory::{MemoryCategory, MemoryItem, Score};
    use lettuce_types::{MemoryId, TimestampMillis};

    use super::{memory_prompt_line, relative_time, render_memory_line, render_observation};
    use crate::BuiltInPromptId;
    use crate::runtime_text::RuntimeText;

    #[test]
    fn observed_memories_carry_the_legacy_suffix() {
        let text = RuntimeText::from_seed(BuiltInPromptId::MemoryRuntime);
        let observed_at = TimestampMillis::new(1_700_000_000_000);
        let mut memory = MemoryItem {
            id: MemoryId::new(),
            short_id: lettuce_memory::MemoryShortId::new(900001).expect("short id"),
            text: "Mira prefers tea".into(),
            category: MemoryCategory::Preference,
            source_message_id: None,
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
            created_at: observed_at,
            last_accessed_at: observed_at,
        };
        let now = TimestampMillis::new(observed_at.get() + 2 * 3_600_000);
        assert_eq!(
            render_memory_line(&text, &memory_prompt_line(&memory, now), true).expect("line"),
            "- Mira prefers tea"
        );
        memory.observed_at = Some(observed_at);
        let line = memory_prompt_line(&memory, now);
        assert_eq!(
            render_memory_line(&text, &line, false).expect("plain"),
            "- Mira prefers tea"
        );
        let observation = line.observed.clone().expect("observed");
        assert_eq!(
            render_memory_line(&text, &line, true).expect("observed"),
            format!(
                "- Mira prefers tea (observed {}, 2 hours ago)",
                observation.local_time
            )
        );
    }

    #[test]
    fn legacy_relative_time_thresholds_and_words_are_preserved() {
        let text = RuntimeText::from_seed(BuiltInPromptId::MemoryRuntime);
        let words = |delta| {
            render_observation(
                &text,
                &lettuce_conversations::MemoryObservation {
                    local_time: "2026-09-11 10:00".into(),
                    relative: relative_time(delta),
                },
            )
            .expect("render")
        };
        assert_eq!(relative_time(44_999), RelativeTime::JustNow);
        assert_eq!(words(44_000), "observed 2026-09-11 10:00, just now");
        assert_eq!(words(45_000), "observed 2026-09-11 10:00, 1 minute ago");
        assert_eq!(words(60_000), "observed 2026-09-11 10:00, 1 minute ago");
        assert_eq!(words(-120_000), "observed 2026-09-11 10:00, in 2 minutes");
        assert_eq!(words(86_400_000), "observed 2026-09-11 10:00, yesterday");
        assert_eq!(words(-86_400_000), "observed 2026-09-11 10:00, tomorrow");
        assert_eq!(
            words(2 * 86_400_000),
            "observed 2026-09-11 10:00, 2 days ago"
        );
        assert_eq!(
            words(14 * 86_400_000),
            "observed 2026-09-11 10:00, 2 weeks ago"
        );
        assert_eq!(
            words(31 * 86_400_000),
            "observed 2026-09-11 10:00, 1 month ago"
        );
        assert_eq!(
            words(-400 * 86_400_000),
            "observed 2026-09-11 10:00, in 1 year"
        );
        assert_eq!(
            words(3_599_000),
            "observed 2026-09-11 10:00, 59 minutes ago"
        );
        assert_eq!(words(3_600_000), "observed 2026-09-11 10:00, 1 hour ago");
        assert_eq!(
            words(-3 * 86_400_000),
            "observed 2026-09-11 10:00, in 3 days"
        );
        assert_eq!(
            words(7 * 86_400_000),
            "observed 2026-09-11 10:00, 1 week ago"
        );
        assert_eq!(
            words(-60 * 86_400_000),
            "observed 2026-09-11 10:00, in 2 months"
        );
        assert_eq!(
            words(730 * 86_400_000),
            "observed 2026-09-11 10:00, 2 years ago"
        );
        assert_eq!(
            relative_time(3 * 3_600_000),
            RelativeTime::Ago {
                count: 3,
                unit: ElapsedUnit::Hour
            }
        );
    }
}
