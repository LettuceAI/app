use std::path::Path;

use rusqlite::TransactionBehavior;

use crate::{Database, DatabaseError};

const DEVICE_LOCAL_TABLES: &[&str] = &[
    "sync_local_state",
    "sync_frontiers",
    "sync_changes",
    "sync_deferred_changes",
    "sync_change_frontiers",
    "sync_peer_frontiers",
    "sync_incoming_batches",
    "sync_incoming_changes",
    "sync_conflicts",
    "installed_whisper_models",
];

impl Database {
    /// Copies device-local state from the previous database file: the sync
    /// journal and installed Whisper model manifests; the local LoRA library,
    /// the local generation metrics, the app shell's install state, the device
    /// settings and each day of app usage unless the new file already has
    /// them (an imported legacy install's); and the discovered voices and
    /// llama.cpp runtime reports of audio providers and models that exist in
    /// this database.
    ///
    /// A LoRA path both files have keeps the more recently updated row, the
    /// rule the legacy import uses; a previous metrics row is skipped when the new file has one with its id
    /// or with the same `created_at`, `model_path` and `summary_json` (the
    /// same legacy generation imported again under an attempt id derived from
    /// a different source fingerprint).
    pub fn carry_device_local_state_from(&self, previous: &Path) -> Result<(), DatabaseError> {
        let previous = previous
            .to_str()
            .ok_or_else(|| rusqlite::Error::InvalidPath(previous.to_path_buf()))?;
        let mut connection = self.connection()?;
        connection.execute("ATTACH DATABASE ?1 AS previous", [previous])?;
        let copied = (|| {
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            for table in DEVICE_LOCAL_TABLES {
                transaction.execute(
                    &format!("INSERT INTO main.{table} SELECT * FROM previous.{table}"),
                    [],
                )?;
            }
            transaction.execute(
                "INSERT INTO main.image_loras (
                    path, filename, bytes_on_disk, modified_at, sha256, keywords,
                    keyword_source, architecture, architecture_source, created_at, updated_at
                 )
                 SELECT path, filename, bytes_on_disk, modified_at, sha256, keywords,
                        keyword_source, architecture, architecture_source, created_at, updated_at
                   FROM previous.image_loras WHERE true
                 ON CONFLICT(path) DO UPDATE SET
                    filename = excluded.filename,
                    bytes_on_disk = excluded.bytes_on_disk,
                    modified_at = excluded.modified_at,
                    sha256 = excluded.sha256,
                    keywords = excluded.keywords,
                    keyword_source = excluded.keyword_source,
                    architecture = excluded.architecture,
                    architecture_source = excluded.architecture_source,
                    created_at = excluded.created_at,
                    updated_at = excluded.updated_at
                 WHERE excluded.updated_at > image_loras.updated_at",
                [],
            )?;
            transaction.execute(
                "INSERT OR IGNORE INTO main.llm_generation_metrics
                 SELECT carried.* FROM previous.llm_generation_metrics carried
                 WHERE NOT EXISTS (
                    SELECT 1 FROM main.llm_generation_metrics held
                    WHERE held.created_at = carried.created_at
                      AND held.model_path IS carried.model_path
                      AND held.summary_json = carried.summary_json
                 )",
                [],
            )?;
            transaction.execute(
                "INSERT OR IGNORE INTO main.device_ui_state SELECT * FROM previous.device_ui_state",
                [],
            )?;
            transaction.execute(
                "INSERT OR IGNORE INTO main.device_settings SELECT * FROM previous.device_settings",
                [],
            )?;
            transaction.execute(
                "INSERT OR IGNORE INTO main.app_usage_days SELECT * FROM previous.app_usage_days",
                [],
            )?;
            transaction.execute(
                "INSERT INTO main.discovered_tts_voices SELECT * FROM previous.discovered_tts_voices WHERE provider_id IN (SELECT id FROM main.audio_providers)",
                [],
            )?;
            transaction.execute(
                "INSERT INTO main.llama_runtime_reports SELECT report.* FROM previous.llama_runtime_reports report JOIN main.model_profiles model ON model.id = report.model_profile_id AND model.external_model_id = report.model_path",
                [],
            )?;
            transaction.commit()
        })();
        connection.execute("DETACH DATABASE previous", [])?;
        copied.map_err(DatabaseError::from)
    }
}

