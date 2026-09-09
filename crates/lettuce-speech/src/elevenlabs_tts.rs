use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use lettuce_jobs::handle::CancellationToken;
use lettuce_network::{JsonAuth, JsonClient, JsonClientError, JsonQueryParameter, RequestPolicy};
use lettuce_settings::{HeaderName, SecretValue};
use serde::{Deserialize, Serialize};

use crate::{
    AudioProvider, AudioProviderConfig, AudioProviderVerificationError, AudioProviderVerifier,
    DiscoveredVoiceDraft, RuntimeSynthesis, SynthesisRequest, TtsRuntime, TtsRuntimeError,
    VoiceDiscovery, VoiceDiscoveryError,
};

const ENDPOINT: &str = "https://api.elevenlabs.io";
const OUTPUT_FORMAT: &str = "mp3_44100_128";

#[derive(Debug, Clone)]
pub struct ElevenLabsTtsRuntime {
    network: Arc<JsonClient>,
    endpoint: String,
}

impl ElevenLabsTtsRuntime {
    #[must_use]
    pub fn new(network: Arc<JsonClient>) -> Self {
        Self {
            network,
            endpoint: ENDPOINT.to_owned(),
        }
    }

    #[cfg(test)]
    fn with_endpoint(network: Arc<JsonClient>, endpoint: String) -> Self {
        Self { network, endpoint }
    }
}

#[derive(Serialize)]
struct ElevenLabsRequest<'a> {
    text: &'a str,
    model_id: &'a str,
}

#[derive(Deserialize)]
struct ElevenLabsVoicesResponse {
    voices: Vec<ElevenLabsVoice>,
    #[serde(default)]
    has_more: bool,
}

#[derive(Deserialize)]
struct ElevenLabsVoice {
    voice_id: String,
    name: String,
    preview_url: Option<String>,
    #[serde(default)]
    labels: BTreeMap<String, String>,
    category: Option<String>,
    description: Option<String>,
}

#[async_trait]
impl AudioProviderVerifier for ElevenLabsTtsRuntime {
    async fn verify_audio_provider(
        &self,
        provider: &AudioProvider,
        credential: Option<&SecretValue>,
    ) -> Result<bool, AudioProviderVerificationError> {
        provider
            .validate()
            .map_err(|_| AudioProviderVerificationError::InvalidInput)?;
        if !matches!(&provider.config, AudioProviderConfig::Elevenlabs) {
            return Err(AudioProviderVerificationError::InvalidInput);
        }
        let credential = credential.ok_or(AudioProviderVerificationError::InvalidInput)?;
        let auth = JsonAuth::Header {
            name: HeaderName::new("xi-api-key")
                .map_err(|_| AudioProviderVerificationError::InvalidInput)?,
            value: credential
                .with(|value| SecretValue::new(value.to_owned()))
                .map_err(|_| AudioProviderVerificationError::InvalidInput)?,
        };
        let response = self
            .network
            .get_json(
                &self.endpoint,
                "/v1/voices",
                &[],
                auth,
                Vec::new(),
                RequestPolicy::PROBE,
            )
            .await
            .map_err(map_verification_network)?;
        Ok((200..300).contains(&response.status))
    }
}

#[async_trait]
impl VoiceDiscovery for ElevenLabsTtsRuntime {
    async fn fetch_configured_voices(
        &self,
        provider: &AudioProvider,
        credential: &SecretValue,
    ) -> Result<Vec<DiscoveredVoiceDraft>, VoiceDiscoveryError> {
        provider
            .validate()
            .map_err(|_| VoiceDiscoveryError::InvalidData)?;
        if !matches!(&provider.config, AudioProviderConfig::Elevenlabs) {
            return Err(VoiceDiscoveryError::InvalidData);
        }
        let auth = JsonAuth::Header {
            name: HeaderName::new("xi-api-key").map_err(|_| VoiceDiscoveryError::InvalidData)?,
            value: credential
                .with(|value| SecretValue::new(value.to_owned()))
                .map_err(|_| VoiceDiscoveryError::InvalidData)?,
        };
        let response = self
            .network
            .get_json(
                &self.endpoint,
                "/v1/voices",
                &[],
                auth,
                Vec::new(),
                RequestPolicy::PROBE,
            )
            .await
            .map_err(map_discovery_network)?;
        if !(200..300).contains(&response.status) {
            return Err(match response.status {
                408 | 429 | 500..=599 => VoiceDiscoveryError::Unavailable,
                _ => VoiceDiscoveryError::InvalidData,
            });
        }
        let response: ElevenLabsVoicesResponse =
            serde_json::from_slice(&response.body).map_err(|_| VoiceDiscoveryError::InvalidData)?;
        if response.has_more || response.voices.len() > crate::MAX_DISCOVERED_VOICES {
            return Err(VoiceDiscoveryError::InvalidData);
        }
        response
            .voices
            .into_iter()
            .map(|voice| {
                let mut labels = voice.labels;
                if let Some(category) = voice.category {
                    labels.insert("category".into(), category);
                }
                if let Some(description) = voice.description {
                    labels.insert("description".into(), description);
                }
                let voice = DiscoveredVoiceDraft {
                    voice_id: voice.voice_id,
                    name: voice.name,
                    preview_url: voice.preview_url,
                    labels,
                };
                voice.validate()?;
                Ok(voice)
            })
            .collect()
    }
}

