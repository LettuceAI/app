//! Gathered from inline tests in src/chat_manager/prompting/prompt_engine.rs.

use lettuceai_lib::chat_manager::prompting::prompt_engine::render_with_context_internal;
use lettuceai_lib::chat_manager::types::{
    Character, Model, Persona, Scene, SceneVariant, Session, Settings, StoredMessage,
};

fn make_character() -> Character {
    Character {
        id: "c1".into(),
        name: "Alice".into(),
        avatar_path: None,
        design_description: None,
        design_reference_image_ids: vec![],
        background_image_path: None,
        description: Some("I am {{char}}. Partner: {{persona}}.".into()),
        definition: Some("I am {{char}}. Partner: {{persona}}.".into()),
        rules: vec![],
        scenes: vec![],
        default_scene_id: None,
        default_model_id: None,
        lora_name: None,
        lora_strength: None,
        mode: "roleplay".into(),
        companion: None,
        memory_type: "manual".into(),
        active_lorebook_ids: vec![],
        prompt_template_id: None,
        group_chat_prompt_template_id: None,
        group_chat_roleplay_prompt_template_id: None,
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
        advanced_model_settings: lettuceai_lib::chat_manager::types::AdvancedModelSettings::default(
        ),
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
        background_image_path: None,
        system_prompt: None,
        mode: "roleplay".into(),
        selected_scene_id: None,
        prompt_template_id: None,
        lorebook_ids_override: None,
        author_note: None,
        persona_id: None,
        persona_disabled: false,
        voice_autoplay: None,
        advanced_model_settings: None,
        companion_state: None,
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
        memory_progress_step: None,
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
        avatar_path: None,
        design_description: None,
        lora_name: None,
        lora_strength: None,
        design_reference_image_ids: vec![],
        active_lorebook_ids: vec![],
        nickname: None,
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
        None,
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
        direction: None,
        background_image_path: None,
        created_at: 0,
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
        None,
    );
    assert!(rendered2.contains("Var Alice"));
    assert!(!rendered2.contains("Starting Scene")); // No hardcoded formatting

    let mut session2_edited = session2.clone();
    session2_edited.messages.push(StoredMessage {
        id: "msg-scene".into(),
        role: "scene".into(),
        content: "Edited scene with {{char}} and {{persona}}".into(),
        created_at: 1,
        visible_in_chat: false,
        scene_edited: true,
        usage: None,
        variants: vec![],
        selected_variant_id: None,
        memory_refs: vec![],
        used_lorebook_entries: vec![],
        is_pinned: false,
        attachments: vec![],
        reasoning: None,
        model_id: None,
    });
    let rendered2_edited = render_with_context_internal(
        None,
        base2,
        &character2,
        persona.as_ref(),
        &session2_edited,
        &settings,
        None,
    );
    assert_eq!(rendered2_edited, "Edited scene with Alice and Bob");

    let mut session3 = session.clone();
    session3.author_note = Some("Keep {{char}} focused on {{persona}}.".into());
    let rendered3 = render_with_context_internal(
        None,
        "{{author_note}}",
        &character,
        persona.as_ref(),
        &session3,
        &settings,
        None,
    );
    assert_eq!(rendered3, "Keep Alice focused on Bob.");

    let rendered4 = render_with_context_internal(
        None,
        "{{date}} {{time_hour}}:{{time_minute}} {{time_12hour_format}} {{datetime_iso}}",
        &character,
        persona.as_ref(),
        &session,
        &settings,
        None,
    );
    assert!(!rendered4.contains("{{date}}"));
    assert!(!rendered4.contains("{{time_hour}}"));
    assert!(!rendered4.contains("{{time_minute}}"));
    assert!(!rendered4.contains("{{time_12hour_format}}"));
    assert!(!rendered4.contains("{{datetime_iso}}"));
}
