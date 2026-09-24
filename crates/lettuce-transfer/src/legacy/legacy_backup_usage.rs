use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use serde_json::Value;

use crate::{
    LegacyBackupAsrPlan, LegacyBackupConversionNotice, LegacyBackupConversionNoticeKind,
    LegacyBackupDocumentKind,
};

const USAGE_RECORD_LIMIT: usize = 1_000_000;
const METADATA_PER_RECORD_LIMIT: usize = 256;
const METADATA_TOTAL_LIMIT: usize = 4_000_000;
const TEXT_LIMIT: usize = 16_384;

#[derive(Debug)]
pub struct LegacyBackupUsagePlan {
    pub records: Vec<LegacyBackupUsageRecord>,
    pub notices: Vec<LegacyBackupConversionNotice>,
    pub source: LegacyBackupAsrPlan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyBackupUsageRecord {
    pub source_id: String,
    pub timestamp: u64,
    pub session_id: String,
    pub character_id: String,
    pub character_name: String,
    pub model_id: String,
    pub model_name: String,
    pub provider_id: String,
    pub provider_label: String,
    pub operation_type: Option<String>,
    pub finish_reason: Option<String>,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub memory_tokens: Option<u64>,
    pub summary_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub image_tokens: Option<u64>,
    pub audio_tokens: Option<u64>,
    pub prompt_cost: Option<f64>,
    pub completion_cost: Option<f64>,
    pub total_cost: Option<f64>,
    pub success: bool,
    pub error_message: Option<String>,
    pub metadata: BTreeMap<String, String>,
    pub aggregation: LegacyBackupUsageAggregation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyBackupUsageAggregation {
    HistoricalOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LegacyBackupUsageError {
    #[error("legacy backup usage document is malformed")]
    Malformed { field: String },
    #[error("legacy backup usage document exceeds its record limit")]
    LimitExceeded,
}

#[derive(Deserialize)]
struct UsageRow {
    id: String,
    timestamp: i64,
    session_id: String,
    character_id: String,
    character_name: String,
    model_id: String,
    model_name: String,
    provider_id: String,
    provider_label: String,
    operation_type: Option<String>,
    finish_reason: Option<String>,
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    total_tokens: Option<i64>,
    memory_tokens: Option<i64>,
    summary_tokens: Option<i64>,
    reasoning_tokens: Option<i64>,
    image_tokens: Option<i64>,
    audio_tokens: Option<i64>,
    prompt_cost: Option<f64>,
    completion_cost: Option<f64>,
    total_cost: Option<f64>,
    success: bool,
    error_message: Option<String>,
    #[serde(default)]
    metadata: Vec<MetadataRow>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct MetadataRow {
    key: String,
    value: String,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

pub fn plan_legacy_backup_usage(
    source: LegacyBackupAsrPlan,
) -> Result<LegacyBackupUsagePlan, LegacyBackupUsageError> {
    let document = source
        .source
        .authored
        .configuration
        .source
        .documents
        .iter()
        .find(|document| document.kind == LegacyBackupDocumentKind::UsageRecords);
    let mut notices = source.notices.clone();
    let records = match document {
        Some(document) => {
            let rows: Vec<UsageRow> =
                serde_json::from_slice(&document.bytes).map_err(|_| malformed("$"))?;
            map_rows(rows, &mut notices)?
        }
        None => {
            notices.push(notice(LegacyBackupConversionNoticeKind::Absent, "$"));
            Vec::new()
        }
    };
    notices.sort();
    notices.dedup();
    Ok(LegacyBackupUsagePlan {
        records,
        notices,
        source,
    })
}

fn map_rows(
    rows: Vec<UsageRow>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<LegacyBackupUsageRecord>, LegacyBackupUsageError> {
    if rows.len() > USAGE_RECORD_LIMIT {
        return Err(LegacyBackupUsageError::LimitExceeded);
    }
    let mut ids = BTreeSet::new();
    let mut metadata_total = 0_usize;
    let mut records = Vec::with_capacity(rows.len());
    for (index, row) in rows.into_iter().enumerate() {
        let path = format!("[{index}]");
        report_extra(&path, &row.extra, notices);
        validate_text(&row.id, &format!("{path}.id"), false)?;
        if !ids.insert(row.id.clone()) {
            return Err(malformed(format!("{path}.id")));
        }
        for (field, value) in [
            ("session_id", row.session_id.as_str()),
            ("character_id", row.character_id.as_str()),
            ("character_name", row.character_name.as_str()),
            ("model_id", row.model_id.as_str()),
            ("model_name", row.model_name.as_str()),
            ("provider_id", row.provider_id.as_str()),
            ("provider_label", row.provider_label.as_str()),
        ] {
            validate_text(value, &format!("{path}.{field}"), true)?;
        }
        for (field, value) in [
            ("operation_type", row.operation_type.as_deref()),
            ("finish_reason", row.finish_reason.as_deref()),
            ("error_message", row.error_message.as_deref()),
        ] {
            if let Some(value) = value {
                validate_text(value, &format!("{path}.{field}"), true)?;
            }
        }
        let timestamp =
            u64::try_from(row.timestamp).map_err(|_| malformed(format!("{path}.timestamp")))?;
        let metadata = map_metadata(row.metadata, &path, notices)?;
        metadata_total = metadata_total
            .checked_add(metadata.len())
            .ok_or(LegacyBackupUsageError::LimitExceeded)?;
        if metadata_total > METADATA_TOTAL_LIMIT {
            return Err(LegacyBackupUsageError::LimitExceeded);
        }
        records.push(LegacyBackupUsageRecord {
            source_id: row.id,
            timestamp,
            session_id: row.session_id,
            character_id: row.character_id,
            character_name: row.character_name,
            model_id: row.model_id,
            model_name: row.model_name,
            provider_id: row.provider_id,
            provider_label: row.provider_label,
            operation_type: row.operation_type,
            finish_reason: row.finish_reason,
            prompt_tokens: optional_count(row.prompt_tokens, &format!("{path}.prompt_tokens"))?,
            completion_tokens: optional_count(
                row.completion_tokens,
                &format!("{path}.completion_tokens"),
            )?,
            total_tokens: optional_count(row.total_tokens, &format!("{path}.total_tokens"))?,
            memory_tokens: optional_count(row.memory_tokens, &format!("{path}.memory_tokens"))?,
            summary_tokens: optional_count(row.summary_tokens, &format!("{path}.summary_tokens"))?,
            reasoning_tokens: optional_count(
                row.reasoning_tokens,
                &format!("{path}.reasoning_tokens"),
            )?,
            image_tokens: optional_count(row.image_tokens, &format!("{path}.image_tokens"))?,
            audio_tokens: optional_count(row.audio_tokens, &format!("{path}.audio_tokens"))?,
            prompt_cost: finite_cost(row.prompt_cost, &format!("{path}.prompt_cost"))?,
            completion_cost: finite_cost(row.completion_cost, &format!("{path}.completion_cost"))?,
            total_cost: finite_cost(row.total_cost, &format!("{path}.total_cost"))?,
            success: row.success,
            error_message: row.error_message,
            metadata,
            aggregation: LegacyBackupUsageAggregation::HistoricalOnly,
        });
    }
    if !records.is_empty() {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Lossy,
            "[].currentUsageOwnership",
        ));
    }
    Ok(records)
}

fn map_metadata(
    rows: Vec<MetadataRow>,
    path: &str,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<BTreeMap<String, String>, LegacyBackupUsageError> {
    if rows.len() > METADATA_PER_RECORD_LIMIT {
        return Err(LegacyBackupUsageError::LimitExceeded);
    }
    let mut metadata = BTreeMap::new();
    for (index, row) in rows.into_iter().enumerate() {
        let item_path = format!("{path}.metadata[{index}]");
        report_extra(&item_path, &row.extra, notices);
        validate_text(&row.key, &format!("{item_path}.key"), false)?;
        validate_text(&row.value, &format!("{item_path}.value"), true)?;
        validate_metadata_value(&row.key, &row.value, &format!("{item_path}.value"))?;
        if metadata.insert(row.key, row.value).is_some() {
            return Err(malformed(format!("{item_path}.key")));
        }
    }
    Ok(metadata)
}

fn validate_metadata_value(
    key: &str,
    value: &str,
    field: &str,
) -> Result<(), LegacyBackupUsageError> {
    let counter = matches!(
        key,
        "cached_prompt_tokens"
            | "openrouter_cached_prompt_tokens"
            | "cache_write_tokens"
            | "web_search_requests"
            | "cost_regular_prompt_tokens"
            | "cost_cached_prompt_tokens"
            | "cost_cache_write_tokens"
            | "cost_reasoning_tokens"
            | "cost_web_search_requests"
    );
    let cost = matches!(
        key,
        "api_cost"
            | "openrouter_api_cost"
            | "cost_prompt_base"
            | "cost_cache_read"
            | "cost_cache_write"
            | "cost_completion_base"
            | "cost_reasoning"
            | "cost_request"
            | "cost_web_search"
            | "cost_authoritative_total"
            | "openrouter_authoritative_total_cost"
    );
    if counter && value.parse::<u64>().is_err()
        || cost
            && value
                .parse::<f64>()
                .map_or(true, |parsed| !parsed.is_finite())
    {
        return Err(malformed(field));
    }
    Ok(())
}

fn validate_text(
    value: &str,
    field: &str,
    allow_empty: bool,
) -> Result<(), LegacyBackupUsageError> {
    if (!allow_empty && value.is_empty())
        || value.chars().count() > TEXT_LIMIT
        || value.contains('\0')
    {
        Err(malformed(field))
    } else {
        Ok(())
    }
}

fn optional_count(value: Option<i64>, field: &str) -> Result<Option<u64>, LegacyBackupUsageError> {
    value
        .map(|value| u64::try_from(value).map_err(|_| malformed(field)))
        .transpose()
}

fn finite_cost(value: Option<f64>, field: &str) -> Result<Option<f64>, LegacyBackupUsageError> {
    if value.is_some_and(|value| !value.is_finite()) {
        Err(malformed(field))
    } else {
        Ok(value)
    }
}

fn report_extra(
    path: &str,
    extra: &BTreeMap<String, Value>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) {
    for field in extra.keys() {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Unsupported,
            &format!("{path}.{field}"),
        ));
    }
}

fn notice(kind: LegacyBackupConversionNoticeKind, field: &str) -> LegacyBackupConversionNotice {
    LegacyBackupConversionNotice {
        kind,
        document: LegacyBackupDocumentKind::UsageRecords,
        field: field.to_owned(),
    }
}

fn malformed(field: impl Into<String>) -> LegacyBackupUsageError {
    LegacyBackupUsageError::Malformed {
        field: field.into(),
    }
}

#[cfg(test)]
mod tests {
    use lettuce_types::ContentHash;
    use serde_json::{Value, json};
    use zeroize::Zeroizing;

    use super::*;
    use crate::{
        LegacyBackupDocument, LegacyBackupInventory, plan_legacy_backup_asr,
        plan_legacy_backup_authored, plan_legacy_backup_authored_media,
        plan_legacy_backup_configuration,
    };

    fn document(value: Value) -> LegacyBackupDocument {
        LegacyBackupDocument {
            kind: LegacyBackupDocumentKind::UsageRecords,
            bytes: Zeroizing::new(serde_json::to_vec(&value).expect("fixture document")),
        }
    }

    fn source(documents: Vec<LegacyBackupDocument>) -> LegacyBackupAsrPlan {
        let inventory = LegacyBackupInventory {
            version: 1,
            created_at: 1,
            app_version: "legacy".into(),
            source_hash: ContentHash::parse("55".repeat(32)).expect("source hash"),
            documents,
            media: Vec::new(),
        };
        let configuration =
            plan_legacy_backup_configuration(inventory).expect("configuration plan");
        let authored = plan_legacy_backup_authored(configuration).expect("authored plan");
        let media = plan_legacy_backup_authored_media(authored).expect("media plan");
        plan_legacy_backup_asr(media).expect("ASR plan")
    }

    fn record() -> Value {
        json!({
            "id": "request-1",
            "timestamp": 1_780_000_000_000_i64,
            "session_id": "legacy-session",
            "character_id": "legacy-character",
            "character_name": "Mira",
            "model_id": "legacy-model",
            "model_name": "Model",
            "provider_id": "openrouter",
            "provider_label": "OpenRouter",
            "operation_type": "chat",
            "finish_reason": "stop",
            "prompt_tokens": 12,
            "completion_tokens": 8,
            "total_tokens": 20,
            "memory_tokens": 3,
            "summary_tokens": 2,
            "reasoning_tokens": 1,
            "image_tokens": 0,
            "audio_tokens": 4,
            "prompt_cost": 0.01,
            "completion_cost": -0.002,
            "total_cost": 0.008,
            "success": true,
            "error_message": null,
            "metadata": [
                {"key": "cached_prompt_tokens", "value": "5"},
                {"key": "cost_authoritative_total", "value": "0.008"},
                {"key": "provider_request_id", "value": "remote-1"}
            ],
            "future_field": "retained as a notice"
        })
    }

    #[test]
    fn backup_usage_preserves_historical_evidence_without_live_ownership() {
        let plan = plan_legacy_backup_usage(source(vec![document(json!([record()]))]))
            .expect("usage plan");
        assert_eq!(plan.records.len(), 1);
        let item = &plan.records[0];
        assert_eq!(item.source_id, "request-1");
        assert_eq!(item.prompt_tokens, Some(12));
        assert_eq!(item.completion_cost, Some(-0.002));
        assert_eq!(item.metadata["cached_prompt_tokens"], "5");
        assert_eq!(
            item.aggregation,
            LegacyBackupUsageAggregation::HistoricalOnly
        );
        assert!(plan.notices.iter().any(|notice| {
            notice.kind == LegacyBackupConversionNoticeKind::Lossy
                && notice.document == LegacyBackupDocumentKind::UsageRecords
                && notice.field == "[].currentUsageOwnership"
        }));
        assert!(plan.notices.iter().any(|notice| {
            notice.kind == LegacyBackupConversionNoticeKind::Unsupported
                && notice.field == "[0].future_field"
        }));
        assert_eq!(
            plan.source
                .source
                .authored
                .configuration
                .source
                .documents
                .len(),
            1
        );
    }

    #[test]
    fn missing_usage_document_is_explicit_and_empty() {
        let plan = plan_legacy_backup_usage(source(Vec::new())).expect("usage plan");
        assert!(plan.records.is_empty());
        assert!(plan.notices.iter().any(|notice| {
            notice.kind == LegacyBackupConversionNoticeKind::Absent
                && notice.document == LegacyBackupDocumentKind::UsageRecords
                && notice.field == "$"
        }));
    }

    #[test]
    fn invalid_counts_duplicate_ids_and_metadata_fail_closed() {
        let mut negative = record();
        negative["prompt_tokens"] = json!(-1);
        assert!(matches!(
            plan_legacy_backup_usage(source(vec![document(json!([negative]))])),
            Err(LegacyBackupUsageError::Malformed { ref field })
                if field == "[0].prompt_tokens"
        ));

        let duplicate = record();
        assert!(matches!(
            plan_legacy_backup_usage(source(vec![document(json!([
                duplicate.clone(), duplicate
            ]))])),
            Err(LegacyBackupUsageError::Malformed { ref field }) if field == "[1].id"
        ));

        let mut invalid_metadata = record();
        invalid_metadata["metadata"][0]["value"] = json!("-5");
        assert!(matches!(
            plan_legacy_backup_usage(source(vec![document(json!([invalid_metadata]))])),
            Err(LegacyBackupUsageError::Malformed { ref field })
                if field == "[0].metadata[0].value"
        ));
    }
}
