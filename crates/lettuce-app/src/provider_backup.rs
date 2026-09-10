use std::fmt;

use lettuce_settings::{SecretState, SecretStore};
use lettuce_transfer::{
    BackupEnvelopeError, ProviderBackupGraphError, ProviderBackupSecret, ProviderBackupSource,
    ProviderBackupSourceError, provider_backup_secret_requirements, provider_backup_sections,
    seal_backup,
};
use lettuce_types::TimestampMillis;

pub struct ProviderBackupCoordinator<'a, R: ?Sized, S: ?Sized> {
    source: &'a R,
    secrets: &'a S,
}

impl<R: ?Sized, S: ?Sized> fmt::Debug for ProviderBackupCoordinator<'_, R, S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderBackupCoordinator")
            .finish_non_exhaustive()
    }
}

impl<'a, R, S> ProviderBackupCoordinator<'a, R, S>
where
    R: ProviderBackupSource + ?Sized,
    S: SecretStore + ?Sized,
{
    #[must_use]
    pub const fn new(source: &'a R, secrets: &'a S) -> Self {
        Self { source, secrets }
    }

    pub async fn export(
        &self,
        app_version: impl Into<String>,
        created_at: TimestampMillis,
        password: &str,
    ) -> Result<Vec<u8>, ProviderBackupError> {
        let graph = self.source.read_provider_backup_graph()?;
        let requirements = provider_backup_secret_requirements(&graph)?;
        let mut values = Vec::with_capacity(requirements.len());
        for (reference, purpose) in requirements {
            let before = self.secrets.status(&reference, &purpose).await?;
            if before.reference != reference
                || before.purpose != purpose
                || before.state != SecretState::Present
                || before.generation == 0
            {
                return Err(ProviderBackupError::SecretChanged);
            }
            let value = self.secrets.load(&reference, &purpose).await?;
            let after = self.secrets.status(&reference, &purpose).await?;
            if after != before {
                return Err(ProviderBackupError::SecretChanged);
            }
            values.push(ProviderBackupSecret {
                reference,
                purpose,
                generation: before.generation,
                value,
            });
        }
        let sections = provider_backup_sections(graph, values)?;
        seal_backup(app_version, created_at, password, sections).map_err(Into::into)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProviderBackupError {
    #[error("provider backup source failed: {0}")]
    Source(#[from] ProviderBackupSourceError),
    #[error("provider backup graph failed validation: {0}")]
    Graph(#[from] ProviderBackupGraphError),
    #[error("provider backup secret could not be read: {0}")]
    Secret(#[from] lettuce_settings::SecretStoreError),
    #[error("provider backup secret changed while it was being read")]
    SecretChanged,
    #[error("provider backup encryption failed: {0}")]
    Envelope(#[from] BackupEnvelopeError),
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use lettuce_models::{
        ProviderAccount, ProviderAccountRepository, ProviderConfig, ProviderProtocol,
    };
    use lettuce_settings::{
        InMemorySecretStore, SecretBackendError, SecretOwnerId, SecretPurpose, SecretRecord,
        SecretRef, SecretStatus, SecretStore, SecretStoreError, SecretValue,
    };
    use lettuce_transfer::{BackupEnvelopeError, open_backup};
    use lettuce_types::{OperationId, ProviderAccountId, Revision};

    use super::*;
    use crate::AppBackend;

    struct ChangingSecretStore {
        reference: SecretRef,
        purpose: SecretPurpose,
        status_calls: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl SecretStore for ChangingSecretStore {
        async fn put(
            &self,
            _: SecretRecord,
            _: SecretValue,
            _: Option<u64>,
        ) -> Result<SecretStatus, SecretStoreError> {
            Err(SecretStoreError::Backend(SecretBackendError::Unavailable))
        }

        async fn load(
            &self,
            reference: &SecretRef,
            purpose: &SecretPurpose,
        ) -> Result<SecretValue, SecretStoreError> {
            if *reference != self.reference || *purpose != self.purpose {
                return Err(SecretStoreError::PurposeMismatch);
            }
            SecretValue::new("changing-secret")
                .map_err(|_| SecretStoreError::Backend(SecretBackendError::Corrupt))
        }

        async fn status(
            &self,
            reference: &SecretRef,
            purpose: &SecretPurpose,
        ) -> Result<SecretStatus, SecretStoreError> {
            if *reference != self.reference || *purpose != self.purpose {
                return Err(SecretStoreError::PurposeMismatch);
            }
            Ok(SecretStatus {
                reference: self.reference,
                purpose: self.purpose.clone(),
                generation: if self.status_calls.fetch_add(1, Ordering::SeqCst) == 0 {
                    1
                } else {
                    2
                },
                state: SecretState::Present,
            })
        }

        async fn delete(
            &self,
            _: &SecretRef,
            _: &SecretPurpose,
            _: Option<u64>,
        ) -> Result<SecretStatus, SecretStoreError> {
            Err(SecretStoreError::Backend(SecretBackendError::Unavailable))
        }
    }

    #[tokio::test]
    async fn file_backed_provider_backup_reopens_and_seals_only_referenced_secrets() {
        let path =
            std::env::temp_dir().join(format!("provider-backup-{}.sqlite3", OperationId::new()));
        let secret_store = Arc::new(InMemorySecretStore::new());
        let reference = SecretRef::new();
        let owner = SecretOwnerId::new();
        let purpose = SecretPurpose::ProviderApiKey { owner };
        let backend = AppBackend::open(&path, TimestampMillis::new(1)).expect("open backend");
        ProviderAccountRepository::upsert(
            backend.database(),
            ProviderAccount {
                id: ProviderAccountId::new(),
                secret_owner_id: owner,
                provider_kind: "openai".into(),
                protocol: ProviderProtocol::OpenAiCompatible,
                label: "Primary".into(),
                endpoint: None,
                enabled: true,
                streaming_enabled: true,
                allow_invalid_tls: false,
                api_key_ref: Some(reference),
                secret_headers: Vec::new(),
                config: ProviderConfig::Standard,
                revision: Revision::new(1),
                created_at: TimestampMillis::new(2),
                updated_at: TimestampMillis::new(2),
            },
            None,
        )
        .expect("store provider");
        secret_store
            .put(
                SecretRecord::new(reference, purpose),
                SecretValue::new("provider-backup-canary").expect("secret"),
                None,
            )
            .await
            .expect("store secret");
        drop(backend);

        let reopened = AppBackend::open(&path, TimestampMillis::new(3)).expect("reopen backend");
        let envelope = ProviderBackupCoordinator::new(reopened.database(), secret_store.as_ref())
            .export("1.0.0", TimestampMillis::new(4), "backup password")
            .await
            .expect("export backup");
        assert_eq!(
            open_backup(&envelope, "wrong password"),
            Err(BackupEnvelopeError::Authentication)
        );
        let sections = open_backup(&envelope, "backup password").expect("open backup");
        assert_eq!(sections.len(), 2);
        assert!(
            sections[1]
                .bytes
                .windows("provider-backup-canary".len())
                .any(|window| window == b"provider-backup-canary")
        );
        assert!(
            !sections[0]
                .bytes
                .windows("provider-backup-canary".len())
                .any(|window| window == b"provider-backup-canary")
        );

        let changing = ChangingSecretStore {
            reference,
            purpose: SecretPurpose::ProviderApiKey { owner },
            status_calls: AtomicUsize::new(0),
        };
        assert_eq!(
            reopened
                .provider_backup(&changing)
                .export("1.0.0", TimestampMillis::new(5), "backup password")
                .await,
            Err(ProviderBackupError::SecretChanged)
        );
    }
}
