use crate::chat_manager::prompting::lorebook_matcher::keyword_matches;
use crate::chat_manager::types::{
    PromptEntryChatMode, PromptEntryCondition, PromptEntryInfoSource, SystemPromptEntry,
};

#[derive(Clone, Debug)]
pub struct PromptEntryConditionContext<'a> {
    pub chat_mode: PromptEntryChatMode,
    pub info_source: PromptEntryInfoSource,
    pub scene_generation_enabled: bool,
    pub avatar_generation_enabled: bool,
    pub is_local_image_generation_model: bool,
    pub is_scene_generation_local_image_model: bool,
    pub has_scene: bool,
    pub has_scene_direction: bool,
    pub has_persona: bool,
    pub message_count: usize,
    pub participant_count: usize,
    pub recent_text: &'a str,
    pub dynamic_memory_enabled: bool,
    pub has_memory_summary: bool,
    pub has_key_memories: bool,
    pub has_lorebook_content: bool,
    pub does_author_note_exists: bool,
    pub has_active_scheduled_note: bool,
    pub has_subject_description: bool,
    pub has_current_description: bool,
    pub has_character_reference_images: bool,
    pub has_chat_background: bool,
    pub has_persona_reference_images: bool,
    pub has_character_reference_text: bool,
    pub has_persona_reference_text: bool,
    pub input_scopes: &'a [String],
    pub output_scopes: &'a [String],
    pub provider_id: Option<&'a str>,
    pub reasoning_enabled: bool,
    pub vision_enabled: bool,
    pub time_awareness_enabled: bool,
    pub companion_mode_enabled: bool,
}

pub fn entry_is_active(
    entry: &SystemPromptEntry,
    context: &PromptEntryConditionContext<'_>,
) -> bool {
    if !entry.enabled && !entry.system_prompt {
        return false;
    }

    entry
        .conditions
        .as_ref()
        .map(|condition| matches_condition(condition, context))
        .unwrap_or(true)
}

pub fn matches_condition(
    condition: &PromptEntryCondition,
    context: &PromptEntryConditionContext<'_>,
) -> bool {
    match condition {
        PromptEntryCondition::ChatMode { value } => value == &context.chat_mode,
        PromptEntryCondition::InfoSource { value } => *value == context.info_source,
        PromptEntryCondition::SceneGenerationEnabled { value } => {
            context.scene_generation_enabled == *value
        }
        PromptEntryCondition::AvatarGenerationEnabled { value } => {
            context.avatar_generation_enabled == *value
        }
        PromptEntryCondition::IsLocalImageGenerationModel { value } => {
            context.is_local_image_generation_model == *value
        }
        PromptEntryCondition::IsSceneGenerationLocalImageModel { value } => {
            context.is_scene_generation_local_image_model == *value
        }
        PromptEntryCondition::HasScene { value } => context.has_scene == *value,
        PromptEntryCondition::HasSceneDirection { value } => context.has_scene_direction == *value,
        PromptEntryCondition::HasPersona { value } => context.has_persona == *value,
        PromptEntryCondition::MessageCountAtLeast { value } => {
            context.message_count >= (*value as usize)
        }
        PromptEntryCondition::ParticipantCountAtLeast { value } => {
            context.participant_count >= (*value as usize)
        }
        PromptEntryCondition::KeywordAny { values } => {
            keyword_list_match_any(values, context.recent_text)
        }
        PromptEntryCondition::KeywordAll { values } => {
            keyword_list_match_all(values, context.recent_text)
        }
        PromptEntryCondition::KeywordNone { values } => {
            !keyword_list_match_any(values, context.recent_text)
        }
        PromptEntryCondition::DynamicMemoryEnabled { value } => {
            context.dynamic_memory_enabled == *value
        }
        PromptEntryCondition::HasMemorySummary { value } => context.has_memory_summary == *value,
        PromptEntryCondition::HasKeyMemories { value } => context.has_key_memories == *value,
        PromptEntryCondition::HasLorebookContent { value } => {
            context.has_lorebook_content == *value
        }
        PromptEntryCondition::DoesAuthorNoteExists { value } => {
            context.does_author_note_exists == *value
        }
        PromptEntryCondition::HasActiveScheduledNote { value } => {
            context.companion_mode_enabled && context.has_active_scheduled_note == *value
        }
        PromptEntryCondition::HasSubjectDescription { value } => {
            context.has_subject_description == *value
        }
        PromptEntryCondition::HasCurrentDescription { value } => {
            context.has_current_description == *value
        }
        PromptEntryCondition::HasCharacterReferenceImages { value } => {
            context.has_character_reference_images == *value
        }
        PromptEntryCondition::HasChatBackground { value } => context.has_chat_background == *value,
        PromptEntryCondition::HasPersonaReferenceImages { value } => {
            context.has_persona_reference_images == *value
        }
        PromptEntryCondition::HasCharacterReferenceText { value } => {
            context.has_character_reference_text == *value
        }
        PromptEntryCondition::HasPersonaReferenceText { value } => {
            context.has_persona_reference_text == *value
        }
        PromptEntryCondition::InputScopeAny { values } => {
            scope_list_match_any(values, context.input_scopes)
        }
        PromptEntryCondition::OutputScopeAny { values } => {
            scope_list_match_any(values, context.output_scopes)
        }
        PromptEntryCondition::ProviderIdAny { values } => values.iter().any(|value| {
            let trimmed = value.trim();
            !trimmed.is_empty()
                && context
                    .provider_id
                    .map(|provider_id| provider_id.eq_ignore_ascii_case(trimmed))
                    .unwrap_or(false)
        }),
        PromptEntryCondition::ReasoningEnabled { value } => context.reasoning_enabled == *value,
        PromptEntryCondition::VisionEnabled { value } => context.vision_enabled == *value,
        PromptEntryCondition::IsTimeAwarenessEnabled { value } => {
            context.time_awareness_enabled == *value
        }
        PromptEntryCondition::IsCompanionMode { value } => context.companion_mode_enabled == *value,
        PromptEntryCondition::All { conditions } => conditions
            .iter()
            .all(|item| matches_condition(item, context)),
        PromptEntryCondition::Any { conditions } => {
            !conditions.is_empty()
                && conditions
                    .iter()
                    .any(|item| matches_condition(item, context))
        }
        PromptEntryCondition::Not { condition } => !matches_condition(condition, context),
    }
}

fn keyword_list_match_any(values: &[String], text: &str) -> bool {
    values
        .iter()
        .any(|value| keyword_matches(value, text, false))
}

fn keyword_list_match_all(values: &[String], text: &str) -> bool {
    let filtered = values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    !filtered.is_empty()
        && filtered
            .iter()
            .all(|value| keyword_matches(value, text, false))
}

fn scope_list_match_any(values: &[String], scopes: &[String]) -> bool {
    let normalized_scopes = scopes
        .iter()
        .map(|scope| scope.trim().to_ascii_lowercase())
        .filter(|scope| !scope.is_empty())
        .collect::<Vec<_>>();

    values.iter().any(|value| {
        let wanted = value.trim().to_ascii_lowercase();
        !wanted.is_empty() && normalized_scopes.iter().any(|scope| scope == &wanted)
    })
}
