//! Typed settings, effective values, secret references, and secure-store ports.
//!
//! The intended ownership, boundaries, migration path, and acceptance gates are
//! specified in the crate PLAN.md. This crate starts behavior-empty so the
//! legacy monolith cannot leak in through premature compatibility APIs.

#![deny(unsafe_op_in_unsafe_fn)]

mod global;
mod secrets;

pub use global::{
    GLOBAL_SETTINGS_FORMAT_VERSION, GlobalSettings, GlobalSettingsStore, GlobalSettingsStoreError,
    LorebookGeneratorSelection, LorebookGeneratorSettings, PureMode, StoredGlobalSettings,
};

pub use secrets::{
    HeaderName, HeaderNameError, InMemorySecretStore, SecretAvailability, SecretBackendError,
    SecretOwnerId, SecretPurpose, SecretRecord, SecretRef, SecretState, SecretStatus, SecretStore,
    SecretStoreError, SecretValue, SecretValueError,
};
