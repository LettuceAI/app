use crate::chat_manager::types::{PromptEntryImageSlot, PromptTemplateType};
use serde::Serialize;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptVariableDefinition {
    pub variable: String,
    pub label: String,
    pub description: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptTypeDefinition {
    pub prompt_type: PromptTemplateType,
    pub label: String,
    pub allowed_variables: Vec<PromptVariableDefinition>,
    pub required_variables: Vec<String>,
    pub allowed_image_slots: Vec<PromptEntryImageSlot>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptParameterEngine {
    pub prompt_types: Vec<PromptTypeDefinition>,
}

fn variable(variable: &str, label: &str, description: &str) -> PromptVariableDefinition {
    PromptVariableDefinition {
        variable: variable.to_string(),
        label: label.to_string(),
        description: description.to_string(),
    }
}

fn direct_chat_variables() -> Vec<PromptVariableDefinition> {
    vec![
        variable(
            "{{char.name}}",
            "Character Name",
            "The character's display name.",
        ),
        variable(
            "{{char.desc}}",
            "Character Definition",
            "The active character definition.",
        ),
        variable("{{scene}}", "Scene", "Starting scene or scenario text."),
        variable(
            "{{scene_direction}}",
            "Scene Direction",
            "Optional hidden direction for the scene.",
        ),
        variable("{{persona.name}}", "User Name", "The active persona name."),
        variable(
            "{{persona.desc}}",
            "User Description",
            "The active persona description.",
        ),
        variable(
            "{{context_summary}}",
            "Context Summary",
            "Dynamic conversation summary.",
        ),
        variable(
            "{{companion_state}}",
            "Companion State",
            "Rendered emotional and relationship state for companion chats.",
        ),
        variable(
            "{{key_memories}}",
            "Key Memories",
            "Relevant long-term memory facts.",
        ),
        variable("{{lorebook}}", "Lorebook", "Matched lorebook content."),
        variable(
            "{{author_note}}",
            "Author Note",
            "Private session note for the current chat.",
        ),
        variable("{{rules}}", "Rules", "Legacy behavioral rules block."),
        variable(
            "{{content_rules}}",
            "Content Rules",
            "Resolved content rules and safety constraints.",
        ),
    ]
}

fn group_chat_conversational_variables() -> Vec<PromptVariableDefinition> {
    vec![
        variable(
            "{{char.name}}",
            "Character Name",
            "The speaking character's display name.",
        ),
        variable(
            "{{char.desc}}",
            "Character Definition",
            "The speaking character definition.",
        ),
        variable("{{persona.name}}", "User Name", "The active persona name."),
        variable(
            "{{persona.desc}}",
            "User Description",
            "The active persona description.",
        ),
        variable(
            "{{group_characters}}",
            "Group Characters",
            "Rendered list of group participants.",
        ),
    ]
}

fn group_chat_roleplay_variables() -> Vec<PromptVariableDefinition> {
    vec![
        variable("{{scene}}", "Scene", "Starting scene or scenario text."),
        variable(
            "{{scene_direction}}",
            "Scene Direction",
            "Optional hidden direction for the scene.",
        ),
        variable(
            "{{char.name}}",
            "Character Name",
            "The speaking character's display name.",
        ),
        variable(
            "{{char.desc}}",
            "Character Definition",
            "The speaking character definition.",
        ),
        variable("{{persona.name}}", "User Name", "The active persona name."),
        variable(
            "{{persona.desc}}",
            "User Description",
            "The active persona description.",
        ),
        variable(
            "{{group_characters}}",
            "Group Characters",
            "Rendered list of group participants.",
        ),
        variable(
            "{{context_summary}}",
            "Context Summary",
            "Dynamic conversation summary.",
        ),
        variable(
            "{{key_memories}}",
            "Key Memories",
            "Relevant long-term memory facts.",
        ),
    ]
}

fn dynamic_memory_summarizer_variables() -> Vec<PromptVariableDefinition> {
    vec![
        variable(
            "{{prev_summary}}",
            "Previous Summary",
            "The cumulative summary so far.",
        ),
        variable(
            "{{character}}",
            "Character",
            "Character summary placeholder.",
        ),
        variable("{{persona}}", "Persona", "Persona summary placeholder."),
    ]
}

fn dynamic_memory_manager_variables() -> Vec<PromptVariableDefinition> {
    vec![
        variable(
            "{{max_entries}}",
            "Max Entries",
            "Maximum number of memory entries allowed.",
        ),
        variable(
            "{{current_memory_tokens}}",
            "Current Memory Tokens",
            "Current hot memory token usage.",
        ),
        variable(
            "{{hot_token_budget}}",
            "Hot Token Budget",
            "Configured token budget for hot memories.",
        ),
    ]
}

fn reply_helper_variables() -> Vec<PromptVariableDefinition> {
    vec![
        variable(
            "{{char.name}}",
            "Character Name",
            "The character being replied to.",
        ),
        variable(
            "{{char.desc}}",
            "Character Definition",
            "The character definition.",
        ),
        variable("{{persona.name}}", "User Name", "The active persona name."),
        variable(
            "{{persona.desc}}",
            "User Description",
            "The active persona description.",
        ),
        variable(
            "{{current_draft}}",
            "Current Draft",
            "The user's unfinished draft reply.",
        ),
    ]
}

fn lorebook_entry_writer_variables() -> Vec<PromptVariableDefinition> {
    vec![
        variable(
            "{{lorebook_name}}",
            "Lorebook Name",
            "Name of the target lorebook.",
        ),
        variable(
            "{{character_name}}",
            "Character Name",
            "Name of the character whose session is being mined.",
        ),
        variable(
            "{{session_title}}",
            "Session Title",
            "Title of the selected session.",
        ),
        variable(
            "{{selected_messages}}",
            "Selected Messages",
            "Chronological transcript of the selected messages (messages source mode).",
        ),
        variable(
            "{{memory_summary}}",
            "Memory Summary",
            "Dynamic memory context summary for the session (memory source mode).",
        ),
        variable(
            "{{selected_memories}}",
            "Selected Memories",
            "Selected memory entries from dynamic memory (memory source mode).",
        ),
        variable(
            "{{direction_prompt}}",
            "Direction Prompt",
            "Optional user guidance for the extraction focus.",
        ),
        variable(
            "{{existing_entries}}",
            "Existing Entries",
            "Existing lorebook entry summaries for duplicate avoidance.",
        ),
    ]
}

fn lorebook_keyword_generator_variables() -> Vec<PromptVariableDefinition> {
    vec![
        variable(
            "{{entry_title}}",
            "Entry Title",
            "Current lorebook entry title.",
        ),
        variable(
            "{{entry_content}}",
            "Entry Content",
            "Current lorebook entry content.",
        ),
        variable(
            "{{existing_keywords}}",
            "Existing Keywords",
            "Current keywords already attached to the entry.",
        ),
        variable(
            "{{direction_prompt}}",
            "Direction Prompt",
            "Optional user guidance for keyword selection.",
        ),
    ]
}

fn avatar_generation_variables() -> Vec<PromptVariableDefinition> {
    vec![
        variable(
            "{{avatar_subject_name}}",
            "Avatar Subject Name",
            "Name of the character or persona the avatar is for.",
        ),
        variable(
            "{{avatar_subject_description}}",
            "Avatar Subject Description",
            "Description of the avatar subject.",
        ),
        variable(
            "{{avatar_request}}",
            "Avatar Request",
            "The requested avatar prompt direction.",
        ),
    ]
}

fn avatar_edit_request_variables() -> Vec<PromptVariableDefinition> {
    vec![
        variable(
            "{{avatar_subject_name}}",
            "Avatar Subject Name",
            "Name of the character or persona the avatar is for.",
        ),
        variable(
            "{{avatar_subject_description}}",
            "Avatar Subject Description",
            "Description of the avatar subject.",
        ),
        variable(
            "{{current_avatar_prompt}}",
            "Current Avatar Prompt",
            "The prompt used to create the current avatar.",
        ),
        variable(
            "{{edit_request}}",
            "Edit Request",
            "Requested changes for the avatar.",
        ),
    ]
}

fn scene_generation_variables() -> Vec<PromptVariableDefinition> {
    vec![
        variable(
            "{{char.name}}",
            "Character Name",
            "The active character name.",
        ),
        variable(
            "{{char.desc}}",
            "Character Definition",
            "The active character definition.",
        ),
        variable("{{persona.name}}", "User Name", "The active persona name."),
        variable(
            "{{persona.desc}}",
            "User Description",
            "The active persona description.",
        ),
        variable(
            "{{image[character]}}",
            "Character Reference Image",
            "Injected image attachment for the character reference.",
        ),
        variable(
            "{{reference[character]}}",
            "Character Reference Text",
            "Rendered text notes for the character design reference.",
        ),
        variable(
            "{{image[persona]}}",
            "Persona Reference Image",
            "Injected image attachment for the persona reference.",
        ),
        variable(
            "{{reference[persona]}}",
            "Persona Reference Text",
            "Rendered text notes for the persona design reference.",
        ),
        variable(
            "{{image[chatBackground]}}",
            "Chat Background Image",
            "Injected image attachment for the chat background reference.",
        ),
        variable(
            "{{reference[chatBackground]}}",
            "Chat Background Text",
            "Rendered text notes for the background/environment reference.",
        ),
        variable(
            "{{recent_messages}}",
            "Recent Messages",
            "Recent chat lines used to derive the scene.",
        ),
        variable(
            "{{scene_request}}",
            "Scene Request",
            "Manual or automatic scene image request.",
        ),
    ]
}

fn design_reference_writer_variables() -> Vec<PromptVariableDefinition> {
    vec![
        variable(
            "{{subject_name}}",
            "Subject Name",
            "Name of the subject being described.",
        ),
        variable(
            "{{subject_description}}",
            "Subject Context",
            "Character or persona context that informs the design notes.",
        ),
        variable(
            "{{current_description}}",
            "Current Notes",
            "Existing design notes to refine.",
        ),
        variable(
            "{{image[avatar]}}",
            "Avatar Image",
            "Injected image attachment for the subject avatar.",
        ),
        variable(
            "{{image[references]}}",
            "Reference Images",
            "Injected image attachments for supporting design references.",
        ),
    ]
}

fn companion_soul_writer_variables() -> Vec<PromptVariableDefinition> {
    vec![
        variable(
            "{{char.name}}",
            "Character Name",
            "Name of the companion character.",
        ),
        variable(
            "{{char.definition}}",
            "Character Definition",
            "Full character definition or card text.",
        ),
        variable(
            "{{char.description}}",
            "Character Description",
            "Shorter public-facing character description.",
        ),
        variable(
            "{{opening_context}}",
            "Opening Context",
            "Starting scene or context used to infer companion grounding.",
        ),
        variable(
            "{{current_soul}}",
            "Current Soul",
            "Existing companion Soul draft as JSON.",
        ),
        variable(
            "{{user_notes}}",
            "User Notes",
            "User direction for how the Soul should be drafted or revised.",
        ),
    ]
}

fn dedupe_variables(
    groups: impl IntoIterator<Item = Vec<PromptVariableDefinition>>,
) -> Vec<PromptVariableDefinition> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for group in groups {
        for variable in group {
            if seen.insert(variable.variable.clone()) {
                out.push(variable);
            }
        }
    }
    out
}

pub fn prompt_type_label(prompt_type: PromptTemplateType) -> &'static str {
    match prompt_type {
        PromptTemplateType::Undefined => "Undefined",
        PromptTemplateType::DirectChat => "Direct Chat",
        PromptTemplateType::CompanionChat => "Companion Chat",
        PromptTemplateType::GroupChatRoleplay => "Group Chat (Roleplay)",
        PromptTemplateType::GroupChatConversational => "Group Chat (Conversation)",
        PromptTemplateType::DynamicMemorySummarizer => "Dynamic Memory Summarizer",
        PromptTemplateType::DynamicMemoryManager => "Dynamic Memory Manager",
        PromptTemplateType::ReplyHelperRoleplay => "Reply Helper (Roleplay)",
        PromptTemplateType::ReplyHelperConversational => "Reply Helper (Conversational)",
        PromptTemplateType::LorebookEntryWriter => "Lorebook Entry Writer",
        PromptTemplateType::LorebookKeywordGenerator => "Lorebook Keyword Generator",
        PromptTemplateType::AvatarGeneration => "Avatar Generation",
        PromptTemplateType::AvatarEditRequest => "Avatar Edit Request",
        PromptTemplateType::SceneGeneration => "Scene Generation",
        PromptTemplateType::ScenePromptWriter => "Scene Prompt Writer",
        PromptTemplateType::DesignReferenceWriter => "Design Reference Writer",
        PromptTemplateType::CompanionSoulWriter => "Companion Soul Writer",
    }
}

