//! Integration tests for lorebook keyword activation logic.

use lettuceai_lib::chat_manager::prompting::lorebook_matcher::{
    activate_lorebook_entries, format_lorebook_for_prompt,
};
use lettuceai_lib::storage_manager::lorebook::{
    LorebookEntry, LorebookEntryActivationContext, LorebookKeywordDetectionMode,
};

fn entry(
    id: &str,
    title: &str,
    keywords: Vec<&str>,
    content: &str,
    always_active: bool,
) -> LorebookEntry {
    LorebookEntry {
        id: id.into(),
        lorebook_id: "lb-1".into(),
        title: title.into(),
        enabled: true,
        always_active,
        keywords: keywords.into_iter().map(String::from).collect(),
        case_sensitive: false,
        keyword_match_mode: Default::default(),
        content: content.into(),
        priority: 0,
        display_order: 0,
        created_at: 0,
        updated_at: 0,
    }
}

fn ctx(e: LorebookEntry) -> LorebookEntryActivationContext {
    LorebookEntryActivationContext {
        entry: e,
        keyword_detection_mode: LorebookKeywordDetectionMode::RecentMessageWindow,
    }
}

#[test]
fn activate_keyword_match_in_recent_messages() {
    let entries = vec![ctx(entry(
        "1",
        "dragons",
        vec!["dragon"],
        "Dragons are large reptilian creatures.",
        false,
    ))];
    let recent = vec!["The dragon roared".to_string()];
    let activated = activate_lorebook_entries(entries, &recent, None);
    assert_eq!(activated.len(), 1);
    assert_eq!(activated[0].id, "1");
}

#[test]
fn activate_no_match_excludes_entry() {
    let entries = vec![ctx(entry(
        "1",
        "dragons",
        vec!["dragon"],
        "Dragons are large reptilian creatures.",
        false,
    ))];
    let recent = vec!["The cat purred".to_string()];
    let activated = activate_lorebook_entries(entries, &recent, None);
    assert!(activated.is_empty());
}

#[test]
fn activate_always_active_with_no_keywords() {
    let entries = vec![ctx(entry("1", "world", vec![], "World setting.", true))];
    let recent = vec!["Hello".to_string()];
    let activated = activate_lorebook_entries(entries, &recent, None);
    assert_eq!(activated.len(), 1);
}

#[test]
fn activate_empty_entries_returns_empty() {
    let entries: Vec<LorebookEntryActivationContext> = vec![];
    let recent = vec!["anything".to_string()];
    let activated = activate_lorebook_entries(entries, &recent, None);
    assert!(activated.is_empty());
}

#[test]
fn activate_sorts_by_display_order() {
    let mut e1 = entry("1", "first", vec!["x"], "first", false);
    e1.display_order = 100;
    let mut e2 = entry("2", "second", vec!["x"], "second", false);
    e2.display_order = 0;
    let entries = vec![ctx(e1), ctx(e2)];
    let recent = vec!["x mark".to_string()];
    let activated = activate_lorebook_entries(entries, &recent, None);
    assert_eq!(activated.len(), 2);
    assert_eq!(activated[0].display_order, 0);
    assert_eq!(activated[1].display_order, 100);
}

#[test]
fn activate_latest_user_message_mode_uses_that_string() {
    let entries = vec![LorebookEntryActivationContext {
        entry: entry("1", "x", vec!["needle"], "content", false),
        keyword_detection_mode: LorebookKeywordDetectionMode::LatestUserMessage,
    }];
    let recent = vec!["nothing relevant".to_string()];
    let activated_with = activate_lorebook_entries(entries, &recent, Some("needle in haystack"));
    assert_eq!(activated_with.len(), 1);
}

#[test]
fn activate_or_logic_across_keywords() {
    let entries = vec![ctx(entry(
        "1",
        "alt",
        vec!["alpha", "beta", "gamma"],
        "content",
        false,
    ))];
    let recent = vec!["just beta here".to_string()];
    let activated = activate_lorebook_entries(entries, &recent, None);
    assert_eq!(
        activated.len(),
        1,
        "any one matching keyword should activate"
    );
}

// format_lorebook_for_prompt
#[test]
fn format_joins_with_double_newlines() {
    let entries = vec![
        entry("1", "a", vec![], "first entry", false),
        entry("2", "b", vec![], "second entry", false),
    ];
    let out = format_lorebook_for_prompt(&entries);
    assert!(out.contains("first entry"));
    assert!(out.contains("second entry"));
    assert!(out.contains("\n\n"));
}

#[test]
fn format_empty_entries_empty_string() {
    let entries: Vec<LorebookEntry> = vec![];
    assert_eq!(format_lorebook_for_prompt(&entries), "");
}

#[test]
fn format_filters_whitespace_only_content() {
    let entries = vec![
        entry("1", "a", vec![], "   ", false),
        entry("2", "b", vec![], "actual content", false),
    ];
    let out = format_lorebook_for_prompt(&entries);
    assert!(out.contains("actual content"));
    // Should not have leading/trailing whitespace block
    assert_eq!(out.trim(), out);
}
