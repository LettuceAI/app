use std::{fmt, sync::Arc};

use async_trait::async_trait;
use lettuce_jobs::handle::CancellationToken;
use lettuce_network::JsonClient;
use lettuce_settings::SecretValue;

use crate::{
    AudioProviderConfig, ElevenLabsTtsRuntime, FishSpeechTtsRuntime, FishTtsRuntime,
    GeminiTtsRuntime, OpenAiCompatibleTtsRuntime, RuntimeSynthesis, RuntimeVoiceDesignPreview,
    SynthesisRequest, TtsRuntime, TtsRuntimeError, VoiceDesignRequest, VoiceDesignRuntime,
    VoiceDesignRuntimeError,
};

pub struct RemoteTtsRuntime {
    gemini: Arc<dyn TtsRuntime>,
    elevenlabs: Arc<dyn TtsRuntime>,
    fish: Arc<dyn TtsRuntime>,
    fish_speech: Arc<dyn TtsRuntime>,
    open_ai: Arc<dyn TtsRuntime>,
    voice_design: Arc<dyn VoiceDesignRuntime>,
}

impl RemoteTtsRuntime {
    #[must_use]
    pub fn new(network: Arc<JsonClient>) -> Self {
        let elevenlabs = Arc::new(ElevenLabsTtsRuntime::new(network.clone()));
        Self {
            gemini: Arc::new(GeminiTtsRuntime::new(network.clone())),
            elevenlabs: elevenlabs.clone(),
            fish: Arc::new(FishTtsRuntime::new(network.clone())),
            fish_speech: Arc::new(FishSpeechTtsRuntime::new(network.clone())),
            open_ai: Arc::new(OpenAiCompatibleTtsRuntime::new(network)),
            voice_design: elevenlabs,
        }
    }

    #[cfg(test)]
    fn with_runtimes(
        gemini: Arc<dyn TtsRuntime>,
        elevenlabs: Arc<dyn TtsRuntime>,
        fish: Arc<dyn TtsRuntime>,
        fish_speech: Arc<dyn TtsRuntime>,
        open_ai: Arc<dyn TtsRuntime>,
        voice_design: Arc<dyn VoiceDesignRuntime>,
    ) -> Self {
        Self {
            gemini,
            elevenlabs,
            fish,
            fish_speech,
            open_ai,
            voice_design,
        }
    }
}

#[async_trait]
impl VoiceDesignRuntime for RemoteTtsRuntime {
    async fn design_voice(
        &self,
        request: &VoiceDesignRequest,
        credential: &SecretValue,
        cancellation: &CancellationToken,
    ) -> Result<Vec<RuntimeVoiceDesignPreview>, VoiceDesignRuntimeError> {
        self.voice_design
            .design_voice(request, credential, cancellation)
            .await
    }
}

impl fmt::Debug for RemoteTtsRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RemoteTtsRuntime")
    }
}

#[async_trait]
impl TtsRuntime for RemoteTtsRuntime {
    async fn synthesize(
        &self,
        request: &SynthesisRequest,
        credential: Option<&SecretValue>,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeSynthesis, TtsRuntimeError> {
        let runtime = match &request.provider.config {
            AudioProviderConfig::Gemini { .. } => &self.gemini,
            AudioProviderConfig::Elevenlabs => &self.elevenlabs,
            AudioProviderConfig::FishTts => &self.fish,
            AudioProviderConfig::FishSpeech { .. } => &self.fish_speech,
            AudioProviderConfig::OpenAiCompatible { .. } => &self.open_ai,
            AudioProviderConfig::Kokoro { .. } => return Err(TtsRuntimeError::Rejected),
        };
        runtime.synthesize(request, credential, cancellation).await
    }
}

#[cfg(test)]
mod tests {
    use lettuce_settings::{SecretOwnerId, SecretRef};
    use lettuce_types::{AssetId, AudioProviderId, RequestId, Revision, TimestampMillis};

    use crate::{AudioProvider, TtsOutputPolicy};

    use super::*;

