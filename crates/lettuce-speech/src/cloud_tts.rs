use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use lettuce_jobs::handle::CancellationToken;
use lettuce_network::{JsonAuth, JsonClient, JsonClientError, JsonStaticHeader, RequestPolicy};
use lettuce_settings::{HeaderName, SecretValue, SecretValueError};
use serde::Serialize;
use tokio::sync::RwLock;

use crate::{
    AudioProviderKind, DiscoveredVoiceDraft, MAX_TTS_RESPONSE_BYTES, RuntimeSynthesis,
    SynthesisRequest, TtsRuntimeError, VoiceDiscoveryError,
    gemini_tts::{GeminiGenerateContentParams, gemini_wav, synthesize_gemini_generate_content},
    tts_catalog_voices,
};

pub const OPENROUTER_DEFAULT_ENDPOINT: &str = "https://openrouter.ai/api/v1";
pub const NANOGPT_DEFAULT_ENDPOINT: &str = "https://api.nano-gpt.com/api/v1";
pub const GEMINI_DEFAULT_ENDPOINT: &str = "https://generativelanguage.googleapis.com";

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudTtsModel {
    pub id: String,
    pub name: String,
    pub voices: Vec<DiscoveredVoiceDraft>,
    pub response_formats: Vec<String>,
    pub has_default_voice: bool,
}

#[derive(Debug, Clone)]
pub struct CloudTtsRuntime {
    network: Arc<JsonClient>,
    catalog_cache: Arc<RwLock<HashMap<(String, AudioProviderKind), Vec<CloudTtsModel>>>>,
}

