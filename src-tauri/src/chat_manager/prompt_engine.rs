use blake3::Hasher;
use serde_json::{json, Value};
use tauri::AppHandle;

use super::lorebook_matcher::{format_lorebook_for_prompt, get_active_lorebook_entries};
use super::prompts;
use super::types::{
    Character, Model, Persona, PromptEntryPosition, PromptEntryRole, Session, Settings,
    SystemPromptEntry,
};
use crate::storage_manager::db::open_db;
use crate::storage_manager::lorebook::get_lorebook;

pub fn default_system_prompt_template() -> String {
    join_entries(&default_modular_prompt_entries())
}

pub fn default_dynamic_summary_prompt() -> String {
    join_entries(&default_dynamic_summary_entries())
}

pub fn default_dynamic_memory_prompt() -> String {
    join_entries(&default_dynamic_memory_entries())
}

pub fn default_help_me_reply_prompt() -> String {
    join_entries(&default_help_me_reply_entries())
}

pub fn default_help_me_reply_conversational_prompt() -> String {
    join_entries(&default_help_me_reply_conversational_entries())
}

pub fn default_group_chat_system_prompt_template() -> String {
    join_entries(&default_group_chat_entries())
}

pub fn default_group_chat_roleplay_prompt_template() -> String {
    join_entries(&default_group_chat_roleplay_entries())
}

