//! Character Selection Logic for Group Chats
//!
//! This module handles:
//! - @mention parsing to detect when user explicitly targets a character
//! - Building prompts for LLM-based character selection
//! - Tool definition for select_next_speaker
//! - Heuristic fallback selection when LLM is unavailable

use serde::{Deserialize, Serialize};

use super::{CharacterInfo, GroupChatContext};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionResult {
    pub character_id: String,
    pub reasoning: Option<String>,
}

fn prompt_quote(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| format!("{:?}", value))
}

fn prompt_field(prompt: &mut String, label: &str, value: &str) {
    prompt.push_str("- ");
    prompt.push_str(label);
    prompt.push_str(": ");
    prompt.push_str(&prompt_quote(value));
    prompt.push('\n');
}

// ============================================================================
// @Mention Parsing
// ============================================================================

/// Parse the user message for @mentions and return the character ID if found
///
/// Supports:
/// - @"Character Name" (quoted, for names with spaces)
/// - @CharacterName (unquoted, single word)
///
/// Returns the character ID if a valid mention is found, None otherwise.
pub fn parse_mentions(message: &str, characters: &[CharacterInfo]) -> Option<String> {
    // First try quoted mentions: @"Character Name"
    let mut i = 0;
    let chars: Vec<char> = message.chars().collect();

    while i < chars.len() {
        if chars[i] == '@' && i + 1 < chars.len() && chars[i + 1] == '"' {
            // Found @" - look for closing quote
            let start = i + 2;
            let mut end = start;
            while end < chars.len() && chars[end] != '"' {
                end += 1;
            }

            if end > start && end < chars.len() {
                let mentioned_name: String = chars[start..end].iter().collect();
                let mentioned_lower = mentioned_name.to_lowercase();

                // Find matching character (case-insensitive)
                for character in characters {
                    if character.name.to_lowercase() == mentioned_lower {
                        return Some(character.id.clone());
                    }
                }
            }
        }
        i += 1;
    }

    // Try unquoted mentions: @CharacterName (single word)
    for word in message.split_whitespace() {
        if word.starts_with('@') && word.len() > 1 {
            let mentioned = &word[1..];
            let mentioned_lower = mentioned.to_lowercase();

            // Exact match first
            for character in characters {
                if character.name.to_lowercase() == mentioned_lower {
                    return Some(character.id.clone());
                }
            }

            // Partial match (starts with)
            for character in characters {
                if character.name.to_lowercase().starts_with(&mentioned_lower) {
                    return Some(character.id.clone());
                }
            }
        }
    }

    None
}

// ============================================================================
// Selection Prompt Building
// ============================================================================

