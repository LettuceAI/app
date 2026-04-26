use serde_json::{json, Value};

use crate::chat_manager::types::{
    Character, Persona, PromptEntryPosition, Settings, StoredMessage, SystemPromptEntry,
};

pub fn is_dynamic_memory_active(settings: &Settings, session_character: &Character) -> bool {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|a| a.dynamic_memory.as_ref())
        .map(|dm| dm.enabled)
        .unwrap_or(false)
        && session_character
            .memory_type
            .eq_ignore_ascii_case("dynamic")
}

pub fn append_image_directive_instructions(
    system_prompt_entries: Vec<SystemPromptEntry>,
    settings: &Settings,
) -> Vec<SystemPromptEntry> {
    let scene_generation_enabled = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.scene_generation_enabled)
        .unwrap_or(true);

    if !scene_generation_enabled {
        return system_prompt_entries
            .into_iter()
            .filter(|entry| entry.id != "entry_scene_image_protocol")
            .collect();
    }

    system_prompt_entries
}

fn prompt_entry_to_message(system_role: &str, entry: &SystemPromptEntry) -> Value {
    let role = match entry.role {
        crate::chat_manager::types::PromptEntryRole::System => system_role,
        crate::chat_manager::types::PromptEntryRole::User => "user",
        crate::chat_manager::types::PromptEntryRole::Assistant => "assistant",
    };
    json!({ "role": role, "content": entry.content })
}

pub fn partition_prompt_entries(
    entries: Vec<SystemPromptEntry>,
) -> (Vec<SystemPromptEntry>, Vec<SystemPromptEntry>) {
    let mut relative = Vec::new();
    let mut in_chat = Vec::new();
    for entry in entries {
        match entry.injection_position {
            PromptEntryPosition::Relative => relative.push(entry),
            PromptEntryPosition::InChat
            | PromptEntryPosition::Conditional
            | PromptEntryPosition::Interval => in_chat.push(entry),
        }
    }
    (relative, in_chat)
}

pub(crate) fn should_insert_in_chat_prompt_entry(
    entry: &SystemPromptEntry,
    turn_count: usize,
) -> bool {
    match entry.injection_position {
        PromptEntryPosition::InChat => true,
        PromptEntryPosition::Conditional => {
            let min_messages = entry.conditional_min_messages.unwrap_or(1) as usize;
            turn_count >= min_messages
        }
        PromptEntryPosition::Interval => {
            let interval = entry.interval_turns.unwrap_or(0) as usize;
            interval > 0 && turn_count > 0 && turn_count.is_multiple_of(interval)
        }
        PromptEntryPosition::Relative => false,
    }
}

pub fn insert_in_chat_prompt_entries(
    messages: &mut Vec<Value>,
    system_role: &str,
    entries: &[SystemPromptEntry],
) {
    if entries.is_empty() {
        return;
    }
    let base_len = messages.len();
    let turn_count = base_len;
    let mut inserts: Vec<(usize, usize, &SystemPromptEntry)> = entries
        .iter()
        .enumerate()
        .filter_map(|(idx, entry)| {
            if !should_insert_in_chat_prompt_entry(entry, turn_count) {
                return None;
            }
            let depth = entry.injection_depth as usize;
            let pos = base_len.saturating_sub(depth);
            Some((pos, idx, entry))
        })
        .collect();
    inserts.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let mut offset = 0usize;
    for (pos, _, entry) in inserts {
        if entry.content.trim().is_empty() {
            continue;
        }
        let insert_at = pos.saturating_add(offset).min(messages.len());
        messages.insert(insert_at, prompt_entry_to_message(system_role, entry));
        offset += 1;
    }
}

pub fn manual_window_size(settings: &Settings) -> usize {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|adv| adv.manual_mode_context_window)
        .map(|n| n as usize)
        .unwrap_or(50)
}

pub fn conversation_window_with_pinned(
    messages: &[StoredMessage],
    limit: usize,
) -> (Vec<StoredMessage>, Vec<StoredMessage>) {
    let pinned: Vec<StoredMessage> = messages.iter().filter(|m| m.is_pinned).cloned().collect();
    let recent_non_pinned: Vec<StoredMessage> = messages
        .iter()
        .rev()
        .filter(|m| {
            !m.is_pinned && (m.role == "user" || m.role == "assistant" || m.role == "scene")
        })
        .take(limit)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    (pinned, recent_non_pinned)
}

pub fn build_enriched_query(messages: &[StoredMessage]) -> String {
    let recent: Vec<&StoredMessage> = messages
        .iter()
        .filter(|m| m.role == "user" || m.role == "assistant")
        .collect();

    match recent.len() {
        0 => String::new(),
        1 => recent[0].content.clone(),
        _ => {
            let last = &recent[recent.len() - 1];
            let second_last = &recent[recent.len() - 2];
            format!("{}\n{}", second_last.content, last.content)
        }
    }
}

pub fn role_swap_enabled(flag: Option<bool>) -> bool {
    flag.unwrap_or(false)
}

pub(crate) fn swap_role_for_api(role: &str) -> &str {
    match role {
        "user" => "assistant",
        "assistant" => "user",
        other => other,
    }
}

pub fn maybe_swap_message_for_api(message: &StoredMessage, swap_places: bool) -> StoredMessage {
    if !swap_places {
        return message.clone();
    }
    let mut swapped = message.clone();
    swapped.role = swap_role_for_api(message.role.as_str()).to_string();
    swapped
}

pub fn swapped_prompt_entities(
    character: &Character,
    persona: Option<&Persona>,
) -> (Character, Option<Persona>) {
    let Some(persona) = persona else {
        return (character.clone(), None);
    };

    let mut swapped_character = character.clone();
    swapped_character.name = persona.title.clone();
    swapped_character.definition = Some(persona.description.clone());
    swapped_character.description = Some(persona.description.clone());

    let mut swapped_persona = persona.clone();
    swapped_persona.title = character.name.clone();
    swapped_persona.description = character
        .definition
        .clone()
        .or(character.description.clone())
        .unwrap_or_default();

    (swapped_character, Some(swapped_persona))
}
