use serde::{Deserialize, Serialize};

use crate::{AssetRef, LoraKeywordDiscovery};

/// What an avatar image prompt is for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum AvatarPromptKind {
    Generation {
        subject_name: String,
        subject_description: String,
        avatar_request: String,
    },
    Edit {
        subject_name: String,
        subject_description: String,
        current_avatar_prompt: String,
        edit_request: String,
    },
}

/// The prompt an avatar generation or edit sends to `model_id`'s provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AvatarPromptRequest {
    pub model_id: String,
    pub kind: AvatarPromptKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AvatarPrompt {
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AvatarGradientRequest {
    pub asset_id: String,
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct GradientColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub hex: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AvatarGradient {
    pub colors: Vec<GradientColor>,
    pub gradient_css: String,
    pub dominant_hue: f64,
    pub text_color: String,
    pub text_secondary: String,
}

/// Design notes written from a subject's avatar and reference images; the
/// job's `GeneratedText` result carries the text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ImageDesignReferenceRequest {
    pub subject_name: Option<String>,
    pub subject_description: Option<String>,
    pub current_description: Option<String>,
    pub avatar: Option<String>,
    pub references: Vec<String>,
    pub client_operation_id: String,
}

/// What a finished LoRA discovery job found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LoraDiscovered {
    pub discovery: LoraKeywordDiscovery,
}

/// The upscaled image and the playground entry that records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ImageUpscaled {
    pub asset: AssetRef,
    pub mime_type: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub history_id: Option<String>,
}