/// Build a prompt for LLM-based character selection
pub fn build_selection_prompt(context: &GroupChatContext) -> String {
    let mut prompt = String::new();
    let muted_ids: std::collections::HashSet<&str> = context
        .session
        .muted_character_ids
        .iter()
        .map(|s| s.as_str())
        .collect();
    let selectable_characters: Vec<&CharacterInfo> = context
        .characters
        .iter()
        .filter(|c| !muted_ids.contains(c.id.as_str()))
        .collect();
    if selectable_characters.is_empty() {
        prompt.push_str(
            "## Participants\n\nAll participants are currently muted for automatic selection.\n",
        );
        prompt.push_str("Only explicit @mentions should trigger a response.\n\n");
        return prompt;
    }

    prompt.push_str(
        "You are a narrator for a group chat. Your task is to select which character should respond next.\n\n",
    );

    // Participants section
    prompt.push_str("## Participants\n\n");

    let total_speaks: i32 = context
        .participation_stats
        .iter()
        .map(|p| p.speak_count)
        .sum();
    let current_turn = context
        .recent_messages
        .last()
        .map(|m| m.turn_number)
        .unwrap_or(0);

    for character in &selectable_characters {
        let stats = context
            .participation_stats
            .iter()
            .find(|p| p.character_id == character.id);

        let speak_count = stats.map(|s| s.speak_count).unwrap_or(0);
        let last_spoke = stats.and_then(|s| s.last_spoke_turn);
        let percentage = if total_speaks > 0 {
            (speak_count as f32 / total_speaks as f32 * 100.0).round() as i32
        } else {
            0
        };

        prompt.push_str("### Participant\n");
        prompt_field(&mut prompt, "Name", &character.name);
        prompt_field(&mut prompt, "ID", &character.id);

        // Include definition if available, otherwise fall back to personality_summary
        if let Some(desc) = character
            .definition
            .as_ref()
            .or(character.description.as_ref())
        {
            if !desc.is_empty() {
                prompt_field(&mut prompt, "Definition", desc);
            }
        }
        if let Some(summary) = &character.personality_summary {
            // Only add personality summary if it's different from definition (i.e., truncated)
            let is_truncated = character
                .definition
                .as_ref()
                .or(character.description.as_ref())
                .map(|d| d.len() > 200)
                .unwrap_or(false);
            if is_truncated {
                prompt_field(&mut prompt, "Personality Summary", summary);
            }
        }

        prompt.push_str(&format!(
            "- Participation: {} messages ({}%)\n",
            speak_count, percentage
        ));

        if let Some(last) = last_spoke {
            let turns_ago = current_turn - last;
            prompt.push_str(&format!("- Last spoke: {} turns ago\n", turns_ago));
        } else {
            prompt.push_str("- Last spoke: never\n");
        }

        prompt.push('\n');
    }

    // Recent conversation
    prompt.push_str("## Recent Conversation\n\n");

    for msg in context.recent_messages.iter().rev().take(10).rev() {
        let speaker = if msg.role == "user" {
            "User".to_string()
        } else if let Some(ref speaker_id) = msg.speaker_character_id {
            context
                .characters
                .iter()
                .find(|c| &c.id == speaker_id)
                .map(|c| c.name.clone())
                .unwrap_or_else(|| "Unknown".to_string())
        } else {
            "Unknown".to_string()
        };

        // Truncate long messages
        let content = if msg.content.len() > 200 {
            format!("{}...", &msg.content[..200])
        } else {
            msg.content.clone()
        };

        prompt.push_str("- Entry\n");
        prompt_field(&mut prompt, "Speaker", &speaker);
        prompt_field(&mut prompt, "Message", &content);
        prompt.push('\n');
    }

    // New user message
    prompt.push_str("\n## New Message from User\n\n");
    prompt.push_str(&prompt_quote(&context.user_message));
    prompt.push_str("\n\n");

    // Selection guidelines
    prompt.push_str("## Selection Guidelines\n\n");
    prompt.push_str("Consider the following when selecting who should respond:\n");
    prompt.push_str("1. **Relevance**: Who is the message directed at or about?\n");
    prompt.push_str("2. **Expertise**: Which character knows most about the topic?\n");
    prompt.push_str("3. **Balance**: Prefer characters who haven't spoken much recently\n");
    prompt.push_str("4. **Natural flow**: Who would naturally respond in this situation?\n");
    prompt.push_str(
        "5. **Allow exceptions**: Private conversations or urgent topics can override balance\n\n",
    );
    if !context.session.muted_character_ids.is_empty() {
        prompt.push_str(
            "6. **Muted participants**: Ignore muted participants for automatic selection.\n\n",
        );
    }

    prompt.push_str("Use the select_next_speaker tool to choose a character.");

    prompt
}

/// Build the tool definition for select_next_speaker
pub fn build_select_next_speaker_tool(characters: &[CharacterInfo]) -> serde_json::Value {
    let character_ids: Vec<&str> = characters.iter().map(|c| c.id.as_str()).collect();

    serde_json::json!({
        "type": "function",
        "function": {
            "name": "select_next_speaker",
            "description": "Select which character should speak next in the group chat. Consider participation balance, conversation relevance, and natural flow.",
            "parameters": {
                "type": "object",
                "properties": {
                    "character_id": {
                        "type": "string",
                        "description": "ID of the character who should respond",
                        "enum": character_ids
                    },
                    "reasoning": {
                        "type": "string",
                        "description": "Brief explanation of why this character should speak"
                    }
                },
                "required": ["character_id"]
            }
        }
    })
}

// ============================================================================
// Heuristic Selection (Fallback)
// ============================================================================