#[cfg(test)]
mod tests {
    use lettuce_sync::LocalChangeJournal;
    use lettuce_types::{OperationId, TimestampMillis};

    use crate::Database;

    #[test]
    fn install_ui_state_is_carried_unless_the_new_file_has_its_own() {
        use lettuce_settings::DeviceUiStateStore;
        let root = std::env::temp_dir().join(format!("device-ui-{}", OperationId::new()));
        std::fs::create_dir_all(&root).expect("fixture root");
        let previous_path = root.join("previous.sqlite3");
        let previous = Database::open(&previous_path).expect("previous database");
        let state = |value: &str| {
            serde_json::json!({"lastSeenAppVersion": value})
                .as_object()
                .expect("state")
                .clone()
        };
        assert!(previous.load_device_ui_state().expect("empty").is_empty());
        previous
            .save_device_ui_state(state("1.0.0"))
            .expect("save previous state");
        use lettuce_usage::AppUsageRepository;
        previous
            .add_app_usage("2026-09-20", 1_000, TimestampMillis::new(1))
            .expect("usage");
        previous
            .add_app_usage("2026-09-20", 500, TimestampMillis::new(2))
            .expect("usage adds");
        assert_eq!(
            previous.add_app_usage("20-09-2026", 1, TimestampMillis::new(3)),
            Err(lettuce_usage::AppUsageError::InvalidDay)
        );
        let carried = Database::open(root.join("carried.sqlite3")).expect("carried database");
        carried
            .carry_device_local_state_from(&previous_path)
            .expect("carry");
        assert_eq!(
            carried.load_device_ui_state().expect("carried"),
            state("1.0.0")
        );
        assert_eq!(
            carried.app_usage_days().expect("carried usage"),
            vec![lettuce_usage::AppUsageDay {
                day: "2026-09-20".into(),
                active_ms: 1_500,
            }]
        );
        let imported = Database::open(root.join("imported.sqlite3")).expect("imported database");
        imported
            .save_device_ui_state(state("2.0.0"))
            .expect("imported state");
        imported
            .carry_device_local_state_from(&previous_path)
            .expect("carry");
        assert_eq!(
            imported.load_device_ui_state().expect("kept"),
            state("2.0.0")
        );

        use lettuce_settings::DeviceSettingsStore;
        let mut device = lettuce_settings::DeviceSettings {
            llm_models_dir: Some("/models".into()),
            ..Default::default()
        };
        previous
            .save_device_settings(device.clone())
            .expect("save device settings");
        device.llm_models_dir = Some("  ".into());
        assert!(previous.save_device_settings(device).is_err());
        let restored = Database::open(root.join("restored.sqlite3")).expect("restored database");
        restored
            .carry_device_local_state_from(&previous_path)
            .expect("carry");
        assert_eq!(
            restored
                .load_device_settings()
                .expect("carried settings")
                .llm_models_dir
                .as_deref(),
            Some("/models")
        );
    }

