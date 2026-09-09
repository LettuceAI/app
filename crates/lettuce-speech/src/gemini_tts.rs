use std::sync::Arc;

use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::STANDARD};
use lettuce_jobs::handle::CancellationToken;
use lettuce_network::{JsonAuth, JsonClient, JsonClientError, JsonSecretHeader, RequestPolicy};
use lettuce_settings::{HeaderName, SecretValue};
use serde::{Deserialize, Serialize};

use crate::{AudioProviderConfig, RuntimeSynthesis, SynthesisRequest, TtsRuntime, TtsRuntimeError};

#[derive(Debug, Clone)]
pub struct GeminiTtsRuntime {
    network: Arc<JsonClient>,
    endpoint_override: Option<String>,
}

impl GeminiTtsRuntime {
    #[must_use]
    pub fn new(network: Arc<JsonClient>) -> Self {
        Self {
            network,
            endpoint_override: None,
        }
    }

    #[cfg(test)]
    fn with_endpoint(network: Arc<JsonClient>, endpoint: String) -> Self {
        Self {
            network,
            endpoint_override: Some(endpoint),
        }
    }
}

#[derive(Serialize)]
struct GeminiRequest {
    contents: GeminiContents,
    generation_config: GeminiGenerationConfig,
}

#[derive(Serialize)]
struct GeminiContents {
    role: &'static str,
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
    language_code: &'static str,
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

#[async_trait]
impl TtsRuntime for GeminiTtsRuntime {
    async fn synthesize(
        &self,
        request: &SynthesisRequest,
        credential: Option<&SecretValue>,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeSynthesis, TtsRuntimeError> {
        request.validate().map_err(|_| TtsRuntimeError::Rejected)?;
        let AudioProviderConfig::Gemini {
            project_id,
            location,
        } = &request.provider.config
        else {
            return Err(TtsRuntimeError::Rejected);
        };
        let project_id = project_id.as_deref().ok_or(TtsRuntimeError::Rejected)?;
        if !valid_host_label(location)
            || !valid_path_segment(project_id)
            || !valid_path_segment(&request.model_id)
        {
            return Err(TtsRuntimeError::Rejected);
        }
        let credential = credential.ok_or(TtsRuntimeError::Rejected)?;
        let auth = credential
            .with(|value| SecretValue::new(value.to_owned()))
            .map_err(|_| TtsRuntimeError::Rejected)?;
        let project_header =
            SecretValue::new(project_id.to_owned()).map_err(|_| TtsRuntimeError::Rejected)?;
        let full_text = match request.prompt.as_deref() {
            Some(prompt) if !prompt.is_empty() => format!("{prompt}: {}", request.text),
            _ => request.text.clone(),
        };
        let body = serde_json::to_vec(&GeminiRequest {
            contents: GeminiContents {
                role: "user",
                parts: GeminiParts { text: full_text },
            },
            generation_config: GeminiGenerationConfig {
                speech_config: GeminiSpeechConfig {
                    language_code: "en-us",
                    voice_config: GeminiVoiceConfig {
                        prebuilt_voice_config: GeminiPrebuiltVoiceConfig {
                            voice_name: resolve_voice_name(&request.voice_id),
                        },
                    },
                },
            },
        })
        .map_err(|_| TtsRuntimeError::Rejected)?;
        if cancellation.is_cancelled() {
            return Err(TtsRuntimeError::Cancelled);
        }
        let endpoint = self
            .endpoint_override
            .clone()
            .unwrap_or_else(|| format!("https://{location}-aiplatform.googleapis.com"));
        let path = format!(
            "/v1beta1/projects/{project_id}/locations/{location}/publishers/google/models/{}:generateContent",
            request.model_id
        );
        let response = tokio::select! {
            response = self.network.post_json(
                &endpoint,
                &path,
                body,
                &[],
                JsonAuth::Bearer(auth),
                vec![JsonSecretHeader {
                    name: HeaderName::new("x-goog-user-project")
                        .map_err(|_| TtsRuntimeError::Rejected)?,
                    value: project_header,
                }],
                RequestPolicy::GENERATION,
            ) => response.map_err(map_network)?,
            () = cancellation.cancelled() => return Err(TtsRuntimeError::Cancelled),
        };
        if !(200..300).contains(&response.status) {
            return Err(match response.status {
                408 | 429 | 500..=599 => TtsRuntimeError::Unavailable,
                _ => TtsRuntimeError::Rejected,
            });
        }
        let response: GeminiResponse =
            serde_json::from_slice(&response.body).map_err(|_| TtsRuntimeError::Rejected)?;
        let encoded = response
            .candidates
            .iter()
            .flat_map(|candidate| &candidate.content.parts)
            .find_map(|part| part.inline_data.as_ref())
            .map(|data| data.data.as_str())
            .ok_or(TtsRuntimeError::Rejected)?;
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|_| TtsRuntimeError::Rejected)?;
        Ok(RuntimeSynthesis {
            bytes,
            declared_mime_type: "audio/wav".into(),
        })
    }
}

fn resolve_voice_name(voice_id: &str) -> String {
    if voice_id.eq_ignore_ascii_case("preview") {
        "kore".into()
    } else {
        voice_id.to_lowercase()
    }
}

fn valid_host_label(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn valid_path_segment(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn map_network(error: JsonClientError) -> TtsRuntimeError {
    match error {
        JsonClientError::InvalidUrl
        | JsonClientError::InvalidRequest
        | JsonClientError::RequestTooLarge
        | JsonClientError::ResponseTooLarge => TtsRuntimeError::Rejected,
        JsonClientError::Transport | JsonClientError::ClientConfiguration => {
            TtsRuntimeError::Unavailable
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use lettuce_settings::{SecretOwnerId, SecretRef};
    use lettuce_types::{AssetId, AudioProviderId, RequestId, Revision, TimestampMillis};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    use crate::{AudioProvider, TtsOutputPolicy};

    use super::*;

    async fn server(status: &'static str, body: Vec<u8>) -> (String, Arc<Mutex<Vec<u8>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let address = listener.local_addr().expect("address");
        let captured = Arc::new(Mutex::new(Vec::new()));
        let request = captured.clone();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut bytes = Vec::new();
            let mut buffer = [0_u8; 4096];
            loop {
                let read = stream.read(&mut buffer).await.expect("read");
                if read == 0 {
                    break;
                }
                bytes.extend_from_slice(&buffer[..read]);
                let Some(header_end) = bytes.windows(4).position(|value| value == b"\r\n\r\n")
                else {
                    continue;
                };
                let headers = String::from_utf8_lossy(&bytes[..header_end]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        line.split_once(':').and_then(|(name, value)| {
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        })
                    })
                    .unwrap_or(0);
                if bytes.len() >= header_end + 4 + content_length {
                    break;
                }
            }
            *request.lock().expect("captured request") = bytes;
            let headers = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\n\r\n",
                body.len()
            );
            stream
                .write_all(headers.as_bytes())
                .await
                .expect("write headers");
            stream.write_all(&body).await.expect("write body");
        });
        (format!("http://{address}"), captured)
    }

    fn request(project_id: &str) -> SynthesisRequest {
        SynthesisRequest {
            id: RequestId::new(),
            provider: AudioProvider {
                id: AudioProviderId::new(),
                secret_owner_id: SecretOwnerId::new(),
                label: "Gemini speech".into(),
                api_key_ref: Some(SecretRef::new()),
                config: AudioProviderConfig::Gemini {
                    project_id: Some(project_id.into()),
                    location: "us-central1".into(),
                },
                revision: Revision::INITIAL,
                created_at: TimestampMillis::new(1),
                updated_at: TimestampMillis::new(1),
            },
            model_id: "gemini-2.5-flash-tts".into(),
            voice_id: "Preview".into(),
            prompt: Some("Speak warmly".into()),
            text: "Keep authored spacing.".into(),
            output_asset_id: AssetId::new(),
            output_policy: TtsOutputPolicy::Retained,
            created_at: TimestampMillis::new(1),
        }
    }

    #[tokio::test]
    async fn sends_legacy_vertex_request_and_decodes_audio() {
        let body = serde_json::to_vec(&serde_json::json!({
            "candidates": [{"content": {"parts": [
                {"text": "metadata"},
                {"inlineData": {"data": "UklGRmF1ZGlv"}}
            ]}}]
        }))
        .expect("response JSON");
        let (endpoint, captured) = server("200 OK", body).await;
        let runtime = GeminiTtsRuntime::with_endpoint(
            Arc::new(JsonClient::new().expect("network client")),
            endpoint,
        );
        let result = runtime
            .synthesize(
                &request("project-canary"),
                Some(&SecretValue::new("access-token-canary").expect("secret")),
                &CancellationToken::new(),
            )
            .await
            .expect("synthesis");
        assert_eq!(result.bytes, b"RIFFaudio");
        assert_eq!(result.declared_mime_type, "audio/wav");
        let captured = captured.lock().expect("captured request");
        let split = captured
            .windows(4)
            .position(|value| value == b"\r\n\r\n")
            .expect("request headers");
        let headers = String::from_utf8_lossy(&captured[..split]);
        assert!(headers.starts_with(
            "POST /v1beta1/projects/project-canary/locations/us-central1/publishers/google/models/gemini-2.5-flash-tts:generateContent HTTP/1.1"
        ));
        let headers = headers.to_ascii_lowercase();
        assert!(headers.contains("authorization: bearer access-token-canary"));
        assert!(headers.contains("x-goog-user-project: project-canary"));
        let value: serde_json::Value =
            serde_json::from_slice(&captured[split + 4..]).expect("request JSON");
        assert_eq!(value["contents"]["role"], "user");
        assert_eq!(
            value["contents"]["parts"]["text"],
            "Speak warmly: Keep authored spacing."
        );
        assert_eq!(
            value["generation_config"]["speech_config"]["language_code"],
            "en-us"
        );
        assert_eq!(
            value["generation_config"]["speech_config"]["voice_config"]["prebuilt_voice_config"]["voice_name"],
            "kore"
        );
    }

    #[tokio::test]
    async fn rejects_unsafe_routing_and_preflight_cancellation() {
        let runtime = GeminiTtsRuntime::with_endpoint(
            Arc::new(JsonClient::new().expect("network client")),
            "http://127.0.0.1:1".into(),
        );
        let credential = SecretValue::new("secret").expect("secret");
        assert!(matches!(
            runtime
                .synthesize(
                    &request("../project"),
                    Some(&credential),
                    &CancellationToken::new(),
                )
                .await,
            Err(TtsRuntimeError::Rejected)
        ));
        let (endpoint, _) = server("400 Bad Request", b"{}".to_vec()).await;
        let runtime = GeminiTtsRuntime::with_endpoint(
            Arc::new(JsonClient::new().expect("network client")),
            endpoint,
        );
        assert!(matches!(
            runtime
                .synthesize(
                    &request("project"),
                    Some(&credential),
                    &CancellationToken::new(),
                )
                .await,
            Err(TtsRuntimeError::Rejected)
        ));
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        assert!(matches!(
            runtime
                .synthesize(&request("project"), Some(&credential), &cancellation)
                .await,
            Err(TtsRuntimeError::Cancelled)
        ));
    }
}
