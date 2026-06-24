use serde_json::{json, Value};

use crate::chat_manager::temporal::message_timestamp_prefix;
use crate::chat_manager::types::{
    ImageAttachment, PromptEntryRole, StoredMessage, SystemPromptEntry,
};

/// Pushes a system message to the API message list if present.
/// Uses the provider-specific system role.
pub fn push_system_message(
    target: &mut Vec<Value>,
    system_role: &str,
    system_prompt: Option<String>,
) {
    if let Some(system) = system_prompt {
        target.push(serde_json::json!({ "role": system_role, "content": system }));
    }
}

pub fn push_prompt_entry_message(
    target: &mut Vec<Value>,
    system_role: &str,
    entry: &SystemPromptEntry,
) {
    if entry.content.trim().is_empty() {
        return;
    }
    let role = match entry.role {
        PromptEntryRole::System => system_role,
        PromptEntryRole::User => "user",
        PromptEntryRole::Assistant => "assistant",
    };
    target.push(serde_json::json!({ "role": role, "content": entry.content }));
}

pub fn audio_format_from_mime(mime: &str) -> &'static str {
    let mime = mime.to_ascii_lowercase();
    if mime.contains("wav") {
        "wav"
    } else if mime.contains("mpeg") || mime.contains("mp3") {
        "mp3"
    } else if mime.contains("ogg") {
        "ogg"
    } else if mime.contains("flac") {
        "flac"
    } else if mime.contains("aac") {
        "aac"
    } else if mime.contains("aiff") || mime.contains("aif") {
        "aiff"
    } else if mime.contains("mp4") || mime.contains("m4a") {
        "m4a"
    } else {
        "wav"
    }
}

fn raw_base64(data: &str) -> &str {
    if let Some(idx) = data.find(";base64,") {
        &data[idx + ";base64,".len()..]
    } else {
        data
    }
}

pub fn build_multimodal_content(
    text: &str,
    attachments: &[ImageAttachment],
    allow_image: bool,
    allow_audio: bool,
) -> Value {
    let mut content_parts: Vec<Value> = Vec::new();

    if !text.is_empty() {
        content_parts.push(json!({
            "type": "text",
            "text": text
        }));
    }

    for attachment in attachments {
        if attachment.data.is_empty() {
            continue;
        }

        if attachment.mime_type.starts_with("audio/") {
            if !allow_audio {
                continue;
            }

            content_parts.push(json!({
                "type": "input_audio",
                "input_audio": {
                    "data": raw_base64(&attachment.data),
                    "format": audio_format_from_mime(&attachment.mime_type)
                }
            }));
            continue;
        }

        if !allow_image {
            continue;
        }

        let image_url = if attachment.data.starts_with("http://")
            || attachment.data.starts_with("https://")
            || attachment.data.starts_with("data:")
        {
            attachment.data.clone()
        } else {
            format!("data:{};base64,{}", attachment.mime_type, attachment.data)
        };

        content_parts.push(json!({
            "type": "image_url",
            "image_url": {
                "url": image_url,
                "detail": "auto"
            }
        }));
    }

    if content_parts.is_empty() {
        content_parts.push(json!({
            "type": "text",
            "text": " "
        }));
    }

    Value::Array(content_parts)
}

/// Pushes a user/assistant message to the API list, skipping scene messages, and performs
/// minimal placeholder replacements ({{char}}, {{persona}}, {{user}}) based on provided names.
pub fn push_user_or_assistant_message_with_context(
    target: &mut Vec<Value>,
    message: &StoredMessage,
    char_name: &str,
    persona_name: &str,
    allow_image_input: bool,
    allow_audio_input: bool,
    time_frame_delta: i64,
    time_stamp_enabled: bool,
) {
    if message.role == "scene" {
        return;
    }

    let persona_name = if persona_name.trim().is_empty() {
        "user"
    } else {
        persona_name
    };
    let mut text = super::request::message_text_for_api(message)
        .replace("{{char}}", char_name)
        .replace("{{persona}}", persona_name)
        .replace("{{user}}", persona_name);

    if time_stamp_enabled {
        let prefix = message_timestamp_prefix(message.created_at, time_frame_delta);
        text = if text.is_empty() {
            prefix
        } else {
            format!("{} {}", prefix, text)
        };
    }

    if (allow_image_input || allow_audio_input)
        && !message.attachments.is_empty()
        && message.role == "user"
    {
        let content = build_multimodal_content(
            &text,
            &message.attachments,
            allow_image_input,
            allow_audio_input,
        );
        target.push(json!({
            "role": message.role,
            "content": content
        }));
    } else {
        target.push(json!({
            "role": message.role,
            "content": text
        }));
    }
}

pub fn sanitize_placeholders_in_api_messages(
    messages: &mut Vec<serde_json::Value>,
    char_name: &str,
    persona_name: &str,
) {
    let persona_name = if persona_name.trim().is_empty() {
        "user"
    } else {
        persona_name
    };
    for msg in messages.iter_mut() {
        if let Some(obj) = msg.as_object_mut() {
            if let Some(content) = obj.get_mut("content") {
                if let Some(s) = content.as_str() {
                    let updated = s
                        .replace("{{char}}", char_name)
                        .replace("{{persona}}", persona_name)
                        .replace("{{user}}", persona_name);
                    *content = serde_json::Value::String(updated);
                } else if let Some(arr) = content.as_array_mut() {
                    for part in arr.iter_mut() {
                        if let Some(part_obj) = part.as_object_mut() {
                            if part_obj.get("type").and_then(|t| t.as_str()) == Some("text") {
                                if let Some(text) = part_obj.get_mut("text") {
                                    if let Some(s) = text.as_str() {
                                        let updated = s
                                            .replace("{{char}}", char_name)
                                            .replace("{{persona}}", persona_name)
                                            .replace("{{user}}", persona_name);
                                        *text = Value::String(updated);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
