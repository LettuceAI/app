use std::collections::HashMap;

use rusqlite::params;

use super::db::DbConnection;
use crate::sync::models::{
    SyncAsrCorrection, SyncAsrIgnoredSuggestion, SyncAsrVocabularyTerm,
};

fn error(error: impl std::fmt::Display) -> String {
    crate::utils::err_to_string(module_path!(), line!(), error)
}

fn key_part(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("-")
}

fn vocabulary_key(term: &SyncAsrVocabularyTerm) -> String {
    format!(
        "{}|{}|{}",
        term.normalized_term,
        key_part(&term.language),
        term.scope
    )
}

fn correction_key(
    normalized_wrong: &str,
    normalized_correct: &str,
    language: &Option<String>,
    scope: &str,
) -> String {
    format!(
        "{}|{}|{}|{}",
        normalized_wrong,
        normalized_correct,
        key_part(language),
        scope
    )
}

pub(crate) fn fetch_vocabulary_terms(
    conn: &DbConnection,
) -> Result<Vec<SyncAsrVocabularyTerm>, String> {
    let mut statement = conn
        .prepare(
            "SELECT term, normalized_term, language, category, scope, priority,
                    use_count, created_at, updated_at
             FROM asr_vocabulary_terms",
        )
        .map_err(error)?;
    let rows = statement
        .query_map([], |row| {
            Ok(SyncAsrVocabularyTerm {
                term: row.get(0)?,
                normalized_term: row.get(1)?,
                language: row.get(2)?,
                category: row.get(3)?,
                scope: row.get(4)?,
                priority: row.get(5)?,
                use_count: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })
        .map_err(error)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(error)
}

pub(crate) fn fetch_corrections(
    conn: &DbConnection,
) -> Result<Vec<SyncAsrCorrection>, String> {
    let mut statement = conn
        .prepare(
            "SELECT wrong, normalized_wrong, correct, normalized_correct, language,
                    scope, confidence, use_count, accepted_count, rejected_count,
                    seen_count, last_seen_at, user_approved, created_at, updated_at
             FROM asr_corrections",
        )
        .map_err(error)?;
    let rows = statement
        .query_map([], |row| {
            Ok(SyncAsrCorrection {
                wrong: row.get(0)?,
                normalized_wrong: row.get(1)?,
                correct: row.get(2)?,
                normalized_correct: row.get(3)?,
                language: row.get(4)?,
                scope: row.get(5)?,
                confidence: row.get(6)?,
                use_count: row.get(7)?,
                accepted_count: row.get(8)?,
                rejected_count: row.get(9)?,
                seen_count: row.get(10)?,
                last_seen_at: row.get(11)?,
                user_approved: row.get(12)?,
                created_at: row.get(13)?,
                updated_at: row.get(14)?,
            })
        })
        .map_err(error)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(error)
}

pub(crate) fn fetch_ignored_suggestions(
    conn: &DbConnection,
) -> Result<Vec<SyncAsrIgnoredSuggestion>, String> {
    let mut statement = conn
        .prepare(
            "SELECT wrong, normalized_wrong, correct, normalized_correct, language,
                    scope, ignored_count, last_ignored_at, created_at, updated_at
             FROM asr_ignored_suggestions",
        )
        .map_err(error)?;
    let rows = statement
        .query_map([], |row| {
            Ok(SyncAsrIgnoredSuggestion {
                wrong: row.get(0)?,
                normalized_wrong: row.get(1)?,
                correct: row.get(2)?,
                normalized_correct: row.get(3)?,
                language: row.get(4)?,
                scope: row.get(5)?,
                ignored_count: row.get(6)?,
                last_ignored_at: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })
        .map_err(error)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(error)
}

pub(crate) fn apply_learning_tables(
    tx: &rusqlite::Transaction<'_>,
    vocabulary_terms: &[SyncAsrVocabularyTerm],
    corrections: &[SyncAsrCorrection],
    ignored_suggestions: &[SyncAsrIgnoredSuggestion],
) -> Result<(), String> {
    let term_links = {
        let mut statement = tx
            .prepare(
                "SELECT e.id, t.normalized_term, t.language, t.scope
                 FROM asr_voice_examples e
                 JOIN asr_vocabulary_terms t ON e.term_id = t.id",
            )
            .map_err(error)?;
        let rows = statement
            .query_map([], |row| {
                let language: Option<String> = row.get(2)?;
                Ok((
                    row.get::<_, i64>(0)?,
                    format!(
                        "{}|{}|{}",
                        row.get::<_, String>(1)?,
                        key_part(&language),
                        row.get::<_, String>(3)?
                    ),
                ))
            })
            .map_err(error)?;
        rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(error)?
    };
    let correction_links = {
        let mut statement = tx
            .prepare(
                "SELECT e.id, c.normalized_wrong, c.normalized_correct, c.language, c.scope
                 FROM asr_voice_examples e
                 JOIN asr_corrections c ON e.correction_id = c.id",
            )
            .map_err(error)?;
        let rows = statement
            .query_map([], |row| {
                let language: Option<String> = row.get(3)?;
                Ok((
                    row.get::<_, i64>(0)?,
                    correction_key(
                        &row.get::<_, String>(1)?,
                        &row.get::<_, String>(2)?,
                        &language,
                        &row.get::<_, String>(4)?,
                    ),
                ))
            })
            .map_err(error)?;
        rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(error)?
    };

    tx.execute("DELETE FROM asr_vocabulary_terms", [])
        .map_err(error)?;
    tx.execute("DELETE FROM asr_corrections", [])
        .map_err(error)?;
    tx.execute("DELETE FROM asr_ignored_suggestions", [])
        .map_err(error)?;

    let mut term_ids = HashMap::new();
    for term in vocabulary_terms {
        tx.execute(
            "INSERT INTO asr_vocabulary_terms (
               term, normalized_term, language, category, scope, priority,
               use_count, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                term.term,
                term.normalized_term,
                term.language,
                term.category,
                term.scope,
                term.priority,
                term.use_count,
                term.created_at,
                term.updated_at,
            ],
        )
        .map_err(error)?;
        term_ids.insert(vocabulary_key(term), tx.last_insert_rowid());
    }

    let mut correction_ids = HashMap::new();
    for correction in corrections {
        tx.execute(
            "INSERT INTO asr_corrections (
               wrong, normalized_wrong, correct, normalized_correct, language, scope,
               confidence, use_count, accepted_count, rejected_count, seen_count,
               last_seen_at, user_approved, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                correction.wrong,
                correction.normalized_wrong,
                correction.correct,
                correction.normalized_correct,
                correction.language,
                correction.scope,
                correction.confidence,
                correction.use_count,
                correction.accepted_count,
                correction.rejected_count,
                correction.seen_count,
                correction.last_seen_at,
                correction.user_approved,
                correction.created_at,
                correction.updated_at,
            ],
        )
        .map_err(error)?;
        correction_ids.insert(
            correction_key(
                &correction.normalized_wrong,
                &correction.normalized_correct,
                &correction.language,
                &correction.scope,
            ),
            tx.last_insert_rowid(),
        );
    }

    for suggestion in ignored_suggestions {
        tx.execute(
            "INSERT OR IGNORE INTO asr_ignored_suggestions (
               wrong, normalized_wrong, correct, normalized_correct, language, scope,
               ignored_count, last_ignored_at, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                suggestion.wrong,
                suggestion.normalized_wrong,
                suggestion.correct,
                suggestion.normalized_correct,
                suggestion.language,
                suggestion.scope,
                suggestion.ignored_count,
                suggestion.last_ignored_at,
                suggestion.created_at,
                suggestion.updated_at,
            ],
        )
        .map_err(error)?;
    }

    for (example_id, key) in term_links {
        if let Some(new_id) = term_ids.get(&key) {
            tx.execute(
                "UPDATE asr_voice_examples SET term_id = ?1 WHERE id = ?2",
                params![new_id, example_id],
            )
            .map_err(error)?;
        }
    }
    for (example_id, key) in correction_links {
        if let Some(new_id) = correction_ids.get(&key) {
            tx.execute(
                "UPDATE asr_voice_examples SET correction_id = ?1 WHERE id = ?2",
                params![new_id, example_id],
            )
            .map_err(error)?;
        }
    }
    Ok(())
}