/// Select the next speaker using heuristics when LLM is unavailable
///
/// This uses a scoring system based on:
/// - Participation balance (favor underrepresented characters)
/// - Recency (soft penalty for speaking too recently)
pub fn heuristic_select_speaker(context: &GroupChatContext) -> Result<SelectionResult, String> {
    let muted_ids: std::collections::HashSet<&str> = context
        .session
        .muted_character_ids
        .iter()
        .map(|s| s.as_str())
        .collect();
    let selectable_characters: Vec<&CharacterInfo> = context
        .characters
        .iter()
        .filter(|c| !muted_ids.contains(c.id.as_str()))
        .collect();
    if selectable_characters.is_empty() {
        return Err("All participants are muted".to_string());
    }

    let total_messages: i32 = context
        .participation_stats
        .iter()
        .map(|p| p.speak_count)
        .sum();

    let current_turn = context
        .recent_messages
        .last()
        .map(|m| m.turn_number)
        .unwrap_or(0);

    // Score each character
    let mut scores: Vec<(String, f32, String)> = Vec::new();

    for character in &selectable_characters {
        let stats = context
            .participation_stats
            .iter()
            .find(|p| p.character_id == character.id);

        let speak_count = stats.map(|s| s.speak_count).unwrap_or(0);
        let last_spoke = stats.and_then(|s| s.last_spoke_turn);

        // Base score
        let mut score: f32 = 100.0;
        let mut reasons: Vec<String> = Vec::new();

        // Favor characters who have spoken less (participation balance)
        if total_messages > 0 {
            let participation_rate = speak_count as f32 / total_messages as f32;
            let expected_rate = 1.0 / selectable_characters.len() as f32;

            if participation_rate < expected_rate {
                // Under-represented, boost score
                let boost = (expected_rate - participation_rate) * 200.0;
                score += boost;
                reasons.push(format!("hasn't spoken much ({} messages)", speak_count));
            } else if participation_rate > expected_rate * 1.5 {
                // Over-represented, reduce score slightly
                score -= 20.0;
            }
        } else {
            // No messages yet, equal opportunity
            reasons.push("conversation just started".to_string());
        }

        // Consider recency (soft factor, not hard cooldown)
        if let Some(last) = last_spoke {
            let turns_ago = current_turn - last;
            if turns_ago == 0 {
                // Just spoke, reduce score but don't eliminate
                score -= 30.0;
            } else if turns_ago == 1 {
                score -= 15.0;
            } else if turns_ago >= 3 {
                score += 10.0;
                reasons.push(format!("hasn't spoken in {} turns", turns_ago));
            }
        } else {
            // Never spoken, boost
            score += 50.0;
            reasons.push("hasn't spoken yet".to_string());
        }

        let reasoning = if reasons.is_empty() {
            format!("{} selected to respond", character.name)
        } else {
            format!(
                "{} selected because: {}",
                character.name,
                reasons.join(", ")
            )
        };

        scores.push((character.id.clone(), score, reasoning));
    }

    // Sort by score descending
    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Pick the highest scoring character
    if let Some((character_id, _score, reasoning)) = scores.first() {
        Ok(SelectionResult {
            character_id: character_id.clone(),
            reasoning: Some(reasoning.clone()),
        })
    } else {
        // Fallback to first character if something went wrong
        let first = context
            .characters
            .first()
            .ok_or_else(|| "No characters in group".to_string())?;

        Ok(SelectionResult {
            character_id: first.id.clone(),
            reasoning: Some("Fallback selection".to_string()),
        })
    }
}

// ============================================================================
// Round-Robin Selection
// ============================================================================

/// Select the next speaker using simple round-robin ordering.
///
/// Picks the next character in the character_ids list after the last speaker.
/// If no one has spoken yet, picks the first character.
pub fn round_robin_select_speaker(context: &GroupChatContext) -> Result<SelectionResult, String> {
    let muted_ids: std::collections::HashSet<&str> = context
        .session
        .muted_character_ids
        .iter()
        .map(|s| s.as_str())
        .collect();
    let selectable_ids: Vec<String> = context
        .session
        .character_ids
        .iter()
        .filter(|id| !muted_ids.contains(id.as_str()))
        .cloned()
        .collect();
    if selectable_ids.is_empty() {
        return Err("All participants are muted".to_string());
    }
    let character_ids: &[String] = &selectable_ids;
    if character_ids.is_empty() {
        return Err("No characters in group".to_string());
    }

    // Find the last assistant message's speaker
    let last_speaker = context
        .recent_messages
        .iter()
        .rev()
        .find(|m| m.role == "assistant")
        .and_then(|m| m.speaker_character_id.as_ref());

    let next_id = if let Some(last_id) = last_speaker {
        // Find the index of the last speaker in character_ids
        if let Some(idx) = character_ids.iter().position(|id| id == last_id) {
            let next_idx = (idx + 1) % character_ids.len();
            character_ids[next_idx].clone()
        } else {
            character_ids[0].clone()
        }
    } else {
        character_ids[0].clone()
    };

    let name = context
        .characters
        .iter()
        .find(|c| c.id == next_id)
        .map(|c| c.name.clone())
        .unwrap_or_else(|| "Unknown".to_string());

    Ok(SelectionResult {
        character_id: next_id,
        reasoning: Some(format!("{} selected (round-robin)", name)),
    })
}

