use lettuce_types::JobId;
use rusqlite::{OptionalExtension, params};
use serde_json::Value;

use crate::{Database, DatabaseError};

/// What a local model job (a download, a pull, a folder move) works on and
/// what it produced, as JSON objects the application defines.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalModelJobRecord {
    pub detail: Value,
    pub result: Option<Value>,
}

/// The job a client operation started, and the digest of its request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalModelOperation {
    pub request_digest: String,
    pub job_id: JobId,
}

fn object_text(value: &Value) -> Result<String, DatabaseError> {
    if !value.is_object() {
        return Err(DatabaseError::Sql(rusqlite::Error::InvalidParameterName(
            "local model job documents are objects".to_owned(),
        )));
    }
    Ok(value.to_string())
}

fn parse_job_id(text: &str) -> rusqlite::Result<JobId> {
    text.parse().map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            "invalid job id".into(),
        )
    })
}

impl Database {
    pub fn local_model_job(
        &self,
        job_id: JobId,
    ) -> Result<Option<LocalModelJobRecord>, DatabaseError> {
        let row: Option<(String, Option<String>)> = self
            .connection()?
            .query_row(
                "SELECT detail_json, result_json FROM local_model_jobs WHERE job_id = ?1",
                params![job_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        Ok(row.and_then(|(detail, result)| {
            Some(LocalModelJobRecord {
                detail: serde_json::from_str(&detail).ok()?,
                result: result.and_then(|result| serde_json::from_str(&result).ok()),
            })
        }))
    }

    /// Records a job's detail unless it has one; returns the detail it has.
    pub fn record_local_model_job(
        &self,
        job_id: JobId,
        detail: &Value,
    ) -> Result<Value, DatabaseError> {
        let text = object_text(detail)?;
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO local_model_jobs (job_id, detail_json) VALUES (?1, ?2)
             ON CONFLICT(job_id) DO NOTHING",
            params![job_id.to_string(), text],
        )?;
        let stored: String = connection.query_row(
            "SELECT detail_json FROM local_model_jobs WHERE job_id = ?1",
            params![job_id.to_string()],
            |row| row.get(0),
        )?;
        Ok(serde_json::from_str(&stored).unwrap_or(Value::Null))
    }

    /// Stores what a job produced; `false` when the job has no detail row.
    pub fn record_local_model_job_result(
        &self,
        job_id: JobId,
        result: &Value,
    ) -> Result<bool, DatabaseError> {
        let text = object_text(result)?;
        Ok(self.connection()?.execute(
            "UPDATE local_model_jobs SET result_json = ?2 WHERE job_id = ?1",
            params![job_id.to_string(), text],
        )? == 1)
    }

    pub fn local_model_operation(
        &self,
        operation_key: &str,
    ) -> Result<Option<LocalModelOperation>, DatabaseError> {
        Ok(self
            .connection()?
            .query_row(
                "SELECT request_digest, job_id FROM local_model_operations
                 WHERE operation_key = ?1",
                params![operation_key],
                |row| {
                    Ok(LocalModelOperation {
                        request_digest: row.get(0)?,
                        job_id: parse_job_id(&row.get::<_, String>(1)?)?,
                    })
                },
            )
            .optional()?)
    }

    /// Ties a client operation to the job it started unless the key is
    /// already taken; returns the operation the key holds.
    pub fn record_local_model_operation(
        &self,
        operation_key: &str,
        request_digest: &str,
        job_id: JobId,
    ) -> Result<LocalModelOperation, DatabaseError> {
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO local_model_operations (operation_key, request_digest, job_id)
             VALUES (?1, ?2, ?3) ON CONFLICT(operation_key) DO NOTHING",
            params![operation_key, request_digest, job_id.to_string()],
        )?;
        Ok(connection.query_row(
            "SELECT request_digest, job_id FROM local_model_operations WHERE operation_key = ?1",
            params![operation_key],
            |row| {
                Ok(LocalModelOperation {
                    request_digest: row.get(0)?,
                    job_id: parse_job_id(&row.get::<_, String>(1)?)?,
                })
            },
        )?)
    }
}

#[cfg(test)]
mod tests {
    use lettuce_jobs::{
        CancellationPolicy, JobKind, JobSpec, JobStore, JobSubject, OutcomeRef, RecoveryPolicy,
        ResourceClass, SubjectKind,
    };
    use lettuce_types::AssetId;
    use serde_json::json;

    use super::*;

    fn job(database: &Database) -> JobId {
        database
            .create_or_get(
                JobSpec::new(
                    JobKind::ArtifactInstall,
                    JobSubject::new(SubjectKind::ArtifactInstall, "local-model").expect("subject"),
                    OutcomeRef::ArtifactInstallation(AssetId::new()),
                )
                .with_resources(vec![ResourceClass::Network])
                .with_policies(
                    RecoveryPolicy::MarkInterrupted,
                    CancellationPolicy::Cooperative,
                ),
            )
            .expect("job")
            .job
            .id
    }

    #[test]
    fn details_are_kept_once_and_operations_keep_their_first_job() {
        let database = Database::open_in_memory().expect("database");
        let first = job(&database);
        assert_eq!(database.local_model_job(first).expect("read"), None);
        assert!(
            !database
                .record_local_model_job_result(first, &json!({"model_path": "/m"}))
                .expect("no row")
        );
        assert_eq!(
            database
                .record_local_model_job(first, &json!({"repo": "org/m"}))
                .expect("detail"),
            json!({"repo": "org/m"})
        );
        assert_eq!(
            database
                .record_local_model_job(first, &json!({"repo": "other"}))
                .expect("kept"),
            json!({"repo": "org/m"})
        );
        assert!(
            database
                .record_local_model_job_result(first, &json!({"model_path": "/m"}))
                .expect("result")
        );
        assert_eq!(
            database.local_model_job(first).expect("read"),
            Some(LocalModelJobRecord {
                detail: json!({"repo": "org/m"}),
                result: Some(json!({"model_path": "/m"})),
            })
        );
        assert!(database.record_local_model_job(first, &json!([])).is_err());
        let second = job(&database);
        assert_eq!(database.local_model_operation("op").expect("read"), None);
        let recorded = database
            .record_local_model_operation("op", "digest-a", first)
            .expect("op");
        assert_eq!(recorded.job_id, first);
        assert_eq!(
            database
                .record_local_model_operation("op", "digest-b", second)
                .expect("taken"),
            recorded
        );
        assert_eq!(
            database.local_model_operation("op").expect("read"),
            Some(recorded)
        );
    }
}
