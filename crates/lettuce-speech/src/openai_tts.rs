use std::sync::Arc;

use async_trait::async_trait;
use lettuce_jobs::handle::CancellationToken;
use lettuce_network::{JsonAuth, JsonClient, JsonClientError, RequestPolicy};
use lettuce_settings::{SecretValue, SecretValueError};
use serde::Serialize;

use crate::{AudioProviderConfig, RuntimeSynthesis, SynthesisRequest, TtsRuntime, TtsRuntimeError};

const DEFAULT_REQUEST_PATH: &str = "/v1/audio/speech";

#[derive(Debug, Clone)]
pub struct OpenAiCompatibleTtsRuntime {
    network: Arc<JsonClient>,
}

impl OpenAiCompatibleTtsRuntime {
    #[must_use]
    pub fn new(network: Arc<JsonClient>) -> Self {
        Self { network }
    }
}

#[derive(Serialize)]
struct OpenAiTtsRequest<'a> {
    model: &'a str,
    input: &'a str,
    voice: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<&'a str>,
    response_format: &'static str,
}

#[async_trait]
impl TtsRuntime for OpenAiCompatibleTtsRuntime {
    async fn synthesize(
        &self,
        request: &SynthesisRequest,
        credential: Option<&SecretValue>,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeSynthesis, TtsRuntimeError> {
        request.validate().map_err(|_| TtsRuntimeError::Rejected)?;
        let AudioProviderConfig::OpenAiCompatible {
            base_url,
            request_path,
        } = &request.provider.config
        else {
            return Err(TtsRuntimeError::Rejected);
        };
        let endpoint = base_url.as_deref().ok_or(TtsRuntimeError::Rejected)?;
        let path = normalized_path(request_path.as_deref());
        let credential = credential.ok_or(TtsRuntimeError::Rejected)?;
        let auth = credential
            .with(|value| SecretValue::new(value.to_owned()))
            .map_err(map_secret_value)
            .map(JsonAuth::Bearer)?;
        let instructions = request.prompt.as_deref().and_then(|value| {
            let value = value.trim();
            (!value.is_empty()).then_some(value)
        });
        let body = serde_json::to_vec(&OpenAiTtsRequest {
            model: &request.model_id,
            input: &request.text,
            voice: &request.voice_id,
            instructions,
            response_format: "mp3",
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
        let declared_mime_type = response
            .content_type
            .as_deref()
            .and_then(|value| value.split(';').next())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("audio/mpeg")
            .to_owned();
        Ok(RuntimeSynthesis {
            bytes: response.body,
            declared_mime_type,
        })
    }
}

fn normalized_path(path: Option<&str>) -> String {
    let path = path.unwrap_or(DEFAULT_REQUEST_PATH);
    if path.is_empty() {
        return DEFAULT_REQUEST_PATH.to_owned();
    }
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

fn map_secret_value(_: SecretValueError) -> TtsRuntimeError {
    TtsRuntimeError::Rejected
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use lettuce_settings::{SecretOwnerId, SecretRef};
    use lettuce_types::{AssetId, AudioProviderId, RequestId, Revision, TimestampMillis};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

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
        (format!("http://{address}/api"), captured)
    }

    fn request(endpoint: String) -> SynthesisRequest {
        SynthesisRequest {
            id: RequestId::new(),
            provider: AudioProvider {
                id: AudioProviderId::new(),
                secret_owner_id: SecretOwnerId::new(),
                label: "Compatible speech".into(),
                api_key_ref: Some(SecretRef::new()),
                config: AudioProviderConfig::OpenAiCompatible {
                    base_url: Some(endpoint),
                    request_path: Some("speech".into()),
                },
                revision: Revision::INITIAL,
                created_at: TimestampMillis::new(1),
                updated_at: TimestampMillis::new(1),
            },
            model_id: "gpt-4o-mini-tts".into(),
            voice_id: "alloy".into(),
            prompt: Some("  Speak clearly.  ".into()),
            text: " Keep authored spacing. ".into(),
            output_asset_id: AssetId::new(),
            output_policy: TtsOutputPolicy::Retained,
            created_at: TimestampMillis::new(1),
        }
    }

    #[tokio::test]
    async fn sends_legacy_payload_and_retains_response_mime() {
        let body = b"ID3audio";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: audio/mpeg; charset=binary\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .into_bytes()
        .into_iter()
        .chain(body.iter().copied())
        .collect();
        let (endpoint, captured) = server(response).await;
        let runtime =
            OpenAiCompatibleTtsRuntime::new(Arc::new(JsonClient::new().expect("network client")));
        let result = runtime
            .synthesize(
                &request(endpoint),
                Some(&SecretValue::new("api-key-canary").expect("secret")),
                &CancellationToken::new(),
            )
            .await
            .expect("synthesis");
        assert_eq!(result.bytes, body);
        assert_eq!(result.declared_mime_type, "audio/mpeg");
        let captured = captured.lock().expect("captured request");
        let split = captured
            .windows(4)
            .position(|value| value == b"\r\n\r\n")
            .expect("request headers");
        let headers = String::from_utf8_lossy(&captured[..split]);
        assert!(headers.starts_with("POST /api/speech HTTP/1.1"));
        assert!(
            headers
                .to_ascii_lowercase()
                .contains("authorization: bearer api-key-canary")
        );
        let value: serde_json::Value =
            serde_json::from_slice(&captured[split + 4..]).expect("request JSON");
        assert_eq!(value["model"], "gpt-4o-mini-tts");
        assert_eq!(value["input"], " Keep authored spacing. ");
        assert_eq!(value["voice"], "alloy");
        assert_eq!(value["instructions"], "Speak clearly.");
        assert_eq!(value["response_format"], "mp3");
    }

    #[tokio::test]
    async fn rejects_provider_errors_and_preflight_cancellation() {
        let response = b"HTTP/1.1 400 Bad Request\r\nContent-Length: 2\r\n\r\n{}".to_vec();
        let (endpoint, _) = server(response).await;
        let runtime =
            OpenAiCompatibleTtsRuntime::new(Arc::new(JsonClient::new().expect("network client")));
        let request = request(endpoint);
        let credential = SecretValue::new("secret").expect("secret");
        assert!(matches!(
            runtime
                .synthesize(&request, Some(&credential), &CancellationToken::new())
                .await,
            Err(TtsRuntimeError::Rejected)
        ));
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        assert!(matches!(
            runtime
                .synthesize(&request, Some(&credential), &cancellation)
                .await,
            Err(TtsRuntimeError::Cancelled)
        ));
    }
}
