// Tool definitions for the Creation Helper
//
// These tools are provided to the LLM to build the character progressively.

use super::types::{CreationGoal, CreationMode};
use crate::chat_manager::tooling::ToolDefinition;
use serde_json::json;

fn character_format_guidance() -> &'static str {
    r#"## Character Format Guidance
- Character definitions should be plain prose or short labeled sections, not JSON, XML, YAML, or code blocks
- Focus the definition on stable character information: personality, speaking style, motivations, boundaries, background, competencies, and relationship hooks
- Keep the definition specific and readable; usually a few short paragraphs or concise bullet-like lines is better than a giant wall of text
- Do not write the definition as a first-message scene, chat transcript, or roleplay exchange
- Avoid generic filler like "can talk about anything" unless the user explicitly wants that

## Scene Format Guidance
- A scene's `content` should read like the actual opening message or opening situation the user will start from
- Write scenes in natural roleplay prose/dialogue, not as notes about what the scene should be
- Keep scenes concrete and playable: who is present, what is happening, and what tension or hook starts the interaction
- Prefer one strong opening scene over several weak ones
- Put meta guidance such as pacing, tone, or hidden intent in the scene `direction` field when needed, not inside the visible scene content
- Do not output JSON, speaker labels for both sides, or placeholders like `[Scene starts here]` unless the user explicitly wants them"#
}

/// Get all tool definitions for the creation helper
pub fn get_creation_helper_tools(
    goal: &CreationGoal,
    _smart_selection: bool,
) -> Vec<ToolDefinition> {
    match goal {
        CreationGoal::Character => character_tools(),
        CreationGoal::Persona => persona_tools(),
        CreationGoal::Lorebook => lorebook_tools(),
    }
}

fn character_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "set_character_name".to_string(),
            description: Some("Set the character's name".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "The character's name"
                    }
                },
                "required": ["name"]
            }),
        },
        ToolDefinition {
            name: "set_character_definition".to_string(),
            description: Some("Set or update the character's definition/personality".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "definition": {
                        "type": "string",
                        "description": "The character's definition, personality, and background"
                    }
                },
                "required": ["definition"]
            }),
        },
        ToolDefinition {
            name: "add_scene".to_string(),
            description: Some("Add a starting scene for the character. Scenes set the initial context for roleplay.".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The scene content - the opening message or situation"
                    },
                    "direction": {
                        "type": "string",
                        "description": "Optional direction/context for how the scene should play out"
                    }
                },
                "required": ["content"]
            }),
        },
        ToolDefinition {
            name: "update_scene".to_string(),
            description: Some("Update an existing scene".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "scene_id": {
                        "type": "string",
                        "description": "The ID of the scene to update"
                    },
                    "content": {
                        "type": "string",
                        "description": "The new scene content"
                    },
                    "direction": {
                        "type": "string",
                        "description": "Optional new direction for the scene"
                    }
                },
                "required": ["scene_id", "content"]
            }),
        },
        ToolDefinition {
            name: "toggle_avatar_gradient".to_string(),
            description: Some("Enable or disable the gradient overlay on the character's avatar".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "enabled": {
                        "type": "boolean",
                        "description": "Whether the gradient should be enabled (true) or disabled (false)"
                    }
                },
                "required": ["enabled"]
            }),
        },
        ToolDefinition {
            name: "set_default_model".to_string(),
            description: Some("Set which AI model this character should use for conversations".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "model_id": {
                        "type": "string",
                        "description": "The ID of the model to use"
                    }
                },
                "required": ["model_id"]
            }),
        },
        ToolDefinition {
            name: "set_system_prompt".to_string(),
            description: Some("Set the system prompt template for this character".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "prompt_id": {
                        "type": "string",
                        "description": "The ID of the system prompt template to use"
                    }
                },
                "required": ["prompt_id"]
            }),
        },
        ToolDefinition {
            name: "get_system_prompt_list".to_string(),
            description: Some("Get the list of available system prompt templates".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "get_model_list".to_string(),
            description: Some("Get the list of available AI models".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "use_uploaded_image_as_avatar".to_string(),
            description: Some("Use an image that the user uploaded in the chat as the character's avatar".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "image_id": {
                        "type": "string",
                        "description": "The ID of the uploaded image to use as avatar"
                    }
                },
                "required": ["image_id"]
            }),
        },
        ToolDefinition {
            name: "use_uploaded_image_as_chat_background".to_string(),
            description: Some("Use an image that the user uploaded in the chat as the chat background".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "image_id": {
                        "type": "string",
                        "description": "The ID of the uploaded image to use as background"
                    }
                },
                "required": ["image_id"]
            }),
        },
        ToolDefinition {
            name: "generate_image".to_string(),
            description: Some("Generate an image for the character (avatar/background) based on a prompt.".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "prompt": {
                        "type": "string",
                        "description": "Prompt describing the image to generate"
                    },
                    "size": {
                        "type": "string",
                        "description": "Optional image size (e.g., 1024x1024)"
                    },
                    "quality": {
                        "type": "string",
                        "description": "Optional quality setting for supported providers"
                    },
                    "style": {
                        "type": "string",
                        "description": "Optional style setting for supported providers"
                    }
                },
                "required": ["prompt"]
            }),
        },
        ToolDefinition {
            name: "show_preview".to_string(),
            description: Some("Show a preview of the character to the user. Use this when you have enough information to show them what the character looks like.".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "A message to show alongside the preview, e.g. 'Here's what your character looks like so far!'"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "request_confirmation".to_string(),
            description: Some("Ask the user if they want to save the character or continue editing. Use this when the character seems complete.".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "A message asking for confirmation, e.g. 'Are you happy with this character?'"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "list_character_lorebooks".to_string(),
            description: Some("List lorebooks assigned to a character.".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "character_id": {
                        "type": "string",
                        "description": "Character ID"
                    }
                },
                "required": ["character_id"]
            }),
        },
        ToolDefinition {
            name: "set_character_lorebooks".to_string(),
            description: Some("Assign lorebooks to a character.".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "character_id": {
                        "type": "string",
                        "description": "Character ID"
                    },
                    "lorebook_ids": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Lorebook IDs to assign"
                    }
                },
                "required": ["character_id", "lorebook_ids"]
            }),
        },
    ]
}

