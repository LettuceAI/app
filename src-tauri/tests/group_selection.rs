//! Group-chat character selection: mentions, heuristic scoring,
//! round-robin, tool-call response parsing.
//!
//! These cover real-world inputs the LLM produces (often messy) plus the
//! invariants the algorithms must hold (e.g., never pick a muted speaker).

use lettuceai_lib::group_chat_manager::selection::{
    heuristic_select_speaker, parse_mentions, parse_tool_call_response, round_robin_select_speaker,
};
use lettuceai_lib::group_chat_manager::{CharacterInfo, GroupChatContext};
use lettuceai_lib::storage_manager::group_sessions::{
    GroupMessage, GroupParticipation, GroupSession,
};

fn character(id: &str, name: &str) -> CharacterInfo {
    CharacterInfo {
        id: id.into(),
        name: name.into(),
        definition: None,
        description: None,
        personality_summary: None,
        memory_type: "manual".into(),
    }
}

fn session(ids: &[&str], muted: &[&str]) -> GroupSession {
    GroupSession {
        id: "s".into(),
        group_character_id: None,
        name: "test".into(),
        character_ids: ids.iter().map(|s| s.to_string()).collect(),
        muted_character_ids: muted.iter().map(|s| s.to_string()).collect(),
        persona_id: None,
        created_at: 0,
        updated_at: 0,
        archived: false,
        author_note: None,
        chat_type: "conversation".into(),
        starting_scene: None,
        background_image_path: None,
        lorebook_ids: vec![],
        disable_character_lorebooks: false,
        memories: vec![],
        memory_embeddings: vec![],
        memory_summary: String::new(),
        memory_summary_token_count: 0,
        memory_tool_events: vec![],
        memory_status: "idle".into(),
        memory_error: None,
        memory_progress_step: None,
        speaker_selection_method: "llm".into(),
        memory_type: "manual".into(),
    }
}

fn participation(char_id: &str, count: i32, last_turn: Option<i32>) -> GroupParticipation {
    GroupParticipation {
        id: format!("p-{char_id}"),
        session_id: "s".into(),
        character_id: char_id.into(),
        speak_count: count,
        last_spoke_turn: last_turn,
        last_spoke_at: None,
    }
}

fn assistant_message(speaker: &str, turn: i32, content: &str) -> GroupMessage {
    GroupMessage {
        id: format!("m-{turn}"),
        session_id: "s".into(),
        role: "assistant".into(),
        content: content.into(),
        speaker_character_id: Some(speaker.into()),
        turn_number: turn,
        created_at: 0,
        usage: None,
        variants: None,
        selected_variant_id: None,
        is_pinned: false,
        attachments: vec![],
        used_lorebook_entries: vec![],
        memory_refs: vec![],
        reasoning: None,
        selection_reasoning: None,
        model_id: None,
    }
}

fn ctx(
    chars: Vec<CharacterInfo>,
    stats: Vec<GroupParticipation>,
    recent: Vec<GroupMessage>,
    muted: &[&str],
) -> GroupChatContext {
    let ids: Vec<&str> = chars.iter().map(|c| c.id.as_str()).collect();
    GroupChatContext {
        session: session(&ids, muted),
        characters: chars,
        participation_stats: stats,
        recent_messages: recent,
        user_message: "Hello everyone".into(),
    }
}

#[test]
fn parse_mention_quoted_name_with_spaces() {
    let chars = vec![character("c1", "Maya Phoenix"), character("c2", "Sam")];
    let id = parse_mentions("@\"Maya Phoenix\", what do you think?", &chars);
    assert_eq!(id.as_deref(), Some("c1"));
}

#[test]
fn parse_mention_id_lowercase_match() {
    let chars = vec![character("Alice", "Alice")];
    let id = parse_mentions("@ALICE", &chars);
    assert_eq!(id.as_deref(), Some("Alice"));
}

#[test]
fn parse_mention_picks_first_when_multiple() {
    let chars = vec![character("a", "A"), character("b", "B")];
    let id = parse_mentions("@a says hi to @b", &chars);
    assert_eq!(id.as_deref(), Some("a"));
}

#[test]
fn parse_mention_no_at_sign_returns_none() {
    let chars = vec![character("alice", "Alice")];
    assert!(parse_mentions("hey alice", &chars).is_none());
}

#[test]
fn parse_mention_unknown_id_returns_none() {
    let chars = vec![character("alice", "Alice")];
    assert!(parse_mentions("@bob is missing", &chars).is_none());
}

#[test]
fn heuristic_never_returns_muted_speaker() {
    let chars = vec![
        character("alice", "Alice"),
        character("bob", "Bob"),
        character("carol", "Carol"),
    ];
    let context = ctx(
        chars,
        vec![
            participation("alice", 100, Some(99)),
            participation("bob", 0, None),
            participation("carol", 0, None),
        ],
        vec![assistant_message("alice", 99, "I keep talking")],
        &["bob", "carol"],
    );
    let result = heuristic_select_speaker(&context).expect("alice unmuted, must pick her");
    assert_eq!(result.character_id, "alice");
}

