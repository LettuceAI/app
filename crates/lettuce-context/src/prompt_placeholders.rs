//! The placeholder registry the prompt editor and prompt writes use: which
//! placeholders a prompt purpose may use and which it must contain.

use crate::prompt::{PromptEntry, PromptEntryImageSlot, PromptEntryPayload, PromptPurpose};

const TIME: &[&str] = &[
    "{{date}}",
    "{{date_full}}",
    "{{weekday}}",
    "{{time_hour}}",
    "{{time_minute}}",
    "{{time_second}}",
    "{{time_full}}",
    "{{time_12hour_format}}",
    "{{time_timezone}}",
    "{{time_timezone_name}}",
    "{{datetime_iso}}",
];
const DIRECT_CHAT: &[&str] = &[
    "{{char.name}}",
    "{{char.desc}}",
    "{{scene}}",
    "{{scene_direction}}",
    "{{persona.name}}",
    "{{persona.desc}}",
    "{{context_summary}}",
    "{{companion_state}}",
    "{{scheduled_notes}}",
    "{{key_memories}}",
    "{{lorebook}}",
    "{{author_note}}",
    "{{rules}}",
    "{{content_rules}}",
];
const GROUP_CHAT_CONVERSATIONAL: &[&str] = &[
    "{{char.name}}",
    "{{char.desc}}",
    "{{persona.name}}",
    "{{persona.desc}}",
    "{{group_characters}}",
];
const GROUP_CHAT_ROLEPLAY: &[&str] = &[
    "{{scene}}",
    "{{scene_direction}}",
    "{{char.name}}",
    "{{char.desc}}",
    "{{persona.name}}",
    "{{persona.desc}}",
    "{{group_characters}}",
    "{{context_summary}}",
    "{{key_memories}}",
];
const MEMORY_SUMMARIZER: &[&str] = &["{{prev_summary}}", "{{character}}", "{{persona}}"];
const MEMORY_MANAGER: &[&str] = &[
    "{{max_entries}}",
    "{{current_memory_tokens}}",
    "{{hot_token_budget}}",
];
const REPLY_HELPER: &[&str] = &[
    "{{char.name}}",
    "{{char.desc}}",
    "{{persona.name}}",
    "{{persona.desc}}",
    "{{current_draft}}",
];
const LOREBOOK_ENTRY_WRITER: &[&str] = &[
    "{{lorebook_name}}",
    "{{character_name}}",
    "{{session_title}}",
    "{{selected_messages}}",
    "{{memory_summary}}",
    "{{selected_memories}}",
    "{{direction_prompt}}",
    "{{existing_entries}}",
];
const LOREBOOK_KEYWORD_GENERATOR: &[&str] = &[
    "{{entry_title}}",
    "{{entry_content}}",
    "{{existing_keywords}}",
    "{{direction_prompt}}",
];
const LOREBOOK_GENERATOR_PLANNER: &[&str] =
    &["{{brief}}", "{{target_count}}", "{{source_excerpts}}"];
const LOREBOOK_GENERATOR_WRITER: &[&str] = &[
    "{{brief}}",
    "{{outline}}",
    "{{entry_title}}",
    "{{entry_category}}",
    "{{entry_proposed_keys}}",
    "{{entry_rationale}}",
    "{{relevant_excerpts}}",
];
const LOREBOOK_GENERATOR_REFINE: &[&str] = &[
    "{{brief}}",
    "{{outline}}",
    "{{entry_title}}",
    "{{entry_keywords}}",
    "{{entry_always_active}}",
    "{{entry_content}}",
    "{{user_feedback}}",
    "{{relevant_excerpts}}",
];
const LOREBOOK_GENERATOR_COHERENCE: &[&str] = &["{{drafted_entries}}"];
const AVATAR_GENERATION: &[&str] = &[
    "{{avatar_subject_name}}",
    "{{avatar_subject_description}}",
    "{{avatar_request}}",
];
const AVATAR_EDIT_REQUEST: &[&str] = &[
    "{{avatar_subject_name}}",
    "{{avatar_subject_description}}",
    "{{current_avatar_prompt}}",
    "{{edit_request}}",
];
const SCENE_GENERATION: &[&str] = &[
    "{{char.name}}",
    "{{char.desc}}",
    "{{persona.name}}",
    "{{persona.desc}}",
    "{{image[character]}}",
    "{{reference[character]}}",
    "{{image[persona]}}",
    "{{reference[persona]}}",
    "{{image[chatBackground]}}",
    "{{reference[chatBackground]}}",
    "{{recent_messages}}",
    "{{scene_request}}",
];
const DESIGN_REFERENCE_WRITER: &[&str] = &[
    "{{subject_name}}",
    "{{subject_description}}",
    "{{current_description}}",
    "{{image[avatar]}}",
    "{{image[references]}}",
];
const COMPANION_SOUL_WRITER: &[&str] = &[
    "{{char.name}}",
    "{{char.definition}}",
    "{{char.description}}",
    "{{opening_context}}",
    "{{current_soul}}",
    "{{user_notes}}",
];
const COMPANION_GROWTHCYCLE: &[&str] = &[
    "{{companion.name}}",
    "{{changeable_categories}}",
    "{{current_growth}}",
    "{{new_memories}}",
];
const COMPANION_CONSOLIDATION: &[&str] = &[
    "{{companion.name}}",
    "{{authored_core}}",
    "{{current_core}}",
    "{{accumulated_growth}}",
];