impl CloudTtsRuntime {
    #[must_use]
    pub fn new(network: Arc<JsonClient>) -> Self {
        Self {
            network,
            catalog_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn verify(
        &self,
        endpoint: Option<&str>,
        kind: AudioProviderKind,
        credential: &SecretValue,
    ) -> Result<bool, VoiceDiscoveryError> {
        let base = resolved_endpoint(endpoint, kind)?;
        let path = catalog_path(base, kind)?;
        let auth = auth_for(kind, credential)?;
        let headers = catalog_headers(kind);
        let response = self
            .network
            .get_json(base, path, headers, auth, Vec::new(), RequestPolicy::PROBE)
            .await
            .map_err(map_discovery_network)?;
        if (200..300).contains(&response.status) {
            return Ok(true);
        }
        if matches!(response.status, 401 | 403) {
            return Ok(false);
        }
        Err(VoiceDiscoveryError::Unavailable {
            reason: "provider returned an unsuccessful verification status",
        })
    }

    pub async fn fetch_catalog(
        &self,
        endpoint: Option<&str>,
        kind: AudioProviderKind,
        credential: &SecretValue,
    ) -> Result<Vec<CloudTtsModel>, VoiceDiscoveryError> {
        let base = resolved_endpoint(endpoint, kind)?;
        let path = catalog_path(base, kind)?;
        let auth = auth_for(kind, credential)?;
        let headers = catalog_headers(kind);
        let response = self
            .network
            .get_json(base, path, headers, auth, Vec::new(), RequestPolicy::BROWSE)
            .await
            .map_err(map_discovery_network)?;
        if !(200..300).contains(&response.status) {
            return Err(VoiceDiscoveryError::Unavailable {
                reason: "provider returned an unsuccessful catalog status",
            });
        }
        let body: serde_json::Value =
            serde_json::from_slice(&response.body).map_err(|_| VoiceDiscoveryError::InvalidData {
                reason: "provider returned malformed catalog json",
            })?;
        let models = match kind {
            AudioProviderKind::OpenRouterTts => parse_openrouter_tts_catalog(&body),
            AudioProviderKind::NanoGptTts => parse_nanogpt_tts_catalog(&body),
            AudioProviderKind::GeminiAudio => parse_gemini_tts_models(&body),
            _ => {
                return Err(VoiceDiscoveryError::InvalidData {
                    reason: "unsupported cloud tts provider kind",
                });
            }
        };
        let mut cache = self.catalog_cache.write().await;
        cache.insert((base.to_owned(), kind), models.clone());
        Ok(models)
    }

    async fn cached_or_fetch_catalog(
        &self,
        endpoint: Option<&str>,
        kind: AudioProviderKind,
        credential: &SecretValue,
    ) -> Option<Vec<CloudTtsModel>> {
        let base = resolved_endpoint(endpoint, kind).ok()?;
        if let Some(models) = self.catalog_cache.read().await.get(&(base.to_owned(), kind)) {
            return Some(models.clone());
        }
        self.fetch_catalog(endpoint, kind, credential).await.ok()
    }

    pub async fn synthesize(
        &self,
        endpoint: Option<&str>,
        kind: AudioProviderKind,
        request: &SynthesisRequest,
        directed_text: Option<&str>,
        credential: &SecretValue,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeSynthesis, TtsRuntimeError> {
        if cancellation.is_cancelled() {
            return Err(TtsRuntimeError::Cancelled);
        }
        if kind == AudioProviderKind::GeminiAudio {
            return self
                .synthesize_gemini(endpoint, request, directed_text, credential, cancellation)
                .await;
        }
        let base = resolved_endpoint(endpoint, kind).map_err(|_| TtsRuntimeError::Rejected {
            reason: "unsupported cloud tts provider kind",
        })?;
        let path = speech_path(base);
        let auth = credential
            .with(|value| SecretValue::new(value.to_owned()))
            .map_err(map_secret_value)
            .map(JsonAuth::Bearer)?;
        let (instructions, speed) = parse_prompt_and_speed(request.prompt.as_deref());
        let requested_voice = {
            let trimmed = request.voice_id.trim();
            (!trimmed.is_empty() && trimmed != "default").then_some(trimmed)
        };
        let catalog = self.cached_or_fetch_catalog(endpoint, kind, credential).await;
        let validated_voice = if let Some(models) = catalog.as_deref() {
            validate_model_voice(models, &request.model_id, requested_voice)?
        } else {
            requested_voice.map(str::to_owned)
        };
        let response_format = if kind == AudioProviderKind::OpenRouterTts {
            let formats = catalog
                .as_deref()
                .and_then(|models| models.iter().find(|model| model.id == request.model_id))
                .map(|model| model.response_formats.as_slice())
                .unwrap_or(&[]);
            select_openrouter_response_format(formats)
        } else {
            "mp3"
        };
        let body = serde_json::to_vec(&CloudTtsRequest {
            model: &request.model_id,
            input: &request.text,
            voice: validated_voice.as_deref(),
            instructions: instructions.as_deref(),
            speed,
            response_format,
        })
        .map_err(|error| {
            tracing::warn!(%error, "cloud tts request body could not be serialized");
            TtsRuntimeError::Rejected {
                reason: "request body could not be serialized",
            }
        })?;
        let headers: &[JsonStaticHeader] = match kind {
            AudioProviderKind::OpenRouterTts => &OPENROUTER_HEADERS,
            _ => &[],
        };
        let response = tokio::select! {
            response = self.network.post_json(
                base,
                path,
                body,
                headers,
                auth,
                Vec::new(),
                RequestPolicy::GENERATION,
            ) => response.map_err(map_runtime_network)?,
            () = cancellation.cancelled() => return Err(TtsRuntimeError::Cancelled),
        };
        if !(200..300).contains(&response.status) {
            return Err(match response.status {
                401 | 403 => TtsRuntimeError::Unauthorized {
                    reason: "provider rejected credentials",
                },
                408 | 429 | 500..=599 => TtsRuntimeError::ProviderUnavailable {
                    status: response.status,
                },
                _ => TtsRuntimeError::Rejected {
                    reason: "provider returned a non-retryable http status",
                },
            });
        }
        if response.body.is_empty() || response.body.len() > MAX_TTS_RESPONSE_BYTES {
            return Err(TtsRuntimeError::Failed {
                reason: "provider returned empty or oversized audio",
            });
        }
        if response_format == "pcm" {
            let wav = gemini_wav(response.body, response.content_type.as_deref())?;
            return Ok(RuntimeSynthesis {
                bytes: wav,
                declared_mime_type: "audio/wav".to_owned(),
            });
        }
        let fallback_mime = if response_format == "wav" {
            "audio/wav"
        } else {
            "audio/mpeg"
        };
        let declared_mime_type = response
            .content_type
            .as_deref()
            .and_then(|value| value.split(';').next())
            .map(str::trim)
            .filter(|value| value.starts_with("audio/"))
            .unwrap_or(fallback_mime)
            .to_owned();
        Ok(RuntimeSynthesis {
            bytes: response.body,
            declared_mime_type,
        })
    }

    async fn synthesize_gemini(
        &self,
        endpoint: Option<&str>,
        request: &SynthesisRequest,
        directed_text: Option<&str>,
        credential: &SecretValue,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeSynthesis, TtsRuntimeError> {
        let base = resolved_endpoint(endpoint, AudioProviderKind::GeminiAudio).map_err(|_| {
            TtsRuntimeError::Rejected {
                reason: "invalid gemini endpoint",
            }
        })?;
        let voice_name = request.voice_id.trim();
        if voice_name.is_empty() || voice_name.eq_ignore_ascii_case("default") {
            return Err(TtsRuntimeError::VoiceRequired);
        }
        let model = request.model_id.trim().trim_start_matches("models/");
        let path = if base.ends_with("/v1beta") || base.ends_with("/v1") {
            format!("/models/{model}:generateContent")
        } else {
            format!("/v1beta/models/{model}:generateContent")
        };
        let auth = gemini_runtime_auth(credential)?;
        let text = directed_text
            .map(str::to_owned)
            .unwrap_or_else(|| request.text.clone());
        synthesize_gemini_generate_content(
            &self.network,
            GeminiGenerateContentParams {
                endpoint: base,
                path: &path,
                auth,
                extra_headers: Vec::new(),
                text,
                voice_name: voice_name.to_owned(),
                include_response_modalities: true,
                language_code: None,
            },
            cancellation,
        )
        .await
    }
}

#[must_use]
pub fn select_openrouter_response_format(supported: &[String]) -> &'static str {
    if supported.is_empty() {
        return "mp3";
    }
    for preferred in ["mp3", "wav", "pcm"] {
        if supported
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(preferred))
        {
            return preferred;
        }
    }
    "mp3"
}

pub fn validate_model_voice(
    models: &[CloudTtsModel],
    model_id: &str,
    voice_id: Option<&str>,
) -> Result<Option<String>, TtsRuntimeError> {
    let Some(model) = models.iter().find(|entry| entry.id == model_id) else {
        return Ok(voice_id.map(str::to_owned));
    };
    match voice_id {
        Some(selected) => {
            if !model.voices.is_empty()
                && !model
                    .voices
                    .iter()
                    .any(|voice| voice.voice_id.eq_ignore_ascii_case(selected))
            {
                return Err(TtsRuntimeError::Rejected {
                    reason: "selected voice is not supported by the chosen model",
                });
            }
            Ok(Some(selected.to_owned()))
        }
        None => {
            if model.has_default_voice || model.voices.is_empty() {
                Ok(None)
            } else {
                Err(TtsRuntimeError::VoiceRequired)
            }
        }
    }
}

#[derive(Serialize)]
struct CloudTtsRequest<'a> {
    model: &'a str,
    input: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    voice: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    speed: Option<f64>,
    response_format: &'static str,
}

fn parse_prompt_and_speed(prompt: Option<&str>) -> (Option<String>, Option<f64>) {
    let Some(raw) = prompt.map(str::trim).filter(|value| !value.is_empty()) else {
        return (None, None);
    };
    if raw.starts_with('{')
        && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(raw)
        && let Some(speed) = parsed.get("speed").and_then(serde_json::Value::as_f64)
        && speed.is_finite()
        && speed > 0.0
    {
        return (None, Some(speed));
    }
    (Some(raw.to_owned()), None)
}

pub fn flatten_catalog_voices(models: &[CloudTtsModel]) -> Vec<DiscoveredVoiceDraft> {
    let mut by_id: BTreeMap<String, (usize, DiscoveredVoiceDraft, Vec<String>)> = BTreeMap::new();
    let mut order = 0usize;
    for model in models {
        for voice in &model.voices {
            if let Some((_, existing, model_ids)) = by_id.get_mut(&voice.voice_id) {
                if !model_ids.contains(&model.id) {
                    model_ids.push(model.id.clone());
                }
                if existing.preview_url.is_none() && voice.preview_url.is_some() {
                    existing.preview_url.clone_from(&voice.preview_url);
                }
            } else {
                let mut draft = voice.clone();
                draft
                    .labels
                    .entry("model_id".into())
                    .or_insert_with(|| model.id.clone());
                draft
                    .labels
                    .entry("model_name".into())
                    .or_insert_with(|| model.name.clone());
                by_id.insert(voice.voice_id.clone(), (order, draft, vec![model.id.clone()]));
                order += 1;
            }
        }
    }
    let mut entries: Vec<_> = by_id.into_values().collect();
    entries.sort_by_key(|(index, _, _)| *index);
    entries
        .into_iter()
        .map(|(_, mut draft, model_ids)| {
            draft.labels.insert("models".into(), model_ids.join(","));
            draft
        })
        .collect()
}

pub fn parse_gemini_tts_models(body: &serde_json::Value) -> Vec<CloudTtsModel> {
    let Some(items) = body.get("models").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    let voices: Vec<DiscoveredVoiceDraft> = tts_catalog_voices(AudioProviderKind::GeminiAudio)
        .into_iter()
        .map(|voice| DiscoveredVoiceDraft {
            voice_id: voice.id.to_owned(),
            name: voice.name.to_owned(),
            preview_url: None,
            labels: voice
                .labels
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect(),
        })
        .collect();
    items
        .iter()
        .filter_map(|item| {
            let raw_name = item.get("name").and_then(serde_json::Value::as_str)?.trim();
            let id = raw_name.strip_prefix("models/").unwrap_or(raw_name).trim();
            if id.is_empty() || !id.to_ascii_lowercase().contains("tts") {
                return None;
            }
            if let Some(methods) = item
                .get("supportedGenerationMethods")
                .and_then(serde_json::Value::as_array)
                && !methods
                    .iter()
                    .any(|method| method.as_str() == Some("generateContent"))
            {
                return None;
            }
            let name = item
                .get("displayName")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(id)
                .to_owned();
            Some(CloudTtsModel {
                id: id.to_owned(),
                name,
                voices: voices.clone(),
                response_formats: vec!["pcm".into(), "wav".into()],
                has_default_voice: false,
            })
        })
        .collect()
}

pub fn parse_openrouter_tts_catalog(body: &serde_json::Value) -> Vec<CloudTtsModel> {
    let items = body
        .get("data")
        .or_else(|| body.get("models"))
        .and_then(serde_json::Value::as_array)
        .or_else(|| body.as_array());
    let Some(items) = items else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let id = item
                .get("id")
                .or_else(|| item.get("model"))
                .and_then(serde_json::Value::as_str)?
                .trim();
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
            let speech_params = item
                .get("output_modalities")
                .and_then(serde_json::Value::as_array)
                .and_then(|modalities| {
                    modalities.iter().find(|entry| {
                        entry.get("type").and_then(serde_json::Value::as_str) == Some("speech")
                    })
                })
                .and_then(|speech| speech.get("supported_parameters"));
            let voice_param = speech_params.and_then(|params| params.get("voice"));
            let speech_modality_voices =
                voice_param.and_then(|voice| voice.get("values").or(Some(voice)));
            let voices_value = speech_modality_voices
                .or_else(|| item.get("voices"))
                .or_else(|| item.get("supported_voices"))
                .or_else(|| {
                    item.get("supported_parameters")
                        .and_then(|params| params.get("voices"))
                });
            let voices = parse_voice_list(voices_value, id, &name);
            let response_formats = speech_params
                .and_then(|params| params.get("response_format"))
                .and_then(|fmt| fmt.get("values").or(Some(fmt)))
                .and_then(serde_json::Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(|entry| entry.as_str().map(str::trim))
                        .filter(|entry| !entry.is_empty())
                        .map(str::to_owned)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let has_default_voice = voice_param.is_some_and(|param| {
                param.get("default").is_some_and(|value| !value.is_null())
                    || param.get("required").and_then(serde_json::Value::as_bool) == Some(false)
            });
            Some(CloudTtsModel {
                id: id.to_owned(),
                name,
                voices,
                response_formats,
                has_default_voice,
            })
        })
        .collect()
}

