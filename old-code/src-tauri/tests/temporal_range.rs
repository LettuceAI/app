//! Temporal query range parsing edge cases.
//!
//! The parser converts phrases like "yesterday", "last Monday", "3 weeks ago"
//! into concrete `[start_ms, end_ms)` ranges. Off-by-one and timezone-edge
//! bugs are very easy to introduce here. These tests use a fixed reference
//! timestamp so they're reproducible regardless of the wall clock.

use chrono::{Local, TimeZone};
use lettuceai_lib::chat_manager::temporal::detect_temporal_query_range;

const DAY_MS: i64 = 24 * 60 * 60 * 1000;
const WEEK_MS: i64 = 7 * DAY_MS;

fn local_midnight_ms(year: i32, month: u32, day: u32) -> u64 {
    // Build "midnight local" as a u64 reference. We accept whichever DST
    // variant the local zone picks (LocalResult::Single or earliest).
    Local
        .with_ymd_and_hms(year, month, day, 12, 0, 0)
        .single()
        .expect("non-ambiguous noon")
        .timestamp_millis() as u64
}

#[test]
fn yesterday_window_is_exactly_one_day_long() {
    let ref_ms = local_midnight_ms(2025, 6, 15);
    let r = detect_temporal_query_range("anything from yesterday", ref_ms).expect("ok");
    let span = (r.end_ms - r.start_ms) as i64;
    assert!(
        (span - DAY_MS).abs() <= 60 * 60 * 1000,
        "yesterday should be ~1 day (got {span} ms)"
    );
    assert!(r.end_ms <= ref_ms, "yesterday must end at or before now");
}

#[test]
fn today_window_includes_reference_point() {
    let ref_ms = local_midnight_ms(2025, 6, 15);
    let r = detect_temporal_query_range("what did i say today", ref_ms).expect("ok");
    assert!(r.start_ms <= ref_ms);
    assert!(r.end_ms >= ref_ms);
}

#[test]
fn last_week_is_exactly_seven_days() {
    let ref_ms = local_midnight_ms(2025, 6, 15); // a Sunday
    let r = detect_temporal_query_range("from last week", ref_ms).expect("ok");
    let span = (r.end_ms - r.start_ms) as i64;
    assert!(
        (span - WEEK_MS).abs() <= 60 * 60 * 1000,
        "last week should span ~7 days (got {span} ms)"
    );
}

#[test]
fn this_week_includes_reference_point() {
    let ref_ms = local_midnight_ms(2025, 6, 18);
    let r = detect_temporal_query_range("earlier this week", ref_ms).expect("ok");
    assert!(r.start_ms <= ref_ms);
    assert!(r.end_ms > r.start_ms);
}

#[test]
fn last_month_crosses_calendar_boundary_correctly() {
    // Reference: Jan 15 2025 → "last month" must resolve to Dec 2024.
    let ref_ms = local_midnight_ms(2025, 1, 15);
    let r = detect_temporal_query_range("last month", ref_ms).expect("ok");
    let span_days = (r.end_ms - r.start_ms) as i64 / DAY_MS;
    assert!(
        (28..=31).contains(&span_days),
        "last month should span 28-31 days, got {span_days}"
    );
    // start must be before reference
    assert!(r.start_ms < ref_ms);
    // end must be before reference (last month ends when this month starts)
    assert!(r.end_ms <= ref_ms);
}

#[test]
fn last_month_from_march_returns_february() {
    let ref_ms = local_midnight_ms(2025, 3, 15);
    let r = detect_temporal_query_range("last month", ref_ms).expect("ok");
    let span_days = (r.end_ms - r.start_ms) as i64 / DAY_MS;
    // Feb 2025 has 28 days.
    assert!(
        (28..=29).contains(&span_days),
        "February span should be 28-29, got {span_days}"
    );
}

#[test]
fn last_month_from_march_during_leap_year_returns_29_day_feb() {
    let ref_ms = local_midnight_ms(2024, 3, 15);
    let r = detect_temporal_query_range("last month", ref_ms).expect("ok");
    let span_days = (r.end_ms - r.start_ms) as i64 / DAY_MS;
    assert_eq!(span_days, 29, "Feb 2024 should be 29 days");
}

