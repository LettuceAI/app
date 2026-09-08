use std::str::FromStr;

use lettuce_speech::{
    AsrCorrectionRule, AsrLearningRepository, AsrLearningRepositoryError, AsrVocabularyTerm,
};
use lettuce_types::{AsrCorrectionId, AsrVocabularyTermId, TimestampMillis};
use rusqlite::{ToSql, Transaction, TransactionBehavior, params};

use crate::Database;

fn storage(_: impl std::fmt::Debug) -> AsrLearningRepositoryError {
    AsrLearningRepositoryError::Storage
}

fn corrupt(_: impl std::fmt::Debug) -> AsrLearningRepositoryError {
    AsrLearningRepositoryError::InvalidData
}

fn query_clause(scopes: &[String]) -> Result<String, AsrLearningRepositoryError> {
    if scopes.is_empty() {
        return Err(AsrLearningRepositoryError::InvalidData);
    }
    Ok((0..scopes.len())
        .map(|index| format!("?{}", index + 1))
        .collect::<Vec<_>>()
        .join(", "))
}

fn query_values<'a>(scopes: &'a [String], language: &'a Option<String>) -> Vec<&'a dyn ToSql> {
    let mut values = scopes
        .iter()
        .map(|scope| scope as &dyn ToSql)
        .collect::<Vec<_>>();
    values.push(language as &dyn ToSql);
    values.push(language as &dyn ToSql);
    values
}

fn load_vocabulary(
    transaction: &Transaction<'_>,
    id: Option<AsrVocabularyTermId>,
    language: Option<&str>,
    scopes: &[String],
) -> Result<Vec<AsrVocabularyTerm>, AsrLearningRepositoryError> {
    let mut statement;
    let rows = if let Some(id) = id {
        statement = transaction
            .prepare(
                "SELECT id, term, normalized_term, language, category, scope, priority,
                        use_count, created_at, updated_at
                   FROM asr_vocabulary_terms WHERE id = ?1",
            )
            .map_err(storage)?;
        statement
            .query_map([id.to_string()], map_vocabulary_row)
            .map_err(storage)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(corrupt)?
    } else {
        let clause = query_clause(scopes)?;
        statement = transaction
            .prepare(&format!(
                "SELECT id, term, normalized_term, language, category, scope, priority,
                        use_count, created_at, updated_at
                   FROM asr_vocabulary_terms
                  WHERE scope IN ({clause})
                    AND (? IS NULL OR language IS NULL OR language = ?)
                  ORDER BY priority DESC, use_count DESC, updated_at DESC, created_at DESC, id DESC"
            ))
            .map_err(storage)?;
        let language = language.map(str::to_owned);
        let values = query_values(scopes, &language);
        statement
            .query_map(rusqlite::params_from_iter(values), map_vocabulary_row)
            .map_err(storage)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(corrupt)?
    };
    rows.into_iter()
        .map(|row| {
            let term = AsrVocabularyTerm {
                id: AsrVocabularyTermId::from_str(&row.0).map_err(corrupt)?,
                term: row.1,
                normalized_term: row.2,
                language: row.3,
                category: row.4,
                scope: row.5,
                priority: row.6,
                use_count: u64::try_from(row.7).map_err(corrupt)?,
                created_at: TimestampMillis::new(row.8),
                updated_at: TimestampMillis::new(row.9),
            };
            term.validate().map_err(corrupt)?;
            Ok(term)
        })
        .collect()
}

type VocabularyRow = (
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    String,
    i64,
    i64,
    i64,
    i64,
);

fn map_vocabulary_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<VocabularyRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
    ))
}

fn load_corrections(
    transaction: &Transaction<'_>,
    id: Option<AsrCorrectionId>,
    language: Option<&str>,
    scopes: &[String],
) -> Result<Vec<AsrCorrectionRule>, AsrLearningRepositoryError> {
    let mut statement;
    let rows = if let Some(id) = id {
        statement = transaction
            .prepare(
                "SELECT id, wrong, normalized_wrong, correct, normalized_correct, language,
                        scope, confidence, use_count, accepted_count, rejected_count, seen_count,
                        last_seen_at, user_approved, created_at, updated_at
                   FROM asr_corrections WHERE id = ?1",
            )
            .map_err(storage)?;
        statement
            .query_map([id.to_string()], map_correction_row)
            .map_err(storage)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(corrupt)?
    } else {
        let clause = query_clause(scopes)?;
        statement = transaction
            .prepare(&format!(
                "SELECT id, wrong, normalized_wrong, correct, normalized_correct, language,
                        scope, confidence, use_count, accepted_count, rejected_count, seen_count,
                        last_seen_at, user_approved, created_at, updated_at
                   FROM asr_corrections
                  WHERE scope IN ({clause})
                    AND (? IS NULL OR language IS NULL OR language = ?)
                  ORDER BY length(normalized_wrong) DESC, confidence DESC, use_count DESC,
                           created_at DESC, id DESC"
            ))
            .map_err(storage)?;
        let language = language.map(str::to_owned);
        let values = query_values(scopes, &language);
        statement
            .query_map(rusqlite::params_from_iter(values), map_correction_row)
            .map_err(storage)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(corrupt)?
    };
    rows.into_iter()
        .map(|row| {
            let correction = AsrCorrectionRule {
                id: AsrCorrectionId::from_str(&row.0).map_err(corrupt)?,
                wrong: row.1,
                normalized_wrong: row.2,
                correct: row.3,
                normalized_correct: row.4,
                language: row.5,
                scope: row.6,
                confidence: row.7,
                use_count: u64::try_from(row.8).map_err(corrupt)?,
                accepted_count: u64::try_from(row.9).map_err(corrupt)?,
                rejected_count: u64::try_from(row.10).map_err(corrupt)?,
                seen_count: u64::try_from(row.11).map_err(corrupt)?,
                last_seen_at: row.12.map(TimestampMillis::new),
                user_approved: row.13,
                created_at: TimestampMillis::new(row.14),
                updated_at: TimestampMillis::new(row.15),
            };
            correction.validate().map_err(corrupt)?;
            Ok(correction)
        })
        .collect()
}