pub fn parse_nanogpt_tts_catalog(body: &serde_json::Value) -> Vec<CloudTtsModel> {
    let Some(items) = body.get("data").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            if item
                .get("capabilities")
                .and_then(|caps| caps.get("text_to_speech"))
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
            let params = item.get("supported_parameters");
            let voices_value = item
                .get("voices")
                .or_else(|| params.and_then(|p| p.get("voices")))
                .or_else(|| params.and_then(|p| p.get("voice")));
            let voices = parse_voice_list(voices_value, id, &name);
            Some(CloudTtsModel {
                id: id.to_owned(),
                name,
                voices,
                response_formats: Vec::new(),
                has_default_voice: true,
            })
        })
        .collect()
}

fn parse_voice_list(
    value: Option<&serde_json::Value>,
    model_id: &str,
    model_name: &str,
) -> Vec<DiscoveredVoiceDraft> {
    let Some(items) = value.and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for item in items {
        let draft = match item {
            serde_json::Value::String(voice_id) => {
                let voice_id = voice_id.trim();
                if voice_id.is_empty() {
                    continue;
                }
                DiscoveredVoiceDraft {
                    voice_id: voice_id.to_owned(),
                    name: voice_id.to_owned(),
                    preview_url: None,
                    labels: BTreeMap::from([
                        ("model_id".into(), model_id.to_owned()),
                        ("model_name".into(), model_name.to_owned()),
                    ]),
                }
            }
            serde_json::Value::Object(map) => {
                let Some(voice_id) = map
                    .get("id")
                    .or_else(|| map.get("voice_id"))
                    .or_else(|| map.get("value"))
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                else {
                    continue;
                };
                let name = map
                    .get("name")
                    .or_else(|| map.get("label"))
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or(voice_id)
                    .to_owned();
                let preview_url = map
                    .get("preview_url")
                    .or_else(|| map.get("sample_url"))
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned);
                let mut labels = BTreeMap::from([
                    ("model_id".into(), model_id.to_owned()),
                    ("model_name".into(), model_name.to_owned()),
                ]);
                for key in ["gender", "language", "accent", "description", "category"] {
                    if let Some(val) = map
                        .get(key)
                        .and_then(serde_json::Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                    {
                        labels.insert(key.into(), val.to_owned());
                    }
                }
                DiscoveredVoiceDraft {
                    voice_id: voice_id.to_owned(),
                    name,
                    preview_url,
                    labels,
                }
            }
            _ => continue,
        };
        if seen.insert(draft.voice_id.clone()) && draft.validate().is_ok() {
            out.push(draft);
        }
    }
    out
}

