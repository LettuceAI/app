//! The small, local secret boundary used by provider-facing adapters.
//!
//! This crate does not encrypt or persist bytes. An application supplies a
//! platform adapter for [`SecretStore`]. Backup/sync vaults, native ingress,
//! and IPC contracts belong to later crates once their formats exist.

use std::{
    collections::BTreeMap,
    fmt,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

const MAX_HEADER_NAME_BYTES: usize = 128;
const MAX_SECRET_VALUE_BYTES: usize = 16 * 1024;

/// An opaque, serializable reference to material held by a [`SecretStore`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SecretRef(Uuid);

impl SecretRef {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    #[must_use]
    pub const fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for SecretRef {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SecretRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Opaque owner identity. The provider/audio domain owns the account record;
/// settings only carries this identity to bind a secret purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SecretOwnerId(Uuid);

impl SecretOwnerId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    #[must_use]
    pub const fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for SecretOwnerId {
    fn default() -> Self {
        Self::new()
    }
}

/// A bounded HTTP header name. Header values are represented by
/// [`SecretValue`], never by this type.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeaderName(String);

impl HeaderName {
    pub fn new(value: impl Into<String>) -> Result<Self, HeaderNameError> {
        let value = value.into();
        if value.is_empty() || value.len() > MAX_HEADER_NAME_BYTES {
            return Err(HeaderNameError::InvalidLength);
        }
        if !value.bytes().all(is_http_token_byte) {
            return Err(HeaderNameError::InvalidCharacter);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for HeaderName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("HeaderName").field(&self.0).finish()
    }
}

impl fmt::Display for HeaderName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Serialize for HeaderName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for HeaderName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

fn is_http_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum HeaderNameError {
    #[error("header name has an invalid length")]
    InvalidLength,
    #[error("header name contains an invalid character")]
    InvalidCharacter,
}

/// The supported secret ownership/purpose vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SecretPurpose {
    ProviderApiKey {
        owner: SecretOwnerId,
    },
    ProviderSecretHeader {
        owner: SecretOwnerId,
        name: HeaderName,
    },
    AudioApiKey {
        owner: SecretOwnerId,
    },
    HuggingFaceAccessToken,
    CivitaiAccessToken,
    HostApiBearerToken,
}

/// Presence is deliberately separate from backend availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecretState {
    Present,
    Missing,
    Unavailable { reason: SecretAvailability },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecretAvailability {
    BackendUnavailable,
    UserAuthRequired,
    KeyInvalidated,
    AccessDenied,
    Corrupt,
}

/// Metadata only; plaintext never appears in this record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretRecord {
    pub reference: SecretRef,
    pub purpose: SecretPurpose,
}

impl SecretRecord {
    #[must_use]
    pub fn new(reference: SecretRef, purpose: SecretPurpose) -> Self {
        Self { reference, purpose }
    }
}

/// A temporary zeroizing value. It is not `Clone`, `Serialize`, or directly
/// extractable; callers borrow it only inside a closure while constructing a
/// provider request.
pub struct SecretValue(Zeroizing<String>);

impl SecretValue {
    #[must_use = "handle the validation error or use the secret value"]
    pub fn new(value: impl Into<String>) -> Result<Self, SecretValueError> {
        let value = value.into();
        if value.is_empty() {
            return Err(SecretValueError::Empty);
        }
        if value.len() > MAX_SECRET_VALUE_BYTES {
            return Err(SecretValueError::TooLarge);
        }
        Ok(Self(Zeroizing::new(value)))
    }

    pub fn with<T>(&self, use_value: impl FnOnce(&str) -> T) -> T {
        use_value(self.0.as_str())
    }

    fn into_inner(self) -> Zeroizing<String> {
        self.0
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

impl fmt::Display for SecretValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SecretValueError {
    #[error("secret value is empty")]
    Empty,
    #[error("secret value exceeds the allowed size")]
    TooLarge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SecretBackendError {
    #[error("secret backend is unavailable")]
    Unavailable,
    #[error("secret backend access was denied")]
    AccessDenied,
    #[error("secret backend data is corrupt")]
    Corrupt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SecretStoreError {
    #[error("secret purpose does not match the reference")]
    PurposeMismatch,
    #[error("secret update has a stale generation")]
    StaleGeneration,
    #[error("secret reference is missing")]
    Missing,
    #[error("secret is unavailable: {0:?}")]
    Unavailable(SecretAvailability),
    #[error("secret backend failed: {0}")]
    Backend(SecretBackendError),
    #[error("secret generation overflowed")]
    GenerationOverflow,
}

/// Read/status metadata returned by the local store. It contains no value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretStatus {
    pub reference: SecretRef,
    pub purpose: SecretPurpose,
    pub generation: u64,
    pub state: SecretState,
}

impl fmt::Display for SecretStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

/// The only production-facing boundary in this slice. It is object-safe so
/// applications can inject Android, desktop, or test implementations later.
#[async_trait]
pub trait SecretStore: Send + Sync {
    async fn put(
        &self,
        record: SecretRecord,
        value: SecretValue,
        expected_generation: Option<u64>,
    ) -> Result<SecretStatus, SecretStoreError>;

    async fn load(
        &self,
        reference: &SecretRef,
        purpose: &SecretPurpose,
    ) -> Result<SecretValue, SecretStoreError>;

    async fn status(
        &self,
        reference: &SecretRef,
        purpose: &SecretPurpose,
    ) -> Result<SecretStatus, SecretStoreError>;

    /// Deletion is idempotent: an absent reference returns `Missing`, even
    /// when an expected generation was supplied. Live entries still validate
    /// purpose and reject a stale expected generation.
    async fn delete(
        &self,
        reference: &SecretRef,
        purpose: &SecretPurpose,
        expected_generation: Option<u64>,
    ) -> Result<SecretStatus, SecretStoreError>;
}

struct StoredSecret {
    record: SecretRecord,
    generation: u64,
    value: Zeroizing<String>,
    unavailable: Option<SecretAvailability>,
}

impl Drop for StoredSecret {
    fn drop(&mut self) {
        self.value.zeroize();
    }
}

/// Deterministic in-memory reference adapter for tests only. It has no
/// encryption, platform protection, persistence, or production guarantees.
#[derive(Clone, Default)]
pub struct InMemorySecretStore {
    entries: Arc<Mutex<BTreeMap<SecretRef, StoredSecret>>>,
}

impl fmt::Debug for InMemorySecretStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InMemorySecretStore")
            .field("entry_count", &self.entry_count())
            .finish()
    }
}

impl InMemorySecretStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn entry_count(&self) -> usize {
        self.entries.lock().map_or(0, |entries| entries.len())
    }

    /// Test-only availability injection; it never carries a backend string.
    pub fn set_unavailable(
        &self,
        reference: &SecretRef,
        reason: Option<SecretAvailability>,
    ) -> Result<(), SecretStoreError> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| SecretStoreError::Backend(SecretBackendError::Corrupt))?;
        entries
            .get_mut(reference)
            .ok_or(SecretStoreError::Missing)?
            .unavailable = reason;
        Ok(())
    }

    fn status_of(entry: &StoredSecret) -> SecretStatus {
        SecretStatus {
            reference: entry.record.reference,
            purpose: entry.record.purpose.clone(),
            generation: entry.generation,
            state: entry.unavailable.map_or(SecretState::Present, |reason| {
                SecretState::Unavailable { reason }
            }),
        }
    }

    fn ensure_purpose(
        entry: &StoredSecret,
        purpose: &SecretPurpose,
    ) -> Result<(), SecretStoreError> {
        if &entry.record.purpose == purpose {
            Ok(())
        } else {
            Err(SecretStoreError::PurposeMismatch)
        }
    }

    fn next_generation(generation: u64) -> Result<u64, SecretStoreError> {
        generation
            .checked_add(1)
            .ok_or(SecretStoreError::GenerationOverflow)
    }
}

