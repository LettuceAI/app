//! Chat templates (conversation starters) read from USC `chat_template`
//! cards and the old app's chat template JSON.

use serde_json::Value;

const MAX_NAME_BYTES: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChatTemplateTransferError {
    #[error("Invalid chat template file: {0}")]
    InvalidJson(String),
    #[error("Chat template name is required.")]
    NameRequired,
    #[error("Chat template name is longer than 1024 bytes.")]
    NameTooLong,
    #[error("Unsupported chat template file.")]
    Unsupported,
}

/// A chat template read from a file; its references are the file's ids and
/// still have to be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedChatTemplate {
    pub name: String,
    /// `(role, content)` for each user or assistant message.
    pub messages: Vec<(String, String)>,
    pub scene_id: Option<String>,
    pub prompt_template_id: Option<String>,
    pub lorebook_ids_override: Option<Vec<String>>,
}

/// A chat template file: a USC card, the old app's `{kind, template}` JSON,
/// or a bare template object.
pub fn parse_chat_template_import(
    json: &str,
) -> Result<ImportedChatTemplate, ChatTemplateTransferError> {
    let value: Value = serde_json::from_str(json)
        .map_err(|error| ChatTemplateTransferError::InvalidJson(error.to_string()))?;
    if value.pointer("/schema/name").and_then(Value::as_str) == Some("USC")
        && value.get("kind").and_then(Value::as_str) == Some("chat_template")
        && let Some(payload) = value
            .get("payload")
            .filter(|payload| crate::files::prompt_transfer::truthy(payload))
    {
        let prompt = payload
            .pointer("/systemPromptTemplate/id")
            .and_then(Value::as_str)
            .map(str::to_owned);
        return normalized(payload, prompt);
    }
    if value.get("kind").and_then(Value::as_str) == Some("chat_template")
        && let Some(template) = value
            .get("template")
            .filter(|template| crate::files::prompt_transfer::truthy(template))
    {
        let prompt = string(template.get("promptTemplateId"));
        return normalized(template, prompt);
    }
    if value.is_object() || value.is_array() {
        let prompt = string(value.get("promptTemplateId"));
        return normalized(&value, prompt);
    }
    Err(ChatTemplateTransferError::Unsupported)
}

fn string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_owned)
}

fn normalized(
    input: &Value,
    prompt_template_id: Option<String>,
) -> Result<ImportedChatTemplate, ChatTemplateTransferError> {
    let name = input
        .get("name")
        .and_then(Value::as_str)
        .map(|name| {
            name.trim_matches(|character: char| {
                character.is_whitespace() || character == '\u{feff}'
            })
        })
        .filter(|name| !name.is_empty())
        .ok_or(ChatTemplateTransferError::NameRequired)?;
    if name.len() > MAX_NAME_BYTES {
        return Err(ChatTemplateTransferError::NameTooLong);
    }
    let name = name.to_owned();
    let messages = input
        .get("messages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|message| {
            let role = message
                .get("role")
                .and_then(Value::as_str)
                .filter(|role| matches!(*role, "user" | "assistant"))?;
            let content = message.get("content").and_then(Value::as_str)?;
            Some((role.to_owned(), content.to_owned()))
        })
        .collect();
    Ok(ImportedChatTemplate {
        name,
        messages,
        scene_id: string(input.get("sceneId")),
        prompt_template_id,
        lorebook_ids_override: input
            .get("lorebookIdsOverride")
            .and_then(Value::as_array)
            .map(|ids| {
                ids.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            }),
    })
}

#[cfg(test)]
mod tests {
    use super::{ChatTemplateTransferError, parse_chat_template_import};

    #[test]
    fn names_are_kept_whole_up_to_the_storage_bound() {
        let long = "\u{e9}".repeat(400);
        let json = serde_json::json!({ "name": long, "messages": [] }).to_string();
        assert_eq!(
            parse_chat_template_import(&json)
                .expect("name past 256 characters")
                .name,
            long
        );
        let json = serde_json::json!({ "name": "n".repeat(1025), "messages": [] }).to_string();
        assert_eq!(
            parse_chat_template_import(&json),
            Err(ChatTemplateTransferError::NameTooLong)
        );
    }
}
