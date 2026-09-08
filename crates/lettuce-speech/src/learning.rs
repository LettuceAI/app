use std::collections::HashSet;

use lettuce_types::{
    AsrCorrectionId, AsrIgnoredSuggestionId, AsrVocabularyTermId, AsrVoiceExampleId, AssetId,
    TimestampMillis,
};
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
const MAX_REPLACEMENT_WORDS: usize = 5;

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AsrLearnedSuggestion {
    pub wrong: String,
    pub normalized_wrong: String,
    pub correct: String,
    pub normalized_correct: String,
    pub language: Option<String>,
    pub scope: String,
    pub confidence: f64,
    pub accepted_count: u64,
    pub rejected_count: u64,
    pub seen_count: u64,
}

impl AsrLearnedSuggestion {
    pub fn validate(&self) -> Result<(), AsrLearningError> {
        validate_authored_text(&self.wrong, MAX_CORRECTION_SCALARS)?;
        validate_authored_text(&self.correct, MAX_CORRECTION_SCALARS)?;
        if self.normalized_wrong != normalize_lookup_text(&self.wrong)
            || self.normalized_correct != normalize_lookup_text(&self.correct)
            || self.normalized_wrong == self.normalized_correct
            || self.language != normalize_language(self.language.as_deref())
            || self.scope != normalize_scope(Some(&self.scope))
            || !self.confidence.is_finite()
            || !(0.0..=1.0).contains(&self.confidence)
        {
            return Err(AsrLearningError::InvalidData);
        }
        validate_optional_bounded(&self.language, MAX_LANGUAGE_SCALARS)?;
        validate_bounded_text(&self.scope, MAX_SCOPE_SCALARS)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AsrIgnoredSuggestion {
    pub id: AsrIgnoredSuggestionId,
    pub wrong: String,
    pub normalized_wrong: String,
    pub correct: String,
    pub normalized_correct: String,
    pub language: Option<String>,
    pub scope: String,
    pub ignored_count: u64,
    pub last_ignored_at: TimestampMillis,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

impl AsrIgnoredSuggestion {
    pub fn validate(&self) -> Result<(), AsrLearningError> {
        validate_authored_text(&self.wrong, MAX_CORRECTION_SCALARS)?;
        validate_authored_text(&self.correct, MAX_CORRECTION_SCALARS)?;
        if self.normalized_wrong != normalize_lookup_text(&self.wrong)
            || self.normalized_correct != normalize_lookup_text(&self.correct)
            || self.normalized_wrong == self.normalized_correct
            || self.language != normalize_language(self.language.as_deref())
            || self.scope != normalize_scope(Some(&self.scope))
            || self.ignored_count == 0
            || self.created_at > self.updated_at
            || self.last_ignored_at > self.updated_at
        {
            return Err(AsrLearningError::InvalidData);
        }
        validate_optional_bounded(&self.language, MAX_LANGUAGE_SCALARS)?;
        validate_bounded_text(&self.scope, MAX_SCOPE_SCALARS)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AsrVoiceExample {
    pub id: AsrVoiceExampleId,
    pub audio_asset_id: AssetId,
    pub expected_text: String,
    pub normalized_expected_text: String,
    pub whisper_output: Option<String>,
    pub normalized_whisper_output: Option<String>,
    pub language: Option<String>,
    pub scope: String,
    pub vocabulary_term_id: Option<AsrVocabularyTermId>,
    pub correction_id: Option<AsrCorrectionId>,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AsrLearningBatch {
    pub vocabulary: Vec<AsrVocabularyTerm>,
    pub corrections: Vec<AsrCorrectionRule>,
    pub ignored_suggestions: Vec<AsrIgnoredSuggestion>,
    pub voice_examples: Vec<AsrVoiceExample>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsrLearningImportReceipt {
    pub vocabulary_count: u64,
    pub correction_count: u64,
    pub ignored_suggestion_count: u64,
    pub voice_example_count: u64,
}

impl AsrVoiceExample {
    pub fn new(
        audio_asset_id: AssetId,
        expected_text: impl Into<String>,
        whisper_output: Option<String>,
        language: Option<&str>,
        scope: Option<&str>,
        now: TimestampMillis,
    ) -> Result<Self, AsrLearningError> {
        let expected_text = expected_text.into();
        let value = Self {
            id: AsrVoiceExampleId::new(),
            audio_asset_id,
            normalized_expected_text: normalize_lookup_text(&expected_text),
            expected_text,
            normalized_whisper_output: whisper_output
                .as_deref()
                .map(normalize_lookup_text)
                .filter(|value| !value.is_empty()),
            whisper_output,
            language: normalize_language(language),
            scope: normalize_scope(scope),
            vocabulary_term_id: None,
            correction_id: None,
            created_at: now,
            updated_at: now,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), AsrLearningError> {
        validate_authored_text(&self.expected_text, MAX_CORRECTION_SCALARS)?;
        validate_optional_payload(&self.whisper_output, MAX_CORRECTION_SCALARS)?;
        let expected_whisper = self
            .whisper_output
            .as_deref()
            .map(normalize_lookup_text)
            .filter(|value| !value.is_empty());
        if self.normalized_expected_text != normalize_lookup_text(&self.expected_text)
            || self.normalized_whisper_output != expected_whisper
            || self.language != normalize_language(self.language.as_deref())
            || self.scope != normalize_scope(Some(&self.scope))
            || self.created_at > self.updated_at
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
    fn get_vocabulary(
        &self,
        id: AsrVocabularyTermId,
    ) -> Result<Option<AsrVocabularyTerm>, AsrLearningRepositoryError>;
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
    fn get_correction(
        &self,
        id: AsrCorrectionId,
    ) -> Result<Option<AsrCorrectionRule>, AsrLearningRepositoryError>;
    fn delete_correction(&self, id: AsrCorrectionId) -> Result<(), AsrLearningRepositoryError>;
    fn find_correction_pair(
        &self,
        normalized_wrong: &str,
        normalized_correct: &str,
        language: Option<&str>,
    ) -> Result<Option<AsrCorrectionRule>, AsrLearningRepositoryError>;
    fn correction_pair_exists(
        &self,
        normalized_wrong: &str,
        normalized_correct: &str,
    ) -> Result<bool, AsrLearningRepositoryError>;
    fn vocabulary_term_exists(
        &self,
        normalized_term: &str,
        language: Option<&str>,
        scope: &str,
    ) -> Result<bool, AsrLearningRepositoryError>;
    fn find_ignored_suggestion(
        &self,
        normalized_wrong: &str,
        normalized_correct: &str,
        language: Option<&str>,
        scope: &str,
        include_global: bool,
    ) -> Result<Option<AsrIgnoredSuggestion>, AsrLearningRepositoryError>;
    fn save_ignored_suggestion(
        &self,
        suggestion: AsrIgnoredSuggestion,
    ) -> Result<AsrIgnoredSuggestion, AsrLearningRepositoryError>;
    fn list_ignored_suggestions(
        &self,
        language: Option<&str>,
        scopes: &[String],
    ) -> Result<Vec<AsrIgnoredSuggestion>, AsrLearningRepositoryError>;
    fn list_voice_examples(
        &self,
        language: Option<&str>,
        scopes: &[String],
    ) -> Result<Vec<AsrVoiceExample>, AsrLearningRepositoryError>;
    fn save_voice_example(
        &self,
        example: AsrVoiceExample,
    ) -> Result<AsrVoiceExample, AsrLearningRepositoryError>;
    fn delete_voice_example(&self, id: AsrVoiceExampleId)
    -> Result<(), AsrLearningRepositoryError>;
    fn import_learning_batch(
        &self,
        batch: AsrLearningBatch,
    ) -> Result<AsrLearningImportReceipt, AsrLearningRepositoryError>;
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

    pub fn get_vocabulary(
        &self,
        id: AsrVocabularyTermId,
    ) -> Result<Option<AsrVocabularyTerm>, AsrLearningError> {
        self.repository.get_vocabulary(id).map_err(Into::into)
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

    pub fn get_correction(
        &self,
        id: AsrCorrectionId,
    ) -> Result<Option<AsrCorrectionRule>, AsrLearningError> {
        self.repository.get_correction(id).map_err(Into::into)
    }

    pub fn delete_correction(&self, id: AsrCorrectionId) -> Result<(), AsrLearningError> {
        self.repository.delete_correction(id).map_err(Into::into)
    }

    pub fn suggest_corrections_from_edit(
        &self,
        before: &str,
        after: &str,
        language: Option<&str>,
        scope: Option<&str>,
    ) -> Result<Vec<AsrLearnedSuggestion>, AsrLearningError> {
        let language = normalize_language(language);
        let scope = normalize_scope(scope);
        validate_optional_bounded(&language, MAX_LANGUAGE_SCALARS)?;
        validate_bounded_text(&scope, MAX_SCOPE_SCALARS)?;
        let before_raw = tokenize_words(before);
        let after_raw = tokenize_words(after);
        let before_tokens = before_raw
            .iter()
            .map(|token| normalize_lookup_text(token))
            .collect::<Vec<_>>();
        let after_tokens = after_raw
            .iter()
            .map(|token| normalize_lookup_text(token))
            .collect::<Vec<_>>();
        let mut last_before = 0;
        let mut last_after = 0;
        let mut suggestions = Vec::new();
        let mut seen = HashSet::new();
        for (before_match, after_match) in lcs_matches(&before_tokens, &after_tokens)
            .into_iter()
            .chain(std::iter::once((before_tokens.len(), after_tokens.len())))
        {
            if before_match > last_before || after_match > last_after {
                let before_slice = &before_raw[last_before..before_match];
                let after_slice = &after_raw[last_after..after_match];
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
                    let pair = (normalized_wrong.clone(), normalized_correct.clone());
                    let correction_exists = self
                        .repository
                        .correction_pair_exists(&normalized_wrong, &normalized_correct)?;
                    let ignored = self.repository.find_ignored_suggestion(
                        &normalized_wrong,
                        &normalized_correct,
                        language.as_deref(),
                        &scope,
                        true,
                    )?;
                    if !normalized_wrong.is_empty()
                        && !normalized_correct.is_empty()
                        && normalized_wrong != normalized_correct
                        && !correction_exists
                        && ignored.is_none()
                        && seen.insert(pair)
                    {
                        let memory = self.repository.find_correction_pair(
                            &normalized_wrong,
                            &normalized_correct,
                            language.as_deref(),
                        )?;
                        let accepted_count = memory.as_ref().map_or(0, |item| item.accepted_count);
                        let rejected_count = memory.as_ref().map_or(0, |item| item.rejected_count);
                        let seen_count = memory.as_ref().map_or(0, |item| item.seen_count);
                        let vocabulary_signal = self.repository.vocabulary_term_exists(
                            &normalized_correct,
                            language.as_deref(),
                            &scope,
                        )?;
                        let confidence = score_suggestion(
                            &wrong,
                            &correct,
                            SuggestionScoreEvidence {
                                before_words: before_slice.len(),
                                after_words: after_slice.len(),
                                vocabulary_signal,
                                accepted_count,
                                rejected_count,
                                seen_count,
                            },
                        );
                        suggestions.push(AsrLearnedSuggestion {
                            wrong,
                            normalized_wrong,
                            correct,
                            normalized_correct,
                            language: language.clone(),
                            scope: scope.clone(),
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
        suggestions.sort_by(|left, right| right.confidence.total_cmp(&left.confidence));
        Ok(suggestions)
    }

    pub fn accept_suggestion(
        &self,
        suggestion: AsrLearnedSuggestion,
        now: TimestampMillis,
    ) -> Result<AsrCorrectionRule, AsrLearningError> {
        suggestion.validate()?;
        let existing = self.repository.find_correction_pair(
            &suggestion.normalized_wrong,
            &suggestion.normalized_correct,
            suggestion.language.as_deref(),
        )?;
        let correction = if let Some(existing) = existing {
            AsrCorrectionRule {
                id: existing.id,
                wrong: suggestion.wrong,
                normalized_wrong: suggestion.normalized_wrong,
                correct: suggestion.correct,
                normalized_correct: suggestion.normalized_correct,
                language: suggestion.language,
                scope: preferred_scope(
                    Some(&existing.scope),
                    &suggestion.scope,
                    existing.accepted_count + 1,
                ),
                confidence: suggestion.confidence,
                use_count: existing.use_count.max(1),
                accepted_count: existing.accepted_count + 1,
                rejected_count: existing.rejected_count,
                seen_count: existing.seen_count + 1,
                last_seen_at: Some(now),
                user_approved: true,
                created_at: existing.created_at,
                updated_at: now,
            }
        } else {
            AsrCorrectionRule {
                id: AsrCorrectionId::new(),
                wrong: suggestion.wrong,
                normalized_wrong: suggestion.normalized_wrong,
                correct: suggestion.correct,
                normalized_correct: suggestion.normalized_correct,
                language: suggestion.language,
                scope: preferred_scope(None, &suggestion.scope, 1),
                confidence: suggestion.confidence,
                use_count: 1,
                accepted_count: 1,
                rejected_count: 0,
                seen_count: 1,
                last_seen_at: Some(now),
                user_approved: true,
                created_at: now,
                updated_at: now,
            }
        };
        correction.validate()?;
        self.repository
            .save_correction(correction)
            .map_err(Into::into)
    }

    pub fn ignore_suggestion(
        &self,
        suggestion: AsrLearnedSuggestion,
        now: TimestampMillis,
    ) -> Result<AsrIgnoredSuggestion, AsrLearningError> {
        suggestion.validate()?;
        let existing = self.repository.find_ignored_suggestion(
            &suggestion.normalized_wrong,
            &suggestion.normalized_correct,
            suggestion.language.as_deref(),
            &suggestion.scope,
            false,
        )?;
        let ignored = AsrIgnoredSuggestion {
            id: existing
                .as_ref()
                .map_or_else(AsrIgnoredSuggestionId::new, |item| item.id),
            wrong: suggestion.wrong,
            normalized_wrong: suggestion.normalized_wrong,
            correct: suggestion.correct,
            normalized_correct: suggestion.normalized_correct,
            language: suggestion.language,
            scope: suggestion.scope,
            ignored_count: existing.as_ref().map_or(1, |item| item.ignored_count + 1),
            last_ignored_at: now,
            created_at: existing.as_ref().map_or(now, |item| item.created_at),
            updated_at: now,
        };
        ignored.validate()?;
        self.repository
            .save_ignored_suggestion(ignored)
            .map_err(Into::into)
    }

    pub fn list_voice_examples(
        &self,
        language: Option<&str>,
        scopes: &[String],
    ) -> Result<Vec<AsrVoiceExample>, AsrLearningError> {
        let (language, scopes) = normalize_query(language, scopes)?;
        self.repository
            .list_voice_examples(language.as_deref(), &scopes)
            .map_err(Into::into)
    }

    pub fn save_voice_example(
        &self,
        example: AsrVoiceExample,
    ) -> Result<AsrVoiceExample, AsrLearningError> {
        example.validate()?;
        self.repository
            .save_voice_example(example)
            .map_err(Into::into)
    }

    pub fn list_ignored_suggestions(
        &self,
        language: Option<&str>,
        scopes: &[String],
    ) -> Result<Vec<AsrIgnoredSuggestion>, AsrLearningError> {
        let (language, scopes) = normalize_query(language, scopes)?;
        self.repository
            .list_ignored_suggestions(language.as_deref(), &scopes)
            .map_err(Into::into)
    }

    pub fn import_learning_batch(
        &self,
        batch: AsrLearningBatch,
    ) -> Result<AsrLearningImportReceipt, AsrLearningError> {
        for term in &batch.vocabulary {
            term.validate()?;
        }
        for correction in &batch.corrections {
            correction.validate()?;
        }
        for ignored in &batch.ignored_suggestions {
            ignored.validate()?;
        }
        for example in &batch.voice_examples {
            example.validate()?;
        }
        self.repository
            .import_learning_batch(batch)
            .map_err(Into::into)
    }

    pub fn delete_voice_example(&self, id: AsrVoiceExampleId) -> Result<(), AsrLearningError> {
        self.repository.delete_voice_example(id).map_err(Into::into)
    }

    pub fn suggest_voice_example_correction(
        &self,
        example: &AsrVoiceExample,
    ) -> Result<Option<AsrLearnedSuggestion>, AsrLearningError> {
        example.validate()?;
        let Some(whisper_output) = example.whisper_output.as_deref() else {
            return Ok(None);
        };
        Ok(self
            .suggest_corrections_from_edit(
                whisper_output,
                &example.expected_text,
                example.language.as_deref(),
                Some(&example.scope),
            )?
            .into_iter()
            .next())
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

fn compact_lookup_text(value: &str) -> String {
    normalize_lookup_text(value).replace(' ', "")
}

fn phonetic_key(value: &str) -> String {
    let mut key = String::new();
    let mut last = '\0';
    for character in compact_lookup_text(value).chars() {
        let mapped = match character {
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

fn scope_rank(scope: &str) -> u8 {
    match scope {
        "conversation" => 0,
        "character" => 1,
        "project" => 2,
        "global" => 3,
        _ => 0,
    }
}

fn promoted_scope(scope: &str, accepted_count: u64) -> String {
    let scope = normalize_scope(Some(scope));
    match scope.as_str() {
        "conversation" if accepted_count >= 4 => "global".to_owned(),
        "conversation" if accepted_count >= 2 => "project".to_owned(),
        "character" | "project" if accepted_count >= 4 => "global".to_owned(),
        _ => scope,
    }
}

fn preferred_scope(existing: Option<&str>, requested: &str, accepted_count: u64) -> String {
    let requested = promoted_scope(requested, accepted_count);
    match existing.map(|scope| normalize_scope(Some(scope))) {
        Some(existing) if scope_rank(&existing) >= scope_rank(&requested) => {
            promoted_scope(&existing, accepted_count)
        }
        _ => requested,
    }
}

fn tokenize_words(value: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    for character in value.chars() {
        if character.is_alphanumeric() || character == '\'' {
            current.push(character);
        } else if !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

fn is_low_value_replacement(before: &[String], after: &[String]) -> bool {
    const STOPWORDS: &[&str] = &[
        "a", "an", "and", "are", "be", "but", "can", "did", "do", "for", "from", "go", "have",
        "he", "her", "him", "i", "in", "is", "it", "its", "me", "my", "not", "of", "on", "or",
        "our", "she", "that", "the", "their", "them", "there", "they", "this", "to", "us", "was",
        "we", "were", "with", "you", "your",
    ];
    let all_common = |tokens: &[String]| {
        tokens
            .iter()
            .all(|token| STOPWORDS.contains(&normalize_lookup_text(token).as_str()))
    };
    let very_short = before.len() == 1
        && after.len() == 1
        && compact_lookup_text(&before[0]).len() <= 3
        && compact_lookup_text(&after[0]).len() <= 3;
    (all_common(before) && all_common(after)) || very_short
}

fn lcs_matches(before: &[String], after: &[String]) -> Vec<(usize, usize)> {
    let mut lengths = vec![vec![0; after.len() + 1]; before.len() + 1];
    for before_index in (0..before.len()).rev() {
        for after_index in (0..after.len()).rev() {
            lengths[before_index][after_index] = if before[before_index] == after[after_index] {
                lengths[before_index + 1][after_index + 1] + 1
            } else {
                lengths[before_index + 1][after_index].max(lengths[before_index][after_index + 1])
            };
        }
    }
    let mut before_index = 0;
    let mut after_index = 0;
    let mut matches = Vec::new();
    while before_index < before.len() && after_index < after.len() {
        if before[before_index] == after[after_index] {
            matches.push((before_index, after_index));
            before_index += 1;
            after_index += 1;
        } else if lengths[before_index + 1][after_index] >= lengths[before_index][after_index + 1] {
            before_index += 1;
        } else {
            after_index += 1;
        }
    }
    matches
}

#[derive(Debug, Clone, Copy)]
struct SuggestionScoreEvidence {
    before_words: usize,
    after_words: usize,
    vocabulary_signal: bool,
    accepted_count: u64,
    rejected_count: u64,
    seen_count: u64,
}

fn score_suggestion(wrong: &str, correct: &str, evidence: SuggestionScoreEvidence) -> f64 {
    let mut score = 0.55_f64;
    if evidence.before_words > 1 || evidence.after_words > 1 {
        score += 0.08;
    }
    if evidence.vocabulary_signal {
        score += 0.14;
    }
    if compact_lookup_text(wrong) == compact_lookup_text(correct) {
        score += 0.08;
    }
    if phonetic_key(wrong) == phonetic_key(correct) {
        score += 0.08;
    }
    if evidence.seen_count >= 2 {
        score += 0.05;
    }
    if evidence.accepted_count > 0 {
        score += 0.10;
    }
    if evidence.rejected_count > 0 {
        score -= 0.22_f64.min(evidence.rejected_count as f64 * 0.12);
    }
    score.clamp(0.35, 0.98)
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

    #[test]
    fn edit_helpers_preserve_tokenization_lcs_and_filter_rules() {
        assert_eq!(
            tokenize_words("Don't split; names_around punctuation."),
            ["Don't", "split", "names", "around", "punctuation"]
        );
        assert_eq!(
            lcs_matches(
                &["keep".to_owned(), "old".to_owned(), "tail".to_owned()],
                &["keep".to_owned(), "new".to_owned(), "tail".to_owned()]
            ),
            [(0, 0), (2, 2)]
        );
        assert!(is_low_value_replacement(
            &["it".to_owned()],
            &["is".to_owned()]
        ));
        assert!(!is_low_value_replacement(
            &["lettus".to_owned()],
            &["lettuce".to_owned()]
        ));
    }

    #[test]
    fn suggestion_score_preserves_counter_signals_and_floor() {
        let remembered = score_suggestion(
            "alpha beta",
            "gamma delta",
            SuggestionScoreEvidence {
                before_words: 2,
                after_words: 2,
                vocabulary_signal: false,
                accepted_count: 1,
                rejected_count: 2,
                seen_count: 2,
            },
        );
        assert!((remembered - 0.56).abs() < f64::EPSILON);
        assert_eq!(
            score_suggestion(
                "orange",
                "purple",
                SuggestionScoreEvidence {
                    before_words: 1,
                    after_words: 1,
                    vocabulary_signal: false,
                    accepted_count: 0,
                    rejected_count: 100,
                    seen_count: 0,
                }
            ),
            0.35
        );
    }
}
