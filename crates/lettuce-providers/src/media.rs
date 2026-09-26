//! The media bytes a provider request may inline. The application supplies
//! the source; adapters only see a MIME type and bytes.

use lettuce_types::AssetId;

#[derive(Clone, PartialEq, Eq)]
pub struct ProviderMedia {
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

impl std::fmt::Debug for ProviderMedia {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderMedia")
            .field("mime_type", &self.mime_type)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProviderMediaError {
    #[error("media asset is unavailable")]
    Unavailable,
}

pub trait ProviderMediaSource: Send + Sync {
    fn load(&self, asset_id: AssetId) -> Result<ProviderMedia, ProviderMediaError>;
}

/// The images of an OpenAI-style `images` list (`[{image_url: {url}}]`);
/// only base64 `data:image/` URLs are kept.
pub(crate) fn openai_generated_images(
    images: &[serde_json::Value],
) -> Vec<lettuce_conversations::GeneratedMedia> {
    images
        .iter()
        .filter_map(|image| {
            image
                .get("image_url")
                .and_then(|url| url.get("url"))
                .and_then(serde_json::Value::as_str)
        })
        .filter_map(lettuce_conversations::GeneratedMedia::from_data_url)
        .collect()
}

/// The image of a Gemini `inlineData` part; other inline data (audio) is not
/// a reply image.
pub(crate) fn gemini_generated_image(
    inline: &serde_json::Value,
) -> Option<lettuce_conversations::GeneratedMedia> {
    let mime_type = inline
        .get("mimeType")
        .or_else(|| inline.get("mime_type"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("image/png");
    let data = inline.get("data").and_then(serde_json::Value::as_str)?;
    lettuce_conversations::GeneratedMedia::from_inline(mime_type, data)
}

/// The user attachments a request inlines, keyed by asset.
pub(crate) type Attachments = std::collections::HashMap<AssetId, ProviderMedia>;

/// Whether the model takes image and audio input.
pub(crate) fn allowed_inputs(request: &lettuce_conversations::InferenceRequest) -> (bool, bool) {
    let inputs = &request.profile.chat_profile.capabilities.input_modalities;
    (
        inputs.image == lettuce_models::CapabilityStatus::Supported,
        inputs.audio == lettuce_models::CapabilityStatus::Supported,
    )
}

/// Reads the user attachments the model can take, off the async executor.
/// Every attachment must be granted; one that cannot be read is left out
/// and its message keeps its shape. Attachments on other roles, or for a model without image or
/// audio input, are never read.
pub(crate) async fn load_attachments(
    request: &lettuce_conversations::InferenceRequest,
    media: Option<std::sync::Arc<dyn ProviderMediaSource>>,
) -> Result<Attachments, crate::common::AdapterError> {
    let (allow_image, allow_audio) = allowed_inputs(request);
    if !allow_image && !allow_audio {
        return Ok(Attachments::new());
    }
    let mut wanted = Vec::new();
    for message in &request.context.messages {
        if message.role != lettuce_conversations::MessageRole::User {
            continue;
        }
        for part in &message.parts {
            if let lettuce_conversations::ProviderContextPart::MediaAsset { asset_id, .. } = part {
                if !request.media_grants.contains(asset_id) {
                    return Err(crate::common::AdapterError::Rejected);
                }
                if !wanted.contains(asset_id) {
                    wanted.push(*asset_id);
                }
            }
        }
    }
    let Some(media) = media.filter(|_| !wanted.is_empty()) else {
        return Ok(Attachments::new());
    };
    tokio::task::spawn_blocking(move || {
        wanted
            .into_iter()
            .filter_map(|asset_id| media.load(asset_id).ok().map(|loaded| (asset_id, loaded)))
            .collect()
    })
    .await
    .map_err(|_| crate::common::AdapterError::Transport)
}

/// The loaded attachments of one request and the input kinds its model
/// accepts.
#[derive(Debug, Default)]
pub(crate) struct RequestMedia {
    attachments: Attachments,
    allowed: (bool, bool),
}

impl RequestMedia {
    pub(crate) async fn load(
        request: &lettuce_conversations::InferenceRequest,
        media: Option<std::sync::Arc<dyn ProviderMediaSource>>,
    ) -> Result<Self, crate::common::AdapterError> {
        Ok(Self {
            attachments: load_attachments(request, media).await?,
            allowed: allowed_inputs(request),
        })
    }

    /// A system prompt, a user turn with text, one image and one audio clip,
    /// and an assistant turn with an image, with the given model inputs.
    #[cfg(test)]
    pub(crate) fn fixture(
        allowed: (bool, bool),
    ) -> (lettuce_conversations::ProviderNeutralContext, Self) {
        use lettuce_conversations::{
            ContextAttributions, ContextBudgetReport, MediaAssetRole, MessageRole,
            ProviderContextPart, ProviderNeutralContext, ProviderNeutralMessage,
        };
        let image = AssetId::new();
        let audio = AssetId::new();
        let reply_image = AssetId::new();
        let context = ProviderNeutralContext {
            messages: vec![
                ProviderNeutralMessage {
                    role: MessageRole::System,
                    parts: vec![ProviderContextPart::Text {
                        text: "sys".to_owned(),
                    }],
                },
                ProviderNeutralMessage {
                    role: MessageRole::User,
                    parts: vec![
                        ProviderContextPart::Text {
                            text: "look".to_owned(),
                        },
                        ProviderContextPart::MediaAsset {
                            asset_id: image,
                            role: MediaAssetRole::Attachment,
                        },
                        ProviderContextPart::MediaAsset {
                            asset_id: audio,
                            role: MediaAssetRole::Attachment,
                        },
                    ],
                },
                ProviderNeutralMessage {
                    role: MessageRole::Assistant,
                    parts: vec![
                        ProviderContextPart::Text {
                            text: "ok".to_owned(),
                        },
                        ProviderContextPart::MediaAsset {
                            asset_id: reply_image,
                            role: MediaAssetRole::Attachment,
                        },
                    ],
                },
            ],
            attributions: ContextAttributions::default(),
            budget: ContextBudgetReport::default(),
        };
        let attachments = [
            (
                image,
                ProviderMedia {
                    mime_type: "image/png".to_owned(),
                    bytes: vec![1, 2, 3],
                },
            ),
            (
                audio,
                ProviderMedia {
                    mime_type: "audio/mpeg".to_owned(),
                    bytes: vec![9],
                },
            ),
            (
                reply_image,
                ProviderMedia {
                    mime_type: "image/png".to_owned(),
                    bytes: vec![4],
                },
            ),
        ]
        .into_iter()
        .collect();
        (
            context,
            Self {
                attachments,
                allowed,
            },
        )
    }

    #[cfg(test)]
    pub(crate) fn attachments(&self) -> &Attachments {
        &self.attachments
    }

    /// The readable images of one message, in order.
    pub(crate) fn images(
        &self,
        message: &lettuce_conversations::ProviderNeutralMessage,
    ) -> Vec<ProviderMedia> {
        if !self.allowed.0 {
            return Vec::new();
        }
        self.readable(message)
            .filter(|media| !media.is_audio())
            .collect()
    }

    /// The readable images and audio clips of one message the model accepts.
    pub(crate) fn readable(
        &self,
        message: &lettuce_conversations::ProviderNeutralMessage,
    ) -> impl Iterator<Item = ProviderMedia> {
        let (allow_image, allow_audio) = self.allowed;
        message_attachments(message, &self.attachments, self.allowed)
            .into_iter()
            .filter(move |media| {
                !media.bytes.is_empty()
                    && if media.is_audio() {
                        allow_audio
                    } else {
                        allow_image
                    }
            })
    }
}

/// The attachments one message carries to the provider: only user messages
/// on a model with image or audio input have any. An attachment that could
/// not be read stays as an empty entry so the message keeps its multimodal
/// shape.
pub(crate) fn message_attachments(
    message: &lettuce_conversations::ProviderNeutralMessage,
    loaded: &Attachments,
    (allow_image, allow_audio): (bool, bool),
) -> Vec<ProviderMedia> {
    if message.role != lettuce_conversations::MessageRole::User || !(allow_image || allow_audio) {
        return Vec::new();
    }
    message
        .parts
        .iter()
        .filter_map(|part| match part {
            lettuce_conversations::ProviderContextPart::MediaAsset { asset_id, .. } => {
                Some(loaded.get(asset_id).cloned().unwrap_or(ProviderMedia {
                    mime_type: String::new(),
                    bytes: Vec::new(),
                }))
            }
            _ => None,
        })
        .collect()
}

impl ProviderMedia {
    pub(crate) fn is_audio(&self) -> bool {
        self.mime_type.starts_with("audio/")
    }

    pub(crate) fn base64(&self) -> String {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.encode(&self.bytes)
    }
}

/// The `input_audio` format name for an audio MIME type, `wav` when unknown.
pub(crate) fn audio_format_from_mime(mime: &str) -> &'static str {
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

/// OpenAI-style multimodal content: the text first, then each attachment
/// the model accepts (images as data URLs with `detail: auto`, audio as
/// `input_audio`), a single blank text part when nothing remains.
pub(crate) fn openai_content_parts(
    text: &str,
    attachments: &[ProviderMedia],
    (allow_image, allow_audio): (bool, bool),
) -> serde_json::Value {
    let mut parts = Vec::new();
    if !text.is_empty() {
        parts.push(serde_json::json!({ "type": "text", "text": text }));
    }
    for attachment in attachments {
        if attachment.bytes.is_empty() {
            continue;
        }
        if attachment.is_audio() {
            if allow_audio {
                parts.push(serde_json::json!({
                    "type": "input_audio",
                    "input_audio": {
                        "data": attachment.base64(),
                        "format": audio_format_from_mime(&attachment.mime_type),
                    }
                }));
            }
            continue;
        }
        if allow_image {
            parts.push(serde_json::json!({
                "type": "image_url",
                "image_url": {
                    "url": format!("data:{};base64,{}", attachment.mime_type, attachment.base64()),
                    "detail": "auto",
                }
            }));
        }
    }
    if parts.is_empty() {
        parts.push(serde_json::json!({ "type": "text", "text": " " }));
    }
    serde_json::Value::Array(parts)
}
