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
                "SELECT api_key_secret_ref, secret_owner_id, secret_headers_json, config_json FROM provider_accounts ORDER BY id",
            )
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| {
                        Ok((
                            row.get::<_, Option<String>>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                        ))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(storage)?;
        for (api_key, owner, headers, config) in accounts {
            let Ok(owner) = uuid(&owner).map(SecretOwnerId::from_uuid) else {
                tracing::warn!("a provider account's secret owner cannot be read");
                continue;
            };
            if let Some(Ok(reference)) = api_key.as_deref().map(uuid) {
                records.push(SecretRecord::new(
                    SecretRef::from_uuid(reference),
                    SecretPurpose::ProviderApiKey { owner },
                ));
            }
            let headers = serde_json::from_str::<Vec<lettuce_models::SecretHeader>>(&headers)
                .unwrap_or_else(|_| {
                    tracing::warn!("a provider account's secret headers cannot be read");
                    Vec::new()
                });
            for header in headers {
                records.push(SecretRecord::new(
                    header.secret_ref,
                    SecretPurpose::ProviderSecretHeader {
                        owner,
                        name: header.name,
                    },
                ));
            }
            if let Ok(lettuce_models::ProviderConfig::Ollama(lettuce_models::OllamaConfig {
                sprout:
                    Some(lettuce_models::SproutConfig {
                        api_key_ref: Some(reference),
                        ..
                    }),
            })) = crate::decode_provider_config(&config)
            {
                records.push(SecretRecord::new(
                    reference,
                    SecretPurpose::SproutApiKey { owner },
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
                tracing::warn!("an audio provider's secret reference cannot be read");
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

    fn secret_mentioned(&self, reference: &SecretRef) -> Result<bool, SyncSecretError> {
        let connection = self.connection().map_err(storage)?;
        connection
            .query_row(
                "SELECT EXISTS(
                   SELECT 1 FROM provider_accounts
                   WHERE instr(lower(coalesce(api_key_secret_ref, '') || ' ' ||
                                     secret_headers_json || ' ' || config_json), ?1) > 0
                 ) OR EXISTS(
                   SELECT 1 FROM audio_providers
                   WHERE instr(lower(coalesce(api_key_secret_ref, '')), ?1) > 0
                 )",
                [reference.to_string().to_lowercase()],
                |row| row.get(0),
            )
            .map_err(storage)
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

#[cfg(test)]
mod tests {
    use lettuce_settings::{SecretOwnerId, SecretRef};
    use lettuce_speech::{AudioProvider, AudioProviderConfig, TtsConfigurationRepository};
    use lettuce_sync::SyncSecretRepository;
    use lettuce_types::{AudioProviderId, Revision, TimestampMillis};

    use crate::Database;

    #[test]
    fn a_reference_that_cannot_be_read_is_still_mentioned() {
        let database = Database::open_in_memory().expect("database");
        let reference = SecretRef::new();
        let provider = AudioProvider {
            id: AudioProviderId::new(),
            secret_owner_id: SecretOwnerId::new(),
            label: "ElevenLabs".to_owned(),
            api_key_ref: Some(reference),
            config: AudioProviderConfig::Elevenlabs,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        };
        database
            .upsert_audio_provider(provider.clone(), None)
            .expect("provider");
        assert!(database.secret_mentioned(&reference).expect("mentioned"));
        database
            .connection()
            .expect("connection")
            .execute_batch(&format!(
                "DROP TRIGGER audio_providers_stable_identity;
                 UPDATE audio_providers SET secret_owner_id = '{}', revision = revision + 1
                 WHERE id = '{}';",
                "z".repeat(36),
                provider.id
            ))
            .expect("an owner this build cannot read");

        assert!(
            database
                .referenced_secrets()
                .expect("referenced")
                .is_empty()
        );
        assert!(database.secret_mentioned(&reference).expect("mentioned"));
        assert!(
            !database
                .secret_mentioned(&SecretRef::new())
                .expect("unrelated")
        );
    }
}
