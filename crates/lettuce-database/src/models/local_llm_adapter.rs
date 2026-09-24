use rusqlite::{OptionalExtension, params};
use serde_json::Value;

use crate::{Database, DatabaseError};

const LLM_METRICS_RETENTION: i64 = 500;
const LLM_METRICS_DEFAULT_LIMIT: usize = 500;
const LLM_METRICS_MAX_LIMIT: usize = 5000;

/// One local generation's recorded metrics.
#[derive(Debug, Clone, PartialEq)]
pub struct LlmGenerationMetric {
    pub id: String,
    pub created_at: i64,
    pub model_path: Option<String>,
    pub summary: Value,
    /// Present when one metric is read; lists leave the samples out.
    pub samples: Option<Vec<Value>>,
}

fn json_or(text: &str, fallback: Value) -> Value {
    serde_json::from_str(text).unwrap_or(fallback)
}

fn metric_with_samples(row: &rusqlite::Row<'_>) -> rusqlite::Result<LlmGenerationMetric> {
    let samples: String = row.get(4)?;
    Ok(LlmGenerationMetric {
        id: row.get(0)?,
        created_at: row.get(1)?,
        model_path: row.get(2)?,
        summary: json_or(
            &row.get::<_, String>(3)?,
            Value::Object(serde_json::Map::new()),
        ),
        samples: Some(match json_or(&samples, Value::Array(Vec::new())) {
            Value::Array(samples) => samples,
            _ => Vec::new(),
        }),
    })
}

const NEWEST_LLAMA_MODEL: &str = "SELECT model.id FROM model_profiles model
     JOIN provider_accounts account ON account.id = model.provider_account_id
     WHERE account.protocol = 'llama_cpp' AND model.external_model_id = ?1
     ORDER BY model.created_at DESC, model.id DESC
     LIMIT 1";

impl Database {
    /// The last runtime report of the newest llama.cpp model using this file
    /// (legacy `llamaLastRuntimeReport` of that model), if it was written for
    /// this file.
    pub fn llama_runtime_report(&self, model_path: &str) -> Result<Option<Value>, DatabaseError> {
        if model_path == lettuce_models::UNPICKED_LOCAL_MODEL_FILE {
            return Ok(None);
        }
        let connection = self.connection()?;
        let report: Option<String> = connection
            .query_row(
                &format!(
                    "SELECT report.report_json FROM llama_runtime_reports report
                     WHERE report.model_profile_id = ({NEWEST_LLAMA_MODEL})
                       AND report.model_path = ?1"
                ),
                params![model_path],
                |row| row.get(0),
            )
            .optional()?;
        Ok(report.and_then(|report| serde_json::from_str(&report).ok()))
    }

    /// Drops the cached smart-offload layer count from every llama.cpp
    /// runtime report, keeping the rest of each report; returns how many
    /// reports had one.
    pub fn clear_llama_layer_caches(&self) -> Result<usize, DatabaseError> {
        Ok(self.connection()?.execute(
            "UPDATE llama_runtime_reports
             SET report_json = json_remove(report_json, '$.actualGpuLayersUsed')
             WHERE json_extract(report_json, '$.actualGpuLayersUsed') IS NOT NULL",
            [],
        )?)
    }

    /// Stores the report on the newest llama.cpp model using this file;
    /// `false` when no such model exists.
    pub fn store_llama_runtime_report(
        &self,
        model_path: &str,
        report: &Value,
        now: i64,
    ) -> Result<bool, DatabaseError> {
        if model_path == lettuce_models::UNPICKED_LOCAL_MODEL_FILE {
            return Ok(false);
        }
        let connection = self.connection()?;
        let model_id: Option<String> = connection
            .query_row(NEWEST_LLAMA_MODEL, params![model_path], |row| row.get(0))
            .optional()?;
        let Some(model_id) = model_id else {
            return Ok(false);
        };
        connection.execute(
            "INSERT INTO llama_runtime_reports (model_profile_id, model_path, report_json, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (model_profile_id) DO UPDATE
             SET model_path = excluded.model_path, report_json = excluded.report_json,
                 updated_at = excluded.updated_at",
            params![model_id, model_path, report.to_string(), now],
        )?;
        Ok(true)
    }

