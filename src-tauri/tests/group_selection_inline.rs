//! Gathered from inline tests in src/group_chat_manager/selection.rs.

use lettuceai_lib::group_chat_manager::selection::*;
use lettuceai_lib::group_chat_manager::{CharacterInfo, GroupChatContext};
use lettuceai_lib::storage_manager::group_sessions::GroupSession;

fn test_characters() -> Vec<CharacterInfo> {
    vec![
        CharacterInfo {
            id: "char-1".to_string(),
            name: "Alice".to_string(),
            definition: Some("A friendly AI assistant".to_string()),
            description: Some("A friendly AI assistant".to_string()),
            personality_summary: Some("Friendly and helpful".to_string()),
            memory_type: "manual".to_string(),
        },
        CharacterInfo {
            id: "char-2".to_string(),
            name: "Bob Smith".to_string(),
            definition: Some("A technical expert".to_string()),
            description: Some("A technical expert".to_string()),
            personality_summary: Some("Technical and precise".to_string()),
            memory_type: "manual".to_string(),
        },
        CharacterInfo {
            id: "char-3".to_string(),
            name: "Charlie".to_string(),
            definition: None,
            description: None,
            personality_summary: None,
            memory_type: "manual".to_string(),
        },
    ]
}

fn test_context(muted_character_ids: Vec<&str>) -> GroupChatContext {
    GroupChatContext {
        session: GroupSession {
            id: "session-1".to_string(),
            author_note: None,
            memory_progress_step: None,
            name: "Test".to_string(),
            memory_type: "manual".to_string(),
            character_ids: vec![
                "char-1".to_string(),
                "char-2".to_string(),
                "char-3".to_string(),
            ],
            group_character_id: None,
            muted_character_ids: muted_character_ids
                .into_iter()
                .map(|s| s.to_string())
                .collect(),
            persona_id: None,
            created_at: 0,
            updated_at: 0,
            archived: false,
            chat_type: "conversation".to_string(),
            starting_scene: None,
            background_image_path: None,
            lorebook_ids: vec![],
            disable_character_lorebooks: false,
            memories: vec![],
            memory_embeddings: vec![],
            memory_summary: String::new(),
            memory_summary_token_count: 0,
            memory_tool_events: vec![],
            memory_status: "idle".to_string(),
            memory_error: None,
            speaker_selection_method: "heuristic".to_string(),
        },
        characters: test_characters(),
        participation_stats: vec![],
        recent_messages: vec![],
        user_message: "hello".to_string(),
    }
}

#[test]
fn test_parse_mentions_unquoted() {
    let characters = test_characters();
    let result = parse_mentions("Hey @Alice, how are you?", &characters);
    assert_eq!(result, Some("char-1".to_string()));
}

#[test]
fn test_parse_mentions_quoted() {
    let characters = test_characters();
    let result = parse_mentions("@\"Bob Smith\" can you help?", &characters);
    assert_eq!(result, Some("char-2".to_string()));
}

#[test]
fn test_parse_mentions_case_insensitive() {
    let characters = test_characters();
    let result = parse_mentions("@ALICE hello", &characters);
    assert_eq!(result, Some("char-1".to_string()));
}

#[test]
fn test_parse_mentions_no_match() {
    let characters = test_characters();
    let result = parse_mentions("Hello everyone!", &characters);
    assert_eq!(result, None);
}

#[test]
fn test_parse_mentions_unknown_character() {
    let characters = test_characters();
    let result = parse_mentions("@Unknown help me", &characters);
    assert_eq!(result, None);
}

#[test]
fn test_parse_mentions_first_match_wins() {
    let characters = test_characters();
    let result = parse_mentions("@Alice and @Charlie", &characters);
    assert_eq!(result, Some("char-1".to_string()));
}

#[test]
fn test_parse_tool_call_response() {
    let response = r#"{"character_id": "char-1", "reasoning": "Alice is best suited"}"#;
    let result = parse_tool_call_response(response);
    assert!(result.is_some());
    let selection = result.unwrap();
    assert_eq!(selection.character_id, "char-1");
    assert_eq!(
        selection.reasoning,
        Some("Alice is best suited".to_string())
    );
}

