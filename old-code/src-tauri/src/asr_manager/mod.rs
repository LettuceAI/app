pub mod models;
pub mod runtime;

use regex::Regex;
use rusqlite::{params, ToSql};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

use crate::storage_manager::db::open_db;

pub(crate) use runtime::{initialize_whisper_logging, WhisperRuntimeState};

const DEFAULT_SCOPE: &str = "global";
const MAX_PROMPT_CHARS: usize = 240;
const MAX_PROMPT_TERMS: usize = 24;
const MAX_REPLACEMENT_WORDS: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrVocabularyTerm {
    pub id: Option<i64>,
    pub term: String,
    pub normalized_term: Option<String>,
    pub language: Option<String>,
    pub category: Option<String>,
    pub scope: Option<String>,
    pub priority: Option<i64>,
    pub use_count: Option<i64>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrCorrection {
    pub id: Option<i64>,
    pub wrong: String,
    pub normalized_wrong: Option<String>,
    pub correct: String,
    pub normalized_correct: Option<String>,
    pub language: Option<String>,
    pub scope: Option<String>,
    pub confidence: Option<f64>,
    pub use_count: Option<i64>,
    pub accepted_count: Option<i64>,
    pub rejected_count: Option<i64>,
    pub seen_count: Option<i64>,
    pub last_seen_at: Option<String>,
    pub user_approved: Option<bool>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrVoiceExample {
    pub id: Option<i64>,
    pub audio_path: String,
    pub expected_text: String,
    pub normalized_expected_text: Option<String>,
    pub whisper_output: Option<String>,
    pub normalized_whisper_output: Option<String>,
    pub language: Option<String>,
    pub scope: Option<String>,
    pub term_id: Option<i64>,
    pub correction_id: Option<i64>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrCorrectionApplication {
    pub correction_id: i64,
    pub wrong: String,
    pub correct: String,
    pub match_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrCorrectionResult {
    pub raw_text: String,
    pub corrected_text: String,
    pub applied: Vec<AsrCorrectionApplication>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrLearnedSuggestion {
    pub wrong: String,
    pub normalized_wrong: String,
    pub correct: String,
    pub normalized_correct: String,
    pub language: Option<String>,
    pub scope: String,
    pub confidence: f64,
    pub accepted_count: i64,
    pub rejected_count: i64,
    pub seen_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrIgnoredSuggestion {
    pub id: Option<i64>,
    pub wrong: String,
    pub normalized_wrong: Option<String>,
    pub correct: String,
    pub normalized_correct: Option<String>,
    pub language: Option<String>,
    pub scope: Option<String>,
    pub ignored_count: Option<i64>,
    pub last_ignored_at: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrExportBundle {
    pub version: u32,
    #[serde(default)]
    pub vocabulary: Vec<AsrVocabularyTerm>,
    #[serde(default)]
    pub corrections: Vec<AsrCorrection>,
    #[serde(default)]
    pub voice_examples: Vec<AsrVoiceExample>,
    #[serde(default)]
    pub ignored_suggestions: Vec<AsrIgnoredSuggestion>,
}

#[derive(Debug, Clone)]
struct StoredCorrection {
    id: i64,
    wrong: String,
    correct: String,
}

#[derive(Debug, Clone)]
struct StoredCorrectionMemory {
    id: i64,
    scope: String,
    use_count: i64,
    accepted_count: i64,
    rejected_count: i64,
    seen_count: i64,
    user_approved: bool,
}

fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_lookup_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_was_space = true;
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            out.extend(ch.to_lowercase());
            last_was_space = false;
        } else if !last_was_space {
            out.push(' ');
            last_was_space = true;
        }
    }
    out.trim().to_string()
}

fn compact_lookup_text(text: &str) -> String {
    normalize_lookup_text(text).replace(' ', "")
}

fn normalize_scope(scope: Option<&str>) -> String {
    let value = scope.unwrap_or(DEFAULT_SCOPE).trim();
    if value.is_empty() {
        DEFAULT_SCOPE.to_string()
    } else {
        value.to_ascii_lowercase()
    }
}

fn normalize_language(language: Option<&str>) -> Option<String> {
    language
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

fn phonetic_key(text: &str) -> String {
    let mut key = String::new();
    let mut last = '\0';
    for ch in compact_lookup_text(text).chars() {
        let mapped = match ch {
            'a' | 'e' | 'i' | 'o' | 'u' | 'y' => continue,
            'b' | 'p' => 'p',
            'c' | 'k' | 'q' => 'k',
            'd' | 't' => 't',
            'f' | 'v' => 'f',
            'g' | 'j' => 'j',
            's' | 'x' | 'z' => 's',
            other => other,
        };
        if mapped != last {
            key.push(mapped);
            last = mapped;
        }
    }
    key
}

fn scope_rank(scope: &str) -> i32 {
    match scope {
        "conversation" => 0,
        "character" => 1,
        "project" => 2,
        "global" => 3,
        _ => 0,
    }
}

fn promoted_scope(scope: &str, accepted_count: i64) -> String {
    let normalized = normalize_scope(Some(scope));
    match normalized.as_str() {
        "conversation" if accepted_count >= 4 => "global".to_string(),
        "conversation" if accepted_count >= 2 => "project".to_string(),
        "character" if accepted_count >= 4 => "global".to_string(),
        "project" if accepted_count >= 4 => "global".to_string(),
        _ => normalized,
    }
}

fn preferred_scope(
    existing_scope: Option<&str>,
    requested_scope: &str,
    accepted_count: i64,
) -> String {
    let existing = existing_scope.map(|value| normalize_scope(Some(value)));
    let promoted_requested = promoted_scope(requested_scope, accepted_count);
    match existing {
        Some(existing_scope) if scope_rank(&existing_scope) >= scope_rank(&promoted_requested) => {
            promoted_scope(&existing_scope, accepted_count)
        }
        _ => promoted_requested,
    }
}

fn correction_regex_for_phrase(phrase: &str) -> Result<Regex, String> {
    let escaped_parts: Vec<String> = phrase.split_whitespace().map(regex::escape).collect();
    let pattern = format!(r"(?i)\b{}\b", escaped_parts.join(r"\s+"));
    Regex::new(&pattern).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn tokenize_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '\'' {
            current.push(ch);
        } else if !current.is_empty() {
            words.push(current.clone());
            current.clear();
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

fn stopwords() -> HashSet<&'static str> {
    [
        "a", "an", "and", "are", "be", "but", "can", "did", "do", "for", "from", "go", "have",
        "he", "her", "him", "i", "in", "is", "it", "its", "me", "my", "not", "of", "on", "or",
        "our", "she", "that", "the", "their", "them", "there", "they", "this", "to", "us", "was",
        "we", "were", "with", "you", "your",
    ]
    .into_iter()
    .collect()
}

fn is_low_value_replacement(before: &[String], after: &[String]) -> bool {
    let stopwords = stopwords();
    let before_all_common = before
        .iter()
        .all(|token| stopwords.contains(normalize_lookup_text(token).as_str()));
    let after_all_common = after
        .iter()
        .all(|token| stopwords.contains(normalize_lookup_text(token).as_str()));
    let very_short_single_word = before.len() == 1
        && after.len() == 1
        && compact_lookup_text(&before[0]).len() <= 3
        && compact_lookup_text(&after[0]).len() <= 3;
    (before_all_common && after_all_common) || very_short_single_word
}

fn lcs_matches(before: &[String], after: &[String]) -> Vec<(usize, usize)> {
    let n = before.len();
    let m = after.len();
    let mut dp = vec![vec![0usize; m + 1]; n + 1];

    for i in (0..n).rev() {
        for j in (0..m).rev() {
            if before[i] == after[j] {
                dp[i][j] = dp[i + 1][j + 1] + 1;
            } else {
                dp[i][j] = dp[i + 1][j].max(dp[i][j + 1]);
            }
        }
    }

    let mut i = 0usize;
    let mut j = 0usize;
    let mut matches = Vec::new();
    while i < n && j < m {
        if before[i] == after[j] {
            matches.push((i, j));
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }

    matches
}

fn map_scopes_for_query(scopes: &[String]) -> Vec<String> {
    if scopes.is_empty() {
        vec![DEFAULT_SCOPE.to_string()]
    } else {
        scopes
            .iter()
            .map(|scope| normalize_scope(Some(scope)))
            .collect()
    }
}

fn query_scope_clause(scopes_len: usize) -> String {
    let placeholders = vec!["?"; scopes_len].join(", ");
    format!("scope IN ({})", placeholders)
}

fn scope_and_language_params<'a>(
    scopes: &'a [String],
    language: &'a Option<String>,
) -> Vec<&'a dyn ToSql> {
    let mut values: Vec<&dyn ToSql> = Vec::with_capacity(scopes.len() + 2);
    for scope in scopes {
        values.push(scope as &dyn ToSql);
    }
    values.push(language as &dyn ToSql);
    values.push(language as &dyn ToSql);
    values
}

#[tauri::command]
pub fn asr_vocabulary_list(
    app: tauri::AppHandle,
    language: Option<String>,
    scopes: Option<Vec<String>>,
) -> Result<Vec<AsrVocabularyTerm>, String> {
    let conn = open_db(&app)?;
    let scopes = map_scopes_for_query(&scopes.unwrap_or_default());
    let language = normalize_language(language.as_deref());
    let sql = format!(
        "SELECT id, term, normalized_term, language, category, scope, priority, use_count, created_at, updated_at
         FROM asr_vocabulary_terms
         WHERE ({})
           AND (? IS NULL OR language IS NULL OR language = ?)
         ORDER BY priority DESC, use_count DESC, updated_at DESC, id DESC",
        query_scope_clause(scopes.len())
    );
    let params = scope_and_language_params(&scopes, &language);
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let terms = stmt
        .query_map(rusqlite::params_from_iter(params), |row| {
            Ok(AsrVocabularyTerm {
                id: Some(row.get(0)?),
                term: row.get(1)?,
                normalized_term: Some(row.get(2)?),
                language: row.get(3)?,
                category: row.get(4)?,
                scope: Some(row.get(5)?),
                priority: Some(row.get(6)?),
                use_count: Some(row.get(7)?),
                created_at: Some(row.get(8)?),
                updated_at: Some(row.get(9)?),
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(terms)
}

#[tauri::command]
pub fn asr_vocabulary_upsert(
    app: tauri::AppHandle,
    term: AsrVocabularyTerm,
) -> Result<AsrVocabularyTerm, String> {
    let conn = open_db(&app)?;
    let normalized_term = normalize_lookup_text(&term.term);
    if normalized_term.is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Vocabulary term cannot be empty",
        ));
    }

    let language = normalize_language(term.language.as_deref());
    let scope = normalize_scope(term.scope.as_deref());
    let priority = term.priority.unwrap_or(50);
    let use_count = term.use_count.unwrap_or(0);

    match term.id {
        Some(id) => {
            conn.execute(
                "UPDATE asr_vocabulary_terms
                 SET term = ?1,
                     normalized_term = ?2,
                     language = ?3,
                     category = ?4,
                     scope = ?5,
                     priority = ?6,
                     use_count = ?7,
                     updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?8",
                params![
                    term.term,
                    normalized_term,
                    language,
                    term.category,
                    scope,
                    priority,
                    use_count,
                    id
                ],
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            Ok(AsrVocabularyTerm {
                id: Some(id),
                term: term.term,
                normalized_term: Some(normalized_term),
                language,
                category: term.category,
                scope: Some(scope),
                priority: Some(priority),
                use_count: Some(use_count),
                created_at: None,
                updated_at: None,
            })
        }
        None => {
            conn.execute(
                "INSERT INTO asr_vocabulary_terms
                 (term, normalized_term, language, category, scope, priority, use_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    term.term,
                    normalized_term,
                    language,
                    term.category,
                    scope,
                    priority,
                    use_count
                ],
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            let id = conn.last_insert_rowid();
            Ok(AsrVocabularyTerm {
                id: Some(id),
                term: term.term,
                normalized_term: Some(normalized_term),
                language,
                category: term.category,
                scope: Some(scope),
                priority: Some(priority),
                use_count: Some(use_count),
                created_at: None,
                updated_at: None,
            })
        }
    }
}

#[tauri::command]
pub fn asr_vocabulary_delete(app: tauri::AppHandle, id: i64) -> Result<(), String> {
    let conn = open_db(&app)?;
    conn.execute(
        "DELETE FROM asr_vocabulary_terms WHERE id = ?1",
        params![id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

#[tauri::command]
pub fn asr_corrections_list(
    app: tauri::AppHandle,
    language: Option<String>,
    scopes: Option<Vec<String>>,
    user_approved_only: Option<bool>,
) -> Result<Vec<AsrCorrection>, String> {
    let conn = open_db(&app)?;
    let scopes = map_scopes_for_query(&scopes.unwrap_or_default());
    let language = normalize_language(language.as_deref());
    let approved = user_approved_only.map(|value| if value { 1 } else { 0 });
    let sql = format!(
        "SELECT id, wrong, normalized_wrong, correct, normalized_correct, language, scope, confidence, use_count, accepted_count, rejected_count, seen_count, last_seen_at, user_approved, created_at, updated_at
         FROM asr_corrections
         WHERE ({})
           AND (? IS NULL OR language IS NULL OR language = ?)
           AND (? IS NULL OR user_approved = ?)
         ORDER BY user_approved DESC, accepted_count DESC, confidence DESC, use_count DESC, updated_at DESC, id DESC",
        query_scope_clause(scopes.len())
    );
    let mut params = scope_and_language_params(&scopes, &language);
    params.push(&approved as &dyn ToSql);
    params.push(&approved as &dyn ToSql);
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let items = stmt
        .query_map(rusqlite::params_from_iter(params), |row| {
            Ok(AsrCorrection {
                id: Some(row.get(0)?),
                wrong: row.get(1)?,
                normalized_wrong: Some(row.get(2)?),
                correct: row.get(3)?,
                normalized_correct: Some(row.get(4)?),
                language: row.get(5)?,
                scope: Some(row.get(6)?),
                confidence: Some(row.get(7)?),
                use_count: Some(row.get(8)?),
                accepted_count: Some(row.get(9)?),
                rejected_count: Some(row.get(10)?),
                seen_count: Some(row.get(11)?),
                last_seen_at: row.get(12)?,
                user_approved: Some(row.get::<_, i64>(13)? != 0),
                created_at: Some(row.get(14)?),
                updated_at: Some(row.get(15)?),
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(items)
}

#[tauri::command]
pub fn asr_correction_upsert(
    app: tauri::AppHandle,
    correction: AsrCorrection,
) -> Result<AsrCorrection, String> {
    let conn = open_db(&app)?;
    let normalized_wrong = normalize_lookup_text(&correction.wrong);
    let normalized_correct = normalize_lookup_text(&correction.correct);
    if normalized_wrong.is_empty() || normalized_correct.is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Correction wrong/correct values cannot be empty",
        ));
    }

    let language = normalize_language(correction.language.as_deref());
    let scope = normalize_scope(correction.scope.as_deref());
    let existing = match correction.id {
        Some(id) => load_correction_memory_by_id(&conn, id)?,
        None => find_existing_correction_memory_for_pair(
            &conn,
            &normalized_wrong,
            &normalized_correct,
            language.as_deref(),
        )?,
    };
    let target_id = correction.id.or(existing.as_ref().map(|item| item.id));
    let approved_now = correction.user_approved.unwrap_or(false);
    let final_use_count = correction
        .use_count
        .unwrap_or(existing.as_ref().map(|item| item.use_count).unwrap_or(1))
        .max(1);
    let base_accepted = existing
        .as_ref()
        .map(|item| item.accepted_count)
        .unwrap_or(0);
    let base_rejected = existing
        .as_ref()
        .map(|item| item.rejected_count)
        .unwrap_or(0);
    let base_seen = existing.as_ref().map(|item| item.seen_count).unwrap_or(0);
    let final_accepted = correction
        .accepted_count
        .unwrap_or(base_accepted + if approved_now { 1 } else { 0 });
    let final_rejected = correction.rejected_count.unwrap_or(base_rejected);
    let final_seen = correction
        .seen_count
        .unwrap_or(base_seen + if approved_now { 1 } else { 0 });
    let final_scope = preferred_scope(
        existing.as_ref().map(|item| item.scope.as_str()),
        &scope,
        final_accepted,
    );
    let final_user_approved = correction.user_approved.unwrap_or(
        existing
            .as_ref()
            .map(|item| item.user_approved)
            .unwrap_or(false),
    ) || final_accepted > 0;
    let final_confidence = correction.confidence.unwrap_or(score_suggestion(
        &conn,
        &correction.wrong,
        &normalized_correct,
        &correction.correct,
        language.as_deref(),
        &final_scope,
        correction.wrong.split_whitespace().count(),
        correction.correct.split_whitespace().count(),
        final_accepted,
        final_rejected,
        final_seen,
    )?);
    let has_seen_signal = correction.last_seen_at.is_some() || final_seen > 0;
    let user_approved = if final_user_approved { 1 } else { 0 };

    if let Some(id) = target_id {
        conn.execute(
            "UPDATE asr_corrections
             SET wrong = ?1,
                 normalized_wrong = ?2,
                 correct = ?3,
                 normalized_correct = ?4,
                 language = ?5,
                 scope = ?6,
                 confidence = ?7,
                 use_count = ?8,
                 accepted_count = ?9,
                 rejected_count = ?10,
                 seen_count = ?11,
                 last_seen_at = CASE WHEN ?12 IS NOT NULL THEN ?12 WHEN ?13 != 0 THEN CURRENT_TIMESTAMP ELSE last_seen_at END,
                 user_approved = ?14,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?15",
            params![
                correction.wrong,
                normalized_wrong,
                correction.correct,
                normalized_correct,
                language,
                final_scope,
                final_confidence,
                final_use_count,
                final_accepted,
                final_rejected,
                final_seen,
                correction.last_seen_at,
                if has_seen_signal { 1 } else { 0 },
                user_approved,
                id
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        conn.execute(
            "DELETE FROM asr_ignored_suggestions
             WHERE normalized_wrong = ?1
               AND normalized_correct = ?2
               AND ((language IS NULL AND ?3 IS NULL) OR language = ?3)",
            params![normalized_wrong, normalized_correct, language],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        return Ok(AsrCorrection {
            id: Some(id),
            wrong: correction.wrong,
            normalized_wrong: Some(normalized_wrong),
            correct: correction.correct,
            normalized_correct: Some(normalized_correct),
            language,
            scope: Some(final_scope),
            confidence: Some(final_confidence),
            use_count: Some(final_use_count),
            accepted_count: Some(final_accepted),
            rejected_count: Some(final_rejected),
            seen_count: Some(final_seen),
            last_seen_at: correction.last_seen_at,
            user_approved: Some(user_approved != 0),
            created_at: None,
            updated_at: None,
        });
    }

    conn.execute(
        "INSERT INTO asr_corrections
         (wrong, normalized_wrong, correct, normalized_correct, language, scope, confidence, use_count, accepted_count, rejected_count, seen_count, last_seen_at, user_approved)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, CASE WHEN ?12 IS NOT NULL THEN ?12 WHEN ?13 != 0 THEN CURRENT_TIMESTAMP ELSE NULL END, ?14)",
        params![
            correction.wrong,
            normalized_wrong,
            correction.correct,
            normalized_correct,
            language,
            final_scope,
            final_confidence,
            final_use_count,
            final_accepted,
            final_rejected,
            final_seen,
            correction.last_seen_at,
            if has_seen_signal { 1 } else { 0 },
            user_approved
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let id = conn.last_insert_rowid();
    conn.execute(
        "DELETE FROM asr_ignored_suggestions
         WHERE normalized_wrong = ?1
           AND normalized_correct = ?2
           AND ((language IS NULL AND ?3 IS NULL) OR language = ?3)",
        params![normalized_wrong, normalized_correct, language],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(AsrCorrection {
        id: Some(id),
        wrong: correction.wrong,
        normalized_wrong: Some(normalized_wrong),
        correct: correction.correct,
        normalized_correct: Some(normalized_correct),
        language,
        scope: Some(final_scope),
        confidence: Some(final_confidence),
        use_count: Some(final_use_count),
        accepted_count: Some(final_accepted),
        rejected_count: Some(final_rejected),
        seen_count: Some(final_seen),
        last_seen_at: correction.last_seen_at,
        user_approved: Some(user_approved != 0),
        created_at: None,
        updated_at: None,
    })
}

#[tauri::command]
pub fn asr_correction_delete(app: tauri::AppHandle, id: i64) -> Result<(), String> {
    let conn = open_db(&app)?;
    conn.execute("DELETE FROM asr_corrections WHERE id = ?1", params![id])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

#[tauri::command]
pub fn asr_voice_examples_list(
    app: tauri::AppHandle,
    language: Option<String>,
    scopes: Option<Vec<String>>,
) -> Result<Vec<AsrVoiceExample>, String> {
    let conn = open_db(&app)?;
    let scopes = map_scopes_for_query(&scopes.unwrap_or_default());
    let language = normalize_language(language.as_deref());
    let sql = format!(
        "SELECT id, audio_path, expected_text, normalized_expected_text, whisper_output, normalized_whisper_output, language, scope, term_id, correction_id, created_at
         FROM asr_voice_examples
         WHERE ({})
           AND (? IS NULL OR language IS NULL OR language = ?)
         ORDER BY created_at DESC, id DESC",
        query_scope_clause(scopes.len())
    );
    let params = scope_and_language_params(&scopes, &language);
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let items = stmt
        .query_map(rusqlite::params_from_iter(params), |row| {
            Ok(AsrVoiceExample {
                id: Some(row.get(0)?),
                audio_path: row.get(1)?,
                expected_text: row.get(2)?,
                normalized_expected_text: Some(row.get(3)?),
                whisper_output: row.get(4)?,
                normalized_whisper_output: row.get(5)?,
                language: row.get(6)?,
                scope: Some(row.get(7)?),
                term_id: row.get(8)?,
                correction_id: row.get(9)?,
                created_at: Some(row.get(10)?),
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(items)
}

#[tauri::command]
pub fn asr_voice_example_upsert(
    app: tauri::AppHandle,
    example: AsrVoiceExample,
) -> Result<AsrVoiceExample, String> {
    let conn = open_db(&app)?;
    let normalized_expected_text = normalize_lookup_text(&example.expected_text);
    if normalized_expected_text.is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Voice example expected text cannot be empty",
        ));
    }

    let normalized_whisper_output = example
        .whisper_output
        .as_deref()
        .map(normalize_lookup_text)
        .filter(|value| !value.is_empty());
    let language = normalize_language(example.language.as_deref());
    let scope = normalize_scope(example.scope.as_deref());

    match example.id {
        Some(id) => {
            conn.execute(
                "UPDATE asr_voice_examples
                 SET audio_path = ?1,
                     expected_text = ?2,
                     normalized_expected_text = ?3,
                     whisper_output = ?4,
                     normalized_whisper_output = ?5,
                     language = ?6,
                     scope = ?7,
                     term_id = ?8,
                     correction_id = ?9
                 WHERE id = ?10",
                params![
                    example.audio_path,
                    example.expected_text,
                    normalized_expected_text,
                    example.whisper_output,
                    normalized_whisper_output,
                    language,
                    scope,
                    example.term_id,
                    example.correction_id,
                    id
                ],
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            Ok(AsrVoiceExample {
                id: Some(id),
                audio_path: example.audio_path,
                expected_text: example.expected_text,
                normalized_expected_text: Some(normalized_expected_text),
                whisper_output: example.whisper_output,
                normalized_whisper_output,
                language,
                scope: Some(scope),
                term_id: example.term_id,
                correction_id: example.correction_id,
                created_at: None,
            })
        }
        None => {
            conn.execute(
                "INSERT INTO asr_voice_examples
                 (audio_path, expected_text, normalized_expected_text, whisper_output, normalized_whisper_output, language, scope, term_id, correction_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    example.audio_path,
                    example.expected_text,
                    normalized_expected_text,
                    example.whisper_output,
                    normalized_whisper_output,
                    language,
                    scope,
                    example.term_id,
                    example.correction_id
                ],
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            let id = conn.last_insert_rowid();
            Ok(AsrVoiceExample {
                id: Some(id),
                audio_path: example.audio_path,
                expected_text: example.expected_text,
                normalized_expected_text: Some(normalized_expected_text),
                whisper_output: example.whisper_output,
                normalized_whisper_output,
                language,
                scope: Some(scope),
                term_id: example.term_id,
                correction_id: example.correction_id,
                created_at: None,
            })
        }
    }
}

#[tauri::command]
pub fn asr_voice_example_delete(app: tauri::AppHandle, id: i64) -> Result<(), String> {
    let conn = open_db(&app)?;
    conn.execute("DELETE FROM asr_voice_examples WHERE id = ?1", params![id])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

#[tauri::command]
pub fn asr_build_prompt(
    app: tauri::AppHandle,
    language: Option<String>,
    scopes: Option<Vec<String>>,
) -> Result<String, String> {
    let terms = asr_vocabulary_list(app, language, scopes)?;
    let mut prompt_terms = Vec::new();
    let mut seen = HashSet::new();
    let mut current_len = 0usize;

    for term in terms {
        let candidate = normalize_whitespace(&term.term);
        if candidate.is_empty() {
            continue;
        }
        let dedupe_key = normalize_lookup_text(&candidate);
        if dedupe_key.is_empty() || !seen.insert(dedupe_key) {
            continue;
        }

        let separator = if prompt_terms.is_empty() { 0 } else { 2 };
        if prompt_terms.len() >= MAX_PROMPT_TERMS
            || current_len + separator + candidate.len() > MAX_PROMPT_CHARS
        {
            break;
        }

        current_len += separator + candidate.len();
        prompt_terms.push(candidate);
    }

    if prompt_terms.is_empty() {
        Ok(String::new())
    } else {
        Ok(format!("{}.", prompt_terms.join(", ")))
    }
}

fn load_corrections_for_processing(
    app: &tauri::AppHandle,
    language: Option<String>,
    scopes: Option<Vec<String>>,
) -> Result<Vec<StoredCorrection>, String> {
    let conn = open_db(app)?;
    let scopes = map_scopes_for_query(&scopes.unwrap_or_default());
    let language = normalize_language(language.as_deref());
    let sql = format!(
        "SELECT id, wrong, correct
         FROM asr_corrections
         WHERE ({})
           AND (? IS NULL OR language IS NULL OR language = ?)
         ORDER BY LENGTH(normalized_wrong) DESC, confidence DESC, use_count DESC, id DESC",
        query_scope_clause(scopes.len())
    );
    let params = scope_and_language_params(&scopes, &language);
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let items = stmt
        .query_map(rusqlite::params_from_iter(params), |row| {
            Ok(StoredCorrection {
                id: row.get(0)?,
                wrong: row.get(1)?,
                correct: row.get(2)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(items)
}

fn find_existing_correction_memory_for_pair(
    conn: &rusqlite::Connection,
    normalized_wrong: &str,
    normalized_correct: &str,
    language: Option<&str>,
) -> Result<Option<StoredCorrectionMemory>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, scope, use_count, accepted_count, rejected_count, seen_count, user_approved
             FROM asr_corrections
             WHERE normalized_wrong = ?1
               AND normalized_correct = ?2
               AND ((language IS NULL AND ?3 IS NULL) OR language = ?3)
             ORDER BY user_approved DESC, accepted_count DESC, confidence DESC, use_count DESC, id DESC
             LIMIT 1",
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut rows = stmt
        .query(params![normalized_wrong, normalized_correct, language])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    match rows
        .next()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    {
        Some(row) => Ok(Some(StoredCorrectionMemory {
            id: row
                .get(0)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            scope: row
                .get(1)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            use_count: row
                .get(2)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            accepted_count: row
                .get(3)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            rejected_count: row
                .get(4)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            seen_count: row
                .get(5)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            user_approved: row
                .get::<_, i64>(6)
                .map(|value| value != 0)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
        })),
        None => Ok(None),
    }
}

fn load_correction_memory_by_id(
    conn: &rusqlite::Connection,
    id: i64,
) -> Result<Option<StoredCorrectionMemory>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, scope, use_count, accepted_count, rejected_count, seen_count, user_approved
             FROM asr_corrections
             WHERE id = ?1
             LIMIT 1",
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut rows = stmt
        .query(params![id])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    match rows
        .next()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    {
        Some(row) => Ok(Some(StoredCorrectionMemory {
            id: row
                .get(0)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            scope: row
                .get(1)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            use_count: row
                .get(2)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            accepted_count: row
                .get(3)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            rejected_count: row
                .get(4)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            seen_count: row
                .get(5)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            user_approved: row
                .get::<_, i64>(6)
                .map(|value| value != 0)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
        })),
        None => Ok(None),
    }
}

fn correction_pair_exists_anywhere(
    conn: &rusqlite::Connection,
    normalized_wrong: &str,
    normalized_correct: &str,
) -> Result<bool, String> {
    let exists: i64 = conn
        .query_row(
            "SELECT EXISTS(
                SELECT 1
                FROM asr_corrections
                WHERE normalized_wrong = ?1
                  AND normalized_correct = ?2
            )",
            params![normalized_wrong, normalized_correct],
            |row| row.get(0),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(exists != 0)
}

fn ignored_suggestion_exists(
    conn: &rusqlite::Connection,
    normalized_wrong: &str,
    normalized_correct: &str,
    language: Option<&str>,
    scope: &str,
) -> Result<bool, String> {
    let exists: i64 = conn
        .query_row(
            "SELECT EXISTS(
                SELECT 1
                FROM asr_ignored_suggestions
                WHERE normalized_wrong = ?1
                  AND normalized_correct = ?2
                  AND ((language IS NULL AND ?3 IS NULL) OR language = ?3)
                  AND (scope = ?4 OR scope = 'global')
            )",
            params![normalized_wrong, normalized_correct, language, scope],
            |row| row.get(0),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(exists != 0)
}

fn find_ignored_suggestion_id(
    conn: &rusqlite::Connection,
    normalized_wrong: &str,
    normalized_correct: &str,
    language: Option<&str>,
    scope: &str,
) -> Result<Option<i64>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id
             FROM asr_ignored_suggestions
             WHERE normalized_wrong = ?1
               AND normalized_correct = ?2
               AND ((language IS NULL AND ?3 IS NULL) OR language = ?3)
               AND scope = ?4
             ORDER BY ignored_count DESC, id DESC
             LIMIT 1",
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut rows = stmt
        .query(params![
            normalized_wrong,
            normalized_correct,
            language,
            scope
        ])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    match rows
        .next()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    {
        Some(row) => row
            .get(0)
            .map(Some)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e)),
        None => Ok(None),
    }
}

fn vocabulary_term_exists(
    conn: &rusqlite::Connection,
    normalized_term: &str,
    language: Option<&str>,
    scope: &str,
) -> Result<bool, String> {
    let exists: i64 = conn
        .query_row(
            "SELECT EXISTS(
                SELECT 1
                FROM asr_vocabulary_terms
                WHERE normalized_term = ?1
                  AND ((language IS NULL AND ?2 IS NULL) OR language = ?2 OR language IS NULL)
                  AND (scope = ?3 OR scope = 'global')
            )",
            params![normalized_term, language, scope],
            |row| row.get(0),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(exists != 0)
}

fn score_suggestion(
    conn: &rusqlite::Connection,
    wrong: &str,
    normalized_correct: &str,
    correct: &str,
    language: Option<&str>,
    scope: &str,
    before_len: usize,
    after_len: usize,
    accepted_count: i64,
    rejected_count: i64,
    seen_count: i64,
) -> Result<f64, String> {
    let mut score = 0.55_f64;
    if before_len > 1 || after_len > 1 {
        score += 0.08;
    }
    if vocabulary_term_exists(conn, normalized_correct, language, scope)? {
        score += 0.14;
    }
    if compact_lookup_text(wrong) == compact_lookup_text(correct) {
        score += 0.08;
    }
    if phonetic_key(wrong) == phonetic_key(correct) {
        score += 0.08;
    }
    if seen_count >= 2 {
        score += 0.05;
    }
    if accepted_count > 0 {
        score += 0.10;
    }
    if rejected_count > 0 {
        score -= 0.22_f64.min(rejected_count as f64 * 0.12);
    }
    Ok(score.clamp(0.35, 0.98))
}

fn list_ignored_suggestions(
    conn: &rusqlite::Connection,
    language: Option<String>,
    scopes: Option<Vec<String>>,
) -> Result<Vec<AsrIgnoredSuggestion>, String> {
    let scopes = map_scopes_for_query(&scopes.unwrap_or_default());
    let language = normalize_language(language.as_deref());
    let sql = format!(
        "SELECT id, wrong, normalized_wrong, correct, normalized_correct, language, scope, ignored_count, last_ignored_at, created_at, updated_at
         FROM asr_ignored_suggestions
         WHERE ({})
           AND (? IS NULL OR language IS NULL OR language = ?)
         ORDER BY last_ignored_at DESC, id DESC",
        query_scope_clause(scopes.len())
    );
    let params = scope_and_language_params(&scopes, &language);
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(params), |row| {
            Ok(AsrIgnoredSuggestion {
                id: Some(row.get(0)?),
                wrong: row.get(1)?,
                normalized_wrong: Some(row.get(2)?),
                correct: row.get(3)?,
                normalized_correct: Some(row.get(4)?),
                language: row.get(5)?,
                scope: Some(row.get(6)?),
                ignored_count: Some(row.get(7)?),
                last_ignored_at: Some(row.get(8)?),
                created_at: Some(row.get(9)?),
                updated_at: Some(row.get(10)?),
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub fn asr_apply_corrections(
    app: tauri::AppHandle,
    raw_text: String,
    language: Option<String>,
    scopes: Option<Vec<String>>,
) -> Result<AsrCorrectionResult, String> {
    let corrections = load_corrections_for_processing(&app, language, scopes)?;
    let mut corrected_text = raw_text.clone();
    let mut applied = Vec::new();

    for correction in corrections {
        let regex = correction_regex_for_phrase(&correction.wrong)?;
        let matches: Vec<String> = regex
            .find_iter(&corrected_text)
            .map(|m| m.as_str().to_string())
            .collect();
        if matches.is_empty() {
            continue;
        }

        corrected_text = regex
            .replace_all(&corrected_text, correction.correct.as_str())
            .to_string();
        for matched in matches {
            applied.push(AsrCorrectionApplication {
                correction_id: correction.id,
                wrong: correction.wrong.clone(),
                correct: correction.correct.clone(),
                match_text: matched,
            });
        }
    }

    Ok(AsrCorrectionResult {
        raw_text,
        corrected_text,
        applied,
    })
}

#[tauri::command]
pub fn asr_suggest_corrections_from_edit(
    app: tauri::AppHandle,
    before: String,
    after: String,
    language: Option<String>,
    scope: Option<String>,
) -> Result<Vec<AsrLearnedSuggestion>, String> {
    let conn = open_db(&app)?;
    let normalized_language = normalize_language(language.as_deref());
    let normalized_scope = normalize_scope(scope.as_deref());
    let before_tokens_raw = tokenize_words(&before);
    let after_tokens_raw = tokenize_words(&after);
    let before_tokens: Vec<String> = before_tokens_raw
        .iter()
        .map(|token| normalize_lookup_text(token))
        .collect();
    let after_tokens: Vec<String> = after_tokens_raw
        .iter()
        .map(|token| normalize_lookup_text(token))
        .collect();

    let matches = lcs_matches(&before_tokens, &after_tokens);
    let mut last_before = 0usize;
    let mut last_after = 0usize;
    let mut suggestions = Vec::new();
    let mut seen = HashSet::new();

    for (before_match, after_match) in matches
        .into_iter()
        .chain(std::iter::once((before_tokens.len(), after_tokens.len())))
    {
        if before_match > last_before || after_match > last_after {
            let before_slice = &before_tokens_raw[last_before..before_match];
            let after_slice = &after_tokens_raw[last_after..after_match];

            if !before_slice.is_empty()
                && !after_slice.is_empty()
                && before_slice.len() <= MAX_REPLACEMENT_WORDS
                && after_slice.len() <= MAX_REPLACEMENT_WORDS
                && !is_low_value_replacement(before_slice, after_slice)
            {
                let wrong = before_slice.join(" ");
                let correct = after_slice.join(" ");
                let normalized_wrong = normalize_lookup_text(&wrong);
                let normalized_correct = normalize_lookup_text(&correct);

                if !normalized_wrong.is_empty()
                    && !normalized_correct.is_empty()
                    && normalized_wrong != normalized_correct
                    && !correction_pair_exists_anywhere(
                        &conn,
                        &normalized_wrong,
                        &normalized_correct,
                    )?
                    && !ignored_suggestion_exists(
                        &conn,
                        &normalized_wrong,
                        &normalized_correct,
                        normalized_language.as_deref(),
                        &normalized_scope,
                    )?
                    && seen.insert((normalized_wrong.clone(), normalized_correct.clone()))
                {
                    let correction_memory = find_existing_correction_memory_for_pair(
                        &conn,
                        &normalized_wrong,
                        &normalized_correct,
                        normalized_language.as_deref(),
                    )?;
                    let accepted_count = correction_memory
                        .as_ref()
                        .map(|item| item.accepted_count)
                        .unwrap_or(0);
                    let rejected_count = correction_memory
                        .as_ref()
                        .map(|item| item.rejected_count)
                        .unwrap_or(0);
                    let seen_count = correction_memory
                        .as_ref()
                        .map(|item| item.seen_count)
                        .unwrap_or(0);
                    let confidence = score_suggestion(
                        &conn,
                        &wrong,
                        &normalized_correct,
                        &correct,
                        normalized_language.as_deref(),
                        &normalized_scope,
                        before_slice.len(),
                        after_slice.len(),
                        accepted_count,
                        rejected_count,
                        seen_count,
                    )?;
                    suggestions.push(AsrLearnedSuggestion {
                        wrong,
                        normalized_wrong,
                        correct,
                        normalized_correct,
                        language: normalized_language.clone(),
                        scope: normalized_scope.clone(),
                        confidence,
                        accepted_count,
                        rejected_count,
                        seen_count,
                    });
                }
            }
        }

        last_before = before_match.saturating_add(1);
        last_after = after_match.saturating_add(1);
    }

    suggestions.sort_by(|left, right| {
        right
            .confidence
            .partial_cmp(&left.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(suggestions)
}

#[tauri::command]
pub fn asr_ignore_suggestion(
    app: tauri::AppHandle,
    suggestion: AsrLearnedSuggestion,
) -> Result<(), String> {
    let conn = open_db(&app)?;
    let language = normalize_language(suggestion.language.as_deref());
    let scope = normalize_scope(Some(&suggestion.scope));
    if let Some(existing_id) = find_ignored_suggestion_id(
        &conn,
        &suggestion.normalized_wrong,
        &suggestion.normalized_correct,
        language.as_deref(),
        &scope,
    )? {
        conn.execute(
            "UPDATE asr_ignored_suggestions
             SET wrong = ?1,
                 correct = ?2,
                 ignored_count = ignored_count + 1,
                 last_ignored_at = CURRENT_TIMESTAMP,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?3",
            params![suggestion.wrong, suggestion.correct, existing_id],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    } else {
        conn.execute(
            "INSERT INTO asr_ignored_suggestions
             (wrong, normalized_wrong, correct, normalized_correct, language, scope, ignored_count, last_ignored_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, CURRENT_TIMESTAMP)",
            params![
                suggestion.wrong,
                suggestion.normalized_wrong,
                suggestion.correct,
                suggestion.normalized_correct,
                language,
                scope,
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    Ok(())
}

#[tauri::command]
pub fn asr_export_library(
    app: tauri::AppHandle,
    language: Option<String>,
    scopes: Option<Vec<String>>,
) -> Result<AsrExportBundle, String> {
    let conn = open_db(&app)?;
    let vocabulary = asr_vocabulary_list(app.clone(), language.clone(), scopes.clone())?;
    let corrections = asr_corrections_list(app.clone(), language.clone(), scopes.clone(), None)?;
    let ignored_suggestions = list_ignored_suggestions(&conn, language.clone(), scopes.clone())?;
    let voice_examples = asr_voice_examples_list(app, language, scopes)?;
    Ok(AsrExportBundle {
        version: 2,
        vocabulary,
        corrections,
        voice_examples,
        ignored_suggestions,
    })
}

#[tauri::command]
pub fn asr_import_library(
    app: tauri::AppHandle,
    bundle: AsrExportBundle,
) -> Result<BTreeMap<String, usize>, String> {
    let mut counts = BTreeMap::new();
    counts.insert("vocabulary".to_string(), 0);
    counts.insert("corrections".to_string(), 0);
    counts.insert("voiceExamples".to_string(), 0);
    counts.insert("ignoredSuggestions".to_string(), 0);

    for term in bundle.vocabulary {
        asr_vocabulary_upsert(app.clone(), AsrVocabularyTerm { id: None, ..term })?;
        *counts.get_mut("vocabulary").expect("count key exists") += 1;
    }

    for correction in bundle.corrections {
        asr_correction_upsert(
            app.clone(),
            AsrCorrection {
                id: None,
                ..correction
            },
        )?;
        *counts.get_mut("corrections").expect("count key exists") += 1;
    }

    for example in bundle.voice_examples {
        asr_voice_example_upsert(
            app.clone(),
            AsrVoiceExample {
                id: None,
                ..example
            },
        )?;
        *counts.get_mut("voiceExamples").expect("count key exists") += 1;
    }

    let conn = open_db(&app)?;
    for ignored in bundle.ignored_suggestions {
        let normalized_wrong = normalize_lookup_text(&ignored.wrong);
        let normalized_correct = normalize_lookup_text(&ignored.correct);
        if normalized_wrong.is_empty() || normalized_correct.is_empty() {
            continue;
        }
        let ignored_language = normalize_language(ignored.language.as_deref());
        let ignored_scope = normalize_scope(ignored.scope.as_deref());
        if let Some(existing_id) = find_ignored_suggestion_id(
            &conn,
            &normalized_wrong,
            &normalized_correct,
            ignored_language.as_deref(),
            &ignored_scope,
        )? {
            conn.execute(
                "UPDATE asr_ignored_suggestions
                 SET wrong = ?1,
                     correct = ?2,
                     ignored_count = MAX(ignored_count, ?3),
                     last_ignored_at = COALESCE(?4, last_ignored_at),
                     updated_at = COALESCE(?5, CURRENT_TIMESTAMP)
                 WHERE id = ?6",
                params![
                    ignored.wrong,
                    ignored.correct,
                    ignored.ignored_count.unwrap_or(1).max(1),
                    ignored.last_ignored_at,
                    ignored.updated_at,
                    existing_id
                ],
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        } else {
            conn.execute(
                "INSERT INTO asr_ignored_suggestions
                 (wrong, normalized_wrong, correct, normalized_correct, language, scope, ignored_count, last_ignored_at, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, COALESCE(?8, CURRENT_TIMESTAMP), COALESCE(?9, CURRENT_TIMESTAMP), COALESCE(?10, CURRENT_TIMESTAMP))",
                params![
                    ignored.wrong,
                    normalized_wrong,
                    ignored.correct,
                    normalized_correct,
                    ignored_language,
                    ignored_scope,
                    ignored.ignored_count.unwrap_or(1).max(1),
                    ignored.last_ignored_at,
                    ignored.created_at,
                    ignored.updated_at
                ],
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
        *counts
            .get_mut("ignoredSuggestions")
            .expect("count key exists") += 1;
    }

    Ok(counts)
}

#[tauri::command]
pub fn asr_voice_example_suggest_correction(
    app: tauri::AppHandle,
    whisper_output: String,
    expected_text: String,
    language: Option<String>,
    scope: Option<String>,
) -> Result<Option<AsrLearnedSuggestion>, String> {
    Ok(
        asr_suggest_corrections_from_edit(app, whisper_output, expected_text, language, scope)?
            .into_iter()
            .next(),
    )
}
