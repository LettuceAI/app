use crate::chat_manager::types::{PromptEntryPosition, PromptEntryRole, SystemPromptEntry};

pub fn prompt_entry(
    id: impl Into<String>,
    name: impl Into<String>,
    role: PromptEntryRole,
    content: impl Into<String>,
    position: PromptEntryPosition,
    depth: u32,
) -> SystemPromptEntry {
    let system_prompt = matches!(role, PromptEntryRole::System);
    SystemPromptEntry {
        id: id.into(),
        name: name.into(),
        role,
        content: content.into(),
        enabled: true,
        injection_position: position,
        injection_depth: depth,
        conditional_min_messages: None,
        interval_turns: None,
        system_prompt,
        conditions: None,
        prompt_entry_payload: None,
    }
}

pub fn relative_system_entry(
    id: impl Into<String>,
    name: impl Into<String>,
    content: impl Into<String>,
) -> SystemPromptEntry {
    prompt_entry(
        id,
        name,
        PromptEntryRole::System,
        content,
        PromptEntryPosition::Relative,
        0,
    )
}

pub fn in_chat_system_entry(
    id: impl Into<String>,
    name: impl Into<String>,
    content: impl Into<String>,
    depth: u32,
) -> SystemPromptEntry {
    prompt_entry(
        id,
        name,
        PromptEntryRole::System,
        content,
        PromptEntryPosition::InChat,
        depth,
    )
}

pub fn in_chat_user_entry(
    id: impl Into<String>,
    name: impl Into<String>,
    content: impl Into<String>,
    depth: u32,
) -> SystemPromptEntry {
    prompt_entry(
        id,
        name,
        PromptEntryRole::User,
        content,
        PromptEntryPosition::InChat,
        depth,
    )
}

pub fn swap_places_entry(character_name: &str, persona_name: &str) -> SystemPromptEntry {
    relative_system_entry(
        "runtime_swap_places",
        "Swap Places",
        format!(
            "Swap places mode is active for this turn. The human is speaking as character '{}' and you must respond as persona '{}'. Keep the response in first person as '{}'.",
            character_name, persona_name, persona_name
        ),
    )
}

pub fn relevant_memories_entry(content: impl Into<String>) -> SystemPromptEntry {
    in_chat_system_entry(
        "runtime_retrieved_memories",
        "Retrieved Memories",
        format!("Relevant memories:\n{}", content.into()),
        0,
    )
}

pub fn regenerate_guidance_entry(guidance: &str) -> SystemPromptEntry {
    in_chat_user_entry(
        "runtime_regenerate_instruction",
        "Regenerate Instruction",
        format!(
            "[REGENERATE INSTRUCTION]\nRegenerate your previous response to the last message. Follow this additional user instruction for the new response:\n{}",
            guidance
        ),
        0,
    )
}

pub fn continue_instruction_entry() -> SystemPromptEntry {
    in_chat_user_entry(
        "runtime_continue_instruction",
        "Continue Instruction",
        "[CONTINUE] You were in the middle of a response. Continue writing from exactly where you left off. Do NOT restart, regenerate, or rewrite what you already said. Simply pick up the narrative thread and continue the scene forward with new content.",
        0,
    )
}