/// The placeholders one prompt purpose may use, the ones it must contain and
/// the image slots its entries may carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptPlaceholders {
    pub purpose: PromptPurpose,
    pub allowed: Vec<&'static str>,
    pub required: Vec<&'static str>,
    pub image_slots: Vec<PromptEntryImageSlot>,
}

fn groups(purpose: PromptPurpose) -> Vec<&'static [&'static str]> {
    use PromptPurpose as Purpose;
    match purpose {
        Purpose::Undefined | Purpose::RuntimeText => vec![
            DIRECT_CHAT,
            GROUP_CHAT_CONVERSATIONAL,
            GROUP_CHAT_ROLEPLAY,
            MEMORY_SUMMARIZER,
            MEMORY_MANAGER,
            REPLY_HELPER,
            AVATAR_GENERATION,
            AVATAR_EDIT_REQUEST,
            SCENE_GENERATION,
            DESIGN_REFERENCE_WRITER,
            COMPANION_SOUL_WRITER,
        ],
        Purpose::DirectChat | Purpose::CompanionChat => vec![DIRECT_CHAT],
        Purpose::GroupChatRoleplay => vec![GROUP_CHAT_ROLEPLAY],
        Purpose::GroupChatConversational => vec![GROUP_CHAT_CONVERSATIONAL],
        Purpose::DynamicMemorySummarizer => vec![MEMORY_SUMMARIZER],
        Purpose::DynamicMemoryManager => vec![MEMORY_MANAGER],
        Purpose::ReplyHelperRoleplay | Purpose::ReplyHelperConversational => vec![REPLY_HELPER],
        Purpose::LorebookEntryWriter => vec![LOREBOOK_ENTRY_WRITER],
        Purpose::LorebookKeywordGenerator => vec![LOREBOOK_KEYWORD_GENERATOR],
        Purpose::LorebookGeneratorPlanner => vec![LOREBOOK_GENERATOR_PLANNER],
        Purpose::LorebookGeneratorWriter => vec![LOREBOOK_GENERATOR_WRITER],
        Purpose::LorebookGeneratorRefine => vec![LOREBOOK_GENERATOR_REFINE],
        Purpose::LorebookGeneratorCoherence => vec![LOREBOOK_GENERATOR_COHERENCE],
        Purpose::AvatarGeneration => vec![AVATAR_GENERATION],
        Purpose::AvatarEditRequest => vec![AVATAR_EDIT_REQUEST],
        Purpose::SceneGeneration | Purpose::ScenePromptWriter => vec![SCENE_GENERATION],
        Purpose::DesignReferenceWriter => vec![DESIGN_REFERENCE_WRITER],
        Purpose::CompanionSoulWriter => vec![COMPANION_SOUL_WRITER],
        Purpose::CompanionGrowthcycle => vec![COMPANION_GROWTHCYCLE],
        Purpose::CompanionConsolidation => vec![COMPANION_CONSOLIDATION],
    }
}

