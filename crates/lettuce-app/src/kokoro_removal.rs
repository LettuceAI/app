use lettuce_model_hub::{
    KokoroInstallError, KokoroInstallStore, KokoroVoiceInstallStore, RemoteKokoroModel,
    RemoteKokoroVoice,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KokoroManagedRemoval {
    pub removed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum KokoroManagedRemovalError {
    #[error("Kokoro managed removal failed: {0}")]
    Install(KokoroInstallError),
}

pub fn remove_managed_kokoro_model(
    installs: &KokoroInstallStore,
    model: &RemoteKokoroModel,
) -> Result<KokoroManagedRemoval, KokoroManagedRemovalError> {
    installs
        .remove_managed_model(model)
        .map(|removed| KokoroManagedRemoval { removed })
        .map_err(KokoroManagedRemovalError::Install)
}

pub fn remove_managed_kokoro_voice(
    installs: &KokoroVoiceInstallStore,
    voice: &RemoteKokoroVoice,
) -> Result<KokoroManagedRemoval, KokoroManagedRemovalError> {
    installs
        .remove_managed(voice)
        .map(|removed| KokoroManagedRemoval { removed })
        .map_err(KokoroManagedRemovalError::Install)
}
