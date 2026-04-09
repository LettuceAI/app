use std::sync::OnceLock;
use tauri::AppHandle;

use crate::storage_manager::{
    characters::characters_list_typed,
    personas::personas_list_typed,
    sessions::{
        messages_list_internal, messages_list_pinned_internal, messages_upsert_batch_internal,
        session_get_meta_internal, session_upsert_meta_internal,
    },
    settings::{read_settings_typed, write_settings_typed},
};

use crate::chat_manager::prompt_engine;
use crate::chat_manager::types::{
    AccessibilitySettings, AccessibilitySoundSettings, AdvancedModelSettings, AdvancedSettings,
    Character, DynamicMemoryStructuredFallbackFormat, Model, Persona, ProviderCredential, Session,
    Settings, StoredMessage, SystemPromptEntry,
};

#[derive(Debug, Clone, Copy)]
pub enum PromptType {
    SystemPrompt,
    LocalRoleplayPrompt,
    DynamicMemoryPrompt,
    DynamicSummaryPrompt,
    HelpMeReplyPrompt,
    HelpMeReplyConversationalPrompt,
    GroupChatPrompt,
    GroupChatRoleplayPrompt,
    AvatarGenerationPrompt,
    AvatarEditPrompt,
    SceneGenerationPrompt,
    DesignReferencePrompt,
}

pub fn get_base_prompt(prompt_type: PromptType) -> String {
    match prompt_type {
        PromptType::SystemPrompt => prompt_engine::default_system_prompt_template(),
        PromptType::LocalRoleplayPrompt => prompt_engine::default_local_roleplay_prompt(),
        PromptType::DynamicMemoryPrompt => prompt_engine::default_dynamic_memory_prompt(),
        PromptType::DynamicSummaryPrompt => prompt_engine::default_dynamic_summary_prompt(),
        PromptType::HelpMeReplyPrompt => prompt_engine::default_help_me_reply_prompt(),
        PromptType::HelpMeReplyConversationalPrompt => {
            prompt_engine::default_help_me_reply_conversational_prompt()
        }
        PromptType::GroupChatPrompt => prompt_engine::default_group_chat_system_prompt_template(),
        PromptType::GroupChatRoleplayPrompt => {
            prompt_engine::default_group_chat_roleplay_prompt_template()
        }
        PromptType::AvatarGenerationPrompt => prompt_engine::default_avatar_generation_prompt(),
        PromptType::AvatarEditPrompt => prompt_engine::default_avatar_edit_prompt(),
        PromptType::SceneGenerationPrompt => prompt_engine::default_scene_generation_prompt(),
        PromptType::DesignReferencePrompt => prompt_engine::default_design_reference_prompt(),
    }
}

pub fn get_base_prompt_entries(prompt_type: PromptType) -> Vec<SystemPromptEntry> {
    match prompt_type {
        PromptType::SystemPrompt => prompt_engine::default_modular_prompt_entries(),
        PromptType::LocalRoleplayPrompt => prompt_engine::default_local_roleplay_entries(),
        PromptType::DynamicMemoryPrompt => prompt_engine::default_dynamic_memory_entries(),
        PromptType::DynamicSummaryPrompt => prompt_engine::default_dynamic_summary_entries(),
        PromptType::HelpMeReplyPrompt => prompt_engine::default_help_me_reply_entries(),
        PromptType::HelpMeReplyConversationalPrompt => {
            prompt_engine::default_help_me_reply_conversational_entries()
        }
        PromptType::GroupChatPrompt => prompt_engine::default_group_chat_entries(),
        PromptType::GroupChatRoleplayPrompt => prompt_engine::default_group_chat_roleplay_entries(),
        PromptType::AvatarGenerationPrompt => prompt_engine::default_avatar_generation_entries(),
        PromptType::AvatarEditPrompt => prompt_engine::default_avatar_edit_entries(),
        PromptType::SceneGenerationPrompt => prompt_engine::default_scene_generation_entries(),
        PromptType::DesignReferencePrompt => prompt_engine::default_design_reference_entries(),
    }
}

