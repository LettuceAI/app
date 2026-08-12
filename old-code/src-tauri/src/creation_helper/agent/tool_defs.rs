use serde_json::{json, Value};

use crate::chat_manager::prompting::prompt_engine;
use crate::chat_manager::tooling::ToolDefinition;

use super::state::TargetKind;
use super::verbs::{allowed_tools, Tool};

pub fn tool_for(tool: Tool) -> ToolDefinition {
    let (description, parameters) = match tool {
        Tool::WriteDefinition => (
            prompt_engine::default_creation_helper_write_definition_description(),
            json!({
                "type": "object",
                "properties": {
                    "definition": { "type": "string" }
                },
                "required": ["definition"]
            }),
        ),
        Tool::WriteScene => (
            prompt_engine::default_creation_helper_write_scene_description(),
            json!({
                "type": "object",
                "properties": {
                    "content": { "type": "string" },
                    "direction": { "type": "string" }
                },
                "required": ["content"]
            }),
        ),
        Tool::WriteLoreEntry => (
            prompt_engine::default_creation_helper_write_lore_entry_description(),
            json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["title", "content"]
            }),
        ),

        Tool::SetName => (
            "Set the character/persona/lorebook name.".into(),
            json!({
                "type": "object",
                "properties": { "name": { "type": "string" } },
                "required": ["name"]
            }),
        ),
        Tool::SetModel => (
            "Set the default AI model for this character. Call READ_MODELS first \
             if you don't know the available ids.".into(),
            json!({
                "type": "object",
                "properties": { "id": { "type": "string", "description": "Model id." } },
                "required": ["id"]
            }),
        ),
        Tool::SetPrompt => (
            "Set the system prompt template. Call READ_PROMPTS first if needed.".into(),
            json!({
                "type": "object",
                "properties": { "id": { "type": "string", "description": "Prompt template id." } },
                "required": ["id"]
            }),
        ),
        Tool::SetAvatarGradient => (
            "Toggle the avatar gradient overlay.".into(),
            json!({
                "type": "object",
                "properties": { "enabled": { "type": "boolean" } },
                "required": ["enabled"]
            }),
        ),
        Tool::AttachLorebooks => (
            "Attach lorebooks to the character. Call READ_LOREBOOKS first if you \
             don't know the available ids.".into(),
            json!({
                "type": "object",
                "properties": {
                    "ids": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Lorebook ids to attach."
                    }
                },
                "required": ["ids"]
            }),
        ),

        Tool::EditScene => (
            "Rewrite an existing scene by id (visible in the draft summary as sc_*).".into(),
            json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Scene id, e.g. sc_2." },
                    "content": { "type": "string", "description": "New full scene prose." },
                    "direction": { "type": "string" }
                },
                "required": ["id", "content"]
            }),
        ),
        Tool::EditLoreEntry => (
            "Rewrite an existing lorebook entry by id.".into(),
            json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "title": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["id", "title", "content"]
            }),
        ),
        Tool::DeleteScene => (
            "Delete a scene by id.".into(),
            json!({
                "type": "object",
                "properties": { "id": { "type": "string" } },
                "required": ["id"]
            }),
        ),
        Tool::DeleteLoreEntry => (
            "Delete a lorebook entry by id.".into(),
            json!({
                "type": "object",
                "properties": { "id": { "type": "string" } },
                "required": ["id"]
            }),
        ),
        Tool::DeletePersona => (
            "Delete the persona by id.".into(),
            json!({
                "type": "object",
                "properties": { "id": { "type": "string" } },
                "required": ["id"]
            }),
        ),
        Tool::DeleteLorebook => (
            "Delete the lorebook by id.".into(),
            json!({
                "type": "object",
                "properties": { "id": { "type": "string" } },
                "required": ["id"]
            }),
        ),
        Tool::ReorderLoreEntries => (
            "Reorder lorebook entries.".into(),
            json!({
                "type": "object",
                "properties": {
                    "order": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Entry ids in their new order."
                    }
                },
                "required": ["order"]
            }),
        ),

        Tool::GenerateImage => (
            "Generate a fresh avatar/background image from a prompt and apply it to the draft. The character's name and definition are automatically supplied to the avatar pipeline; the prompt argument should be the *visual request* (\"weather-beaten, mid-thirties, in a faded captain's coat, dim tavern lighting\") not a re-statement of the character's biography. Use this for the first image or whenever the user asks to start over from scratch. Use edit_avatar_image instead when the user wants to change details on an existing image.".into(),
            json!({
                "type": "object",
                "properties": {
                    "prompt": { "type": "string", "description": "Visual description for the new image: appearance, expression, clothing, lighting, framing." },
                    "role": {
                        "type": "string",
                        "enum": ["avatar", "background", "persona_avatar"],
                        "description": "Where to apply the generated image. Defaults to avatar."
                    }
                },
                "required": ["prompt"]
            }),
        ),
        Tool::EditAvatarImage => (
            "Iterate on an existing avatar by editing it. Pass the image_id of an image already in the gallery and a short edit_prompt describing what should change (e.g. \"swap the brown jacket for a black trench coat\", \"slightly older, more grey at the temples\"). The character context, the previous render prompt, and the source image (when the image-gen model accepts image input) are supplied automatically. Use this when the user asks for tweaks; use generate_image instead to start from scratch.".into(),
            json!({
                "type": "object",
                "properties": {
                    "image_id": { "type": "string", "description": "Id of the source image from the gallery (e.g. img_a3)." },
                    "edit_prompt": { "type": "string", "description": "What to change in the image. Short and specific." },
                    "role": {
                        "type": "string",
                        "enum": ["avatar", "background", "persona_avatar"],
                        "description": "Where to apply the edited image. Defaults to avatar."
                    }
                },
                "required": ["image_id", "edit_prompt"]
            }),
        ),
        Tool::UseUploadedImage => (
            "Use an image the user uploaded as avatar/background.".into(),
            json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Uploaded image id." },
                    "role": {
                        "type": "string",
                        "enum": ["avatar", "background", "persona_avatar"]
                    }
                },
                "required": ["id"]
            }),
        ),

        Tool::ListModels => (
            "Fetch the list of available AI models. Call before SET_MODEL if needed.".into(),
            json!({ "type": "object", "properties": {} }),
        ),
        Tool::ListPrompts => (
            "Fetch the list of available system prompt templates.".into(),
            json!({ "type": "object", "properties": {} }),
        ),
        Tool::ListPersonas => (
            "Fetch existing personas.".into(),
            json!({ "type": "object", "properties": {} }),
        ),
        Tool::ListLorebooks => (
            "Fetch existing lorebooks.".into(),
            json!({ "type": "object", "properties": {} }),
        ),
        Tool::ListLoreEntries => (
            "Fetch entries of a lorebook.".into(),
            json!({
                "type": "object",
                "properties": { "id": { "type": "string", "description": "Lorebook id." } },
                "required": ["id"]
            }),
        ),

        Tool::ShowPreview => (
            "Render a preview of the current draft to the user.".into(),
            json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "Optional remark shown alongside the preview."
                    }
                }
            }),
        ),
        Tool::RequestConfirmation => (
            "Ask the user to confirm saving the draft.".into(),
            json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "Optional remark shown alongside the confirmation prompt."
                    }
                }
            }),
        ),
    };

    ToolDefinition {
        name: tool.as_str().to_string(),
        description: Some(description.to_string()),
        parameters,
    }
}

pub fn tools_for_target(target: TargetKind) -> Vec<ToolDefinition> {
    allowed_tools(target)
        .iter()
        .copied()
        .map(tool_for)
        .collect()
}

pub fn schema_summary_for_target(target: TargetKind) -> String {
    let mut s = String::new();
    s.push_str("Available tools (call each by its exact name):\n");
    for v in allowed_tools(target) {
        let def = tool_for(*v);
        s.push_str(&format!(
            "  {} — {}\n    args: {}\n",
            def.name,
            def.description.as_deref().unwrap_or("").trim(),
            args_summary(&def.parameters),
        ));
    }
    s
}

fn args_summary(schema: &Value) -> String {
    let Some(props) = schema.get("properties").and_then(Value::as_object) else {
        return "(none)".to_string();
    };
    if props.is_empty() {
        return "(none)".to_string();
    }
    let required: std::collections::HashSet<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    let mut parts = Vec::new();
    for (k, v) in props {
        let ty = v.get("type").and_then(Value::as_str).unwrap_or("any");
        let mark = if required.contains(k.as_str()) {
            ""
        } else {
            "?"
        };
        parts.push(format!("{}{}: {}", k, mark, ty));
    }
    parts.join(", ")
}