type CorrectionRow = (
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    String,
    f64,
    i64,
    i64,
    i64,
    i64,
    Option<i64>,
    bool,
    i64,
    i64,
);

fn map_correction_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CorrectionRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
        row.get(10)?,
        row.get(11)?,
        row.get(12)?,
        row.get(13)?,
        row.get(14)?,
        row.get(15)?,
    ))
}

fn to_i64(value: u64) -> Result<i64, AsrLearningRepositoryError> {
    i64::try_from(value).map_err(corrupt)
}

impl AsrLearningRepository for Database {
    fn list_vocabulary(
        &self,
        language: Option<&str>,
        scopes: &[String],
    ) -> Result<Vec<AsrVocabularyTerm>, AsrLearningRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(storage)?;
        let terms = load_vocabulary(&transaction, None, language, scopes)?;
        transaction.commit().map_err(storage)?;
        Ok(terms)
    }

    fn save_vocabulary(
        &self,
        term: AsrVocabularyTerm,
    ) -> Result<AsrVocabularyTerm, AsrLearningRepositoryError> {
        term.validate().map_err(corrupt)?;
        let use_count = to_i64(term.use_count)?;
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let changed = transaction
            .execute(
                "INSERT INTO asr_vocabulary_terms (
                    id, term, normalized_term, language, category, scope, priority,
                    use_count, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(id) DO UPDATE SET
                    term = excluded.term,
                    normalized_term = excluded.normalized_term,
                    language = excluded.language,
                    category = excluded.category,
                    scope = excluded.scope,
                    priority = excluded.priority,
                    use_count = excluded.use_count,
                    updated_at = excluded.updated_at
                 WHERE asr_vocabulary_terms.created_at = excluded.created_at",
                params![
                    term.id.to_string(),
                    term.term,
                    term.normalized_term,
                    term.language,
                    term.category,
                    term.scope,
                    term.priority,
                    use_count,
                    term.created_at.get(),
                    term.updated_at.get(),
                ],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(AsrLearningRepositoryError::Conflict);
        }
        let stored = load_vocabulary(&transaction, Some(term.id), None, &[])?
            .into_iter()
            .next()
            .ok_or(AsrLearningRepositoryError::Storage)?;
        transaction.commit().map_err(storage)?;
        Ok(stored)
    }

    fn delete_vocabulary(&self, id: AsrVocabularyTermId) -> Result<(), AsrLearningRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        connection
            .execute(
                "DELETE FROM asr_vocabulary_terms WHERE id = ?1",
                [id.to_string()],
            )
            .map_err(storage)?;
        Ok(())
    }

    fn list_corrections(
        &self,
        language: Option<&str>,
        scopes: &[String],
    ) -> Result<Vec<AsrCorrectionRule>, AsrLearningRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(storage)?;
        let corrections = load_corrections(&transaction, None, language, scopes)?;
        transaction.commit().map_err(storage)?;
        Ok(corrections)
    }

    fn save_correction(
        &self,
        correction: AsrCorrectionRule,
    ) -> Result<AsrCorrectionRule, AsrLearningRepositoryError> {
        correction.validate().map_err(corrupt)?;
        let use_count = to_i64(correction.use_count)?;
        let accepted_count = to_i64(correction.accepted_count)?;
        let rejected_count = to_i64(correction.rejected_count)?;
        let seen_count = to_i64(correction.seen_count)?;
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let changed = transaction
            .execute(
                "INSERT INTO asr_corrections (
                    id, wrong, normalized_wrong, correct, normalized_correct, language, scope,
                    confidence, use_count, accepted_count, rejected_count, seen_count,
                    last_seen_at, user_approved, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
                 ON CONFLICT(id) DO UPDATE SET
                    wrong = excluded.wrong,
                    normalized_wrong = excluded.normalized_wrong,
                    correct = excluded.correct,
                    normalized_correct = excluded.normalized_correct,
                    language = excluded.language,
                    scope = excluded.scope,
                    confidence = excluded.confidence,
                    use_count = excluded.use_count,
                    accepted_count = excluded.accepted_count,
                    rejected_count = excluded.rejected_count,
                    seen_count = excluded.seen_count,
                    last_seen_at = excluded.last_seen_at,
                    user_approved = excluded.user_approved,
                    updated_at = excluded.updated_at
                 WHERE asr_corrections.created_at = excluded.created_at",
                params![
                    correction.id.to_string(),
                    correction.wrong,
                    correction.normalized_wrong,
                    correction.correct,
                    correction.normalized_correct,
                    correction.language,
                    correction.scope,
                    correction.confidence,
                    use_count,
                    accepted_count,
                    rejected_count,
                    seen_count,
                    correction.last_seen_at.map(TimestampMillis::get),
                    correction.user_approved,
                    correction.created_at.get(),
                    correction.updated_at.get(),
                ],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(AsrLearningRepositoryError::Conflict);
        }
        let stored = load_corrections(&transaction, Some(correction.id), None, &[])?
            .into_iter()
            .next()
            .ok_or(AsrLearningRepositoryError::Storage)?;
        transaction.commit().map_err(storage)?;
        Ok(stored)
    }

    fn delete_correction(&self, id: AsrCorrectionId) -> Result<(), AsrLearningRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        connection
            .execute(
                "DELETE FROM asr_corrections WHERE id = ?1",
                [id.to_string()],
            )
            .map_err(storage)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use lettuce_speech::{AsrLearningLibrary, AsrPromptLibrary};
    use lettuce_types::OperationId;

    use super::*;

    #[test]
    fn library_reopens_with_legacy_prompt_filtering_and_correction_order() {
        let path =
            std::env::temp_dir().join(format!("asr-learning-{}.sqlite3", OperationId::new()));
        let now = TimestampMillis::new(10);
        let global = AsrVocabularyTerm::new(
            "  Lettuce AI  ",
            None,
            Some("  authored category  "),
            Some("global"),
            90,
            now,
        )
        .expect("global term");
        let conversation =
            AsrVocabularyTerm::new("Megalith", Some("en"), None, Some("conversation"), 80, now)
                .expect("conversation term");
        let excluded = AsrVocabularyTerm::new(
            "Hidden Project",
            Some("en"),
            None,
            Some("project"),
            100,
            now,
        )
        .expect("excluded term");
        let long = AsrCorrectionRule::new(
            "lettuce a i",
            "LettuceAI",
            Some("en"),
            Some("conversation"),
            true,
            now,
        )
        .expect("long correction");
        let short = AsrCorrectionRule::new("lettuce", "salad", None, Some("global"), false, now)
            .expect("short correction");
        {
            let database = Database::open(&path).expect("database");
            let library = AsrLearningLibrary::new(&database);
            for term in [global.clone(), conversation.clone(), excluded] {
                assert_eq!(library.save_vocabulary(term.clone()), Ok(term));
            }
            for correction in [long.clone(), short.clone()] {
                assert_eq!(library.save_correction(correction.clone()), Ok(correction));
            }
        }
        let database = Database::open(&path).expect("reopened database");
        let library = AsrLearningLibrary::new(&database);
        let scopes = vec!["Conversation".to_owned(), "GLOBAL".to_owned()];
        assert_eq!(
            library.build_prompt(Some("EN"), &scopes),
            Ok("Lettuce AI, Megalith.".to_owned())
        );
        let (corrected, applied) = library
            .apply_corrections("LETTUCE A I uses lettuce.", Some("en"), &scopes)
            .expect("corrected text");
        assert_eq!(corrected, "LettuceAI uses salad.");
        assert_eq!(
            applied
                .iter()
                .map(|item| item.correction_id.as_str())
                .collect::<Vec<_>>(),
            vec![long.id.to_string(), short.id.to_string()]
        );
        assert_eq!(
            library
                .list_vocabulary(Some("fr"), &["global".to_owned()])
                .expect("language-neutral vocabulary"),
            vec![global.clone()]
        );
        library
            .delete_vocabulary(global.id)
            .expect("delete vocabulary");
        library
            .delete_correction(short.id)
            .expect("delete correction");
        drop(database);
        let database = Database::open(&path).expect("second reopen");
        let library = AsrLearningLibrary::new(&database);
        assert!(
            library
                .list_vocabulary(Some("en"), &["global".to_owned()])
                .expect("vocabulary after delete")
                .is_empty()
        );
        assert!(
            library
                .list_corrections(Some("en"), &["global".to_owned()])
                .expect("corrections after delete")
                .is_empty()
        );
        drop(database);
        std::fs::remove_file(path).expect("cleanup");
    }
}