pub fn allowed_variables_for_prompt_type(
    prompt_type: PromptTemplateType,
) -> Vec<PromptVariableDefinition> {
    match prompt_type {
        PromptTemplateType::Undefined => dedupe_variables([
            direct_chat_variables(),
            group_chat_conversational_variables(),
            group_chat_roleplay_variables(),
            dynamic_memory_summarizer_variables(),
            dynamic_memory_manager_variables(),
            reply_helper_variables(),
            avatar_generation_variables(),
            avatar_edit_request_variables(),
            scene_generation_variables(),
            scene_generation_variables(),
            design_reference_writer_variables(),
            companion_soul_writer_variables(),
        ]),
        PromptTemplateType::DirectChat => direct_chat_variables(),
        PromptTemplateType::CompanionChat => direct_chat_variables(),
        PromptTemplateType::GroupChatRoleplay => group_chat_roleplay_variables(),
        PromptTemplateType::GroupChatConversational => group_chat_conversational_variables(),
        PromptTemplateType::DynamicMemorySummarizer => dynamic_memory_summarizer_variables(),
        PromptTemplateType::DynamicMemoryManager => dynamic_memory_manager_variables(),
        PromptTemplateType::ReplyHelperRoleplay => reply_helper_variables(),
        PromptTemplateType::ReplyHelperConversational => reply_helper_variables(),
        PromptTemplateType::LorebookEntryWriter => lorebook_entry_writer_variables(),
        PromptTemplateType::LorebookKeywordGenerator => lorebook_keyword_generator_variables(),
        PromptTemplateType::AvatarGeneration => avatar_generation_variables(),
        PromptTemplateType::AvatarEditRequest => avatar_edit_request_variables(),
        PromptTemplateType::SceneGeneration => scene_generation_variables(),
        PromptTemplateType::ScenePromptWriter => scene_generation_variables(),
        PromptTemplateType::DesignReferenceWriter => design_reference_writer_variables(),
        PromptTemplateType::CompanionSoulWriter => companion_soul_writer_variables(),
    }
}

