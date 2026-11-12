use lettuce_context::{
    PromptConditionContext, PromptEntryRole, PromptRenderContext, PromptVariable as Variable,
    render_prompt,
};
use lettuce_conversations::MediaAssetRole;
use lettuce_conversations::{
    MessagePart, MessageRole, ProposedToolCall, ProviderContextPart, ProviderNeutralMessage,
    ToolOutput,
};
use lettuce_creation::{
    CreationDialogueTurn, CreationDraft, CreationOperation, CreationOperationError,
    CreationOperationOutcome, CreationReference, CreationRejection, CreationTargetKind,
    CreationTurnInputs, creation_image_label,
};
use lettuce_models::ProviderProtocol;
use lettuce_types::AssetId;
use serde_json::Value;

use crate::generation::runtime_text::{RuntimeText, RuntimeTextError};

/// Every draft-view fragment of `prompt_app_creation_runtime`.
#[cfg(test)]
pub(crate) const DRAFT_VIEW_KEYS: [&str; 20] = [
    "creation_target_character",
    "creation_target_persona",
    "creation_target_lorebook",
    "creation_draft_header",
    "creation_draft_name",
    "creation_draft_unset",
    "creation_draft_definition",
    "creation_draft_definition_unset",
    "creation_draft_scenes",
    "creation_draft_scene",
    "creation_draft_entries",
    "creation_draft_entry",
    "creation_draft_avatar",
    "creation_draft_images",
    "creation_draft_image_uploaded",
    "creation_draft_image_generated",
    "creation_draft_image_edited",
    "creation_draft_image_active",
    "creation_draft_preview",
    "creation_draft_preview_truncated",
];

/// Every tool-result fragment of `prompt_app_creation_runtime`.
#[cfg(test)]
pub(crate) const RESULT_KEYS: [&str; 43] = [
    "creation_result_set_name",
    "creation_result_write_definition",
    "creation_result_write_scene",
    "creation_result_edit_scene",
    "creation_result_delete_scene",
    "creation_result_write_lore_entry",
    "creation_result_edit_lore_entry",
    "creation_result_delete_lore_entry",
    "creation_result_reorder_lore_entries",
    "creation_result_show_preview",
    "creation_result_request_confirmation",
    "creation_result_use_uploaded_image",
    "creation_result_generate_image",
    "creation_result_edit_avatar_image",
    "creation_error_message",
    "creation_error_unknown_tool",
    "creation_error_scene_not_found",
    "creation_error_delete_scene_not_found",
    "creation_error_entry_not_found",
    "creation_error_entries_not_found",
    "creation_error_duplicate_entries",
    "creation_error_blank_text",
    "creation_error_limit",
    "creation_error_wrong_target",
    "creation_error_set_name_name",
    "creation_error_write_definition_definition",
    "creation_error_write_scene_content",
    "creation_error_edit_scene_id",
    "creation_error_edit_scene_content",
    "creation_error_delete_scene_id",
    "creation_error_write_lore_entry_content",
    "creation_error_write_lore_entry_title",
    "creation_error_edit_lore_entry_id",
    "creation_error_edit_lore_entry_title",
    "creation_error_edit_lore_entry_content",
    "creation_error_delete_lore_entry_id",
    "creation_error_reorder_lore_entries_order",
    "creation_error_use_uploaded_image_image_id",
    "creation_error_image_not_found",
    "creation_error_generate_image_prompt",
    "creation_error_edit_avatar_image_image_id",
    "creation_error_edit_avatar_image_edit_prompt",
    "creation_error_image_failed",
];

/// Every fragment of `prompt_app_creation_runtime` that renders what the user attached to a message.
#[cfg(test)]
pub(crate) const INPUT_KEYS: [&str; 6] = [
    "creation_message_attachments",
    "creation_reference_character",
    "creation_reference_persona",
    "creation_reference_no_definition",
    "creation_reference_no_description",
    "creation_uploaded_image",
];

/// Every structured-fallback fragment of `prompt_app_creation_runtime`.
#[cfg(test)]
pub(crate) const FALLBACK_KEYS: [&str; 5] = [
    "creation_fallback_json",
    "creation_fallback_xml",
    "creation_fallback_tools_header",
    "creation_fallback_tool",
    "creation_fallback_no_arguments",
];

const ITEM_PREVIEW_CHARS: usize = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CreationImageOrigin {
    Uploaded,
    Generated,
    Edited,
}

/// An image of the session's gallery, in the order the creator lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CreationPromptImage {
    pub asset_id: AssetId,
    pub origin: CreationImageOrigin,
}

/// A referenced character or persona as it reads now; `text` is its definition or description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreationReferenceText {
    pub name: String,
    pub text: Option<String>,
}

/// The session's images and references for one request. References that no longer exist are left out of `references`
/// and so out of the prompt; uploads become image parts only when the model accepts image input.
#[derive(Debug, Clone, Default)]
pub(crate) struct CreationPromptMedia {
    pub gallery: Vec<CreationPromptImage>,
    pub references: std::collections::HashMap<CreationReference, CreationReferenceText>,
    pub image_input: bool,
}

