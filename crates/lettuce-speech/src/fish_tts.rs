use std::sync::Arc;

use async_trait::async_trait;
use lettuce_jobs::handle::CancellationToken;
use lettuce_network::{JsonAuth, JsonClient, JsonClientError, JsonSecretHeader, RequestPolicy};
use lettuce_settings::{HeaderName, SecretValue};
use serde::Serialize;

use crate::{AudioProviderConfig, RuntimeSynthesis, SynthesisRequest, TtsRuntime, TtsRuntimeError};

const ENDPOINT: &str = "https://api.fish.audio";

#[derive(Debug, Clone)]
pub struct FishTtsRuntime {
    network: Arc<JsonClient>,
    endpoint: String,
}

impl FishTtsRuntime {
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
struct FishProsodyControl {
    speed: f32,
    volume: i32,
    normalize_loudness: bool,
}

#[derive(Serialize)]
struct FishTtsRequest<'a> {
    text: &'a str,
    reference_id: &'a str,
    format: &'static str,
    sample_rate: u32,
    mp3_bitrate: u32,
    normalize: bool,
    latency: &'static str,
    prosody: FishProsodyControl,
}

#[async_trait]
impl TtsRuntime for FishTtsRuntime {
    async fn synthesize(
        &self,
        request: &SynthesisRequest,
        credential: Option<&SecretValue>,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeSynthesis, TtsRuntimeError> {
        request.validate().map_err(|_| TtsRuntimeError::Rejected)?;
        if !matches!(&request.provider.config, AudioProviderConfig::FishTts) {
            return Err(TtsRuntimeError::Rejected);
        }
        let credential = credential.ok_or(TtsRuntimeError::Rejected)?;
        let auth = credential
            .with(|value| SecretValue::new(value.to_owned()))
            .map_err(|_| TtsRuntimeError::Rejected)?;
        let model =
            SecretValue::new(request.model_id.clone()).map_err(|_| TtsRuntimeError::Rejected)?;
        let speed = request
            .prompt
            .as_deref()
            .and_then(parse_speed_from_prompt)
            .unwrap_or(1.0)
            .clamp(0.7, 1.3);
        let body = serde_json::to_vec(&FishTtsRequest {
            text: &request.text,
            reference_id: &request.voice_id,
            format: "mp3",
            sample_rate: 44_100,
            mp3_bitrate: 128,
            normalize: true,
            latency: "normal",
            prosody: FishProsodyControl {
                speed,
                volume: 0,
                normalize_loudness: true,
            },
        })
        .map_err(|_| TtsRuntimeError::Rejected)?;
        if cancellation.is_cancelled() {
            return Err(TtsRuntimeError::Cancelled);
        }
        let response = tokio::select! {
            response = self.network.post_json(
                &self.endpoint,
                "/v1/tts",
                body,
                &[],
                JsonAuth::Bearer(auth),
                vec![JsonSecretHeader {
                    name: HeaderName::new("model").map_err(|_| TtsRuntimeError::Rejected)?,
                    value: model,
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
        Ok(RuntimeSynthesis {
            bytes: response.body,
            declared_mime_type: "audio/mpeg".into(),
        })
    }
}

fn parse_speed_from_prompt(prompt: &str) -> Option<f32> {
    serde_json::from_str::<serde_json::Value>(prompt)
        .ok()
        .and_then(|value| value.get("speed").and_then(serde_json::Value::as_f64))
        .map(|value| value as f32)
        .filter(|value| value.is_finite() && *value > 0.0)
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

    fn request(prompt: Option<&str>) -> SynthesisRequest {
        SynthesisRequest {
            id: RequestId::new(),
            provider: AudioProvider {
                id: AudioProviderId::new(),
                secret_owner_id: SecretOwnerId::new(),
                label: "Fish Audio".into(),
                api_key_ref: Some(SecretRef::new()),
                config: AudioProviderConfig::FishTts,
                revision: Revision::INITIAL,
                created_at: TimestampMillis::new(1),
                updated_at: TimestampMillis::new(1),
            },
            model_id: "s2-pro".into(),
            voice_id: "reference-canary".into(),
            prompt: prompt.map(str::to_owned),
            text: "Keep authored spacing.".into(),
            output_asset_id: AssetId::new(),
            output_policy: TtsOutputPolicy::Retained,
            created_at: TimestampMillis::new(1),
        }
    }

    #[tokio::test]
    async fn sends_legacy_headers_payload_and_clamped_speed() {
        let (endpoint, captured) =
            server(b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\n\r\nID3audio".to_vec()).await;
        let runtime = FishTtsRuntime::with_endpoint(
            Arc::new(JsonClient::new().expect("network client")),
            endpoint,
        );
        let result = runtime
            .synthesize(
                &request(Some(r#"{"speed": 2.5}"#)),
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
        assert!(headers.starts_with("POST /v1/tts HTTP/1.1"));
        let lowered = headers.to_ascii_lowercase();
        assert!(lowered.contains("authorization: bearer api-key-canary"));
        assert!(lowered.contains("model: s2-pro"));
        let value: serde_json::Value =
            serde_json::from_slice(&captured[split + 4..]).expect("request JSON");
        assert_eq!(value["text"], "Keep authored spacing.");
        assert_eq!(value["reference_id"], "reference-canary");
        assert_eq!(value["format"], "mp3");
        assert_eq!(value["sample_rate"], 44_100);
        assert_eq!(value["mp3_bitrate"], 128);
        assert_eq!(value["normalize"], true);
        assert_eq!(value["latency"], "normal");
        assert_eq!(value["prosody"]["speed"], 1.3);
        assert_eq!(value["prosody"]["volume"], 0);
        assert_eq!(value["prosody"]["normalize_loudness"], true);
    }

    #[tokio::test]
    async fn rejects_provider_errors_and_preflight_cancellation() {
        let (endpoint, _) =
            server(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 2\r\n\r\n{}".to_vec()).await;
        let runtime = FishTtsRuntime::with_endpoint(
            Arc::new(JsonClient::new().expect("network client")),
            endpoint,
        );
        let credential = SecretValue::new("secret").expect("secret");
        assert!(matches!(
            runtime
                .synthesize(&request(None), Some(&credential), &CancellationToken::new(),)
                .await,
            Err(TtsRuntimeError::Rejected)
        ));
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        assert!(matches!(
            runtime
                .synthesize(&request(None), Some(&credential), &cancellation)
                .await,
            Err(TtsRuntimeError::Cancelled)
        ));
    }

    #[test]
    fn preserves_legacy_speed_prompt_rules() {
        assert_eq!(
            parse_speed_from_prompt(r#"{"speed": 0.1}"#)
                .expect("positive finite speed")
                .clamp(0.7, 1.3),
            0.7
        );
        assert_eq!(parse_speed_from_prompt(r#"{"speed": 1.15}"#), Some(1.15));
        assert_eq!(parse_speed_from_prompt(r#"{"speed": -1}"#), None);
        assert_eq!(parse_speed_from_prompt(r#"{"speed": "fast"}"#), None);
        assert_eq!(parse_speed_from_prompt("not JSON"), None);
    }
}
