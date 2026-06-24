//! Gathered from inline tests in src/storage_manager/system_cards.rs.

use lettuceai_lib::chat_manager::types::{Model, PromptTemplateType};
use lettuceai_lib::storage_manager::lorebook::{Lorebook, LorebookEntry};
use lettuceai_lib::storage_manager::system_cards::*;
use lettuceai_lib::sync::models::{ChatTemplate, ChatTemplateMessage};

#[test]
fn parse_rejects_non_usc_schema() {
    let value = serde_json::json!({
        "schema": { "name": "UEC", "version": "1.0" },
        "kind": "lorebook",
        "payload": {}
    });

    let error = parse_usc_value(&value).unwrap_err();
    assert!(error.contains("schema name"));
}

#[test]
fn create_chat_template_maps_prompt_template_to_system_prompt_template_ref() {
    let template = ChatTemplate {
        id: "template-1".into(),
        character_id: "character-1".into(),
        name: "Opener".into(),
        scene_id: Some("scene-1".into()),
        prompt_template_id: Some("prompt-1".into()),
        lorebook_ids_override: None,
        created_at: 123,
    };
    let messages = vec![
        ChatTemplateMessage {
            id: "msg-2".into(),
            template_id: template.id.clone(),
            idx: 1,
            role: "assistant".into(),
            content: "Second".into(),
        },
        ChatTemplateMessage {
            id: "msg-1".into(),
            template_id: template.id.clone(),
            idx: 0,
            role: "user".into(),
            content: "First".into(),
        },
    ];

    let card = create_chat_template_usc(&template, &messages);

    assert_eq!(card.kind, UscKind::ChatTemplate);
    assert_eq!(card.payload.messages[0].id, "msg-1");
    assert_eq!(
        card.payload
            .system_prompt_template
            .as_ref()
            .map(|item| item.id.as_str()),
        Some("prompt-1")
    );
}

#[test]
fn create_model_profile_does_not_store_credentials() {
    let model = Model {
        id: "model-1".into(),
        name: "gpt-4o-mini".into(),
        provider_id: "openai".into(),
        provider_credential_id: Some("credential-1".into()),
        provider_label: "OpenAI".into(),
        display_name: "GPT-4o Mini".into(),
        created_at: 456,
        input_scopes: vec!["text".into()],
        output_scopes: vec!["text".into()],
        advanced_model_settings: None,
        prompt_template_id: Some("prompt-2".into()),
        voice_config: None,
        system_prompt: None,
    };

    let card = create_model_profile_usc(&model);
    let json = serde_json::to_value(&card).unwrap();

    assert!(json.get("providerCredentialId").is_none());
    assert_eq!(
        json.pointer("/payload/systemPromptTemplate/id")
            .and_then(|value| value.as_str()),
        Some("prompt-2")
    );
}

#[test]
fn parse_system_prompt_template_round_trips() {
    let card = UscCard {
        schema: UscSchemaInfo::default(),
        kind: UscKind::SystemPromptTemplate,
        payload: UscSystemPromptTemplatePayload {
            id: "prompt-1".into(),
            name: "RP Core".into(),
            description: None,
            prompt_type: PromptTemplateType::DirectChat,
            content: "Stay in character.".into(),
            entries: vec![],
            condense_prompt_entries: false,
            created_at: 1,
            updated_at: 2,
            variables: None,
            requires: None,
        },
        app_specific_settings: None::<UscSystemPromptTemplateAppSettings>,
        meta: None,
        extensions: None,
    };

    let value = serde_json::to_value(&card).unwrap();
    let parsed = parse_usc_value(&value).unwrap();

    match parsed {
        AnyUscCard::SystemPromptTemplate(parsed_card) => {
            assert_eq!(parsed_card.payload.id, "prompt-1");
            assert_eq!(parsed_card.payload.name, "RP Core");
        }
        _ => panic!("expected system prompt template card"),
    }
}

#[test]
fn serialize_uses_app_specific_settings_key() {
    let card = UscCard {
        schema: UscSchemaInfo::default(),
        kind: UscKind::ChatTemplate,
        payload: UscChatTemplatePayload {
            id: "template-1".into(),
            name: "Starter".into(),
            description: None,
            messages: vec![],
            scene_id: None,
            system_prompt_template: None,
            lorebook_ids_override: None,
            variables: None,
            opening_notes: None,
            requires: None,
            created_at: 1,
        },
        app_specific_settings: Some(UscChatTemplateAppSettings {
            pinned: Some(true),
            editor: None,
            extra: UscExtensions::default(),
        }),
        meta: None,
        extensions: None,
    };

    let value = serde_json::to_value(&card).unwrap();

    assert!(value.get("app_specific_settings").is_some());
    assert!(value.get("appSpecificSettings").is_none());
}

#[test]
fn create_lorebook_omits_internal_lorebook_id_from_entries() {
    let lorebook = Lorebook {
        id: "lorebook-1".into(),
        name: "World".into(),
        avatar_path: None,
        keyword_detection_mode:
            lettuceai_lib::storage_manager::lorebook::LorebookKeywordDetectionMode::RecentMessageWindow,
        created_at: 1,
        updated_at: 2,
    };
    let entries = vec![LorebookEntry {
        id: "entry-1".into(),
        lorebook_id: lorebook.id.clone(),
        title: "North Gate".into(),
        enabled: true,
        always_active: false,
        keywords: vec!["north gate".into()],
        case_sensitive: false,
        content: "Guarded day and night.".into(),
        priority: 0,
        display_order: 0,
        created_at: 3,
        updated_at: 4,
    }];

    let card = create_lorebook_usc(&lorebook, &entries);
    let value = serde_json::to_value(&card).unwrap();

    assert!(
        value.pointer("/payload/entries/0/lorebookId").is_none(),
        "USC lorebook entries should not leak internal lorebookId"
    );
}