    #[test]
    fn generation_metrics_the_new_file_already_has_are_kept() {
        let root = std::env::temp_dir().join(format!("device-metrics-{}", OperationId::new()));
        std::fs::create_dir_all(&root).expect("fixture root");
        let previous_path = root.join("previous.sqlite3");
        let previous = Database::open(&previous_path).expect("previous database");
        for (id, summary) in [("gen-1", "previous"), ("gen-2", "previous")] {
            previous
                .record_llm_generation_metrics(
                    id,
                    None,
                    &serde_json::json!({ summary: true }),
                    &[],
                    1,
                )
                .expect("previous metrics");
        }
        let imported = Database::open(root.join("imported.sqlite3")).expect("imported database");
        imported
            .record_llm_generation_metrics(
                "gen-1",
                None,
                &serde_json::json!({"imported": true}),
                &[],
                1,
            )
            .expect("imported metrics");
        for (database, id) in [
            (&previous, "attempt-from-first-source"),
            (&imported, "attempt-from-second-source"),
        ] {
            database
                .record_llm_generation_metrics(
                    id,
                    Some("/models/a.gguf"),
                    &serde_json::json!({"completionTokens": 3}),
                    &[serde_json::json!({"t": 1})],
                    5,
                )
                .expect("the same legacy metric under a source-derived id");
        }
        imported
            .carry_device_local_state_from(&previous_path)
            .expect("carry");
        let mut ids = imported
            .llm_generation_metrics(None)
            .expect("list")
            .into_iter()
            .map(|metric| metric.id)
            .collect::<Vec<_>>();
        ids.sort();
        assert_eq!(ids, ["attempt-from-second-source", "gen-1", "gen-2"]);
        assert_eq!(
            imported
                .llm_generation_metric("gen-1")
                .expect("read")
                .expect("kept metric")
                .summary,
            serde_json::json!({"imported": true})
        );
        drop((previous, imported));
        std::fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn lora_paths_both_files_have_keep_the_more_recently_updated_row() {
        use lettuce_image_generation::sd_runtime::lora_library::{
            LoraArchitectureSource, LoraKeywordSource, LoraLibraryRepository, LoraRecord,
        };
        let root = std::env::temp_dir().join(format!("device-loras-{}", OperationId::new()));
        std::fs::create_dir_all(&root).expect("fixture root");
        let lora = |path: &str, keyword: &str| LoraRecord {
            path: path.to_owned(),
            filename: "style.safetensors".to_owned(),
            bytes_on_disk: 1,
            modified_at: 1,
            sha256: None,
            keywords: vec![keyword.to_owned()],
            keyword_source: LoraKeywordSource::Manual,
            architecture: None,
            architecture_source: LoraArchitectureSource::None,
        };
        let previous_path = root.join("previous.sqlite3");
        let previous = Database::open(&previous_path).expect("previous database");
        for record in [
            lora("/loras/style.safetensors", "previous"),
            lora("/loras/ink.safetensors", "ink"),
        ] {
            previous
                .save_lora(&record, TimestampMillis::new(1))
                .expect("previous lora");
        }
        previous
            .save_lora(
                &lora("/loras/edited.safetensors", "edited later"),
                TimestampMillis::new(5),
            )
            .expect("previous edited lora");
        let imported = Database::open(root.join("imported.sqlite3")).expect("imported database");
        imported
            .save_lora(
                &lora("/loras/style.safetensors", "imported"),
                TimestampMillis::new(2),
            )
            .expect("imported lora");
        imported
            .save_lora(
                &lora("/loras/edited.safetensors", "stale"),
                TimestampMillis::new(2),
            )
            .expect("imported stale lora");
        imported
            .carry_device_local_state_from(&previous_path)
            .expect("carry");
        assert_eq!(
            imported
                .lora("/loras/style.safetensors")
                .expect("read")
                .expect("kept lora")
                .keywords,
            vec!["imported".to_owned()]
        );
        assert_eq!(
            imported
                .lora("/loras/edited.safetensors")
                .expect("read")
                .expect("edited lora")
                .keywords,
            vec!["edited later".to_owned()]
        );
        assert!(
            imported
                .lora("/loras/ink.safetensors")
                .expect("read")
                .is_some()
        );
        drop((previous, imported));
        std::fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn device_local_sync_identity_moves_to_the_restored_database() {
        let root = std::env::temp_dir().join(format!("device-local-{}", OperationId::new()));
        std::fs::create_dir_all(&root).expect("fixture root");
        let previous_path = root.join("previous.sqlite3");
        let previous = Database::open(&previous_path).expect("previous database");
        let device = previous
            .local_device_id(TimestampMillis::new(1))
            .expect("previous device");
        let restored = Database::open(root.join("restored.sqlite3")).expect("restored database");
        restored
            .carry_device_local_state_from(&previous_path)
            .expect("carry device-local state");
        assert_eq!(
            restored
                .local_device_id(TimestampMillis::new(2))
                .expect("restored device"),
            device
        );
        assert!(
            restored
                .carry_device_local_state_from(&previous_path)
                .is_err()
        );
    }
}
