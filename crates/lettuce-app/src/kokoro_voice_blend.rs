use lettuce_model_hub::{
    KokoroInstallError, KokoroVoiceInstallStore, RemoteKokoroVoice,
};
use lettuce_speech::{
    KokoroVoiceBlend, KokoroVoiceBlendSpec, KokoroVoiceError, KokoroVoiceMaterial,
    blend_kokoro_voices, normalize_kokoro_voice_blend,
};

#[derive(Debug, thiserror::Error)]
pub enum KokoroVoiceBlendCoordinatorError {
    #[error("Kokoro voice installation failed: {0}")]
    Install(#[from] KokoroInstallError),
    #[error("Kokoro voice blend failed: {0}")]
    Voice(#[from] KokoroVoiceError),
    #[error("Kokoro voice descriptor is missing or ambiguous")]
    MissingDescriptor,
    #[error("Kokoro voice assets are incomplete")]
    MissingAssets,
}

#[derive(Debug)]
pub struct KokoroVoiceBlendCoordinator {
    installs: KokoroVoiceInstallStore,
}

impl KokoroVoiceBlendCoordinator {
    #[must_use]
    pub const fn new(installs: KokoroVoiceInstallStore) -> Self {
        Self { installs }
    }

    pub fn blend(
        &self,
        remotes: &[RemoteKokoroVoice],
        specs: &[KokoroVoiceBlendSpec],
    ) -> Result<KokoroVoiceBlend, KokoroVoiceBlendCoordinatorError> {
        let normalized = normalize_kokoro_voice_blend(specs)?;
        let mut selected = Vec::with_capacity(normalized.len());
        for spec in &normalized {
            let mut matches = remotes.iter().filter(|remote| remote.id == spec.voice_id);
            let remote = matches
                .next()
                .ok_or(KokoroVoiceBlendCoordinatorError::MissingDescriptor)?;
            if matches.next().is_some() {
                return Err(KokoroVoiceBlendCoordinatorError::MissingDescriptor);
            }
            selected.push(remote.clone());
        }
        let materialized = self
            .installs
            .materialize(&selected)?
            .ok_or(KokoroVoiceBlendCoordinatorError::MissingAssets)?;
        let materials = materialized
            .iter()
            .map(|voice| KokoroVoiceMaterial {
                voice_id: voice.id(),
                bytes: voice.bytes(),
            })
            .collect::<Vec<_>>();
        blend_kokoro_voices(&normalized, &materials).map_err(Into::into)
    }

    pub fn blend_installed(
        &self,
        specs: &[KokoroVoiceBlendSpec],
    ) -> Result<KokoroVoiceBlend, KokoroVoiceBlendCoordinatorError> {
        let remotes = self.installs.installed_descriptors()?;
        self.blend(&remotes, specs)
    }
}

#[cfg(test)]
mod tests {
    use lettuce_model_hub::{KOKORO_SOURCE_REVISION, KokoroVoicePreparation};
    use lettuce_speech::KOKORO_STYLE_DIMENSIONS;
    use lettuce_types::AssetId;

    use super::*;

    #[test]
    fn materializes_verified_voice_before_selecting_its_style() {
        let root = std::env::temp_dir().join(format!("kokoro-blend-{}", AssetId::new()));
        let bytes = std::iter::repeat_n(2.5_f32, KOKORO_STYLE_DIMENSIONS)
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>();
        let remote = RemoteKokoroVoice::pinned(
            "af_heart",
            KOKORO_SOURCE_REVISION,
            u64::try_from(bytes.len()).expect("voice size"),
            "e3389576be529aa72dbdaf5de49605c383ac1e6c91815e735500bf362260cc2c",
        )
        .expect("remote voice");
        let installs = KokoroVoiceInstallStore::open(&root).expect("install store");
        let KokoroVoicePreparation::Download(mut download) =
            installs.prepare(remote.clone()).expect("preparation")
        else {
            panic!("expected download");
        };
        download.append(&bytes).expect("voice bytes");
        download.finish().expect("verified voice");
        drop(installs);
        let coordinator = KokoroVoiceBlendCoordinator::new(
            KokoroVoiceInstallStore::open(&root).expect("reopened install store"),
        );

        let blend = coordinator
            .blend_installed(&[KokoroVoiceBlendSpec {
                voice_id: "af_heart".to_owned(),
                weight: 4.0,
            }])
            .expect("voice blend");

        assert_eq!(blend.normalized_specs()[0].weight, 1.0);
        assert_eq!(
            blend.style_for_token_count(500),
            [2.5; KOKORO_STYLE_DIMENSIONS]
        );
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