fn resolved_endpoint(
    endpoint: Option<&str>,
    kind: AudioProviderKind,
) -> Result<&str, VoiceDiscoveryError> {
    let custom = endpoint.map(str::trim).filter(|value| !value.is_empty());
    let base = match (custom, kind) {
        (Some(url), _) => url,
        (None, AudioProviderKind::OpenRouterTts) => OPENROUTER_DEFAULT_ENDPOINT,
        (None, AudioProviderKind::NanoGptTts) => NANOGPT_DEFAULT_ENDPOINT,
        (None, AudioProviderKind::GeminiAudio) => GEMINI_DEFAULT_ENDPOINT,
        (None, _) => {
            return Err(VoiceDiscoveryError::InvalidData {
                reason: "unsupported cloud tts provider kind",
            });
        }
    };
    Ok(base.trim_end_matches('/'))
}

fn speech_path(base: &str) -> &'static str {
    if base.ends_with("/v1") {
        "/audio/speech"
    } else {
        "/v1/audio/speech"
    }
}

fn catalog_path(base: &str, kind: AudioProviderKind) -> Result<&'static str, VoiceDiscoveryError> {
    match kind {
        AudioProviderKind::OpenRouterTts => Ok(if base.ends_with("/v1") {
            "/audio/speech/models"
        } else {
            "/v1/audio/speech/models"
        }),
        AudioProviderKind::NanoGptTts => Ok(if base.ends_with("/v1") {
            "/audio-models?type=tts&detailed=true"
        } else {
            "/v1/audio-models?type=tts&detailed=true"
        }),
        AudioProviderKind::GeminiAudio => Ok(if base.ends_with("/v1beta") || base.ends_with("/v1") {
            "/models"
        } else {
            "/v1beta/models"
        }),
        _ => Err(VoiceDiscoveryError::InvalidData {
            reason: "unsupported cloud tts provider kind",
        }),
    }
}

