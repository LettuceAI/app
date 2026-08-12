use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::Serialize;

use super::types::AudioModel;

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8080";
const DEFAULT_REQUEST_PATH: &str = "/v1/tts";
const HEALTH_PATH: &str = "/v1/health";

pub fn default_models() -> Vec<AudioModel> {
    vec![AudioModel {
        id: "server-default".to_string(),
        name: "Server Default (configured at startup)".to_string(),
        provider_type: "fish_speech".to_string(),
    }]
}

#[derive(Serialize)]
struct FishSpeechRequest<'a> {
    text: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    reference_id: Option<&'a str>,
    format: &'a str,
}

fn normalize_base_url(base_url: Option<&str>) -> String {
    base_url
        .unwrap_or(DEFAULT_BASE_URL)
        .trim()
        .trim_end_matches('/')
        .to_string()
}

fn normalize_request_path(request_path: Option<&str>) -> String {
    let trimmed = request_path.unwrap_or(DEFAULT_REQUEST_PATH).trim();
    if trimmed.is_empty() {
        DEFAULT_REQUEST_PATH.to_string()
    } else if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{}", trimmed)
    }
}

fn maybe_bearer(
    builder: reqwest::RequestBuilder,
    api_key: Option<&str>,
) -> reqwest::RequestBuilder {
    match api_key.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => builder.header(AUTHORIZATION, format!("Bearer {}", value)),
        None => builder,
    }
}

pub async fn generate_speech(
    base_url: Option<&str>,
    request_path: Option<&str>,
    api_key: Option<&str>,
    text: &str,
    voice_id: &str,
) -> Result<Vec<u8>, String> {
    let url = format!(
        "{}{}",
        normalize_base_url(base_url),
        normalize_request_path(request_path)
    );
    let reference_id = voice_id.trim();
    let request = FishSpeechRequest {
        text,
        reference_id: if reference_id.is_empty() {
            None
        } else {
            Some(reference_id)
        },
        format: "mp3",
    };

    let client = reqwest::Client::new();
    let response = maybe_bearer(
        client
            .post(&url)
            .header(CONTENT_TYPE, "application/json")
            .json(&request),
        api_key,
    )
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
            format!("Fish Speech (Local) TTS error ({}): {}", status, body),
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

pub async fn verify_server(base_url: Option<&str>, api_key: Option<&str>) -> Result<bool, String> {
    let url = format!("{}{}", normalize_base_url(base_url), HEALTH_PATH);
    let client = reqwest::Client::new();
    let response = maybe_bearer(client.get(&url), api_key)
        .send()
        .await
        .map_err(|e| {
            crate::utils::err_msg(module_path!(), line!(), format!("Request failed: {}", e))
        })?;
    Ok(response.status().is_success())
}
