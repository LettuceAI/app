use std::sync::Arc;

use async_trait::async_trait;
use lettuce_jobs::handle::CancellationToken;
use lettuce_network::{JsonAuth, JsonClient, JsonClientError, RequestPolicy};
use lettuce_settings::SecretValue;
use serde::Serialize;

use crate::{
    AudioProvider, AudioProviderConfig, AudioProviderVerificationError, AudioProviderVerifier,
    RuntimeSynthesis, SynthesisRequest, TtsRuntime, TtsRuntimeError,
};

const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:8080";
const DEFAULT_REQUEST_PATH: &str = "/v1/tts";
const HEALTH_PATH: &str = "/v1/health";

#[derive(Debug, Clone)]
pub struct FishSpeechTtsRuntime {
    network: Arc<JsonClient>,
}

impl FishSpeechTtsRuntime {
    #[must_use]
    pub fn new(network: Arc<JsonClient>) -> Self {
        Self { network }
    }
}

#[derive(Serialize)]
struct FishSpeechRequest<'a> {
    text: &'a str,
    reference_id: &'a str,
    format: &'static str,
}

#[async_trait]
impl AudioProviderVerifier for FishSpeechTtsRuntime {
    async fn verify_audio_provider(
        &self,
        provider: &AudioProvider,
        credential: Option<&SecretValue>,
    ) -> Result<bool, AudioProviderVerificationError> {
        provider
            .validate()
            .map_err(|_| AudioProviderVerificationError::InvalidInput)?;
        let AudioProviderConfig::FishSpeech { base_url, .. } = &provider.config else {
            return Err(AudioProviderVerificationError::InvalidInput);
        };
        let auth = match credential {
            Some(credential) => JsonAuth::Bearer(
                credential
                    .with(|value| SecretValue::new(value.to_owned()))
                    .map_err(|_| AudioProviderVerificationError::InvalidInput)?,
            ),
            None => JsonAuth::None,
        };
        let response = self
            .network
            .get_json(
                base_url.as_deref().unwrap_or(DEFAULT_ENDPOINT),
                HEALTH_PATH,
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
impl TtsRuntime for FishSpeechTtsRuntime {
    async fn synthesize(
        &self,
        request: &SynthesisRequest,
        credential: Option<&SecretValue>,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeSynthesis, TtsRuntimeError> {
        request.validate().map_err(|_| TtsRuntimeError::Rejected)?;
        let AudioProviderConfig::FishSpeech {
            base_url,
            request_path,
        } = &request.provider.config
        else {
            return Err(TtsRuntimeError::Rejected);
        };
        let endpoint = base_url.as_deref().unwrap_or(DEFAULT_ENDPOINT);
        let path = normalized_path(request_path.as_deref());
        let auth = match credential {
            Some(credential) => JsonAuth::Bearer(
                credential
                    .with(|value| SecretValue::new(value.to_owned()))
                    .map_err(|_| TtsRuntimeError::Rejected)?,
            ),
            None => JsonAuth::None,
        };
        let body = serde_json::to_vec(&FishSpeechRequest {
            text: &request.text,
            reference_id: &request.voice_id,
            format: "mp3",
        })
        .map_err(|_| TtsRuntimeError::Rejected)?;
        if cancellation.is_cancelled() {
            return Err(TtsRuntimeError::Cancelled);
        }
        let response = tokio::select! {
            response = self.network.post_json(
                endpoint,
                &path,
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

fn normalized_path(path: Option<&str>) -> String {
    let path = path.unwrap_or(DEFAULT_REQUEST_PATH);
    if path.starts_with('/') {
        path.to_owned()
    } else {
        format!("/{path}")
    }
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
        (format!("http://{address}/root"), captured)
    }

    fn request(endpoint: String, credential: bool) -> SynthesisRequest {
        SynthesisRequest {
            id: RequestId::new(),
            provider: AudioProvider {
                id: AudioProviderId::new(),
                secret_owner_id: SecretOwnerId::new(),
                label: "Fish Speech local".into(),
                api_key_ref: credential.then(SecretRef::new),
                config: AudioProviderConfig::FishSpeech {
                    base_url: Some(endpoint),
                    request_path: Some("speech".into()),
                },
                revision: Revision::INITIAL,
                created_at: TimestampMillis::new(1),
                updated_at: TimestampMillis::new(1),
            },
            model_id: "server-default".into(),
            voice_id: "reference-canary".into(),
            prompt: Some("Server does not consume this prompt".into()),
            text: "Keep authored spacing.".into(),
            output_asset_id: AssetId::new(),
            output_policy: TtsOutputPolicy::Retained,
            created_at: TimestampMillis::new(1),
        }
    }

    #[tokio::test]
    async fn sends_configured_path_optional_auth_and_legacy_payload() {
        let response = b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\n\r\nID3audio".to_vec();
        let (endpoint, captured) = server(response).await;
        let runtime =
            FishSpeechTtsRuntime::new(Arc::new(JsonClient::new().expect("network client")));
        let result = runtime
            .synthesize(
                &request(endpoint, true),
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
        assert!(headers.starts_with("POST /root/speech HTTP/1.1"));
        assert!(
            headers
                .to_ascii_lowercase()
                .contains("authorization: bearer api-key-canary")
        );
        let value: serde_json::Value =
            serde_json::from_slice(&captured[split + 4..]).expect("request JSON");
        assert_eq!(value["text"], "Keep authored spacing.");
        assert_eq!(value["reference_id"], "reference-canary");
        assert_eq!(value["format"], "mp3");
        assert_eq!(value.as_object().expect("object").len(), 3);
    }

    #[tokio::test]
    async fn supports_no_auth_and_rejects_provider_errors() {
        let response = b"HTTP/1.1 400 Bad Request\r\nContent-Length: 2\r\n\r\n{}".to_vec();
        let (endpoint, captured) = server(response).await;
        let runtime =
            FishSpeechTtsRuntime::new(Arc::new(JsonClient::new().expect("network client")));
        assert!(matches!(
            runtime
                .synthesize(&request(endpoint, false), None, &CancellationToken::new(),)
                .await,
            Err(TtsRuntimeError::Rejected)
        ));
        {
            let captured = captured.lock().expect("captured request");
            let headers = String::from_utf8_lossy(&captured);
            assert!(!headers.to_ascii_lowercase().contains("authorization:"));
        }
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        assert!(matches!(
            runtime
                .synthesize(
                    &request("http://127.0.0.1:1".into(), false),
                    None,
                    &cancellation,
                )
                .await,
            Err(TtsRuntimeError::Cancelled)
        ));
    }

    #[tokio::test]
    async fn verifies_health_with_optional_authentication() {
        let response = b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n".to_vec();
        let (endpoint, captured) = server(response).await;
        let runtime =
            FishSpeechTtsRuntime::new(Arc::new(JsonClient::new().expect("network client")));
        let provider = request(endpoint, true).provider;
        assert!(
            runtime
                .verify_audio_provider(
                    &provider,
                    Some(&SecretValue::new("health-canary").expect("secret")),
                )
                .await
                .expect("authenticated health")
        );
        {
            let captured = captured.lock().expect("captured request");
            let headers = String::from_utf8_lossy(&captured);
            assert!(headers.starts_with("GET /root/v1/health HTTP/1.1"));
            assert!(
                headers
                    .to_ascii_lowercase()
                    .contains("authorization: bearer health-canary")
            );
        }

        let response = b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 2\r\n\r\n{}".to_vec();
        let (endpoint, captured) = server(response).await;
        let provider = request(endpoint, false).provider;
        assert!(
            !runtime
                .verify_audio_provider(&provider, None)
                .await
                .expect("unauthenticated health rejection")
        );
        let captured = captured.lock().expect("captured request");
        let headers = String::from_utf8_lossy(&captured);
        assert!(!headers.to_ascii_lowercase().contains("authorization:"));
    }
}