fn catalog_headers(kind: AudioProviderKind) -> &'static [JsonStaticHeader] {
    match kind {
        AudioProviderKind::OpenRouterTts => &OPENROUTER_ACCEPT_HEADERS,
        _ => &ACCEPT_JSON,
    }
}

fn auth_for(
    kind: AudioProviderKind,
    credential: &SecretValue,
) -> Result<JsonAuth, VoiceDiscoveryError> {
    let secret = credential
        .with(|value| SecretValue::new(value.to_owned()))
        .map_err(|_| VoiceDiscoveryError::InvalidData {
            reason: "api key is empty",
        })?;
    if kind == AudioProviderKind::GeminiAudio {
        return Ok(JsonAuth::Header {
            name: HeaderName::new("x-goog-api-key").expect("static header name"),
            value: secret,
        });
    }
    Ok(JsonAuth::Bearer(secret))
}

fn gemini_runtime_auth(credential: &SecretValue) -> Result<JsonAuth, TtsRuntimeError> {
    let secret = credential
        .with(|value| SecretValue::new(value.to_owned()))
        .map_err(map_secret_value)?;
    Ok(JsonAuth::Header {
        name: HeaderName::new("x-goog-api-key").expect("static header name"),
        value: secret,
    })
}

fn map_secret_value(_: SecretValueError) -> TtsRuntimeError {
    TtsRuntimeError::Rejected {
        reason: "api key is empty",
    }
}

