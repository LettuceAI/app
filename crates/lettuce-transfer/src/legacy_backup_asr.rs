use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use serde_json::Value;

use crate::{
    LEGACY_ASR_RECORD_PLAN_LIMIT, LEGACY_ASR_TABLE_PLAN_LIMIT, LegacyAsrCorrectionCandidate,
    LegacyAsrIgnoredSuggestionCandidate, LegacyAsrPlan, LegacyAsrVocabularyCandidate,
    LegacyBackupAuthoredMediaPlan, LegacyBackupConversionNotice, LegacyBackupConversionNoticeKind,
    LegacyBackupDocumentKind,
};

#[derive(Debug)]
pub struct LegacyBackupAsrPlan {
    pub asr: LegacyAsrPlan,
    pub notices: Vec<LegacyBackupConversionNotice>,
    pub source: LegacyBackupAuthoredMediaPlan,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LegacyBackupAsrError {
    #[error("legacy backup ASR learning document is malformed")]
    Malformed { field: String },
    #[error("legacy backup ASR learning document exceeds its record limit")]
    LimitExceeded,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AsrLearningDocument {
    #[serde(default)]
    vocabulary_terms: Vec<VocabularyRow>,
    #[serde(default)]
    corrections: Vec<CorrectionRow>,
    #[serde(default)]
    ignored_suggestions: Vec<IgnoredSuggestionRow>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct VocabularyRow {
    term: String,
    normalized_term: String,
    language: Option<String>,
    category: Option<String>,
    scope: String,
    priority: i64,
    use_count: i64,
    created_at: String,
    updated_at: String,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct CorrectionRow {
    wrong: String,
    normalized_wrong: String,
    correct: String,
    normalized_correct: String,
    language: Option<String>,
    scope: String,
    confidence: f64,
    use_count: i64,
    accepted_count: i64,
    rejected_count: i64,
    seen_count: i64,
    last_seen_at: Option<String>,
    user_approved: i64,
    created_at: String,
    updated_at: String,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct IgnoredSuggestionRow {
    wrong: String,
    normalized_wrong: String,
    correct: String,
    normalized_correct: String,
    language: Option<String>,
    scope: String,
    ignored_count: i64,
    last_ignored_at: String,
    created_at: String,
    updated_at: String,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

pub fn plan_legacy_backup_asr(
    source: LegacyBackupAuthoredMediaPlan,
) -> Result<LegacyBackupAsrPlan, LegacyBackupAsrError> {
    let document = source
        .authored
        .configuration
        .source
        .documents
        .iter()
        .find(|document| document.kind == LegacyBackupDocumentKind::AsrLearning);
    let mut notices = source.authored.notices.clone();
    let asr = match document {
        Some(document) => {
            let parsed: AsrLearningDocument =
                serde_json::from_slice(&document.bytes).map_err(|_| malformed("$"))?;
            report_extra("$", &parsed.extra, &mut notices);
            validate_bounds(&parsed)?;
            let vocabulary = map_vocabulary(parsed.vocabulary_terms, &mut notices)?;
            let corrections = map_corrections(parsed.corrections, &mut notices)?;
            let ignored_suggestions =
                map_ignored_suggestions(parsed.ignored_suggestions, &mut notices)?;
            for field in [
                "vocabularyTerms[].id",
                "corrections[].id",
                "ignoredSuggestions[].id",
            ] {
                notices.push(notice(LegacyBackupConversionNoticeKind::Lossy, field));
            }
            notices.push(notice(
                LegacyBackupConversionNoticeKind::Absent,
                "voiceExamples",
            ));
            LegacyAsrPlan {
                vocabulary,
                corrections,
                ignored_suggestions,
                voice_examples: Vec::new(),
            }
        }
        None => {
            notices.push(notice(LegacyBackupConversionNoticeKind::Absent, "$"));
            LegacyAsrPlan {
                vocabulary: Vec::new(),
                corrections: Vec::new(),
                ignored_suggestions: Vec::new(),
                voice_examples: Vec::new(),
            }
        }
    };
    notices.sort();
    notices.dedup();
    Ok(LegacyBackupAsrPlan {
        asr,
        notices,
        source,
    })
}

fn validate_bounds(document: &AsrLearningDocument) -> Result<(), LegacyBackupAsrError> {
    let table_limit = usize::try_from(LEGACY_ASR_TABLE_PLAN_LIMIT)
        .map_err(|_| LegacyBackupAsrError::LimitExceeded)?;
    if document.vocabulary_terms.len() > table_limit
        || document.corrections.len() > table_limit
        || document.ignored_suggestions.len() > table_limit
    {
        return Err(LegacyBackupAsrError::LimitExceeded);
    }
    let aggregate = document
        .vocabulary_terms
        .len()
        .checked_add(document.corrections.len())
        .and_then(|count| count.checked_add(document.ignored_suggestions.len()))
        .ok_or(LegacyBackupAsrError::LimitExceeded)?;
    if aggregate
        > usize::try_from(LEGACY_ASR_RECORD_PLAN_LIMIT)
            .map_err(|_| LegacyBackupAsrError::LimitExceeded)?
    {
        return Err(LegacyBackupAsrError::LimitExceeded);
    }
    Ok(())
}

fn map_vocabulary(
    rows: Vec<VocabularyRow>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<LegacyAsrVocabularyCandidate>, LegacyBackupAsrError> {
    let mut identities = BTreeSet::new();
    let mut candidates = Vec::with_capacity(rows.len());
    for (index, row) in rows.into_iter().enumerate() {
        let path = format!("vocabularyTerms[{index}]");
        report_extra(&path, &row.extra, notices);
        validate_common(
            &row.term,
            &row.normalized_term,
            row.language.as_deref(),
            &row.scope,
            &row.created_at,
            Some(&row.updated_at),
            &path,
        )?;
        if row
            .category
            .as_ref()
            .is_some_and(|value| value.chars().count() > 512 || value.contains('\0'))
        {
            return Err(malformed(format!("{path}.category")));
        }
        let identity = (
            row.normalized_term.clone(),
            row.language.clone(),
            row.scope.clone(),
        );
        if !identities.insert(identity) {
            return Err(malformed(format!("{path}.normalized_term")));
        }
        candidates.push(LegacyAsrVocabularyCandidate {
            source_id: synthetic_id(index)?,
            term: row.term,
            normalized_term: row.normalized_term,
            language: row.language,
            category: row.category,
            scope: row.scope,
            priority: row.priority,
            use_count: count(row.use_count, &format!("{path}.use_count"))?,
            created_at: row.created_at,
            updated_at: row.updated_at,
        });
    }
    Ok(candidates)
}

fn map_corrections(
    rows: Vec<CorrectionRow>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<LegacyAsrCorrectionCandidate>, LegacyBackupAsrError> {
    let mut identities = BTreeSet::new();
    let mut candidates = Vec::with_capacity(rows.len());
    for (index, row) in rows.into_iter().enumerate() {
        let path = format!("corrections[{index}]");
        report_extra(&path, &row.extra, notices);
        validate_common(
            &row.wrong,
            &row.normalized_wrong,
            row.language.as_deref(),
            &row.scope,
            &row.created_at,
            Some(&row.updated_at),
            &path,
        )?;
        validate_pair(
            &row.correct,
            &row.normalized_correct,
            &format!("{path}.correct"),
        )?;
        let use_count = count(row.use_count, &format!("{path}.use_count"))?;
        if !row.confidence.is_finite()
            || !(0.0..=1.0).contains(&row.confidence)
            || use_count == 0
            || row.last_seen_at.as_ref().is_some_and(|value| {
                !valid_timestamp(value) || value.as_str() > row.updated_at.as_str()
            })
        {
            return Err(malformed(format!("{path}.metrics")));
        }
        let user_approved = match row.user_approved {
            0 => false,
            1 => true,
            _ => return Err(malformed(format!("{path}.user_approved"))),
        };
        let identity = (
            row.normalized_wrong.clone(),
            row.normalized_correct.clone(),
            row.language.clone(),
            row.scope.clone(),
        );
        if !identities.insert(identity) {
            return Err(malformed(format!("{path}.normalized_wrong")));
        }
        candidates.push(LegacyAsrCorrectionCandidate {
            source_id: synthetic_id(index)?,
            wrong: row.wrong,
            normalized_wrong: row.normalized_wrong,
            correct: row.correct,
            normalized_correct: row.normalized_correct,
            language: row.language,
            scope: row.scope,
            confidence: row.confidence,
            use_count,
            accepted_count: count(row.accepted_count, &format!("{path}.accepted_count"))?,
            rejected_count: count(row.rejected_count, &format!("{path}.rejected_count"))?,
            seen_count: count(row.seen_count, &format!("{path}.seen_count"))?,
            last_seen_at: row.last_seen_at,
            user_approved,
            created_at: row.created_at,
            updated_at: row.updated_at,
        });
    }
    Ok(candidates)
}

fn map_ignored_suggestions(
    rows: Vec<IgnoredSuggestionRow>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<LegacyAsrIgnoredSuggestionCandidate>, LegacyBackupAsrError> {
    let mut identities = BTreeSet::new();
    let mut candidates = Vec::with_capacity(rows.len());
    for (index, row) in rows.into_iter().enumerate() {
        let path = format!("ignoredSuggestions[{index}]");
        report_extra(&path, &row.extra, notices);
        validate_common(
            &row.wrong,
            &row.normalized_wrong,
            row.language.as_deref(),
            &row.scope,
            &row.created_at,
            Some(&row.updated_at),
            &path,
        )?;
        validate_pair(
            &row.correct,
            &row.normalized_correct,
            &format!("{path}.correct"),
        )?;
        let ignored_count = count(row.ignored_count, &format!("{path}.ignored_count"))?;
        if row.normalized_wrong == row.normalized_correct
            || ignored_count == 0
            || !valid_timestamp(&row.last_ignored_at)
            || row.last_ignored_at > row.updated_at
        {
            return Err(malformed(format!("{path}.ignored_count")));
        }
        let identity = (
            row.normalized_wrong.clone(),
            row.normalized_correct.clone(),
            row.language.clone(),
            row.scope.clone(),
        );
        if !identities.insert(identity) {
            return Err(malformed(format!("{path}.normalized_wrong")));
        }
        candidates.push(LegacyAsrIgnoredSuggestionCandidate {
            source_id: synthetic_id(index)?,
            wrong: row.wrong,
            normalized_wrong: row.normalized_wrong,
            correct: row.correct,
            normalized_correct: row.normalized_correct,
            language: row.language,
            scope: row.scope,
            ignored_count,
            last_ignored_at: row.last_ignored_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        });
    }
    Ok(candidates)
}

fn validate_common(
    authored: &str,
    normalized: &str,
    language: Option<&str>,
    scope: &str,
    created_at: &str,
    updated_at: Option<&str>,
    path: &str,
) -> Result<(), LegacyBackupAsrError> {
    validate_pair(authored, normalized, &format!("{path}.normalized_text"))?;
    if language.is_some_and(|value| {
        value.is_empty()
            || value.trim() != value
            || value.to_ascii_lowercase() != value
            || value.chars().count() > 32
            || value.chars().any(char::is_control)
    }) || scope.is_empty()
        || scope.trim() != scope
        || scope.to_ascii_lowercase() != scope
        || scope.chars().count() > 64
        || scope.chars().any(char::is_control)
        || !valid_timestamp(created_at)
        || updated_at.is_some_and(|value| !valid_timestamp(value) || created_at > value)
    {
        return Err(malformed(format!("{path}.metadata")));
    }
    Ok(())
}

fn validate_pair(
    authored: &str,
    normalized: &str,
    field: &str,
) -> Result<(), LegacyBackupAsrError> {
    if authored.is_empty()
        || authored.chars().count() > 4_096
        || authored.contains('\0')
        || normalized.is_empty()
        || normalized != normalize_asr(authored)
    {
        Err(malformed(field))
    } else {
        Ok(())
    }
}

fn normalize_asr(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut last_was_space = true;
    for character in value.chars() {
        if character.is_alphanumeric() {
            normalized.extend(character.to_lowercase());
            last_was_space = false;
        } else if !last_was_space {
            normalized.push(' ');
            last_was_space = true;
        }
    }
    normalized.trim().to_owned()
}

fn valid_timestamp(value: &str) -> bool {
    !value.trim().is_empty() && value.trim() == value && !value.contains('\0')
}

fn count(value: i64, field: &str) -> Result<u64, LegacyBackupAsrError> {
    u64::try_from(value).map_err(|_| malformed(field))
}

fn synthetic_id(index: usize) -> Result<i64, LegacyBackupAsrError> {
    index
        .checked_add(1)
        .and_then(|value| i64::try_from(value).ok())
        .ok_or(LegacyBackupAsrError::LimitExceeded)
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
        document: LegacyBackupDocumentKind::AsrLearning,
        field: field.to_owned(),
    }
}

fn malformed(field: impl Into<String>) -> LegacyBackupAsrError {
    LegacyBackupAsrError::Malformed {
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
        LegacyBackupDocument, LegacyBackupInventory, plan_legacy_backup_authored,
        plan_legacy_backup_authored_media, plan_legacy_backup_configuration,
    };

    fn document(value: Value) -> LegacyBackupDocument {
        LegacyBackupDocument {
            kind: LegacyBackupDocumentKind::AsrLearning,
            bytes: Zeroizing::new(serde_json::to_vec(&value).expect("fixture document")),
        }
    }

    fn source(documents: Vec<LegacyBackupDocument>) -> LegacyBackupAuthoredMediaPlan {
        let inventory = LegacyBackupInventory {
            version: 1,
            created_at: 1,
            app_version: "legacy".into(),
            source_hash: ContentHash::parse("44".repeat(32)).expect("source hash"),
            documents,
            media: Vec::new(),
        };
        let configuration =
            plan_legacy_backup_configuration(inventory).expect("configuration plan");
        let authored = plan_legacy_backup_authored(configuration).expect("authored plan");
        plan_legacy_backup_authored_media(authored).expect("media plan")
    }

    fn complete_document() -> Value {
        json!({
            "vocabularyTerms": [{
                "term": "Lettuce AI",
                "normalized_term": "lettuce ai",
                "language": "en",
                "category": "product",
                "scope": "global",
                "priority": 8,
                "use_count": 3,
                "created_at": "2026-01-01T00:00:00Z",
                "updated_at": "2026-01-02T00:00:00Z",
                "future_metric": 5
            }],
            "corrections": [{
                "wrong": "letus",
                "normalized_wrong": "letus",
                "correct": "lettuce",
                "normalized_correct": "lettuce",
                "language": "en",
                "scope": "global",
                "confidence": 0.75,
                "use_count": 4,
                "accepted_count": 2,
                "rejected_count": 1,
                "seen_count": 5,
                "last_seen_at": "2026-01-02T00:00:00Z",
                "user_approved": 1,
                "created_at": "2026-01-01T00:00:00Z",
                "updated_at": "2026-01-03T00:00:00Z"
            }],
            "ignoredSuggestions": [{
                "wrong": "colour",
                "normalized_wrong": "colour",
                "correct": "color",
                "normalized_correct": "color",
                "language": "en",
                "scope": "project",
                "ignored_count": 2,
                "last_ignored_at": "2026-01-02T00:00:00Z",
                "created_at": "2026-01-01T00:00:00Z",
                "updated_at": "2026-01-03T00:00:00Z"
            }],
            "futureRoot": true
        })
    }

    #[test]
    fn backup_asr_preserves_exported_learning_and_reports_omissions() {
        let plan =
            plan_legacy_backup_asr(source(vec![document(complete_document())])).expect("ASR plan");
        assert_eq!(plan.asr.vocabulary.len(), 1);
        assert_eq!(plan.asr.vocabulary[0].source_id, 1);
        assert_eq!(plan.asr.vocabulary[0].use_count, 3);
        assert_eq!(plan.asr.corrections[0].accepted_count, 2);
        assert!(plan.asr.corrections[0].user_approved);
        assert_eq!(plan.asr.ignored_suggestions[0].ignored_count, 2);
        assert!(plan.asr.voice_examples.is_empty());
        assert!(plan.notices.iter().any(|item| {
            item.kind == LegacyBackupConversionNoticeKind::Absent
                && item.document == LegacyBackupDocumentKind::AsrLearning
                && item.field == "voiceExamples"
        }));
        assert!(plan.notices.iter().any(|item| {
            item.kind == LegacyBackupConversionNoticeKind::Lossy && item.field == "corrections[].id"
        }));
        assert!(plan.notices.iter().any(|item| {
            item.kind == LegacyBackupConversionNoticeKind::Unsupported
                && item.field == "vocabularyTerms[0].future_metric"
        }));
        assert!(plan.notices.iter().any(|item| {
            item.kind == LegacyBackupConversionNoticeKind::Unsupported
                && item.field == "$.futureRoot"
        }));
        assert_eq!(plan.source.authored.configuration.source.documents.len(), 1);
    }

    #[test]
    fn missing_backup_asr_document_produces_an_empty_explicit_plan() {
        let plan = plan_legacy_backup_asr(source(Vec::new())).expect("empty ASR plan");
        assert!(plan.asr.vocabulary.is_empty());
        assert!(plan.asr.corrections.is_empty());
        assert!(plan.asr.ignored_suggestions.is_empty());
        assert!(plan.asr.voice_examples.is_empty());
        assert!(plan.notices.iter().any(|item| {
            item.kind == LegacyBackupConversionNoticeKind::Absent
                && item.document == LegacyBackupDocumentKind::AsrLearning
                && item.field == "$"
        }));
    }

    #[test]
    fn backup_asr_rejects_negative_counters_and_duplicate_semantic_rows() {
        let mut negative = complete_document();
        negative["vocabularyTerms"][0]["use_count"] = json!(-1);
        assert!(matches!(
            plan_legacy_backup_asr(source(vec![document(negative)])),
            Err(LegacyBackupAsrError::Malformed { ref field })
                if field == "vocabularyTerms[0].use_count"
        ));

        let mut invalid_timestamp = complete_document();
        invalid_timestamp["ignoredSuggestions"][0]["last_ignored_at"] =
            json!(" 2026-01-02T00:00:00Z");
        assert!(matches!(
            plan_legacy_backup_asr(source(vec![document(invalid_timestamp)])),
            Err(LegacyBackupAsrError::Malformed { ref field })
                if field == "ignoredSuggestions[0].ignored_count"
        ));

        let mut duplicate = complete_document();
        let row = duplicate["corrections"][0].clone();
        duplicate["corrections"]
            .as_array_mut()
            .expect("correction array")
            .push(row);
        assert!(matches!(
            plan_legacy_backup_asr(source(vec![document(duplicate)])),
            Err(LegacyBackupAsrError::Malformed { ref field })
                if field == "corrections[1].normalized_wrong"
        ));
    }
}