    /// Records one local generation's metrics, keeping the newest 500 as
    /// legacy did.
    pub fn record_llm_generation_metrics(
        &self,
        id: &str,
        model_path: Option<&str>,
        summary: &Value,
        samples: &[Value],
        now: i64,
    ) -> Result<(), DatabaseError> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT OR REPLACE INTO llm_generation_metrics
                (id, created_at, model_path, summary_json, samples_json)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                id,
                now,
                model_path,
                summary.to_string(),
                Value::Array(samples.to_vec()).to_string()
            ],
        )?;
        transaction.execute(
            "DELETE FROM llm_generation_metrics WHERE id NOT IN (
                SELECT id FROM llm_generation_metrics ORDER BY created_at DESC LIMIT ?1
             )",
            params![LLM_METRICS_RETENTION],
        )?;
        transaction.commit()?;
        Ok(())
    }
}

impl Database {
    /// The newest recorded metrics, without samples; `limit` defaults to 500
    /// and is kept within 1..=5000.
    pub fn llm_generation_metrics(
        &self,
        limit: Option<usize>,
    ) -> Result<Vec<LlmGenerationMetric>, DatabaseError> {
        let limit = limit
            .unwrap_or(LLM_METRICS_DEFAULT_LIMIT)
            .clamp(1, LLM_METRICS_MAX_LIMIT);
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, created_at, model_path, summary_json FROM llm_generation_metrics
             ORDER BY created_at DESC, id DESC LIMIT ?1",
        )?;
        let rows =
            statement.query_map(params![i64::try_from(limit).unwrap_or(i64::MAX)], |row| {
                Ok(LlmGenerationMetric {
                    id: row.get(0)?,
                    created_at: row.get(1)?,
                    model_path: row.get(2)?,
                    summary: json_or(
                        &row.get::<_, String>(3)?,
                        Value::Object(serde_json::Map::new()),
                    ),
                    samples: None,
                })
            })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn llm_generation_metric(
        &self,
        id: &str,
    ) -> Result<Option<LlmGenerationMetric>, DatabaseError> {
        Ok(self
            .connection()?
            .query_row(
                "SELECT id, created_at, model_path, summary_json, samples_json
                 FROM llm_generation_metrics WHERE id = ?1",
                params![id],
                metric_with_samples,
            )
            .optional()?)
    }

    /// The newest metrics of any generation that produced a candidate of the
    /// message: a local generation records its metrics under its attempt id.
    pub fn llm_generation_metric_for_message(
        &self,
        conversation_id: &str,
        message_id: &str,
    ) -> Result<Option<LlmGenerationMetric>, DatabaseError> {
        Ok(self
            .connection()?
            .query_row(
                "SELECT metric.id, metric.created_at, metric.model_path, metric.summary_json,
                        metric.samples_json
                 FROM llm_generation_metrics metric
                 JOIN conversation_message_candidates candidate
                   ON candidate.attempt_id = metric.id
                 WHERE candidate.conversation_id = ?1 AND candidate.message_id = ?2
                 ORDER BY metric.created_at DESC, metric.id DESC
                 LIMIT 1",
                params![conversation_id, message_id],
                metric_with_samples,
            )
            .optional()?)
    }

