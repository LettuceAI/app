use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use serde_json::Value;

use crate::{
    LegacyBackupConversionNotice, LegacyBackupConversionNoticeKind, LegacyBackupDocumentKind,
    LegacyBackupUsagePlan,
};

const PRICING_CACHE_LIMIT: usize = 10_000;
const MODEL_ID_LIMIT: usize = 1_024;
const PRICING_JSON_LIMIT: usize = 64 * 1_024;

#[derive(Debug)]
pub struct LegacyBackupPricingPlan {
    pub entries: Vec<LegacyBackupPricingEntry>,
    pub notices: Vec<LegacyBackupConversionNotice>,
    pub source: LegacyBackupUsagePlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyBackupPricingEntry {
    pub model_id: String,
    pub pricing_json: Option<String>,
    pub cached_at: u64,
    pub disposition: LegacyBackupPricingDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyBackupPricingDisposition {
    HistoricalOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LegacyBackupPricingError {
    #[error("legacy backup pricing cache is malformed")]
    Malformed { field: String },
    #[error("legacy backup pricing cache exceeds its record limit")]
    LimitExceeded,
}

#[derive(Deserialize)]
struct CacheRow {
    model_id: String,
    pricing_json: Option<String>,
    cached_at: i64,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PricingDocument {
    prompt: String,
    completion: String,
    #[serde(default)]
    request: String,
    #[serde(default)]
    image: String,
    #[serde(default)]
    image_output: String,
    #[serde(default)]
    web_search: String,
    #[serde(default)]
    internal_reasoning: String,
    #[serde(default)]
    input_cache_read: String,
    #[serde(default)]
    input_cache_write: String,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

pub fn plan_legacy_backup_pricing(
    source: LegacyBackupUsagePlan,
) -> Result<LegacyBackupPricingPlan, LegacyBackupPricingError> {
    let document = source
        .source
        .source
        .authored
        .configuration
        .source
        .documents
        .iter()
        .find(|document| document.kind == LegacyBackupDocumentKind::ModelPricingCache);
    let mut notices = source.notices.clone();
    let entries = match document {
        Some(document) => {
            let rows: Vec<CacheRow> =
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
    Ok(LegacyBackupPricingPlan {
        entries,
        notices,
        source,
    })
}

fn map_rows(
    rows: Vec<CacheRow>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<LegacyBackupPricingEntry>, LegacyBackupPricingError> {
    if rows.len() > PRICING_CACHE_LIMIT {
        return Err(LegacyBackupPricingError::LimitExceeded);
    }
    let mut model_ids = BTreeSet::new();
    let mut entries = Vec::with_capacity(rows.len());
    for (index, row) in rows.into_iter().enumerate() {
        let path = format!("[{index}]");
        report_extra(&path, &row.extra, notices);
        if row.model_id.trim().is_empty()
            || row.model_id.trim() != row.model_id
            || row.model_id.chars().count() > MODEL_ID_LIMIT
            || row.model_id.chars().any(char::is_control)
            || !model_ids.insert(row.model_id.clone())
        {
            return Err(malformed(format!("{path}.model_id")));
        }
        let cached_at =
            u64::try_from(row.cached_at).map_err(|_| malformed(format!("{path}.cached_at")))?;
        if let Some(raw) = &row.pricing_json {
            if raw.len() > PRICING_JSON_LIMIT {
                return Err(LegacyBackupPricingError::LimitExceeded);
            }
            let pricing: PricingDocument =
                serde_json::from_str(raw).map_err(|_| malformed(format!("{path}.pricing_json")))?;
            validate_pricing(&pricing, &path)?;
            report_extra(&format!("{path}.pricing_json"), &pricing.extra, notices);
        }
        entries.push(LegacyBackupPricingEntry {
            model_id: row.model_id,
            pricing_json: row.pricing_json,
            cached_at,
            disposition: LegacyBackupPricingDisposition::HistoricalOnly,
        });
    }
    if !entries.is_empty() {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Lossy,
            "[].currentEndpointIdentity",
        ));
    }
    Ok(entries)
}

fn validate_pricing(pricing: &PricingDocument, path: &str) -> Result<(), LegacyBackupPricingError> {
    for (field, value, required) in [
        ("prompt", pricing.prompt.as_str(), true),
        ("completion", pricing.completion.as_str(), true),
        ("request", pricing.request.as_str(), false),
        ("image", pricing.image.as_str(), false),
        ("imageOutput", pricing.image_output.as_str(), false),
        ("webSearch", pricing.web_search.as_str(), false),
        (
            "internalReasoning",
            pricing.internal_reasoning.as_str(),
            false,
        ),
        ("inputCacheRead", pricing.input_cache_read.as_str(), false),
        ("inputCacheWrite", pricing.input_cache_write.as_str(), false),
    ] {
        if value.is_empty() && !required {
            continue;
        }
        if !value
            .parse::<f64>()
            .is_ok_and(|parsed| parsed.is_finite() && parsed >= 0.0)
        {
            return Err(malformed(format!("{path}.pricing_json.{field}")));
        }
    }
    Ok(())
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
        document: LegacyBackupDocumentKind::ModelPricingCache,
        field: field.to_owned(),
    }
}

fn malformed(field: impl Into<String>) -> LegacyBackupPricingError {
    LegacyBackupPricingError::Malformed {
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
        plan_legacy_backup_configuration, plan_legacy_backup_usage,
    };

    fn source(value: Option<Value>) -> LegacyBackupUsagePlan {
        let documents = value.into_iter().map(|value| LegacyBackupDocument {
            kind: LegacyBackupDocumentKind::ModelPricingCache,
            bytes: Zeroizing::new(serde_json::to_vec(&value).expect("fixture document")),
        });
        let inventory = LegacyBackupInventory {
            version: 1,
            created_at: 1,
            app_version: "legacy".into(),
            source_hash: ContentHash::parse("66".repeat(32)).expect("source hash"),
            documents: documents.collect(),
            media: Vec::new(),
        };
        let configuration =
            plan_legacy_backup_configuration(inventory).expect("configuration plan");
        let authored = plan_legacy_backup_authored(configuration).expect("authored plan");
        let media = plan_legacy_backup_authored_media(authored).expect("media plan");
        let asr = plan_legacy_backup_asr(media).expect("ASR plan");
        plan_legacy_backup_usage(asr).expect("usage plan")
    }

    fn pricing_json() -> String {
        serde_json::to_string(&json!({
            "prompt": "0.000001",
            "completion": "0.000002",
            "request": "0.01",
            "image": "",
            "imageOutput": "",
            "webSearch": "0.004",
            "internalReasoning": "0.000003",
            "inputCacheRead": "0.0000001",
            "inputCacheWrite": "0.0000012",
            "futurePrice": "0.5"
        }))
        .expect("pricing JSON")
    }

    #[test]
    fn backup_pricing_retains_exact_payload_as_historical_evidence() {
        let raw = pricing_json();
        let plan = plan_legacy_backup_pricing(source(Some(json!([{
            "model_id": "author/model",
            "pricing_json": raw,
            "cached_at": 1_780_000_000_i64,
            "future_cache_field": true
        }]))))
        .expect("pricing plan");
        assert_eq!(plan.entries.len(), 1);
        assert_eq!(plan.entries[0].model_id, "author/model");
        assert_eq!(plan.entries[0].pricing_json.as_deref(), Some(raw.as_str()));
        assert_eq!(
            plan.entries[0].disposition,
            LegacyBackupPricingDisposition::HistoricalOnly
        );
        assert!(plan.notices.iter().any(|notice| {
            notice.kind == LegacyBackupConversionNoticeKind::Lossy
                && notice.document == LegacyBackupDocumentKind::ModelPricingCache
                && notice.field == "[].currentEndpointIdentity"
        }));
        assert!(plan.notices.iter().any(|notice| {
            notice.kind == LegacyBackupConversionNoticeKind::Unsupported
                && notice.field == "[0].pricing_json.futurePrice"
        }));
        assert!(plan.notices.iter().any(|notice| {
            notice.kind == LegacyBackupConversionNoticeKind::Unsupported
                && notice.field == "[0].future_cache_field"
        }));
    }

    #[test]
    fn backup_pricing_rejects_duplicates_invalid_time_and_invalid_prices() {
        let row = json!({
            "model_id": "author/model",
            "pricing_json": pricing_json(),
            "cached_at": 1
        });
        assert!(matches!(
            plan_legacy_backup_pricing(source(Some(json!([row.clone(), row])))),
            Err(LegacyBackupPricingError::Malformed { ref field })
                if field == "[1].model_id"
        ));

        assert!(matches!(
            plan_legacy_backup_pricing(source(Some(json!([{
                "model_id": "author/model",
                "pricing_json": pricing_json(),
                "cached_at": -1
            }])))),
            Err(LegacyBackupPricingError::Malformed { ref field })
                if field == "[0].cached_at"
        ));

        let invalid = serde_json::to_string(&json!({
            "prompt": "NaN",
            "completion": "0.1"
        }))
        .expect("invalid pricing JSON");
        assert!(matches!(
            plan_legacy_backup_pricing(source(Some(json!([{
                "model_id": "author/model",
                "pricing_json": invalid,
                "cached_at": 1
            }])))),
            Err(LegacyBackupPricingError::Malformed { ref field })
                if field == "[0].pricing_json.prompt"
        ));
    }

    #[test]
    fn missing_pricing_document_is_explicit_and_empty() {
        let plan = plan_legacy_backup_pricing(source(None)).expect("pricing plan");
        assert!(plan.entries.is_empty());
        assert!(plan.notices.iter().any(|notice| {
            notice.kind == LegacyBackupConversionNoticeKind::Absent
                && notice.document == LegacyBackupDocumentKind::ModelPricingCache
                && notice.field == "$"
        }));
    }
}