#[test]
fn test_build_selection_prompt_escapes_quotes() {
    let mut context = test_context(vec![]);
    context.characters[0].name = r#"Alice "Quoted""#.to_string();
    context.characters[0].definition = Some(r#"Says "hello" often"#.to_string());
    context.user_message = r#"Tell "Alice" to respond"#.to_string();

    let prompt = build_selection_prompt(&context);

    assert!(prompt.contains(r#"- Name: "Alice \"Quoted\"""#));
    assert!(prompt.contains(r#"- Definition: "Says \"hello\" often""#));
    assert!(prompt.contains(r#""Tell \"Alice\" to respond""#));
}

#[test]
fn test_build_selection_prompt_truncates_utf8_safely() {
    let mut context = test_context(vec![]);
    let mut content = "a".repeat(510);
    content.push('—');
    content.push_str("tail");
    context.recent_messages = vec![
        lettuceai_lib::storage_manager::group_sessions::GroupMessage {
            id: "m1".to_string(),
            session_id: "session-1".to_string(),
            role: "assistant".to_string(),
            content,
            speaker_character_id: Some("char-1".to_string()),
            turn_number: 1,
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
        },
    ];

    let prompt = build_selection_prompt(&context);
    assert!(prompt.contains(&format!("{}—t…", "a".repeat(510))));
    assert!(!prompt.contains("tail"));
}

#[test]
fn test_heuristic_ignores_muted_participants() {
    let context = test_context(vec!["char-1", "char-2"]);
    let result = heuristic_select_speaker(&context).expect("heuristic selection should succeed");
    assert_eq!(result.character_id, "char-3");
}

#[test]
fn test_round_robin_ignores_muted_participants() {
    let mut context = test_context(vec!["char-2"]);
    context.recent_messages = vec![
        lettuceai_lib::storage_manager::group_sessions::GroupMessage {
            id: "m1".to_string(),
            session_id: "session-1".to_string(),
            role: "assistant".to_string(),
            content: "hey".to_string(),
            speaker_character_id: Some("char-1".to_string()),
            turn_number: 1,
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
        },
    ];
    let result =
        round_robin_select_speaker(&context).expect("round-robin selection should succeed");
    assert_eq!(result.character_id, "char-3");
}

#[test]
fn test_heuristic_does_not_treat_plain_name_as_forced_selection() {
    let mut context = test_context(vec![]);
    context.user_message = "Alice should maybe answer this.".to_string();
    context.participation_stats = vec![
        lettuceai_lib::storage_manager::group_sessions::GroupParticipation {
            id: "p1".to_string(),
            session_id: "session-1".to_string(),
            character_id: "char-1".to_string(),
            speak_count: 10,
            last_spoke_turn: Some(4),
            last_spoke_at: Some(0),
        },
        lettuceai_lib::storage_manager::group_sessions::GroupParticipation {
            id: "p2".to_string(),
            session_id: "session-1".to_string(),
            character_id: "char-2".to_string(),
            speak_count: 0,
            last_spoke_turn: None,
            last_spoke_at: None,
        },
    ];
    context.recent_messages = vec![
        lettuceai_lib::storage_manager::group_sessions::GroupMessage {
            id: "m1".to_string(),
            session_id: "session-1".to_string(),
            memory_refs: vec![],
            role: "assistant".to_string(),
            content: "Previous reply".to_string(),
            speaker_character_id: Some("char-1".to_string()),
            turn_number: 4,
            created_at: 0,
            usage: None,
            variants: None,
            selected_variant_id: None,
            is_pinned: false,
            attachments: vec![],
            used_lorebook_entries: vec![],
            reasoning: None,
            selection_reasoning: None,
            model_id: None,
        },
    ];

    let result = heuristic_select_speaker(&context).expect("heuristic selection should succeed");
    assert_eq!(result.character_id, "char-2");
}
