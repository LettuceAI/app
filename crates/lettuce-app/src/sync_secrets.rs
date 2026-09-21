use async_trait::async_trait;
use lettuce_jobs::handle::CancellationToken;
use lettuce_settings::{SecretRecord, SecretRef, SecretState, SecretStore, SecretValue};
use lettuce_sync::{
    MAX_SYNC_SECRETS, StoredSecretVersion, SyncDeviceId, SyncSecretEntry, SyncSecretError,
    SyncSecretRepository, SyncSecretVersion,
};
use lettuce_types::TimestampMillis;

#[async_trait]
pub trait AuthenticatedSecretSyncTransport: Send {
    /// Trades inventories; the local entries that hold a value are the ones
    /// this side serves while the phase lasts.
    async fn exchange_secret_inventory(
        &mut self,
        local: Vec<SyncSecretEntry>,
        cancellation: &CancellationToken,
    ) -> Result<Vec<SyncSecretEntry>, SecretSyncTransportError>;

    /// The peer's value for a secret it listed, or `None` when it can no
    /// longer read it.
    async fn fetch_secret(
        &mut self,
        reference: &SecretRef,
        cancellation: &CancellationToken,
    ) -> Result<Option<SecretValue>, SecretSyncTransportError>;

    /// Ends the secret phase, serving the peer's requests until it ends too.
    async fn finish_secrets(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), SecretSyncTransportError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SecretSyncTransportError {
    #[error("secret sync transport was cancelled")]
    Cancelled,
    #[error("secret sync peer disconnected")]
    Disconnected,
    #[error("secret sync transport protocol failed")]
    Protocol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SyncSecretExchangeError {
    #[error("secret sync state failed: {0}")]
    Repository(SyncSecretError),
    #[error("secret sync transport failed: {0}")]
    Transport(SecretSyncTransportError),
    #[error("secret sync was cancelled")]
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecretSyncReport {
    pub received: usize,
    pub skipped: usize,
}

/// Runs the secret phase of a sync session. A secret this device lacks, or
/// holds an older version of, is fetched from the peer and written to the
/// native secret store; a local value changed since the last session gets a
/// new version first. A secret the store cannot read or write here is skipped
/// and retried next session.
#[derive(Debug)]
pub struct SyncSecretCoordinator<'a, R: ?Sized, S: ?Sized> {
    repository: &'a R,
    secrets: &'a S,
}

impl<'a, R: ?Sized, S: ?Sized> SyncSecretCoordinator<'a, R, S> {
    #[must_use]
    pub const fn new(repository: &'a R, secrets: &'a S) -> Self {
        Self {
            repository,
            secrets,
        }
    }
}

impl<R, S> SyncSecretCoordinator<'_, R, S>
where
    R: SyncSecretRepository + ?Sized,
    S: SecretStore + ?Sized,
{
    pub async fn run<T>(
        &self,
        transport: &mut T,
        device: SyncDeviceId,
        cancellation: &CancellationToken,
        now: TimestampMillis,
    ) -> Result<SecretSyncReport, SyncSecretExchangeError>
    where
        T: AuthenticatedSecretSyncTransport + ?Sized,
    {
        check_cancelled(cancellation)?;
        let records = self
            .repository
            .referenced_secrets()
            .map_err(SyncSecretExchangeError::Repository)?;
        let mut local = Vec::with_capacity(records.len());
        for record in records.iter().take(MAX_SYNC_SECRETS) {
            local.push(self.local_entry(record, device, now).await?);
        }
        let peer = transport
            .exchange_secret_inventory(local.clone(), cancellation)
            .await
            .map_err(SyncSecretExchangeError::Transport)?;
        let mut report = SecretSyncReport {
            received: 0,
            skipped: 0,
        };
        for entry in &local {
            check_cancelled(cancellation)?;
            let Some(offered) = peer
                .iter()
                .find(|candidate| {
                    candidate.reference == entry.reference && candidate.purpose == entry.purpose
                })
                .and_then(|candidate| candidate.version)
            else {
                continue;
            };
            if entry.version.is_some_and(|held| held >= offered) {
                continue;
            }
            let Some(value) = transport
                .fetch_secret(&entry.reference, cancellation)
                .await
                .map_err(SyncSecretExchangeError::Transport)?
            else {
                report.skipped += 1;
                continue;
            };
            let record = SecretRecord::new(entry.reference, entry.purpose.clone());
            let expected = match self.secrets.status(&entry.reference, &entry.purpose).await {
                Ok(status) if status.state == SecretState::Present => Some(status.generation),
                Ok(_) => None,
                Err(_) => {
                    report.skipped += 1;
                    continue;
                }
            };
            let Ok(status) = self.secrets.put(record, value, expected).await else {
                report.skipped += 1;
                continue;
            };
            self.repository
                .record_secret_version(
                    &entry.reference,
                    StoredSecretVersion {
                        generation: status.generation,
                        version: offered,
                    },
                )
                .map_err(SyncSecretExchangeError::Repository)?;
            report.received += 1;
        }
        transport
            .finish_secrets(cancellation)
            .await
            .map_err(SyncSecretExchangeError::Transport)?;
        Ok(report)
    }

    async fn local_entry(
        &self,
        record: &SecretRecord,
        device: SyncDeviceId,
        now: TimestampMillis,
    ) -> Result<SyncSecretEntry, SyncSecretExchangeError> {
        let status = self
            .secrets
            .status(&record.reference, &record.purpose)
            .await
            .ok()
            .filter(|status| status.state == SecretState::Present);
        let version = match status {
            None => None,
            Some(status) => {
                let stored = self
                    .repository
                    .secret_version(&record.reference)
                    .map_err(SyncSecretExchangeError::Repository)?;
                match stored {
                    Some(stored) if stored.generation == status.generation => Some(stored.version),
                    _ => {
                        let version = SyncSecretVersion {
                            set_at: now,
                            device,
                        };
                        self.repository
                            .record_secret_version(
                                &record.reference,
                                StoredSecretVersion {
                                    generation: status.generation,
                                    version,
                                },
                            )
                            .map_err(SyncSecretExchangeError::Repository)?;
                        Some(version)
                    }
                }
            }
        };
        Ok(SyncSecretEntry {
            reference: record.reference,
            purpose: record.purpose.clone(),
            version,
        })
    }
}

fn check_cancelled(cancellation: &CancellationToken) -> Result<(), SyncSecretExchangeError> {
    if cancellation.is_cancelled() {
        Err(SyncSecretExchangeError::Cancelled)
    } else {
        Ok(())
    }
}