/// Get the system prompt for the creation helper
pub fn get_creation_helper_system_prompt(
    goal: &CreationGoal,
    mode: &CreationMode,
    target_type: Option<&CreationGoal>,
    target_id: Option<&str>,
    _smart_selection: bool,
) -> String {
    let mut base = match goal {
        CreationGoal::Character => format!(
            r#"You are a character creation assistant for a roleplay app. Your goal is to help the user create a compelling character through conversation.

## Your Approach
1. Start by asking what kind of character they want to create
2. Ask follow-up questions to understand their vision (personality, background, appearance, etc.)
3. Use the available tools to build the character progressively as you learn more
4. Be creative and suggest ideas, but always respect the user's preferences
5. When you have enough information, show a preview and ask for confirmation

## Guidelines
- Keep responses conversational and helpful
- Don't ask too many questions at once - 1-2 questions per message is ideal
- Use tools proactively - set the name as soon as you know it, build the definition incrementally
- For scenes, help them craft engaging opening scenarios
- If they upload an image, ask if they want to use it as an avatar or background

## Tools Available
- set_character_name: Set the name
- set_character_definition: Build/update the definition
- add_scene: Add starting scenes (the opening message/situation)
- update_scene: Modify existing scenes
- toggle_avatar_gradient: Control the avatar visual style
- set_default_model: Set which AI model powers conversations
- set_system_prompt: Set behavioral guidelines
- get_system_prompt_list: See available prompts
- get_model_list: See available models
- use_uploaded_image_as_avatar: Use an uploaded image as avatar
- use_uploaded_image_as_chat_background: Use an uploaded image as background
- generate_image: Generate an image using the image model
- show_preview: Let them see the character so far
- request_confirmation: Ask if they're ready to save
- list_character_lorebooks: List lorebooks on the character
- set_character_lorebooks: Assign lorebooks to the character

{}

Remember: You're helping create a character for roleplay. Make the process fun and collaborative!"#,
            character_format_guidance()
        ),
        CreationGoal::Persona => r#"You are a persona creation assistant. Your goal is to help the user craft a reusable persona (their voice, style, or identity) through conversation.

## Your Approach
1. Ask what the persona should represent (voice, background, tone, goals)
2. Ask follow-up questions to clarify style, preferences, and boundaries
3. Use the tools to create or update the persona once you have enough detail
4. Show a preview and ask if they'd like to keep editing or review it

## Guidelines
- Keep responses conversational and helpful
- Ask 1-2 questions at a time
- Use tools proactively once details are clear

## Tools Available
- list_personas: List existing personas
- upsert_persona: Create or update a persona
- use_uploaded_image_as_persona_avatar: Set a persona avatar from an uploaded image
- generate_image: Generate a persona avatar image
- show_preview: Let them see the persona so far
- request_confirmation: Ask if they're ready to review/save
- delete_persona: Remove a persona
- get_default_persona: Check the current default persona

Remember: Personas define how the user wants to show up in chats. Make the process fun and collaborative!"#
            .to_string(),
        CreationGoal::Lorebook => r#"You are a lorebook creation assistant. Your goal is to help the user build a world lorebook and its entries.

## Your Approach
1. Ask about the lorebook theme, scope, and key topics
2. Gather details for entries (titles, keywords, content, always-active rules)
3. Use the tools to create or update the lorebook and entries
4. Show a preview and ask if they'd like to keep editing or review it

## Guidelines
- Keep responses conversational and helpful
- Ask 1-2 questions at a time
- Use tools proactively when details are clear

## Tools Available
- list_lorebooks: List existing lorebooks
- upsert_lorebook: Create or update a lorebook
- delete_lorebook: Remove a lorebook
- list_lorebook_entries: List entries for a lorebook
- get_lorebook_entry: Fetch a specific entry
- upsert_lorebook_entry: Create or update an entry
- delete_lorebook_entry: Remove an entry
- create_blank_lorebook_entry: Create a placeholder entry for edits
- reorder_lorebook_entries: Adjust entry ordering
- show_preview: Let them see the lorebook so far
- request_confirmation: Ask if they're ready to review/save

Remember: Lorebooks should be clear, scannable, and useful in roleplay. Make the process fun and collaborative!"#
            .to_string(),
    };

    if *mode == CreationMode::Edit {
        let target_type_text = target_type
            .map(|t| match t {
                CreationGoal::Character => "character",
                CreationGoal::Persona => "persona",
                CreationGoal::Lorebook => "lorebook",
            })
            .unwrap_or("entity");
        let target_id_text = target_id.unwrap_or("unknown");
        base.push_str(
            format!(
                "\n\n## Edit Mode\nYou are editing an existing {} with ID `{}`.\nDo not create a new entity unless the user explicitly asks for a duplicate.\nPrefer update operations for this target and preserve existing intent/style.",
                target_type_text, target_id_text
            )
            .as_str(),
        );
    }

    base
}

