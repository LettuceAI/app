use lettuce_transfer::BackupApiOperationReceipt;
use lettuce_types::TimestampMillis;
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Serialize, de::DeserializeOwned};

use crate::Database;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ApiOperationError {
    #[error("the operation key was already used for a different request")]
    Conflict,
    #[error("the operation receipt is invalid")]
    InvalidData,
    #[error("operation storage failed")]
    Storage,
}

#[derive(Debug)]
pub struct ApiOperationTransaction<'a, 'connection> {
    pub(crate) transaction: &'a Transaction<'connection>,
}

fn storage(_: impl std::fmt::Debug) -> ApiOperationError { ApiOperationError::Storage }
fn invalid(_: impl std::fmt::Debug) -> ApiOperationError { ApiOperationError::InvalidData }

fn lookup_in(connection: &Connection, command: &str, key: &str) -> Result<Option<BackupApiOperationReceipt>, ApiOperationError> {
    connection.query_row(
        "SELECT request_digest,result_json,committed_at FROM api_operation_receipts WHERE command=?1 AND client_operation_id=?2",
        params![command, key], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?)),
    ).optional().map_err(storage)?.map(|(request_digest, result, at)| Ok(BackupApiOperationReceipt {
        result_format_version: 2, command: command.to_owned(), client_operation_id: key.to_owned(), request_digest,
        result: crate::decode_versioned(&result, 2).map_err(invalid)?, committed_at: TimestampMillis::new(at),
    })).transpose()
}

pub(crate) fn insert_in(connection: &Connection, receipt: &BackupApiOperationReceipt) -> Result<(), ApiOperationError> {
    if receipt.result_format_version != 2 || receipt.command.trim().is_empty() || receipt.client_operation_id.trim().is_empty() || receipt.request_digest.trim().is_empty() {
        return Err(ApiOperationError::InvalidData);
    }
    connection.execute("INSERT INTO api_operation_receipts(command,client_operation_id,request_digest,result_json,committed_at) VALUES (?1,?2,?3,?4,?5)",
        params![receipt.command, receipt.client_operation_id, receipt.request_digest,
            crate::encode_versioned(&receipt.result, 2).map_err(invalid)?, receipt.committed_at.get()],
    ).map_err(storage)?;
    Ok(())
}

pub(crate) fn receipts_in(connection: &Connection) -> Result<Vec<BackupApiOperationReceipt>, ApiOperationError> {
    let keys = connection.prepare("SELECT command,client_operation_id FROM api_operation_receipts ORDER BY command,client_operation_id")
        .map_err(storage)?.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(storage)?.collect::<rusqlite::Result<Vec<_>>>().map_err(storage)?;
    keys.into_iter().map(|(command, key)| lookup_in(connection, &command, &key)?.ok_or(ApiOperationError::InvalidData)).collect()
}

impl Database {
    pub fn lookup_api_operation(&self, command: &str, key: &str) -> Result<Option<BackupApiOperationReceipt>, ApiOperationError> {
        let connection = self.connection().map_err(storage)?;
        lookup_in(&connection, command, key)
    }

    pub fn commit_api_operation<T, E>(
        &self, command: &str, key: &str, digest: &str, at: TimestampMillis,
        apply: impl FnOnce(&ApiOperationTransaction<'_, '_>) -> Result<T, E>,
    ) -> Result<T, E>
    where T: Serialize + DeserializeOwned, E: From<ApiOperationError> {
        if command.trim().is_empty() || key.trim().is_empty() || digest.trim().is_empty() {
            return Err(ApiOperationError::InvalidData.into());
        }
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage)?;
        if let Some(receipt) = lookup_in(&transaction, command, key)? {
            if receipt.request_digest != digest { return Err(ApiOperationError::Conflict.into()); }
            return serde_json::from_value(receipt.result).map_err(|error| invalid(error).into());
        }
        let result = apply(&ApiOperationTransaction { transaction: &transaction })?;
        insert_in(&transaction, &BackupApiOperationReceipt {
            result_format_version: 2, command: command.to_owned(), client_operation_id: key.to_owned(), request_digest: digest.to_owned(),
            result: serde_json::to_value(&result).map_err(invalid)?, committed_at: at,
        })?;
        transaction.commit().map_err(storage)?;
        Ok(result)
    }
}

impl ApiOperationTransaction<'_, '_> {
    pub fn create_audio_provider(&self, provider: &lettuce_speech::AudioProvider) -> Result<(), lettuce_speech::TtsConfigurationRepositoryError> {
        crate::media::tts_adapter::insert_audio_provider(self.transaction, provider)
    }

