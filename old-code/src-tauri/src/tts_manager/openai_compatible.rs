use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::Serialize;

use super::types::AudioModel;

const DEFAULT_REQUEST_PATH: &str = "/v1/audio/speech";

pub fn default_models() -> Vec<AudioModel> {
    vec![
        AudioModel {
            id: "gpt-4o-mini-tts".to_string(),
            name: "gpt-4o-mini-tts".to_string(),
            provider_type: "openai_tts".to_string(),
        },
        AudioModel {
            id: "tts-1".to_string(),
            name: "tts-1".to_string(),
            provider_type: "openai_tts".to_string(),
        },
        AudioModel {
            id: "tts-1-hd".to_string(),
            name: "tts-1-hd".to_string(),
            provider_type: "openai_tts".to_string(),
        },
    ]
}

#[derive(Serialize)]
struct OpenAiTtsRequest<'a> {
    model: &'a str,
    input: &'a str,
    voice: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<&'a str>,
    response_format: &'a str,
}

fn normalize_base_url(base_url: &str) -> String {
    base_url.trim().trim_end_matches('/').to_string()
}

fn normalize_request_path(request_path: Option<&str>) -> String {
    let trimmed = request_path.unwrap_or(DEFAULT_REQUEST_PATH).trim();
    if trimmed.is_empty() {
        return DEFAULT_REQUEST_PATH.to_string();
    }
    if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{}", trimmed)
    }
}

pub async fn generate_speech(
    base_url: &str,
    request_path: Option<&str>,
    api_key: &str,
    text: &str,
    voice_id: &str,
    model: &str,
    prompt: Option<&str>,
) -> Result<(Vec<u8>, String), String> {
    let url = format!(
        "{}{}",
        normalize_base_url(base_url),
        normalize_request_path(request_path)
    );

    let instructions = prompt.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });

    let request = OpenAiTtsRequest {
        model,
        input: text,
        voice: voice_id,
        instructions,
        response_format: "mp3",
    };

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header(AUTHORIZATION, format!("Bearer {}", api_key))
        .header(CONTENT_TYPE, "application/json")
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
            format!("OpenAI-compatible TTS error ({}): {}", status, body),
        ));
    }

    let format = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            value
                .split(';')
                .next()
                .unwrap_or("audio/mpeg")
                .trim()
                .to_string()
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "audio/mpeg".to_string());

    response
        .bytes()
        .await
        .map(|bytes| (bytes.to_vec(), format))
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to read audio: {}", e),
            )
        })
}