#[test]
fn heuristic_all_muted_is_error() {
    let chars = vec![character("alice", "Alice"), character("bob", "Bob")];
    let context = ctx(
        chars,
        vec![
            participation("alice", 0, None),
            participation("bob", 0, None),
        ],
        vec![],
        &["alice", "bob"],
    );
    let err = heuristic_select_speaker(&context).unwrap_err();
    assert!(err.to_lowercase().contains("muted"), "got: {err}");
}

#[test]
fn heuristic_favors_underrepresented_speaker() {
    let chars = vec![character("loud", "Loud"), character("quiet", "Quiet")];
    let context = ctx(
        chars,
        vec![
            participation("loud", 100, Some(99)),
            participation("quiet", 0, None),
        ],
        vec![assistant_message("loud", 99, "again")],
        &[],
    );
    let result = heuristic_select_speaker(&context).expect("ok");
    assert_eq!(
        result.character_id, "quiet",
        "imbalanced participation should boost the underrepresented speaker"
    );
}

#[test]
fn round_robin_advances_in_order() {
    let chars = vec![
        character("a", "A"),
        character("b", "B"),
        character("c", "C"),
    ];
    let stats = vec![
        participation("a", 1, Some(0)),
        participation("b", 0, None),
        participation("c", 0, None),
    ];
    let context = ctx(
        chars,
        stats,
        vec![assistant_message("a", 0, "starting")],
        &[],
    );
    let next = round_robin_select_speaker(&context).expect("ok");
    assert_eq!(
        next.character_id, "b",
        "after 'a' the round robin must pick 'b'"
    );
}

#[test]
fn round_robin_skips_muted() {
    let chars = vec![
        character("a", "A"),
        character("b", "B"),
        character("c", "C"),
    ];
    let stats = vec![
        participation("a", 1, Some(0)),
        participation("b", 0, None),
        participation("c", 0, None),
    ];
    let context = ctx(
        chars,
        stats,
        vec![assistant_message("a", 0, "spoke")],
        &["b"],
    );
    let next = round_robin_select_speaker(&context).expect("ok");
    assert_eq!(next.character_id, "c", "round robin must skip muted 'b'");
}

#[test]
fn round_robin_all_muted_errors() {
    let chars = vec![character("a", "A"), character("b", "B")];
    let context = ctx(chars, vec![], vec![], &["a", "b"]);
    let err = round_robin_select_speaker(&context).unwrap_err();
    assert!(err.to_lowercase().contains("muted"));
}

#[test]
fn tool_call_response_plain_object() {
    let r = parse_tool_call_response(r#"{"character_id": "alice"}"#).expect("ok");
    assert_eq!(r.character_id, "alice");
}

#[test]
fn tool_call_response_object_with_reasoning() {
    let r = parse_tool_call_response(r#"{"character_id": "bob", "reasoning": "he was asked"}"#)
        .expect("ok");
    assert_eq!(r.character_id, "bob");
    assert_eq!(r.reasoning.as_deref(), Some("he was asked"));
}

#[test]
fn tool_call_response_wrapped_with_stringified_arguments() {
    let raw = r#"{
      "tool_calls": [{
        "name": "select_next_speaker",
        "arguments": "{\"character_id\":\"carol\",\"reasoning\":\"her turn\"}"
      }]
    }"#;
    let r = parse_tool_call_response(raw).expect("must parse stringified args");
    assert_eq!(r.character_id, "carol");
}

#[test]
fn tool_call_response_wrapped_with_object_arguments() {
    let raw = r#"{
      "tool_calls": [{
        "name": "select_next_speaker",
        "arguments": {"character_id": "dave"}
      }]
    }"#;
    let r = parse_tool_call_response(raw).expect("must parse object args");
    assert_eq!(r.character_id, "dave");
}

#[test]
fn tool_call_response_extracts_from_surrounding_prose() {
    let raw = "I'll pick: {\"character_id\":\"eve\"} because she was mentioned.";
    let r = parse_tool_call_response(raw).expect("must extract JSON from prose");
    assert_eq!(r.character_id, "eve");
}

#[test]
fn tool_call_response_pure_text_returns_none() {
    assert!(parse_tool_call_response("alice should speak").is_none());
}

#[test]
fn tool_call_response_empty_string_returns_none() {
    assert!(parse_tool_call_response("").is_none());
}

#[test]
fn tool_call_response_object_missing_character_id_returns_none() {
    let r = parse_tool_call_response(r#"{"reasoning": "no id"}"#);
    assert!(r.is_none(), "must require character_id field");
}

#[test]
fn tool_call_response_malformed_json_returns_none() {
    assert!(parse_tool_call_response("{character_id: alice}").is_none());
}
