//! Typed settings, effective values, secret references, and secure-store ports.
//!
//! The intended ownership, boundaries, and acceptance gates are specified in
//! the crate PLAN.md.

#![deny(unsafe_op_in_unsafe_fn)]

mod device;
mod global;
#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "ios",
    target_os = "windows"
))]
mod native_secrets;
mod passphrase_vault;
mod secrets;

pub use device::{
    DeviceEmbeddingSettings, DeviceSettings, DeviceSettingsStore, EmbeddingModelVersion,
    MAX_CERTIFICATE_NAME_BYTES, TrustedCertificate,
};
pub use global::{
    CompanionSoulWriterSettings, CreationHelperSettings, CreationHelperToolFallback,
    DEFAULT_LOREBOOK_SCAN_DEPTH, DEFAULT_MIN_SIMILARITY_BASIS_POINTS,
    DeviceUiStateStore, DynamicMemoryPromptSelection, DynamicMemorySettings, EmbeddingSettings,
    GLOBAL_SETTINGS_FORMAT_VERSION, GlobalSettings, GlobalSettingsStore, GlobalSettingsStoreError,
    HelpMeReplySettings, HelpMeReplyStyle, ImageGenerationSettings, LorebookEntryGeneratorSettings,
    LOREBOOK_SCAN_DEPTH_RANGE, LorebookGeneratorSelection, LorebookGeneratorSettings,
    MemoryRetrievalStrategy, MemoryRunMode,
    MAX_UI_PREFERENCES_BYTES, MemoryStructuredFallbackFormat, PureMode, SceneGenerationMode,
    StoredGlobalSettings, UiPreferences,
};

pub use passphrase_vault::{PassphraseVault, PassphraseVaultError, VaultFile};
pub use secrets::{
    HeaderName, HeaderNameError, InMemorySecretStore, SecretAvailability, SecretBackendError,
    SecretOwnerId, SecretPurpose, SecretRecord, SecretRef, SecretState, SecretStatus, SecretStore,
    SecretStoreError, SecretValue, SecretValueError,
};

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "ios",
    target_os = "windows"
))]
pub use native_secrets::NativeSecretStore;
