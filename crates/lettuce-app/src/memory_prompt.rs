use chrono::{Local, LocalResult, TimeZone};
use lettuce_conversations::MemoryPromptLine;
use lettuce_memory::MemoryItem;
use lettuce_types::TimestampMillis;

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
            format!(
                "observed {}, {}",
                observed.format("%Y-%m-%d %H:%M"),
                humanize_relative(effective_now.get() - observed_at.get())
            )
        }),
    }
}

pub(crate) fn humanize_relative(delta_ms: i64) -> String {
    let future = delta_ms < 0;
    let seconds = delta_ms.unsigned_abs() / 1000;
    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;
    const WEEK: u64 = 7 * DAY;
    const MONTH: u64 = 30 * DAY;
    const YEAR: u64 = 365 * DAY;

    if seconds < 45 {
        return "just now".to_owned();
    }
    let (count, unit) = if seconds < HOUR {
        (seconds / MINUTE, "minute")
    } else if seconds < DAY {
        (seconds / HOUR, "hour")
    } else if seconds < WEEK {
        (seconds / DAY, "day")
    } else if seconds < MONTH {
        (seconds / WEEK, "week")
    } else if seconds < YEAR {
        (seconds / MONTH, "month")
    } else {
        (seconds / YEAR, "year")
    };
    let count = count.max(1);
    if unit == "day" && count == 1 {
        return if future { "tomorrow" } else { "yesterday" }.to_owned();
    }
    let plural = if count == 1 { "" } else { "s" };
    if future {
        format!("in {count} {unit}{plural}")
    } else {
        format!("{count} {unit}{plural} ago")
    }
}

#[cfg(test)]
mod tests {
    use lettuce_memory::{MemoryCategory, MemoryItem, Score};
    use lettuce_types::{MemoryId, TimestampMillis};

    use super::{humanize_relative, memory_prompt_line};

    #[test]
    fn observed_memories_carry_the_legacy_suffix() {
        let observed_at = TimestampMillis::new(1_700_000_000_000);
        let mut memory = MemoryItem {
            id: MemoryId::new(),
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
            memory_prompt_line(&memory, now).with_observed(),
            "- Mira prefers tea"
        );
        memory.observed_at = Some(observed_at);
        let line = memory_prompt_line(&memory, now);
        assert_eq!(line.plain(), "- Mira prefers tea");
        let rendered = line.with_observed();
        assert!(rendered.starts_with("- Mira prefers tea (observed "));
        assert!(rendered.ends_with(", 2 hours ago)"));
    }

    #[test]
    fn legacy_relative_time_thresholds_are_preserved() {
        assert_eq!(humanize_relative(44_000), "just now");
        assert_eq!(humanize_relative(60_000), "1 minute ago");
        assert_eq!(humanize_relative(86_400_000), "yesterday");
        assert_eq!(humanize_relative(-86_400_000), "tomorrow");
        assert_eq!(humanize_relative(14 * 86_400_000), "2 weeks ago");
    }
}
