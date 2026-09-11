//! Typed settings, effective values, secret references, and secure-store ports.
//!
//! The intended ownership, boundaries, migration path, and acceptance gates are
//! specified in the crate PLAN.md. This crate starts behavior-empty so the
//! legacy monolith cannot leak in through premature compatibility APIs.

#![deny(unsafe_op_in_unsafe_fn)]

mod global;
#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "ios",
    target_os = "windows"
))]
mod native_secrets;
mod secrets;

pub use global::{
    DynamicMemorySettings, GLOBAL_SETTINGS_FORMAT_VERSION, GlobalSettings, GlobalSettingsStore,
    GlobalSettingsStoreError, LorebookGeneratorSelection, LorebookGeneratorSettings,
    MemoryRetrievalStrategy, MemoryRunMode, PureMode, StoredGlobalSettings,
};

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