/// The creation helper's system messages (the `prompt_app_creation_helper`
/// entries with the target label and draft view), the earlier dialogue and the
/// current user message, in that order.
///
/// A structured fallback protocol follows the helper entries as one more
/// system message.
///
/// For the local llama.cpp engine every entry is trimmed and the non-empty ones
/// are joined with a blank line into one leading system message, because chat
/// templates such as Gemma reject a second system message.
#[allow(clippy::too_many_arguments)]
pub(crate) fn creation_context_messages(
    helper: &RuntimeText,
    runtime: &RuntimeText,
    draft: &CreationDraft,
    media: &CreationPromptMedia,
    dialogue: &[CreationDialogueTurn],
    user_message: &str,
    inputs: &CreationTurnInputs,
    protocol: ProviderProtocol,
    fallback_protocol: Option<&str>,
) -> Result<Vec<ProviderNeutralMessage>, RuntimeTextError> {
    let mut values = crate::generation::runtime_text::literal_name_values();
    values
        .purpose_values
        .insert(Variable::TargetLabel, target_label(runtime, draft.kind())?);
    values.purpose_values.insert(
        Variable::DraftState,
        render_draft_view(runtime, draft, &media.gallery)?,
    );
    let rendered = render_prompt(
        helper.document(),
        &PromptRenderContext {
            conditions: PromptConditionContext::default(),
            values,
        },
    )
    .map_err(|_| RuntimeTextError::Render)?;
    let prompt_message = |entry: &lettuce_context::RenderedPromptMessage| {
        (entry.payload.is_none() && !entry.content.trim().is_empty())
            .then(|| text_message(entry_role(entry.role), entry.content.clone()))
    };
    let (mut messages, in_chat) = if protocol == ProviderProtocol::LlamaCpp {
        let merged = rendered
            .relative
            .iter()
            .chain(&rendered.in_chat)
            .filter(|entry| entry.payload.is_none())
            .map(|entry| entry.content.trim())
            .chain(fallback_protocol.map(str::trim))
            .filter(|content| !content.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");
        let leading = (!merged.is_empty()).then(|| text_message(MessageRole::System, merged));
        (leading.into_iter().collect::<Vec<_>>(), Vec::new())
    } else {
        (
            rendered
                .relative
                .iter()
                .filter_map(prompt_message)
                .chain(
                    fallback_protocol
                        .filter(|text| !text.trim().is_empty())
                        .map(|text| text_message(MessageRole::System, text.to_owned())),
                )
                .collect::<Vec<_>>(),
            rendered
                .in_chat
                .iter()
                .filter_map(|entry| prompt_message(entry).map(|message| (entry.depth, message)))
                .collect::<Vec<_>>(),
        )
    };
    for turn in dialogue {
        messages.push(user_turn_message(
            runtime,
            &turn.user_message,
            &turn.inputs,
            media,
        )?);
        let reply = turn
            .assistant_parts
            .iter()
            .filter_map(|part| match part {
                MessagePart::Text { text } if !text.trim().is_empty() => Some(text.trim()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        if !reply.is_empty() {
            messages.push(text_message(MessageRole::Assistant, reply));
        }
    }
    messages.push(user_turn_message(runtime, user_message, inputs, media)?);
    crate::companion::companion_memory_inference::insert_in_chat_messages(&mut messages, in_chat);
    Ok(messages)
}

/// The structured tool-call protocol for a model without native tool calling:
/// the format's instruction followed by every offered tool with its
/// description and argument list.
pub(crate) fn fallback_protocol(
    text: &RuntimeText,
    tools: &lettuce_conversations::ToolRequest,
    format: lettuce_creation::CreationFallbackFormat,
) -> Result<String, RuntimeTextError> {
    let mut summary = text.render_with("creation_fallback_tools_header", [])?;
    summary.push('\n');
    for definition in &tools.definitions {
        let properties = definition
            .parameters
            .get("properties")
            .and_then(Value::as_object)
            .filter(|properties| !properties.is_empty());
        let arguments = match properties {
            None => text.render_with("creation_fallback_no_arguments", [])?,
            Some(properties) => {
                let required = definition
                    .parameters
                    .get("required")
                    .and_then(Value::as_array)
                    .map(|names| names.iter().filter_map(Value::as_str).collect::<Vec<_>>())
                    .unwrap_or_default();
                properties
                    .iter()
                    .map(|(name, schema)| {
                        format!(
                            "{name}{}: {}",
                            if required.contains(&name.as_str()) {
                                ""
                            } else {
                                "?"
                            },
                            schema.get("type").and_then(Value::as_str).unwrap_or("any")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        };
        summary.push_str(
            &text.render_with(
                "creation_fallback_tool",
                [
                    (Variable::ToolName, definition.name.clone()),
                    (
                        Variable::ToolDescription,
                        definition
                            .description
                            .as_deref()
                            .unwrap_or_default()
                            .trim()
                            .to_owned(),
                    ),
                    (Variable::ToolArguments, arguments),
                ],
            )?,
        );
        summary.push('\n');
    }
    text.render_with(
        match format {
            lettuce_creation::CreationFallbackFormat::Json => "creation_fallback_json",
            lettuce_creation::CreationFallbackFormat::Xml => "creation_fallback_xml",
        },
        [(Variable::ToolSchemaSummary, summary)],
    )
}

/// The `DRAFT (...)` summary the model reads in `<current_draft>`.
pub(crate) fn render_draft_view(
    text: &RuntimeText,
    draft: &CreationDraft,
    gallery: &[CreationPromptImage],
) -> Result<String, RuntimeTextError> {
    let mut view = String::new();
    push_line(
        &mut view,
        text.render_with(
            "creation_draft_header",
            [(Variable::TargetLabel, target_label(text, draft.kind())?)],
        )?,
    );
    let (name, definition) = match draft {
        CreationDraft::Character {
            name, definition, ..
        } => (name, definition.as_ref()),
        CreationDraft::Persona {
            name, description, ..
        } => (name, description.as_ref()),
        CreationDraft::Lorebook { name, .. } => (name, None),
    };
    let name = match name {
        Some(name) => name.clone(),
        None => text.render_with("creation_draft_unset", [])?,
    };
    push_line(
        &mut view,
        text.render_with("creation_draft_name", [(Variable::DraftName, name)])?,
    );
    push_line(
        &mut view,
        match definition {
            Some(definition) => text.render_with(
                "creation_draft_definition",
                [
                    (Variable::TextChars, definition.chars().count().to_string()),
                    (Variable::TextQuote, full_text(text, definition)?),
                ],
            )?,
            None => text.render_with("creation_draft_definition_unset", [])?,
        },
    );
    match draft {
        CreationDraft::Character { scenes, .. } if !scenes.is_empty() => {
            push_line(&mut view, text.render_with("creation_draft_scenes", [])?);
            for scene in scenes {
                push_line(
                    &mut view,
                    text.render_with(
                        "creation_draft_scene",
                        [
                            (Variable::DraftItemId, scene.id.to_string()),
                            (
                                Variable::TextChars,
                                scene.content.chars().count().to_string(),
                            ),
                            (Variable::TextQuote, full_text(text, &scene.content)?),
                        ],
                    )?,
                );
            }
        }
        CreationDraft::Lorebook { entries, .. } if !entries.is_empty() => {
            push_line(&mut view, text.render_with("creation_draft_entries", [])?);
            for entry in entries {
                push_line(
                    &mut view,
                    text.render_with(
                        "creation_draft_entry",
                        [
                            (Variable::DraftItemId, entry.id.to_string()),
                            (Variable::EntryTitle, entry.title.clone()),
                            (
                                Variable::TextChars,
                                entry.content.chars().count().to_string(),
                            ),
                            (
                                Variable::TextQuote,
                                preview(text, &entry.content, ITEM_PREVIEW_CHARS)?,
                            ),
                        ],
                    )?,
                );
            }
        }
        CreationDraft::Character { .. }
        | CreationDraft::Persona { .. }
        | CreationDraft::Lorebook { .. } => {}
    }
    let avatar = draft.avatar();
    if let Some(avatar) = avatar {
        push_line(
            &mut view,
            text.render_with(
                "creation_draft_avatar",
                [(Variable::DraftItemId, creation_image_label(avatar))],
            )?,
        );
    }
    if !gallery.is_empty() {
        push_line(&mut view, text.render_with("creation_draft_images", [])?);
        for (index, image) in gallery.iter().enumerate() {
            let mut line = text.render_with(
                match image.origin {
                    CreationImageOrigin::Uploaded => "creation_draft_image_uploaded",
                    CreationImageOrigin::Generated => "creation_draft_image_generated",
                    CreationImageOrigin::Edited => "creation_draft_image_edited",
                },
                [
                    (Variable::ItemNumber, (index + 1).to_string()),
                    (Variable::DraftItemId, creation_image_label(image.asset_id)),
                ],
            )?;
            if avatar == Some(image.asset_id) {
                line.push_str(&text.render_with("creation_draft_image_active", [])?);
            }
            push_line(&mut view, line);
        }
    }
    Ok(view)
}

/// The tool result for one call: `{success, message, error?, ...}`; preview
/// and confirmation also carry `action` and the draft.
/// The answer to one tool call. `image` is what an image call produced: the image, or `None` when it could not be
/// made.
pub(crate) fn render_tool_output(
    text: &RuntimeText,
    call: &ProposedToolCall,
    outcome: &CreationOperationOutcome,
    draft: &CreationDraft,
    image: Option<AssetId>,
) -> Result<ToolOutput, RuntimeTextError> {
    if outcome.error.is_none()
        && matches!(outcome.operation, CreationOperation::GenerateImage { .. })
        && image.is_none()
    {
        let reason = text.render_with("creation_error_image_failed", [])?;
        return Ok(ToolOutput {
            value: serde_json::json!({
                "success": false,
                "message": text.render_with(
                    "creation_error_message",
                    [(Variable::ToolError, reason.clone())],
                )?,
                "error": reason,
            }),
            is_error: true,
        });
    }
    let Some(error) = outcome.error else {
        let mut value = serde_json::Map::new();
        value.insert("success".to_owned(), Value::Bool(true));
        let message = match &outcome.operation {
            CreationOperation::SetName { value } => text.render_with(
                "creation_result_set_name",
                [(Variable::DraftName, value.clone())],
            )?,
            CreationOperation::SetDescription { .. } | CreationOperation::ReplaceDraft { .. } => {
                text.render_with("creation_result_write_definition", [])?
            }
            CreationOperation::AddScene { id, .. } => {
                value.insert("scene_id".to_owned(), Value::String(id.to_string()));
                text.render_with("creation_result_write_scene", [])?
            }
            CreationOperation::UpdateScene { .. } => {
                text.render_with("creation_result_edit_scene", [])?
            }
            CreationOperation::DeleteScene { id } => text.render_with(
                "creation_result_delete_scene",
                [(Variable::DraftItemId, id.to_string())],
            )?,
            CreationOperation::UpsertLorebookEntry { id, title, content } => {
                value.insert(
                    "entry".to_owned(),
                    serde_json::json!({ "id": id, "title": title, "content": content }),
                );
                text.render_with("creation_result_write_lore_entry", [])?
            }
            CreationOperation::UpdateLorebookEntry { id, title, content } => {
                value.insert(
                    "entry".to_owned(),
                    serde_json::json!({ "id": id, "title": title, "content": content }),
                );
                text.render_with("creation_result_edit_lore_entry", [])?
            }
            CreationOperation::DeleteLorebookEntry { .. } => {
                text.render_with("creation_result_delete_lore_entry", [])?
            }
            CreationOperation::ReorderLorebookEntries { .. } => {
                text.render_with("creation_result_reorder_lore_entries", [])?
            }
            CreationOperation::SetAvatar { asset_id } => {
                if let Some(asset_id) = asset_id {
                    value.insert(
                        "image_id".to_owned(),
                        Value::String(creation_image_label(*asset_id)),
                    );
                }
                text.render_with("creation_result_use_uploaded_image", [])?
            }
            CreationOperation::GenerateImage { source, .. } => {
                if let Some(image) = image {
                    value.insert(
                        "image_id".to_owned(),
                        Value::String(creation_image_label(image)),
                    );
                }
                text.render_with(
                    if source.is_some() {
                        "creation_result_edit_avatar_image"
                    } else {
                        "creation_result_generate_image"
                    },
                    [],
                )?
            }
            CreationOperation::ShowPreview | CreationOperation::RequestConfirmation => {
                let action = if matches!(outcome.operation, CreationOperation::ShowPreview) {
                    "show_preview"
                } else {
                    "request_confirmation"
                };
                value.insert("action".to_owned(), Value::String(action.to_owned()));
                value.insert(
                    "draft".to_owned(),
                    serde_json::to_value(draft).map_err(|_| RuntimeTextError::Render)?,
                );
                match call
                    .arguments
                    .get("message")
                    .and_then(Value::as_str)
                    .filter(|message| !message.trim().is_empty())
                {
                    Some(message) => message.to_owned(),
                    None => text.render_with(&format!("creation_result_{action}"), [])?,
                }
            }
            CreationOperation::UndeclaredTool { .. } | CreationOperation::Rejected { .. } => {
                return Err(RuntimeTextError::Render);
            }
        };
        value.insert("message".to_owned(), Value::String(message));
        let mut output = ToolOutput {
            value: Value::Object(value),
            is_error: false,
        };
        if output.validate().is_err()
            && let Some(object) = output.value.as_object_mut()
        {
            object.remove("draft");
        }
        return Ok(output);
    };
    let tool = lettuce_creation::canonical_tool_name(&call.name);
    let named = |key: &str| text.render_with(key, [(Variable::ToolName, tool.clone())]);
    let item = |key: &str, id: String| text.render_with(key, [(Variable::DraftItemId, id)]);
    let reason = match (&outcome.operation, error) {
        (CreationOperation::UndeclaredTool { name }, _) => {
            let error = text.render_with(
                "creation_error_unknown_tool",
                [(Variable::ToolName, name.clone())],
            )?;
            return Ok(ToolOutput {
                value: serde_json::json!({ "success": false, "error": error }),
                is_error: true,
            });
        }
        (
            CreationOperation::Rejected {
                tool,
                reason: CreationRejection::MissingArgument { argument },
            },
            _,
        ) => text.render_with(&format!("creation_error_{tool}_{argument}"), [])?,
        (
            CreationOperation::Rejected {
                tool,
                reason: CreationRejection::UnknownId { id },
            },
            _,
        ) => match tool.as_str() {
            "edit_scene" => text.render_with("creation_error_scene_not_found", [])?,
            "delete_scene" => item("creation_error_delete_scene_not_found", id.clone())?,
            "reorder_lore_entries" => text.render_with("creation_error_entries_not_found", [])?,
            "use_uploaded_image" | "edit_avatar_image" => {
                item("creation_error_image_not_found", id.clone())?
            }
            _ => item("creation_error_entry_not_found", id.clone())?,
        },
        (CreationOperation::UpdateScene { .. }, CreationOperationError::NotFound) => {
            text.render_with("creation_error_scene_not_found", [])?
        }
        (CreationOperation::DeleteScene { id }, CreationOperationError::NotFound) => {
            item("creation_error_delete_scene_not_found", id.to_string())?
        }
        (
            CreationOperation::UpdateLorebookEntry { id, .. }
            | CreationOperation::DeleteLorebookEntry { id },
            CreationOperationError::NotFound,
        ) => item("creation_error_entry_not_found", id.to_string())?,
        (_, CreationOperationError::NotFound) => {
            text.render_with("creation_error_entries_not_found", [])?
        }
        (_, CreationOperationError::DuplicateIdentity) => {
            text.render_with("creation_error_duplicate_entries", [])?
        }
        (_, CreationOperationError::InvalidText) => named("creation_error_blank_text")?,
        (_, CreationOperationError::LimitExceeded) => named("creation_error_limit")?,
        (_, CreationOperationError::WrongTarget) => named("creation_error_wrong_target")?,
        (_, CreationOperationError::UnknownTool | CreationOperationError::InvalidArguments) => {
            named("creation_error_wrong_target")?
        }
    };
    Ok(ToolOutput {
        value: serde_json::json!({
            "success": false,
            "message": text.render_with(
                "creation_error_message",
                [(Variable::ToolError, reason.clone())],
            )?,
            "error": reason,
        }),
        is_error: true,
    })
}

fn target_label(text: &RuntimeText, kind: CreationTargetKind) -> Result<String, RuntimeTextError> {
    text.render_with(
        match kind {
            CreationTargetKind::Character => "creation_target_character",
            CreationTargetKind::Persona => "creation_target_persona",
            CreationTargetKind::Lorebook => "creation_target_lorebook",
        },
        [],
    )
}

fn full_text(text: &RuntimeText, value: &str) -> Result<String, RuntimeTextError> {
    text.render_with(
        "creation_draft_preview",
        [(Variable::TextPreview, value.to_owned())],
    )
}

fn preview(text: &RuntimeText, value: &str, max: usize) -> Result<String, RuntimeTextError> {
    let shown = value
        .chars()
        .take(max)
        .collect::<String>()
        .replace('\n', " ");
    text.render_with(
        if value.chars().count() > max {
            "creation_draft_preview_truncated"
        } else {
            "creation_draft_preview"
        },
        [(Variable::TextPreview, shown)],
    )
}

fn push_line(view: &mut String, line: String) {
    if !line.is_empty() {
        view.push_str(&line);
        view.push('\n');
    }
}

const fn entry_role(role: PromptEntryRole) -> MessageRole {
    match role {
        PromptEntryRole::System => MessageRole::System,
        PromptEntryRole::User => MessageRole::User,
        PromptEntryRole::Assistant => MessageRole::Assistant,
    }
}

/// A user message with what was attached to it: the referenced characters and personas and the uploaded images' ids
/// follow the text after a separator, as legacy appended them, and the images themselves follow as image parts when
/// the model accepts image input.
fn user_turn_message(
    text: &RuntimeText,
    message: &str,
    inputs: &CreationTurnInputs,
    media: &CreationPromptMedia,
) -> Result<ProviderNeutralMessage, RuntimeTextError> {
    let mut blocks = Vec::new();
    for reference in &inputs.references {
        let Some(found) = media.references.get(reference) else {
            continue;
        };
        let (key, id, missing) = match reference {
            CreationReference::Character { id } => (
                "creation_reference_character",
                id.to_string(),
                "creation_reference_no_definition",
            ),
            CreationReference::Persona { id } => (
                "creation_reference_persona",
                id.to_string(),
                "creation_reference_no_description",
            ),
        };
        let description = match found.text.as_deref().filter(|text| !text.trim().is_empty()) {
            Some(description) => description.to_owned(),
            None => text.render_with(missing, [])?,
        };
        blocks.push(text.render_with(
            key,
            [
                (Variable::SubjectName, found.name.clone()),
                (Variable::DraftItemId, id),
                (Variable::SubjectDescription, description),
            ],
        )?);
    }
    let uploads = inputs
        .images
        .iter()
        .map(|asset_id| {
            text.render_with(
                "creation_uploaded_image",
                [(Variable::DraftItemId, creation_image_label(*asset_id))],
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !uploads.is_empty() {
        blocks.push(uploads.join("\n"));
    }
    let mut content = message.to_owned();
    if !blocks.is_empty() {
        content = format!(
            "{content}\n\n{}\n{}",
            text.render_with("creation_message_attachments", [])?,
            blocks.join("\n\n")
        );
    }
    let mut parts = vec![ProviderContextPart::Text { text: content }];
    if media.image_input {
        parts.extend(
            inputs
                .images
                .iter()
                .map(|asset_id| ProviderContextPart::MediaAsset {
                    asset_id: *asset_id,
                    role: MediaAssetRole::Attachment,
                }),
        );
    }
    Ok(ProviderNeutralMessage {
        role: MessageRole::User,
        parts,
    })
}

fn text_message(role: MessageRole, text: String) -> ProviderNeutralMessage {
    ProviderNeutralMessage {
        role,
        parts: vec![ProviderContextPart::Text { text }],
    }
}

#[cfg(test)]
mod tests {
    use lettuce_conversations::{MessagePart, MessageRole, ProviderContextPart};
    use lettuce_creation::{
        CreationDialogueTurn, CreationDraft, CreationLorebookEntry, CreationScene,
    };
    use lettuce_models::ProviderProtocol;
    use lettuce_types::{CreationTurnId, LorebookEntryId, SceneId};

    use super::{CreationPromptMedia, creation_context_messages, render_draft_view};
    use crate::BuiltInPromptId;
    use crate::generation::runtime_text::RuntimeText;
    use lettuce_creation::CreationTurnInputs;

    fn text_of(message: &lettuce_conversations::ProviderNeutralMessage) -> &str {
        match &message.parts[0] {
            ProviderContextPart::Text { text } => text,
            _ => panic!("text part"),
        }
    }

    #[test]
    fn draft_view_shows_full_texts_and_previews_lore_entries() {
        let text = RuntimeText::from_seed(BuiltInPromptId::CreationRuntime);
        let scene = SceneId::new();
        let draft = CreationDraft::Character {
            name: Some("Lys".into()),
            definition: Some(format!("{}\nmore", "a".repeat(80))),
            scenes: vec![CreationScene {
                id: scene,
                content: "She looks up.\nThe door shuts.".into(),
                direction: None,
            }],
            avatar: None,
        };
        assert_eq!(
            render_draft_view(&text, &draft, &[]).expect("view"),
            format!(
                "DRAFT (character):\n  name: Lys\n  definition: <85 chars> \"{}\nmore\"\n  scenes:\n    {scene} <29 chars> \"She looks up.\nThe door shuts.\"\n",
                "a".repeat(80)
            )
        );
        assert_eq!(
            render_draft_view(
                &text,
                &CreationDraft::Persona {
                    name: None,
                    description: None,
                    avatar: None,
                },
                &[]
            )
            .expect("persona"),
            "DRAFT (persona):\n  name: <unset>\n  definition: <unset>\n"
        );
        let entry = LorebookEntryId::new();
        assert_eq!(
            render_draft_view(
                &text,
                &CreationDraft::Lorebook {
                    name: Some("Saltmoor".into()),
                    description: None,
                    entries: vec![CreationLorebookEntry {
                        id: entry,
                        title: "Pact".into(),
                        content: "Sworn.".into(),
                    }],
                },
                &[]
            )
            .expect("lorebook"),
            format!(
                "DRAFT (lorebook):\n  name: Saltmoor\n  definition: <unset>\n  entries:\n    {entry} Pact <6 chars> \"Sworn.\"\n"
            )
        );
    }

    #[test]
    fn context_is_the_legacy_system_entries_then_the_dialogue() {
        let helper = RuntimeText::from_seed(BuiltInPromptId::CreationHelper);
        let runtime = RuntimeText::from_seed(BuiltInPromptId::CreationRuntime);
        let draft = CreationDraft::Persona {
            name: None,
            description: None,
            avatar: None,
        };
        let messages = creation_context_messages(
            &helper,
            &runtime,
            &draft,
            &CreationPromptMedia::default(),
            &[
                CreationDialogueTurn {
                    turn_id: CreationTurnId::new(),
                    user_message: "Hi! I want to create a new persona.".into(),
                    inputs: Default::default(),
                    assistant_parts: vec![
                        MessagePart::ReasoningSummary {
                            text: "hidden".into(),
                        },
                        MessagePart::Text {
                            text: "What tone?\n".into(),
                        },
                        MessagePart::Text { text: "  ".into() },
                        MessagePart::Text {
                            text: " Any era?".into(),
                        },
                    ],
                },
                CreationDialogueTurn {
                    turn_id: CreationTurnId::new(),
                    user_message: "Quiet.".into(),
                    inputs: Default::default(),
                    assistant_parts: Vec::new(),
                },
            ],
            "You decide.",
            &CreationTurnInputs::default(),
            ProviderProtocol::OpenAiCompatible,
            None,
        )
        .expect("context");
        assert_eq!(messages.len(), 11);
        assert!(
            messages[..7]
                .iter()
                .all(|message| message.role == MessageRole::System)
        );
        assert!(text_of(&messages[0]).starts_with(
            "<role>\nYou help the user build a persona for a roleplay app by calling the available tools."
        ));
        assert_eq!(
            text_of(&messages[6]),
            "<current_draft>\nDRAFT (persona):\n  name: <unset>\n  definition: <unset>\n\n</current_draft>"
        );
        assert_eq!(
            messages[7..]
                .iter()
                .map(|message| (message.role, text_of(message)))
                .collect::<Vec<_>>(),
            [
                (MessageRole::User, "Hi! I want to create a new persona."),
                (MessageRole::Assistant, "What tone?\n\nAny era?"),
                (MessageRole::User, "Quiet."),
                (MessageRole::User, "You decide."),
            ]
        );
    }

    #[test]
    fn manual_draft_context_keeps_text_beyond_the_short_summary() {
        let helper = RuntimeText::from_seed(BuiltInPromptId::CreationHelper);
        let runtime = RuntimeText::from_seed(BuiltInPromptId::CreationRuntime);
        let description = format!(
            "{}Remember the brass compass.",
            "A long description. ".repeat(20)
        );
        let messages = creation_context_messages(
            &helper,
            &runtime,
            &CreationDraft::Persona {
                name: Some("Mira".into()),
                description: Some(description.clone()),
                avatar: None,
            },
            &CreationPromptMedia::default(),
            &[],
            "Keep her background.",
            &CreationTurnInputs::default(),
            ProviderProtocol::OpenAiCompatible,
            None,
        )
        .expect("context");
        assert!(text_of(&messages[6]).contains(&description));
    }

    #[test]
    fn llama_cpp_gets_one_leading_system_message_and_remote_providers_keep_entries() {
        let helper = RuntimeText::from_seed(BuiltInPromptId::CreationHelper);
        let runtime = RuntimeText::from_seed(BuiltInPromptId::CreationRuntime);
        let draft = CreationDraft::Persona {
            name: Some("Ari".into()),
            description: None,
            avatar: None,
        };
        let dialogue = [CreationDialogueTurn {
            turn_id: CreationTurnId::new(),
            user_message: "Hi.".into(),
            inputs: Default::default(),
            assistant_parts: vec![MessagePart::Text {
                text: "Hello.".into(),
            }],
        }];
        let build = |protocol| {
            creation_context_messages(
                &helper,
                &runtime,
                &draft,
                &CreationPromptMedia::default(),
                &dialogue,
                "Go.",
                &CreationTurnInputs::default(),
                protocol,
                None,
            )
            .expect("context")
        };
        let remote = build(ProviderProtocol::OpenAiCompatible);
        let entries = remote
            .iter()
            .take_while(|message| message.role == MessageRole::System)
            .map(text_of)
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 7);
        for protocol in [
            ProviderProtocol::Anthropic,
            ProviderProtocol::Gemini,
            ProviderProtocol::Ollama,
        ] {
            assert_eq!(build(protocol), remote);
        }
        let local = build(ProviderProtocol::LlamaCpp);
        assert_eq!(local.len(), remote.len() - 6);
        assert_eq!(local[0].role, MessageRole::System);
        assert_eq!(
            text_of(&local[0]),
            entries
                .iter()
                .map(|entry| entry.trim())
                .collect::<Vec<_>>()
                .join("\n\n")
        );
        assert!(
            local[1..]
                .iter()
                .all(|message| message.role != MessageRole::System)
        );
        assert_eq!(local[1..], remote[7..]);
    }

    #[test]
    fn the_whole_dialogue_is_sent_like_legacy() {
        let helper = RuntimeText::from_seed(BuiltInPromptId::CreationHelper);
        let runtime = RuntimeText::from_seed(BuiltInPromptId::CreationRuntime);
        let dialogue = (0..300)
            .map(|index| CreationDialogueTurn {
                turn_id: CreationTurnId::new(),
                user_message: format!("turn {index}"),
                inputs: Default::default(),
                assistant_parts: vec![MessagePart::Text {
                    text: format!("reply {index}"),
                }],
            })
            .collect::<Vec<_>>();
        let messages = creation_context_messages(
            &helper,
            &runtime,
            &CreationDraft::Persona {
                name: None,
                description: None,
                avatar: None,
            },
            &CreationPromptMedia::default(),
            &dialogue,
            "latest",
            &CreationTurnInputs::default(),
            ProviderProtocol::OpenAiCompatible,
            None,
        )
        .expect("context");
        assert_eq!(messages.len(), 7 + 600 + 1);
        assert_eq!(text_of(&messages[7]), "turn 0");
        assert_eq!(text_of(&messages[8]), "reply 0");
        assert_eq!(text_of(&messages[messages.len() - 2]), "reply 299");
        assert_eq!(text_of(messages.last().expect("last")), "latest");
    }

    #[test]
    fn tool_results_use_the_legacy_shape_and_wording() {
        use lettuce_conversations::ProposedToolCall;
        use lettuce_creation::{
            CreationOperation, CreationOperationError, CreationOperationOutcome, CreationRejection,
        };
        let text = RuntimeText::from_seed(BuiltInPromptId::CreationRuntime);
        let draft = CreationDraft::Persona {
            name: Some("Ari".into()),
            description: None,
            avatar: None,
        };
        let render = |name: &str,
                      arguments: serde_json::Value,
                      operation: CreationOperation,
                      error: Option<CreationOperationError>| {
            super::render_tool_output(
                &text,
                &ProposedToolCall {
                    provider_call_id: None,
                    name: name.into(),
                    arguments,
                    raw_arguments: None,
                    provider_replay: None,
                },
                &CreationOperationOutcome {
                    ordinal: 0,
                    operation,
                    error,
                },
                &draft,
                None,
            )
            .expect("render")
        };
        let named = render(
            "set_name",
            serde_json::json!({"name": "Ari"}),
            CreationOperation::SetName {
                value: "Ari".into(),
            },
            None,
        );
        assert!(!named.is_error);
        assert_eq!(
            named.value,
            serde_json::json!({"success": true, "message": "Name set to 'Ari'"})
        );
        let preview = render(
            "show_preview",
            serde_json::json!({}),
            CreationOperation::ShowPreview,
            None,
        );
        assert_eq!(
            preview.value,
            serde_json::json!({
                "success": true,
                "action": "show_preview",
                "message": "Here's a preview of what we've built so far!",
                "draft": draft,
            })
        );
        let confirm = render(
            "request_confirmation",
            serde_json::json!({"message": "Save her?"}),
            CreationOperation::RequestConfirmation,
            None,
        );
        assert_eq!(confirm.value["message"], "Save her?");
        assert_eq!(confirm.value["action"], "request_confirmation");
        let unknown = render(
            "generate_image",
            serde_json::json!({}),
            CreationOperation::UndeclaredTool {
                name: "generate_image".into(),
            },
            Some(CreationOperationError::UnknownTool),
        );
        assert!(unknown.is_error);
        assert_eq!(
            unknown.value,
            serde_json::json!({"success": false, "error": "unknown tool: generate_image"})
        );
        let missing = render(
            "set_name",
            serde_json::json!({}),
            CreationOperation::Rejected {
                tool: "set_name".into(),
                reason: CreationRejection::MissingArgument {
                    argument: "name".into(),
                },
            },
            Some(CreationOperationError::InvalidArguments),
        );
        assert_eq!(
            missing.value,
            serde_json::json!({
                "success": false,
                "message": "error: SET_NAME requires args: name=<text>",
                "error": "SET_NAME requires args: name=<text>",
            })
        );
        let scene = SceneId::new();
        let absent = render(
            "delete_scene",
            serde_json::json!({"id": scene}),
            CreationOperation::DeleteScene { id: scene },
            Some(CreationOperationError::NotFound),
        );
        assert_eq!(absent.value["error"], format!("scene {scene} not found"));
        let edited = render(
            "edit_scene",
            serde_json::json!({"id": "sc_2", "content": "x"}),
            CreationOperation::Rejected {
                tool: "edit_scene".into(),
                reason: CreationRejection::UnknownId { id: "sc_2".into() },
            },
            Some(CreationOperationError::NotFound),
        );
        assert_eq!(edited.value["error"], "Scene not found");
        for (tool, argument) in [
            ("set_name", "name"),
            ("write_definition", "definition"),
            ("write_scene", "content"),
            ("edit_scene", "id"),
            ("edit_scene", "content"),
            ("delete_scene", "id"),
            ("write_lore_entry", "content"),
            ("write_lore_entry", "title"),
            ("edit_lore_entry", "id"),
            ("edit_lore_entry", "title"),
            ("edit_lore_entry", "content"),
            ("delete_lore_entry", "id"),
            ("reorder_lore_entries", "order"),
        ] {
            assert!(
                text.render_with(&format!("creation_error_{tool}_{argument}"), [])
                    .is_ok_and(|message| !message.is_empty()),
                "{tool} {argument}"
            );
        }
    }

    #[test]
    fn large_preview_drafts_are_kept_in_the_result() {
        use lettuce_conversations::ProposedToolCall;
        use lettuce_creation::{CreationOperation, CreationOperationOutcome};
        let text = RuntimeText::from_seed(BuiltInPromptId::CreationRuntime);
        let draft = CreationDraft::Lorebook {
            name: Some("Saltmoor".into()),
            description: None,
            entries: (0..6)
                .map(|index| CreationLorebookEntry {
                    id: LorebookEntryId::new(),
                    title: format!("Entry {index}"),
                    content: "x".repeat(256 * 1024),
                })
                .collect(),
        };
        let output = super::render_tool_output(
            &text,
            &ProposedToolCall {
                provider_call_id: None,
                name: "show_preview".into(),
                arguments: serde_json::json!({}),
                raw_arguments: None,
                provider_replay: None,
            },
            &CreationOperationOutcome {
                ordinal: 0,
                operation: CreationOperation::ShowPreview,
                error: None,
            },
            &draft,
            None,
        )
        .expect("render");
        output.validate().expect("bounded result");
        assert!(output.value.get("draft").is_some());
        assert_eq!(output.value["action"], "show_preview");
    }

    #[test]
    fn attachments_follow_the_message_and_the_gallery_marks_the_avatar() {
        use super::{CreationImageOrigin, CreationPromptImage, CreationReferenceText};
        use lettuce_creation::CreationReference;
        let helper = RuntimeText::from_seed(BuiltInPromptId::CreationHelper);
        let runtime = RuntimeText::from_seed(BuiltInPromptId::CreationRuntime);
        let upload = lettuce_types::AssetId::new();
        let ada = lettuce_types::CharacterId::new();
        let gone = lettuce_types::PersonaId::new();
        let label = lettuce_creation::creation_image_label(upload);
        let draft = CreationDraft::Persona {
            name: Some("Mira".into()),
            description: None,
            avatar: Some(upload),
        };
        let mut media = CreationPromptMedia {
            gallery: vec![CreationPromptImage {
                asset_id: upload,
                origin: CreationImageOrigin::Uploaded,
            }],
            ..CreationPromptMedia::default()
        };
        media.references.insert(
            CreationReference::Character { id: ada },
            CreationReferenceText {
                name: "Ada".into(),
                text: None,
            },
        );
        let inputs = CreationTurnInputs {
            references: vec![
                CreationReference::Character { id: ada },
                CreationReference::Persona { id: gone },
            ],
            images: vec![upload],
        };
        let build = |media: &CreationPromptMedia| {
            creation_context_messages(
                &helper,
                &runtime,
                &draft,
                media,
                &[],
                "Make a persona for her.",
                &inputs,
                ProviderProtocol::OpenAiCompatible,
                None,
            )
            .expect("context")
        };
        let messages = build(&media);
        let last = messages.last().expect("user message");
        assert_eq!(
            text_of(last),
            format!(
                "Make a persona for her.\n\n---\n[Referenced Character: \"Ada\" (id:{ada})]\nNo definition available.\n\n[Uploaded Image (id:{label})]"
            )
        );
        assert_eq!(last.parts.len(), 1);
        assert!(text_of(&messages[6]).contains(&format!(
            "  avatar: {label}\n  generated_images:\n    [1] id={label} uploaded [active avatar]\n"
        )));
        media.image_input = true;
        let seeing = build(&media);
        assert!(matches!(
            seeing.last().expect("user message").parts[1],
            ProviderContextPart::MediaAsset { asset_id, .. } if asset_id == upload
        ));
    }

    #[test]
    fn the_helper_teaches_name_placeholders_and_keeps_them_in_the_draft() {
        let helper = RuntimeText::from_seed(BuiltInPromptId::CreationHelper);
        let runtime = RuntimeText::from_seed(BuiltInPromptId::CreationRuntime).with_literal_names();
        let draft = CreationDraft::Persona {
            name: Some("Mira".into()),
            description: Some("{{user}} keeps the lighthouse.".into()),
            avatar: None,
        };
        let messages = creation_context_messages(
            &helper,
            &runtime,
            &draft,
            &CreationPromptMedia::default(),
            &[],
            "Go.",
            &CreationTurnInputs::default(),
            ProviderProtocol::OpenAiCompatible,
            None,
        )
        .expect("context");
        let system = messages
            .iter()
            .take_while(|message| message.role == MessageRole::System)
            .map(text_of)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(system.contains("marked `{{user}}:`"));
        assert!(system.contains("{{char}}: \""));
        assert!(system.contains("{{user}} keeps the lighthouse."));
        let tools = lettuce_creation::describe_creation_tools(
            &lettuce_creation::creation_tool_request(
                lettuce_creation::CreationTargetKind::Character,
            ),
            &|key| runtime.render_with(key, []).unwrap_or_default(),
        );
        let scene = tools
            .definitions
            .iter()
            .find(|definition| definition.name == "write_scene")
            .and_then(|definition| definition.description.clone())
            .expect("scene tool");
        assert!(scene.contains("{{char}}") && scene.contains("{{user}}"));
    }
}
