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
            let owner = SecretOwnerId::from_uuid(uuid(&owner)?);
            if let Some(reference) = api_key {
                records.push(SecretRecord::new(
                    SecretRef::from_uuid(uuid(&reference)?),
                    SecretPurpose::ProviderApiKey { owner },
                ));
            }
            for header in serde_json::from_str::<Vec<lettuce_models::SecretHeader>>(&headers)
                .map_err(corrupt)?
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
            records.push(SecretRecord::new(
                SecretRef::from_uuid(uuid(&reference)?),
                SecretPurpose::AudioApiKey {
                    owner: SecretOwnerId::from_uuid(uuid(&owner)?),
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
                "SELECT generation, set_at, device_id FROM sync_secret_versions WHERE reference = ?1",
                [reference.to_string()],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(storage)?
            .map(|(generation, set_at, device)| {
                Ok(StoredSecretVersion {
                    generation: u64::try_from(generation).map_err(corrupt)?,
                    version: SyncSecretVersion {
                        set_at: TimestampMillis::new(set_at),
                        device: SyncDeviceId::from_uuid(uuid(&device)?),
                    },
                })
            })
            .transpose()
    }

    fn record_secret_version(
        &self,
        reference: &SecretRef,
        stored: StoredSecretVersion,
    ) -> Result<(), SyncSecretError> {
        let connection = self.connection().map_err(storage)?;
        connection
            .execute(
                "INSERT INTO sync_secret_versions (reference, generation, set_at, device_id) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(reference) DO UPDATE SET generation = excluded.generation, set_at = excluded.set_at, device_id = excluded.device_id",
                params![
                    reference.to_string(),
                    i64::try_from(stored.generation).map_err(corrupt)?,
                    stored.version.set_at.get(),
                    stored.version.device.as_uuid().to_string(),
                ],
            )
            .map_err(storage)?;
        Ok(())
    }
}
