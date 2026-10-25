use std::sync::{Arc, Mutex};
use lettuce_jobs::handle::CancellationToken;
use lettuce_model_hub::{
    KokoroArtifactRole, KokoroInstallError, KokoroInstallStore, RemoteKokoroModel,
};
use lettuce_speech::{
    KokoroPhonemization, KokoroRuntimeError, KokoroVoiceBlend, OnnxKokoroRuntime,
    OnnxRuntimeCommitted, synthesize_kokoro_tokens,
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

type KokoroSessionCache = crate::onnx_models::OnnxSessionCache<
    (std::path::PathBuf, u64, Option<std::time::SystemTime>), Mutex<OnnxKokoroRuntime>,
>;

pub struct KokoroNativeSynthesisCoordinator {
    installs: KokoroInstallStore,
    cache: Arc<KokoroSessionCache>,
    settings: Option<Arc<dyn lettuce_settings::DeviceSettingsStore>>,
}

impl KokoroNativeSynthesisCoordinator {
    #[must_use]
    pub fn new(installs: KokoroInstallStore) -> Self {
        Self::with_cache(installs, crate::onnx_models::OnnxModelManager::new(|_| {}), None)
    }

    pub(crate) fn with_cache(
        installs: KokoroInstallStore,
        manager: Arc<crate::onnx_models::OnnxModelManager>,
        settings: Option<Arc<dyn lettuce_settings::DeviceSettingsStore>>,
    ) -> Self {
        Self { installs, cache: crate::onnx_models::OnnxSessionCache::new(lettuce_settings::OnnxModelKind::Kokoro, manager), settings }
    }

    pub(crate) fn synthesize(
        &self,
        model: &RemoteKokoroModel,
        runtime: OnnxRuntimeCommitted,
        phonemization: &KokoroPhonemization,
        voice: &KokoroVoiceBlend,
        speed: f32,
        cancellation: &CancellationToken,
    ) -> Result<Vec<u8>, KokoroNativeSynthesisError> {
        if cancellation.is_cancelled() {
            return Err(KokoroRuntimeError::Cancelled.into());
        }
        let files = self
            .installs
            .model_files(model)?
            .ok_or(KokoroNativeSynthesisError::MissingAssets)?;
        let model = files
            .iter()
            .find(|file| file.role == KokoroArtifactRole::Model)
            .ok_or(KokoroNativeSynthesisError::MissingAssets)?;
        let metadata = std::fs::metadata(&model.path).map_err(|_| KokoroRuntimeError::ModelLoad)?;
        if let Some(settings) = &self.settings {
            self.cache.refresh_settings(|| settings.load_device_settings())
                .map_err(|_| KokoroRuntimeError::Unavailable)?;
        }
        let loaded = self.cache.acquire((model.path.clone(), metadata.len(), metadata.modified().ok()), || {
            OnnxKokoroRuntime::load(&model.path, runtime).map(|runtime| Arc::new(Mutex::new(runtime)))
        })?;
        let mut runtime = loaded.lock().map_err(|_| KokoroRuntimeError::Unavailable)?;
        synthesize_kokoro_tokens(
            &mut *runtime,
            &phonemization.token_ids,
            voice,
            speed,
            cancellation,
        )
        .map_err(Into::into)
    }
}

impl std::fmt::Debug for KokoroNativeSynthesisCoordinator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KokoroNativeSynthesisCoordinator").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use lettuce_model_hub::{KokoroModelVariant, pinned_kokoro_model};
    use lettuce_types::AssetId;

    use super::*;

    /// The runtime evidence is created without a committed environment, which
    /// is sound here because the missing model bundle is rejected before any
    /// Kokoro session is built.
    #[test]
    fn rejects_missing_model_bundle_before_runtime_initialization() {
        let root = std::env::temp_dir().join(format!("kokoro-native-{}", AssetId::new()));
        let coordinator = KokoroNativeSynthesisCoordinator::new(
            KokoroInstallStore::open(&root).expect("install store"),
        );
        let result = coordinator.synthesize(
            &pinned_kokoro_model(KokoroModelVariant::Int8),
            unsafe { OnnxRuntimeCommitted::after_process_commit() },
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