    pub fn save_vocabulary(&self, term: lettuce_speech::AsrVocabularyTerm) -> Result<lettuce_speech::AsrVocabularyTerm, lettuce_speech::AsrLearningRepositoryError> {
        crate::media::speech_learning_adapter::save_vocabulary_in(self.transaction, term)
    }

    pub fn save_correction_draft(&self, draft: lettuce_speech::AsrCorrectionDraft, now: TimestampMillis) -> Result<lettuce_speech::AsrCorrectionRule, lettuce_speech::AsrLearningRepositoryError> {
        crate::media::speech_learning_adapter::save_correction_draft_in(self.transaction, draft, now)
    }

    pub fn save_ignored_suggestion(&self, suggestion: lettuce_speech::AsrIgnoredSuggestion) -> Result<lettuce_speech::AsrIgnoredSuggestion, lettuce_speech::AsrLearningRepositoryError> {
        crate::media::speech_learning_adapter::save_ignored_suggestion_in(self.transaction, suggestion)
    }

    pub fn save_voice_example(&self, example: lettuce_speech::AsrVoiceExample) -> Result<lettuce_speech::AsrVoiceExample, lettuce_speech::AsrLearningRepositoryError> {
        crate::media::speech_learning_adapter::save_voice_example_in(self.transaction, example)
    }

    pub fn import_learning_batch(&self, batch: lettuce_speech::AsrLearningBatch) -> Result<lettuce_speech::AsrLearningImportReceipt, lettuce_speech::AsrLearningRepositoryError> {
        crate::media::speech_learning_adapter::import_learning_batch_in(self.transaction, batch)
    }

    pub fn get_vocabulary(&self, id: lettuce_types::AsrVocabularyTermId) -> Result<Option<lettuce_speech::AsrVocabularyTerm>, lettuce_speech::AsrLearningRepositoryError> {
        Ok(crate::media::speech_learning_adapter::load_vocabulary(self.transaction, Some(id), None, &[])?.into_iter().next())
    }

    pub fn get_voice_example(&self, id: lettuce_types::AsrVoiceExampleId) -> Result<Option<lettuce_speech::AsrVoiceExample>, lettuce_speech::AsrLearningRepositoryError> {
        crate::media::speech_learning_adapter::get_voice_example_in(self.transaction, id)
    }

    pub fn ignore_suggestion(&self, suggestion: lettuce_speech::AsrLearnedSuggestion, now: TimestampMillis) -> Result<lettuce_speech::AsrIgnoredSuggestion, lettuce_speech::AsrLearningRepositoryError> {
        use lettuce_speech::AsrLearningRepositoryError;
        suggestion.validate().map_err(|_| AsrLearningRepositoryError::InvalidData)?;
        let existing = crate::media::speech_learning_adapter::find_ignored_suggestion_in(self.transaction,
            &suggestion.normalized_wrong, &suggestion.normalized_correct, suggestion.language.as_deref(), &suggestion.scope, false)?;
        let value = lettuce_speech::AsrIgnoredSuggestion {
            id: existing.as_ref().map_or_else(lettuce_types::AsrIgnoredSuggestionId::new, |item| item.id),
            wrong: suggestion.wrong, normalized_wrong: suggestion.normalized_wrong,
            correct: suggestion.correct, normalized_correct: suggestion.normalized_correct,
            language: suggestion.language, scope: suggestion.scope,
            ignored_count: existing.as_ref().map_or(Ok(1), |item| item.ignored_count.checked_add(1).ok_or(AsrLearningRepositoryError::InvalidData))?,
            last_ignored_at: now, created_at: existing.as_ref().map_or(now, |item| item.created_at),
            updated_at: existing.as_ref().map_or(now, |item| now.max(item.updated_at)),
        };
        self.save_ignored_suggestion(value)
    }

