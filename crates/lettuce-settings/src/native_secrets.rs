use std::{
    fmt,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

use crate::{
    SecretAvailability, SecretBackendError, SecretPurpose, SecretRecord, SecretRef, SecretState,
    SecretStatus, SecretStore, SecretStoreError, SecretValue,
};

const SERVICE_NAME: &str = "com.lettuceai.app.secrets.v1";
const ENVELOPE_VERSION: u32 = 1;
const MAX_ENVELOPE_BYTES: usize = 128 * 1024;

#[derive(Clone)]
pub struct NativeSecretStore {
    backend: Arc<dyn CredentialBackend>,
    mutation_lock: Arc<Mutex<()>>,
}

impl fmt::Debug for NativeSecretStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativeSecretStore")
            .finish_non_exhaustive()
    }
}

#[cfg(not(target_os = "android"))]
impl Default for NativeSecretStore {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeSecretStore {
    #[cfg(not(target_os = "android"))]
    #[must_use]
    pub fn new() -> Self {
        Self {
            backend: Arc::new(KeyringBackend),
            mutation_lock: Arc::new(Mutex::new(())),
        }
    }

    #[cfg(not(target_os = "android"))]
    pub fn try_new() -> Result<Self, SecretStoreError> {
        Ok(Self::new())
    }

    #[cfg(target_os = "android")]
    pub fn try_new() -> Result<Self, SecretStoreError> {
        Ok(Self {
            backend: Arc::new(AndroidKeyringBackend::try_new().map_err(backend_error)?),
            mutation_lock: Arc::new(Mutex::new(())),
        })
    }

    #[cfg(test)]
    fn with_backend(backend: Arc<dyn CredentialBackend>) -> Self {
        Self {
            backend,
            mutation_lock: Arc::new(Mutex::new(())),
        }
    }

    fn load_envelope(
        &self,
        reference: &SecretRef,
    ) -> Result<Option<SecretEnvelope>, SecretStoreError> {
        let Some(bytes) = self
            .backend
            .load(&reference.to_string())
            .map_err(backend_error)?
        else {
            return Ok(None);
        };
        if bytes.len() > MAX_ENVELOPE_BYTES {
            return Err(SecretStoreError::Backend(SecretBackendError::Corrupt));
        }
        let envelope: SecretEnvelope = serde_json::from_slice(&bytes)
            .map_err(|_| SecretStoreError::Backend(SecretBackendError::Corrupt))?;
        if envelope.version != ENVELOPE_VERSION
            || envelope.generation == 0
            || SecretValue::new(envelope.value.as_str()).is_err()
        {
            return Err(SecretStoreError::Backend(SecretBackendError::Corrupt));
        }
        Ok(Some(envelope))
    }

    fn missing_status(reference: SecretRef, purpose: SecretPurpose) -> SecretStatus {
        SecretStatus {
            reference,
            purpose,
            generation: 0,
            state: SecretState::Missing,
        }
    }

    fn status_of(
        reference: SecretRef,
        purpose: SecretPurpose,
        generation: u64,
        state: SecretState,
    ) -> SecretStatus {
        SecretStatus {
            reference,
            purpose,
            generation,
            state,
        }
    }
}

#[async_trait]
impl SecretStore for NativeSecretStore {
    async fn put(
        &self,
        record: SecretRecord,
        value: SecretValue,
        expected_generation: Option<u64>,
    ) -> Result<SecretStatus, SecretStoreError> {
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| SecretStoreError::Backend(SecretBackendError::Corrupt))?;
        let generation = match self.load_envelope(&record.reference)? {
            Some(existing) => {
                if existing.purpose != record.purpose {
                    return Err(SecretStoreError::PurposeMismatch);
                }
                if expected_generation != Some(existing.generation) {
                    return Err(SecretStoreError::StaleGeneration);
                }
                existing
                    .generation
                    .checked_add(1)
                    .ok_or(SecretStoreError::GenerationOverflow)?
            }
            None => {
                if expected_generation.is_some() {
                    return Err(SecretStoreError::StaleGeneration);
                }
                1
            }
        };
        let encoded = value
            .with(|secret| {
                serde_json::to_vec(&SecretEnvelopeRef {
                    version: ENVELOPE_VERSION,
                    purpose: &record.purpose,
                    generation,
                    value: secret,
                })
            })
            .map(Zeroizing::new)
            .map_err(|_| SecretStoreError::Backend(SecretBackendError::Corrupt))?;
        if encoded.len() > MAX_ENVELOPE_BYTES {
            return Err(SecretStoreError::Backend(SecretBackendError::Corrupt));
        }
        self.backend
            .store(&record.reference.to_string(), &encoded)
            .map_err(backend_error)?;
        Ok(Self::status_of(
            record.reference,
            record.purpose,
            generation,
            SecretState::Present,
        ))
    }

