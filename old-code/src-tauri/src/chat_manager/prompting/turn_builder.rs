use serde_json::Value;

use crate::chat_manager::types::{
    Character, Persona, PromptEntryPosition, Settings, StoredMessage, SystemPromptEntry,
};

pub fn message_visible_to_model(message: &StoredMessage) -> bool {
    message.role == "user"
        || message.role == "assistant"
        || message.role == "scene"
        || (message.role == "system" && message.visible_in_chat)
}

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
        .unwrap_or(false);

    let active = if scene_generation_enabled {
        scene_image_protocol_variant(settings)
    } else {
        None
    };

    system_prompt_entries
        .into_iter()
        .filter(|entry| match scene_image_protocol_kind(entry) {
            Some(kind) => Some(kind) == active,
            None => true,
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SceneImageProtocolKind {
    Remote,
    Local,
}

fn scene_image_protocol_variant(settings: &Settings) -> Option<SceneImageProtocolKind> {
    let (model, _) = crate::chat_manager::scene::resolve_image_generation_target(
        settings,
        settings
            .advanced_settings
            .as_ref()
            .and_then(|advanced| advanced.scene_generation_model_id.as_deref()),
    )
    .ok()?;

    Some(if model.provider_id == "sdcpp" {
        SceneImageProtocolKind::Local
    } else {
        SceneImageProtocolKind::Remote
    })
}

fn scene_image_protocol_kind(entry: &SystemPromptEntry) -> Option<SceneImageProtocolKind> {
    match entry.id.as_str() {
        "entry_scene_image_protocol" => Some(SceneImageProtocolKind::Remote),
        "entry_scene_image_protocol_local" => Some(SceneImageProtocolKind::Local),
        _ => None,
    }
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
        if let Some(message) =
            crate::chat_manager::messages::prompt_entry_message(system_role, entry)
        {
            messages.insert(insert_at, message);
        }
        offset += 1;
    }
}

pub fn assemble_prompt_messages(
    entries: Vec<SystemPromptEntry>,
    mut conversation_messages: Vec<Value>,
    system_role: &str,
) -> Vec<Value> {
    let (relative_entries, in_chat_entries) = partition_prompt_entries(entries);
    let mut messages = Vec::new();
    for entry in &relative_entries {
        crate::chat_manager::messages::push_prompt_entry_message(&mut messages, system_role, entry);
    }
    insert_in_chat_prompt_entries(&mut conversation_messages, system_role, &in_chat_entries);
    messages.extend(conversation_messages);
    messages
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
    let pinned: Vec<StoredMessage> = messages
        .iter()
        .filter(|m| m.is_pinned && message_visible_to_model(m))
        .cloned()
        .collect();
    let recent_non_pinned: Vec<StoredMessage> = messages
        .iter()
        .rev()
        .filter(|m| !m.is_pinned && message_visible_to_model(m))
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
        .filter(|m| {
            m.role == "user" || m.role == "assistant" || (m.role == "system" && m.visible_in_chat)
        })
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

#[cfg(test)]
mod tests {
    use super::{
        append_image_directive_instructions, assemble_prompt_messages,
        insert_in_chat_prompt_entries, scene_image_protocol_kind, SceneImageProtocolKind,
    };
    use crate::chat_manager::entries::{in_chat_system_entry, relative_system_entry};
    use crate::chat_manager::types::{PromptEntryPosition, PromptEntryRole, SystemPromptEntry};
    use serde_json::json;

    fn in_chat_entry(content: &str, depth: u32) -> SystemPromptEntry {
        SystemPromptEntry {
            id: content.to_string(),
            name: content.to_string(),
            role: PromptEntryRole::System,
            content: content.to_string(),
            enabled: true,
            injection_position: PromptEntryPosition::InChat,
            injection_depth: depth,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
            conditions: None,
            prompt_entry_payload: None,
        }
    }

    fn settings_with(provider_id: &str) -> crate::chat_manager::types::Settings {
        let mut settings = crate::chat_manager::persistence::storage::default_settings();
        if let Some(advanced) = settings.advanced_settings.as_mut() {
            advanced.scene_generation_enabled = Some(true);
        }
        settings.models = vec![crate::chat_manager::types::Model {
            id: "m1".into(),
            name: "img".into(),
            provider_id: provider_id.into(),
            provider_credential_id: Some("cred1".into()),
            provider_label: provider_id.into(),
            display_name: "Img".into(),
            created_at: 0,
            input_scopes: vec!["text".into()],
            output_scopes: vec!["image".into()],
            advanced_model_settings: None,
            prompt_template_id: None,
            system_prompt: None,
            voice_config: None,
        }];
        settings
            .provider_credentials
            .push(crate::chat_manager::types::ProviderCredential {
                id: "cred1".into(),
                provider_id: provider_id.into(),
                label: provider_id.into(),
                api_key: Some("k".into()),
                base_url: None,
                default_model: None,
                headers: None,
                config: None,
            });
        settings
    }

    fn protocol_entries() -> Vec<SystemPromptEntry> {
        vec![
            relative_system_entry("entry_instructions", "Instructions", "body"),
            relative_system_entry("entry_scene_image_protocol", "Remote", "remote"),
            relative_system_entry("entry_scene_image_protocol_local", "Local", "local"),
        ]
    }

    fn kept_ids(settings: &crate::chat_manager::types::Settings) -> Vec<String> {
        append_image_directive_instructions(protocol_entries(), settings)
            .into_iter()
            .map(|entry| entry.id)
            .collect()
    }

    #[test]
    fn no_image_model_drops_both_scene_protocol_variants() {
        let mut settings = crate::chat_manager::persistence::storage::default_settings();
        if let Some(advanced) = settings.advanced_settings.as_mut() {
            advanced.scene_generation_enabled = Some(true);
        }
        assert_eq!(kept_ids(&settings), vec!["entry_instructions".to_string()]);
    }

    #[test]
    fn a_remote_image_model_keeps_only_the_remote_protocol() {
        let ids = kept_ids(&settings_with("literouter"));
        assert!(ids.contains(&"entry_scene_image_protocol".to_string()));
        assert!(!ids.contains(&"entry_scene_image_protocol_local".to_string()));
    }

    #[test]
    fn a_local_image_model_keeps_only_the_local_protocol() {
        let ids = kept_ids(&settings_with("sdcpp"));
        assert!(ids.contains(&"entry_scene_image_protocol_local".to_string()));
        assert!(!ids.contains(&"entry_scene_image_protocol".to_string()));
    }

    #[test]
    fn scene_generation_is_off_when_unset() {
        let mut settings = settings_with("literouter");
        if let Some(advanced) = settings.advanced_settings.as_mut() {
            advanced.scene_generation_enabled = None;
        }
        assert_eq!(kept_ids(&settings), vec!["entry_instructions".to_string()]);
    }

    #[test]
    fn disabling_scene_generation_drops_both_variants() {
        let mut settings = settings_with("literouter");
        if let Some(advanced) = settings.advanced_settings.as_mut() {
            advanced.scene_generation_enabled = Some(false);
        }
        assert_eq!(kept_ids(&settings), vec!["entry_instructions".to_string()]);
    }

    #[test]
    fn both_scene_image_protocol_variants_are_recognized() {
        let mut entry = relative_system_entry(
            "entry_scene_image_protocol",
            "Scene Image Protocol",
            "remote",
        );
        assert_eq!(
            scene_image_protocol_kind(&entry),
            Some(SceneImageProtocolKind::Remote)
        );
        entry.id = "entry_scene_image_protocol_local".to_string();
        assert_eq!(
            scene_image_protocol_kind(&entry),
            Some(SceneImageProtocolKind::Local)
        );
        entry.id = "entry_instructions".to_string();
        assert_eq!(scene_image_protocol_kind(&entry), None);
    }

    #[test]
    fn depth_zero_context_is_hidden_after_the_latest_user_message() {
        let mut messages = vec![
            json!({"role": "assistant", "content": "Earlier reply"}),
            json!({"role": "user", "content": "Latest user message"}),
        ];

        insert_in_chat_prompt_entries(
            &mut messages,
            "system",
            &[in_chat_entry("Current lore and memory", 0)],
        );

        assert_eq!(messages[1]["content"], "Latest user message");
        assert_eq!(messages[2]["role"], "system");
        assert_eq!(messages[2]["content"], "Current lore and memory");
    }

    #[test]
    fn assembler_keeps_stable_entries_before_transcript_and_volatile_entries_after_it() {
        let entries = vec![
            relative_system_entry("stable", "Stable", "Stable instructions"),
            in_chat_system_entry("volatile", "Volatile", "Current memory and lore", 0),
        ];
        let transcript = vec![json!({"role": "user", "content": "Latest message"})];

        let messages = assemble_prompt_messages(entries, transcript, "developer");

        assert_eq!(messages[0]["role"], "developer");
        assert_eq!(messages[0]["content"], "Stable instructions");
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[1]["content"], "Latest message");
        assert_eq!(messages[2]["role"], "developer");
        assert_eq!(messages[2]["content"], "Current memory and lore");
    }
}