    pub fn create_user_voice(&self, voice: &lettuce_speech::UserVoice) -> Result<(), lettuce_speech::TtsConfigurationRepositoryError> {
        crate::media::tts_adapter::insert_user_voice(self.transaction, voice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receipts_survive_backup_restore_and_device_carry_without_entering_sync() {
        use lettuce_transfer::{ProviderBackupSource, ProviderBackupRestoreWriter};
        use lettuce_sync::LocalChangeJournal;
        let source = Database::open_in_memory().expect("source");
        source.journal_current_state(TimestampMillis::new(1)).expect("baseline");
        let baseline: i64 = source.connection().expect("connection").query_row("SELECT count(*) FROM sync_changes", [], |row| row.get(0)).expect("baseline changes");
        let result: String = source.commit_api_operation("future_create", "key", "digest", TimestampMillis::new(1), |_| Ok::<_, ApiOperationError>("original".into())).expect("receipt");
        let mut graph = source.read_provider_backup_graph().expect("backup graph");
        lettuce_transfer::canonicalize_and_validate(&mut graph).expect("valid graph");
        assert_eq!(graph.job_backup.api_operation_receipts.len(), 1);
        let restored = Database::open_in_memory().expect("restored");
        restored.restore_provider_backup_graph(&graph, &[]).expect("restore");
        let replay: String = restored.commit_api_operation("future_create", "key", "digest", TimestampMillis::new(2), |_| -> Result<String, ApiOperationError> { panic!("restored replay"); }).expect("restored receipt");
        assert_eq!(replay, result);
        source.journal_current_state(TimestampMillis::new(2)).expect("journal");
        let connection = source.connection().expect("connection");
        let changes: i64 = connection.query_row("SELECT count(*) FROM sync_changes", [], |row| row.get(0)).expect("journal count");
        assert_eq!(changes, baseline, "receipts are device-local bookkeeping");
        drop(connection);
        let root = std::env::temp_dir().join(format!("lettuce-receipt-carry-{}", lettuce_types::OperationId::new()));
        std::fs::create_dir_all(&root).expect("root");
        let previous_path = root.join("previous.sqlite3");
        let previous = Database::open(&previous_path).expect("previous");
        previous.commit_api_operation("another_future_create", "old-key", "old-digest", TimestampMillis::new(1), |_| Ok::<_, ApiOperationError>("previous".to_owned())).expect("previous receipt");
        let next = Database::open(root.join("next.sqlite3")).expect("next");
        next.carry_device_local_state_from(&previous_path).expect("carry receipts");
        assert_eq!(next.lookup_api_operation("another_future_create", "old-key").expect("lookup").expect("receipt").result, serde_json::json!("previous"));
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn concurrent_same_key_applies_once() {
        let database = std::sync::Arc::new(Database::open_in_memory().expect("database"));
        let applied = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let threads = (0..8).map(|_| {
            let database = database.clone(); let applied = applied.clone();
            std::thread::spawn(move || database.commit_api_operation("future_create", "key", "digest", TimestampMillis::new(1), |_| {
                applied.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok::<_, ApiOperationError>("one result".to_owned())
            }).expect("concurrent replay"))
        }).collect::<Vec<_>>();
        for thread in threads { assert_eq!(thread.join().expect("thread"), "one result"); }
        assert_eq!(applied.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn receipts_replay_conflict_and_rollback_with_the_callers_changes() {
        let database = Database::open_in_memory().expect("database");
        let apply = |transaction: &ApiOperationTransaction<'_, '_>| -> Result<String, ApiOperationError> {
            transaction.transaction.execute("INSERT INTO app_usage_days(day,active_ms,updated_at) VALUES ('2026-10-05',1,1)", []).map_err(storage)?;
            Ok("original".into())
        };
        let first: String = database.commit_api_operation("future_create", "key", "digest", TimestampMillis::new(1), apply).expect("commit");
        database.connection().expect("connection").execute("DELETE FROM app_usage_days", []).expect("later deletion");
        let replay: String = database.commit_api_operation("future_create", "key", "digest", TimestampMillis::new(2), |_| -> Result<String, ApiOperationError> { panic!("replay must not apply") }).expect("replay");
        assert_eq!(first, replay);
        let conflict = database.commit_api_operation("future_create", "key", "changed", TimestampMillis::new(2), |_| -> Result<String, ApiOperationError> { panic!("conflict must not apply") });
        assert_eq!(conflict, Err(ApiOperationError::Conflict));
        let failed = database.commit_api_operation("future_create", "failed", "digest", TimestampMillis::new(3), |transaction| -> Result<String, ApiOperationError> {
            transaction.transaction.execute("INSERT INTO app_usage_days(day,active_ms,updated_at) VALUES ('2026-10-05',1,1)", []).map_err(storage)?;
            Err(ApiOperationError::InvalidData)
        });
        assert_eq!(failed, Err(ApiOperationError::InvalidData));
        assert!(database.lookup_api_operation("future_create", "failed").expect("lookup").is_none());
        let count: i64 = database.connection().expect("connection").query_row("SELECT count(*) FROM app_usage_days", [], |row| row.get(0)).expect("count");
        assert_eq!(count, 0);
    }
}