#[async_trait]
impl SecretStore for InMemorySecretStore {
    async fn put(
        &self,
        record: SecretRecord,
        value: SecretValue,
        expected_generation: Option<u64>,
    ) -> Result<SecretStatus, SecretStoreError> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| SecretStoreError::Backend(SecretBackendError::Corrupt))?;
        let generation = if let Some(existing) = entries.get(&record.reference) {
            Self::ensure_purpose(existing, &record.purpose)?;
            if expected_generation != Some(existing.generation) {
                return Err(SecretStoreError::StaleGeneration);
            }
            Self::next_generation(existing.generation)?
        } else {
            if expected_generation.is_some() {
                return Err(SecretStoreError::StaleGeneration);
            }
            1
        };

        let reference = record.reference;
        let entry = StoredSecret {
            record,
            generation,
            value: value.into_inner(),
            unavailable: None,
        };
        let status = Self::status_of(&entry);
        entries.insert(reference, entry);
        Ok(status)
    }

    async fn load(
        &self,
        reference: &SecretRef,
        purpose: &SecretPurpose,
    ) -> Result<SecretValue, SecretStoreError> {
        let entries = self
            .entries
            .lock()
            .map_err(|_| SecretStoreError::Backend(SecretBackendError::Corrupt))?;
        let entry = entries.get(reference).ok_or(SecretStoreError::Missing)?;
        Self::ensure_purpose(entry, purpose)?;
        if let Some(reason) = entry.unavailable {
            return Err(SecretStoreError::Unavailable(reason));
        }
        SecretValue::new(entry.value.as_str())
            .map_err(|_| SecretStoreError::Backend(SecretBackendError::Corrupt))
    }

    async fn status(
        &self,
        reference: &SecretRef,
        purpose: &SecretPurpose,
    ) -> Result<SecretStatus, SecretStoreError> {
        let entries = self
            .entries
            .lock()
            .map_err(|_| SecretStoreError::Backend(SecretBackendError::Corrupt))?;
        let Some(entry) = entries.get(reference) else {
            return Ok(SecretStatus {
                reference: *reference,
                purpose: purpose.clone(),
                generation: 0,
                state: SecretState::Missing,
            });
        };
        Self::ensure_purpose(entry, purpose)?;
        Ok(Self::status_of(entry))
    }

    async fn delete(
        &self,
        reference: &SecretRef,
        purpose: &SecretPurpose,
        expected_generation: Option<u64>,
    ) -> Result<SecretStatus, SecretStoreError> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| SecretStoreError::Backend(SecretBackendError::Corrupt))?;
        let Some(entry) = entries.get(reference) else {
            return Ok(SecretStatus {
                reference: *reference,
                purpose: purpose.clone(),
                generation: 0,
                state: SecretState::Missing,
            });
        };
        Self::ensure_purpose(entry, purpose)?;
        if expected_generation.is_some_and(|generation| generation != entry.generation) {
            return Err(SecretStoreError::StaleGeneration);
        }
        let generation = entry.generation;
        entries.remove(reference);
        Ok(SecretStatus {
            reference: *reference,
            purpose: purpose.clone(),
            generation,
            state: SecretState::Missing,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use serde_json::to_string;

    use super::*;

    fn provider_purpose(owner: SecretOwnerId) -> SecretPurpose {
        SecretPurpose::ProviderApiKey { owner }
    }

    fn record(reference: SecretRef, owner: SecretOwnerId) -> SecretRecord {
        SecretRecord::new(reference, provider_purpose(owner))
    }

    #[tokio::test]
    async fn set_rotate_load_status_and_delete_are_generation_scoped() {
        let store = InMemorySecretStore::new();
        let reference = SecretRef::new();
        let owner = SecretOwnerId::new();
        let first = store
            .put(
                record(reference, owner),
                SecretValue::new("first-canary").expect("fixture value is valid"),
                None,
            )
            .await
            .expect("create should succeed");
        assert_eq!(first.generation, 1);
        assert_eq!(first.state, SecretState::Present);

        let value = store
            .load(&reference, &provider_purpose(owner))
            .await
            .expect("load should succeed");
        assert_eq!(value.with(str::to_owned), "first-canary");

        let rotated = store
            .put(
                record(reference, owner),
                SecretValue::new("second-canary").expect("fixture value is valid"),
                Some(1),
            )
            .await
            .expect("rotation should succeed");
        assert_eq!(rotated.generation, 2);
        assert_eq!(
            store
                .status(&reference, &provider_purpose(owner))
                .await
                .expect("status should exist")
                .state,
            SecretState::Present
        );

        let deleted = store
            .delete(&reference, &provider_purpose(owner), Some(2))
            .await
            .expect("delete should succeed");
        assert_eq!(deleted.state, SecretState::Missing);
        assert_eq!(store.entry_count(), 0);
        assert_eq!(
            store
                .delete(&reference, &provider_purpose(owner), Some(2))
                .await
                .expect("repeated cleanup delete is idempotent")
                .state,
            SecretState::Missing
        );
    }

    #[tokio::test]
    async fn missing_unavailable_and_stale_updates_are_typed() {
        let store = InMemorySecretStore::new();
        let reference = SecretRef::new();
        let owner = SecretOwnerId::new();
        let purpose = provider_purpose(owner);
        assert_eq!(
            store
                .status(&reference, &purpose)
                .await
                .expect("unknown status is representable")
                .state,
            SecretState::Missing
        );
        store
            .put(
                record(reference, owner),
                SecretValue::new("value").expect("fixture value is valid"),
                None,
            )
            .await
            .expect("create should succeed");
        store
            .set_unavailable(&reference, Some(SecretAvailability::BackendUnavailable))
            .expect("test availability mutation should work");
        assert_eq!(
            store
                .status(&reference, &purpose)
                .await
                .expect("status")
                .state,
            SecretState::Unavailable {
                reason: SecretAvailability::BackendUnavailable
            }
        );
        assert_eq!(
            store
                .load(&reference, &purpose)
                .await
                .expect_err("unavailable"),
            SecretStoreError::Unavailable(SecretAvailability::BackendUnavailable)
        );
        store
            .set_unavailable(&reference, None)
            .expect("availability reset should work");
        assert_eq!(
            store
                .put(
                    record(reference, owner),
                    SecretValue::new("stale").expect("fixture value is valid"),
                    Some(0),
                )
                .await
                .expect_err("generation gap is stale"),
            SecretStoreError::StaleGeneration
        );
    }

    #[tokio::test]
    async fn purpose_mismatch_and_redaction_canaries_hold() {
        let store = InMemorySecretStore::new();
        let reference = SecretRef::new();
        let owner = SecretOwnerId::new();
        let canary = "plaintext-canary";
        let value = SecretValue::new(canary).expect("fixture value is valid");
        assert_eq!(format!("{value:?}"), "[REDACTED]");
        assert_eq!(value.to_string(), "[REDACTED]");
        assert!(!format!("{value:?}").contains(canary));
        assert!(!value.to_string().contains(canary));
        store
            .put(record(reference, owner), value, None)
            .await
            .expect("create should succeed");

        let other = SecretPurpose::HuggingFaceAccessToken;
        assert_eq!(
            store
                .load(&reference, &other)
                .await
                .expect_err("wrong purpose"),
            SecretStoreError::PurposeMismatch
        );
        assert_eq!(
            store
                .status(&reference, &other)
                .await
                .expect_err("status purpose must be checked"),
            SecretStoreError::PurposeMismatch
        );
        let status = store
            .status(&reference, &provider_purpose(owner))
            .await
            .expect("status");
        assert!(
            !to_string(&status)
                .expect("status serializes")
                .contains(canary)
        );
        assert!(
            !to_string(&record(reference, owner))
                .expect("record serializes")
                .contains(canary)
        );
        let mut error = String::new();
        write!(&mut error, "{}", SecretStoreError::Missing).expect("error formats");
        assert!(!error.contains(canary));
    }

    #[test]
    fn bounded_header_and_non_serializable_value_contracts() {
        assert!(HeaderName::new("Authorization").is_ok());
        assert!(HeaderName::new("bad header").is_err());
        assert!(HeaderName::new("x".repeat(MAX_HEADER_NAME_BYTES + 1)).is_err());
        assert!(matches!(SecretValue::new(""), Err(SecretValueError::Empty)));
        assert!(matches!(
            SecretValue::new("x".repeat(MAX_SECRET_VALUE_BYTES + 1)),
            Err(SecretValueError::TooLarge)
        ));
        let error = SecretValueError::TooLarge;
        assert!(!error.to_string().contains("plaintext-canary"));
        let uuid = Uuid::new_v4();
        assert_eq!(SecretRef::from_uuid(uuid).as_uuid(), uuid);
        assert_eq!(SecretOwnerId::from_uuid(uuid).as_uuid(), uuid);
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SecretValue>();
        // SecretValue deliberately has no Clone or Serialize implementation.
    }
}
