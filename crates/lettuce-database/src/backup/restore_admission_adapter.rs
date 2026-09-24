use lettuce_transfer::{
    BackupRestoreAdmission, BackupRestoreAdmissionError, BackupRestoreAdmissionRepository,
    BackupRestoreAdmissionRequest, BackupRestoreAdmissionStatus, BackupRestoreCounts,
    BackupRestoreSourceVersion,
};
use lettuce_types::{ContentHash, OperationId, TimestampMillis};
use rusqlite::{OptionalExtension, params};

use crate::Database;

impl BackupRestoreAdmissionRepository for Database {
    fn admit_backup_restore(
        &self,
        request: BackupRestoreAdmissionRequest,
    ) -> Result<BackupRestoreAdmission, BackupRestoreAdmissionError> {
        validate_request(&request)?;
        let document_count = to_i64(request.counts.document_count)?;
        let media_count = to_i64(request.counts.media_count)?;
        let secret_count = to_i64(request.counts.secret_count)?;
        let artifact_count = to_i64(request.counts.artifact_count)?;
        let source_version = match request.source_version {
            BackupRestoreSourceVersion::LegacyV1 => 1_i64,
            BackupRestoreSourceVersion::CurrentV2 => 2_i64,
        };
        let mut connection = self
            .connection()
            .map_err(|_| BackupRestoreAdmissionError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| BackupRestoreAdmissionError::Storage)?;
        if let Some(stored) = load(&transaction, request.id)? {
            let retried = BackupRestoreAdmissionRequest {
                admitted_at: stored.request.admitted_at,
                ..request
            };
            return if stored.request == retried {
                Ok(stored)
            } else {
                Err(BackupRestoreAdmissionError::Conflict)
            };
        }
        transaction
            .execute(
                "INSERT INTO backup_restore_admissions (
                    id,source_version,source_hash,plan_fingerprint,staging_receipt_fingerprint,
                    document_count,media_count,secret_count,artifact_count,status,admitted_at
                 ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'admitted',?10)",
                params![
                    request.id.to_string(),
                    source_version,
                    request.source_hash.as_str(),
                    request.plan_fingerprint.as_str(),
                    request.staging_receipt_fingerprint.as_str(),
                    document_count,
                    media_count,
                    secret_count,
                    artifact_count,
                    request.admitted_at.get(),
                ],
            )
            .map_err(|_| BackupRestoreAdmissionError::Storage)?;
        transaction
            .commit()
            .map_err(|_| BackupRestoreAdmissionError::Storage)?;
        Ok(BackupRestoreAdmission {
            request,
            status: BackupRestoreAdmissionStatus::Admitted,
            replayed: false,
        })
    }
}

fn validate_request(
    request: &BackupRestoreAdmissionRequest,
) -> Result<(), BackupRestoreAdmissionError> {
    if request.admitted_at.get() < 0 {
        return Err(BackupRestoreAdmissionError::InvalidInput);
    }
    Ok(())
}

fn to_i64(value: u64) -> Result<i64, BackupRestoreAdmissionError> {
    i64::try_from(value).map_err(|_| BackupRestoreAdmissionError::InvalidInput)
}

