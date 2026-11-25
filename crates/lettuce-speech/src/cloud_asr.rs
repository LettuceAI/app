use std::sync::Arc;

use base64::Engine as _;
use lettuce_jobs::handle::CancellationToken;
use lettuce_network::{
    JsonAuth, JsonClient, JsonClientError, JsonStaticHeader, MultipartField, RequestPolicy,
};
use lettuce_settings::{HeaderName, SecretValue};
use serde::{Deserialize, Serialize};

use crate::{
    AsrRuntimeError, GEMINI_DEFAULT_ENDPOINT, NANOGPT_DEFAULT_ENDPOINT,
    OPENROUTER_DEFAULT_ENDPOINT, RuntimeTranscription, TranscriptionOptions, TranscriptionSegment,
};

pub const OPENROUTER_MAX_AUDIO_BYTES: usize = 25 * 1024 * 1024;
pub const GEMINI_MAX_INLINE_AUDIO_BYTES: usize = 20 * 1024 * 1024;

const OPENROUTER_HEADERS: [JsonStaticHeader; 2] = [
    JsonStaticHeader {
        name: "X-OpenRouter-Title",
        value: "LettuceAI",
    },
    JsonStaticHeader {
        name: "X-OpenRouter-Categories",
        value: "roleplay",
    },
];

const ACCEPT_JSON: [JsonStaticHeader; 1] = [JsonStaticHeader {
    name: "Accept",
    value: "application/json",
}];

