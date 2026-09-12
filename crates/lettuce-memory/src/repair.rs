use lettuce_conversations::{ToolChoice, ToolDefinition, ToolRequest};
use serde_json::json;

use crate::{DynamicMemoryStructuredFallbackFormat, MemoryCategory};

pub const MEMORY_REPAIR_TOOL_NAME: &str = "retag_memory";
pub const MEMORY_REPAIR_TOOL_TEXT_KEYS: [&str; 3] = [
    "memory_repair_tool",
    "memory_repair_text_parameter",
    "memory_repair_category_parameter",
];

/// The categories a memory may carry, in the order legacy listed them.
pub const MEMORY_CATEGORIES: [MemoryCategory; 6] = [
    MemoryCategory::CharacterTrait,
    MemoryCategory::Relationship,
    MemoryCategory::PlotEvent,
    MemoryCategory::WorldDetail,
    MemoryCategory::Preference,
    MemoryCategory::Other,
];

impl MemoryCategory {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CharacterTrait => "character_trait",
            Self::Relationship => "relationship",
            Self::PlotEvent => "plot_event",
            Self::WorldDetail => "world_detail",
            Self::Preference => "preference",
            Self::Other => "other",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        MEMORY_CATEGORIES
            .into_iter()
            .find(|category| category.as_str() == value)
    }
}

/// The single-tool contract legacy used to re-tag memories whose category the
/// manager left out or invented.
#[must_use]
pub fn memory_repair_tool_request(text: &dyn Fn(&str) -> String) -> ToolRequest {
    ToolRequest {
        definitions: vec![ToolDefinition {
            name: MEMORY_REPAIR_TOOL_NAME.to_owned(),
            description: Some(text("memory_repair_tool")),
            parameters: json!({
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": text("memory_repair_text_parameter")
                    },
                    "category": {
                        "type": "string",
                        "enum": MEMORY_CATEGORIES.map(MemoryCategory::as_str),
                        "description": text("memory_repair_category_parameter")
                    }
                },
                "required": ["text", "category"]
            }),
            version: 1,
        }],
        choice: ToolChoice::Required,
    }
}

/// Runtime catalog key of the repair fallback instruction.
#[must_use]
pub const fn memory_repairs_fallback_prompt_key(
    format: DynamicMemoryStructuredFallbackFormat,
) -> &'static str {
    match format {
        DynamicMemoryStructuredFallbackFormat::Json => "memory_repair_fallback_json",
        DynamicMemoryStructuredFallbackFormat::Xml => "memory_repair_fallback_xml",
    }
}

/// Legacy's keyword buckets, used when a repair request answers with nothing.
#[must_use]
pub fn guess_memory_category(text: &str) -> MemoryCategory {
    const BUCKETS: [(MemoryCategory, &[&str]); 5] = [
        (
            MemoryCategory::Preference,
            &[
                "prefer",
                "preference",
                "likes",
                "dislikes",
                "favorite",
                "boundary",
                "request",
                "wants",
                "doesn't want",
                "does not want",
            ],
        ),
        (
            MemoryCategory::Relationship,
            &[
                "friend",
                "ally",
                "enemy",
                "trust",
                "relationship",
                "bond",
                "dating",
                "married",
                "siblings",
                "partners",
            ],
        ),
        (
            MemoryCategory::WorldDetail,
            &[
                "city", "town", "kingdom", "forest", "artifact", "magic", "rule", "world",
                "location", "village",
            ],
        ),
        (
            MemoryCategory::PlotEvent,
            &[
                "decided",
                "chose",
                "agreed",
                "arrived",
                "left",
                "found",
                "discovered",
                "promised",
                "killed",
                "saved",
                "escaped",
            ],
        ),
        (
            MemoryCategory::CharacterTrait,
            &[
                "afraid",
                "fear",
                "goal",
                "trait",
                "personality",
                "backstory",
                "secret",
                "revealed",
                "believes",
                "hates",
                "loves",
            ],
        ),
    ];
    let lower = text.to_ascii_lowercase();
    BUCKETS
        .into_iter()
        .find(|(_, keywords)| keywords.iter().any(|keyword| lower.contains(keyword)))
        .map_or(MemoryCategory::Other, |(category, _)| category)
}

#[cfg(test)]
mod tests {
    use super::{MEMORY_CATEGORIES, guess_memory_category, memory_repair_tool_request};
    use crate::MemoryCategory;

    #[test]
    fn the_repair_tool_matches_the_legacy_contract() {
        let request = memory_repair_tool_request(&|key| key.to_owned());
        request.validate().expect("valid repair tool");
        assert_eq!(request.choice, lettuce_conversations::ToolChoice::Required);
        assert_eq!(request.definitions[0].name, "retag_memory");
        assert_eq!(
            request.definitions[0].parameters["properties"]["category"]["enum"],
            serde_json::json!([
                "character_trait",
                "relationship",
                "plot_event",
                "world_detail",
                "preference",
                "other"
            ])
        );
        assert_eq!(
            request.definitions[0].parameters["required"],
            serde_json::json!(["text", "category"])
        );
        assert_eq!(MEMORY_CATEGORIES.len(), 6);
        assert_eq!(
            MemoryCategory::parse("plot_event"),
            Some(MemoryCategory::PlotEvent)
        );
        assert_eq!(MemoryCategory::parse("milestone"), None);
    }

    #[test]
    fn the_keyword_guess_follows_the_legacy_bucket_order() {
        assert_eq!(
            guess_memory_category("Mira prefers tea"),
            MemoryCategory::Preference
        );
        assert_eq!(
            guess_memory_category("They trust the captain"),
            MemoryCategory::Relationship
        );
        assert_eq!(
            guess_memory_category("The city burned"),
            MemoryCategory::WorldDetail
        );
        assert_eq!(
            guess_memory_category("She decided to stay"),
            MemoryCategory::PlotEvent
        );
        assert_eq!(
            guess_memory_category("He is afraid of storms"),
            MemoryCategory::CharacterTrait
        );
        assert_eq!(
            guess_memory_category("Nothing matches"),
            MemoryCategory::Other
        );
        assert_eq!(
            guess_memory_category("A friend who prefers tea"),
            MemoryCategory::Preference,
            "the first matching bucket wins"
        );
    }
}