pub fn required_variables_for_prompt_type(prompt_type: PromptTemplateType) -> Vec<String> {
    match prompt_type {
        PromptTemplateType::Undefined => Vec::new(),
        PromptTemplateType::DirectChat => vec![
            "{{scene}}".to_string(),
            "{{scene_direction}}".to_string(),
            "{{char.name}}".to_string(),
            "{{char.desc}}".to_string(),
            "{{persona.name}}".to_string(),
            "{{persona.desc}}".to_string(),
            "{{context_summary}}".to_string(),
            "{{key_memories}}".to_string(),
        ],
        PromptTemplateType::CompanionChat => vec![
            "{{char.name}}".to_string(),
            "{{char.desc}}".to_string(),
            "{{persona.name}}".to_string(),
            "{{persona.desc}}".to_string(),
            "{{context_summary}}".to_string(),
            "{{key_memories}}".to_string(),
        ],
        PromptTemplateType::GroupChatRoleplay => vec![
            "{{scene}}".to_string(),
            "{{scene_direction}}".to_string(),
            "{{char.name}}".to_string(),
            "{{char.desc}}".to_string(),
            "{{persona.name}}".to_string(),
            "{{persona.desc}}".to_string(),
            "{{group_characters}}".to_string(),
            "{{context_summary}}".to_string(),
            "{{key_memories}}".to_string(),
        ],
        PromptTemplateType::GroupChatConversational => vec![
            "{{char.name}}".to_string(),
            "{{char.desc}}".to_string(),
            "{{persona.name}}".to_string(),
            "{{persona.desc}}".to_string(),
            "{{group_characters}}".to_string(),
        ],
        PromptTemplateType::DynamicMemorySummarizer => vec!["{{prev_summary}}".to_string()],
        PromptTemplateType::DynamicMemoryManager => vec!["{{max_entries}}".to_string()],
        PromptTemplateType::ReplyHelperRoleplay | PromptTemplateType::ReplyHelperConversational => {
            vec![
                "{{char.name}}".to_string(),
                "{{char.desc}}".to_string(),
                "{{persona.name}}".to_string(),
                "{{persona.desc}}".to_string(),
                "{{current_draft}}".to_string(),
            ]
        }
        PromptTemplateType::LorebookEntryWriter => vec![
            "{{selected_messages}}".to_string(),
            "{{memory_summary}}".to_string(),
            "{{selected_memories}}".to_string(),
            "{{direction_prompt}}".to_string(),
        ],
        PromptTemplateType::LorebookKeywordGenerator => vec![
            "{{entry_content}}".to_string(),
            "{{direction_prompt}}".to_string(),
        ],
        PromptTemplateType::AvatarGeneration => vec!["{{avatar_request}}".to_string()],
        PromptTemplateType::AvatarEditRequest => vec![
            "{{current_avatar_prompt}}".to_string(),
            "{{edit_request}}".to_string(),
        ],
        PromptTemplateType::SceneGeneration | PromptTemplateType::ScenePromptWriter => vec![
            "{{recent_messages}}".to_string(),
            "{{scene_request}}".to_string(),
        ],
        PromptTemplateType::DesignReferenceWriter => vec![
            "{{subject_name}}".to_string(),
            "{{image[avatar]}}".to_string(),
        ],
        PromptTemplateType::CompanionSoulWriter => vec!["{{char.name}}".to_string()],
    }
}

