//! Legacy local generation metrics (`llm_generation_metrics`), which only the
//! live legacy database holds: archives never carried them. Rows keep their
//! legacy ids and timestamps; a summary that is not a JSON object reads back
//! as `{}` and samples that are not a JSON array as `[]`, the way the legacy
//! reader treated them, and only the newest 500 rows are kept, as both apps
//! retain.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use crate::{
    LegacyBackupConversionNotice, LegacyBackupConversionNoticeKind, LegacyBackupDocumentKind,
    LegacyBackupInventory, LegacyImportSkip, LegacyImportSkipReason, legacy_value_skip,
};

/// How many metrics rows legacy and the new app keep.
pub const LEGACY_LLM_METRICS_RETENTION: usize = 500;
const LLM_METRICS_RECORD_LIMIT: usize = 100_000;

/// One legacy metrics row as the new table stores it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyLlmMetricRecord {
    pub id: String,
    pub created_at: i64,
    pub model_path: Option<String>,
    /// A JSON object.
    pub summary_json: String,
    /// A JSON array.
    pub samples_json: String,
    /// The legacy direct or group message the row was attached to.
    pub message_source_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LegacyBackupLlmMetricsPlan {
    /// Newest first.
    pub metrics: Vec<LegacyLlmMetricRecord>,
    pub notices: Vec<LegacyBackupConversionNotice>,
    pub skipped: Vec<LegacyImportSkip>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LegacyBackupLlmMetricsError {
    #[error("legacy metrics document is malformed")]
    Malformed,
    #[error("legacy metrics document exceeds its record limit")]
    LimitExceeded,
}

const KNOWN_COLUMNS: [&str; 6] = [
    "id",
    "created_at",
    "model_name",
    "summary_json",
    "samples_json",
    "message_id",
];

pub fn plan_legacy_backup_llm_metrics(
    inventory: &LegacyBackupInventory,
) -> Result<LegacyBackupLlmMetricsPlan, LegacyBackupLlmMetricsError> {
    let mut plan = LegacyBackupLlmMetricsPlan::default();
    let Some(document) = inventory
        .documents
        .iter()
        .find(|document| document.kind == LegacyBackupDocumentKind::LlmGenerationMetrics)
    else {
        return Ok(plan);
    };
    let rows: Vec<Map<String, Value>> = serde_json::from_slice(&document.bytes)
        .map_err(|_| LegacyBackupLlmMetricsError::Malformed)?;
    if rows.len() > LLM_METRICS_RECORD_LIMIT {
        return Err(LegacyBackupLlmMetricsError::LimitExceeded);
    }
    let mut ids = BTreeSet::new();
    for (index, row) in rows.into_iter().enumerate() {
        if row.keys().any(|key| !KNOWN_COLUMNS.contains(&key.as_str())) {
            plan.notices.push(LegacyBackupConversionNotice {
                kind: LegacyBackupConversionNoticeKind::Unsupported,
                document: LegacyBackupDocumentKind::LlmGenerationMetrics,
                field: format!("[{index}]"),
            });
        }
        let id = row
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.trim().is_empty())
            .map(str::to_owned);
        let Some(id) = id.filter(|id| ids.insert(id.clone())) else {
            plan.skipped.push(legacy_value_skip(
                "llm_generation_metrics.id",
                &index.to_string(),
                LegacyImportSkipReason::MalformedLegacyValue,
            ));
            continue;
        };
        let Some(created_at) = row.get("created_at").and_then(Value::as_i64) else {
            plan.skipped.push(legacy_value_skip(
                "llm_generation_metrics.created_at",
                &id,
                LegacyImportSkipReason::MalformedLegacyValue,
            ));
            continue;
        };
        let mut skip = |field: &str| {
            plan.skipped.push(legacy_value_skip(
                field,
                &id,
                LegacyImportSkipReason::MalformedLegacyValue,
            ));
        };
        let model_path = optional_text(&row, "model_name", || {
            skip("llm_generation_metrics.model_name");
        });
        let summary_json = json_column(&row, "summary_json", Value::is_object, || {
            skip("llm_generation_metrics.summary_json");
        })
        .unwrap_or_else(|| Value::Object(Map::new()).to_string());
        let samples_json = json_column(&row, "samples_json", Value::is_array, || {
            skip("llm_generation_metrics.samples_json");
        })
        .unwrap_or_else(|| Value::Array(Vec::new()).to_string());
        let message_source_id = optional_text(&row, "message_id", || {
            skip("llm_generation_metrics.message_id");
        })
        .filter(|message| !message.trim().is_empty());
        plan.metrics.push(LegacyLlmMetricRecord {
            id,
            created_at,
            model_path,
            summary_json,
            samples_json,
            message_source_id,
        });
    }
    plan.metrics
        .sort_by(|left, right| (right.created_at, &right.id).cmp(&(left.created_at, &left.id)));
    if plan.metrics.len() > LEGACY_LLM_METRICS_RETENTION {
        plan.metrics.truncate(LEGACY_LLM_METRICS_RETENTION);
        plan.notices.push(LegacyBackupConversionNotice {
            kind: LegacyBackupConversionNoticeKind::Lossy,
            document: LegacyBackupDocumentKind::LlmGenerationMetrics,
            field: "retention".to_owned(),
        });
    }
    plan.notices.sort();
    plan.notices.dedup();
    plan.skipped.sort();
    Ok(plan)
}

fn optional_text(
    row: &Map<String, Value>,
    column: &str,
    mut malformed: impl FnMut(),
) -> Option<String> {
    match row.get(column) {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) => Some(value.clone()),
        Some(_) => {
            malformed();
            None
        }
    }
}

