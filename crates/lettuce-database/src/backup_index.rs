use lettuce_types::{JobId, TimestampMillis};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::{ApiOperationError, ApiOperationTransaction, Database};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupIndexRecord {
    pub id: JobId,
    pub uri: String,
    pub version: u32,
    pub created_at: TimestampMillis,
    pub app_version: String,
    pub size: u64,
}

fn storage(error: impl std::fmt::Debug) -> ApiOperationError {
    tracing::warn!(?error, "backup index storage step failed");
    ApiOperationError::Storage {
        reason: "backup index: a storage step failed",
    }
}

fn read_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<BackupIndexRecord> {
    let id: String = row.get(0)?;
    let size: i64 = row.get(5)?;
    Ok(BackupIndexRecord {
        id: id.parse().map_err(|_| rusqlite::Error::InvalidQuery)?,
        uri: row.get(1)?,
        version: row.get(2)?,
        created_at: TimestampMillis::new(row.get(3)?),
        app_version: row.get(4)?,
        size: u64::try_from(size).map_err(|_| rusqlite::Error::InvalidQuery)?,
    })
}

fn lookup(connection: &Connection, id: JobId) -> Result<Option<BackupIndexRecord>, ApiOperationError> {
    connection.query_row(
        "SELECT id,uri,version,created_at,app_version,size FROM backups_index WHERE id=?1",
        [id.to_string()], read_record,
    ).optional().map_err(storage)
}

impl Database {
    pub fn backup_index(&self) -> Result<Vec<BackupIndexRecord>, ApiOperationError> {
        let connection = self.connection().map_err(storage)?;
        let mut statement = connection.prepare(
            "SELECT id,uri,version,created_at,app_version,size FROM backups_index ORDER BY created_at DESC,id DESC",
        ).map_err(storage)?;
        statement.query_map([], read_record).map_err(storage)?
            .collect::<rusqlite::Result<Vec<_>>>().map_err(storage)
    }

    pub fn record_backup(&self, entry: &BackupIndexRecord) -> Result<(), ApiOperationError> {
        if entry.uri.trim().is_empty() || entry.version != 2
            || entry.app_version.is_empty() || entry.app_version.len() > 64
            || entry.app_version.chars().any(char::is_control)
        {
            return Err(ApiOperationError::InvalidData {
                reason: "record backup: a value failed validation",
            });
        }
        let size = i64::try_from(entry.size).map_err(|error| { tracing::warn!(%error, "record backup failed"); ApiOperationError::InvalidData {
            reason: "record backup: a value could not be read or converted",
        } })?;
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(storage)?;
        if let Some(existing) = lookup(&transaction, entry.id)? {
            return if existing == *entry { Ok(()) } else { Err(ApiOperationError::Conflict) };
        }
        transaction.execute(
            "INSERT INTO backups_index (id,uri,version,created_at,app_version,size) VALUES (?1,?2,?3,?4,?5,?6)
             ON CONFLICT(uri) DO UPDATE SET id=excluded.id,version=excluded.version,created_at=excluded.created_at,app_version=excluded.app_version,size=excluded.size",
            params![entry.id.to_string(),entry.uri,entry.version,entry.created_at.get(),entry.app_version,size],
        ).map_err(storage)?;
        transaction.commit().map_err(storage)
    }
}

impl ApiOperationTransaction<'_, '_> {
    pub fn backup_index_record(&self, id: JobId) -> Result<Option<BackupIndexRecord>, ApiOperationError> {
        lookup(self.transaction, id)
    }