fn required(purpose: PromptPurpose) -> &'static [&'static str] {
    use PromptPurpose as Purpose;
    match purpose {
        Purpose::Undefined | Purpose::RuntimeText => &[],
        Purpose::DirectChat => &[
            "{{scene}}",
            "{{scene_direction}}",
            "{{char.name}}",
            "{{char.desc}}",
            "{{persona.name}}",
            "{{persona.desc}}",
            "{{context_summary}}",
            "{{key_memories}}",
        ],
        Purpose::CompanionChat => &[
            "{{char.name}}",
            "{{char.desc}}",
            "{{persona.name}}",
            "{{persona.desc}}",
            "{{context_summary}}",
            "{{key_memories}}",
        ],
        Purpose::GroupChatRoleplay => &[
            "{{scene}}",
            "{{scene_direction}}",
            "{{char.name}}",
            "{{char.desc}}",
            "{{persona.name}}",
            "{{persona.desc}}",
            "{{group_characters}}",
            "{{context_summary}}",
            "{{key_memories}}",
        ],
        Purpose::GroupChatConversational => &[
            "{{char.name}}",
            "{{char.desc}}",
            "{{persona.name}}",
            "{{persona.desc}}",
            "{{group_characters}}",
        ],
        Purpose::DynamicMemorySummarizer => &["{{prev_summary}}"],
        Purpose::DynamicMemoryManager => &["{{max_entries}}"],
        Purpose::ReplyHelperRoleplay | Purpose::ReplyHelperConversational => &[
            "{{char.name}}",
            "{{char.desc}}",
            "{{persona.name}}",
            "{{persona.desc}}",
            "{{current_draft}}",
        ],
        Purpose::LorebookEntryWriter => &[
            "{{selected_messages}}",
            "{{memory_summary}}",
            "{{selected_memories}}",
            "{{direction_prompt}}",
        ],
        Purpose::LorebookKeywordGenerator => &["{{entry_content}}", "{{direction_prompt}}"],
        Purpose::LorebookGeneratorPlanner => {
            &["{{brief}}", "{{target_count}}", "{{source_excerpts}}"]
        }
        Purpose::LorebookGeneratorWriter => &[
            "{{brief}}",
            "{{outline}}",
            "{{entry_title}}",
            "{{entry_category}}",
            "{{entry_proposed_keys}}",
            "{{entry_rationale}}",
            "{{relevant_excerpts}}",
        ],
        Purpose::LorebookGeneratorRefine => &[
            "{{entry_title}}",
            "{{entry_keywords}}",
            "{{entry_content}}",
            "{{entry_always_active}}",
            "{{user_feedback}}",
        ],
        Purpose::LorebookGeneratorCoherence => &["{{drafted_entries}}"],
        Purpose::AvatarGeneration => &["{{avatar_request}}"],
        Purpose::AvatarEditRequest => &["{{current_avatar_prompt}}", "{{edit_request}}"],
        Purpose::SceneGeneration | Purpose::ScenePromptWriter => {
            &["{{recent_messages}}", "{{scene_request}}"]
        }
        Purpose::DesignReferenceWriter => &["{{subject_name}}", "{{image[avatar]}}"],
        Purpose::CompanionSoulWriter => &["{{char.name}}"],
        Purpose::CompanionGrowthcycle => &["{{changeable_categories}}", "{{new_memories}}"],
        Purpose::CompanionConsolidation => &["{{authored_core}}", "{{accumulated_growth}}"],
    }
}

fn image_slots(purpose: PromptPurpose) -> Vec<PromptEntryImageSlot> {
    use PromptEntryImageSlot as Slot;
    use PromptPurpose as Purpose;
    match purpose {
        Purpose::Undefined | Purpose::RuntimeText => vec![
            Slot::Character,
            Slot::Persona,
            Slot::ChatBackground,
            Slot::Avatar,
            Slot::References,
        ],
        Purpose::SceneGeneration | Purpose::ScenePromptWriter => {
            vec![Slot::Character, Slot::Persona, Slot::ChatBackground]
        }
        Purpose::DesignReferenceWriter => vec![Slot::Avatar, Slot::References],
        _ => Vec::new(),
    }
}

