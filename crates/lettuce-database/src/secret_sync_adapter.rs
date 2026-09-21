//! Secret references and versions for the sync secret phase.

use lettuce_settings::{SecretOwnerId, SecretPurpose, SecretRecord, SecretRef};
use lettuce_sync::{
    StoredSecretVersion, SyncDeviceId, SyncSecretError, SyncSecretRepository, SyncSecretVersion,
};
use lettuce_types::TimestampMillis;
use rusqlite::{OptionalExtension, params};

use crate::Database;

fn storage(_: impl std::fmt::Debug) -> SyncSecretError {
    SyncSecretError::Storage
}

fn corrupt(_: impl std::fmt::Debug) -> SyncSecretError {
    SyncSecretError::Corrupt
}

fn uuid(value: &str) -> Result<uuid::Uuid, SyncSecretError> {
    uuid::Uuid::parse_str(value).map_err(corrupt)
}

impl SyncSecretRepository for Database {
    fn referenced_secrets(&self) -> Result<Vec<SecretRecord>, SyncSecretError> {
        let connection = self.connection().map_err(storage)?;
        let mut records = Vec::new();
        let accounts = connection
            .prepare(
                "SELECT api_key_secret_ref, secret_owner_id, secret_headers_json FROM provider_accounts ORDER BY id",
            )
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| {
                        Ok((
                            row.get::<_, Option<String>>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(storage)?;
        for (api_key, owner, headers) in accounts {
            let Ok(owner) = uuid(&owner).map(SecretOwnerId::from_uuid) else {
                continue;
            };
            if let Some(Ok(reference)) = api_key.as_deref().map(uuid) {
                records.push(SecretRecord::new(
                    SecretRef::from_uuid(reference),
                    SecretPurpose::ProviderApiKey { owner },
                ));
            }
            for header in serde_json::from_str::<Vec<lettuce_models::SecretHeader>>(&headers)
                .unwrap_or_default()
            {
                records.push(SecretRecord::new(
                    header.secret_ref,
                    SecretPurpose::ProviderSecretHeader {
                        owner,
                        name: header.name,
                    },
                ));
            }
        }
        let audio = connection
            .prepare(
                "SELECT api_key_secret_ref, secret_owner_id FROM audio_providers WHERE api_key_secret_ref IS NOT NULL ORDER BY id",
            )
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(storage)?;
        for (reference, owner) in audio {
            let (Ok(reference), Ok(owner)) = (uuid(&reference), uuid(&owner)) else {
                continue;
            };
            records.push(SecretRecord::new(
                SecretRef::from_uuid(reference),
                SecretPurpose::AudioApiKey {
                    owner: SecretOwnerId::from_uuid(owner),
                },
            ));
        }
        Ok(records)
    }

    fn secret_version(
        &self,
        reference: &SecretRef,
    ) -> Result<Option<StoredSecretVersion>, SyncSecretError> {
        let connection = self.connection().map_err(storage)?;
        connection
            .query_row(
                "SELECT purpose_json, generation, set_at, device_id FROM sync_secret_versions WHERE reference = ?1",
                [reference.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(storage)?
            .map(stored_version)
            .transpose()
    }

    fn record_secret_version(
        &self,
        reference: &SecretRef,
        stored: &StoredSecretVersion,
    ) -> Result<(), SyncSecretError> {
        let connection = self.connection().map_err(storage)?;
        connection
            .execute(
                "INSERT INTO sync_secret_versions (reference, purpose_json, generation, set_at, device_id) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(reference) DO UPDATE SET purpose_json = excluded.purpose_json, generation = excluded.generation, set_at = excluded.set_at, device_id = excluded.device_id",
                params![
                    reference.to_string(),
                    serde_json::to_string(&stored.purpose).map_err(corrupt)?,
                    i64::try_from(stored.generation).map_err(corrupt)?,
                    stored.version.set_at.get(),
                    stored.version.device.as_uuid().to_string(),
                ],
            )
            .map_err(storage)?;
        Ok(())
    }

    fn recorded_secret_versions(
        &self,
    ) -> Result<Vec<(SecretRef, StoredSecretVersion)>, SyncSecretError> {
        let connection = self.connection().map_err(storage)?;
        let rows = connection
            .prepare(
                "SELECT reference, purpose_json, generation, set_at, device_id FROM sync_secret_versions ORDER BY reference",
            )
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            (
                                row.get::<_, String>(1)?,
                                row.get::<_, i64>(2)?,
                                row.get::<_, i64>(3)?,
                                row.get::<_, String>(4)?,
                            ),
                        ))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(storage)?;
        let mut recorded = Vec::with_capacity(rows.len());
        for (reference, row) in rows {
            let (Ok(reference), Ok(stored)) = (uuid(&reference), stored_version(row)) else {
                continue;
            };
            recorded.push((SecretRef::from_uuid(reference), stored));
        }
        Ok(recorded)
    }

    fn forget_secret_version(&self, reference: &SecretRef) -> Result<(), SyncSecretError> {
        self.connection()
            .map_err(storage)?
            .execute(
                "DELETE FROM sync_secret_versions WHERE reference = ?1",
                [reference.to_string()],
            )
            .map_err(storage)?;
        Ok(())
    }
}

fn stored_version(
    (purpose, generation, set_at, device): (String, i64, i64, String),
) -> Result<StoredSecretVersion, SyncSecretError> {
    Ok(StoredSecretVersion {
        purpose: serde_json::from_str(&purpose).map_err(corrupt)?,
        generation: u64::try_from(generation).map_err(corrupt)?,
        version: SyncSecretVersion {
            set_at: TimestampMillis::new(set_at),
            device: SyncDeviceId::from_uuid(uuid(&device)?),
        },
    })
}
