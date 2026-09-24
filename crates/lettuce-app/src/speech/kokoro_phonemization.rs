use lettuce_model_hub::{
    KokoroArtifactRole, KokoroInstallError, KokoroInstallStore, RemoteKokoroModel,
};
use lettuce_platform::EspeakPhonemizer;
use lettuce_speech::{
    KokoroPhonemization, KokoroPhonemizationError, KokoroPhonemizationInput, phonemize_kokoro,
};

#[derive(Debug, thiserror::Error)]
pub enum KokoroPhonemizationCoordinatorError {
    #[error("Kokoro model assets are unavailable: {0}")]
    Install(#[from] KokoroInstallError),
    #[error("Kokoro model assets are incomplete")]
    MissingAssets,
    #[error("Kokoro phonemization failed: {0}")]
    Phonemization(#[from] KokoroPhonemizationError),
}

#[derive(Debug)]
pub struct KokoroPhonemizationCoordinator {
    installs: KokoroInstallStore,
}

impl KokoroPhonemizationCoordinator {
    #[must_use]
    pub const fn new(installs: KokoroInstallStore) -> Self {
        Self { installs }
    }

    pub fn phonemize<P: EspeakPhonemizer + ?Sized>(
        &self,
        model: &RemoteKokoroModel,
        process: &P,
        input: &KokoroPhonemizationInput,
    ) -> Result<KokoroPhonemization, KokoroPhonemizationCoordinatorError> {
        let installed = self
            .installs
            .installed(model)?
            .ok_or(KokoroPhonemizationCoordinatorError::MissingAssets)?;
        for role in [
            KokoroArtifactRole::Config,
            KokoroArtifactRole::Tokenizer,
            KokoroArtifactRole::TokenizerConfig,
            KokoroArtifactRole::Model,
        ] {
            if !installed
                .artifacts
                .iter()
                .any(|artifact| artifact.role == role)
            {
                return Err(KokoroPhonemizationCoordinatorError::MissingAssets);
            }
        }
        phonemize_kokoro(process, input).map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use lettuce_model_hub::{KokoroModelVariant, pinned_kokoro_model};
    use lettuce_platform::EspeakNgError;
    use lettuce_types::AssetId;

    use super::*;

    struct CountingPhonemizer(AtomicUsize);

    impl EspeakPhonemizer for CountingPhonemizer {
        fn phonemize(&self, _: &str, _: &str) -> Result<String, EspeakNgError> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok("həlˈoʊ".to_owned())
        }
    }

    #[test]
    fn rejects_missing_managed_assets_before_process_execution() {
        let root = std::env::temp_dir().join(format!("kokoro-phonemize-{}", AssetId::new()));
        let coordinator = KokoroPhonemizationCoordinator::new(
            KokoroInstallStore::open(&root).expect("install store"),
        );
        let process = CountingPhonemizer(AtomicUsize::new(0));
        let result = coordinator.phonemize(
            &pinned_kokoro_model(KokoroModelVariant::Int8),
            &process,
            &KokoroPhonemizationInput {
                voice_id: "af_heart".to_owned(),
                text: "Hello".to_owned(),
                lexicon: HashMap::new(),
            },
        );

        assert!(matches!(
            result,
            Err(KokoroPhonemizationCoordinatorError::MissingAssets)
        ));
        assert_eq!(process.0.load(Ordering::Relaxed), 0);
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
