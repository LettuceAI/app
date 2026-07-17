use serde::{Deserialize, Serialize};

use crate::chat_manager::types::AdvancedModelSettings;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageLora {
    pub path: String,
    pub multiplier: f64,
    #[serde(default)]
    pub is_high_noise: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageGenerationRequest {
    pub prompt: String,
    pub model: String,
    pub provider_id: String,
    pub credential_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advanced_model_settings: Option<AdvancedModelSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_images: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask_image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loras: Option<Vec<ImageLora>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_modalities: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
    /// Optional usage attribution: when set, recorded usage is associated
    /// with the originating chat session/character instead of the generic
    /// "Image Generation" placeholder. Standalone callers can omit these.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character_name: Option<String>,
    /// Optional sub-flow tag (e.g. "scene", "manual"). Stored in metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedImage {
    pub asset_id: String,
    pub file_path: String,
    pub mime_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// Image generation response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageGenerationResponse {
    pub images: Vec<GeneratedImage>,
    pub model: String,
    pub provider_id: String,
}
