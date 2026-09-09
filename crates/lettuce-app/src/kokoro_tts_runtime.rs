use std::{collections::HashMap, fmt, sync::Arc};

use async_trait::async_trait;
use lettuce_jobs::handle::CancellationToken;
use lettuce_model_hub::{
    KokoroInstallError, KokoroInstallStore, KokoroModelVariant, KokoroVoiceInstallStore,
    kokoro_platform_allows_variant, pinned_kokoro_model,
};
use lettuce_platform::{EspeakNgError, EspeakPhonemizer};
use lettuce_settings::SecretValue;
use lettuce_speech::{
    AudioProviderConfig, AudioProviderKind, KokoroOnnxRuntimeLink, KokoroPhonemizationError,
    KokoroPhonemizationInput, KokoroRuntimeError, KokoroVoiceBlendSpec, KokoroVoiceError,
    RuntimeSynthesis, SynthesisRequest, TtsRuntime, TtsRuntimeError,
};
use serde::Deserialize;

use crate::{
    KokoroNativeSynthesisCoordinator, KokoroNativeSynthesisError, KokoroPhonemizationCoordinator,
    KokoroPhonemizationCoordinatorError, KokoroVoiceBlendCoordinator,
    KokoroVoiceBlendCoordinatorError,
};

pub struct ApplicationTtsRuntime {
    remote: Arc<dyn TtsRuntime>,
    kokoro: Arc<dyn TtsRuntime>,
}

impl ApplicationTtsRuntime {
    #[must_use]
    pub fn new(remote: Arc<dyn TtsRuntime>, kokoro: Arc<dyn TtsRuntime>) -> Self {
        Self { remote, kokoro }
    }
}

impl fmt::Debug for ApplicationTtsRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ApplicationTtsRuntime")
    }
}

#[async_trait]
impl TtsRuntime for ApplicationTtsRuntime {
    async fn synthesize(
        &self,
        request: &SynthesisRequest,
        credential: Option<&SecretValue>,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeSynthesis, TtsRuntimeError> {
        let runtime = if request.provider.config.provider_kind() == AudioProviderKind::Kokoro {
            &self.kokoro
        } else {
            &self.remote
        };
        runtime.synthesize(request, credential, cancellation).await
    }
}

pub struct KokoroTtsRuntime {
    models: KokoroInstallStore,
    voices: KokoroVoiceInstallStore,
    phonemizer: Arc<dyn EspeakPhonemizer>,
    runtime_link: KokoroOnnxRuntimeLink,
}

impl KokoroTtsRuntime {
    #[must_use]
    pub fn new(
        models: KokoroInstallStore,
        voices: KokoroVoiceInstallStore,
        phonemizer: Arc<dyn EspeakPhonemizer>,
        runtime_link: KokoroOnnxRuntimeLink,
    ) -> Self {
        Self {
            models,
            voices,
            phonemizer,
            runtime_link,
        }
    }
}

impl fmt::Debug for KokoroTtsRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("KokoroTtsRuntime")
    }
}

