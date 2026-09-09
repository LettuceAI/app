use std::path::Path;

use lettuce_model_hub::{
    KokoroAssetError, KokoroAssetStatus, KokoroAssetStore, KokoroModelVariant,
    KokoroModelVariantInfo, kokoro_supported_model_variants,
};
use lettuce_types::ContentHash;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KokoroArtifactSummary {
    pub byte_size: u64,
    pub blake3: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KokoroInstalledVoiceSummary {
    pub id: String,
    pub artifact: KokoroArtifactSummary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KokoroAssetInventory {
    pub variant: String,
    pub variant_allowed_on_platform: bool,
    pub model: Option<KokoroArtifactSummary>,
    pub config: Option<KokoroArtifactSummary>,
    pub tokenizer: Option<KokoroArtifactSummary>,
    pub tokenizer_config: Option<KokoroArtifactSummary>,
    pub installed_voices: Vec<KokoroInstalledVoiceSummary>,
    pub selected_voice_installed: Option<bool>,
}

#[derive(Debug)]
pub struct KokoroAssetInventoryCoordinator {
    store: KokoroAssetStore,
}

impl KokoroAssetInventoryCoordinator {
    pub fn open_managed(root: impl AsRef<Path>) -> Result<Self, KokoroAssetInventoryError> {
        Ok(Self {
            store: KokoroAssetStore::open(root).map_err(KokoroAssetInventoryError::Assets)?,
        })
    }

    #[must_use]
    pub fn supported_variants(&self) -> Vec<KokoroModelVariantInfo> {
        kokoro_supported_model_variants()
    }

    pub fn inspect(
        &self,
        variant: &str,
        selected_voice_id: Option<&str>,
    ) -> Result<KokoroAssetInventory, KokoroAssetInventoryError> {
        let variant =
            KokoroModelVariant::parse(variant).map_err(KokoroAssetInventoryError::Assets)?;
        self.store
            .inspect(variant, selected_voice_id)
            .map(inventory)
            .map_err(KokoroAssetInventoryError::Assets)
    }

    pub fn installed_voices(
        &self,
    ) -> Result<Vec<KokoroInstalledVoiceSummary>, KokoroAssetInventoryError> {
        self.store
            .installed_voices()
            .map(|voices| {
                voices
                    .into_iter()
                    .map(|voice| KokoroInstalledVoiceSummary {
                        id: voice.id,
                        artifact: artifact_summary(voice.artifact),
                    })
                    .collect()
            })
            .map_err(KokoroAssetInventoryError::Assets)
    }
}

fn inventory(status: KokoroAssetStatus) -> KokoroAssetInventory {
    KokoroAssetInventory {
        variant: status.variant.id().to_owned(),
        variant_allowed_on_platform: status.variant_allowed_on_platform,
        model: status.model.map(artifact_summary),
        config: status.config.map(artifact_summary),
        tokenizer: status.tokenizer.map(artifact_summary),
        tokenizer_config: status.tokenizer_config.map(artifact_summary),
        installed_voices: status
            .installed_voices
            .into_iter()
            .map(|voice| KokoroInstalledVoiceSummary {
                id: voice.id,
                artifact: artifact_summary(voice.artifact),
            })
            .collect(),
        selected_voice_installed: status.selected_voice_installed,
    }
}

fn artifact_summary(artifact: lettuce_model_hub::InstalledModelArtifact) -> KokoroArtifactSummary {
    KokoroArtifactSummary {
        byte_size: artifact.byte_size,
        blake3: artifact.blake3,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum KokoroAssetInventoryError {
    #[error("Kokoro asset inventory failed: {0}")]
    Assets(KokoroAssetError),
}

#[cfg(test)]
mod tests {
    use lettuce_types::OperationId;

    use super::*;

    #[test]
    fn exposes_managed_inventory_without_native_paths() {
        let root = std::env::temp_dir().join(format!("kokoro-app-{}", OperationId::new()));
        std::fs::create_dir_all(root.join("onnx")).expect("onnx directory");
        std::fs::create_dir_all(root.join("voices")).expect("voices directory");
        std::fs::write(root.join("onnx/model_quantized.onnx"), b"model").expect("model");
        std::fs::write(root.join("voices/af_heart.bin"), b"voice").expect("voice");
        let coordinator =
            KokoroAssetInventoryCoordinator::open_managed(&root).expect("coordinator");

        let status = coordinator
            .inspect("q8", Some("af_heart"))
            .expect("inventory");
        assert_eq!(status.variant, "int8");
        assert!(status.model.is_some());
        assert_eq!(status.installed_voices[0].id, "af_heart");
        assert_eq!(status.selected_voice_installed, Some(true));
        assert_eq!(
            coordinator.installed_voices().expect("installed voices"),
            status.installed_voices
        );
        assert_eq!(
            coordinator.inspect("q4", None),
            Err(KokoroAssetInventoryError::Assets(
                KokoroAssetError::UnsupportedVariant
            ))
        );
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