fn json_column(
    row: &Map<String, Value>,
    column: &str,
    accepted: fn(&Value) -> bool,
    mut malformed: impl FnMut(),
) -> Option<String> {
    let parsed = row
        .get(column)
        .and_then(Value::as_str)
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .filter(accepted);
    if parsed.is_none() {
        malformed();
    }
    parsed.map(|value| value.to_string())
}

#[cfg(test)]
mod tests {
    use lettuce_types::ContentHash;
    use serde_json::json;

    use super::*;
    use crate::LegacyBackupDocument;

    fn inventory(rows: &Value) -> LegacyBackupInventory {
        LegacyBackupInventory {
            version: 1,
            created_at: 1,
            app_version: "legacy".into(),
            source_hash: ContentHash::parse("ef".repeat(32)).expect("hash"),
            documents: vec![LegacyBackupDocument {
                kind: LegacyBackupDocumentKind::LlmGenerationMetrics,
                bytes: zeroize::Zeroizing::new(serde_json::to_vec(rows).expect("rows")),
            }],
            media: Vec::new(),
        }
    }

    fn row(id: &str, created_at: i64) -> Value {
        json!({
            "id": id,
            "created_at": created_at,
            "model_name": "/models/a.gguf",
            "summary_json": "{\"completionTokens\":3}",
            "samples_json": "[{\"t\":1}]",
            "message_id": null
        })
    }

    #[test]
    fn rows_keep_their_values_and_unreadable_json_reads_back_like_legacy() {
        let mut linked = row("gen-2", 20);
        linked["message_id"] = json!("message-1");
        let mut broken = row("gen-3", 30);
        broken["summary_json"] = json!("not json");
        broken["samples_json"] = json!("{\"not\":\"an array\"}");
        broken["model_name"] = json!(7);
        let mut listed = row("gen-4", 40);
        listed["summary_json"] = json!("[1]");
        listed.as_object_mut().expect("row").remove("message_id");
        let mut unknown = row("gen-5", 5);
        unknown["future_column"] = json!(true);
        let plan = plan_legacy_backup_llm_metrics(&inventory(&json!([
            row("gen-1", 10),
            linked,
            broken,
            listed,
            unknown,
            row("gen-1", 11),
            row(" ", 12),
            json!({"id": "gen-6", "created_at": "late", "summary_json": "{}"}),
        ])))
        .expect("plan");
        assert_eq!(
            plan.metrics
                .iter()
                .map(|metric| metric.id.as_str())
                .collect::<Vec<_>>(),
            ["gen-4", "gen-3", "gen-2", "gen-1", "gen-5"]
        );
        assert_eq!(
            plan.metrics[2],
            LegacyLlmMetricRecord {
                id: "gen-2".into(),
                created_at: 20,
                model_path: Some("/models/a.gguf".into()),
                summary_json: "{\"completionTokens\":3}".into(),
                samples_json: "[{\"t\":1}]".into(),
                message_source_id: Some("message-1".into()),
            }
        );
        let broken = &plan.metrics[1];
        assert_eq!(
            (
                broken.summary_json.as_str(),
                broken.samples_json.as_str(),
                broken.model_path.as_deref()
            ),
            ("{}", "[]", None)
        );
        assert_eq!(plan.metrics[0].summary_json, "{}");
        assert_eq!(plan.metrics[0].message_source_id, None);
        let skip = |field: &str, row: &str| {
            legacy_value_skip(field, row, LegacyImportSkipReason::MalformedLegacyValue)
        };
        let mut expected = vec![
            skip("llm_generation_metrics.model_name", "gen-3"),
            skip("llm_generation_metrics.summary_json", "gen-3"),
            skip("llm_generation_metrics.samples_json", "gen-3"),
            skip("llm_generation_metrics.summary_json", "gen-4"),
            skip("llm_generation_metrics.id", "5"),
            skip("llm_generation_metrics.id", "6"),
            skip("llm_generation_metrics.created_at", "gen-6"),
        ];
        expected.sort();
        assert_eq!(plan.skipped, expected);
        assert_eq!(
            plan.notices,
            vec![LegacyBackupConversionNotice {
                kind: LegacyBackupConversionNoticeKind::Unsupported,
                document: LegacyBackupDocumentKind::LlmGenerationMetrics,
                field: "[4]".into(),
            }]
        );
    }

    #[test]
    fn only_the_newest_500_rows_are_kept() {
        let rows = (0..520)
            .map(|index| row(&format!("gen-{index:03}"), index / 2))
            .collect::<Vec<_>>();
        let plan = plan_legacy_backup_llm_metrics(&inventory(&Value::Array(rows))).expect("plan");
        assert_eq!(plan.metrics.len(), LEGACY_LLM_METRICS_RETENTION);
        assert_eq!(plan.metrics[0].id, "gen-519");
        assert_eq!(plan.metrics[499].id, "gen-020");
        assert_eq!(
            plan.notices,
            vec![LegacyBackupConversionNotice {
                kind: LegacyBackupConversionNoticeKind::Lossy,
                document: LegacyBackupDocumentKind::LlmGenerationMetrics,
                field: "retention".into(),
            }]
        );
    }

    #[test]
    fn a_source_without_the_table_imports_no_metrics() {
        let mut empty = inventory(&json!([]));
        empty.documents.clear();
        assert_eq!(
            plan_legacy_backup_llm_metrics(&empty).expect("plan"),
            LegacyBackupLlmMetricsPlan::default()
        );
        assert_eq!(
            plan_legacy_backup_llm_metrics(&inventory(&json!({}))),
            Err(LegacyBackupLlmMetricsError::Malformed)
        );
    }
}
