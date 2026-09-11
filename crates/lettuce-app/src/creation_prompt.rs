use lettuce_context::{
    PromptConditionContext, PromptEntryRole, PromptRenderContext, PromptRenderValues,
    PromptVariable as Variable, render_prompt,
};
use lettuce_conversations::{
    MAX_PROVIDER_CONTEXT_MESSAGES, MessagePart, MessageRole, ProviderContextPart,
    ProviderNeutralMessage,
};
use lettuce_creation::{
    CreationDialogueTurn, CreationDraft, CreationTargetKind, MAX_CREATION_INFERENCE_ROUNDS,
};

use crate::runtime_text::{RuntimeText, RuntimeTextError};

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

const DEFINITION_PREVIEW_CHARS: usize = 80;
const ITEM_PREVIEW_CHARS: usize = 60;

/// The creation helper's system messages (the `prompt_app_creation_helper`
/// entries with the target label and draft view), the earlier dialogue and the
/// current user message, in the legacy order.
pub(crate) fn creation_context_messages(
    helper: &RuntimeText,
    runtime: &RuntimeText,
    draft: &CreationDraft,
    dialogue: &[CreationDialogueTurn],
    user_message: &str,
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
    let mut messages = rendered
        .relative
        .iter()
        .filter_map(prompt_message)
        .collect::<Vec<_>>();
    let in_chat = rendered
        .in_chat
        .iter()
        .filter_map(|entry| prompt_message(entry).map(|message| (entry.depth, message)))
        .collect::<Vec<_>>();
    let budget = MAX_PROVIDER_CONTEXT_MESSAGES
        .saturating_sub(messages.len() + in_chat.len() + 1)
        .saturating_sub(2 * usize::from(MAX_CREATION_INFERENCE_ROUNDS));
    let mut history = Vec::new();
    for turn in dialogue.iter().rev() {
        let reply = turn
            .assistant_parts
            .iter()
            .filter_map(|part| match part {
                MessagePart::Text { text } if !text.trim().is_empty() => Some(text.trim()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        let turn_messages = if reply.is_empty() { 1 } else { 2 };
        if history.len() + turn_messages > budget {
            break;
        }
        if !reply.is_empty() {
            history.push(text_message(MessageRole::Assistant, reply));
        }
        history.push(text_message(MessageRole::User, turn.user_message.clone()));
    }
    messages.extend(history.into_iter().rev());
    messages.push(text_message(MessageRole::User, user_message.to_owned()));
    crate::companion_memory_inference::insert_in_chat_messages(&mut messages, in_chat);
    Ok(messages)
}

/// The legacy `DRAFT (...)` summary the model reads in `<current_draft>`.
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
    use lettuce_types::{CreationTurnId, LorebookEntryId, SceneId};

    use super::{creation_context_messages, render_draft_view};
    use crate::BuiltInPromptId;
    use crate::runtime_text::RuntimeText;

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
    fn oldest_turns_are_dropped_to_leave_room_for_eight_rounds() {
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
        )
        .expect("context");
        assert_eq!(
            messages.len(),
            lettuce_conversations::MAX_PROVIDER_CONTEXT_MESSAGES - 16
        );
        assert_eq!(text_of(&messages[7]), "turn 56");
        assert_eq!(text_of(&messages[messages.len() - 2]), "reply 299");
        assert_eq!(text_of(messages.last().expect("last")), "latest");
    }
}