pub fn default_character_rules(pure_mode_level: &str) -> Vec<String> {
    let mut rules = vec![
        "Embody the character naturally without breaking immersion".to_string(),
        "Respond based on your personality, background, and current situation".to_string(),
        "Show emotions and reactions authentically through your words".to_string(),
        "Engage with the conversation organically, not like an assistant".to_string(),
        "You may roleplay as background characters or NPCs in the scene when needed (e.g., if you're a police officer and a witness appears, you can act as that witness). However, NEVER roleplay as the user's character - only control your own character and third-party characters".to_string(),
    ];

    match pure_mode_level {
        "off" => {}
        "low" => {
            rules.push("Avoid explicit sexual content".to_string());
        }
        "strict" => {
            rules.push(
                "Never generate sexually explicit, pornographic, or erotic content".to_string(),
            );
            rules.push(
                "Never describe sexual acts, nudity in sexual contexts, or sexual arousal"
                    .to_string(),
            );
            rules.push(
                "If asked to generate such content, decline and redirect the conversation"
                    .to_string(),
            );
            rules.push(
                "Romantic content must remain PG-13 — no explicit physical descriptions"
                    .to_string(),
            );
            rules.push("Violence descriptions should avoid gratuitous gore or torture".to_string());
            rules.push(
                "Do not use suggestive, flirty, or sexually charged language or tone".to_string(),
            );
        }
        // "standard" and anything else
        _ => {
            rules.push(
                "Never generate sexually explicit, pornographic, or erotic content".to_string(),
            );
            rules.push(
                "Never describe sexual acts, nudity in sexual contexts, or sexual arousal"
                    .to_string(),
            );
            rules.push(
                "If asked to generate such content, decline and redirect the conversation"
                    .to_string(),
            );
            rules.push(
                "Romantic content must remain PG-13 — no explicit physical descriptions"
                    .to_string(),
            );
            rules.push("Violence descriptions should avoid gratuitous gore or torture".to_string());
        }
    }

    rules
}

pub fn load_settings(app: &AppHandle) -> Result<Settings, String> {
    if let Some(settings) = read_settings_typed(app)? {
        Ok(settings)
    } else {
        let defaults = default_settings();
        write_settings_typed(app, &defaults)?;
        Ok(defaults)
    }
}

fn default_settings() -> Settings {
    Settings {
        default_provider_credential_id: None,
        default_model_id: None,
        provider_credentials: Vec::new(),
        models: Vec::new(),
        app_state: serde_json::Value::Null,
        advanced_model_settings: AdvancedModelSettings::default(),
        advanced_settings: Some(AdvancedSettings {
            summarisation_model_id: None,
            dynamic_memory_structured_fallback_format: Some(
                DynamicMemoryStructuredFallbackFormat::Xml,
            ),
            dynamic_memory_llama_sampler_overwrite_enabled: Some(true),
            avatar_generation_enabled: Some(true),
            avatar_generation_model_id: None,
            scene_generation_enabled: Some(true),
            scene_generation_mode: Some("auto".to_string()),
            scene_generation_model_id: None,
            scene_writer_model_id: None,
            creation_helper_enabled: None,
            creation_helper_model_id: None,
            help_me_reply_enabled: None,
            help_me_reply_model_id: None,
            help_me_reply_streaming: None,
            help_me_reply_max_tokens: None,
            help_me_reply_style: None,
            dynamic_memory: None,
            group_dynamic_memory: None,
            manual_mode_context_window: None,
            embedding_max_tokens: None,
            host_api: None,
            accessibility: Some(AccessibilitySettings {
                send: AccessibilitySoundSettings {
                    enabled: false,
                    volume: 0.5,
                },
                success: AccessibilitySoundSettings {
                    enabled: false,
                    volume: 0.6,
                },
                failure: AccessibilitySoundSettings {
                    enabled: false,
                    volume: 0.6,
                },
            }),
        }),
        prompt_template_id: None,
        system_prompt: None,
        migration_version: 0,
    }
}

pub fn load_characters(app: &AppHandle) -> Result<Vec<Character>, String> {
    characters_list_typed(app)
}

pub fn load_personas(app: &AppHandle) -> Result<Vec<Persona>, String> {
    personas_list_typed(app)
}

