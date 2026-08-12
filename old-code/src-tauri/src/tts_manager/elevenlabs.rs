use super::types::{AudioModel, ProviderVoice};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub fn get_models() -> Vec<AudioModel> {
    vec![
        AudioModel {
            id: "eleven_v3".to_string(),
            name: "Eleven v3 (5K chars)".to_string(),
            provider_type: "elevenlabs".to_string(),
        },
        AudioModel {
            id: "eleven_flash_v2_5".to_string(),
            name: "Flash v2.5 (40K chars)".to_string(),
            provider_type: "elevenlabs".to_string(),
        },
        AudioModel {
            id: "eleven_flash_v2".to_string(),
            name: "Flash v2 (30K chars)".to_string(),
            provider_type: "elevenlabs".to_string(),
        },
        AudioModel {
            id: "eleven_turbo_v2_5".to_string(),
            name: "Turbo v2.5 (40K chars)".to_string(),
            provider_type: "elevenlabs".to_string(),
        },
        AudioModel {
            id: "eleven_turbo_v2".to_string(),
            name: "Turbo v2 (30K chars)".to_string(),
            provider_type: "elevenlabs".to_string(),
        },
        AudioModel {
            id: "eleven_multilingual_v2".to_string(),
            name: "Multilingual v2 (10K chars)".to_string(),
            provider_type: "elevenlabs".to_string(),
        },
        AudioModel {
            id: "eleven_multilingual_v1".to_string(),
            name: "Multilingual v1 (10K chars)".to_string(),
            provider_type: "elevenlabs".to_string(),
        },
        AudioModel {
            id: "eleven_english_sts_v2".to_string(),
            name: "English STS v2 (10K chars)".to_string(),
            provider_type: "elevenlabs".to_string(),
        },
    ]
}

pub fn get_voice_design_models() -> Vec<AudioModel> {
    vec![
        AudioModel {
            id: "eleven_multilingual_ttv_v2".to_string(),
            name: "Multilingual TTV v2 (Default)".to_string(),
            provider_type: "elevenlabs".to_string(),
        },
        AudioModel {
            id: "eleven_ttv_v3".to_string(),
            name: "TTV v3 (Latest)".to_string(),
            provider_type: "elevenlabs".to_string(),
        },
    ]
}

#[derive(Deserialize)]
struct VoicesResponse {
    voices: Vec<VoiceData>,
    #[serde(default)]
    #[allow(dead_code)]
    has_more: bool,
}

#[derive(Deserialize)]
struct VoiceData {
    voice_id: String,
    name: String,
    preview_url: Option<String>,
    #[serde(default)]
    labels: Option<HashMap<String, String>>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

pub async fn fetch_voices(
    api_key: &str,
    search: Option<&str>,
) -> Result<Vec<ProviderVoice>, String> {
    let client = reqwest::Client::new();

    let mut url = "https://api.elevenlabs.io/v1/voices".to_string();
    if let Some(query) = search {
        url = format!("{}?search={}", url, urlencoding::encode(query));
    }

    let response = client
        .get(&url)
        .header("xi-api-key", api_key)
        .send()
        .await
        .map_err(|e| {
            crate::utils::err_msg(module_path!(), line!(), format!("Request failed: {}", e))
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("ElevenLabs error ({}): {}", status, body),
        ));
    }

    let data: VoicesResponse = response.json().await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to parse response: {}", e),
        )
    })?;

    Ok(data
        .voices
        .into_iter()
        .map(|v| {
            let mut labels = v.labels.unwrap_or_default();
            if let Some(cat) = v.category {
                labels.insert("category".to_string(), cat);
            }
            if let Some(desc) = v.description {
                labels.insert("description".to_string(), desc);
            }
            ProviderVoice {
                voice_id: v.voice_id,
                name: v.name,
                preview_url: v.preview_url,
                labels,
            }
        })
        .collect())
}

#[derive(Serialize)]
struct TtsRequest {
    text: String,
    model_id: String,
}

pub async fn generate_speech(
    text: &str,
    voice_id: &str,
    model: &str,
    api_key: &str,
) -> Result<Vec<u8>, String> {
    let url = format!(
        "https://api.elevenlabs.io/v1/text-to-speech/{}?output_format=mp3_44100_128",
        voice_id
    );

    let request = TtsRequest {
        text: text.to_string(),
        model_id: model.to_string(),
    };

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("xi-api-key", api_key)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| {
            crate::utils::err_msg(module_path!(), line!(), format!("Request failed: {}", e))
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("ElevenLabs TTS error ({}): {}", status, body),
        ));
    }

    response.bytes().await.map(|b| b.to_vec()).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to read audio: {}", e),
        )
    })
}

#[derive(Serialize)]
struct VoiceDesignRequest {
    text: String,
    voice_description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    loudness: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_previews: Option<u32>,
}

#[derive(Deserialize)]
pub struct VoicePreview {
    #[serde(alias = "generatedVoiceId")]
    pub generated_voice_id: String,
    #[serde(alias = "audio_base64")]
    pub audio_base_64: String,
    #[serde(alias = "durationSecs")]
    #[serde(default)]
    pub duration_secs: f32,
    #[serde(alias = "mediaType")]
    pub media_type: String,
}

#[derive(Deserialize)]
struct VoiceDesignResponse {
    previews: Vec<VoicePreview>,
}

pub async fn design_voice(
    api_key: &str,
    text_sample: &str,
    voice_description: &str,
    model_id: Option<&str>,
    num_previews: Option<u32>,
) -> Result<Vec<VoicePreview>, String> {
    let url = "https://api.elevenlabs.io/v1/text-to-voice/design";

    let request = VoiceDesignRequest {
        text: text_sample.to_string(),
        voice_description: voice_description.to_string(),
        model_id: model_id.map(|s| s.to_string()),
        loudness: None,
        num_previews,
    };

    let client = reqwest::Client::new();
    let response = client
        .post(url)
        .header("xi-api-key", api_key)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| {
            crate::utils::err_msg(module_path!(), line!(), format!("Request failed: {}", e))
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Voice design error ({}): {}", status, body),
        ));
    }

    let data: VoiceDesignResponse = response.json().await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to parse response: {}", e),
        )
    })?;

    Ok(data.previews)
}

#[derive(Serialize)]
struct CreateVoiceRequest {
    voice_name: String,
    generated_voice_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    voice_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    labels: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
pub struct CreatedVoice {
    pub voice_id: String,
}

pub async fn create_voice_from_preview(
    api_key: &str,
    voice_name: &str,
    generated_voice_id: &str,
    voice_description: Option<&str>,
    labels: Option<HashMap<String, String>>,
) -> Result<CreatedVoice, String> {
    let url = "https://api.elevenlabs.io/v1/text-to-voice";

    let request = CreateVoiceRequest {
        voice_name: voice_name.to_string(),
        generated_voice_id: generated_voice_id.to_string(),
        voice_description: voice_description.map(|s| s.to_string()),
        labels,
    };

    let client = reqwest::Client::new();
    let response = client
        .post(url)
        .header("xi-api-key", api_key)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| {
            crate::utils::err_msg(module_path!(), line!(), format!("Request failed: {}", e))
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Create voice error ({}): {}", status, body),
        ));
    }

    response.json().await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to parse response: {}", e),
        )
    })
}

pub async fn verify_api_key(api_key: &str) -> Result<bool, String> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.elevenlabs.io/v1/voices")
        .header("xi-api-key", api_key)
        .send()
        .await
        .map_err(|e| {
            crate::utils::err_msg(module_path!(), line!(), format!("Request failed: {}", e))
        })?;

    Ok(response.status().is_success())
}
