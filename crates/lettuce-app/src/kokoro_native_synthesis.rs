use lettuce_jobs::handle::CancellationToken;
use lettuce_model_hub::{
    KokoroArtifactRole, KokoroInstallError, KokoroInstallStore, RemoteKokoroModel,
};
use lettuce_speech::{
    KokoroOnnxRuntimeLink, KokoroPhonemization, KokoroRuntimeError, KokoroVoiceBlend,
    OnnxKokoroRuntime, synthesize_kokoro_tokens,
};

#[derive(Debug, thiserror::Error)]
pub enum KokoroNativeSynthesisError {
    #[error("Kokoro model installation failed: {0}")]
    Install(#[from] KokoroInstallError),
    #[error("Kokoro model assets are incomplete")]
    MissingAssets,
    #[error("Kokoro native synthesis failed: {0}")]
    Runtime(#[from] KokoroRuntimeError),
}

#[derive(Debug)]
pub struct KokoroNativeSynthesisCoordinator {
    installs: KokoroInstallStore,
}

impl KokoroNativeSynthesisCoordinator {
    #[must_use]
    pub const fn new(installs: KokoroInstallStore) -> Self {
        Self { installs }
    }

    pub fn synthesize(
        &self,
        model: &RemoteKokoroModel,
        runtime_link: &KokoroOnnxRuntimeLink,
        phonemization: &KokoroPhonemization,
        voice: &KokoroVoiceBlend,
        speed: f32,
        cancellation: &CancellationToken,
    ) -> Result<Vec<u8>, KokoroNativeSynthesisError> {
        if cancellation.is_cancelled() {
            return Err(KokoroRuntimeError::Cancelled.into());
        }
        let installed = self
            .installs
            .installed(model)?
            .ok_or(KokoroNativeSynthesisError::MissingAssets)?;
        let model = installed
            .artifacts
            .iter()
            .find(|artifact| artifact.role == KokoroArtifactRole::Model)
            .ok_or(KokoroNativeSynthesisError::MissingAssets)?;
        let mut runtime = OnnxKokoroRuntime::load(&model.artifact.path, runtime_link)?;
        synthesize_kokoro_tokens(
            &mut runtime,
            &phonemization.token_ids,
            voice,
            speed,
            cancellation,
        )
        .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use lettuce_model_hub::{KokoroModelVariant, pinned_kokoro_model};
    use lettuce_types::AssetId;

    use super::*;

    #[test]
    fn rejects_missing_model_bundle_before_runtime_initialization() {
        let root = std::env::temp_dir().join(format!("kokoro-native-{}", AssetId::new()));
        let coordinator = KokoroNativeSynthesisCoordinator::new(
            KokoroInstallStore::open(&root).expect("install store"),
        );
        let result = coordinator.synthesize(
            &pinned_kokoro_model(KokoroModelVariant::Int8),
            &KokoroOnnxRuntimeLink::Linked,
            &KokoroPhonemization {
                normalized_text: String::new(),
                effective_text: String::new(),
                language: "en-US".to_owned(),
                used_lexicon_entries: Vec::new(),
                segments: Vec::new(),
                token_ids: Vec::new(),
            },
            &voice(),
            1.0,
            &CancellationToken::new(),
        );

        assert!(matches!(result, Err(KokoroNativeSynthesisError::MissingAssets)));
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    fn voice() -> KokoroVoiceBlend {
        let bytes = std::iter::repeat_n(1.0_f32, lettuce_speech::KOKORO_STYLE_DIMENSIONS)
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>();
        lettuce_speech::blend_kokoro_voices(
            &[lettuce_speech::KokoroVoiceBlendSpec {
                voice_id: "af_heart".to_owned(),
                weight: 1.0,
            }],
            &[lettuce_speech::KokoroVoiceMaterial {
                voice_id: "af_heart",
                bytes: &bytes,
            }],
        )
        .expect("voice")
    }
}