pub fn load_session(app: &AppHandle, session_id: &str) -> Result<Option<Session>, String> {
    let Some(mut session) = session_get_meta_internal(app, session_id)? else {
        return Ok(None);
    };
    let recent = messages_list_internal(app, session_id, 120, None, None)?;
    let pinned = messages_list_pinned_internal(app, session_id)?;

    let mut by_id = std::collections::HashMap::<String, StoredMessage>::new();
    for m in pinned.into_iter().chain(recent.into_iter()) {
        by_id.insert(m.id.clone(), m);
    }
    let mut merged: Vec<StoredMessage> = by_id.into_values().collect();
    merged.sort_by(|a, b| {
        a.created_at
            .cmp(&b.created_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    session.messages = merged;
    Ok(Some(session))
}

pub fn save_session(app: &AppHandle, session: &Session) -> Result<(), String> {
    let mut meta = session.clone();
    meta.messages = Vec::new();
    session_upsert_meta_internal(app, &meta)?;

    if let Some(last) = session.messages.last() {
        messages_upsert_batch_internal(app, &session.id, std::slice::from_ref(last))?;
    }
    Ok(())
}

pub fn select_model_with_credential<'a>(
    settings: &'a Settings,
    character: &Character,
) -> Result<(&'a Model, &'a ProviderCredential), String> {
    let model_id = character
        .default_model_id
        .clone()
        .or_else(|| settings.default_model_id.clone())
        .ok_or_else(|| "No default model configured".to_string())?;

    let model = settings
        .models
        .iter()
        .find(|m| m.id == model_id)
        .ok_or_else(|| "Model not found".to_string())?;

    let credential = resolve_credential_for_model(settings, model)
        .ok_or_else(|| "Provider credential not found".to_string())?;

    Ok((model, credential))
}

pub fn resolve_credential_for_model<'a>(
    settings: &'a Settings,
    model: &Model,
) -> Option<&'a ProviderCredential> {
    if model.provider_id.eq_ignore_ascii_case("llamacpp") {
        static LLAMA_CPP_CREDENTIAL: OnceLock<ProviderCredential> = OnceLock::new();
        return Some(LLAMA_CPP_CREDENTIAL.get_or_init(|| ProviderCredential {
            id: "builtin-llamacpp".to_string(),
            provider_id: "llamacpp".to_string(),
            label: "llama.cpp (Local)".to_string(),
            api_key: Some(String::new()),
            base_url: None,
            default_model: None,
            headers: None,
            config: None,
        }));
    }

    if let Some(model_cred_id) = model.provider_credential_id.as_ref() {
        if let Some(explicit_match) = settings
            .provider_credentials
            .iter()
            .find(|cred| &cred.id == model_cred_id && cred.provider_id == model.provider_id)
        {
            return Some(explicit_match);
        }
    }

    let candidates: Vec<&ProviderCredential> = settings
        .provider_credentials
        .iter()
        .filter(|cred| cred.provider_id == model.provider_id)
        .collect();

    if candidates.is_empty() {
        return None;
    }

    if let Some(default_cred_id) = settings.default_provider_credential_id.as_ref() {
        if let Some(default_match) = candidates
            .iter()
            .copied()
            .find(|cred| &cred.id == default_cred_id)
        {
            return Some(default_match);
        }
    }

    if candidates.len() == 1 {
        return candidates.first().copied();
    }

    if !model.provider_label.trim().is_empty() {
        if let Some(label_match) = candidates
            .iter()
            .copied()
            .find(|cred| cred.label == model.provider_label)
        {
            return Some(label_match);
        }
    }

    if let Some(default_model_match) = candidates
        .iter()
        .copied()
        .find(|cred| cred.default_model.as_deref() == Some(model.name.as_str()))
    {
        return Some(default_model_match);
    }

    // Multiple credentials exist for the same provider type and none matched.
    // Returning None avoids silently routing to the wrong endpoint.
    None
}

#[cfg(test)]
mod tests {
    use super::resolve_credential_for_model;
    use crate::chat_manager::types::{AdvancedModelSettings, Model, ProviderCredential, Settings};
    use serde_json::Value;

    fn mk_model(provider_id: &str, provider_label: &str, name: &str) -> Model {
        Model {
            id: "model-1".to_string(),
            name: name.to_string(),
            provider_id: provider_id.to_string(),
            provider_credential_id: None,
            provider_label: provider_label.to_string(),
            display_name: name.to_string(),
            created_at: 0,
            input_scopes: vec!["text".to_string()],
            output_scopes: vec!["text".to_string()],
            advanced_model_settings: None,
            prompt_template_id: None,
            voice_config: None,
            system_prompt: None,
        }
    }

    fn mk_cred(
        id: &str,
        provider_id: &str,
        label: &str,
        default_model: Option<&str>,
    ) -> ProviderCredential {
        ProviderCredential {
            id: id.to_string(),
            provider_id: provider_id.to_string(),
            label: label.to_string(),
            api_key: Some("k".to_string()),
            base_url: Some("https://example.com".to_string()),
            default_model: default_model.map(str::to_string),
            headers: None,
            config: None,
        }
    }

