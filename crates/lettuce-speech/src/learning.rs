use std::collections::HashSet;

use lettuce_types::{AsrCorrectionId, AsrVocabularyTermId, TimestampMillis};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::{AppliedCorrection, AsrLibraryError, AsrPromptLibrary};

const DEFAULT_SCOPE: &str = "global";
const MAX_PROMPT_BYTES: usize = 240;
const MAX_PROMPT_TERMS: usize = 24;
const MAX_TERM_SCALARS: usize = 4_096;
const MAX_CORRECTION_SCALARS: usize = 4_096;
const MAX_CATEGORY_SCALARS: usize = 512;
const MAX_LANGUAGE_SCALARS: usize = 32;
const MAX_SCOPE_SCALARS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AsrVocabularyTerm {
    pub id: AsrVocabularyTermId,
    pub term: String,
    pub normalized_term: String,
    pub language: Option<String>,
    pub category: Option<String>,
    pub scope: String,
    pub priority: i64,
    pub use_count: u64,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

impl AsrVocabularyTerm {
    pub fn new(
        term: impl Into<String>,
        language: Option<&str>,
        category: Option<&str>,
        scope: Option<&str>,
        priority: i64,
        now: TimestampMillis,
    ) -> Result<Self, AsrLearningError> {
        let term = term.into();
        let value = Self {
            id: AsrVocabularyTermId::new(),
            normalized_term: normalize_lookup_text(&term),
            term,
            language: normalize_language(language),
            category: category.map(str::to_owned),
            scope: normalize_scope(scope),
            priority,
            use_count: 0,
            created_at: now,
            updated_at: now,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), AsrLearningError> {
        validate_authored_text(&self.term, MAX_TERM_SCALARS)?;
        if self.normalized_term != normalize_lookup_text(&self.term)
            || self.language != normalize_language(self.language.as_deref())
            || self.scope != normalize_scope(Some(&self.scope))
            || self.created_at > self.updated_at
        {
            return Err(AsrLearningError::InvalidData);
        }
        validate_optional_bounded(&self.language, MAX_LANGUAGE_SCALARS)?;
        validate_optional_payload(&self.category, MAX_CATEGORY_SCALARS)?;
        validate_bounded_text(&self.scope, MAX_SCOPE_SCALARS)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AsrCorrectionRule {
    pub id: AsrCorrectionId,
    pub wrong: String,
    pub normalized_wrong: String,
    pub correct: String,
    pub normalized_correct: String,
    pub language: Option<String>,
    pub scope: String,
    pub confidence: f64,
    pub use_count: u64,
    pub accepted_count: u64,
    pub rejected_count: u64,
    pub seen_count: u64,
    pub last_seen_at: Option<TimestampMillis>,
    pub user_approved: bool,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

impl AsrCorrectionRule {
    pub fn new(
        wrong: impl Into<String>,
        correct: impl Into<String>,
        language: Option<&str>,
        scope: Option<&str>,
        user_approved: bool,
        now: TimestampMillis,
    ) -> Result<Self, AsrLearningError> {
        let wrong = wrong.into();
        let correct = correct.into();
        let accepted_count = u64::from(user_approved);
        let value = Self {
            id: AsrCorrectionId::new(),
            normalized_wrong: normalize_lookup_text(&wrong),
            wrong,
            normalized_correct: normalize_lookup_text(&correct),
            correct,
            language: normalize_language(language),
            scope: normalize_scope(scope),
            confidence: 0.75,
            use_count: 1,
            accepted_count,
            rejected_count: 0,
            seen_count: accepted_count,
            last_seen_at: user_approved.then_some(now),
            user_approved,
            created_at: now,
            updated_at: now,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), AsrLearningError> {
        validate_authored_text(&self.wrong, MAX_CORRECTION_SCALARS)?;
        validate_authored_text(&self.correct, MAX_CORRECTION_SCALARS)?;
        if self.normalized_wrong != normalize_lookup_text(&self.wrong)
            || self.normalized_correct != normalize_lookup_text(&self.correct)
            || self.language != normalize_language(self.language.as_deref())
            || self.scope != normalize_scope(Some(&self.scope))
            || !self.confidence.is_finite()
            || !(0.0..=1.0).contains(&self.confidence)
            || self.use_count == 0
            || self.created_at > self.updated_at
            || self
                .last_seen_at
                .is_some_and(|value| value > self.updated_at)
        {
            return Err(AsrLearningError::InvalidData);
        }
        validate_optional_bounded(&self.language, MAX_LANGUAGE_SCALARS)?;
        validate_bounded_text(&self.scope, MAX_SCOPE_SCALARS)
    }
}

pub trait AsrLearningRepository: Send + Sync {
    fn list_vocabulary(
        &self,
        language: Option<&str>,
        scopes: &[String],
    ) -> Result<Vec<AsrVocabularyTerm>, AsrLearningRepositoryError>;
    fn save_vocabulary(
        &self,
        term: AsrVocabularyTerm,
    ) -> Result<AsrVocabularyTerm, AsrLearningRepositoryError>;
    fn delete_vocabulary(&self, id: AsrVocabularyTermId) -> Result<(), AsrLearningRepositoryError>;
    fn list_corrections(
        &self,
        language: Option<&str>,
        scopes: &[String],
    ) -> Result<Vec<AsrCorrectionRule>, AsrLearningRepositoryError>;
    fn save_correction(
        &self,
        correction: AsrCorrectionRule,
    ) -> Result<AsrCorrectionRule, AsrLearningRepositoryError>;
    fn delete_correction(&self, id: AsrCorrectionId) -> Result<(), AsrLearningRepositoryError>;
}

#[derive(Debug)]
pub struct AsrLearningLibrary<'a, R: ?Sized> {
    repository: &'a R,
}

impl<'a, R: ?Sized> AsrLearningLibrary<'a, R> {
    #[must_use]
    pub const fn new(repository: &'a R) -> Self {
        Self { repository }
    }
}

impl<R: AsrLearningRepository + ?Sized> AsrLearningLibrary<'_, R> {
    pub fn list_vocabulary(
        &self,
        language: Option<&str>,
        scopes: &[String],
    ) -> Result<Vec<AsrVocabularyTerm>, AsrLearningError> {
        let (language, scopes) = normalize_query(language, scopes)?;
        self.repository
            .list_vocabulary(language.as_deref(), &scopes)
            .map_err(Into::into)
    }

    pub fn save_vocabulary(
        &self,
        term: AsrVocabularyTerm,
    ) -> Result<AsrVocabularyTerm, AsrLearningError> {
        term.validate()?;
        self.repository.save_vocabulary(term).map_err(Into::into)
    }

    pub fn delete_vocabulary(&self, id: AsrVocabularyTermId) -> Result<(), AsrLearningError> {
        self.repository.delete_vocabulary(id).map_err(Into::into)
    }

    pub fn list_corrections(
        &self,
        language: Option<&str>,
        scopes: &[String],
    ) -> Result<Vec<AsrCorrectionRule>, AsrLearningError> {
        let (language, scopes) = normalize_query(language, scopes)?;
        self.repository
            .list_corrections(language.as_deref(), &scopes)
            .map_err(Into::into)
    }

    pub fn save_correction(
        &self,
        correction: AsrCorrectionRule,
    ) -> Result<AsrCorrectionRule, AsrLearningError> {
        correction.validate()?;
        self.repository
            .save_correction(correction)
            .map_err(Into::into)
    }

    pub fn delete_correction(&self, id: AsrCorrectionId) -> Result<(), AsrLearningError> {
        self.repository.delete_correction(id).map_err(Into::into)
    }
}

impl<R: AsrLearningRepository + ?Sized> AsrPromptLibrary for AsrLearningLibrary<'_, R> {
    fn build_prompt(
        &self,
        language: Option<&str>,
        scopes: &[String],
    ) -> Result<String, AsrLibraryError> {
        let (language, scopes) =
            normalize_query(language, scopes).map_err(|_| AsrLibraryError::InvalidData)?;
        let terms = self
            .repository
            .list_vocabulary(language.as_deref(), &scopes)
            .map_err(map_repository_error)?;
        build_prompt(terms).map_err(|_| AsrLibraryError::InvalidData)
    }

    fn apply_corrections(
        &self,
        text: &str,
        language: Option<&str>,
        scopes: &[String],
    ) -> Result<(String, Vec<AppliedCorrection>), AsrLibraryError> {
        let (language, scopes) =
            normalize_query(language, scopes).map_err(|_| AsrLibraryError::InvalidData)?;
        let corrections = self
            .repository
            .list_corrections(language.as_deref(), &scopes)
            .map_err(map_repository_error)?;
        apply_corrections(text, &corrections).map_err(|_| AsrLibraryError::InvalidData)
    }
}

fn build_prompt(terms: Vec<AsrVocabularyTerm>) -> Result<String, AsrLearningError> {
    let mut prompt_terms = Vec::new();
    let mut seen = HashSet::new();
    let mut current_len = 0usize;
    for term in terms {
        term.validate()?;
        let candidate = normalize_whitespace(&term.term);
        let dedupe_key = normalize_lookup_text(&candidate);
        if candidate.is_empty() || dedupe_key.is_empty() || !seen.insert(dedupe_key) {
            continue;
        }
        let separator = usize::from(!prompt_terms.is_empty()) * 2;
        if prompt_terms.len() >= MAX_PROMPT_TERMS
            || current_len + separator + candidate.len() > MAX_PROMPT_BYTES
        {
            break;
        }
        current_len += separator + candidate.len();
        prompt_terms.push(candidate);
    }
    Ok(if prompt_terms.is_empty() {
        String::new()
    } else {
        format!("{}.", prompt_terms.join(", "))
    })
}

fn apply_corrections(
    text: &str,
    corrections: &[AsrCorrectionRule],
) -> Result<(String, Vec<AppliedCorrection>), AsrLearningError> {
    let mut corrected = text.to_owned();
    let mut applied = Vec::new();
    for correction in corrections {
        correction.validate()?;
        let escaped = correction
            .wrong
            .split_whitespace()
            .map(regex::escape)
            .collect::<Vec<_>>()
            .join(r"\s+");
        let pattern = Regex::new(&format!(r"(?i)\b{escaped}\b"))
            .map_err(|_| AsrLearningError::InvalidData)?;
        let matches = pattern
            .find_iter(&corrected)
            .map(|found| found.as_str().to_owned())
            .collect::<Vec<_>>();
        if matches.is_empty() {
            continue;
        }
        corrected = pattern
            .replace_all(&corrected, regex::NoExpand(&correction.correct))
            .into_owned();
        applied.extend(matches.into_iter().map(|matched_text| AppliedCorrection {
            correction_id: correction.id.to_string(),
            wrong: correction.wrong.clone(),
            correct: correction.correct.clone(),
            matched_text,
        }));
    }
    Ok((corrected, applied))
}

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_lookup_text(value: &str) -> String {
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

fn normalize_language(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase)
}

fn normalize_scope(value: Option<&str>) -> String {
    let value = value.unwrap_or(DEFAULT_SCOPE).trim();
    if value.is_empty() {
        DEFAULT_SCOPE.to_owned()
    } else {
        value.to_ascii_lowercase()
    }
}

fn normalize_query(
    language: Option<&str>,
    scopes: &[String],
) -> Result<(Option<String>, Vec<String>), AsrLearningError> {
    let language = normalize_language(language);
    validate_optional_bounded(&language, MAX_LANGUAGE_SCALARS)?;
    if scopes.len() > 8 {
        return Err(AsrLearningError::InvalidData);
    }
    let source = if scopes.is_empty() {
        vec![DEFAULT_SCOPE.to_owned()]
    } else {
        scopes
            .iter()
            .map(|scope| normalize_scope(Some(scope)))
            .collect()
    };
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for scope in source {
        validate_bounded_text(&scope, MAX_SCOPE_SCALARS)?;
        if seen.insert(scope.clone()) {
            normalized.push(scope);
        }
    }
    Ok((language, normalized))
}

fn validate_bounded_text(value: &str, max: usize) -> Result<(), AsrLearningError> {
    if value.trim() != value
        || value.is_empty()
        || value.chars().count() > max
        || value.chars().any(char::is_control)
    {
        return Err(AsrLearningError::InvalidData);
    }
    Ok(())
}

fn validate_optional_bounded(value: &Option<String>, max: usize) -> Result<(), AsrLearningError> {
    if let Some(value) = value {
        validate_bounded_text(value, max)?;
    }
    Ok(())
}

fn validate_authored_text(value: &str, max: usize) -> Result<(), AsrLearningError> {
    if value.is_empty()
        || value.chars().count() > max
        || value.contains('\0')
        || normalize_lookup_text(value).is_empty()
    {
        return Err(AsrLearningError::InvalidData);
    }
    Ok(())
}

fn validate_optional_payload(value: &Option<String>, max: usize) -> Result<(), AsrLearningError> {
    if value
        .as_ref()
        .is_some_and(|value| value.chars().count() > max || value.contains('\0'))
    {
        return Err(AsrLearningError::InvalidData);
    }
    Ok(())
}

fn map_repository_error(error: AsrLearningRepositoryError) -> AsrLibraryError {
    match error {
        AsrLearningRepositoryError::InvalidData => AsrLibraryError::InvalidData,
        AsrLearningRepositoryError::NotFound
        | AsrLearningRepositoryError::Conflict
        | AsrLearningRepositoryError::Storage => AsrLibraryError::Unavailable,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AsrLearningRepositoryError {
    #[error("ASR learning record was not found")]
    NotFound,
    #[error("ASR learning record conflicts with stored data")]
    Conflict,
    #[error("ASR learning record is invalid")]
    InvalidData,
    #[error("ASR learning storage failed")]
    Storage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AsrLearningError {
    #[error("ASR learning data are invalid")]
    InvalidData,
    #[error("ASR learning persistence failed: {0}")]
    Repository(#[from] AsrLearningRepositoryError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_preserves_legacy_order_deduplication_and_bound() {
        let now = TimestampMillis::new(10);
        let mut terms = vec![
            AsrVocabularyTerm::new("Lettuce AI", Some("EN"), None, Some("GLOBAL"), 80, now)
                .expect("term"),
            AsrVocabularyTerm::new("Lettuce   AI", Some("en"), None, Some("global"), 70, now)
                .expect("term"),
        ];
        for index in 0..30 {
            terms.push(
                AsrVocabularyTerm::new(
                    format!("term {index}"),
                    Some("en"),
                    None,
                    Some("global"),
                    50,
                    now,
                )
                .expect("term"),
            );
        }
        let prompt = build_prompt(terms).expect("prompt");
        assert!(prompt.starts_with("Lettuce AI, term 0"));
        assert_eq!(prompt.matches("Lettuce AI").count(), 1);
        assert!(prompt.len() <= MAX_PROMPT_BYTES + 1);
        assert!(prompt.ends_with('.'));
    }

    #[test]
    fn corrections_preserve_longest_first_case_insensitive_replacement() {
        let now = TimestampMillis::new(10);
        let long = AsrCorrectionRule::new(
            "lettuce a i",
            "LettuceAI",
            Some("en"),
            Some("global"),
            true,
            now,
        )
        .expect("long correction");
        let short =
            AsrCorrectionRule::new("lettuce", "salad", Some("en"), Some("global"), false, now)
                .expect("short correction");
        let (text, applied) =
            apply_corrections("LETTUCE A I uses lettuce.", &[long, short]).expect("corrected");
        assert_eq!(text, "LettuceAI uses salad.");
        assert_eq!(applied.len(), 2);
        assert_eq!(applied[0].matched_text, "LETTUCE A I");
    }

    #[test]
    fn correction_replacements_preserve_dollar_characters() {
        let correction = AsrCorrectionRule::new(
            "five dollars",
            "$5",
            Some("en"),
            Some("global"),
            true,
            TimestampMillis::new(10),
        )
        .expect("correction");
        let (text, _) = apply_corrections("It costs five dollars.", &[correction])
            .expect("literal replacement");
        assert_eq!(text, "It costs $5.");
    }
}