    /// Deletes every recorded metric; returns how many there were.
    pub fn clear_llm_generation_metrics(&self) -> Result<usize, DatabaseError> {
        Ok(self
            .connection()?
            .execute("DELETE FROM llm_generation_metrics", [])?)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Database;

    #[test]
    fn metrics_keep_the_newest_five_hundred() {
        let database = Database::open_in_memory().expect("database");
        for index in 0..502_i64 {
            database
                .record_llm_generation_metrics(
                    &format!("gen-{index}"),
                    Some("/models/a.gguf"),
                    &json!({"completionTokens": index}),
                    &[json!({"tMs": 1})],
                    index,
                )
                .expect("record");
        }
        let (count, oldest): (i64, i64) = database
            .connection()
            .expect("connection")
            .query_row(
                "SELECT count(*), min(created_at) FROM llm_generation_metrics",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("count");
        assert_eq!((count, oldest), (500, 2));
        let newest = database.llm_generation_metrics(Some(0)).expect("list");
        assert_eq!(newest.len(), 1);
        assert_eq!(newest[0].id, "gen-501");
        assert_eq!(newest[0].samples, None);
        assert_eq!(
            database.llm_generation_metrics(None).expect("list").len(),
            500
        );
        let one = database
            .llm_generation_metric("gen-501")
            .expect("get")
            .expect("metric");
        assert_eq!(one.summary, json!({"completionTokens": 501}));
        assert_eq!(one.samples, Some(vec![json!({"tMs": 1})]));
        assert_eq!(database.llm_generation_metric("gen-0").expect("get"), None);
        assert_eq!(
            database
                .llm_generation_metric_for_message("conversation", "message")
                .expect("by message"),
            None
        );
        assert_eq!(database.clear_llm_generation_metrics().expect("clear"), 500);
        assert!(
            database
                .llm_generation_metrics(None)
                .expect("list")
                .is_empty()
        );
    }

    #[test]
    fn reports_need_a_llama_model_for_the_file() {
        let database = Database::open_in_memory().expect("database");
        assert!(
            !database
                .store_llama_runtime_report(
                    "/models/missing.gguf",
                    &json!({"status": "succeeded"}),
                    1
                )
                .expect("store")
        );
        assert_eq!(
            database
                .llama_runtime_report("/models/missing.gguf")
                .expect("load"),
            None
        );
    }

    #[test]
    fn reports_live_on_the_newest_llama_model_for_the_file() {
        let database = Database::open_in_memory().expect("database");
        {
            let connection = database.connection().expect("connection");
            connection
                .execute_batch(
                    "INSERT INTO provider_accounts (id, provider_kind, protocol, label, endpoint, enabled, streaming_enabled, allow_invalid_tls, api_key_secret_ref, secret_owner_id, secret_headers_json, config_json, revision, created_at, updated_at)
                     VALUES ('account', 'llamacpp', 'llama_cpp', 'llama.cpp', NULL, 1, 1, 0, NULL, 'owner', '[]', '{}', 1, 1, 1);
                     INSERT INTO model_profiles (id, provider_account_id, external_model_id, display_name, kind, config_json, revision, created_at, updated_at)
                     VALUES ('old', 'account', '/models/a.gguf', 'A', 'chat', '{}', 1, 1, 1),
                            ('new', 'account', '/models/a.gguf', 'A2', 'chat', '{}', 1, 2, 2);",
                )
                .expect("seed");
        }
        let report = json!({"status": "succeeded", "actualGpuLayersUsed": 12});
        assert!(
            database
                .store_llama_runtime_report("/models/a.gguf", &report, 5)
                .expect("store")
        );
        assert_eq!(
            database
                .llama_runtime_report("/models/a.gguf")
                .expect("load"),
            Some(report)
        );
        let owner: String = database
            .connection()
            .expect("connection")
            .query_row(
                "SELECT model_profile_id FROM llama_runtime_reports",
                [],
                |row| row.get(0),
            )
            .expect("owner");
        assert_eq!(owner, "new");
        assert_eq!(database.clear_llama_layer_caches().expect("clear"), 1);
        assert_eq!(
            database
                .llama_runtime_report("/models/a.gguf")
                .expect("load"),
            Some(json!({"status": "succeeded"}))
        );
        assert_eq!(database.clear_llama_layer_caches().expect("clear again"), 0);
        database
            .connection()
            .expect("connection")
            .execute("DELETE FROM model_profiles WHERE id = 'new'", [])
            .expect("delete model");
        assert_eq!(
            database
                .llama_runtime_report("/models/a.gguf")
                .expect("load"),
            None
        );
    }

    #[test]
    fn unpicked_synced_models_have_no_report() {
        let database = Database::open_in_memory().expect("database");
        let placeholder = lettuce_models::UNPICKED_LOCAL_MODEL_FILE;
        assert!(
            !database
                .store_llama_runtime_report(placeholder, &json!({}), 1)
                .expect("store")
        );
        assert_eq!(
            database.llama_runtime_report(placeholder).expect("load"),
            None
        );
    }
}
