//! Gathered from inline tests in src/storage_manager/companion_scheduled_notes.rs.

use lettuceai_lib::storage_manager::companion_scheduled_notes::{
    is_note_active, local_datetime_to_ms, most_recent_occurrence_on_or_before,
    next_occurrence_after_ms, resolve_local_datetime, CompanionScheduledNote,
};

fn note(
    available_at: u64,
    recurrence: &str,
    recurrence_window_ms: Option<u64>,
) -> CompanionScheduledNote {
    CompanionScheduledNote {
        id: "n1".to_string(),
        character_id: "c1".to_string(),
        label: String::new(),
        content: "Test".to_string(),
        available_at,
        expires_at: None,
        recurrence: recurrence.to_string(),
        recurrence_window_ms,
        enabled: true,
        created_at: available_at,
        updated_at: available_at,
    }
}

fn ms(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> u64 {
    local_datetime_to_ms(
        resolve_local_datetime(year, month, day, hour, minute, 0, 0).expect("valid local datetime"),
    )
}

#[test]
fn non_recurring_note_activates_after_start() {
    let start = ms(2026, 6, 15, 9, 0);
    let note = note(start, "none", None);
    assert!(!is_note_active(&note, start.saturating_sub(1)).unwrap());
    assert!(is_note_active(&note, start).unwrap());
}

#[test]
fn yearly_note_uses_window() {
    let start = ms(2024, 6, 15, 9, 0);
    let note = note(start, "yearly", Some(24 * 60 * 60 * 1000));
    assert!(is_note_active(&note, ms(2026, 6, 15, 10, 0)).unwrap());
    assert!(!is_note_active(&note, ms(2026, 6, 16, 10, 0)).unwrap());
}

#[test]
fn feb_29_yearly_rounds_to_feb_28() {
    let start = ms(2024, 2, 29, 8, 0);
    let occurrence =
        most_recent_occurrence_on_or_before(start, "yearly", ms(2025, 2, 28, 12, 0)).unwrap();
    assert_eq!(occurrence, ms(2025, 2, 28, 8, 0));
}

#[test]
fn monthly_note_rounds_to_last_day_of_month() {
    let start = ms(2026, 1, 31, 8, 0);
    let occurrence =
        most_recent_occurrence_on_or_before(start, "monthly", ms(2026, 2, 28, 12, 0)).unwrap();
    assert_eq!(occurrence, ms(2026, 2, 28, 8, 0));
}

#[test]
fn next_occurrence_advances_weekly() {
    let start = ms(2026, 5, 1, 9, 0);
    let next = next_occurrence_after_ms(start, "weekly", start)
        .unwrap()
        .unwrap();
    assert_eq!(next, ms(2026, 5, 8, 9, 0));
}