    fn mk_settings(
        default_provider_credential_id: Option<&str>,
        provider_credentials: Vec<ProviderCredential>,
    ) -> Settings {
        Settings {
            default_provider_credential_id: default_provider_credential_id.map(str::to_string),
            default_model_id: None,
            provider_credentials,
            models: vec![],
            app_state: Value::Null,
            advanced_model_settings: AdvancedModelSettings::default(),
            advanced_settings: None,
            prompt_template_id: None,
            system_prompt: None,
            migration_version: 0,
        }
    }

    #[test]
    fn resolves_single_candidate() {
        let model = mk_model("custom", "local", "glm-auto");
        let settings = mk_settings(None, vec![mk_cred("c1", "custom", "local", None)]);
        let picked = resolve_credential_for_model(&settings, &model).map(|c| c.id.clone());
        assert_eq!(picked.as_deref(), Some("c1"));
    }

    #[test]
    fn resolves_to_default_provider_credential_when_present() {
        let model = mk_model("custom", "local", "glm-auto");
        let settings = mk_settings(
            Some("c2"),
            vec![
                mk_cred("c1", "custom", "local", None),
                mk_cred("c2", "custom", "modal", None),
            ],
        );
        let picked = resolve_credential_for_model(&settings, &model).map(|c| c.id.clone());
        assert_eq!(picked.as_deref(), Some("c2"));
    }

    #[test]
    fn resolves_by_provider_label_when_multiple_candidates_exist() {
        let model = mk_model("custom", "local", "glm-auto");
        let settings = mk_settings(
            None,
            vec![
                mk_cred("c1", "custom", "modal", None),
                mk_cred("c2", "custom", "local", None),
            ],
        );
        let picked = resolve_credential_for_model(&settings, &model).map(|c| c.id.clone());
        assert_eq!(picked.as_deref(), Some("c2"));
    }

    #[test]
    fn resolves_by_credential_default_model_when_label_does_not_match() {
        let model = mk_model("custom", "unknown", "glm-auto");
        let settings = mk_settings(
            None,
            vec![
                mk_cred("c1", "custom", "modal", None),
                mk_cred("c2", "custom", "local", Some("glm-auto")),
            ],
        );
        let picked = resolve_credential_for_model(&settings, &model).map(|c| c.id.clone());
        assert_eq!(picked.as_deref(), Some("c2"));
    }

    #[test]
    fn returns_none_for_ambiguous_multiple_candidates() {
        let model = mk_model("custom", "", "glm-auto");
        let settings = mk_settings(
            None,
            vec![
                mk_cred("c1", "custom", "one", None),
                mk_cred("c2", "custom", "two", None),
            ],
        );
        let picked = resolve_credential_for_model(&settings, &model).map(|c| c.id.clone());
        assert!(picked.is_none());
    }

    #[test]
    fn resolves_explicit_model_provider_credential_id_first() {
        let mut model = mk_model("custom", "local", "glm-auto");
        model.provider_credential_id = Some("c2".to_string());
        let settings = mk_settings(
            None,
            vec![
                mk_cred("c1", "custom", "local", Some("glm-auto")),
                mk_cred("c2", "custom", "modal", None),
            ],
        );
        let picked = resolve_credential_for_model(&settings, &model).map(|c| c.id.clone());
        assert_eq!(picked.as_deref(), Some("c2"));
    }
}

pub fn choose_persona<'a>(
    personas: &'a [Persona],
    explicit: Option<&String>,
) -> Option<&'a Persona> {
    if let Some(id) = explicit {
        if id.is_empty() {
            return None;
        }
        if let Some(p) = personas.iter().find(|p| &p.id == id) {
            return Some(p);
        }
    }
    personas.iter().find(|p| p.is_default)
}

pub fn build_system_prompt(
    app: &AppHandle,
    character: &Character,
    model: &Model,
    persona: Option<&Persona>,
    session: &Session,
    settings: &Settings,
) -> Vec<SystemPromptEntry> {
    prompt_engine::build_system_prompt_entries(app, character, model, persona, session, settings)
}

pub fn recent_messages(session: &Session, limit: usize) -> Vec<StoredMessage> {
    let mut recent_msgs: Vec<StoredMessage> =
        session.messages.iter().rev().take(limit).cloned().collect();
    recent_msgs.reverse();
    recent_msgs
}
