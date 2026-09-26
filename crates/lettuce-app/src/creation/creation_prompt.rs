use lettuce_context::{
    PromptConditionContext, PromptEntryRole, PromptRenderContext, PromptRenderValues,
    PromptVariable as Variable, render_prompt,
};
use lettuce_conversations::{
    MessagePart, MessageRole, ProposedToolCall, ProviderContextPart, ProviderNeutralMessage,
    ToolOutput,
};
use lettuce_creation::{
    CreationDialogueTurn, CreationDraft, CreationOperation, CreationOperationError,
    CreationOperationOutcome, CreationRejection, CreationTargetKind,
};
use lettuce_models::ProviderProtocol;
use serde_json::Value;

use crate::generation::runtime_text::{RuntimeText, RuntimeTextError};

/// Every draft-view fragment of `prompt_app_creation_runtime`.
#[cfg(test)]
pub(crate) const DRAFT_VIEW_KEYS: [&str; 14] = [
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
    "creation_draft_preview",
    "creation_draft_preview_truncated",
];

/// Every tool-result fragment of `prompt_app_creation_runtime`.
#[cfg(test)]
pub(crate) const RESULT_KEYS: [&str; 34] = [
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

const DEFINITION_PREVIEW_CHARS: usize = 80;
const ITEM_PREVIEW_CHARS: usize = 60;

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
pub(crate) fn creation_context_messages(
    helper: &RuntimeText,
    runtime: &RuntimeText,
    draft: &CreationDraft,
    dialogue: &[CreationDialogueTurn],
    user_message: &str,
    protocol: ProviderProtocol,
    fallback_protocol: Option<&str>,
) -> Result<Vec<ProviderNeutralMessage>, RuntimeTextError> {
    let mut values = PromptRenderValues::default();
    values
        .purpose_values
        .insert(Variable::TargetLabel, target_label(runtime, draft.kind())?);
    values
        .purpose_values
        .insert(Variable::DraftState, render_draft_view(runtime, draft)?);
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
        messages.push(text_message(MessageRole::User, turn.user_message.clone()));
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
    messages.push(text_message(MessageRole::User, user_message.to_owned()));
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
        CreationDraft::Persona { name, description } => (name, description.as_ref()),
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
                    (
                        Variable::TextQuote,
                        preview(text, definition, DEFINITION_PREVIEW_CHARS)?,
                    ),
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
                            (
                                Variable::TextQuote,
                                preview(text, &scene.content, ITEM_PREVIEW_CHARS)?,
                            ),
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
    Ok(view)
}

/// The tool result for one call: `{success, message, error?, ...}`; preview
/// and confirmation also carry `action` and the draft.
pub(crate) fn render_tool_output(
    text: &RuntimeText,
    call: &ProposedToolCall,
    outcome: &CreationOperationOutcome,
    draft: &CreationDraft,
) -> Result<ToolOutput, RuntimeTextError> {
    let Some(error) = outcome.error else {
        let mut value = serde_json::Map::new();
        value.insert("success".to_owned(), Value::Bool(true));
        let message = match &outcome.operation {
            CreationOperation::SetName { value } => text.render_with(
                "creation_result_set_name",
                [(Variable::DraftName, value.clone())],
            )?,
            CreationOperation::SetDescription { .. } => {
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

    use super::{creation_context_messages, render_draft_view};
    use crate::BuiltInPromptId;
    use crate::generation::runtime_text::RuntimeText;

    fn text_of(message: &lettuce_conversations::ProviderNeutralMessage) -> &str {
        match &message.parts[0] {
            ProviderContextPart::Text { text } => text,
            _ => panic!("text part"),
        }
    }

    #[test]
    fn draft_view_matches_the_legacy_summary() {
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
        };
        assert_eq!(
            render_draft_view(&text, &draft).expect("view"),
            format!(
                "DRAFT (character):\n  name: Lys\n  definition: <85 chars> \"{}…\"\n  scenes:\n    {scene} <29 chars> \"She looks up. The door shuts.\"\n",
                "a".repeat(80)
            )
        );
        assert_eq!(
            render_draft_view(
                &text,
                &CreationDraft::Persona {
                    name: None,
                    description: None
                }
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
                }
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
        };
        let messages = creation_context_messages(
            &helper,
            &runtime,
            &draft,
            &[
                CreationDialogueTurn {
                    turn_id: CreationTurnId::new(),
                    user_message: "Hi! I want to create a new persona.".into(),
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
                    assistant_parts: Vec::new(),
                },
            ],
            "You decide.",
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
    fn llama_cpp_gets_one_leading_system_message_and_remote_providers_keep_entries() {
        let helper = RuntimeText::from_seed(BuiltInPromptId::CreationHelper);
        let runtime = RuntimeText::from_seed(BuiltInPromptId::CreationRuntime);
        let draft = CreationDraft::Persona {
            name: Some("Ari".into()),
            description: None,
        };
        let dialogue = [CreationDialogueTurn {
            turn_id: CreationTurnId::new(),
            user_message: "Hi.".into(),
            assistant_parts: vec![MessagePart::Text {
                text: "Hello.".into(),
            }],
        }];
        let build = |protocol| {
            creation_context_messages(&helper, &runtime, &draft, &dialogue, "Go.", protocol, None)
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
            },
            &dialogue,
            "latest",
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
    fn oversized_preview_drafts_are_left_out_of_the_result() {
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
        )
        .expect("render");
        output.validate().expect("bounded result");
        assert!(output.value.get("draft").is_none());
        assert_eq!(output.value["action"], "show_preview");
    }
}
