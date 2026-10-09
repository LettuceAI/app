use crate::Database;
use lettuce_models::ModelSettingsLayer;
use lettuce_settings::{DeviceSettings, GlobalSettingsStoreError, StoredGlobalSettings};
use lettuce_types::TimestampMillis;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

type Snapshot = (
    StoredGlobalSettings,
    ModelSettingsLayer,
    DeviceSettings,
    lettuce_types::Revision,
);

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
        connection
            .query_row(
                "SELECT revision FROM device_settings WHERE id=1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|_| GlobalSettingsStoreError::Storage)?
            .map(crate::to_revision)
            .transpose()
            .map_err(|_| GlobalSettingsStoreError::InvalidData)?
            .unwrap_or(lettuce_types::Revision::INITIAL),
    ))
}

fn filter_state_ordered(
    generation: impl FnOnce() -> u64,
    snapshot: impl FnOnce() -> Result<(bool, lettuce_settings::PureMode), GlobalSettingsStoreError>,
) -> Result<(u64, bool, lettuce_settings::PureMode), GlobalSettingsStoreError> {
    let generation = generation();
    let (enabled, mode) = snapshot()?;
    Ok((generation, enabled, mode))
}

impl Database {
    pub fn settings_filter_state(
        &self,
    ) -> Result<(u64, bool, lettuce_settings::PureMode), GlobalSettingsStoreError> {
        let mut connection = self
            .connection()
            .map_err(|_| GlobalSettingsStoreError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(|_| GlobalSettingsStoreError::Storage)?;
        let result = filter_state_ordered(
            || self.changes.settings_generation(),
            || {
                let (stored, _, _, _) = snapshot(&transaction)?;
                Ok((
                    stored.settings.developer_mode_enabled,
                    stored.settings.pure_mode,
                ))
            },
        )?;
        transaction
            .commit()
            .map_err(|_| GlobalSettingsStoreError::Storage)?;
        Ok(result)
    }

    pub fn save_device_embedding(
        &self,
        embedding: lettuce_settings::DeviceEmbeddingSettings,
        expected: lettuce_types::Revision,
    ) -> Result<Snapshot, GlobalSettingsStoreError> {
        let mut connection = self
            .connection()
            .map_err(|_| GlobalSettingsStoreError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| GlobalSettingsStoreError::Storage)?;
        let (_, _, mut device, revision) = snapshot(&transaction)?;
        if revision != expected {
            return Err(GlobalSettingsStoreError::StaleRevision);
        }
        device.embedding = embedding;
        device.validate()?;
        self.changes.settings_section("device");
        crate::replace_device_settings_in(&transaction, &device)
            .map_err(|_| GlobalSettingsStoreError::Storage)?;
        let result = snapshot(&transaction)?;
        transaction
            .commit()
            .map_err(|_| GlobalSettingsStoreError::Storage)?;
        Ok(result)
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
        section: &'static str,
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
        let next = stored
            .revision
            .next()
            .map_err(|_| GlobalSettingsStoreError::Storage)?;
        let payload = serde_json::to_string(&stored.settings)
            .map_err(|_| GlobalSettingsStoreError::InvalidData)?;
        let layer = crate::encode_global_model_settings(&sampler)
            .map_err(|_| GlobalSettingsStoreError::InvalidData)?;
        self.changes.settings_section(section);
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
    fn device_embedding_cas_rollback_and_retry_preserve_global_and_other_device_fields() {
        use lettuce_settings::DeviceSettingsStore;
        let database = Database::open_in_memory().expect("database");
        let before = database.settings_snapshot().expect("snapshot");
        let embedding = lettuce_settings::DeviceEmbeddingSettings {
            max_tokens: Some(1024),
            ..Default::default()
        };
        database.connection().expect("connection").execute_batch("CREATE TRIGGER fail_settings_update BEFORE INSERT ON device_settings BEGIN SELECT RAISE(ABORT, 'injected crash'); END;").expect("trigger");
        assert!(database.save_device_embedding(embedding, before.3).is_err());
        assert_eq!(database.settings_snapshot().expect("unchanged"), before);
        database
            .connection()
            .expect("connection")
            .execute_batch("DROP TRIGGER fail_settings_update;")
            .expect("remove fault");
        database
            .update_device_settings(&|device| {
                device.llm_models_dir = Some("/tmp/settings-models".into())
            })
            .expect("concurrent edit");
        assert_eq!(
            database.save_device_embedding(embedding, before.3),
            Err(GlobalSettingsStoreError::StaleRevision)
        );
        let current = database.settings_snapshot().expect("snapshot");
        let after = database
            .save_device_embedding(embedding, current.3)
            .expect("retry");
        assert_eq!(after.0, before.0);
        assert_eq!(after.1, before.1);
        assert_eq!(after.2.llm_models_dir, current.2.llm_models_dir);
        assert_eq!(after.2.embedding, embedding);
        assert_eq!(
            database.save_device_embedding(embedding, current.3),
            Err(GlobalSettingsStoreError::StaleRevision)
        );
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
    #[test]
    fn filter_generation_is_captured_before_the_snapshot_interleaving() {
        let generation = std::cell::Cell::new(1);
        let state = filter_state_ordered(
            || generation.get(),
            || {
                generation.set(2);
                Ok((false, lettuce_settings::PureMode::Standard))
            },
        )
        .expect("snapshot");
        assert_eq!(state.0, 1);
    }

    #[test]
    fn settings_section_feed_has_no_rollback_event() {
        let database = Database::open_in_memory().expect("database");
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let signal = seen.clone();
        database.on_settings_section_change(move |section| {
            signal.lock().expect("events").push(section)
        });
        database
            .connection()
            .expect("connection")
            .execute_batch("BEGIN; UPDATE app_settings SET revision=revision+1; ROLLBACK;")
            .expect("rollback");
        assert!(seen.lock().expect("events").is_empty());
        database
            .connection()
            .expect("connection")
            .execute_batch("UPDATE app_settings SET revision=revision+1;")
            .expect("commit");
        assert_eq!(*seen.lock().expect("events"), vec!["general"]);
    }
    #[test]
    fn device_record_section_cannot_be_overridden_by_writer_labels() {
        use lettuce_settings::DeviceSettingsStore;
        let database = Database::open_in_memory().expect("database");
        let sections = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let observed = sections.clone();
        database.on_settings_section_change(move |section| {
            observed.lock().expect("sections").push(section)
        });
        for label in ["device_embedding", "certificates", "models"] {
            database.changes.settings_section(label);
            database
                .update_device_settings(&|device| {
                    device.llm_models_dir = Some(format!("/tmp/{label}"));
                })
                .expect("device commit");
        }
        assert_eq!(*sections.lock().expect("sections"), vec!["device"; 3]);
    }
}