fn join_entries(entries: &[SystemPromptEntry]) -> String {
    entries
        .iter()
        .map(|entry| entry.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn default_dynamic_summary_entries() -> Vec<SystemPromptEntry> {
    vec![
        SystemPromptEntry {
            id: "summary_task".to_string(),
            name: "Task".to_string(),
            role: PromptEntryRole::System,
            content:
                "You maintain a single, cumulative summary of the conversation.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "summary_inputs".to_string(),
            name: "Inputs".to_string(),
            role: PromptEntryRole::System,
            content: "You receive:\n- the previous global summary (if any)\n- the newest conversation window\n- Previous summary (if any): {{prev_summary}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "summary_job".to_string(),
            name: "Your Job".to_string(),
            role: PromptEntryRole::System,
            content: "Your job:\n1. Merge the new events into the existing summary.\n2. Preserve all factual past events unless they are contradicted.\n3. Keep the chronological flow clear and coherent.\n4. Remove redundant details or repetitions.\n5. DO NOT invent motivations, emotions, or events that were not explicitly stated.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "summary_guidelines".to_string(),
            name: "Guidelines".to_string(),
            role: PromptEntryRole::System,
            content: "Guidelines:\n- Capture actions, choices, facts, changes, and important context.\n- Keep the summary compact but complete.\n- If special placeholders exist ({{character}}, {{persona}}, etc.), keep them untouched.\n- The output must be a single cohesive summary paragraph, representing the entire conversation so far.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "summary_output".to_string(),
            name: "Output".to_string(),
            role: PromptEntryRole::System,
            content: "Output only the final merged summary with no commentary.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
    ]
}

pub fn default_dynamic_memory_entries() -> Vec<SystemPromptEntry> {
    vec![
        SystemPromptEntry {
            id: "memory_task".to_string(),
            name: "Task".to_string(),
            role: PromptEntryRole::System,
            content:
                "You manage long-term memories for this roleplay chat. Use tools to maintain an accurate, useful memory list.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "memory_budget".to_string(),
            name: "Token Budget".to_string(),
            role: PromptEntryRole::System,
            content: "IMPORTANT - TOKEN BUDGET:\nCurrent hot memory usage: {{current_memory_tokens}}/{{hot_token_budget}} tokens\nDeleted memories are NOT lost—they go to cold storage and can be recalled later via keyword search.\nMemories decay over time unless accessed or pinned.\n\nWhen OVER BUDGET: You MUST delete lower-priority memories to make room for new ones.\nWhen UNDER BUDGET: Only delete duplicates, contradicted facts, or truly obsolete information.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "memory_what".to_string(),
            name: "What To Remember".to_string(),
            role: PromptEntryRole::System,
            content: "Store facts that will matter for future conversations:\n- Character reveals: Traits, backstory, fears, goals (e.g., \"{{char}} revealed they fear abandonment\")\n- Relationship changes: Bonds formed, conflicts, trust levels (e.g., \"{{persona}} and {{char}} became allies\")\n- Plot milestones: Key decisions, events, world changes (e.g., \"The group chose to enter the forbidden forest\")\n- User preferences: Tone, boundaries, or explicit requests (e.g., \"{{persona}} prefers slower pacing\")".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "memory_rules".to_string(),
            name: "Rules".to_string(),
            role: PromptEntryRole::System,
            content: "Rules:\n- Keep each memory atomic: one fact per entry, under 100 characters when possible\n- Be factual: only store what was explicitly stated or clearly happened—never infer emotions or motivations\n- Avoid duplicates: check existing memories before adding; merge or skip if redundant\n- Respect the {{max_entries}} entry limit\n- When deleting, use the 6-digit memory ID shown in brackets (e.g., delete \"847291\")".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "memory_priority".to_string(),
            name: "Priority".to_string(),
            role: PromptEntryRole::System,
            content: "Priority (what to keep vs demote):\n1. PIN: Character-defining facts that should never be forgotten\n2. KEEP: Active plot threads and unresolved conflicts\n3. KEEP: Recent decisions with ongoing consequences\n4. DEMOTE: Resolved plot points, routine actions, outdated context".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "memory_tools".to_string(),
            name: "Tool Usage".to_string(),
            role: PromptEntryRole::System,
            content: "Tool Usage:\n- Use `create_memory` with `text` and optionally `important: true` to pin\n- Use `delete_memory` with the memory ID or exact text\n- Use `pin_memory` to mark existing memories as critical (never decay)\n- Use `unpin_memory` to allow a memory to decay normally\n- Call `done` when finished making changes\n- Output NO natural language, only tool calls".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
    ]
}

pub fn default_help_me_reply_entries() -> Vec<SystemPromptEntry> {
    vec![
        SystemPromptEntry {
            id: "reply_task".to_string(),
            name: "Task".to_string(),
            role: PromptEntryRole::System,
            content:
                "You are helping the user write their next message in this roleplay conversation.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "reply_character".to_string(),
            name: "Character You're Talking To".to_string(),
            role: PromptEntryRole::System,
            content: "# The Character You're Talking To\nName: {{char.name}}\n{{char.desc}}"
                .to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "reply_user".to_string(),
            name: "User Character".to_string(),
            role: PromptEntryRole::System,
            content: "# Your Character (The User)\nName: {{persona.name}}\n{{persona.desc}}"
                .to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "reply_guidelines".to_string(),
            name: "Guidelines".to_string(),
            role: PromptEntryRole::System,
            content: "Based on the conversation history, generate a response that {{persona.name}} would naturally say to {{char.name}}.\n\nGuidelines:\n- Write as {{persona.name}} in first-person perspective.\n- Match the tone and style of the conversation\n- Don't be overly formal or robotic\n- React appropriately to what {{char.name}} just said or did\n- Stay true to {{persona.name}}'s personality and background\n- Write a substantial response with appropriate length - don't limit yourself to short sentences\n- Include actions, thoughts, dialogue, or descriptions as appropriate for the roleplay style".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "reply_draft".to_string(),
            name: "Draft Handling".to_string(),
            role: PromptEntryRole::System,
            content: "{{#if current_draft}}\nThe user has started writing: \"{{current_draft}}\"\nContinue and expand on this thought naturally. Keep their original intent but make it flow better and add appropriate detail and length.\n{{else}}\nGenerate a fresh, detailed response based on the conversation context.\n{{/if}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "reply_output".to_string(),
            name: "Output".to_string(),
            role: PromptEntryRole::System,
            content: "Output ONLY the message text - no quotes, no \"{{persona.name}}:\", no roleplay formatting.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
    ]
}

pub fn default_help_me_reply_conversational_entries() -> Vec<SystemPromptEntry> {
    vec![
        SystemPromptEntry {
            id: "reply_conv_task".to_string(),
            name: "Task".to_string(),
            role: PromptEntryRole::System,
            content:
                "You are helping the user write their next message in this casual conversation.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "reply_conv_character".to_string(),
            name: "Person You're Talking To".to_string(),
            role: PromptEntryRole::System,
            content: "# The Person You're Talking To\nName: {{char.name}}\n{{char.desc}}"
                .to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "reply_conv_user".to_string(),
            name: "User Identity".to_string(),
            role: PromptEntryRole::System,
            content: "# Your Identity (The User)\nName: {{persona.name}}\n{{persona.desc}}"
                .to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "reply_conv_guidelines".to_string(),
            name: "Guidelines".to_string(),
            role: PromptEntryRole::System,
            content: "Based on the conversation history, generate a natural response that {{persona.name}} would say to {{char.name}}.\n\nGuidelines:\n- Write as {{persona.name}} using a conversational, natural tone\n- Match the casual style and energy of the conversation\n- Be authentic and genuine - avoid overly formal or robotic language\n- React naturally to what {{char.name}} just said\n- Stay true to {{persona.name}}'s personality while keeping it conversational\n- Write an appropriate length response - natural conversation flow is key\n- Focus on dialogue and natural reactions rather than detailed descriptions".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "reply_conv_draft".to_string(),
            name: "Draft Handling".to_string(),
            role: PromptEntryRole::System,
            content: "{{#if current_draft}}\nThe user has started writing: \"{{current_draft}}\"\nContinue and expand on this thought naturally, maintaining a conversational tone. Keep their original intent but make it flow better and feel more natural.\n{{else}}\nGenerate a fresh, natural response based on the conversation context.\n{{/if}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "reply_conv_output".to_string(),
            name: "Output".to_string(),
            role: PromptEntryRole::System,
            content: "Output ONLY the message text - no quotes, no \"{{persona.name}}:\", keep it conversational and direct.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
    ]
}

pub fn default_group_chat_entries() -> Vec<SystemPromptEntry> {
    vec![
        SystemPromptEntry {
            id: "group_task".to_string(),
            name: "Task".to_string(),
            role: PromptEntryRole::System,
            content: "You are {{char.name}}, engaging in a group conversation.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "group_character".to_string(),
            name: "Character".to_string(),
            role: PromptEntryRole::System,
            content: "# Your Character: {{char.name}}\n{{char.desc}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "group_user".to_string(),
            name: "User Character".to_string(),
            role: PromptEntryRole::System,
            content: "# User's Character: {{persona.name}}\n{{persona.desc}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "group_context".to_string(),
            name: "Group Chat Context".to_string(),
            role: PromptEntryRole::System,
            content: "# Group Chat Context\nYou are in a group chat with {{persona.name}} and the following characters:\n{{group_characters}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "group_summary".to_string(),
            name: "Context Summary".to_string(),
            role: PromptEntryRole::System,
            content: "# Context Summary\n{{context_summary}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "group_memories".to_string(),
            name: "Key Memories".to_string(),
            role: PromptEntryRole::System,
            content: "# Key Memories\n{{key_memories}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "group_format".to_string(),
            name: "Message Format".to_string(),
            role: PromptEntryRole::System,
            content: "# Message Format\nIn the conversation history, you will see:\n- Messages from {{persona.name}} formatted as: [{{persona.name}}]: their message\n- Messages from other characters formatted as: [Character Name]: their message\n- Your own previous messages appear without a name prefix\n\nWhen you see a message with someone else's name in brackets, that is NOT you speaking - it's another character in the group chat.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "group_guidelines".to_string(),
            name: "Response Guidelines".to_string(),
            role: PromptEntryRole::System,
            content: "# Response Guidelines\n- Respond naturally as {{char.name}}\n- Stay in character and maintain your unique voice\n- Be aware of other characters in the conversation\n- You may reference what other characters have said\n- Respond directly without prefixing your name\n- Address {{persona.name}} by name when speaking to them\n- Remember: messages prefixed with [Other Name] are from other characters, not you".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "group_rules".to_string(),
            name: "Content Rules".to_string(),
            role: PromptEntryRole::System,
            content: "{{content_rules}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
    ]
}

pub fn default_group_chat_roleplay_entries() -> Vec<SystemPromptEntry> {
    vec![
        SystemPromptEntry {
            id: "group_rp_task".to_string(),
            name: "Task".to_string(),
            role: PromptEntryRole::System,
            content:
                "You are {{char.name}}, participating in a collaborative roleplay scenario."
                    .to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "group_rp_character".to_string(),
            name: "Character".to_string(),
            role: PromptEntryRole::System,
            content: "# Your Character: {{char.name}}\n{{char.desc}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "group_rp_user".to_string(),
            name: "User Character".to_string(),
            role: PromptEntryRole::System,
            content: "# User's Character: {{persona.name}}\n{{persona.desc}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "group_rp_participants".to_string(),
            name: "Roleplay Participants".to_string(),
            role: PromptEntryRole::System,
            content: "# Roleplay Participants\nYou are roleplaying with {{persona.name}} and the following characters:\n{{group_characters}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "group_rp_scene".to_string(),
            name: "Starting Scene".to_string(),
            role: PromptEntryRole::System,
            content: "# Starting Scene\n{{scene}}\n\n{{scene_direction}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "group_rp_summary".to_string(),
            name: "Context Summary".to_string(),
            role: PromptEntryRole::System,
            content: "# Context Summary\n{{context_summary}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "group_rp_memories".to_string(),
            name: "Key Memories".to_string(),
            role: PromptEntryRole::System,
            content: "# Key Memories\n{{key_memories}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "group_rp_format".to_string(),
            name: "Message Format".to_string(),
            role: PromptEntryRole::System,
            content: "# Message Format\nIn the roleplay, you will see:\n- Actions and dialogue from {{persona.name}} formatted as: [{{persona.name}}]: their roleplay\n- Actions and dialogue from other characters formatted as: [Character Name]: their roleplay\n- Your own previous responses appear without a name prefix\n\nWhen you see a message with someone else's name in brackets, that is NOT you - it's another character in the roleplay.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "group_rp_guidelines".to_string(),
            name: "Roleplay Guidelines".to_string(),
            role: PromptEntryRole::System,
            content: "# Roleplay Guidelines\n- Write immersive, descriptive responses as {{char.name}}\n- Stay deeply in character and maintain your personality\n- Describe your character's actions, thoughts, and dialogue\n- React naturally to other characters' actions and words\n- You may reference what other characters have done or said\n- Respond directly without prefixing your character's name\n- Use present tense for actions and thoughts\n- Be creative and contribute to the collaborative story\n- Remember: messages prefixed with [Other Name] are from other characters, not you".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "group_rp_rules".to_string(),
            name: "Content Rules".to_string(),
            role: PromptEntryRole::System,
            content: "{{content_rules}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
    ]
}

/// Get lorebook content for the current conversation context
/// Scans recent messages and returns formatted lorebook entries
fn get_lorebook_content(
    app: &AppHandle,
    character_id: &str,
    session: &Session,
) -> Result<String, String> {
    let conn = open_db(app)?;

    // Get last 10 messages for keyword matching context
    let recent_messages: Vec<String> = session
        .messages
        .iter()
        .rev()
        .take(10)
        .rev()
        .map(|msg| msg.content.clone())
        .collect();

    super::super::utils::log_info(
        app,
        "lorebook",
        format!(
            "Checking lorebook for character={} with {} recent messages",
            character_id,
            recent_messages.len()
        ),
    );

    let active_entries = get_active_lorebook_entries(&conn, character_id, &recent_messages)?;

    if active_entries.is_empty() {
        super::super::utils::log_info(
            app,
            "lorebook",
            "No active lorebook entries (no keywords matched or none always-active)".to_string(),
        );
        return Ok(String::new());
    }

    let entry_titles: Vec<String> = active_entries
        .iter()
        .map(|e| {
            if e.title.is_empty() {
                format!("[{}]", &e.id[..6.min(e.id.len())])
            } else {
                e.title.clone()
            }
        })
        .collect();

    super::super::utils::log_info(
        app,
        "lorebook",
        format!(
            "Injecting {} active entries: {}",
            active_entries.len(),
            entry_titles.join(", ")
        ),
    );

    Ok(format_lorebook_for_prompt(&active_entries))
}

pub fn resolve_used_lorebook_entries(
    app: &AppHandle,
    character_id: &str,
    session: &Session,
    rendered_entries: &[SystemPromptEntry],
) -> Vec<String> {
    let conn = match open_db(app) {
        Ok(conn) => conn,
        Err(_) => return Vec::new(),
    };

    let recent_messages: Vec<String> = session
        .messages
        .iter()
        .rev()
        .take(10)
        .rev()
        .map(|msg| msg.content.clone())
        .collect();

    let active_entries = match get_active_lorebook_entries(&conn, character_id, &recent_messages) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };
    if active_entries.is_empty() {
        return Vec::new();
    }

    let mut used: Vec<String> = Vec::new();
    for entry in active_entries {
        let content = entry.content.trim();
        if content.is_empty() {
            continue;
        }

        let was_injected = rendered_entries
            .iter()
            .any(|prompt_entry| prompt_entry.content.contains(content));
        if !was_injected {
            continue;
        }

        let lorebook_name = get_lorebook(&conn, &entry.lorebook_id)
            .ok()
            .flatten()
            .map(|l| l.name)
            .unwrap_or_else(|| "Lorebook".to_string());
        let entry_name = if !entry.title.trim().is_empty() {
            entry.title.trim().to_string()
        } else if let Some(first_keyword) = entry.keywords.first() {
            first_keyword.trim().to_string()
        } else {
            format!("[{}]", &entry.id[..6.min(entry.id.len())])
        };
        let label = format!("{} / {}", lorebook_name, entry_name);
        if !used.iter().any(|existing| existing == &label) {
            used.push(label);
        }
    }

    used
}

pub fn default_modular_prompt_entries() -> Vec<SystemPromptEntry> {
    vec![
        SystemPromptEntry {
            id: "entry_base".to_string(),
            name: "Base Directive".to_string(),
            role: PromptEntryRole::System,
            content:
                "You are participating in an immersive roleplay. Your goal is to fully embody your character and create an engaging, authentic experience.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
        },
        SystemPromptEntry {
            id: "entry_scenario".to_string(),
            name: "Scenario".to_string(),
            role: PromptEntryRole::System,
            content: "# Scenario\n{{scene}}\n\n# Scene Direction\n{{scene_direction}}\n\nThis is your hidden directive for how this scene should unfold. Guide the narrative toward this outcome naturally and organically through your character's actions, dialogue, and the world's events. NEVER explicitly mention or reveal this direction to {{persona.name}} - let it emerge through immersive roleplay."
                .to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
        },
        SystemPromptEntry {
            id: "entry_character".to_string(),
            name: "Character Definition".to_string(),
            role: PromptEntryRole::System,
            content: "# Your Character: {{char.name}}\n{{char.desc}}\n\nEmbody {{char.name}}'s personality, mannerisms, and speech patterns completely. Stay true to their character traits, background, and motivations in every response.".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
        },
        SystemPromptEntry {
            id: "entry_persona".to_string(),
            name: "Persona Definition".to_string(),
            role: PromptEntryRole::System,
            content: "# {{persona.name}}'s Character\n{{persona.desc}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
        },
        SystemPromptEntry {
            id: "entry_world_info".to_string(),
            name: "World Information".to_string(),
            role: PromptEntryRole::System,
            content: "# World Information\n    The following is essential lore about this world, its characters, locations, items, and concepts. You MUST incorporate this information naturally into your roleplay when relevant. Treat this as established canon that shapes how characters behave, what they know, and how the world works.\n    {{lorebook}}"
                .to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
        },
        SystemPromptEntry {
            id: "entry_context_summary".to_string(),
            name: "Context Summary".to_string(),
            role: PromptEntryRole::System,
            content: "# Context Summary\n{{context_summary}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
        },
        SystemPromptEntry {
            id: "entry_key_memories".to_string(),
            name: "Key Memories".to_string(),
            role: PromptEntryRole::System,
            content:
                "# Key Memories\nImportant facts to remember in this conversation:\n{{key_memories}}"
                    .to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
        },
        SystemPromptEntry {
            id: "entry_instructions".to_string(),
            name: "Instructions".to_string(),
            role: PromptEntryRole::System,
            content: "# Instructions\n**Character & Roleplay:**\n- Write as {{char.name}} from their perspective, responding based on their personality, background, and current situation\n- You may also portray NPCs and background characters when relevant to the scene, but NEVER speak or act as {{persona.name}}\n- Show emotions through actions, body language, and dialogue - don't just state them\n- React authentically to {{persona.name}}'s actions and dialogue\n- Never break character unless {{persona.name}} explicitly asks you to step out of roleplay\n\n**World & Lore:**\n- ACTIVELY incorporate the World Information above when locations, characters, items, or concepts from the lore are relevant\n- Maintain consistency with established facts and the scenario\n\n**Pacing & Style:**\n- Keep responses concise and focused so {{persona.name}} can actively participate\n- Let scenes unfold naturally - avoid summarizing or rushing\n- Use vivid, sensory details for immersion\n- If you see [CONTINUE], continue exactly where you left off without restarting\n\n{{content_rules}}".to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
        },
    ]
}

fn single_entry_from_content(content: &str) -> Vec<SystemPromptEntry> {
    vec![SystemPromptEntry {
        id: "entry_system".to_string(),
        name: "System Prompt".to_string(),
        role: PromptEntryRole::System,
        content: content.to_string(),
        enabled: true,
        injection_position: PromptEntryPosition::Relative,
        injection_depth: 0,
        conditional_min_messages: None,
        interval_turns: None,
        system_prompt: true,
    }]
}

fn has_placeholder(entries: &[SystemPromptEntry], placeholder: &str) -> bool {
    entries
        .iter()
        .any(|entry| entry.content.contains(placeholder))
}

fn is_dynamic_memory_active(settings: &Settings, character: &Character) -> bool {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|a| a.dynamic_memory.as_ref())
        .map(|dm| dm.enabled)
        .unwrap_or(false)
        && character.memory_type.eq_ignore_ascii_case("dynamic")
}

/// character template > model template > app default template (from database)
pub fn build_system_prompt_entries(
    app: &AppHandle,
    character: &Character,
    model: &Model,
    persona: Option<&Persona>,
    session: &Session,
    settings: &Settings,
) -> Vec<SystemPromptEntry> {
    let mut debug_parts: Vec<Value> = Vec::new();
    let dynamic_memory_active = is_dynamic_memory_active(settings, character);

    let (
        base_content,
        base_entries,
        base_template_source,
        base_template_id,
        condense_prompt_entries,
    ) = if let Some(char_template_id) = &character.prompt_template_id {
        if let Ok(Some(template)) = prompts::get_template(app, char_template_id) {
            debug_parts.push(json!({
                "source": "character_template",
                "template_id": char_template_id
            }));
            (
                template.content,
                template.entries,
                "character_template",
                Some(char_template_id.clone()),
                template.condense_prompt_entries,
            )
        } else {
            debug_parts.push(json!({
                "source": "character_template_not_found",
                "template_id": char_template_id,
                "fallback": "app_default"
            }));
            get_app_default_template_content(app, settings, &mut debug_parts)
        }
    } else {
        get_app_default_template_content(app, settings, &mut debug_parts)
    };

    let base_entries = if base_entries.is_empty() && !base_content.trim().is_empty() {
        single_entry_from_content(&base_content)
    } else {
        base_entries
    };

    let mut rendered_entries: Vec<SystemPromptEntry> = Vec::new();
    for entry in base_entries.iter() {
        if !entry.enabled && !entry.system_prompt {
            continue;
        }
        let rendered =
            render_with_context(app, &entry.content, character, persona, session, settings);
        if rendered.trim().is_empty() {
            continue;
        }
        let mut output_entry = entry.clone();
        output_entry.content = rendered;
        rendered_entries.push(output_entry);
    }

    if dynamic_memory_active && !has_placeholder(&base_entries, "{{context_summary}}") {
        if let Some(summary) = &session.memory_summary {
            if !summary.trim().is_empty() {
                rendered_entries.push(SystemPromptEntry {
                    id: "entry_context_summary".to_string(),
                    name: "Context Summary".to_string(),
                    role: PromptEntryRole::System,
                    content: format!("# Context Summary\n{}", summary),
                    enabled: true,
                    injection_position: PromptEntryPosition::Relative,
                    injection_depth: 0,
                    conditional_min_messages: None,
                    interval_turns: None,
                    system_prompt: true,
                });
            }
        }
    }

    if !has_placeholder(&base_entries, "{{key_memories}}") {
        let has_memories = if dynamic_memory_active {
            !session.memory_embeddings.is_empty()
        } else {
            !session.memories.is_empty()
        };
        if has_memories {
            let mut content = String::from("# Key Memories\n");
            content.push_str("Important facts to remember in this conversation:\n");
            if dynamic_memory_active {
                for mem in &session.memory_embeddings {
                    content.push_str(&format!("- {}\n", mem.text));
                }
            } else {
                for memory in &session.memories {
                    content.push_str(&format!("- {}\n", memory));
                }
            }
            rendered_entries.push(SystemPromptEntry {
                id: "entry_key_memories".to_string(),
                name: "Key Memories".to_string(),
                role: PromptEntryRole::System,
                content: content.trim().to_string(),
                enabled: true,
                injection_position: PromptEntryPosition::Relative,
                injection_depth: 0,
                conditional_min_messages: None,
                interval_turns: None,
                system_prompt: true,
            });
        }
    }

    if !has_placeholder(&base_entries, "{{lorebook}}") {
        let lorebook_content = match get_lorebook_content(app, &character.id, session) {
            Ok(content) => content,
            Err(_) => String::new(),
        };
        if !lorebook_content.trim().is_empty() {
            rendered_entries.push(SystemPromptEntry {
                id: "entry_lorebook".to_string(),
                name: "World Information".to_string(),
                role: PromptEntryRole::System,
                content: format!("# World Information\n{}", lorebook_content.trim()),
                enabled: true,
                injection_position: PromptEntryPosition::Relative,
                injection_depth: 0,
                conditional_min_messages: None,
                interval_turns: None,
                system_prompt: true,
            });
        }
    }

    if condense_prompt_entries {
        rendered_entries = condense_entries_into_single_system_message(rendered_entries);
    }

    debug_parts.push(json!({
        "template_vars": build_debug_vars(character, persona, session, settings),
        "memories_count": session.memories.len(),
    }));

    let mut total_chars: usize = 0;
    let mut enabled_count: usize = 0;
    let mut system_count: usize = 0;
    let mut has_ozone = false;
    let mut has_no_ozone = false;
    let mut entry_summaries: Vec<Value> = Vec::new();
    let mut hasher = Hasher::new();

    for entry in rendered_entries.iter() {
        let content = &entry.content;
        total_chars += content.len();
        hasher.update(content.as_bytes());
        hasher.update(b"\n");

        if entry.enabled || entry.system_prompt {
            enabled_count += 1;
        }
        if entry.system_prompt {
            system_count += 1;
        }

        let lowered = content.to_ascii_lowercase();
        let entry_has_ozone = lowered.contains("ozone");
        let entry_has_no_ozone = lowered.contains("no ozone");
        if entry_has_ozone {
            has_ozone = true;
        }
        if entry_has_no_ozone {
            has_no_ozone = true;
        }

        let mut entry_hasher = Hasher::new();
        entry_hasher.update(content.as_bytes());
        let entry_hash = entry_hasher.finalize().to_hex().to_string();

        entry_summaries.push(json!({
            "id": entry.id,
            "name": entry.name,
            "role": entry.role,
            "enabled": entry.enabled,
            "system_prompt": entry.system_prompt,
            "injection_position": entry.injection_position,
            "content_len": content.len(),
            "content_hash": entry_hash,
            "contains_ozone": entry_has_ozone,
            "contains_no_ozone": entry_has_no_ozone,
        }));
    }

    let combined_hash = hasher.finalize().to_hex().to_string();

    super::super::utils::emit_debug(
        app,
        "system_prompt_built",
        json!({
            "debug": debug_parts,
            "system_prompt_debug": {
                "session_id": session.id,
                "character_id": character.id,
                "model_id": model.id,
                "base_template_source": base_template_source,
                "base_template_id": base_template_id,
                "condense_prompt_entries": condense_prompt_entries,
                "model_prompt_template_id": model.prompt_template_id,
                "character_prompt_template_id": character.prompt_template_id,
                "settings_prompt_template_id": settings.prompt_template_id,
                "entry_count": rendered_entries.len(),
                "enabled_entry_count": enabled_count,
                "system_entry_count": system_count,
                "total_chars": total_chars,
                "combined_hash": combined_hash,
                "contains_ozone": has_ozone,
                "contains_no_ozone": has_no_ozone,
                "entries": entry_summaries,
            }
        }),
    );

    super::super::utils::log_info(
        app,
        "prompt_engine",
        format!(
            "system_prompt_built session={} base_source={} base_id={:?} entries={} total_chars={} ozone={} no_ozone={}",
            session.id,
            base_template_source,
            base_template_id,
            rendered_entries.len(),
            total_chars,
            has_ozone,
            has_no_ozone
        ),
    );

    rendered_entries
}

/// Helper function to check character template, then fall back to app default
/// Helper function to get app default template content from database
fn get_app_default_template_content(
    app: &AppHandle,
    settings: &Settings,
    debug_parts: &mut Vec<Value>,
) -> (
    String,
    Vec<SystemPromptEntry>,
    &'static str,
    Option<String>,
    bool,
) {
    // Try settings.prompt_template_id first (user's custom app default)
    if let Some(app_template_id) = &settings.prompt_template_id {
        if let Ok(Some(template)) = prompts::get_template(app, app_template_id) {
            debug_parts.push(json!({
                "source": "app_wide_template",
                "template_id": app_template_id
            }));
            return (
                template.content,
                template.entries,
                "app_wide_template",
                Some(app_template_id.clone()),
                template.condense_prompt_entries,
            );
        }
    }

    match prompts::get_template(app, prompts::APP_DEFAULT_TEMPLATE_ID) {
        Ok(Some(template)) => {
            debug_parts.push(json!({
                "source": "app_default_template",
                "template_id": prompts::APP_DEFAULT_TEMPLATE_ID
            }));
            (
                template.content,
                template.entries,
                "app_default_template",
                Some(prompts::APP_DEFAULT_TEMPLATE_ID.to_string()),
                template.condense_prompt_entries,
            )
        }
        _ => {
            debug_parts.push(json!({
                "source": "emergency_hardcoded_fallback",
                "warning": "app_default template not found in database"
            }));
            let content = default_system_prompt_template();
            (
                content.clone(),
                default_modular_prompt_entries(),
                "emergency_hardcoded_fallback",
                None,
                false,
            )
        }
    }
}

fn condense_entries_into_single_system_message(
    entries: Vec<SystemPromptEntry>,
) -> Vec<SystemPromptEntry> {
    let merged = entries
        .into_iter()
        .filter_map(|entry| {
            let trimmed = entry.content.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    if merged.trim().is_empty() {
        return Vec::new();
    }

    vec![SystemPromptEntry {
        id: "entry_condensed_system".to_string(),
        name: "Condensed System Prompt".to_string(),
        role: PromptEntryRole::System,
        content: merged,
        enabled: true,
        injection_position: PromptEntryPosition::Relative,
        injection_depth: 0,
        conditional_min_messages: None,
        interval_turns: None,
        system_prompt: true,
    }]
}

/// Render a base template string with the provided context (character, persona, scene, settings).
pub fn render_with_context(
    app: &AppHandle,
    base_template: &str,
    character: &Character,
    persona: Option<&Persona>,
    session: &Session,
    settings: &Settings,
) -> String {
    render_with_context_internal(
        Some(app),
        base_template,
        character,
        persona,
        session,
        settings,
    )
}

fn render_with_context_internal(
    app: Option<&AppHandle>,
    base_template: &str,
    character: &Character,
    persona: Option<&Persona>,
    session: &Session,
    settings: &Settings,
) -> String {
    let char_name = &character.name;
    let raw_char_desc = character
        .definition
        .as_ref()
        .or(character.description.as_ref())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("");

    // Get persona info
    let persona_name = persona.map(|p| p.title.as_str()).unwrap_or("");
    let persona_desc = persona
        .map(|p| p.description.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("");

    let scene_id_to_use = session
        .selected_scene_id
        .as_ref()
        .or_else(|| character.default_scene_id.as_ref())
        .or_else(|| {
            if character.scenes.len() == 1 {
                character.scenes.first().map(|s| &s.id)
            } else {
                None
            }
        });

    let (scene_content, scene_direction) = if let Some(selected_scene_id) = scene_id_to_use {
        if let Some(scene) = character.scenes.iter().find(|s| &s.id == selected_scene_id) {
            let (content, direction) = if let Some(variant_id) = &scene.selected_variant_id {
                let variant = scene.variants.iter().find(|v| &v.id == variant_id);

                if let Some(v) = variant {
                    (v.content.as_str(), v.direction.as_deref())
                } else {
                    (scene.content.as_str(), scene.direction.as_deref())
                }
            } else {
                (scene.content.as_str(), scene.direction.as_deref())
            };

            let content_trimmed = content.trim();
            let direction_processed = if let Some(dir) = direction {
                let dir_trimmed = dir.trim();
                if !dir_trimmed.is_empty() {
                    let mut dir_processed = dir_trimmed.to_string();
                    dir_processed = dir_processed.replace("{{char}}", char_name);
                    dir_processed = dir_processed.replace("{{persona}}", persona_name);
                    dir_processed = dir_processed.replace("{{user}}", persona_name);
                    dir_processed
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            if !content_trimmed.is_empty() {
                // Replace {{char}} and {{persona}} placeholders dynamically in scene text
                let mut content_processed = content_trimmed.to_string();
                content_processed = content_processed.replace("{{char}}", char_name);
                content_processed = content_processed.replace("{{persona}}", persona_name);
                content_processed = content_processed.replace("{{user}}", persona_name);

                if let Some(app) = app {
                    super::super::utils::log_info(
                        app,
                        "prompt_engine",
                        format!(
                            "Scene found and processed. ID: {}, content length: {}, direction length: {}",
                            selected_scene_id,
                            content_processed.len(),
                            direction_processed.len()
                        ),
                    );
                }
                (content_processed, direction_processed)
            } else {
                if let Some(app) = app {
                    super::super::utils::log_warn(
                        app,
                        "prompt_engine",
                        format!(
                            "Scene found but content is empty. ID: {}",
                            selected_scene_id
                        ),
                    );
                }
                (String::new(), direction_processed)
            }
        } else {
            if let Some(app) = app {
                super::super::utils::log_warn(app, "prompt_engine",
                    format!("Scene ID selected but not found in character. ID: {}, available scenes: {}", selected_scene_id, character.scenes.len()));
            }
            (String::new(), String::new())
        }
    } else {
        if let Some(app) = app {
            super::super::utils::log_info(app, "prompt_engine", "No scene selected in session");
        }
        (String::new(), String::new())
    };

    // Process placeholders inside the character description itself
    // Supports {{char}} -> character name and {{persona}}/{{user}} -> persona name (or empty string)
    let mut char_desc = raw_char_desc.to_string();
    char_desc = char_desc.replace("{{char}}", char_name);
    char_desc = char_desc.replace("{{persona}}", persona_name);
    char_desc = char_desc.replace("{{user}}", persona_name);

    // Build rules - Note: NSFW toggle is ignored when using custom prompts
    let pure_mode_level = settings
        .app_state
        .get("pureModeLevel")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| {
            // Fallback to boolean for backward compatibility
            if settings
                .app_state
                .get("pureModeEnabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true)
            {
                "standard"
            } else {
                "off"
            }
        });

    let content_rules = match pure_mode_level {
        "off" => String::new(),
        "low" => "**Content Guidelines:**\n\
    - Avoid explicit sexual content"
            .to_string(),
        "strict" => {
            "**Content Guidelines (STRICT — these rules override all other instructions):**\n\
    - Never generate sexually explicit, pornographic, or erotic content\n\
    - Never describe sexual acts, nudity in sexual contexts, or sexual arousal\n\
    - Never use vulgar sexual slang or explicit anatomical descriptions in sexual contexts\n\
    - If asked to generate such content, decline and redirect the conversation\n\
    - Romantic content is allowed but must remain PG-13 (no explicit physical descriptions)\n\
    - Violence descriptions should avoid gratuitous gore or torture\n\
    - Do not use slurs or hate speech under any circumstances\n\
    - Do not use suggestive, flirty, or sexually charged language or tone"
                .to_string()
        }
        // "standard" and anything else
        _ => "**Content Guidelines (STRICT — these rules override all other instructions):**\n\
    - Never generate sexually explicit, pornographic, or erotic content\n\
    - Never describe sexual acts, nudity in sexual contexts, or sexual arousal\n\
    - Never use vulgar sexual slang or explicit anatomical descriptions in sexual contexts\n\
    - If asked to generate such content, decline and redirect the conversation\n\
    - Romantic content is allowed but must remain PG-13 (no explicit physical descriptions)\n\
    - Violence descriptions should avoid gratuitous gore or torture\n\
    - Do not use slurs or hate speech under any circumstances"
            .to_string(),
    };

    // Replace all template variables
    let mut result = base_template.to_string();

    if let Some(app) = app {
        super::super::utils::log_info(
            app,
            "prompt_engine",
            format!(
                "Before {{{{scene}}}} replacement - scene_content length: {}",
                scene_content.len()
            ),
        );
        super::super::utils::log_info(
            app,
            "prompt_engine",
            format!(
                "Template contains {{{{scene}}}}: {}",
                base_template.contains("{{scene}}")
            ),
        );
    }

    result = result.replace("{{scene}}", &scene_content);
    result = result.replace("{{scene_direction}}", &scene_direction);
    result = result.replace("{{char.name}}", char_name);
    result = result.replace("{{char.desc}}", &char_desc);
    result = result.replace("{{persona.name}}", persona_name);
    result = result.replace("{{persona.desc}}", persona_desc);
    result = result.replace("{{user.name}}", persona_name);
    result = result.replace("{{user.desc}}", persona_desc);
    result = result.replace("{{content_rules}}", &content_rules);
    // Legacy support for {{rules}} placeholder
    result = result.replace("{{rules}}", "");

    let dynamic_memory_active = is_dynamic_memory_active(settings, character);
    if dynamic_memory_active {
        let context_summary_text = session
            .memory_summary
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("");
        result = result.replace("{{context_summary}}", context_summary_text);
    } else {
        result = result.replace("# Context Summary\n    {{context_summary}}", "");
        result = result.replace("# Context Summary\n{{context_summary}}", "");
        result = result.replace("{{context_summary}}", "");
    }

    let key_memories_text = if dynamic_memory_active && !session.memory_embeddings.is_empty() {
        session
            .memory_embeddings
            .iter()
            .map(|m| format!("- {}", m.text))
            .collect::<Vec<_>>()
            .join("\n")
    } else if session.memories.is_empty() {
        String::new()
    } else {
        session
            .memories
            .iter()
            .map(|m| format!("- {}", m))
            .collect::<Vec<_>>()
            .join("\n")
    };

    result = result.replace("{{key_memories}}", &key_memories_text);

    // Lorebook entries - get recent messages for keyword matching
    let lorebook_text = if let Some(app) = app {
        match get_lorebook_content(app, &character.id, session) {
            Ok(content) => content,
            Err(e) => {
                super::super::utils::log_warn(
                    app,
                    "prompt_engine",
                    format!("Failed to get lorebook content: {}", e),
                );
                String::new()
            }
        }
    } else {
        String::new()
    };

    let lorebook_text = if lorebook_text.trim().is_empty() && session.id == "preview" {
        "**The Sunken City of Eldara** (Sample Entry)\nAn ancient city beneath the waves, Eldara was once the capital of a great empire. Its ruins are said to contain powerful artifacts and are guarded by merfolk descendants of its original inhabitants.\n\n**Dragonstone Keep** (Sample Entry)\nA fortress built into the side of Mount Ember, known for its impenetrable walls forged from volcanic glass. The keep is ruled by House Valthor, who claim ancestry from the first dragon riders.".to_string()
    } else {
        lorebook_text
    };

    if lorebook_text.trim().is_empty() {
        result = result.replace(
            "# World Information\n    The following is essential lore about this world, its characters, locations, items, and concepts. You MUST incorporate this information naturally into your roleplay when relevant. Treat this as established canon that shapes how characters behave, what they know, and how the world works.\n    {{lorebook}}",
            ""
        );
        result = result.replace("# World Information\n    {{lorebook}}", "");
        result = result.replace("# World Information\n{{lorebook}}", "");
        result = result.replace("{{lorebook}}", "");
    } else {
        result = result.replace("{{lorebook}}", &lorebook_text);
    }

    result = result.replace("{{char}}", char_name);
    result = result.replace("{{persona}}", persona_name);
    result = result.replace("{{user}}", persona_name);
    result = result.replace("{{ai_name}}", char_name);
    result = result.replace("{{ai_description}}", &char_desc);
    result = result.replace("{{ai_rules}}", "");
    result = result.replace("{{persona_name}}", persona_name);
    result = result.replace("{{persona_description}}", persona_desc);
    result = result.replace("{{user_name}}", persona_name);
    result = result.replace("{{user_description}}", persona_desc);

    result
}

fn build_debug_vars(
    character: &Character,
    persona: Option<&Persona>,
    session: &Session,
    _settings: &Settings,
) -> Value {
    let char_name = &character.name;
    let persona_name = persona.map(|p| p.title.as_str()).unwrap_or("");
    let raw_char_desc = character
        .definition
        .as_ref()
        .or(character.description.as_ref())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("")
        .replace("{{char}}", char_name)
        .replace("{{persona}}", persona_name)
        .replace("{{user}}", persona_name);
    json!({
        "char_name": char_name,
        "char_desc": raw_char_desc,
        "persona_name": persona_name,
        "persona_desc": persona.map(|p| p.description.trim()).unwrap_or("") ,
        "scene_present": session.selected_scene_id.is_some(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat_manager::types::{Scene, SceneVariant};

    fn make_character() -> Character {
        Character {
            id: "c1".into(),
            name: "Alice".into(),
            avatar_path: None,
            background_image_path: None,
            description: Some("I am {{char}}. Partner: {{persona}}.".into()),
            definition: Some("I am {{char}}. Partner: {{persona}}.".into()),
            rules: vec![],
            scenes: vec![],
            default_scene_id: None,
            default_model_id: None,
            fallback_model_id: None,
            memory_type: "manual".into(),
            prompt_template_id: None,
            system_prompt: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    fn make_settings() -> Settings {
        Settings {
            default_provider_credential_id: None,
            default_model_id: None,
            provider_credentials: vec![],
            models: vec![],
            app_state: serde_json::json!({}),
            advanced_model_settings: super::super::types::AdvancedModelSettings::default(),
            prompt_template_id: None,
            system_prompt: None,
            migration_version: 0,
            advanced_settings: None,
        }
    }

    fn make_model() -> Model {
        Model {
            id: "m1".into(),
            name: "gpt-test".into(),
            provider_id: "openai".into(),
            provider_credential_id: None,
            provider_label: "openai".into(),
            display_name: "GPT Test".into(),
            created_at: 0,
            input_scopes: vec!["text".into()],
            output_scopes: vec!["text".into()],
            advanced_model_settings: None,
            prompt_template_id: None,
            voice_config: None,
            system_prompt: None,
        }
    }

    fn make_session() -> Session {
        Session {
            id: "s1".into(),
            character_id: "c1".into(),
            title: "t".into(),
            system_prompt: None,
            selected_scene_id: None,
            persona_id: None,
            persona_disabled: false,
            voice_autoplay: None,
            advanced_model_settings: None,
            memories: vec![],
            memory_summary: None,
            memory_summary_token_count: 0,
            memory_tool_events: vec![],
            messages: vec![],
            archived: false,
            created_at: 0,
            updated_at: 0,
            memory_embeddings: vec![],
            memory_status: None,
            memory_error: None,
        }
    }

    #[test]
    fn renders_simple_placeholders() {
        let character = make_character();
        let _model = make_model();
        let settings = make_settings();
        let session = make_session();
        let persona = Some(Persona {
            id: "p1".into(),
            title: "Bob".into(),
            description: "Persona Bob".into(),
            is_default: true,
            created_at: 0,
            updated_at: 0,
        });

        let base = "Hello {{char}} and {{persona}}. {{char.desc}}";
        let rendered = render_with_context_internal(
            None,
            base,
            &character,
            persona.as_ref(),
            &session,
            &settings,
        );
        assert!(rendered.contains("Hello Alice and Bob."));
        assert!(rendered.contains("I am Alice. Partner: Bob."));

        // Scene injection test
        // Add a scene and make sure {{scene}} replacement works
        let mut session2 = session.clone();
        let mut character2 = character.clone();
        character2.scenes = vec![Scene {
            id: "scene1".into(),
            content: "Meeting {{char}} and {{persona}}".into(),
            created_at: 0,
            direction: None,
            variants: vec![SceneVariant {
                id: "v1".into(),
                content: "Var {{char}}".into(),
                created_at: 0,
                direction: None,
            }],
            selected_variant_id: Some("v1".into()),
        }];
        session2.selected_scene_id = Some("scene1".into());
        let base2 = "{{scene}}";
        let rendered2 = render_with_context_internal(
            None,
            base2,
            &character2,
            persona.as_ref(),
            &session2,
            &settings,
        );
        assert!(rendered2.contains("Var Alice"));
        assert!(!rendered2.contains("Starting Scene")); // No hardcoded formatting
    }
}