pub fn allowed_image_slots_for_prompt_type(
    prompt_type: PromptTemplateType,
) -> Vec<PromptEntryImageSlot> {
    match prompt_type {
        PromptTemplateType::Undefined => vec![
            PromptEntryImageSlot::Character,
            PromptEntryImageSlot::Persona,
            PromptEntryImageSlot::ChatBackground,
            PromptEntryImageSlot::Avatar,
            PromptEntryImageSlot::References,
        ],
        PromptTemplateType::SceneGeneration | PromptTemplateType::ScenePromptWriter => vec![
            PromptEntryImageSlot::Character,
            PromptEntryImageSlot::Persona,
            PromptEntryImageSlot::ChatBackground,
        ],
        PromptTemplateType::DesignReferenceWriter => {
            vec![
                PromptEntryImageSlot::Avatar,
                PromptEntryImageSlot::References,
            ]
        }
        PromptTemplateType::CompanionSoulWriter => Vec::new(),
        _ => Vec::new(),
    }
}

pub fn validate_required_variables(
    prompt_type: PromptTemplateType,
    content: &str,
) -> Result<(), Vec<String>> {
    let missing = required_variables_for_prompt_type(prompt_type)
        .into_iter()
        .filter(|variable| !content.contains(variable))
        .collect::<Vec<_>>();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing)
    }
}

