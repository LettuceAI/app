use async_trait::async_trait;
use lettuce_jobs::handle::CancellationToken;
use lettuce_settings::{SecretRecord, SecretRef, SecretState, SecretStore, SecretValue};
use lettuce_sync::{
    MAX_SECRET_VERSION_AHEAD_MILLIS, StoredSecretVersion, SyncDeviceId, SyncSecretEntry,
    SyncSecretError, SyncSecretRepository, SyncSecretVersion,
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

    /// Stops serving secrets, whether the phase finished or failed.
    fn stop_serving_secrets(&mut self);
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
/// native secret store under the generation seen when the phase began (a key
/// changed meanwhile is kept); a local value changed since the last session
/// gets a new version stamped with the time the store wrote it (the session
/// time when the store does not record it), never earlier than a version it
/// had, so the later rotation wins on every device. A value this device lost is
/// fetched again rather than deleted elsewhere, and a value nothing here
/// references any more (its provider was deleted) is removed from the store;
/// one a provider row still mentions, even unreadably, is kept.
/// A secret the store cannot read or write is skipped and retried next
/// session.
#[derive(Debug)]
pub struct SyncSecretCoordinator<'a, R: ?Sized, S: ?Sized> {
    repository: &'a R,
    secrets: &'a S,
}

struct LocalSecret {
    entry: SyncSecretEntry,
    generation: Option<u64>,
    writable: bool,
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
        let result = self.exchange(transport, device, cancellation, now).await;
        transport.stop_serving_secrets();
        result
    }

    async fn exchange<T>(
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
        for record in &records {
            local.push(self.local_secret(record, device, now).await?);
        }
        let peer = transport
            .exchange_secret_inventory(
                local.iter().map(|secret| secret.entry.clone()).collect(),
                cancellation,
            )
            .await
            .map_err(SyncSecretExchangeError::Transport)?;
        let mut report = SecretSyncReport {
            received: 0,
            skipped: 0,
        };
        let newest_allowed = now.get().saturating_add(MAX_SECRET_VERSION_AHEAD_MILLIS);
        for secret in &local {
            check_cancelled(cancellation)?;
            let entry = &secret.entry;
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
            if !secret.writable || offered.set_at.get() > newest_allowed {
                report.skipped += 1;
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
            let Ok(status) = self.secrets.put(record, value, secret.generation).await else {
                report.skipped += 1;
                continue;
            };
            self.repository
                .record_secret_version(
                    &entry.reference,
                    &StoredSecretVersion {
                        purpose: entry.purpose.clone(),
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
        self.forget_unreferenced(&records).await?;
        Ok(report)
    }

    async fn local_secret(
        &self,
        record: &SecretRecord,
        device: SyncDeviceId,
        now: TimestampMillis,
    ) -> Result<LocalSecret, SyncSecretExchangeError> {
        let status = self
            .secrets
            .status(&record.reference, &record.purpose)
            .await
            .ok();
        let stored = self
            .repository
            .secret_version(&record.reference)
            .map_err(SyncSecretExchangeError::Repository)?;
        let (version, generation, writable) = match status {
            Some(status) if status.state == SecretState::Present => {
                let version = match stored {
                    Some(stored)
                        if stored.generation == status.generation
                            && stored.purpose == record.purpose =>
                    {
                        stored.version
                    }
                    earlier => {
                        let written = status
                            .set_at
                            .map_or(now.get(), |set_at| set_at.get().min(now.get()));
                        let version = SyncSecretVersion {
                            set_at: TimestampMillis::new(earlier.map_or(written, |stored| {
                                written.max(stored.version.set_at.get().saturating_add(1))
                            })),
                            device,
                        };
                        self.repository
                            .record_secret_version(
                                &record.reference,
                                &StoredSecretVersion {
                                    purpose: record.purpose.clone(),
                                    generation: status.generation,
                                    version,
                                },
                            )
                            .map_err(SyncSecretExchangeError::Repository)?;
                        version
                    }
                };
                (Some(version), Some(status.generation), true)
            }
            Some(status) if status.state == SecretState::Missing => {
                if stored.is_some() {
                    self.repository
                        .forget_secret_version(&record.reference)
                        .map_err(SyncSecretExchangeError::Repository)?;
                }
                (None, None, true)
            }
            _ => (None, None, false),
        };
        Ok(LocalSecret {
            entry: SyncSecretEntry {
                reference: record.reference,
                purpose: record.purpose.clone(),
                version,
            },
            generation,
            writable,
        })
    }

    async fn forget_unreferenced(
        &self,
        records: &[SecretRecord],
    ) -> Result<(), SyncSecretExchangeError> {
        let recorded = self
            .repository
            .recorded_secret_versions()
            .map_err(SyncSecretExchangeError::Repository)?;
        for (reference, stored) in recorded {
            if records.iter().any(|record| record.reference == reference)
                || self
                    .repository
                    .secret_mentioned(&reference)
                    .map_err(SyncSecretExchangeError::Repository)?
            {
                continue;
            }
            if self
                .secrets
                .delete(&reference, &stored.purpose, Some(stored.generation))
                .await
                .is_ok()
            {
                self.repository
                    .forget_secret_version(&reference)
                    .map_err(SyncSecretExchangeError::Repository)?;
            }
        }
        Ok(())
    }
}

fn check_cancelled(cancellation: &CancellationToken) -> Result<(), SyncSecretExchangeError> {
    if cancellation.is_cancelled() {
        Err(SyncSecretExchangeError::Cancelled)
    } else {
        Ok(())
    }
}
