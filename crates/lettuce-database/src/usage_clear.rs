use crate::{ApiOperationError, ApiOperationTransaction};
use lettuce_types::TimestampMillis;
use rusqlite::Connection;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(crate) fn install_guard(connection: &Connection) -> rusqlite::Result<Arc<AtomicBool>> {
    let flag = Arc::new(AtomicBool::new(false));
    let query = Arc::clone(&flag);
    connection.create_scalar_function(
        "usage_delete_allowed",
        0,
        rusqlite::functions::FunctionFlags::SQLITE_UTF8,
        move |_| Ok(query.load(Ordering::Acquire)),
    )?;
    Ok(flag)
}

struct DeleteGuard<'a>(&'a AtomicBool);
impl Drop for DeleteGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl ApiOperationTransaction<'_, '_> {
    pub fn clear_usage_before(&self, before: TimestampMillis) -> Result<u64, ApiOperationError> {
        if self.command != "usage_clear_before" {
            return Err(ApiOperationError::InvalidData);
        }
        let connection = self.transaction;
        self.usage_delete_allowed.store(true, Ordering::Release);
        let _guard = DeleteGuard(self.usage_delete_allowed);
        let error = |_| ApiOperationError::Storage;
        connection.execute_batch("CREATE TEMP TABLE usage_clear_events(id TEXT PRIMARY KEY); CREATE TEMP TABLE usage_clear_dispatches(id TEXT PRIMARY KEY);").map_err(error)?;
        connection.execute("INSERT INTO usage_clear_events SELECT event.id FROM usage_events event
            WHERE event.recorded_at < ?1
            AND NOT EXISTS (SELECT 1 FROM generation_attempts attempt WHERE attempt.id=event.attempt_id AND attempt.status NOT IN ('succeeded','failed','cancelled','interrupted'))
            AND NOT EXISTS (SELECT 1 FROM generation_attempts attempt JOIN jobs job ON job.id=attempt.job_id WHERE attempt.id=event.attempt_id AND job.state NOT IN ('succeeded','failed','cancelled','interrupted'))", [before.get()]).map_err(error)?;
        connection.execute("INSERT INTO usage_clear_dispatches SELECT dispatch.id FROM job_inference_usage dispatch
            WHERE dispatch.admitted_at < ?1 AND dispatch.result_json IS NOT NULL
            AND NOT EXISTS (SELECT 1 FROM jobs job WHERE job.id=dispatch.job_id AND job.state NOT IN ('succeeded','failed','cancelled','interrupted'))
            AND NOT EXISTS (SELECT 1 FROM generation_attempts attempt WHERE attempt.id=json_extract(dispatch.record_json, '$.value.logical_attempt_id') AND attempt.status NOT IN ('succeeded','failed','cancelled','interrupted'))", [before.get()]).map_err(error)?;
        let events = connection.prepare("SELECT event.id,event.conversation_id,event.turn_id,event.attempt_id,attempt.job_id FROM usage_events event LEFT JOIN generation_attempts attempt ON attempt.id=event.attempt_id WHERE event.id IN (SELECT id FROM usage_clear_events)")
            .map_err(error)?.query_map([], |row| Ok((row.get::<_, String>(0)?,row.get::<_, String>(1)?,row.get::<_, String>(2)?,row.get::<_, String>(3)?,row.get::<_, Option<String>>(4)?)))
            .map_err(error)?.collect::<rusqlite::Result<Vec<_>>>().map_err(error)?;
        for (id, conversation, turn, attempt, job) in events {
            insert_tombstone_in(
                connection,
                &lettuce_usage::UsageTombstone::Conversation {
                    event_id: id.parse().map_err(|_| ApiOperationError::InvalidData)?,
                    conversation_id: conversation
                        .parse()
                        .map_err(|_| ApiOperationError::InvalidData)?,
                    turn_id: turn.parse().map_err(|_| ApiOperationError::InvalidData)?,
                    attempt_id: attempt
                        .parse()
                        .map_err(|_| ApiOperationError::InvalidData)?,
                    job_id: job
                        .map(|id| id.parse())
                        .transpose()
                        .map_err(|_| ApiOperationError::InvalidData)?,
                },
            )?;
        }
        let dispatches = connection.prepare("SELECT record_json FROM job_inference_usage WHERE id IN (SELECT id FROM usage_clear_dispatches)")
            .map_err(error)?.query_map([], |row| row.get::<_, String>(0)).map_err(error)?.collect::<rusqlite::Result<Vec<_>>>().map_err(error)?;
        for bytes in dispatches {
            let record: lettuce_usage::JobInferenceUsage =
                crate::decode_versioned(&bytes, 1).map_err(|_| ApiOperationError::InvalidData)?;
            insert_tombstone_in(
                connection,
                &lettuce_usage::UsageTombstone::Dispatch {
                    event_id: record.id,
                    attempt_id: record.logical_attempt_id,
                    job_id: record.job_id,
                },
            )?;
        }
        let legacy = connection
            .prepare("SELECT run_id,source_id FROM legacy_usage_records WHERE recorded_at < ?1")
            .map_err(error)?
            .query_map([before.get()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(error)?;
        for (run_id, source_id) in legacy {
            insert_tombstone_in(
                connection,
                &lettuce_usage::UsageTombstone::Legacy { run_id, source_id },
            )?;
        }
        connection
            .execute(
                "DELETE FROM usage_costs WHERE event_id IN (SELECT id FROM usage_clear_events)",
                [],
            )
            .map_err(error)?;
        connection.execute("DELETE FROM job_usage_costs WHERE event_id IN (SELECT id FROM usage_clear_dispatches)", []).map_err(error)?;
        let events = connection
            .execute(
                "DELETE FROM usage_events WHERE id IN (SELECT id FROM usage_clear_events)",
                [],
            )
            .map_err(error)?;
        let jobs = connection.execute("DELETE FROM job_inference_usage WHERE id IN (SELECT id FROM usage_clear_dispatches)", []).map_err(error)?;
        let legacy = connection
            .execute(
                "DELETE FROM legacy_usage_records WHERE recorded_at < ?1",
                [before.get()],
            )
            .map_err(error)?;
        connection
            .execute_batch("DROP TABLE usage_clear_events; DROP TABLE usage_clear_dispatches;")
            .map_err(error)?;
        Ok((events + jobs + legacy) as u64)
    }
}

pub(crate) fn insert_tombstone_in(
    connection: &Connection,
    proof: &lettuce_usage::UsageTombstone,
) -> Result<(), ApiOperationError> {
    use rusqlite::OptionalExtension;
    let (ledger, key) = proof.key();
    let bytes = crate::encode_versioned(proof, 1).map_err(|_| ApiOperationError::InvalidData)?;
    let existing: Option<String> = connection
        .query_row(
            "SELECT proof_json FROM usage_tombstones WHERE ledger=?1 AND event_key=?2",
            rusqlite::params![ledger, key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| ApiOperationError::Storage)?;
    if let Some(existing) = existing {
        let existing: lettuce_usage::UsageTombstone =
            crate::decode_versioned(&existing, 1).map_err(|_| ApiOperationError::InvalidData)?;
        return if existing == *proof {
            Ok(())
        } else {
            Err(ApiOperationError::Conflict)
        };
    }
    connection
        .execute(
            "INSERT INTO usage_tombstones VALUES (?1,?2,?3)",
            rusqlite::params![ledger, key, bytes],
        )
        .map_err(|_| ApiOperationError::Storage)?;
    Ok(())
}

pub(crate) fn dispatch_tombstone_in(
    connection: &Connection,
    id: lettuce_types::UsageEventId,
) -> Result<Option<lettuce_usage::UsageTombstone>, ApiOperationError> {
    use rusqlite::OptionalExtension;
    let bytes: Option<String> = connection
        .query_row(
            "SELECT proof_json FROM usage_tombstones WHERE ledger='job' AND event_key=?1",
            [id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| ApiOperationError::Storage)?;
    bytes.map(|bytes| {
        let proof: lettuce_usage::UsageTombstone = crate::decode_versioned(&bytes, 1).map_err(|_| ApiOperationError::InvalidData)?;
        if !matches!(&proof, lettuce_usage::UsageTombstone::Dispatch { event_id, .. } if *event_id == id) { return Err(ApiOperationError::InvalidData); }
        Ok(proof)
    }).transpose()
}

pub(crate) fn tombstones_in(
    connection: &Connection,
) -> Result<Vec<lettuce_usage::UsageTombstone>, ApiOperationError> {
    let values = connection
        .prepare(
            "SELECT ledger,event_key,proof_json FROM usage_tombstones ORDER BY ledger,event_key",
        )
        .map_err(|_| ApiOperationError::Storage)?
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|_| ApiOperationError::Storage)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| ApiOperationError::Storage)?;
    values
        .into_iter()
        .map(|(ledger, key, bytes)| {
            let proof: lettuce_usage::UsageTombstone =
                crate::decode_versioned(&bytes, 1).map_err(|_| ApiOperationError::InvalidData)?;
            if proof.key() != (ledger.as_str(), key) {
                return Err(ApiOperationError::InvalidData);
            }
            Ok(proof)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::{ApiOperationError, Database};
    use lettuce_jobs::{
        CancellationReason, JobKind, JobMutation, JobSpec, JobStore, JobSubject, OutcomeRef,
        SubjectKind,
    };
    use lettuce_types::{
        AssetId, GenerationAttemptId, ModelProfileId, ProviderAccountId, Revision, TimestampMillis,
        UsageEventId,
    };
    use lettuce_usage::{JobInferenceUsage, JobInferenceUsageResult, JobUsageLedger};

    fn dispatch(database: &Database) -> JobInferenceUsage {
        let job = database
            .create_or_get(
                JobSpec::new(
                    JobKind::ArtifactInstall,
                    JobSubject::new(SubjectKind::ArtifactInstall, "clear-test")
                        .expect("test operation succeeds"),
                    OutcomeRef::ArtifactInstallation(AssetId::new()),
                )
                .with_resources(vec![lettuce_jobs::ResourceClass::Network]),
            )
            .expect("test operation succeeds")
            .job;
        let record = JobInferenceUsage {
            id: UsageEventId::new(),
            job_id: job.id,
            logical_attempt_id: GenerationAttemptId::new(),
            model_profile_id: ModelProfileId::new(),
            model_revision: Revision::INITIAL,
            provider_account_id: ProviderAccountId::new(),
            provider_account_revision: Revision::INITIAL,
            admitted_at: TimestampMillis::new(10),
            result: None,
        };
        database
            .admit_job_usage(record.clone())
            .expect("test operation succeeds");
        record
    }

    fn clear(database: &Database, key: &str, before: i64) -> Result<u64, ApiOperationError> {
        database.commit_api_operation(
            "usage_clear_before",
            key,
            &before.to_string(),
            TimestampMillis::new(100),
            |transaction| transaction.clear_usage_before(TimestampMillis::new(before)),
        )
    }

    #[test]
    fn clear_skips_nonterminal_owners_and_pending_dispatches_and_replays_receipt() {
        let database = Database::open_in_memory().expect("test operation succeeds");
        let record = dispatch(&database);
        database
            .settle_job_usage(record.id, JobInferenceUsageResult::InferenceFailed)
            .expect("test operation succeeds");
        assert_eq!(
            clear(&database, "first", 11).expect("test operation succeeds"),
            0
        );
        assert_eq!(
            database
                .job_usage(record.job_id)
                .expect("test operation succeeds")
                .len(),
            1
        );
        let job = database
            .get(record.job_id)
            .expect("test operation succeeds")
            .expect("test operation succeeds");
        database
            .append_and_transition(JobMutation::RequestCancellation {
                id: job.id,
                reason: CancellationReason::User,
                at: job.updated_at,
            })
            .expect("test operation succeeds");
        database
            .append_and_transition(JobMutation::FinishQueuedCancellation {
                id: job.id,
                at: job.updated_at,
            })
            .expect("test operation succeeds");
        let mut pending = record.clone();
        pending.id = UsageEventId::new();
        pending.result = None;
        database
            .admit_job_usage(pending.clone())
            .expect("test operation succeeds");
        assert_eq!(
            clear(&database, "strict", 10).expect("test operation succeeds"),
            0
        );
        assert_eq!(
            clear(&database, "second", 11).expect("test operation succeeds"),
            1
        );
        assert_eq!(
            database
                .job_usage(record.job_id)
                .expect("test operation succeeds"),
            vec![pending]
        );
        assert_eq!(
            clear(&database, "second", 11).expect("test operation succeeds"),
            1
        );
        assert_eq!(
            clear(&database, "second", 12),
            Err(ApiOperationError::Conflict)
        );
        assert!(
            database
                .connection()
                .expect("test operation succeeds")
                .execute("DELETE FROM job_inference_usage", [])
                .is_err()
        );
    }

    #[test]
    fn failed_clear_rolls_back_deletes_tombstones_and_receipt_and_restores_guard() {
        let database = Database::open_in_memory().expect("test operation succeeds");
        let record = dispatch(&database);
        database
            .settle_job_usage(record.id, JobInferenceUsageResult::Cancelled)
            .expect("test operation succeeds");
        database
            .connection()
            .expect("test operation succeeds")
            .execute("DELETE FROM jobs WHERE id=?1", [record.job_id.to_string()])
            .expect("test operation succeeds");
        let outcome: Result<u64, ApiOperationError> = database.commit_api_operation(
            "usage_clear_before",
            "rollback",
            "11",
            TimestampMillis::new(100),
            |transaction| {
                assert_eq!(transaction.clear_usage_before(TimestampMillis::new(11))?, 1);
                Err(ApiOperationError::Storage)
            },
        );
        assert_eq!(outcome, Err(ApiOperationError::Storage));
        assert_eq!(
            database
                .job_usage(record.job_id)
                .expect("test operation succeeds")
                .len(),
            1
        );
        assert!(
            super::tombstones_in(&database.connection().expect("test operation succeeds"))
                .expect("test operation succeeds")
                .is_empty()
        );
        assert!(
            database
                .lookup_api_operation("usage_clear_before", "rollback")
                .expect("test operation succeeds")
                .is_none()
        );
        assert!(
            database
                .connection()
                .expect("test operation succeeds")
                .execute("DELETE FROM job_inference_usage", [])
                .is_err()
        );
        assert_eq!(
            clear(&database, "retry", 11).expect("test operation succeeds"),
            1
        );
    }
    #[test]
    fn process_exit_inside_clear_rolls_back_the_entire_transaction() {
        let path = std::env::temp_dir().join(format!(
            "lettuce-clear-crash-{}.sqlite3",
            uuid::Uuid::new_v4()
        ));
        let database = Database::open(&path).expect("test operation succeeds");
        let record = dispatch(&database);
        database
            .settle_job_usage(record.id, JobInferenceUsageResult::Cancelled)
            .expect("test operation succeeds");
        database
            .connection()
            .expect("test operation succeeds")
            .execute("DELETE FROM jobs WHERE id=?1", [record.job_id.to_string()])
            .expect("test operation succeeds");
        drop(database);
        let status =
            std::process::Command::new(std::env::current_exe().expect("test operation succeeds"))
                .args([
                    "--exact",
                    "usage_clear::tests::clear_crash_child",
                    "--nocapture",
                ])
                .env("LETTUCE_USAGE_CLEAR_CRASH_DATABASE", &path)
                .status()
                .expect("test operation succeeds");
        assert_eq!(status.code(), Some(77));
        let database = Database::open(&path).expect("test operation succeeds");
        assert_eq!(
            database
                .job_usage(record.job_id)
                .expect("test operation succeeds")
                .len(),
            1
        );
        assert!(
            super::tombstones_in(&database.connection().expect("test operation succeeds"))
                .expect("test operation succeeds")
                .is_empty()
        );
        assert!(
            database
                .lookup_api_operation("usage_clear_before", "crash")
                .expect("test operation succeeds")
                .is_none()
        );
        assert!(
            database
                .connection()
                .expect("test operation succeeds")
                .execute("DELETE FROM job_inference_usage", [])
                .is_err()
        );
        assert_eq!(
            clear(&database, "retry", 11).expect("test operation succeeds"),
            1
        );
        drop(database);
        std::fs::remove_file(path).expect("test operation succeeds");
    }

    #[test]
    fn clear_crash_child() {
        let Some(path) = std::env::var_os("LETTUCE_USAGE_CLEAR_CRASH_DATABASE") else {
            return;
        };
        let database = Database::open(path).expect("test operation succeeds");
        let _: Result<u64, ApiOperationError> = database.commit_api_operation(
            "usage_clear_before",
            "crash",
            "11",
            TimestampMillis::new(100),
            |transaction| {
                assert_eq!(transaction.clear_usage_before(TimestampMillis::new(11))?, 1);
                std::process::exit(77);
            },
        );
    }

    #[test]
    fn sync_resend_of_a_tombstoned_id_is_consumed_without_restoring_settled_dispatches() {
        use crate::sync::row_sync_adapter::{JOB_INFERENCE_USAGE, row_current, row_materialize};
        let source = Database::open_in_memory().expect("test operation succeeds");
        let record = dispatch(&source);
        source
            .settle_job_usage(record.id, JobInferenceUsageResult::Cancelled)
            .expect("test operation succeeds");
        let mut source_connection = source.connection().expect("test operation succeeds");
        let source_transaction = source_connection
            .transaction()
            .expect("test operation succeeds");
        let bytes = row_current(
            &source_transaction,
            &JOB_INFERENCE_USAGE,
            &record.id.to_string(),
        )
        .expect("test operation succeeds")
        .expect("test operation succeeds");
        drop(source_transaction);
        drop(source_connection);
        let target = Database::open_in_memory().expect("test operation succeeds");
        {
            let mut connection = target.connection().expect("test operation succeeds");
            let transaction = connection.transaction().expect("test operation succeeds");
            assert!(
                row_materialize(
                    &transaction,
                    &JOB_INFERENCE_USAGE,
                    &record.id.to_string(),
                    &bytes
                )
                .expect("test operation succeeds")
            );
            transaction.commit().expect("test operation succeeds");
        }
        assert_eq!(
            clear(&target, "cut", 11).expect("test operation succeeds"),
            1
        );
        let mut connection = target.connection().expect("test operation succeeds");
        let transaction = connection.transaction().expect("test operation succeeds");
        assert!(
            row_materialize(
                &transaction,
                &JOB_INFERENCE_USAGE,
                &record.id.to_string(),
                &bytes
            )
            .expect("test operation succeeds")
        );
        assert!(
            row_current(&transaction, &JOB_INFERENCE_USAGE, &record.id.to_string())
                .expect("test operation succeeds")
                .is_none()
        );
        transaction.commit().expect("test operation succeeds");
    }

    #[test]
    fn another_api_command_cannot_enable_usage_deletion() {
        let database = Database::open_in_memory().expect("test operation succeeds");
        let result: Result<u64, ApiOperationError> = database.commit_api_operation(
            "unrelated",
            "key",
            "11",
            TimestampMillis::new(100),
            |transaction| transaction.clear_usage_before(TimestampMillis::new(11)),
        );
        assert_eq!(result, Err(ApiOperationError::InvalidData));
    }
    #[test]
    fn clear_preserves_dispatches_claimed_by_another_worker_during_cancel() {
        let database = Database::open_in_memory().expect("test operation succeeds");
        let record = dispatch(&database);
        database
            .settle_job_usage(record.id, JobInferenceUsageResult::Cancelled)
            .expect("test operation succeeds");
        let now = database
            .get(record.job_id)
            .expect("test operation succeeds")
            .expect("test operation succeeds")
            .updated_at;
        let claim = database
            .claim(
                record.job_id,
                lettuce_jobs::WorkerId::new(),
                now,
                std::time::Duration::from_secs(60),
                &lettuce_jobs::ResourceAvailability::all(),
            )
            .expect("test operation succeeds")
            .expect("test operation succeeds");
        std::thread::scope(|scope| {
            let cancel = scope.spawn(|| {
                database
                    .append_and_transition(JobMutation::RequestCancellation {
                        id: record.job_id,
                        reason: CancellationReason::User,
                        at: now,
                    })
                    .expect("test operation succeeds")
            });
            assert_eq!(
                clear(&database, "claimed", 11).expect("test operation succeeds"),
                0
            );
            cancel.join().expect("test operation succeeds");
        });
        assert_eq!(
            database
                .job_usage(record.job_id)
                .expect("test operation succeeds")
                .len(),
            1
        );
        assert_eq!(
            database
                .get(record.job_id)
                .expect("test operation succeeds")
                .expect("test operation succeeds")
                .claim
                .as_ref()
                .expect("test operation succeeds")
                .worker_id,
            claim.claim.worker_id
        );
        assert!(
            super::tombstones_in(&database.connection().expect("test operation succeeds"))
                .expect("test operation succeeds")
                .is_empty()
        );
    }
}