// ============================================================================
// Tool Call Parsing
// ============================================================================

/// Extract the selection result from an LLM tool call response (text format fallback)
pub fn parse_tool_call_response(response: &str) -> Option<SelectionResult> {
    // Try to parse JSON from the response
    // Handle cases where the response contains JSON embedded in text

    // Look for JSON object pattern
    if let Some(start) = response.find('{') {
        if let Some(end) = response.rfind('}') {
            let json_str = &response[start..=end];
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str) {
                // Try direct structure
                if let Some(character_id) = value.get("character_id").and_then(|v| v.as_str()) {
                    let reasoning = value
                        .get("reasoning")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    return Some(SelectionResult {
                        character_id: character_id.to_string(),
                        reasoning,
                    });
                }

                // Try wrapped in tool_calls
                if let Some(tool_calls) = value.get("tool_calls").and_then(|v| v.as_array()) {
                    for call in tool_calls {
                        if call.get("name").and_then(|v| v.as_str()) == Some("select_next_speaker")
                        {
                            if let Some(args) = call.get("arguments") {
                                let args_obj = if args.is_string() {
                                    serde_json::from_str(args.as_str().unwrap()).ok()
                                } else {
                                    Some(args.clone())
                                };

                                if let Some(args_val) = args_obj {
                                    if let Some(character_id) =
                                        args_val.get("character_id").and_then(|v| v.as_str())
                                    {
                                        let reasoning = args_val
                                            .get("reasoning")
                                            .and_then(|v| v.as_str())
                                            .map(String::from);
                                        return Some(SelectionResult {
                                            character_id: character_id.to_string(),
                                            reasoning,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage_manager::group_sessions::GroupSession;

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
    fn test_heuristic_ignores_muted_participants() {
        let context = test_context(vec!["char-1", "char-2"]);
        let result =
            heuristic_select_speaker(&context).expect("heuristic selection should succeed");
        assert_eq!(result.character_id, "char-3");
    }

    #[test]
    fn test_round_robin_ignores_muted_participants() {
        let mut context = test_context(vec!["char-2"]);
        context.recent_messages = vec![crate::storage_manager::group_sessions::GroupMessage {
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
            reasoning: None,
            selection_reasoning: None,
            model_id: None,
        }];
        let result =
            round_robin_select_speaker(&context).expect("round-robin selection should succeed");
        assert_eq!(result.character_id, "char-3");
    }

    #[test]
    fn test_heuristic_does_not_treat_plain_name_as_forced_selection() {
        let mut context = test_context(vec![]);
        context.user_message = "Alice should maybe answer this.".to_string();
        context.participation_stats = vec![
            crate::storage_manager::group_sessions::GroupParticipation {
                id: "p1".to_string(),
                session_id: "session-1".to_string(),
                character_id: "char-1".to_string(),
                speak_count: 10,
                last_spoke_turn: Some(4),
                last_spoke_at: Some(0),
            },
            crate::storage_manager::group_sessions::GroupParticipation {
                id: "p2".to_string(),
                session_id: "session-1".to_string(),
                character_id: "char-2".to_string(),
                speak_count: 0,
                last_spoke_turn: None,
                last_spoke_at: None,
            },
        ];
        context.recent_messages = vec![crate::storage_manager::group_sessions::GroupMessage {
            id: "m1".to_string(),
            session_id: "session-1".to_string(),
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
        }];

        let result =
            heuristic_select_speaker(&context).expect("heuristic selection should succeed");
        assert_eq!(result.character_id, "char-2");
    }
}