fn persona_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "list_personas".to_string(),
            description: Some("List all available personas.".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "upsert_persona".to_string(),
            description: Some("Create or update a persona.".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Optional persona ID (omit to create a new one)"
                    },
                    "title": {
                        "type": "string",
                        "description": "Persona name/title"
                    },
                    "description": {
                        "type": "string",
                        "description": "Persona description"
                    },
                    "avatar_path": {
                        "type": "string",
                        "description": "Optional avatar path or data URL"
                    },
                    "is_default": {
                        "type": "boolean",
                        "description": "Mark this persona as the default"
                    }
                },
                "required": ["title", "description"]
            }),
        },
        ToolDefinition {
            name: "use_uploaded_image_as_persona_avatar".to_string(),
            description: Some("Use an uploaded image as a persona avatar.".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "persona_id": {
                        "type": "string",
                        "description": "Persona ID to update"
                    },
                    "image_id": {
                        "type": "string",
                        "description": "Uploaded image ID"
                    }
                },
                "required": ["persona_id", "image_id"]
            }),
        },
        ToolDefinition {
            name: "generate_image".to_string(),
            description: Some("Generate a persona avatar image based on a prompt.".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "prompt": {
                        "type": "string",
                        "description": "Prompt describing the image to generate"
                    },
                    "size": {
                        "type": "string",
                        "description": "Optional image size (e.g., 1024x1024)"
                    },
                    "quality": {
                        "type": "string",
                        "description": "Optional quality setting for supported providers"
                    },
                    "style": {
                        "type": "string",
                        "description": "Optional style setting for supported providers"
                    }
                },
                "required": ["prompt"]
            }),
        },
        ToolDefinition {
            name: "show_preview".to_string(),
            description: Some(
                "Show a preview of the persona to the user when you have enough information."
                    .to_string(),
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "A message to show alongside the preview"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "request_confirmation".to_string(),
            description: Some(
                "Ask the user if they want to save the persona or continue editing.".to_string(),
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "A message asking for confirmation"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "delete_persona".to_string(),
            description: Some("Delete a persona by ID.".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Persona ID"
                    }
                },
                "required": ["id"]
            }),
        },
        ToolDefinition {
            name: "get_default_persona".to_string(),
            description: Some("Get the default persona if one exists.".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        },
    ]
}

