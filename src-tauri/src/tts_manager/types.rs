use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AudioProviderType {
    GeminiTts,
    Elevenlabs,
    OpenAiTts,
    Kokoro,
}

impl AudioProviderType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "gemini_tts" => Some(Self::GeminiTts),
            "elevenlabs" => Some(Self::Elevenlabs),
            "openai_tts" => Some(Self::OpenAiTts),
            "kokoro" => Some(Self::Kokoro),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioProvider {
    pub id: String,
    pub provider_type: String,
    pub label: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub request_path: Option<String>,
    #[serde(default)]
    pub kokoro_variant: Option<String>,
    #[serde(default)]
    pub asset_root: Option<String>,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioModel {
    pub id: String,
    pub name: String,
    pub provider_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderVoice {
    pub voice_id: String,
    pub name: String,
    #[serde(default)]
    pub preview_url: Option<String>,
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedVoice {
    pub id: String,
    pub provider_id: String,
    pub voice_id: String,
    pub name: String,
    #[serde(default)]
    pub preview_url: Option<String>,
    #[serde(default)]
    pub labels: HashMap<String, String>,
    pub cached_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserVoice {
    pub id: String,
    pub provider_id: String,
    pub name: String,
    pub model_id: String,
    pub voice_id: String,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsPreviewResponse {
    pub audio_base64: String,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceDesignPreview {
    pub generated_voice_id: String,
    pub audio_base64: String,
    pub duration_secs: f32,
    pub media_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedVoiceResult {
    pub voice_id: String,
}
