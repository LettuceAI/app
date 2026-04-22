use crate::chat_manager::prompting::lorebook_matcher::keyword_matches;
use crate::chat_manager::types::{PromptEntryChatMode, PromptEntryCondition, SystemPromptEntry};

#[derive(Clone, Debug)]
pub(crate) struct PromptEntryConditionContext<'a> {
    pub(crate) chat_mode: PromptEntryChatMode,
    pub(crate) scene_generation_enabled: bool,
    pub(crate) avatar_generation_enabled: bool,
    pub(crate) has_scene: bool,
    pub(crate) has_scene_direction: bool,
    pub(crate) has_persona: bool,
    pub(crate) message_count: usize,
    pub(crate) participant_count: usize,
    pub(crate) recent_text: &'a str,
    pub(crate) dynamic_memory_enabled: bool,
    pub(crate) has_memory_summary: bool,
    pub(crate) has_key_memories: bool,
    pub(crate) has_lorebook_content: bool,
    pub(crate) does_author_note_exists: bool,
    pub(crate) has_subject_description: bool,
    pub(crate) has_current_description: bool,
    pub(crate) has_character_reference_images: bool,
    pub(crate) has_chat_background: bool,
    pub(crate) has_persona_reference_images: bool,
    pub(crate) has_character_reference_text: bool,
    pub(crate) has_persona_reference_text: bool,
    pub(crate) input_scopes: &'a [String],
    pub(crate) output_scopes: &'a [String],
    pub(crate) provider_id: Option<&'a str>,
    pub(crate) reasoning_enabled: bool,
    pub(crate) vision_enabled: bool,
}

pub(crate) fn entry_is_active(
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

pub(crate) fn matches_condition(
    condition: &PromptEntryCondition,
    context: &PromptEntryConditionContext<'_>,
) -> bool {
    match condition {
        PromptEntryCondition::ChatMode { value } => value == &context.chat_mode,
        PromptEntryCondition::SceneGenerationEnabled { value } => {
            context.scene_generation_enabled == *value
        }
        PromptEntryCondition::AvatarGenerationEnabled { value } => {
            context.avatar_generation_enabled == *value
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_context<'a>() -> PromptEntryConditionContext<'a> {
        let input_scopes = Box::leak(Box::new(vec!["text".to_string(), "image".to_string()]));
        let output_scopes = Box::leak(Box::new(vec!["text".to_string()]));
        PromptEntryConditionContext {
            chat_mode: PromptEntryChatMode::Group,
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
}
