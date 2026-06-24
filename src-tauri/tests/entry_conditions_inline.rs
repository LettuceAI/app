//! Gathered from inline tests in src/chat_manager/prompting/entry_conditions.rs.

use lettuceai_lib::chat_manager::prompting::entry_conditions::*;
use lettuceai_lib::chat_manager::types::{
    PromptEntryChatMode, PromptEntryCondition, PromptEntryInfoSource,
};

fn sample_context<'a>() -> PromptEntryConditionContext<'a> {
    let input_scopes = Box::leak(Box::new(vec!["text".to_string(), "image".to_string()]));
    let output_scopes = Box::leak(Box::new(vec!["text".to_string()]));
    PromptEntryConditionContext {
        chat_mode: PromptEntryChatMode::Group,
        info_source: PromptEntryInfoSource::Messages,
        scene_generation_enabled: true,
        avatar_generation_enabled: true,
        has_scene: true,
        has_scene_direction: false,
        has_persona: true,
        message_count: 12,
        participant_count: 4,
        recent_text: "The sunset beach scene has four people talking about dinner.",
        dynamic_memory_enabled: true,
        has_memory_summary: true,
        has_key_memories: false,
        has_lorebook_content: true,
        does_author_note_exists: true,
        has_active_scheduled_note: false,
        has_subject_description: false,
        has_current_description: false,
        has_character_reference_images: false,
        has_chat_background: false,
        has_persona_reference_images: false,
        has_character_reference_text: false,
        has_persona_reference_text: false,
        input_scopes,
        output_scopes,
        provider_id: Some("openai"),
        reasoning_enabled: true,
        vision_enabled: true,
        time_awareness_enabled: false,
        companion_mode_enabled: false,
    }
}

#[test]
fn matches_nested_conditions() {
    let condition = PromptEntryCondition::All {
        conditions: vec![
            PromptEntryCondition::ChatMode {
                value: PromptEntryChatMode::Group,
            },
            PromptEntryCondition::Any {
                conditions: vec![
                    PromptEntryCondition::KeywordAny {
                        values: vec!["sunset".to_string()],
                    },
                    PromptEntryCondition::KeywordAny {
                        values: vec!["rain".to_string()],
                    },
                ],
            },
            PromptEntryCondition::Not {
                condition: Box::new(PromptEntryCondition::HasKeyMemories { value: true }),
            },
        ],
    };

    assert!(matches_condition(&condition, &sample_context()));
}

#[test]
fn matches_scope_conditions_case_insensitively() {
    let condition = PromptEntryCondition::InputScopeAny {
        values: vec!["IMAGE".to_string()],
    };

    assert!(matches_condition(&condition, &sample_context()));
}

#[test]
fn matches_time_awareness_condition() {
    let mut context = sample_context();
    context.time_awareness_enabled = true;

    let condition = PromptEntryCondition::IsTimeAwarenessEnabled { value: true };

    assert!(matches_condition(&condition, &context));
}

#[test]
fn matches_companion_mode_condition() {
    let mut context = sample_context();
    context.companion_mode_enabled = true;

    let condition = PromptEntryCondition::IsCompanionMode { value: true };

    assert!(matches_condition(&condition, &context));
}

#[test]
fn active_scheduled_note_condition_only_matches_in_companion_mode() {
    let mut context = sample_context();
    context.has_active_scheduled_note = true;

    let condition = PromptEntryCondition::HasActiveScheduledNote { value: true };
    assert!(!matches_condition(&condition, &context));

    context.companion_mode_enabled = true;
    assert!(matches_condition(&condition, &context));
}