fn lorebook_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "list_lorebooks".to_string(),
            description: Some("List all lorebooks.".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "upsert_lorebook".to_string(),
            description: Some("Create or update a lorebook.".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Optional lorebook ID (omit to create a new one)"
                    },
                    "name": {
                        "type": "string",
                        "description": "Lorebook name"
                    }
                },
                "required": ["name"]
            }),
        },
        ToolDefinition {
            name: "delete_lorebook".to_string(),
            description: Some("Delete a lorebook by ID.".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "lorebook_id": {
                        "type": "string",
                        "description": "Lorebook ID"
                    }
                },
                "required": ["lorebook_id"]
            }),
        },
        ToolDefinition {
            name: "list_lorebook_entries".to_string(),
            description: Some("List all entries for a lorebook.".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "lorebook_id": {
                        "type": "string",
                        "description": "Lorebook ID"
                    }
                },
                "required": ["lorebook_id"]
            }),
        },
        ToolDefinition {
            name: "get_lorebook_entry".to_string(),
            description: Some("Get a single lorebook entry by ID.".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "entry_id": {
                        "type": "string",
                        "description": "Lorebook entry ID"
                    }
                },
                "required": ["entry_id"]
            }),
        },
        ToolDefinition {
            name: "upsert_lorebook_entry".to_string(),
            description: Some("Create or update a lorebook entry.".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Optional entry ID (omit to create a new one)"
                    },
                    "lorebook_id": {
                        "type": "string",
                        "description": "Lorebook ID"
                    },
                    "title": {
                        "type": "string",
                        "description": "Entry title"
                    },
                    "content": {
                        "type": "string",
                        "description": "Entry content"
                    },
                    "keywords": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Trigger keywords"
                    },
                    "enabled": {
                        "type": "boolean",
                        "description": "Whether the entry is enabled"
                    },
                    "always_active": {
                        "type": "boolean",
                        "description": "Always inject regardless of keywords"
                    },
                    "case_sensitive": {
                        "type": "boolean",
                        "description": "Keyword matching case sensitivity"
                    },
                    "priority": {
                        "type": "integer",
                        "description": "Entry priority"
                    },
                    "display_order": {
                        "type": "integer",
                        "description": "Entry display order"
                    }
                },
                "required": ["lorebook_id", "title", "content"]
            }),
        },
        ToolDefinition {
            name: "delete_lorebook_entry".to_string(),
            description: Some("Delete a lorebook entry by ID.".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "entry_id": {
                        "type": "string",
                        "description": "Lorebook entry ID"
                    }
                },
                "required": ["entry_id"]
            }),
        },
        ToolDefinition {
            name: "create_blank_lorebook_entry".to_string(),
            description: Some("Create a blank lorebook entry for a lorebook.".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "lorebook_id": {
                        "type": "string",
                        "description": "Lorebook ID"
                    }
                },
                "required": ["lorebook_id"]
            }),
        },
        ToolDefinition {
            name: "reorder_lorebook_entries".to_string(),
            description: Some("Reorder lorebook entries.".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "updates": {
                        "type": "array",
                        "description": "List of entry order updates",
                        "items": {
                            "type": "object",
                            "properties": {
                                "entry_id": { "type": "string" },
                                "display_order": { "type": "integer" }
                            },
                            "required": ["entry_id", "display_order"]
                        }
                    }
                },
                "required": ["updates"]
            }),
        },
        ToolDefinition {
            name: "show_preview".to_string(),
            description: Some(
                "Show a preview of the lorebook to the user when you have enough information."
                    .to_string(),
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "A message to show alongside the preview"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "request_confirmation".to_string(),
            description: Some(
                "Ask the user if they want to keep editing or review the lorebook.".to_string(),
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "A message asking for confirmation"
                    }
                }
            }),
        },
    ]
}
