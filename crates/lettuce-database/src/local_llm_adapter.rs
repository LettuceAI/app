use rusqlite::{OptionalExtension, params};
use serde_json::Value;

use crate::{Database, DatabaseError};

const LLM_METRICS_RETENTION: i64 = 500;

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
        let connection = database.connection().expect("connection");
        let (count, oldest): (i64, i64) = connection
            .query_row(
                "SELECT count(*), min(created_at) FROM llm_generation_metrics",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("count");
        assert_eq!((count, oldest), (500, 2));
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