#[async_trait]
impl TtsRuntime for ElevenLabsTtsRuntime {
    async fn synthesize(
        &self,
        request: &SynthesisRequest,
        credential: Option<&SecretValue>,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeSynthesis, TtsRuntimeError> {
        request.validate().map_err(|_| TtsRuntimeError::Rejected)?;
        if !matches!(&request.provider.config, AudioProviderConfig::Elevenlabs)
            || !valid_path_segment(&request.voice_id)
        {
            return Err(TtsRuntimeError::Rejected);
        }
        let credential = credential.ok_or(TtsRuntimeError::Rejected)?;
        let auth = JsonAuth::Header {
            name: HeaderName::new("xi-api-key").map_err(|_| TtsRuntimeError::Rejected)?,
            value: credential
                .with(|value| SecretValue::new(value.to_owned()))
                .map_err(|_| TtsRuntimeError::Rejected)?,
        };
        let body = serde_json::to_vec(&ElevenLabsRequest {
            text: &request.text,
            model_id: &request.model_id,
        })
        .map_err(|_| TtsRuntimeError::Rejected)?;
        if cancellation.is_cancelled() {
            return Err(TtsRuntimeError::Cancelled);
        }
        let path = format!("/v1/text-to-speech/{}", request.voice_id);
        let response = tokio::select! {
            response = self.network.post_json_with_query(
                &self.endpoint,
                &path,
                &[JsonQueryParameter { name: "output_format", value: OUTPUT_FORMAT }],
                body,
                &[],
                auth,
                Vec::new(),
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
        Ok(RuntimeSynthesis {
            bytes: response.body,
            declared_mime_type: "audio/mpeg".into(),
        })
    }
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

fn map_discovery_network(error: JsonClientError) -> VoiceDiscoveryError {
    match error {
        JsonClientError::InvalidUrl
        | JsonClientError::InvalidRequest
        | JsonClientError::RequestTooLarge
        | JsonClientError::ResponseTooLarge => VoiceDiscoveryError::InvalidData,
        JsonClientError::Transport | JsonClientError::ClientConfiguration => {
            VoiceDiscoveryError::Unavailable
        }
    }
}

fn map_verification_network(error: JsonClientError) -> AudioProviderVerificationError {
    match error {
        JsonClientError::InvalidUrl
        | JsonClientError::InvalidRequest
        | JsonClientError::RequestTooLarge
        | JsonClientError::ResponseTooLarge => AudioProviderVerificationError::InvalidInput,
        JsonClientError::Transport | JsonClientError::ClientConfiguration => {
            AudioProviderVerificationError::Unavailable
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

    async fn server(response: Vec<u8>) -> (String, Arc<Mutex<Vec<u8>>>) {
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
            stream.write_all(&response).await.expect("write response");
        });
        (format!("http://{address}"), captured)
    }

    fn request(voice_id: &str) -> SynthesisRequest {
        SynthesisRequest {
            id: RequestId::new(),
            provider: AudioProvider {
                id: AudioProviderId::new(),
                secret_owner_id: SecretOwnerId::new(),
                label: "ElevenLabs speech".into(),
                api_key_ref: Some(SecretRef::new()),
                config: AudioProviderConfig::Elevenlabs,
                revision: Revision::INITIAL,
                created_at: TimestampMillis::new(1),
                updated_at: TimestampMillis::new(1),
            },
            model_id: "eleven_multilingual_v2".into(),
            voice_id: voice_id.into(),
            prompt: Some("Ignored by this provider".into()),
            text: "Keep authored spacing.".into(),
            output_asset_id: AssetId::new(),
            output_policy: TtsOutputPolicy::Retained,
            created_at: TimestampMillis::new(1),
        }
    }

    #[tokio::test]
    async fn sends_legacy_endpoint_auth_query_and_payload() {
        let (endpoint, captured) =
            server(b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\n\r\nID3audio".to_vec()).await;
        let runtime = ElevenLabsTtsRuntime::with_endpoint(
            Arc::new(JsonClient::new().expect("network client")),
            endpoint,
        );
        let result = runtime
            .synthesize(
                &request("voice.canary-1"),
                Some(&SecretValue::new("api-key-canary").expect("secret")),
                &CancellationToken::new(),
            )
            .await
            .expect("synthesis");
        assert_eq!(result.bytes, b"ID3audio");
        assert_eq!(result.declared_mime_type, "audio/mpeg");
        let captured = captured.lock().expect("captured request");
        let split = captured
            .windows(4)
            .position(|value| value == b"\r\n\r\n")
            .expect("request headers");
        let headers = String::from_utf8_lossy(&captured[..split]);
        assert!(headers.starts_with(
            "POST /v1/text-to-speech/voice.canary-1?output_format=mp3_44100_128 HTTP/1.1"
        ));
        assert!(
            headers
                .to_ascii_lowercase()
                .contains("xi-api-key: api-key-canary")
        );
        let value: serde_json::Value =
            serde_json::from_slice(&captured[split + 4..]).expect("request JSON");
        assert_eq!(value["text"], "Keep authored spacing.");
        assert_eq!(value["model_id"], "eleven_multilingual_v2");
        assert_eq!(value.as_object().expect("object").len(), 2);
    }

    #[tokio::test]
    async fn rejects_unsafe_voice_path_before_transport() {
        let runtime = ElevenLabsTtsRuntime::with_endpoint(
            Arc::new(JsonClient::new().expect("network client")),
            "http://127.0.0.1:1".into(),
        );
        assert!(matches!(
            runtime
                .synthesize(
                    &request("../voice"),
                    Some(&SecretValue::new("secret").expect("secret")),
                    &CancellationToken::new(),
                )
                .await,
            Err(TtsRuntimeError::Rejected)
        ));
    }

    #[tokio::test]
    async fn discovers_configured_voices_and_merges_provider_metadata() {
        let body = br#"{"voices":[{"voice_id":"voice-1","name":"Narrator","preview_url":"https://audio.example/preview.mp3","labels":{"accent":"calm","category":"old"},"category":"professional","description":"Warm voice"}],"has_more":false}"#;
        let headers = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len());
        let response = headers
            .into_bytes()
            .into_iter()
            .chain(body.iter().copied())
            .collect();
        let (endpoint, captured) = server(response).await;
        let runtime = ElevenLabsTtsRuntime::with_endpoint(
            Arc::new(JsonClient::new().expect("network client")),
            endpoint,
        );
        let voices = runtime
            .fetch_configured_voices(
                &request("voice").provider,
                &SecretValue::new("api-key-canary").expect("secret"),
            )
            .await
            .expect("voice discovery");
        assert_eq!(voices.len(), 1);
        assert_eq!(voices[0].voice_id, "voice-1");
        assert_eq!(voices[0].name, "Narrator");
        assert_eq!(voices[0].labels["accent"], "calm");
        assert_eq!(voices[0].labels["category"], "professional");
        assert_eq!(voices[0].labels["description"], "Warm voice");
        let captured = captured.lock().expect("captured request");
        let headers = String::from_utf8_lossy(&captured);
        assert!(headers.starts_with("GET /v1/voices HTTP/1.1"));
        assert!(
            headers
                .to_ascii_lowercase()
                .contains("xi-api-key: api-key-canary")
        );
    }

    #[tokio::test]
    async fn verifies_credentials_from_http_status() {
        let (endpoint, captured) =
            server(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n".to_vec()).await;
        let runtime = ElevenLabsTtsRuntime::with_endpoint(
            Arc::new(JsonClient::new().expect("network client")),
            endpoint,
        );
        let provider = request("voice").provider;
        assert!(
            runtime
                .verify_audio_provider(
                    &provider,
                    Some(&SecretValue::new("verification-canary").expect("secret")),
                )
                .await
                .expect("verification")
        );
        {
            let captured = captured.lock().expect("captured request");
            let headers = String::from_utf8_lossy(&captured);
            assert!(headers.starts_with("GET /v1/voices HTTP/1.1"));
            assert!(
                headers
                    .to_ascii_lowercase()
                    .contains("xi-api-key: verification-canary")
            );
        }

        let (endpoint, _) =
            server(b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 2\r\n\r\n{}".to_vec()).await;
        let runtime = ElevenLabsTtsRuntime::with_endpoint(
            Arc::new(JsonClient::new().expect("network client")),
            endpoint,
        );
        assert!(
            !runtime
                .verify_audio_provider(
                    &provider,
                    Some(&SecretValue::new("rejected").expect("secret")),
                )
                .await
                .expect("rejected verification")
        );
    }
}