    async fn load(
        &self,
        reference: &SecretRef,
        purpose: &SecretPurpose,
    ) -> Result<SecretValue, SecretStoreError> {
        let envelope = self
            .load_envelope(reference)?
            .ok_or(SecretStoreError::Missing)?;
        if &envelope.purpose != purpose {
            return Err(SecretStoreError::PurposeMismatch);
        }
        SecretValue::new(envelope.value.as_str())
            .map_err(|_| SecretStoreError::Backend(SecretBackendError::Corrupt))
    }

    async fn status(
        &self,
        reference: &SecretRef,
        purpose: &SecretPurpose,
    ) -> Result<SecretStatus, SecretStoreError> {
        let Some(envelope) = self.load_envelope(reference)? else {
            return Ok(Self::missing_status(*reference, purpose.clone()));
        };
        if &envelope.purpose != purpose {
            return Err(SecretStoreError::PurposeMismatch);
        }
        Ok(Self::status_of(
            *reference,
            envelope.purpose.clone(),
            envelope.generation,
            SecretState::Present,
        ))
    }

    async fn delete(
        &self,
        reference: &SecretRef,
        purpose: &SecretPurpose,
        expected_generation: Option<u64>,
    ) -> Result<SecretStatus, SecretStoreError> {
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| SecretStoreError::Backend(SecretBackendError::Corrupt))?;
        let Some(envelope) = self.load_envelope(reference)? else {
            return Ok(Self::missing_status(*reference, purpose.clone()));
        };
        if &envelope.purpose != purpose {
            return Err(SecretStoreError::PurposeMismatch);
        }
        if expected_generation.is_some_and(|expected| expected != envelope.generation) {
            return Err(SecretStoreError::StaleGeneration);
        }
        self.backend
            .delete(&reference.to_string())
            .map_err(backend_error)?;
        Ok(Self::status_of(
            *reference,
            envelope.purpose.clone(),
            envelope.generation,
            SecretState::Missing,
        ))
    }
}

#[derive(Serialize)]
struct SecretEnvelopeRef<'a> {
    version: u32,
    purpose: &'a SecretPurpose,
    generation: u64,
    value: &'a str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SecretEnvelope {
    version: u32,
    purpose: SecretPurpose,
    generation: u64,
    value: String,
}