/// The registry entry for `purpose`. Every purpose allows the time
/// placeholders; the lists keep their catalog order without duplicates.
#[must_use]
pub fn prompt_placeholders(purpose: PromptPurpose) -> PromptPlaceholders {
    let mut allowed = Vec::new();
    for group in std::iter::once(TIME).chain(groups(purpose)) {
        for placeholder in group {
            if !allowed.contains(placeholder) {
                allowed.push(*placeholder);
            }
        }
    }
    PromptPlaceholders {
        purpose,
        allowed,
        required: required(purpose).to_vec(),
        image_slots: image_slots(purpose),
    }
}

const fn payload_placeholder(payload: &PromptEntryPayload) -> &'static str {
    match payload {
        PromptEntryPayload::ImageSlot { slot } => match slot {
            PromptEntryImageSlot::Character => "{{image[character]}}",
            PromptEntryImageSlot::Persona => "{{image[persona]}}",
            PromptEntryImageSlot::ChatBackground => "{{image[chatBackground]}}",
            PromptEntryImageSlot::Avatar => "{{image[avatar]}}",
            PromptEntryImageSlot::References => "{{image[references]}}",
        },
    }
}

/// The required placeholders of `purpose` that the entries do not contain.
/// Enabled entries and system entries count, with the placeholder of their
/// image payload.
#[must_use]
pub fn missing_required_placeholders(
    purpose: PromptPurpose,
    entries: &[PromptEntry],
) -> Vec<&'static str> {
    let text = entries
        .iter()
        .filter(|entry| entry.enabled || entry.system_prompt)
        .flat_map(|entry| {
            let mut parts = Vec::new();
            if !entry.content.trim().is_empty() {
                parts.push(entry.content.as_str());
            }
            if let Some(payload) = &entry.payload {
                parts.push(payload_placeholder(payload));
            }
            parts
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    required(purpose)
        .iter()
        .copied()
        .filter(|placeholder| !text.contains(placeholder))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(content: &str, enabled: bool, system_prompt: bool) -> PromptEntry {
        PromptEntry {
            content: content.into(),
            enabled,
            system_prompt,
            name: "Entry".into(),
            ..PromptEntry::default()
        }
    }

    #[test]
    fn lorebook_generator_purposes_are_in_the_registry() {
        for (purpose, required) in [
            (
                PromptPurpose::LorebookGeneratorPlanner,
                vec!["{{brief}}", "{{target_count}}", "{{source_excerpts}}"],
            ),
            (
                PromptPurpose::LorebookGeneratorCoherence,
                vec!["{{drafted_entries}}"],
            ),
        ] {
            let registry = prompt_placeholders(purpose);
            assert_eq!(registry.required, required);
            assert!(registry.allowed.contains(&"{{date}}"));
            assert!(
                required
                    .iter()
                    .all(|value| registry.allowed.contains(value))
            );
        }
        assert!(
            prompt_placeholders(PromptPurpose::LorebookGeneratorRefine)
                .allowed
                .contains(&"{{user_feedback}}")
        );
        assert!(
            prompt_placeholders(PromptPurpose::LorebookGeneratorWriter)
                .allowed
                .contains(&"{{entry_rationale}}")
        );
    }

    #[test]
    fn required_placeholders_count_enabled_and_system_entries_and_payloads() {
        let entries = vec![
            entry("{{subject_name}}", true, false),
            entry("{{brief}}", false, false),
        ];
        assert_eq!(
            missing_required_placeholders(PromptPurpose::DesignReferenceWriter, &entries),
            vec!["{{image[avatar]}}"]
        );
        let mut with_image = entry("", true, false);
        with_image.payload = Some(PromptEntryPayload::ImageSlot {
            slot: PromptEntryImageSlot::Avatar,
        });
        assert!(
            missing_required_placeholders(
                PromptPurpose::DesignReferenceWriter,
                &[entries[0].clone(), with_image]
            )
            .is_empty()
        );
        assert_eq!(
            missing_required_placeholders(
                PromptPurpose::LorebookGeneratorPlanner,
                &[entry("{{brief}} {{target_count}}", false, true)]
            ),
            vec!["{{source_excerpts}}"]
        );
        assert_eq!(
            missing_required_placeholders(PromptPurpose::LorebookGeneratorPlanner, &entries),
            vec!["{{brief}}", "{{target_count}}", "{{source_excerpts}}"]
        );
    }
}
