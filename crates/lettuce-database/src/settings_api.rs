use crate::Database;
use lettuce_models::ModelSettingsLayer;
use lettuce_settings::{DeviceSettings, GlobalSettingsStoreError, StoredGlobalSettings};
use lettuce_types::TimestampMillis;
use rusqlite::{Connection, TransactionBehavior, params};

type Snapshot = (StoredGlobalSettings, ModelSettingsLayer, DeviceSettings);

fn snapshot(connection: &Connection) -> Result<Snapshot, GlobalSettingsStoreError> {
    let app = crate::sync_load_app_settings(connection).map_err(|error| match error {
        rusqlite::Error::InvalidQuery => GlobalSettingsStoreError::InvalidData,
        _ => GlobalSettingsStoreError::Storage,
    })?;
    if !app.settings.within_bounds() {
        return Err(GlobalSettingsStoreError::InvalidData);
    }
    Ok((
        StoredGlobalSettings {
            settings: app.settings,
            default_model_profile_id: app.default_model_profile_id,
            default_prompt_document_id: app.default_prompt_document_id,
            dynamic_memory_model_profile_id: app.dynamic_memory_model_profile_id,
            group_speaker_model_profile_id: app.group_speaker_model_profile_id,
            revision: app.revision,
            created_at: app.created_at,
            updated_at: app.updated_at,
        },
        app.model_settings,
        crate::read_device_settings(connection)?,
    ))
}

impl Database {
    pub fn settings_filter_state(
        &self,
    ) -> Result<(u64, bool, lettuce_settings::PureMode), GlobalSettingsStoreError> {
        let connection = self
            .connection()
            .map_err(|_| GlobalSettingsStoreError::Storage)?;
        let (stored, _, _) = snapshot(&connection)?;
        Ok((
            self.changes.settings_generation(),
            stored.settings.developer_mode_enabled,
            stored.settings.pure_mode,
        ))
    }

