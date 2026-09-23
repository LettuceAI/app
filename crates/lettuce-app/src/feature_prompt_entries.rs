//! A feature prompt (scene writer, design reference writer) rendered the way
//! legacy's feature commands did: active entries in template order, each
//! condensed, merged into one system entry when the template condenses.

use lettuce_context::{
    PromptDocument, PromptEntryImageSlot, PromptEntryPayload, PromptEntryPosition, PromptEntryRole,
    PromptRenderContext, PromptRenderError, render_prompt,
};
use lettuce_conversations::MessageRole;

/// A rendered entry in template order, with the placement legacy gave it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FeatureEntry {
    pub(crate) role: PromptEntryRole,
    pub(crate) content: String,
    pub(crate) slot: Option<PromptEntryImageSlot>,
    pub(crate) position: PromptEntryPosition,
    pub(crate) depth: u32,
    pub(crate) conditional_min_messages: Option<u32>,
    pub(crate) interval_turns: Option<u32>,
}

impl FeatureEntry {
    fn image_bound(&self, image_tokens: &[&str]) -> bool {
        self.slot.is_some()
            || image_tokens
                .iter()
                .any(|token| self.content.contains(token))
    }
}

/// Legacy `condense_prompt_whitespace`.
pub(crate) fn condense(input: &str) -> String {
    let mut output = input.to_owned();
    while output.contains("\n\n\n") {
        output = output.replace("\n\n\n", "\n\n");
    }
    output.trim().to_owned()
}

pub(crate) fn strip_tokens(content: &str, tokens: &[&str]) -> String {
    tokens.iter().fold(content.to_owned(), |content, token| {
        content.replace(token, "")
    })
}

pub(crate) fn message_role(role: PromptEntryRole) -> MessageRole {
    match role {
        PromptEntryRole::System => MessageRole::System,
        PromptEntryRole::User => MessageRole::User,
        PromptEntryRole::Assistant => MessageRole::Assistant,
    }
}

/// Conditional and interval entries render regardless of the message count;
/// each feature places them itself. An entry whose text names one of
/// `image_tokens` stays out of the condensed merge, like one with an image
/// slot.
pub(crate) fn render_feature_entries(
    document: &PromptDocument,
    context: &PromptRenderContext,
    image_tokens: &[&str],
) -> Result<Vec<FeatureEntry>, PromptRenderError> {
    let mut placed = document.clone();
    for entry in &mut placed.entries {
        if matches!(
            entry.injection_position,
            PromptEntryPosition::Conditional | PromptEntryPosition::Interval
        ) {
            entry.injection_position = PromptEntryPosition::InChat;
        }
    }
    let rendered = render_prompt(&placed, context)?;
    let mut entries = rendered
        .relative
        .into_iter()
        .chain(rendered.in_chat)
        .filter_map(|message| {
            let index = document
                .entries
                .iter()
                .position(|entry| entry.id == message.entry_id)?;
            let entry = &document.entries[index];
            let content = condense(&message.content);
            if content.is_empty() && message.payload.is_none() {
                return None;
            }
            Some((
                index,
                FeatureEntry {
                    role: message.role,
                    content,
                    slot: message.payload.map(|payload| match payload {
                        PromptEntryPayload::ImageSlot { slot } => slot,
                    }),
                    position: entry.injection_position,
                    depth: entry.depth,
                    conditional_min_messages: entry.conditional_min_messages,
                    interval_turns: entry.interval_turns,
                },
            ))
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|(index, _)| *index);
    let entries = entries.into_iter().map(|(_, entry)| entry);
    if !document.condense {
        return Ok(entries.collect());
    }
    let (images, sections): (Vec<_>, Vec<_>) =
        entries.partition(|entry| entry.image_bound(image_tokens));
    let merged = sections
        .iter()
        .map(|entry| entry.content.trim())
        .filter(|content| !content.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    Ok((!merged.trim().is_empty())
        .then_some(FeatureEntry {
            role: PromptEntryRole::System,
            content: merged,
            slot: None,
            position: PromptEntryPosition::Relative,
            depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
        })
        .into_iter()
        .chain(images)
        .collect())
}
