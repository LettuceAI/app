//! Gathered from inline tests in src/chat_manager/reply_helper/mod.rs.

use lettuceai_lib::chat_manager::prompting::turn_builder::swapped_prompt_entities;
use lettuceai_lib::chat_manager::reply_helper::help_me_reply_participant_names;
use lettuceai_lib::chat_manager::types::{Character, Persona};

fn make_character() -> Character {
    Character {
        id: "char-1".to_string(),
        name: "Astra".to_string(),
        avatar_path: None,
        design_description: None,
        design_reference_image_ids: Vec::new(),
        background_image_path: None,
        definition: Some("A starship captain".to_string()),
        description: Some("Commanding and curious".to_string()),
        rules: Vec::new(),
        scenes: Vec::new(),
        default_scene_id: None,
        default_model_id: None,
        lora_name: None,
        lora_strength: None,
        mode: "roleplay".to_string(),
        companion: None,
        memory_type: "manual".to_string(),
        active_lorebook_ids: Vec::new(),
        prompt_template_id: None,
        group_chat_prompt_template_id: None,
        group_chat_roleplay_prompt_template_id: None,
        system_prompt: None,
        created_at: 0,
        updated_at: 0,
    }
}

fn make_persona() -> Persona {
    Persona {
        id: "persona-1".to_string(),
        title: "Milo".to_string(),
        description: "A reckless smuggler".to_string(),
        nickname: None,
        avatar_path: None,
        lora_name: None,
        lora_strength: None,
        design_description: None,
        design_reference_image_ids: Vec::new(),
        active_lorebook_ids: Vec::new(),
        is_default: false,
        created_at: 0,
        updated_at: 0,
    }
}

#[test]
fn help_me_reply_names_match_unswapped_prompt_entities() {
    let character = make_character();
    let persona = make_persona();

    let (effective_user_name, effective_assistant_name) =
        help_me_reply_participant_names(&character, Some(&persona));

    assert_eq!(effective_user_name, "Milo");
    assert_eq!(effective_assistant_name, "Astra");
}

#[test]
fn help_me_reply_names_follow_swapped_prompt_entities() {
    let character = make_character();
    let persona = make_persona();
    let (prompt_character, prompt_persona) = swapped_prompt_entities(&character, Some(&persona));

    let (effective_user_name, effective_assistant_name) =
        help_me_reply_participant_names(&prompt_character, prompt_persona.as_ref());

    assert_eq!(effective_user_name, "Astra");
    assert_eq!(effective_assistant_name, "Milo");
}
