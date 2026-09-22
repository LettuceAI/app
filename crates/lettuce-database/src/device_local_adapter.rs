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
    "llm_generation_metrics",
];

impl Database {
    /// Copies device-local state from the previous database file: the sync
    /// journal, installed Whisper model manifests, local generation metrics,
    /// and the discovered voices and llama.cpp runtime reports of audio
    /// providers and models that exist in this database.
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
