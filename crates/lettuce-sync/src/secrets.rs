//! Secrets exchanged by sync.
//!
//! API keys never enter the change journal (it is stored in SQLite). After
//! the change exchange, paired devices trade an inventory of the secrets their
//! synced provider records reference, each with the version it was last set
//! at, and fetch the values they lack or hold an older version of over the
//! encrypted session. Values go straight into the native secret store.

use lettuce_settings::{SecretPurpose, SecretRecord, SecretRef};
use lettuce_types::TimestampMillis;
use serde::{Deserialize, Serialize};

use crate::SyncDeviceId;

/// Most secrets one inventory may list.
pub const MAX_SYNC_SECRETS: usize = 1024;

/// When and where a secret value was last set; the later one wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SyncSecretVersion {
    pub set_at: TimestampMillis,
    pub device: SyncDeviceId,
}

/// One secret a device's synced records reference; `version` is `None` when
/// the device does not hold its value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncSecretEntry {
    pub reference: SecretRef,
    pub purpose: SecretPurpose,
    pub version: Option<SyncSecretVersion>,
}

/// The version recorded for the local value of a secret, with the local
/// secret-store generation it was recorded for (a different generation means
/// the value changed here since).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoredSecretVersion {
    pub generation: u64,
    pub version: SyncSecretVersion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SyncSecretError {
    #[error("sync secret state is corrupt")]
    Corrupt,
    #[error("sync secret storage failed")]
    Storage,
}

pub trait SyncSecretRepository: Send + Sync {
    /// Every secret the synced provider accounts and audio providers reference.
    fn referenced_secrets(&self) -> Result<Vec<SecretRecord>, SyncSecretError>;

    fn secret_version(
        &self,
        reference: &SecretRef,
    ) -> Result<Option<StoredSecretVersion>, SyncSecretError>;

    fn record_secret_version(
        &self,
        reference: &SecretRef,
        stored: StoredSecretVersion,
    ) -> Result<(), SyncSecretError>;
}
