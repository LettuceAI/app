use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::types::{AudioModel, ProviderVoice};

const API_BASE: &str = "https://api.fish.audio";

pub fn default_models() -> Vec<AudioModel> {
    vec![
        AudioModel {
            id: "s2-pro".to_string(),
            name: "S2 Pro".to_string(),
            provider_type: "fish_tts".to_string(),
        },
        AudioModel {
            id: "s1".to_string(),
            name: "S1".to_string(),
            provider_type: "fish_tts".to_string(),
        },
    ]
}

#[derive(Debug, Deserialize)]
struct FishModelsResponse {
    items: Vec<FishModel>,
}

#[derive(Debug, Deserialize)]
struct FishModel {
    #[serde(rename = "_id")]
    id: String,
    title: String,
    #[serde(default, rename = "type")]
    model_type: Option<String>,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    languages: Vec<String>,
    #[serde(default)]
    description: String,
}

pub async fn fetch_voices(api_key: &str) -> Result<Vec<ProviderVoice>, String> {
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{API_BASE}/model"))
        .header(AUTHORIZATION, format!("Bearer {}", api_key))
        .query(&[
            ("self", "true"),
            ("page_size", "100"),
            ("sort_by", "created_at"),
        ])
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
            format!("Fish Audio (Cloud) model list error ({}): {}", status, body),
        ));
    }

    let data: FishModelsResponse = response.json().await.map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to parse response: {}", e),
        )
    })?;

    Ok(data
        .items
        .into_iter()
        .filter(|model| model.model_type.as_deref().unwrap_or("tts") == "tts")
        .filter(|model| model.state.as_deref() != Some("failed"))
        .map(|model| {
            let mut labels = HashMap::new();
            if let Some(state) = model.state {
                labels.insert("state".to_string(), state);
            }
            if !model.tags.is_empty() {
                labels.insert("tags".to_string(), model.tags.join(", "));
            }
            if !model.languages.is_empty() {
                labels.insert("languages".to_string(), model.languages.join(", "));
            }
            if !model.description.trim().is_empty() {
                labels.insert(
                    "description".to_string(),
                    model.description.trim().to_string(),
                );
            }
            labels.insert("category".to_string(), "library".to_string());
            labels.insert("engine".to_string(), "fish".to_string());

            ProviderVoice {
                voice_id: model.id,
                name: model.title,
                preview_url: None,
                labels,
            }
        })
        .collect())
}

#[derive(Debug, Serialize)]
struct FishProsodyControl {
    speed: f32,
    volume: i32,
    normalize_loudness: bool,
}

#[derive(Debug, Serialize)]
struct FishTtsRequest<'a> {
    text: &'a str,
    reference_id: &'a str,
    format: &'a str,
    sample_rate: u32,
    mp3_bitrate: u32,
    normalize: bool,
    latency: &'a str,
    prosody: FishProsodyControl,
}

pub async fn generate_speech(
    api_key: &str,
    text: &str,
    voice_id: &str,
    model: &str,
    prompt: Option<&str>,
) -> Result<Vec<u8>, String> {
    let trimmed_voice_id = voice_id.trim();
    if trimmed_voice_id.is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Fish Audio (Cloud) requires a voice model ID",
        ));
    }

    let speed = prompt
        .and_then(parse_speed_from_prompt)
        .unwrap_or(1.0)
        .clamp(0.7, 1.3);

    let request = FishTtsRequest {
        text,
        reference_id: trimmed_voice_id,
        format: "mp3",
        sample_rate: 44100,
        mp3_bitrate: 128,
        normalize: true,
        latency: "normal",
        prosody: FishProsodyControl {
            speed,
            volume: 0,
            normalize_loudness: true,
        },
    };

    let resolved_model = if model.trim().is_empty() {
        "s2-pro"
    } else {
        model
    };
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{API_BASE}/v1/tts"))
        .header(AUTHORIZATION, format!("Bearer {}", api_key))
        .header(CONTENT_TYPE, "application/json")
        .header("model", resolved_model)
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
            format!("Fish Audio (Cloud) TTS error ({}): {}", status, body),
        ));
    }

    response
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to read audio: {}", e),
            )
        })
}

fn parse_speed_from_prompt(prompt: &str) -> Option<f32> {
    serde_json::from_str::<serde_json::Value>(prompt)
        .ok()
        .and_then(|value| value.get("speed").and_then(|speed| speed.as_f64()))
        .map(|value| value as f32)
        .filter(|value| value.is_finite() && *value > 0.0)
}

pub async fn verify_api_key(api_key: &str) -> Result<bool, String> {
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{API_BASE}/model"))
        .header(AUTHORIZATION, format!("Bearer {}", api_key))
        .query(&[("self", "true"), ("page_size", "1")])
        .send()
        .await
        .map_err(|e| {
            crate::utils::err_msg(module_path!(), line!(), format!("Request failed: {}", e))
        })?;

    Ok(response.status().is_success())
}