    pub fn settings_snapshot(&self) -> Result<Snapshot, GlobalSettingsStoreError> {
        let mut connection = self
            .connection()
            .map_err(|_| GlobalSettingsStoreError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(|_| GlobalSettingsStoreError::Storage)?;
        let result = snapshot(&transaction)?;
        transaction
            .commit()
            .map_err(|_| GlobalSettingsStoreError::Storage)?;
        Ok(result)
    }

    pub fn save_settings_snapshot(
        &self,
        stored: StoredGlobalSettings,
        sampler: ModelSettingsLayer,
        device_embedding: Option<lettuce_settings::DeviceEmbeddingSettings>,
        at: TimestampMillis,
    ) -> Result<Snapshot, GlobalSettingsStoreError> {
        if !stored.settings.within_bounds() || sampler.validate().is_err() {
            return Err(GlobalSettingsStoreError::InvalidData);
        }
        let mut connection = self
            .connection()
            .map_err(|_| GlobalSettingsStoreError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| GlobalSettingsStoreError::Storage)?;
        if snapshot(&transaction)?.0.revision != stored.revision {
            return Err(GlobalSettingsStoreError::StaleRevision);
        }
        for id in stored
            .settings
            .selected_model_profiles()
            .into_iter()
            .flatten()
            .chain(
                [
                    stored.default_model_profile_id,
                    stored.dynamic_memory_model_profile_id,
                    stored.group_speaker_model_profile_id,
                ]
                .into_iter()
                .flatten(),
            )
        {
            let exists: bool = transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM model_profiles WHERE id=?1)",
                    [id.to_string()],
                    |row| row.get(0),
                )
                .map_err(|_| GlobalSettingsStoreError::Storage)?;
            if !exists {
                return Err(GlobalSettingsStoreError::ModelProfileMissing);
            }
        }
        let prompts = std::cell::RefCell::new(Vec::new());
        stored.settings.clone().clear_prompts(|id| {
            prompts.borrow_mut().push(id);
            false
        });
        let mut prompts = prompts.into_inner();
        prompts.extend(stored.default_prompt_document_id);
        for id in prompts {
            let exists: bool = transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM prompt_documents WHERE id=?1 AND status='active')",
                    [id.to_string()],
                    |row| row.get(0),
                )
                .map_err(|_| GlobalSettingsStoreError::Storage)?;
            if !exists {
                return Err(GlobalSettingsStoreError::InvalidData);
            }
        }
        if let Some(embedding) = device_embedding {
            let mut device = crate::read_device_settings(&transaction)?;
            device.embedding = embedding;
            device.validate()?;
            crate::replace_device_settings_in(&transaction, &device)
                .map_err(|_| GlobalSettingsStoreError::Storage)?;
        }
        let next = stored
            .revision
            .next()
            .map_err(|_| GlobalSettingsStoreError::Storage)?;
        let payload = serde_json::to_string(&stored.settings)
            .map_err(|_| GlobalSettingsStoreError::InvalidData)?;
        let layer = crate::encode_global_model_settings(&sampler)
            .map_err(|_| GlobalSettingsStoreError::InvalidData)?;
        transaction.execute("UPDATE app_settings SET payload_json=?1,model_settings_json=?2,default_model_profile_id=?3,default_prompt_document_id=?4,dynamic_memory_model_profile_id=?5,group_speaker_model_profile_id=?6,revision=?7,updated_at=?8 WHERE id=1 AND revision=?9", params![payload,layer,stored.default_model_profile_id.map(|id| id.to_string()),stored.default_prompt_document_id.map(|id| id.to_string()),stored.dynamic_memory_model_profile_id.map(|id| id.to_string()),stored.group_speaker_model_profile_id.map(|id| id.to_string()),crate::to_i64(next.get()).map_err(|_| GlobalSettingsStoreError::Storage)?,at.get(),crate::to_i64(stored.revision.get()).map_err(|_| GlobalSettingsStoreError::Storage)?]).map_err(|error| match error {
            rusqlite::Error::SqliteFailure(code, _) if code.code == rusqlite::ErrorCode::ConstraintViolation => GlobalSettingsStoreError::InvalidData,
            _ => GlobalSettingsStoreError::Storage,
        })?;
        let result = snapshot(&transaction)?;
        transaction
            .commit()
            .map_err(|_| GlobalSettingsStoreError::Storage)?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_device_write_rolls_back_when_global_write_aborts() {
        let database = Database::open_in_memory().expect("database");
        let (stored, sampler, _) = database.settings_snapshot().expect("settings");
        database.connection().expect("connection").execute_batch("CREATE TRIGGER fail_settings_update BEFORE UPDATE ON app_settings BEGIN SELECT RAISE(ABORT, 'injected crash'); END;").expect("trigger");
        let result = database.save_settings_snapshot(
            stored.clone(),
            sampler.clone(),
            Some(lettuce_settings::DeviceEmbeddingSettings {
                max_tokens: Some(1024),
                ..Default::default()
            }),
            TimestampMillis::new(2),
        );
        assert!(result.is_err());
        let (after, after_sampler, device) = database.settings_snapshot().expect("settings");
        assert_eq!(after, stored);
        assert_eq!(after_sampler, sampler);
        assert_eq!(device, DeviceSettings::default());
    }
    #[test]
    fn settings_embedding_preserves_device_changes_after_snapshot_read() {
        use lettuce_settings::DeviceSettingsStore;
        let database = Database::open_in_memory().expect("database");
        let (stored, sampler, _) = database.settings_snapshot().expect("snapshot");
        database
            .update_device_settings(&|device| {
                device.llm_models_dir = Some("/tmp/settings-models".to_owned());
                device
                    .trusted_certificates
                    .push(lettuce_settings::TrustedCertificate {
                        id: uuid::Uuid::new_v4(),
                        name: "root.pem".to_owned(),
                        pem: "-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----"
                            .to_owned(),
                        imported_at: 1,
                    });
            })
            .expect("concurrent device change");
        let before = database.load_device_settings().expect("device");
        let (_, _, after) = database
            .save_settings_snapshot(
                stored,
                sampler,
                Some(lettuce_settings::DeviceEmbeddingSettings {
                    max_tokens: Some(1024),
                    ..Default::default()
                }),
                TimestampMillis::new(2),
            )
            .expect("settings update");
        assert_eq!(after.llm_models_dir, before.llm_models_dir);
        assert_eq!(after.trusted_certificates, before.trusted_certificates);
        assert_eq!(after.embedding.max_tokens, Some(1024));
    }
    #[test]
    fn logging_generation_follows_commits_even_when_sync_lowers_revision() {
        use lettuce_settings::GlobalSettingsStore;
        let database = Database::open_in_memory().expect("database");
        let seen = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let signal = seen.clone();
        database.on_settings_change(move || {
            signal.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
        let mut stored = database.load().expect("settings");
        stored.settings.developer_mode_enabled = true;
        database
            .save(
                stored.settings,
                stored.default_model_profile_id,
                stored.revision,
            )
            .expect("enable");
        let enabled = database.settings_filter_state().expect("state");
        let connection = database.connection().expect("connection");
        connection
            .execute_batch("BEGIN; UPDATE app_settings SET revision=1; ROLLBACK;")
            .expect("rollback");
        assert_eq!(seen.load(std::sync::atomic::Ordering::SeqCst), 1);
        connection
            .execute(
                "UPDATE app_settings SET revision=1,payload_json=?1",
                [
                    serde_json::to_string(&lettuce_settings::GlobalSettings::default())
                        .expect("payload"),
                ],
            )
            .expect("remote winner");
        drop(connection);
        let disabled = database.settings_filter_state().expect("state");
        assert!(enabled.1);
        assert!(!disabled.1);
        assert!(disabled.0 > enabled.0);
        assert_eq!(seen.load(std::sync::atomic::Ordering::SeqCst), 2);
    }
}
