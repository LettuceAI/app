use super::types::{AudioModel, ProviderVoice};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub fn get_models() -> Vec<AudioModel> {
    vec![
        AudioModel {
            id: "gemini-2.5-flash-tts".to_string(),
            name: "Gemini 2.5 Flash TTS".to_string(),
            provider_type: "gemini_tts".to_string(),
        },
        AudioModel {
            id: "gemini-2.5-flash-lite-preview-tts".to_string(),
            name: "Gemini 2.5 Flash Lite TTS (Preview)".to_string(),
            provider_type: "gemini_tts".to_string(),
        },
        AudioModel {
            id: "gemini-2.5-pro-tts".to_string(),
            name: "Gemini 2.5 Pro TTS".to_string(),
            provider_type: "gemini_tts".to_string(),
        },
    ]
}

pub fn get_voices() -> Vec<ProviderVoice> {
    let voices = [
        ("Kore", "female", "Warm and friendly"),
        ("Aoede", "female", "Bright and articulate"),
        ("Algieba", "male", "Professional and clear"),
        ("Callirrhoe", "female", "Expressive and dynamic"),
        ("Leda", "female", "Calm and soothing"),
        ("Puck", "male", "Energetic and youthful"),
        ("Charon", "male", "Deep and authoritative"),
        ("Fenrir", "male", "Strong and bold"),
        ("Orus", "male", "Warm and resonant"),
        ("Zephyr", "neutral", "Light and airy"),
    ];

    voices
        .iter()
        .map(|(name, gender, description)| {
            let mut labels = HashMap::new();
            labels.insert("gender".to_string(), gender.to_string());
            labels.insert("description".to_string(), description.to_string());
            ProviderVoice {
                voice_id: name.to_lowercase(),
                name: name.to_string(),
                preview_url: None,
                labels,
            }
        })
        .collect()
}

#[derive(Serialize)]
struct GeminiRequest {
    contents: GeminiContents,
    generation_config: GeminiGenerationConfig,
}

#[derive(Serialize)]
struct GeminiContents {
    role: String,
    parts: GeminiParts,
}

#[derive(Serialize)]
struct GeminiParts {
    text: String,
}

#[derive(Serialize)]
struct GeminiGenerationConfig {
    speech_config: GeminiSpeechConfig,
}

#[derive(Serialize)]
struct GeminiSpeechConfig {
    language_code: String,
    voice_config: GeminiVoiceConfig,
}

#[derive(Serialize)]
struct GeminiVoiceConfig {
    prebuilt_voice_config: GeminiPrebuiltVoiceConfig,
}

#[derive(Serialize)]
struct GeminiPrebuiltVoiceConfig {
    voice_name: String,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: GeminiContent,
}

#[derive(Deserialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Deserialize)]
struct GeminiPart {
    #[serde(rename = "inlineData")]
    inline_data: Option<GeminiInlineData>,
}

#[derive(Deserialize)]
struct GeminiInlineData {
    data: String,
}

pub async fn generate_speech(
    text: &str,
    voice_id: &str,
    model: &str,
    prompt: Option<&str>,
    api_key: &str,
    project_id: &str,
    location: Option<&str>,
) -> Result<Vec<u8>, String> {
    let location = location.unwrap_or("us-central1");

    let full_text = match prompt {
        Some(p) if !p.is_empty() => format!("{}: {}", p, text),
        _ => text.to_string(),
    };

    let url = format!(
        "https://{}-aiplatform.googleapis.com/v1beta1/projects/{}/locations/{}/publishers/google/models/{}:generateContent",
        location, project_id, location, model
    );

    let request = GeminiRequest {
        contents: GeminiContents {
            role: "user".to_string(),
            parts: GeminiParts { text: full_text },
        },
        generation_config: GeminiGenerationConfig {
            speech_config: GeminiSpeechConfig {
                language_code: "en-us".to_string(),
                voice_config: GeminiVoiceConfig {
                    prebuilt_voice_config: GeminiPrebuiltVoiceConfig {
                        voice_name: resolve_voice_name(voice_id),
                    },
                },
            },
        },
    };

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("x-goog-user-project", project_id)
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
            format!("Gemini TTS error ({}): {}", status, body),
        ));
    }

    let response: GeminiResponse = response.json().await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to parse response: {}", e),
        )
    })?;

    let audio_data = response
        .candidates
        .first()
        .and_then(|c| c.content.parts.first())
        .and_then(|p| p.inline_data.as_ref())
        .map(|d| &d.data)
        .ok_or_else(|| "No audio data in response".to_string())?;

    STANDARD.decode(audio_data).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to decode audio: {}", e),
        )
    })
}

fn resolve_voice_name(voice_id: &str) -> String {
    let trimmed = voice_id.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("preview") {
        return "kore".to_string();
    }

    trimmed.to_lowercase()
}

pub async fn verify_api_key(api_key: &str, project_id: &str) -> Result<bool, String> {
    let url = format!(
        "https://us-central1-aiplatform.googleapis.com/v1beta1/projects/{}/locations/us-central1/publishers/google/models",
        project_id
    );

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("x-goog-user-project", project_id)
        .send()
        .await
        .map_err(|e| {
            crate::utils::err_msg(module_path!(), line!(), format!("Request failed: {}", e))
        })?;

    Ok(response.status().is_success())
}