pub fn build_parameter_engine() -> PromptParameterEngine {
    let prompt_types = [
        PromptTemplateType::Undefined,
        PromptTemplateType::DirectChat,
        PromptTemplateType::CompanionChat,
        PromptTemplateType::GroupChatRoleplay,
        PromptTemplateType::GroupChatConversational,
        PromptTemplateType::DynamicMemorySummarizer,
        PromptTemplateType::DynamicMemoryManager,
        PromptTemplateType::ReplyHelperRoleplay,
        PromptTemplateType::ReplyHelperConversational,
        PromptTemplateType::LorebookEntryWriter,
        PromptTemplateType::LorebookKeywordGenerator,
        PromptTemplateType::AvatarGeneration,
        PromptTemplateType::AvatarEditRequest,
        PromptTemplateType::SceneGeneration,
        PromptTemplateType::ScenePromptWriter,
        PromptTemplateType::DesignReferenceWriter,
        PromptTemplateType::CompanionSoulWriter,
    ]
    .into_iter()
    .map(|prompt_type| PromptTypeDefinition {
        prompt_type,
        label: prompt_type_label(prompt_type).to_string(),
        allowed_variables: allowed_variables_for_prompt_type(prompt_type),
        required_variables: required_variables_for_prompt_type(prompt_type),
        allowed_image_slots: allowed_image_slots_for_prompt_type(prompt_type),
    })
    .collect();

    PromptParameterEngine { prompt_types }
}
