//! Moved from inline tests in src/chat_manager/temporal.rs.

use chrono::{Local, TimeZone};
use lettuceai_lib::chat_manager::temporal::{
    detect_temporal_query_range, format_message_timestamp, strip_echoed_time_stamps,
    timestamped_message_text,
};
use lettuceai_lib::chat_manager::types::StoredMessage;

fn local_ms(year: i32, month: u32, day: u32, hour: u32) -> u64 {
    Local
        .with_ymd_and_hms(year, month, day, hour, 0, 0)
        .earliest()
        .expect("valid local datetime")
        .timestamp_millis() as u64
}

#[test]
fn parses_last_week() {
    let reference = local_ms(2026, 5, 10, 12);
    let range =
        detect_temporal_query_range("what place did we go to last week", reference).expect("range");
    assert!(range.start_ms < range.end_ms);
}

#[test]
fn parses_days_ago() {
    let reference = local_ms(2026, 5, 10, 12);
    let range =
        detect_temporal_query_range("where did we eat 2 days ago", reference).expect("range");
    assert!(range.start_ms < range.end_ms);
}

#[test]
fn parses_last_saturday() {
    let reference = local_ms(2026, 5, 10, 12);
    let range = detect_temporal_query_range("what did we do after coffee last saturday", reference)
        .expect("range");
    assert!(range.start_ms < range.end_ms);
}

#[test]
fn parses_five_weeks_ago_today() {
    let reference = local_ms(2026, 5, 10, 12);
    let range =
        detect_temporal_query_range("what did we do 5 week ago today", reference).expect("range");
    assert!(range.end_ms - range.start_ms <= 86_400_000);
}

#[test]
fn parses_word_number_weekday_ago() {
    let reference = local_ms(2026, 5, 10, 12);
    let range =
        detect_temporal_query_range("what did we do two fridays ago", reference).expect("range");
    assert!(range.start_ms < range.end_ms);
}

#[test]
fn stamp_is_wrapped_in_a_time_tag() {
    let stamp = format_message_timestamp(local_ms(2026, 3, 12, 18));
    assert_eq!(stamp, "<time>2026-03-12 18:00</time>");
}

#[test]
fn stored_effective_time_wins_over_real_creation_time_for_either_role() {
    for role in ["user", "assistant"] {
        let message = StoredMessage {
            id: format!("{role}-message"),
            role: role.to_string(),
            content: "Hello".to_string(),
            created_at: local_ms(2026, 8, 7, 9),
            effective_at: Some(local_ms(2034, 1, 2, 21)),
            visible_in_chat: false,
            scene_edited: false,
            usage: None,
            variants: Vec::new(),
            selected_variant_id: None,
            memory_refs: Vec::new(),
            used_lorebook_entries: Vec::new(),
            is_pinned: false,
            attachments: Vec::new(),
            reasoning: None,
            model_id: None,
            gemini_content: None,
        };
        assert_eq!(
            timestamped_message_text(&message),
            "<time>2034-01-02 21:00</time> Hello"
        );
    }
}

#[test]
fn strips_echoed_time_tag() {
    let stripped = strip_echoed_time_stamps("<time>Thu 6:50 PM, 2026-03-12</time> Hey, you're up late.");
    assert_eq!(stripped, "Hey, you're up late.");
}

#[test]
fn strips_unclosed_time_tag_at_line_end() {
    let stripped = strip_echoed_time_stamps("<time>Thu 6:50 PM, 2026-03-12\nHey, you're up late.");
    assert_eq!(stripped, "Hey, you're up late.");
}

#[test]
fn strips_legacy_bracket_stamp() {
    let stripped = strip_echoed_time_stamps("[Thu 6:50 PM, 2026-03-12] Hey.");
    assert_eq!(stripped, "Hey.");
}

#[test]
fn strips_invented_leading_stamp() {
    assert_eq!(strip_echoed_time_stamps("[6:50 PM] Hey."), "Hey.");
    assert_eq!(strip_echoed_time_stamps("(Thursday, 6:50 PM) Hey."), "Hey.");
    assert_eq!(strip_echoed_time_stamps("**6:50 AM** Hey."), "Hey.");
    assert_eq!(
        strip_echoed_time_stamps("[2026-03-12 18:50] Hey."),
        "[2026-03-12 18:50] Hey."
    );
}

#[test]
fn keeps_roleplay_brackets() {
    assert_eq!(strip_echoed_time_stamps("[she smiles] Hey."), "[she smiles] Hey.");
    assert_eq!(
        strip_echoed_time_stamps("[she checks her watch at 6:50] Hey."),
        "[she checks her watch at 6:50] Hey."
    );
    assert_eq!(strip_echoed_time_stamps("*she smiles* Hey."), "*she smiles* Hey.");
}

#[test]
fn keeps_a_mid_message_clock_reference() {
    let text = "I'll be there at 6:50 PM, promise.";
    assert_eq!(strip_echoed_time_stamps(text), text);
}
