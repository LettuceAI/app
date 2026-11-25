use lettuce_types::TimestampMillis;

use crate::{MemorySummaryUnit, MemorySummaryUnitKind};

const SEPARATOR: &str = "\n\n";

/// The time range covering `units` in order, or `None` when any of them has no range.
#[must_use]
pub fn combined_time_range(
    units: &[&MemorySummaryUnit],
) -> Option<(TimestampMillis, TimestampMillis)> {
    let mut start: Option<TimestampMillis> = None;
    let mut end: Option<TimestampMillis> = None;
    for unit in units {
        let (unit_start, unit_end) = (unit.time_start?, unit.time_end?);
        start = Some(start.map_or(unit_start, |current| current.min(unit_start)));
        end = Some(end.map_or(unit_end, |current| current.max(unit_end)));
    }
    start.zip(end)
}

#[must_use]
pub fn render_summary_units(units: &[MemorySummaryUnit]) -> String {
    units
        .iter()
        .filter(|unit| unit.kind != MemorySummaryUnitKind::Reset)
        .map(|unit| unit.text.trim())
        .collect::<Vec<_>>()
        .join(SEPARATOR)
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryAdoption {
    pub start: usize,
    pub end: usize,
    pub kind: MemorySummaryUnitKind,
    pub level: u32,
    pub window_start: u64,
    pub window_end: u64,
    pub time_start: Option<TimestampMillis>,
    pub time_end: Option<TimestampMillis>,
    pub text: String,
}

impl SummaryAdoption {
    #[must_use]
    pub const fn covers_all(&self, parts: usize) -> bool {
        self.start == 0 && self.end == parts
    }
}
#[must_use]
pub fn plan_summary_adoption(
    parts: &[MemorySummaryUnit],
    published: &str,
    fallback_window_end: u64,
) -> Option<SummaryAdoption> {
    let texts: Vec<&str> = parts.iter().map(|part| part.text.trim()).collect();
    let count = texts.len();
    let mut rest = published.trim();

    let mut prefix = 0;
    while prefix < count {
        let Some(after) = rest.strip_prefix(texts[prefix]) else {
            break;
        };
        if after.is_empty() {
            rest = after;
            prefix += 1;
            continue;
        }
        let Some(after) = after.strip_prefix(SEPARATOR) else {
            break;
        };
        rest = after.trim_start();
        prefix += 1;
    }

    let mut suffix = 0;
    while prefix + suffix < count {
        let Some(before) = rest.strip_suffix(texts[count - 1 - suffix]) else {
            break;
        };
        if before.is_empty() {
            rest = before;
            suffix += 1;
            continue;
        }
        let Some(before) = before.strip_suffix(SEPARATOR) else {
            break;
        };
        rest = before.trim_end();
        suffix += 1;
    }

    let mut start = prefix;
    let mut end = count - suffix;
    let mut text = rest.trim().to_owned();
    if start == end && text.is_empty() {
        return None;
    }
    if text.is_empty() || start == end {
        if start > 0 {
            start -= 1;
            text = join(texts[start], &text);
        } else if end < count {
            text = join(&text, texts[end]);
            end += 1;
        }
    }
    if text.is_empty() {
        return None;
    }

    let covered = &parts[start..end];
    let covered_refs: Vec<&MemorySummaryUnit> = covered.iter().collect();
    let (time_start, time_end) = combined_time_range(&covered_refs)
        .map_or((None, None), |(start, end)| (Some(start), Some(end)));
    let rewrites_all = start == 0 && end == count && count > 1;
    let window_end = covered
        .iter()
        .map(|part| part.window_end)
        .max()
        .unwrap_or(fallback_window_end);
    if covered.is_empty()
        || rewrites_all
        || covered
            .iter()
            .any(|part| part.kind == MemorySummaryUnitKind::Seed)
    {
        return Some(SummaryAdoption {
            start,
            end,
            kind: MemorySummaryUnitKind::Seed,
            level: 0,
            window_start: 0,
            window_end,
            time_start,
            time_end,
            text,
        });
    }
    let level = covered.iter().map(|part| part.level).max().unwrap_or(0);
    Some(SummaryAdoption {
        start,
        end,
        kind: if level == 0 {
            MemorySummaryUnitKind::Cycle
        } else {
            MemorySummaryUnitKind::Chapter
        },
        level,
        window_start: covered
            .iter()
            .map(|part| part.window_start)
            .min()
            .unwrap_or(0),
        window_end,
        time_start,
        time_end,
        text,
    })
}

fn join(first: &str, second: &str) -> String {
    match (first.is_empty(), second.is_empty()) {
        (true, _) => second.to_owned(),
        (_, true) => first.to_owned(),
        _ => format!("{first}{SEPARATOR}{second}"),
    }
}

#[cfg(test)]
mod tests {
    use lettuce_types::{
        ConversationBranchId, ConversationId, DynamicMemoryRunId, MemorySpaceId,
        MemorySummaryUnitId, TimestampMillis,
    };

    use super::*;
    use crate::{MemoryOrigin, MemorySummaryUnitScope};

    fn part(
        kind: MemorySummaryUnitKind,
        level: u32,
        window: (u64, u64),
        text: &str,
    ) -> MemorySummaryUnit {
        MemorySummaryUnit {
            id: MemorySummaryUnitId::new(),
            scope: MemorySummaryUnitScope {
                space_id: MemorySpaceId::new(),
                conversation_id: ConversationId::new(),
                branch_id: ConversationBranchId::new(),
            },
            created_by_run_id: DynamicMemoryRunId::new(),
            kind,
            level,
            origin: MemoryOrigin::Model,
            window_start: window.0,
            window_end: window.1,
            time_start: None,
            time_end: None,
            text: text.to_owned(),
            token_count: 1,
            created_at: TimestampMillis::new(1),
        }
    }

    fn sample() -> Vec<MemorySummaryUnit> {
        vec![
            part(MemorySummaryUnitKind::Chapter, 1, (0, 12), "Chapter one."),
            part(
                MemorySummaryUnitKind::Cycle,
                0,
                (12, 18),
                "Cycle two.\n\nWith a second paragraph.",
            ),
            part(MemorySummaryUnitKind::Cycle, 0, (18, 24), "Cycle three."),
        ]
    }

    #[test]
    fn rendered_parts_are_unchanged() {
        let parts = sample();
        assert_eq!(
            plan_summary_adoption(&parts, &render_summary_units(&parts), 24),
            None
        );
        assert_eq!(
            plan_summary_adoption(&parts, &format!("  {}\n", render_summary_units(&parts)), 24),
            None
        );
    }

    #[test]
    fn an_edited_middle_part_replaces_only_that_part() {
        let parts = sample();
        let published = "Chapter one.\n\nCycle two, corrected.\n\nCycle three.";
        let plan = plan_summary_adoption(&parts, published, 24).expect("plan");
        assert_eq!((plan.start, plan.end), (1, 2));
        assert_eq!(plan.kind, MemorySummaryUnitKind::Cycle);
        assert_eq!((plan.window_start, plan.window_end), (12, 18));
        assert_eq!(plan.text, "Cycle two, corrected.");
    }

    #[test]
    fn an_edited_chapter_keeps_its_level() {
        let parts = sample();
        let published =
            "Chapter one, fixed.\n\nCycle two.\n\nWith a second paragraph.\n\nCycle three.";
        let plan = plan_summary_adoption(&parts, published, 24).expect("plan");
        assert_eq!((plan.start, plan.end, plan.level), (0, 1, 1));
        assert_eq!(plan.kind, MemorySummaryUnitKind::Chapter);
    }

    #[test]
    fn a_deleted_part_is_absorbed_by_its_previous_neighbor() {
        let parts = sample();
        let plan = plan_summary_adoption(&parts, "Chapter one.\n\nCycle three.", 24).expect("plan");
        assert_eq!((plan.start, plan.end), (0, 2));
        assert_eq!(plan.text, "Chapter one.");
        assert_eq!(plan.kind, MemorySummaryUnitKind::Chapter);
        assert_eq!((plan.window_start, plan.window_end), (0, 18));
    }

    #[test]
    fn a_deleted_first_part_is_absorbed_by_the_next_part() {
        let parts = sample();
        let published = "Cycle two.\n\nWith a second paragraph.\n\nCycle three.";
        let plan = plan_summary_adoption(&parts, published, 24).expect("plan");
        assert_eq!((plan.start, plan.end), (0, 2));
        assert_eq!(plan.text, "Cycle two.\n\nWith a second paragraph.");
    }

    #[test]
    fn an_inserted_paragraph_joins_the_part_before_it() {
        let parts = sample();
        let published = format!("{}\n\nA note from me.", render_summary_units(&parts));
        let plan = plan_summary_adoption(&parts, &published, 24).expect("plan");
        assert_eq!((plan.start, plan.end), (2, 3));
        assert_eq!(plan.text, "Cycle three.\n\nA note from me.");
    }

    #[test]
    fn a_full_rewrite_becomes_the_seed() {
        let parts = sample();
        let plan = plan_summary_adoption(&parts, "Something else entirely.", 30).expect("plan");
        assert!(plan.covers_all(parts.len()));
        assert_eq!(plan.kind, MemorySummaryUnitKind::Seed);
        assert_eq!((plan.window_start, plan.window_end), (0, 24));
    }

    #[test]
    fn editing_the_only_part_keeps_it_a_cycle() {
        let parts = vec![part(MemorySummaryUnitKind::Cycle, 0, (0, 6), "Only part.")];
        let plan = plan_summary_adoption(&parts, "Only part, edited.", 6).expect("plan");
        assert_eq!(plan.kind, MemorySummaryUnitKind::Cycle);
    }

    #[test]
    fn a_summary_without_parts_becomes_the_seed() {
        let plan = plan_summary_adoption(&[], "Imported summary.", 40).expect("plan");
        assert_eq!(plan.kind, MemorySummaryUnitKind::Seed);
        assert_eq!((plan.start, plan.end, plan.window_end), (0, 0, 40));
    }

    #[test]
    fn editing_the_seed_keeps_it_the_seed() {
        let mut parts = sample();
        parts[0] = part(MemorySummaryUnitKind::Seed, 0, (0, 12), "Seed text.");
        let published =
            "Seed text, edited.\n\nCycle two.\n\nWith a second paragraph.\n\nCycle three.";
        let plan = plan_summary_adoption(&parts, published, 24).expect("plan");
        assert_eq!((plan.start, plan.end), (0, 1));
        assert_eq!(plan.kind, MemorySummaryUnitKind::Seed);
    }

    #[test]
    fn plan_summary_adoption_carries_combined_time_range_and_none_for_legacy_seed() {
        let mut parts = sample();
        parts[0].time_start = Some(TimestampMillis::new(1_000));
        parts[0].time_end = Some(TimestampMillis::new(2_000));
        parts[1].time_start = Some(TimestampMillis::new(3_000));
        parts[1].time_end = Some(TimestampMillis::new(4_000));
        parts[2].time_start = Some(TimestampMillis::new(5_000));
        parts[2].time_end = Some(TimestampMillis::new(6_000));

        let merged =
            plan_summary_adoption(&parts, "Chapter one.\n\nCycle three.", 24).expect("merged");
        assert_eq!(merged.time_start, Some(TimestampMillis::new(1_000)));
        assert_eq!(merged.time_end, Some(TimestampMillis::new(4_000)));

        let rewritten =
            plan_summary_adoption(&parts, "Completely rewritten.", 24).expect("rewritten");
        assert_eq!(rewritten.kind, MemorySummaryUnitKind::Seed);
        assert_eq!(rewritten.time_start, Some(TimestampMillis::new(1_000)));
        assert_eq!(rewritten.time_end, Some(TimestampMillis::new(6_000)));

        parts[1].time_start = None;
        parts[1].time_end = None;
        let partial =
            plan_summary_adoption(&parts, "Chapter one.\n\nCycle three.", 24).expect("partial");
        assert_eq!(partial.time_start, None);
        assert_eq!(partial.time_end, None);

        let legacy = plan_summary_adoption(&[], "Imported summary.", 40).expect("legacy");
        assert_eq!(legacy.kind, MemorySummaryUnitKind::Seed);
        assert_eq!(legacy.time_start, None);
        assert_eq!(legacy.time_end, None);
    }
}