impl Drop for SecretEnvelope {
    fn drop(&mut self) {
        self.value.zeroize();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CredentialBackendError {
    Unavailable,
    AccessDenied,
    Corrupt,
}

trait CredentialBackend: Send + Sync {
    fn load(&self, key: &str) -> Result<Option<Zeroizing<Vec<u8>>>, CredentialBackendError>;
    fn store(&self, key: &str, value: &[u8]) -> Result<(), CredentialBackendError>;
    fn delete(&self, key: &str) -> Result<(), CredentialBackendError>;
}

#[cfg(not(target_os = "android"))]
struct KeyringBackend;

#[cfg(not(target_os = "android"))]
impl CredentialBackend for KeyringBackend {
    fn load(&self, key: &str) -> Result<Option<Zeroizing<Vec<u8>>>, CredentialBackendError> {
        let entry = keyring::Entry::new(SERVICE_NAME, key).map_err(keyring_error)?;
        match entry.get_secret() {
            Ok(value) => Ok(Some(Zeroizing::new(value))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(keyring_error(error)),
        }
    }

    fn store(&self, key: &str, value: &[u8]) -> Result<(), CredentialBackendError> {
        keyring::Entry::new(SERVICE_NAME, key)
            .map_err(keyring_error)?
            .set_secret(value)
            .map_err(keyring_error)
    }

    fn delete(&self, key: &str) -> Result<(), CredentialBackendError> {
        let entry = keyring::Entry::new(SERVICE_NAME, key).map_err(keyring_error)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(keyring_error(error)),
        }
    }
}

#[cfg(not(target_os = "android"))]
fn keyring_error(error: keyring::Error) -> CredentialBackendError {
    match error {
        keyring::Error::NoStorageAccess(_) => CredentialBackendError::AccessDenied,
        keyring::Error::PlatformFailure(_) => CredentialBackendError::Unavailable,
        keyring::Error::NoEntry
        | keyring::Error::BadEncoding(_)
        | keyring::Error::TooLong(_, _)
        | keyring::Error::Invalid(_, _)
        | keyring::Error::Ambiguous(_)
        | _ => CredentialBackendError::Corrupt,
    }
}

#[cfg(target_os = "android")]
struct AndroidKeyringBackend {
    store: Arc<android_native_keyring_store::Store>,
}

#[cfg(target_os = "android")]
impl AndroidKeyringBackend {
    fn try_new() -> Result<Self, CredentialBackendError> {
        android_native_keyring_store::Store::new()
            .map(|store| Self { store })
            .map_err(android_keyring_error)
    }

    fn entry(&self, key: &str) -> Result<keyring_core::Entry, CredentialBackendError> {
        use keyring_core::api::CredentialStoreApi;

        self.store
            .build(SERVICE_NAME, key, None)
            .map_err(android_keyring_error)
    }
}

#[cfg(target_os = "android")]
impl CredentialBackend for AndroidKeyringBackend {
    fn load(&self, key: &str) -> Result<Option<Zeroizing<Vec<u8>>>, CredentialBackendError> {
        match self.entry(key)?.get_secret() {
            Ok(value) => Ok(Some(Zeroizing::new(value))),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(error) => Err(android_keyring_error(error)),
        }
    }

    fn store(&self, key: &str, value: &[u8]) -> Result<(), CredentialBackendError> {
        self.entry(key)?
            .set_secret(value)
            .map_err(android_keyring_error)
    }

    fn delete(&self, key: &str) -> Result<(), CredentialBackendError> {
        match self.entry(key)?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(error) => Err(android_keyring_error(error)),
        }
    }
}

#[cfg(target_os = "android")]
fn android_keyring_error(error: keyring_core::Error) -> CredentialBackendError {
    match error {
        keyring_core::Error::NoStorageAccess(_) => CredentialBackendError::AccessDenied,
        keyring_core::Error::PlatformFailure(_) | keyring_core::Error::NoDefaultStore => {
            CredentialBackendError::Unavailable
        }
        keyring_core::Error::NoEntry
        | keyring_core::Error::BadEncoding(_)
        | keyring_core::Error::BadDataFormat(_, _)
        | keyring_core::Error::BadStoreFormat(_)
        | keyring_core::Error::TooLong(_, _)
        | keyring_core::Error::Invalid(_, _)
        | keyring_core::Error::Ambiguous(_)
        | keyring_core::Error::NotSupportedByStore(_)
        | _ => CredentialBackendError::Corrupt,
    }
}

fn backend_error(error: CredentialBackendError) -> SecretStoreError {
    match error {
        CredentialBackendError::Unavailable => {
            SecretStoreError::Unavailable(SecretAvailability::BackendUnavailable)
        }
        CredentialBackendError::AccessDenied => {
            SecretStoreError::Backend(SecretBackendError::AccessDenied)
        }
        CredentialBackendError::Corrupt => SecretStoreError::Backend(SecretBackendError::Corrupt),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::{SecretOwnerId, SecretValue};

    #[derive(Default)]
    struct TestBackend {
        entries: Mutex<BTreeMap<String, Vec<u8>>>,
        failure: Mutex<Option<CredentialBackendError>>,
    }

    impl TestBackend {
        fn inject(&self, reference: SecretRef, bytes: Vec<u8>) {
            self.entries
                .lock()
                .expect("test entries lock")
                .insert(reference.to_string(), bytes);
        }

        fn fail_once(&self, error: CredentialBackendError) {
            *self.failure.lock().expect("test failure lock") = Some(error);
        }

        fn take_failure(&self) -> Result<(), CredentialBackendError> {
            match self.failure.lock().expect("test failure lock").take() {
                Some(error) => Err(error),
                None => Ok(()),
            }
        }
    }

    impl CredentialBackend for TestBackend {
        fn load(&self, key: &str) -> Result<Option<Zeroizing<Vec<u8>>>, CredentialBackendError> {
            self.take_failure()?;
            Ok(self
                .entries
                .lock()
                .map_err(|_| CredentialBackendError::Corrupt)?
                .get(key)
                .cloned()
                .map(Zeroizing::new))
        }

        fn store(&self, key: &str, value: &[u8]) -> Result<(), CredentialBackendError> {
            self.take_failure()?;
            self.entries
                .lock()
                .map_err(|_| CredentialBackendError::Corrupt)?
                .insert(key.to_owned(), value.to_vec());
            Ok(())
        }

        fn delete(&self, key: &str) -> Result<(), CredentialBackendError> {
            self.take_failure()?;
            self.entries
                .lock()
                .map_err(|_| CredentialBackendError::Corrupt)?
                .remove(key);
            Ok(())
        }
    }

    fn purpose(owner: SecretOwnerId) -> SecretPurpose {
        SecretPurpose::ProviderApiKey { owner }
    }

    #[tokio::test]
    async fn native_store_preserves_generation_and_value_across_reopen() {
        let backend = Arc::new(TestBackend::default());
        let store = NativeSecretStore::with_backend(backend.clone());
        let reference = SecretRef::new();
        let owner = SecretOwnerId::new();
        let purpose = purpose(owner);
        let first = store
            .put(
                SecretRecord::new(reference, purpose.clone()),
                SecretValue::new("native-canary-one").expect("valid secret"),
                None,
            )
            .await
            .expect("store first generation");
        assert_eq!(first.generation, 1);
        assert_eq!(first.state, SecretState::Present);
        assert!(
            store
                .load(&reference, &purpose)
                .await
                .expect("load first generation")
                .with(|value| value == "native-canary-one")
        );

        let reopened = NativeSecretStore::with_backend(backend);
        assert_eq!(
            reopened
                .status(&reference, &purpose)
                .await
                .expect("status after reopen")
                .generation,
            1
        );
        assert_eq!(
            reopened
                .put(
                    SecretRecord::new(reference, purpose.clone()),
                    SecretValue::new("native-canary-stale").expect("valid secret"),
                    Some(2),
                )
                .await,
            Err(SecretStoreError::StaleGeneration)
        );
        let second = reopened
            .put(
                SecretRecord::new(reference, purpose.clone()),
                SecretValue::new("native-canary-two").expect("valid secret"),
                Some(1),
            )
            .await
            .expect("rotate secret");
        assert_eq!(second.generation, 2);
        assert!(
            reopened
                .load(&reference, &purpose)
                .await
                .expect("load rotated secret")
                .with(|value| value == "native-canary-two")
        );
        assert_eq!(
            reopened.delete(&reference, &purpose, Some(1)).await,
            Err(SecretStoreError::StaleGeneration)
        );
        let deleted = reopened
            .delete(&reference, &purpose, Some(2))
            .await
            .expect("delete current generation");
        assert_eq!(deleted.generation, 2);
        assert_eq!(deleted.state, SecretState::Missing);
        assert_eq!(
            reopened
                .delete(&reference, &purpose, Some(2))
                .await
                .expect("repeat missing delete")
                .generation,
            0
        );
    }

    #[tokio::test]
    async fn native_store_rejects_wrong_purpose_corruption_and_backend_failures() {
        let backend = Arc::new(TestBackend::default());
        let store = NativeSecretStore::with_backend(backend.clone());
        let reference = SecretRef::new();
        let owner = SecretOwnerId::new();
        let purpose = purpose(owner);
        store
            .put(
                SecretRecord::new(reference, purpose.clone()),
                SecretValue::new("redaction-canary").expect("valid secret"),
                None,
            )
            .await
            .expect("store secret");
        assert_eq!(
            store
                .status(
                    &reference,
                    &SecretPurpose::AudioApiKey {
                        owner: SecretOwnerId::new(),
                    },
                )
                .await,
            Err(SecretStoreError::PurposeMismatch)
        );
        assert!(!format!("{store:?}").contains("redaction-canary"));

        backend.inject(reference, b"not-an-envelope".to_vec());
        assert_eq!(
            store.status(&reference, &purpose).await,
            Err(SecretStoreError::Backend(SecretBackendError::Corrupt))
        );
        backend.fail_once(CredentialBackendError::AccessDenied);
        assert_eq!(
            store.status(&SecretRef::new(), &purpose).await,
            Err(SecretStoreError::Backend(SecretBackendError::AccessDenied))
        );
        backend.fail_once(CredentialBackendError::Unavailable);
        assert_eq!(
            store.status(&SecretRef::new(), &purpose).await,
            Err(SecretStoreError::Unavailable(
                SecretAvailability::BackendUnavailable
            ))
        );
    }

    #[tokio::test]
    async fn native_store_accepts_maximum_escaped_secret_value() {
        let backend = Arc::new(TestBackend::default());
        let store = NativeSecretStore::with_backend(backend);
        let reference = SecretRef::new();
        let purpose = purpose(SecretOwnerId::new());
        let canary = "\0".repeat(16 * 1024);

        store
            .put(
                SecretRecord::new(reference, purpose.clone()),
                SecretValue::new(canary.clone()).expect("maximum secret is valid"),
                None,
            )
            .await
            .expect("store escaped maximum secret");

        assert!(
            store
                .load(&reference, &purpose)
                .await
                .expect("load escaped maximum secret")
                .with(|value| value == canary)
        );
    }
}