const OPENROUTER_ACCEPT_HEADERS: [JsonStaticHeader; 3] = [
    JsonStaticHeader {
        name: "Accept",
        value: "application/json",
    },
    OPENROUTER_HEADERS[0],
    OPENROUTER_HEADERS[1],
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudAsrProviderKind {
    OpenRouter,
    NanoGpt,
    Gemini,
}

impl CloudAsrProviderKind {
    #[must_use]
    pub fn from_provider_kind(kind: &str) -> Option<Self> {
        match kind.trim().to_ascii_lowercase().as_str() {
            "openrouter" => Some(Self::OpenRouter),
            "nanogpt" => Some(Self::NanoGpt),
            "gemini" | "google" | "google-gemini" => Some(Self::Gemini),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenRouter => "openrouter",
            Self::NanoGpt => "nanogpt",
            Self::Gemini => "gemini",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudAsrModel {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct CloudAsrRuntime {
    network: Arc<JsonClient>,
}

impl CloudAsrRuntime {
    #[must_use]
    pub fn new(network: Arc<JsonClient>) -> Self {
        Self { network }
    }

    pub async fn fetch_models(
        &self,
        endpoint: Option<&str>,
        kind: CloudAsrProviderKind,
        credential: &SecretValue,
    ) -> Result<Vec<CloudAsrModel>, AsrRuntimeError> {
        let base = resolved_endpoint(endpoint, kind);
        let path = models_path(base, kind);
        let auth = auth_for(kind, credential)?;
        let headers = match kind {
            CloudAsrProviderKind::OpenRouter => &OPENROUTER_ACCEPT_HEADERS[..],
            CloudAsrProviderKind::NanoGpt | CloudAsrProviderKind::Gemini => &ACCEPT_JSON[..],
        };
        let response = self
            .network
            .get_json(base, path, headers, auth, Vec::new(), RequestPolicy::BROWSE)
            .await
            .map_err(map_network)?;
        if !(200..300).contains(&response.status) {
            return Err(map_http_status(response.status));
        }
        let body: serde_json::Value =
            serde_json::from_slice(&response.body).map_err(|_| AsrRuntimeError::Failed {
                reason: "cloud stt provider returned malformed model catalog json",
            })?;
        Ok(match kind {
            CloudAsrProviderKind::OpenRouter => parse_openrouter_stt_models(&body),
            CloudAsrProviderKind::NanoGpt => parse_nanogpt_stt_models(&body),
            CloudAsrProviderKind::Gemini => parse_gemini_stt_models(&body),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn transcribe(
        &self,
        endpoint: Option<&str>,
        kind: CloudAsrProviderKind,
        model_id: &str,
        mono_16khz: &[f32],
        prompt: &str,
        gemini_instruction: Option<&str>,
        options: &TranscriptionOptions,
        credential: &SecretValue,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeTranscription, AsrRuntimeError> {
        if cancellation.is_cancelled() {
            return Err(AsrRuntimeError::Cancelled);
        }
        let base = resolved_endpoint(endpoint, kind);
        let wav = encode_wav_pcm16(mono_16khz);
        let language = options
            .language
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("auto"));
        let trimmed_prompt = {
            let trimmed = prompt.trim();
            (!trimmed.is_empty()).then_some(trimmed)
        };
        if kind == CloudAsrProviderKind::Gemini {
            return self
                .transcribe_gemini(
                    base,
                    model_id,
                    &wav,
                    language,
                    trimmed_prompt,
                    gemini_instruction,
                    credential,
                    cancellation,
                )
                .await;
        }
        let path = transcriptions_path(base);
        let auth = bearer_auth(credential)?;

        let response = match kind {
            CloudAsrProviderKind::OpenRouter => {
                if wav.len() > OPENROUTER_MAX_AUDIO_BYTES {
                    return Err(AsrRuntimeError::RequestTooLarge {
                        reason: "audio exceeds the OpenRouter 25 MB upload limit",
                    });
                }
                let mut fields = vec![
                    MultipartField::File {
                        name: "file".into(),
                        filename: "audio.wav".into(),
                        mime_type: "audio/wav".into(),
                        bytes: wav,
                    },
                    MultipartField::Text {
                        name: "model".into(),
                        value: model_id.to_owned(),
                    },
                    MultipartField::Text {
                        name: "response_format".into(),
                        value: "verbose_json".into(),
                    },
                ];
                if let Some(language) = language {
                    fields.push(MultipartField::Text {
                        name: "language".into(),
                        value: language.to_owned(),
                    });
                }
                if let Some(prompt) = trimmed_prompt {
                    fields.push(MultipartField::Text {
                        name: "prompt".into(),
                        value: prompt.to_owned(),
                    });
                }
                tokio::select! {
                    response = self.network.post_multipart(
                        base,
                        path,
                        fields,
                        &OPENROUTER_HEADERS,
                        auth,
                        Vec::new(),
                        RequestPolicy::GENERATION,
                    ) => response.map_err(map_network)?,
                    () = cancellation.cancelled() => return Err(AsrRuntimeError::Cancelled),
                }
            }
            CloudAsrProviderKind::NanoGpt => {
                let mut fields = vec![
                    MultipartField::File {
                        name: "file".into(),
                        filename: "audio.wav".into(),
                        mime_type: "audio/wav".into(),
                        bytes: wav,
                    },
                    MultipartField::Text {
                        name: "model".into(),
                        value: model_id.to_owned(),
                    },
                ];
                if let Some(language) = language {
                    fields.push(MultipartField::Text {
                        name: "language".into(),
                        value: language.to_owned(),
                    });
                }
                tokio::select! {
                    response = self.network.post_multipart(
                        base,
                        path,
                        fields,
                        &[],
                        auth,
                        Vec::new(),
                        RequestPolicy::GENERATION,
                    ) => response.map_err(map_network)?,
                    () = cancellation.cancelled() => return Err(AsrRuntimeError::Cancelled),
                }
            }
            CloudAsrProviderKind::Gemini => unreachable!("handled above"),
        };

        parse_cloud_transcription_http_response(response.status, &response.body)
    }

    #[allow(clippy::too_many_arguments)]
    async fn transcribe_gemini(
        &self,
        base: &str,
        model_id: &str,
        wav: &[u8],
        language: Option<&str>,
        prompt: Option<&str>,
        gemini_instruction: Option<&str>,
        credential: &SecretValue,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeTranscription, AsrRuntimeError> {
        if wav.len() > GEMINI_MAX_INLINE_AUDIO_BYTES {
            return Err(AsrRuntimeError::RequestTooLarge {
                reason: "audio exceeds the Gemini 20 MB inline limit",
            });
        }
        let model = model_id.trim().trim_start_matches("models/");
        let path = if base.ends_with("/v1beta") || base.ends_with("/v1") {
            format!("/models/{model}:generateContent")
        } else {
            format!("/v1beta/models/{model}:generateContent")
        };
        let auth = auth_for(CloudAsrProviderKind::Gemini, credential)?;
        let encoded = base64::engine::general_purpose::STANDARD.encode(wav);
        let is_dedicated = model.contains("transcribe");
        let mut parts = Vec::new();
        if !is_dedicated {
            let instruction = gemini_instruction
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or(AsrRuntimeError::Rejected {
                    reason: "gemini transcription instruction prompt is missing",
                })?;
            parts.push(serde_json::json!({ "text": instruction }));
        } else if let Some(hint) = prompt {
            parts.push(serde_json::json!({ "text": hint }));
        }
        parts.push(serde_json::json!({
            "inlineData": {
                "mimeType": "audio/wav",
                "data": encoded,
            }
        }));
        let mut payload = serde_json::json!({
            "contents": [{ "role": "user", "parts": parts }]
        });
        if is_dedicated && let Some(lang) = language {
            payload["generationConfig"] = serde_json::json!({
                "audioTranscriptionConfig": {
                    "languageCodes": [lang]
                }
            });
        }
        let body = serde_json::to_vec(&payload).map_err(|_| AsrRuntimeError::Rejected {
            reason: "could not serialize gemini transcription request",
        })?;
        let response = tokio::select! {
            response = self.network.post_json(
                base,
                &path,
                body,
                &[],
                auth,
                Vec::new(),
                RequestPolicy::GENERATION,
            ) => response.map_err(map_network)?,
            () = cancellation.cancelled() => return Err(AsrRuntimeError::Cancelled),
        };
        if !(200..300).contains(&response.status) {
            return Err(map_http_status(response.status));
        }
        let body: serde_json::Value =
            serde_json::from_slice(&response.body).map_err(|_| AsrRuntimeError::Failed {
                reason: "gemini stt returned malformed json",
            })?;
        let text = body
            .get("candidates")
            .and_then(serde_json::Value::as_array)
            .and_then(|c| c.first())
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(serde_json::Value::as_array)
            .map(|parts| {
                parts
                    .iter()
                    .filter_map(|p| p.get("text").and_then(serde_json::Value::as_str))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default()
            .trim()
            .to_owned();
        let transcription = RuntimeTranscription {
            raw_text: text,
            detected_language: None,
            segments: Vec::new(),
        };
        transcription
            .validate()
            .map_err(|_| AsrRuntimeError::Failed {
                reason: "gemini transcription response failed validation",
            })?;
        Ok(transcription)
    }
}

pub fn parse_cloud_transcription_http_response(
    status: u16,
    body: &[u8],
) -> Result<RuntimeTranscription, AsrRuntimeError> {
    if status == 202 {
        return Err(AsrRuntimeError::UnsupportedAsyncModel {
            reason: "provider returned 202 pending for an asynchronous speech model",
        });
    }
    if !(200..300).contains(&status) {
        return Err(map_http_status(status));
    }
    let parsed: serde_json::Value =
        serde_json::from_slice(body).map_err(|_| AsrRuntimeError::Failed {
            reason: "cloud stt provider returned malformed transcription json",
        })?;
    if parsed.get("status").and_then(serde_json::Value::as_str) == Some("pending")
        && parsed.get("run_id").is_some()
    {
        return Err(AsrRuntimeError::UnsupportedAsyncModel {
            reason: "provider returned a pending async transcription job",
        });
    }
    parse_transcription_response(&parsed)
}

#[must_use]
pub fn encode_wav_pcm16(mono_16khz: &[f32]) -> Vec<u8> {
    let sample_rate: u32 = 16_000;
    let channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * u32::from(channels) * u32::from(bits_per_sample / 8);
    let block_align = channels * (bits_per_sample / 8);
    let data_bytes = (mono_16khz.len() * 2) as u32;
    let riff_size = 36u32.saturating_add(data_bytes);

    let mut out = Vec::with_capacity(44 + mono_16khz.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits_per_sample.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_bytes.to_le_bytes());
    for &sample in mono_16khz {
        let clamped = if sample.is_finite() {
            sample.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        let pcm = (clamped * f32::from(i16::MAX)).round() as i16;
        out.extend_from_slice(&pcm.to_le_bytes());
    }
    out
}

pub fn parse_transcription_response(
    body: &serde_json::Value,
) -> Result<RuntimeTranscription, AsrRuntimeError> {
    let raw_text = body
        .get("text")
        .and_then(serde_json::Value::as_str)
        .ok_or(AsrRuntimeError::Failed {
            reason: "transcription response is missing text",
        })?
        .trim()
        .to_owned();
    let detected_language = body
        .get("language")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let segments = body
        .get("segments")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .enumerate()
                .filter_map(|(index, item)| {
                    let text = item.get("text").and_then(serde_json::Value::as_str)?;
                    let start = item
                        .get("start")
                        .and_then(serde_json::Value::as_f64)
                        .unwrap_or(0.0);
                    let end = item
                        .get("end")
                        .and_then(serde_json::Value::as_f64)
                        .unwrap_or(start);
                    let start_ms = seconds_to_ms(start);
                    let end_ms = seconds_to_ms(end).max(start_ms);
                    let no_speech_probability = item
                        .get("no_speech_prob")
                        .and_then(serde_json::Value::as_f64)
                        .map(|value| value.clamp(0.0, 1.0) as f32)
                        .unwrap_or(0.0);
                    Some(TranscriptionSegment {
                        index: u32::try_from(index).unwrap_or(u32::MAX),
                        start_ms,
                        end_ms,
                        text: text.trim().to_owned(),
                        no_speech_probability,
                        speaker_turn_next: false,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let transcription = RuntimeTranscription {
        raw_text,
        detected_language,
        segments,
    };
    transcription
        .validate()
        .map_err(|_| AsrRuntimeError::Failed {
            reason: "transcription response failed validation",
        })?;
    Ok(transcription)
}

pub fn parse_openrouter_stt_models(body: &serde_json::Value) -> Vec<CloudAsrModel> {
    let Some(items) = body.get("data").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let id = item.get("id").and_then(serde_json::Value::as_str)?.trim();
            if id.is_empty() {
                return None;
            }
            let name = item
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(id)
                .to_owned();
            Some(CloudAsrModel {
                id: id.to_owned(),
                name,
            })
        })
        .collect()
}

pub fn parse_nanogpt_stt_models(body: &serde_json::Value) -> Vec<CloudAsrModel> {
    let Some(items) = body.get("data").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            if item
                .get("capabilities")
                .and_then(|caps| caps.get("speech_to_text"))
                .and_then(serde_json::Value::as_bool)
                == Some(false)
            {
                return None;
            }
            let id = item.get("id").and_then(serde_json::Value::as_str)?.trim();
            if id.is_empty() {
                return None;
            }
            let name = item
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(id)
                .to_owned();
            Some(CloudAsrModel {
                id: id.to_owned(),
                name,
            })
        })
        .collect()
}

pub fn parse_gemini_stt_models(body: &serde_json::Value) -> Vec<CloudAsrModel> {
    let Some(items) = body.get("models").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let raw_name = item.get("name").and_then(serde_json::Value::as_str)?.trim();
            let id = raw_name.strip_prefix("models/").unwrap_or(raw_name).trim();
            if id.is_empty() {
                return None;
            }
            let lower = id.to_ascii_lowercase();
            let supports_generate = item
                .get("supportedGenerationMethods")
                .and_then(serde_json::Value::as_array)
                .is_none_or(|methods| {
                    methods
                        .iter()
                        .any(|method| method.as_str() == Some("generateContent"))
                });
            if !supports_generate {
                return None;
            }
            let is_audio_capable = lower.contains("transcribe")
                || (lower.starts_with("gemini-")
                    && !lower.contains("tts")
                    && !lower.contains("image")
                    && !lower.contains("embedding")
                    && !lower.contains("live"));
            if !is_audio_capable {
                return None;
            }
            let name = item
                .get("displayName")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(id)
                .to_owned();
            Some(CloudAsrModel {
                id: id.to_owned(),
                name,
            })
        })
        .collect()
}

fn seconds_to_ms(seconds: f64) -> u64 {
    if !seconds.is_finite() || seconds <= 0.0 {
        return 0;
    }
    (seconds * 1000.0).round() as u64
}

fn resolved_endpoint(endpoint: Option<&str>, kind: CloudAsrProviderKind) -> &str {
    let custom = endpoint.map(str::trim).filter(|value| !value.is_empty());
    let base = match (custom, kind) {
        (Some(url), _) => url,
        (None, CloudAsrProviderKind::OpenRouter) => OPENROUTER_DEFAULT_ENDPOINT,
        (None, CloudAsrProviderKind::NanoGpt) => NANOGPT_DEFAULT_ENDPOINT,
        (None, CloudAsrProviderKind::Gemini) => GEMINI_DEFAULT_ENDPOINT,
    };
    base.trim_end_matches('/')
}

fn transcriptions_path(base: &str) -> &'static str {
    if base.ends_with("/v1") {
        "/audio/transcriptions"
    } else {
        "/v1/audio/transcriptions"
    }
}

fn models_path(base: &str, kind: CloudAsrProviderKind) -> &'static str {
    match kind {
        CloudAsrProviderKind::OpenRouter => {
            if base.ends_with("/v1") {
                "/models?output_modalities=transcription"
            } else {
                "/v1/models?output_modalities=transcription"
            }
        }
        CloudAsrProviderKind::NanoGpt => {
            if base.ends_with("/v1") {
                "/audio-models?type=stt&detailed=true"
            } else {
                "/v1/audio-models?type=stt&detailed=true"
            }
        }
        CloudAsrProviderKind::Gemini => {
            if base.ends_with("/v1beta") || base.ends_with("/v1") {
                "/models"
            } else {
                "/v1beta/models"
            }
        }
    }
}

fn auth_for(
    kind: CloudAsrProviderKind,
    credential: &SecretValue,
) -> Result<JsonAuth, AsrRuntimeError> {
    let secret = credential
        .with(|value| SecretValue::new(value.to_owned()))
        .map_err(|_| AsrRuntimeError::Unauthorized {
            reason: "cloud stt api key is empty",
        })?;
    if kind == CloudAsrProviderKind::Gemini {
        return Ok(JsonAuth::Header {
            name: HeaderName::new("x-goog-api-key").expect("static header name"),
            value: secret,
        });
    }
    Ok(JsonAuth::Bearer(secret))
}

fn bearer_auth(credential: &SecretValue) -> Result<JsonAuth, AsrRuntimeError> {
    auth_for(CloudAsrProviderKind::OpenRouter, credential)
}

fn map_http_status(status: u16) -> AsrRuntimeError {
    match status {
        401 | 403 => AsrRuntimeError::Unauthorized {
            reason: "cloud stt provider rejected credentials",
        },
        408 | 429 | 500..=599 => AsrRuntimeError::ProviderUnavailable { status },
        400..=499 => AsrRuntimeError::Rejected {
            reason: "cloud stt provider rejected the request",
        },
        _ => AsrRuntimeError::Unavailable {
            reason: "cloud stt provider returned an unexpected http status",
        },
    }
}

fn map_network(error: JsonClientError) -> AsrRuntimeError {
    tracing::warn!(%error, "cloud stt request failed");
    match error {
        JsonClientError::RequestTooLarge => AsrRuntimeError::RequestTooLarge {
            reason: "transcription request exceeds http client size limit",
        },
        JsonClientError::InvalidUrl { .. }
        | JsonClientError::InvalidRequest { .. }
        | JsonClientError::ResponseTooLarge => AsrRuntimeError::Rejected {
            reason: "transcription request url, body or response size was rejected",
        },
        JsonClientError::Transport { reason } | JsonClientError::ClientConfiguration { reason } => {
            AsrRuntimeError::Unavailable { reason }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_valid_16khz_mono_pcm16_wav() {
        let wav = encode_wav_pcm16(&[0.0, 0.5, -0.5, 1.0]);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(wav.len(), 44 + 8);
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 16_000);
        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 8);
    }

    #[test]
    fn parses_verbose_and_plain_transcription_responses() {
        let verbose = serde_json::json!({
            "text": "Hello world",
            "language": "en",
            "segments": [
                { "id": 0, "start": 0.0, "end": 1.25, "text": " Hello", "no_speech_prob": 0.02 },
                { "id": 1, "start": 1.25, "end": 2.5, "text": " world", "no_speech_prob": 0.01 }
            ]
        });
        let parsed = parse_transcription_response(&verbose).expect("verbose");
        assert_eq!(parsed.raw_text, "Hello world");
        assert_eq!(parsed.detected_language.as_deref(), Some("en"));
        assert_eq!(parsed.segments.len(), 2);
        assert_eq!(parsed.segments[0].end_ms, 1250);
        assert_eq!(parsed.segments[1].text, "world");

        let plain = serde_json::json!({ "text": "Plain text" });
        let parsed_plain = parse_transcription_response(&plain).expect("plain");
        assert_eq!(parsed_plain.raw_text, "Plain text");
        assert!(parsed_plain.segments.is_empty());
    }

    #[test]
    fn maps_http_202_pending_to_typed_async_unsupported_error() {
        let body = br#"{"status":"pending","run_id":"abc-123"}"#;
        let err = parse_cloud_transcription_http_response(202, body)
            .expect_err("202 must fail with typed error");
        assert!(matches!(err, AsrRuntimeError::UnsupportedAsyncModel { .. }));
    }

    #[test]
    fn maps_http_status_and_network_errors_distinctly() {
        assert!(matches!(
            map_http_status(401),
            AsrRuntimeError::Unauthorized { .. }
        ));
        assert!(matches!(
            map_http_status(403),
            AsrRuntimeError::Unauthorized { .. }
        ));
        assert_eq!(
            map_http_status(429),
            AsrRuntimeError::ProviderUnavailable { status: 429 }
        );
        assert_eq!(
            map_http_status(503),
            AsrRuntimeError::ProviderUnavailable { status: 503 }
        );
        assert!(matches!(
            map_http_status(400),
            AsrRuntimeError::Rejected { .. }
        ));
        assert!(matches!(
            map_network(JsonClientError::RequestTooLarge),
            AsrRuntimeError::RequestTooLarge { .. }
        ));
    }

    #[test]
    fn parses_openrouter_nanogpt_and_gemini_stt_model_lists() {
        let or = serde_json::json!({
            "data": [
                { "id": "openai/whisper-large-v3-turbo", "name": "Whisper Large v3 Turbo" }
            ]
        });
        assert_eq!(parse_openrouter_stt_models(&or).len(), 1);

        let ng = serde_json::json!({
            "data": [
                { "id": "Whisper-Large-V3", "name": "Whisper Large V3", "capabilities": { "speech_to_text": true } },
                { "id": "Kokoro-82m", "name": "Kokoro", "capabilities": { "speech_to_text": false } }
            ]
        });
        let ng_models = parse_nanogpt_stt_models(&ng);
        assert_eq!(ng_models.len(), 1);
        assert_eq!(ng_models[0].id, "Whisper-Large-V3");

        let gm = serde_json::json!({
            "models": [
                { "name": "models/gemini-3.5-transcribe", "displayName": "Gemini 3.5 Transcribe", "supportedGenerationMethods": ["generateContent"] },
                { "name": "models/gemini-2.5-flash", "displayName": "Gemini 2.5 Flash", "supportedGenerationMethods": ["generateContent"] },
                { "name": "models/gemini-3.8-flash-tts", "displayName": "Gemini 3.8 Flash TTS", "supportedGenerationMethods": ["generateContent"] },
                { "name": "models/text-embedding-004", "displayName": "Embedding", "supportedGenerationMethods": ["embedContent"] }
            ]
        });
        let gm_models = parse_gemini_stt_models(&gm);
        assert_eq!(gm_models.len(), 2);
        assert_eq!(gm_models[0].id, "gemini-3.5-transcribe");
        assert_eq!(gm_models[1].id, "gemini-2.5-flash");
    }
}