    struct Runtime(&'static str);

    #[async_trait]
    impl TtsRuntime for Runtime {
        async fn synthesize(
            &self,
            _: &SynthesisRequest,
            _: Option<&SecretValue>,
            _: &CancellationToken,
        ) -> Result<RuntimeSynthesis, TtsRuntimeError> {
            Ok(RuntimeSynthesis {
                bytes: self.0.as_bytes().to_vec(),
                declared_mime_type: "audio/mpeg".into(),
            })
        }
    }

    #[async_trait]
    impl VoiceDesignRuntime for Runtime {
        async fn design_voice(
            &self,
            _: &VoiceDesignRequest,
            _: &SecretValue,
            _: &CancellationToken,
        ) -> Result<Vec<RuntimeVoiceDesignPreview>, VoiceDesignRuntimeError> {
            Ok(vec![RuntimeVoiceDesignPreview {
                generated_voice_id: self.0.into(),
                bytes: b"ID3audio".to_vec(),
                duration_secs: 1.0,
                declared_mime_type: "audio/mpeg".into(),
            }])
        }
    }

    fn request(config: AudioProviderConfig) -> SynthesisRequest {
        let key_required = !matches!(
            &config,
            AudioProviderConfig::FishSpeech { .. } | AudioProviderConfig::Kokoro { .. }
        );
        SynthesisRequest {
            id: RequestId::new(),
            provider: AudioProvider {
                id: AudioProviderId::new(),
                secret_owner_id: SecretOwnerId::new(),
                label: "Routed speech".into(),
                api_key_ref: key_required.then(SecretRef::new),
                config,
                revision: Revision::INITIAL,
                created_at: TimestampMillis::new(1),
                updated_at: TimestampMillis::new(1),
            },
            model_id: "model".into(),
            voice_id: "voice".into(),
            prompt: None,
            text: "Speech".into(),
            output_asset_id: AssetId::new(),
            output_policy: TtsOutputPolicy::Retained,
            created_at: TimestampMillis::new(1),
        }
    }

    #[tokio::test]
    async fn routes_each_remote_provider_and_rejects_kokoro() {
        let runtime = RemoteTtsRuntime::with_runtimes(
            Arc::new(Runtime("gemini")),
            Arc::new(Runtime("elevenlabs")),
            Arc::new(Runtime("fish")),
            Arc::new(Runtime("fish-speech")),
            Arc::new(Runtime("open-ai")),
            Arc::new(Runtime("designed")),
        );
        let cases = [
            (
                AudioProviderConfig::Gemini {
                    project_id: Some("project".into()),
                    location: "us-central1".into(),
                },
                "gemini",
            ),
            (AudioProviderConfig::Elevenlabs, "elevenlabs"),
            (AudioProviderConfig::FishTts, "fish"),
            (
                AudioProviderConfig::FishSpeech {
                    base_url: None,
                    request_path: None,
                },
                "fish-speech",
            ),
            (
                AudioProviderConfig::OpenAiCompatible {
                    base_url: Some("https://speech.example".into()),
                    request_path: None,
                },
                "open-ai",
            ),
        ];
        for (config, expected) in cases {
            let result = runtime
                .synthesize(&request(config), None, &CancellationToken::new())
                .await
                .expect("routed synthesis");
            assert_eq!(result.bytes, expected.as_bytes());
        }
        let design_request = VoiceDesignRequest {
            id: RequestId::new(),
            provider: request(AudioProviderConfig::Elevenlabs).provider,
            text_sample: "A".repeat(100),
            voice_description: "A warm and expressive narrator".into(),
            model_id: None,
            num_previews: Some(1),
            expires_at: TimestampMillis::new(2),
            created_at: TimestampMillis::new(1),
        };
        let designed = runtime
            .design_voice(
                &design_request,
                &SecretValue::new("secret").expect("secret"),
                &CancellationToken::new(),
            )
            .await
            .expect("routed voice design");
        assert_eq!(designed[0].generated_voice_id, "designed");
        assert!(matches!(
            runtime
                .synthesize(
                    &request(AudioProviderConfig::Kokoro { variant: None }),
                    None,
                    &CancellationToken::new(),
                )
                .await,
            Err(TtsRuntimeError::Rejected)
        ));
    }
}