    pub fn forget_backup(&self, id: JobId) -> Result<bool, ApiOperationError> {
        self.transaction.execute("DELETE FROM backups_index WHERE id=?1", [id.to_string()])
            .map(|count| count == 1).map_err(storage)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn backup_index_is_device_local_and_has_one_record_per_path() {
        let database = crate::Database::open_in_memory().expect("database");
        let connection = database.connection().expect("lock");
        connection.execute("INSERT INTO backups_index VALUES ('first','/backup.lettuce',2,1,'1.0',100)", []).expect("index");
        assert!(connection.execute("INSERT INTO backups_index VALUES ('second','/backup.lettuce',2,2,'1.0',100)", []).is_err());
        assert_eq!(connection.query_row("SELECT count(*) FROM backups_index", [], |row| row.get::<_, i64>(0)).expect("count"), 1);
    }
}

#[cfg(test)]
mod mutation_tests {
    use crate::{ApiOperationError, BackupIndexRecord, Database};
    use lettuce_types::{JobId, TimestampMillis};
    use lettuce_transfer::ProviderBackupSource;

    fn entry(uri: &str, at: i64) -> BackupIndexRecord {
        BackupIndexRecord { id: JobId::new(), uri: uri.into(), version: 2, created_at: TimestampMillis::new(at), app_version: "1.0".into(), size: 100 }
    }

    #[test]
    fn replacing_a_path_keeps_one_entry_and_forget_replays_or_conflicts() {
        let database = Database::open_in_memory().expect("database");
        let first = entry("/backup.lettuce", 1);
        let second = entry("/backup.lettuce", 2);
        database.record_backup(&first).expect("first");
        database.record_backup(&second).expect("replace");
        assert_eq!(database.backup_index().expect("index"), vec![second.clone()]);
        database.record_backup(&second).expect("record replay");
        let changed = BackupIndexRecord { size: 101, ..second.clone() };
        assert_eq!(database.record_backup(&changed), Err(ApiOperationError::Conflict));
        database.commit_api_operation("backup-forget", "key", "digest", TimestampMillis::new(3), |transaction| {
            assert_eq!(transaction.backup_index_record(second.id)?, Some(second.clone()));
            assert!(transaction.forget_backup(second.id)?);
            Ok::<_, ApiOperationError>(())
        }).expect("forget");
        database.commit_api_operation("backup-forget", "key", "digest", TimestampMillis::new(4), |_| -> Result<(), ApiOperationError> { panic!("replay cannot apply twice") }).expect("replay");
        assert_eq!(database.commit_api_operation("backup-forget", "key", "changed", TimestampMillis::new(4), |_| Ok::<_, ApiOperationError>(())), Err(ApiOperationError::Conflict));
        assert!(database.backup_index().expect("empty").is_empty());
    }

    #[test]
    fn failure_before_receipt_commit_rolls_back_index_removal() {
        let database = Database::open_in_memory().expect("database");
        let entry = entry("/backup.lettuce", 1);
        database.record_backup(&entry).expect("entry");
        let failure = database.commit_api_operation("backup-delete", "key", "digest", TimestampMillis::new(2), |transaction| {
            assert!(transaction.forget_backup(entry.id)?);
            Err::<(), _>(ApiOperationError::Storage {
                reason: "injected failure",
            })
        });
        assert!(matches!(failure, Err(ApiOperationError::Storage { .. })));
        assert_eq!(database.backup_index().expect("retained"), vec![entry]);
        assert!(database.lookup_api_operation("backup-delete", "key").expect("receipt").is_none());
    }

    #[test]
    fn index_carries_to_a_fresh_database_but_is_absent_from_the_backup_graph() {
        let root = std::env::temp_dir().join(format!("lettuce-backup-index-{}", JobId::new()));
        std::fs::create_dir_all(&root).expect("folder");
        let old = Database::open(root.join("old.sqlite3")).expect("old");
        let entry = entry("/private-backup-location.lettuce", 1);
        old.record_backup(&entry).expect("entry");
        let graph = old.read_provider_backup_graph().expect("graph");
        assert!(!serde_json::to_string(&graph).expect("graph JSON").contains(&entry.uri));
        let new = Database::open(root.join("new.sqlite3")).expect("new");
        new.carry_device_local_state_from(&root.join("old.sqlite3")).expect("carry");
        assert_eq!(new.backup_index().expect("carried"), vec![entry]);
        drop(new);
        drop(old);
        std::fs::remove_dir_all(root).expect("remove folder");
    }
}