#[async_trait]
impl TtsRuntime for KokoroTtsRuntime {
    async fn synthesize(
        &self,
        request: &SynthesisRequest,
        credential: Option<&SecretValue>,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeSynthesis, TtsRuntimeError> {
        if !matches!(request.provider.config, AudioProviderConfig::Kokoro { .. })
            || credential.is_some()
        {
            return Err(TtsRuntimeError::Rejected);
        }
        if cancellation.is_cancelled() {
            return Err(TtsRuntimeError::Cancelled);
        }
        let variant =
            KokoroModelVariant::parse(&request.model_id).map_err(|_| TtsRuntimeError::Rejected)?;
        if !kokoro_platform_allows_variant(variant) {
            return Err(TtsRuntimeError::Rejected);
        }
        let specs = parse_voice_blend(&request.voice_id)?;
        let speed = parse_speed(request.prompt.as_deref());
        let text = request.text.clone();
        let models = self.models.clone();
        let voices = self.voices.clone();
        let phonemizer = Arc::clone(&self.phonemizer);
        let runtime_link = self.runtime_link.clone();
        let cancellation = cancellation.clone();
        tokio::task::spawn_blocking(move || {
            let model = pinned_kokoro_model(variant);
            let voice = KokoroVoiceBlendCoordinator::new(voices)
                .blend_installed(&specs)
                .map_err(map_voice_error)?;
            let primary_voice = voice
                .normalized_specs()
                .first()
                .ok_or(TtsRuntimeError::Rejected)?
                .voice_id
                .clone();
            let phonemization = KokoroPhonemizationCoordinator::new(models.clone())
                .phonemize(
                    &model,
                    phonemizer.as_ref(),
                    &KokoroPhonemizationInput {
                        voice_id: primary_voice,
                        text,
                        lexicon: HashMap::new(),
                    },
                )
                .map_err(map_phonemization_error)?;
            let bytes = KokoroNativeSynthesisCoordinator::new(models)
                .synthesize(
                    &model,
                    &runtime_link,
                    &phonemization,
                    &voice,
                    speed,
                    &cancellation,
                )
                .map_err(map_native_error)?;
            Ok(RuntimeSynthesis {
                bytes,
                declared_mime_type: "audio/wav".to_owned(),
            })
        })
        .await
        .map_err(|_| TtsRuntimeError::Failed)?
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PersistedVoiceBlend {
    voice_id: String,
    weight: f32,
}

fn parse_voice_blend(value: &str) -> Result<Vec<KokoroVoiceBlendSpec>, TtsRuntimeError> {
    if value.starts_with('[') {
        let entries = serde_json::from_str::<Vec<PersistedVoiceBlend>>(value)
            .map_err(|_| TtsRuntimeError::Rejected)?;
        return Ok(entries
            .into_iter()
            .filter(|entry| entry.weight > 0.0)
            .map(|entry| KokoroVoiceBlendSpec {
                voice_id: entry.voice_id,
                weight: entry.weight,
            })
            .collect());
    }
    Ok(vec![KokoroVoiceBlendSpec {
        voice_id: value.to_owned(),
        weight: 1.0,
    }])
}

fn parse_speed(prompt: Option<&str>) -> f32 {
    prompt
        .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
        .and_then(|value| value.get("speed").and_then(serde_json::Value::as_f64))
        .map(|value| value as f32)
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(1.0)
}

fn map_voice_error(error: KokoroVoiceBlendCoordinatorError) -> TtsRuntimeError {
    match error {
        KokoroVoiceBlendCoordinatorError::Install(KokoroInstallError::Mismatch)
        | KokoroVoiceBlendCoordinatorError::Install(KokoroInstallError::InvalidArtifact)
        | KokoroVoiceBlendCoordinatorError::Install(KokoroInstallError::Unreadable) => {
            TtsRuntimeError::Failed
        }
        KokoroVoiceBlendCoordinatorError::Install(KokoroInstallError::InvalidManifest)
        | KokoroVoiceBlendCoordinatorError::Voice(
            KokoroVoiceError::InvalidBlend | KokoroVoiceError::InvalidVoiceData,
        ) => TtsRuntimeError::Rejected,
        KokoroVoiceBlendCoordinatorError::Install(KokoroInstallError::Platform(_))
        | KokoroVoiceBlendCoordinatorError::Voice(KokoroVoiceError::MissingVoice)
        | KokoroVoiceBlendCoordinatorError::MissingDescriptor
        | KokoroVoiceBlendCoordinatorError::MissingAssets => TtsRuntimeError::Unavailable,
    }
}

fn map_phonemization_error(error: KokoroPhonemizationCoordinatorError) -> TtsRuntimeError {
    match error {
        KokoroPhonemizationCoordinatorError::Install(KokoroInstallError::InvalidManifest)
        | KokoroPhonemizationCoordinatorError::Phonemization(
            KokoroPhonemizationError::InvalidInput | KokoroPhonemizationError::LimitExceeded,
        ) => TtsRuntimeError::Rejected,
        KokoroPhonemizationCoordinatorError::MissingAssets
        | KokoroPhonemizationCoordinatorError::Install(KokoroInstallError::Platform(_))
        | KokoroPhonemizationCoordinatorError::Phonemization(KokoroPhonemizationError::Process(
            EspeakNgError::Unavailable,
        )) => TtsRuntimeError::Unavailable,
        KokoroPhonemizationCoordinatorError::Install(_)
        | KokoroPhonemizationCoordinatorError::Phonemization(_) => TtsRuntimeError::Failed,
    }
}

fn map_native_error(error: KokoroNativeSynthesisError) -> TtsRuntimeError {
    match error {
        KokoroNativeSynthesisError::Runtime(KokoroRuntimeError::Cancelled) => {
            TtsRuntimeError::Cancelled
        }
        KokoroNativeSynthesisError::Runtime(KokoroRuntimeError::InvalidInput) => {
            TtsRuntimeError::Rejected
        }
        KokoroNativeSynthesisError::MissingAssets
        | KokoroNativeSynthesisError::Install(KokoroInstallError::Platform(_))
        | KokoroNativeSynthesisError::Runtime(KokoroRuntimeError::Unavailable) => {
            TtsRuntimeError::Unavailable
        }
        KokoroNativeSynthesisError::Install(KokoroInstallError::InvalidManifest) => {
            TtsRuntimeError::Rejected
        }
        KokoroNativeSynthesisError::Install(_) | KokoroNativeSynthesisError::Runtime(_) => {
            TtsRuntimeError::Failed
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use lettuce_platform::EspeakNgError;
    use lettuce_settings::SecretOwnerId;
    use lettuce_types::{AssetId, AudioProviderId, RequestId, Revision, TimestampMillis};

    use super::*;

    struct CountingPhonemizer(AtomicUsize);

    impl EspeakPhonemizer for CountingPhonemizer {
        fn phonemize(&self, _: &str, _: &str) -> Result<String, EspeakNgError> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok("həlˈoʊ".to_owned())
        }
    }

    struct RecordingRuntime {
        calls: Arc<AtomicUsize>,
        bytes: &'static [u8],
    }

    #[async_trait]
    impl TtsRuntime for RecordingRuntime {
        async fn synthesize(
            &self,
            _: &SynthesisRequest,
            _: Option<&SecretValue>,
            _: &CancellationToken,
        ) -> Result<RuntimeSynthesis, TtsRuntimeError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Ok(RuntimeSynthesis {
                bytes: self.bytes.to_vec(),
                declared_mime_type: "audio/wav".to_owned(),
            })
        }
    }

    fn request() -> SynthesisRequest {
        SynthesisRequest {
            id: RequestId::new(),
            provider: lettuce_speech::AudioProvider {
                id: AudioProviderId::new(),
                secret_owner_id: SecretOwnerId::new(),
                label: "Local speech".to_owned(),
                api_key_ref: None,
                config: AudioProviderConfig::Kokoro {
                    variant: Some("int8".to_owned()),
                },
                revision: Revision::INITIAL,
                created_at: TimestampMillis::new(1),
                updated_at: TimestampMillis::new(1),
            },
            model_id: "int8".to_owned(),
            voice_id: "af_heart".to_owned(),
            prompt: Some(r#"{"speed":1.25}"#.to_owned()),
            text: "Hello".to_owned(),
            output_asset_id: AssetId::new(),
            output_policy: lettuce_speech::TtsOutputPolicy::Retained,
            created_at: TimestampMillis::new(1),
        }
    }

    #[test]
    fn preserves_legacy_voice_and_speed_documents() {
        let specs = parse_voice_blend(
            r#"[{"voiceId":"af_heart","weight":2.0},{"voiceId":"bf_emma","weight":0.0}]"#,
        )
        .expect("voice blend");
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].voice_id, "af_heart");
        assert_eq!(specs[0].weight, 2.0);
        assert_eq!(parse_speed(Some(r#"{"speed":1.25}"#)), 1.25);
        assert_eq!(parse_speed(Some(r#"{"speed":-1}"#)), 1.0);
        assert_eq!(parse_speed(Some("invalid")), 1.0);
    }

    #[tokio::test]
    async fn missing_voice_retries_before_phonemization() {
        let root = std::env::temp_dir().join(format!(
            "kokoro-tts-runtime-{}",
            lettuce_types::OperationId::new()
        ));
        let phonemizer = Arc::new(CountingPhonemizer(AtomicUsize::new(0)));
        let runtime = KokoroTtsRuntime::new(
            KokoroInstallStore::open(&root).expect("model store"),
            KokoroVoiceInstallStore::open(&root).expect("voice store"),
            phonemizer.clone(),
            KokoroOnnxRuntimeLink::Linked,
        );

        assert!(matches!(
            runtime
                .synthesize(&request(), None, &CancellationToken::new())
                .await,
            Err(TtsRuntimeError::Unavailable)
        ));
        assert_eq!(phonemizer.0.load(Ordering::Relaxed), 0);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[tokio::test]
    async fn application_runtime_routes_kokoro_without_remote_dispatch() {
        let remote_calls = Arc::new(AtomicUsize::new(0));
        let kokoro_calls = Arc::new(AtomicUsize::new(0));
        let runtime = ApplicationTtsRuntime::new(
            Arc::new(RecordingRuntime {
                calls: remote_calls.clone(),
                bytes: b"remote",
            }),
            Arc::new(RecordingRuntime {
                calls: kokoro_calls.clone(),
                bytes: b"kokoro",
            }),
        );

        let output = runtime
            .synthesize(&request(), None, &CancellationToken::new())
            .await
            .expect("Kokoro synthesis");

        assert_eq!(output.bytes, b"kokoro");
        assert_eq!(kokoro_calls.load(Ordering::Relaxed), 1);
        assert_eq!(remote_calls.load(Ordering::Relaxed), 0);
    }
}