fn load(
    connection: &rusqlite::Connection,
    id: OperationId,
) -> Result<Option<BackupRestoreAdmission>, BackupRestoreAdmissionError> {
    let row = connection
        .query_row(
            "SELECT source_version,source_hash,plan_fingerprint,staging_receipt_fingerprint,
                    document_count,media_count,secret_count,artifact_count,status,admitted_at
             FROM backup_restore_admissions WHERE id=?1",
            [id.to_string()],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, i64>(9)?,
                ))
            },
        )
        .optional()
        .map_err(|_| BackupRestoreAdmissionError::Storage)?;
    row.map(|row| {
        let source_version = match row.0 {
            1 => BackupRestoreSourceVersion::LegacyV1,
            2 => BackupRestoreSourceVersion::CurrentV2,
            _ => return Err(BackupRestoreAdmissionError::Storage),
        };
        if row.8 != "admitted" {
            return Err(BackupRestoreAdmissionError::Storage);
        }
        Ok(BackupRestoreAdmission {
            request: BackupRestoreAdmissionRequest {
                id,
                source_version,
                source_hash: ContentHash::parse(row.1)
                    .map_err(|_| BackupRestoreAdmissionError::Storage)?,
                plan_fingerprint: ContentHash::parse(row.2)
                    .map_err(|_| BackupRestoreAdmissionError::Storage)?,
                staging_receipt_fingerprint: ContentHash::parse(row.3)
                    .map_err(|_| BackupRestoreAdmissionError::Storage)?,
                counts: BackupRestoreCounts {
                    document_count: from_i64(row.4)?,
                    media_count: from_i64(row.5)?,
                    secret_count: from_i64(row.6)?,
                    artifact_count: from_i64(row.7)?,
                },
                admitted_at: TimestampMillis::new(row.9),
            },
            status: BackupRestoreAdmissionStatus::Admitted,
            replayed: true,
        })
    })
    .transpose()
}

fn from_i64(value: i64) -> Result<u64, BackupRestoreAdmissionError> {
    u64::try_from(value).map_err(|_| BackupRestoreAdmissionError::Storage)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(id: OperationId) -> BackupRestoreAdmissionRequest {
        BackupRestoreAdmissionRequest {
            id,
            source_version: BackupRestoreSourceVersion::LegacyV1,
            source_hash: ContentHash::parse("11".repeat(32)).expect("source hash"),
            plan_fingerprint: ContentHash::parse("22".repeat(32)).expect("plan fingerprint"),
            staging_receipt_fingerprint: ContentHash::parse("33".repeat(32))
                .expect("receipt fingerprint"),
            counts: BackupRestoreCounts {
                document_count: 23,
                media_count: 4,
                secret_count: 2,
                artifact_count: 0,
            },
            admitted_at: TimestampMillis::new(1_700_000_000_000),
        }
    }

    #[test]
    fn admission_replays_after_reopen_and_changed_input_writes_nothing() {
        let path =
            std::env::temp_dir().join(format!("restore-admission-{}.sqlite3", OperationId::new()));
        let database = Database::open(&path).expect("database");
        let request = request(OperationId::new());

        let first = database
            .admit_backup_restore(request.clone())
            .expect("first admission");
        assert!(!first.replayed);
        drop(database);

        let reopened = Database::open(&path).expect("reopen database");
        let mut retried = request.clone();
        retried.admitted_at = TimestampMillis::new(1_700_000_000_500);
        let replay = reopened
            .admit_backup_restore(retried)
            .expect("replay admission");
        assert!(replay.replayed);
        assert_eq!(replay.request, request);

        let mut changed = request.clone();
        changed.counts.media_count += 1;
        assert_eq!(
            reopened.admit_backup_restore(changed),
            Err(BackupRestoreAdmissionError::Conflict)
        );

        let mut repeated_source = request.clone();
        repeated_source.id = OperationId::new();
        assert!(
            !reopened
                .admit_backup_restore(repeated_source)
                .expect("repeated source admission")
                .replayed
        );

        let mut invalid = request;
        invalid.id = OperationId::new();
        invalid.source_hash = ContentHash::parse("44".repeat(32)).expect("source hash");
        invalid.admitted_at = TimestampMillis::new(-1);
        assert_eq!(
            reopened.admit_backup_restore(invalid),
            Err(BackupRestoreAdmissionError::InvalidInput)
        );
        let count: i64 = reopened
            .connection()
            .expect("database lock")
            .query_row(
                "SELECT count(*) FROM backup_restore_admissions",
                [],
                |row| row.get(0),
            )
            .expect("admission count");
        assert_eq!(count, 2);
        drop(reopened);
        std::fs::remove_file(path).expect("remove database");
    }
}
