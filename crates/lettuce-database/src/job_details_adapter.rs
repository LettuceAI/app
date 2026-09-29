use lettuce_types::JobId;
use rusqlite::{OptionalExtension, params};
use serde_json::Value;

use crate::{Database, DatabaseError};

/// What a text feature or manual image job works on and
/// what it produced, as JSON objects the application defines.
#[derive(Debug, Clone, PartialEq)]
pub struct JobDetailRecord {
    pub detail: Value,
    pub result: Option<Value>,
    pub failure: Option<Value>,
}

/// The job a client operation started, and the digest of its request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobOperation {
    pub request_digest: String,
    pub job_id: JobId,
}

fn object_text(value: &Value) -> Result<String, DatabaseError> {
    if !value.is_object() {
        return Err(DatabaseError::Sql(rusqlite::Error::InvalidParameterName(
            "generic job documents are objects".to_owned(),
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

fn parse_value(text: Option<String>) -> rusqlite::Result<Option<Value>> {
    text.map(|text| {
        serde_json::from_str(&text).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, error.into())
        })
    })
    .transpose()
}

/// Every generic job's stored detail, result and failure, for a backup.
pub(crate) fn jobs_in(
    connection: &rusqlite::Connection,
) -> rusqlite::Result<Vec<lettuce_transfer::BackupJobDetail>> {
    let mut statement = connection.prepare(
        "SELECT job_id, detail_json, result_json, failure_json FROM job_details
         ORDER BY job_id",
    )?;
    statement
        .query_map([], |row| {
            Ok(lettuce_transfer::BackupJobDetail {
                job_id: parse_job_id(&row.get::<_, String>(0)?)?,
                detail: parse_value(Some(row.get(1)?))?.unwrap_or(Value::Null),
                result: parse_value(row.get(2)?)?,
                failure: parse_value(row.get(3)?)?,
            })
        })?
        .collect()
}

/// Every client operation, for a backup.
pub(crate) fn operations_in(
    connection: &rusqlite::Connection,
) -> rusqlite::Result<Vec<lettuce_transfer::BackupJobOperation>> {
    let mut statement = connection.prepare(
        "SELECT operation_key, request_digest, job_id FROM job_operations
         ORDER BY operation_key",
    )?;
    statement
        .query_map([], |row| {
            Ok(lettuce_transfer::BackupJobOperation {
                operation_key: row.get(0)?,
                request_digest: row.get(1)?,
                job_id: parse_job_id(&row.get::<_, String>(2)?)?,
            })
        })?
        .collect()
}

/// Writes a restored backup's generic job rows.
pub(crate) fn insert_restored_in(
    connection: &rusqlite::Connection,
    backup: &lettuce_transfer::JobBackup,
) -> Result<(), DatabaseError> {
    for job in &backup.job_details {
        connection.execute(
            "INSERT INTO job_details (job_id, detail_json, result_json, failure_json)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                job.job_id.to_string(),
                object_text(&job.detail)?,
                job.result.as_ref().map(object_text).transpose()?,
                job.failure.as_ref().map(object_text).transpose()?,
            ],
        )?;
    }
    for operation in &backup.job_operations {
        connection.execute(
            "INSERT INTO job_operations (operation_key, request_digest, job_id)
             VALUES (?1, ?2, ?3)",
            params![
                operation.operation_key,
                operation.request_digest,
                operation.job_id.to_string()
            ],
        )?;
    }
    Ok(())
}

impl Database {
    pub fn job_detail(&self, job_id: JobId) -> Result<Option<JobDetailRecord>, DatabaseError> {
        let row: Option<(String, Option<String>, Option<String>)> = self
            .connection()?
            .query_row(
                "SELECT detail_json, result_json, failure_json FROM job_details
                 WHERE job_id = ?1",
                params![job_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        row.map(|(detail, result, failure)| {
            Ok(JobDetailRecord {
                detail: serde_json::from_str(&detail)
                    .map_err(|_| DatabaseError::Sql(rusqlite::Error::InvalidQuery))?,
                result: parse_value(result)?,
                failure: parse_value(failure)?,
            })
        })
        .transpose()
    }

    /// Records a job's detail unless it has one; returns the detail it has.
    pub fn record_job_detail(&self, job_id: JobId, detail: &Value) -> Result<Value, DatabaseError> {
        let text = object_text(detail)?;
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO job_details (job_id, detail_json) VALUES (?1, ?2)
             ON CONFLICT(job_id) DO NOTHING",
            params![job_id.to_string(), text],
        )?;
        let stored: String = connection.query_row(
            "SELECT detail_json FROM job_details WHERE job_id = ?1",
            params![job_id.to_string()],
            |row| row.get(0),
        )?;
        serde_json::from_str(&stored).map_err(|_| DatabaseError::Sql(rusqlite::Error::InvalidQuery))
    }

    /// Stores what a job produced; `false` when the job has no detail row.
    pub fn record_job_detail_result(
        &self,
        job_id: JobId,
        result: &Value,
    ) -> Result<bool, DatabaseError> {
        let text = object_text(result)?;
        Ok(self.connection()?.execute(
            "UPDATE job_details SET result_json = ?2 WHERE job_id = ?1",
            params![job_id.to_string(), text],
        )? == 1)
    }

    /// Stores why a job failed; `false` when the job has no detail row.
    pub fn record_job_detail_failure(
        &self,
        job_id: JobId,
        failure: &Value,
    ) -> Result<bool, DatabaseError> {
        let text = object_text(failure)?;
        Ok(self.connection()?.execute(
            "UPDATE job_details SET failure_json = ?2 WHERE job_id = ?1",
            params![job_id.to_string(), text],
        )? == 1)
    }

    pub fn job_operation(
        &self,
        operation_key: &str,
    ) -> Result<Option<JobOperation>, DatabaseError> {
        Ok(self
            .connection()?
            .query_row(
                "SELECT request_digest, job_id FROM job_operations
                 WHERE operation_key = ?1",
                params![operation_key],
                |row| {
                    Ok(JobOperation {
                        request_digest: row.get(0)?,
                        job_id: parse_job_id(&row.get::<_, String>(1)?)?,
                    })
                },
            )
            .optional()?)
    }

    /// Ties a client operation to the job it started unless the key is
    /// already taken; returns the operation the key holds.
    pub fn record_job_operation(
        &self,
        operation_key: &str,
        request_digest: &str,
        job_id: JobId,
    ) -> Result<JobOperation, DatabaseError> {
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO job_operations (operation_key, request_digest, job_id)
             VALUES (?1, ?2, ?3) ON CONFLICT(operation_key) DO NOTHING",
            params![operation_key, request_digest, job_id.to_string()],
        )?;
        Ok(connection.query_row(
            "SELECT request_digest, job_id FROM job_operations WHERE operation_key = ?1",
            params![operation_key],
            |row| {
                Ok(JobOperation {
                    request_digest: row.get(0)?,
                    job_id: parse_job_id(&row.get::<_, String>(1)?)?,
                })
            },
        )?)
    }
}