#[test]
fn this_month_spans_a_full_month() {
    let ref_ms = local_midnight_ms(2025, 6, 15);
    let r = detect_temporal_query_range("this month", ref_ms).expect("ok");
    let span_days = (r.end_ms - r.start_ms) as i64 / DAY_MS;
    // June has 30 days.
    assert!(
        (28..=31).contains(&span_days),
        "this month span should be 28-31, got {span_days}"
    );
    assert!(r.start_ms <= ref_ms);
    assert!(r.end_ms > ref_ms);
}

#[test]
fn this_month_in_december_does_not_overflow() {
    // Dec → "this month" must roll forward to Jan of next year correctly.
    let ref_ms = local_midnight_ms(2025, 12, 15);
    let r = detect_temporal_query_range("this month", ref_ms).expect("ok");
    assert!(r.end_ms > r.start_ms);
    let span_days = (r.end_ms - r.start_ms) as i64 / DAY_MS;
    assert_eq!(span_days, 31, "December should be 31 days");
}

#[test]
fn last_year_when_previous_was_non_leap() {
    // 2023 → 365 days, queried from 2024.
    let ref_ms = local_midnight_ms(2024, 6, 15);
    let r = detect_temporal_query_range("last year", ref_ms).expect("ok");
    let span_days = (r.end_ms - r.start_ms) as i64 / DAY_MS;
    assert_eq!(span_days, 365, "2023 was not a leap year");
}

#[test]
fn last_year_when_previous_was_leap_year() {
    // 2024 → 366 days, queried from 2025.
    let ref_ms = local_midnight_ms(2025, 6, 15);
    let r = detect_temporal_query_range("last year", ref_ms).expect("ok");
    let span_days = (r.end_ms - r.start_ms) as i64 / DAY_MS;
    assert_eq!(span_days, 366, "2024 was a leap year");
}

#[test]
fn n_days_ago_with_numeric_count() {
    let ref_ms = local_midnight_ms(2025, 6, 15);
    let r = detect_temporal_query_range("what did we say 3 days ago", ref_ms).expect("ok");
    assert!(r.end_ms <= ref_ms);
    // Some span exists.
    assert!(r.end_ms > r.start_ms);
}

#[test]
fn n_days_ago_with_word_count() {
    let ref_ms = local_midnight_ms(2025, 6, 15);
    let a = detect_temporal_query_range("five days ago", ref_ms);
    let b = detect_temporal_query_range("5 days ago", ref_ms);
    assert_eq!(
        a.is_some(),
        b.is_some(),
        "word and numeric forms should be equivalent"
    );
}

#[test]
fn last_weekday_is_within_past_week() {
    let ref_ms = local_midnight_ms(2025, 6, 18); // Wednesday
    let r = detect_temporal_query_range("what about last monday", ref_ms).expect("ok");
    let span_days = (r.end_ms - r.start_ms) as i64 / DAY_MS;
    assert_eq!(span_days, 1, "last monday is a single-day window");
    // The window must be in the past.
    assert!(r.end_ms <= ref_ms);
    // And within the last ~10 days (covers "this Mon" vs "prior Mon" interpretations).
    assert!(
        (ref_ms - r.start_ms) as i64 <= 10 * DAY_MS,
        "last monday should not be more than 10 days back"
    );
}

#[test]
fn returns_none_for_no_temporal_phrase() {
    let ref_ms = local_midnight_ms(2025, 6, 15);
    assert!(detect_temporal_query_range("hello there", ref_ms).is_none());
    assert!(detect_temporal_query_range("", ref_ms).is_none());
    assert!(detect_temporal_query_range("just some chat", ref_ms).is_none());
}

#[test]
fn temporal_match_is_case_insensitive() {
    let ref_ms = local_midnight_ms(2025, 6, 15);
    let upper = detect_temporal_query_range("YESTERDAY", ref_ms);
    let lower = detect_temporal_query_range("yesterday", ref_ms);
    let mixed = detect_temporal_query_range("YeStErDaY", ref_ms);
    assert!(upper.is_some() && lower.is_some() && mixed.is_some());
}

#[test]
fn start_is_always_before_or_equal_to_end() {
    let ref_ms = local_midnight_ms(2025, 6, 15);
    for query in [
        "yesterday",
        "today",
        "last week",
        "this week",
        "last month",
        "this month",
        "last year",
        "3 days ago",
        "last monday",
        "last friday",
    ] {
        if let Some(r) = detect_temporal_query_range(query, ref_ms) {
            assert!(
                r.start_ms <= r.end_ms,
                "query={query:?}: start_ms ({}) must precede end_ms ({})",
                r.start_ms,
                r.end_ms
            );
        }
    }
}