fn map_discovery_network(error: JsonClientError) -> VoiceDiscoveryError {
    tracing::warn!(%error, "cloud tts catalog request failed");
    VoiceDiscoveryError::Unavailable {
        reason: "could not reach the audio provider",
    }
}

fn map_runtime_network(error: JsonClientError) -> TtsRuntimeError {
    tracing::warn!(%error, "cloud tts synthesis request failed");
    match error {
        JsonClientError::InvalidUrl { .. }
        | JsonClientError::InvalidRequest { .. }
        | JsonClientError::RequestTooLarge
        | JsonClientError::ResponseTooLarge => TtsRuntimeError::Rejected {
            reason: "request url, body or response size was rejected by the http client",
        },
        JsonClientError::Transport { reason } => TtsRuntimeError::NetworkUnavailable { reason },
        JsonClientError::ClientConfiguration { reason } => TtsRuntimeError::Unavailable { reason },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_openrouter_tts_catalog_formats_and_merges_shared_voice_models() {
        let body = serde_json::json!({
            "data": [
                {
                    "id": "google/gemini-2.5-pro-tts",
                    "name": "Google Gemini 2.5 Pro TTS",
                    "output_modalities": [
                        {
                            "type": "speech",
                            "supported_parameters": {
                                "voice": { "type": "enum", "values": ["kore", "puck"] },
                                "response_format": { "type": "enum", "values": ["pcm"] }
                            }
                        }
                    ]
                },
                {
                    "id": "openai/tts-1",
                    "name": "OpenAI TTS-1",
                    "voices": ["alloy", { "id": "nova", "name": "Nova", "gender": "female" }]
                },
                {
                    "id": "openai/gpt-audio",
                    "name": "OpenAI GPT Audio",
                    "voices": ["alloy", "verse"]
                }
            ]
        });
        let models = parse_openrouter_tts_catalog(&body);
        assert_eq!(models.len(), 3);
        assert_eq!(models[0].response_formats, vec!["pcm"]);
        assert_eq!(select_openrouter_response_format(&models[0].response_formats), "pcm");
        assert_eq!(
            select_openrouter_response_format(&["pcm".into(), "wav".into()]),
            "wav"
        );
        assert_eq!(
            select_openrouter_response_format(&["pcm".into(), "mp3".into()]),
            "mp3"
        );
        assert!(validate_model_voice(&models, "google/gemini-2.5-pro-tts", Some("kore")).is_ok());
        assert!(matches!(
            validate_model_voice(&models, "google/gemini-2.5-pro-tts", Some("alloy")),
            Err(TtsRuntimeError::Rejected { .. })
        ));
        assert!(matches!(
            validate_model_voice(&models, "google/gemini-2.5-pro-tts", None),
            Err(TtsRuntimeError::VoiceRequired)
        ));
        let flattened = flatten_catalog_voices(&models);
        assert_eq!(flattened.len(), 5);
        let alloy = flattened
            .iter()
            .find(|voice| voice.voice_id == "alloy")
            .expect("alloy");
        assert_eq!(
            alloy.labels.get("models").map(String::as_str),
            Some("openai/tts-1,openai/gpt-audio")
        );
    }

    #[test]
    fn parses_gemini_tts_models_from_v1beta_models_response() {
        let body = serde_json::json!({
            "models": [
                {
                    "name": "models/gemini-3.8-flash-tts",
                    "displayName": "Gemini 3.8 Flash TTS",
                    "supportedGenerationMethods": ["generateContent"]
                },
                {
                    "name": "models/gemini-2.5-flash",
                    "displayName": "Gemini 2.5 Flash",
                    "supportedGenerationMethods": ["generateContent"]
                },
                {
                    "name": "models/embedding-tts-001",
                    "displayName": "Embedding TTS",
                    "supportedGenerationMethods": ["embedContent"]
                }
            ]
        });
        let models = parse_gemini_tts_models(&body);
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "gemini-3.8-flash-tts");
        assert_eq!(models[0].name, "Gemini 3.8 Flash TTS");
        assert!(!models[0].voices.is_empty());
    }

    #[test]
    fn resolves_paths_with_and_without_v1_suffix_and_uses_documented_nanogpt_endpoint() {
        assert_eq!(NANOGPT_DEFAULT_ENDPOINT, "https://api.nano-gpt.com/api/v1");
        assert_eq!(speech_path("https://openrouter.ai/api/v1"), "/audio/speech");
        assert_eq!(speech_path("https://api.nano-gpt.com/api/v1"), "/audio/speech");
        assert_eq!(speech_path("https://api.nano-gpt.com/api"), "/v1/audio/speech");
        assert_eq!(
            catalog_path("https://openrouter.ai/api/v1", AudioProviderKind::OpenRouterTts)
                .unwrap(),
            "/audio/speech/models"
        );
        assert_eq!(
            catalog_path("https://api.nano-gpt.com/api/v1", AudioProviderKind::NanoGptTts).unwrap(),
            "/audio-models?type=tts&detailed=true"
        );
        assert_eq!(
            catalog_path("https://api.nano-gpt.com/api", AudioProviderKind::NanoGptTts).unwrap(),
            "/v1/audio-models?type=tts&detailed=true"
        );
    }
}
