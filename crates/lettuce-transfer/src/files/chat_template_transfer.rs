//! Chat templates (conversation starters) read from and written to USC
//! `chat_template` cards and the old app's chat template JSON.

use serde::Serialize;
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
    #[error("Failed to serialize chat template export")]
    Serialize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatTemplateTransferMessage {
    pub id: String,
    /// `user` or `assistant`.
    pub role: String,
    pub content: String,
}

/// A stored chat template in the shape its files carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatTemplateTransfer {
    pub id: String,
    pub name: String,
    pub messages: Vec<ChatTemplateTransferMessage>,
    pub scene_id: Option<String>,
    pub prompt_template_id: Option<String>,
    /// `None` inherits the character's lorebooks.
    pub lorebook_ids_override: Option<Vec<String>>,
    pub created_at: i64,
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

#[derive(Serialize)]
struct UscRef<'a> {
    kind: &'static str,
    id: &'a str,
}

#[derive(Serialize)]
struct UscMessage<'a> {
    id: &'a str,
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UscPayload<'a> {
    id: &'a str,
    name: &'a str,
    messages: Vec<UscMessage<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scene_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_prompt_template: Option<UscRef<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lorebook_ids_override: Option<&'a Vec<String>>,
    created_at: i64,
}

#[derive(Serialize)]
struct UscSchema {
    name: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
struct UscCard<'a> {
    schema: UscSchema,
    kind: &'static str,
    payload: UscPayload<'a>,
}

/// The template as a USC `chat_template` card, pretty-printed.
pub fn export_chat_template_usc(
    template: &ChatTemplateTransfer,
) -> Result<String, ChatTemplateTransferError> {
    serde_json::to_string_pretty(&UscCard {
        schema: UscSchema {
            name: "USC",
            version: "1.0",
        },
        kind: "chat_template",
        payload: UscPayload {
            id: &template.id,
            name: &template.name,
            messages: template
                .messages
                .iter()
                .map(|message| UscMessage {
                    id: &message.id,
                    role: &message.role,
                    content: &message.content,
                })
                .collect(),
            scene_id: template.scene_id.as_deref(),
            system_prompt_template: template.prompt_template_id.as_deref().map(|id| UscRef {
                kind: "system_prompt_template",
                id,
            }),
            lorebook_ids_override: template.lorebook_ids_override.as_ref(),
            created_at: template.created_at,
        },
    })
    .map_err(|_| ChatTemplateTransferError::Serialize)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonTemplate<'a> {
    name: &'a str,
    messages: Vec<JsonMessage<'a>>,
    scene_id: Option<&'a str>,
    prompt_template_id: Option<&'a str>,
    lorebook_ids_override: Option<&'a Vec<String>>,
}

#[derive(Serialize)]
struct JsonMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct JsonExport<'a> {
    version: u32,
    kind: &'static str,
    template: JsonTemplate<'a>,
}

/// The template as the old app's plain chat template JSON, pretty-printed.
pub fn export_chat_template_json(
    template: &ChatTemplateTransfer,
) -> Result<String, ChatTemplateTransferError> {
    serde_json::to_string_pretty(&JsonExport {
        version: 1,
        kind: "chat_template",
        template: JsonTemplate {
            name: &template.name,
            messages: template
                .messages
                .iter()
                .map(|message| JsonMessage {
                    role: &message.role,
                    content: &message.content,
                })
                .collect(),
            scene_id: template.scene_id.as_deref(),
            prompt_template_id: template.prompt_template_id.as_deref(),
            lorebook_ids_override: template.lorebook_ids_override.as_ref(),
        },
    })
    .map_err(|_| ChatTemplateTransferError::Serialize)
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
