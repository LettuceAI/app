//! Lorebook records, legacy-compatible matching, activation previews, and
//! explanation data.

use std::cmp::Ordering;

use lettuce_types::{AssetId, LorebookEntryId, LorebookId, Revision, TimestampMillis};
use regex::RegexBuilder;
use serde::{Deserialize, Serialize};

use crate::prompt::{
    LifecycleStatus, MAX_AUTHORED_BYTES, MAX_LABEL_BYTES, validate_label, validate_prose,
};

/// The legacy runtime always inspected this many recent messages.
pub const LEGACY_RECENT_MESSAGE_LIMIT: usize = 10;
pub const MAX_LOREBOOK_ENTRIES: usize = 512;
pub const MAX_KEYWORDS_PER_ENTRY: usize = 128;
pub const MAX_REGEX_KEYWORDS_PER_BOOK: usize = 64;
pub const MAX_MATCH_CONTEXT_BYTES: usize = 1024 * 1024;
const REGEX_SIZE_LIMIT: usize = 256 * 1024;
const REGEX_DFA_SIZE_LIMIT: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum DetectionPolicy {
    #[default]
    RecentMessageWindow,
    LatestUserMessage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum KeywordMatchMode {
    #[default]
    Literal,
    Regex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum LorebookBehaviorVersion {
    #[default]
    LegacyV1,
    DeterministicV2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lorebook {
    pub id: LorebookId,
    pub status: LifecycleStatus,
    pub name: String,
    pub detection_policy: DetectionPolicy,
    pub icon_asset_id: Option<AssetId>,
    pub behavior_version: LorebookBehaviorVersion,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LorebookEntry {
    pub id: LorebookEntryId,
    pub lorebook_id: LorebookId,
    pub title: String,
    pub enabled: bool,
    pub always_active: bool,
    pub keywords: Vec<String>,
    pub case_sensitive: bool,
    pub match_mode: KeywordMatchMode,
    pub content: String,
    pub priority: i32,
    pub ordinal: u32,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LorebookValidationError {
    #[error("{0}")]
    Prompt(#[from] crate::prompt::PromptValidationError),
    #[error("lorebook has too many keywords")]
    TooManyKeywords,
    #[error("lorebook has too many entries")]
    TooManyEntries,
    #[error("lorebook has too many regex keywords")]
    TooManyRegexKeywords,
    #[error("lorebook authored payload exceeds 8 MiB")]
    AuthoredPayloadTooLarge,
    #[error("keyword exceeds the 1 KiB limit")]
    KeywordTooLarge,
    #[error("regex keyword is invalid: {0}")]
    InvalidRegex(String),
    #[error("lorebook entry belongs to a different book")]
    WrongBook,
    #[error("lorebook entries contain duplicate ids")]
    DuplicateEntry,
    #[error("lorebook entries must have contiguous zero-based ordinals")]
    InvalidOrdering,
    #[error("lorebook revision must be at least one")]
    ZeroRevision,
    #[error("lorebook match context exceeds the 1 MiB limit")]
    MatchContextTooLarge,
}

impl Lorebook {
    pub fn validate(&self) -> Result<(), LorebookValidationError> {
        validate_label(&self.name, "lorebook name")?;
        if self.revision.get() == 0 {
            return Err(LorebookValidationError::ZeroRevision);
        }
        Ok(())
    }
}

impl LorebookEntry {
    pub fn validate(&self) -> Result<(), LorebookValidationError> {
        validate_label(&self.title, "lorebook entry title")?;
        validate_prose(&self.content, "lorebook entry content")?;
        if self.revision.get() == 0 {
            return Err(LorebookValidationError::ZeroRevision);
        }
        if self.keywords.len() > MAX_KEYWORDS_PER_ENTRY {
            return Err(LorebookValidationError::TooManyKeywords);
        }
        if self
            .keywords
            .iter()
            .any(|keyword| keyword.len() > MAX_LABEL_BYTES)
        {
            return Err(LorebookValidationError::KeywordTooLarge);
        }
        if self.match_mode == KeywordMatchMode::Regex {
            for keyword in &self.keywords {
                compile_regex(keyword, self.case_sensitive)?;
            }
        }
        Ok(())
    }
}

pub fn validate_entries(
    lorebook_id: LorebookId,
    entries: &[LorebookEntry],
) -> Result<(), LorebookValidationError> {
    if entries.len() > MAX_LOREBOOK_ENTRIES {
        return Err(LorebookValidationError::TooManyEntries);
    }
    let mut ids = std::collections::HashSet::with_capacity(entries.len());
    let mut authored_bytes = 0_usize;
    let mut regex_keywords = 0_usize;
    for (ordinal, entry) in entries.iter().enumerate() {
        entry.validate()?;
        if entry.lorebook_id != lorebook_id {
            return Err(LorebookValidationError::WrongBook);
        }
        if !ids.insert(entry.id) {
            return Err(LorebookValidationError::DuplicateEntry);
        }
        if entry.ordinal as usize != ordinal {
            return Err(LorebookValidationError::InvalidOrdering);
        }
        authored_bytes = authored_bytes
            .checked_add(entry.title.len())
            .and_then(|value| value.checked_add(entry.content.len()))
            .and_then(|value| {
                entry
                    .keywords
                    .iter()
                    .try_fold(value, |total, keyword| total.checked_add(keyword.len()))
            })
            .ok_or(LorebookValidationError::AuthoredPayloadTooLarge)?;
        if authored_bytes > MAX_AUTHORED_BYTES {
            return Err(LorebookValidationError::AuthoredPayloadTooLarge);
        }
        if entry.match_mode == KeywordMatchMode::Regex {
            regex_keywords = regex_keywords
                .checked_add(entry.keywords.len())
                .ok_or(LorebookValidationError::TooManyRegexKeywords)?;
            if regex_keywords > MAX_REGEX_KEYWORDS_PER_BOOK {
                return Err(LorebookValidationError::TooManyRegexKeywords);
            }
        }
    }
    Ok(())
}

fn contains_unsegmented_script(text: &str) -> bool {
    text.chars().any(|ch| {
        matches!(
            ch as u32,
            0x0E00..=0x0E7F // Thai
                | 0x0E80..=0x0EFF // Lao
                | 0x1000..=0x109F // Myanmar
                | 0x1780..=0x17FF // Khmer
                | 0x3040..=0x30FF // Hiragana and Katakana
                | 0x3400..=0x4DBF // CJK extension A
                | 0x4E00..=0x9FFF // CJK unified ideographs
                | 0xAC00..=0xD7AF // Hangul
                | 0xF900..=0xFAFF // CJK compatibility ideographs
        )
    })
}

fn normalize_literal(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || character.is_whitespace() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Matches one keyword using the legacy runtime's punctuation, word-boundary,
/// wildcard, CJK and regex rules.
pub(crate) fn keyword_matches(
    keyword: &str,
    text: &str,
    case_sensitive: bool,
) -> Result<bool, LorebookValidationError> {
    keyword_matches_with_mode(keyword, text, case_sensitive, KeywordMatchMode::Literal)
}

pub(crate) fn keyword_matches_with_mode(
    keyword: &str,
    text: &str,
    case_sensitive: bool,
    mode: KeywordMatchMode,
) -> Result<bool, LorebookValidationError> {
    validate_match_context(text)?;
    let keyword = keyword.trim();
    if keyword.is_empty() {
        return Ok(false);
    }
    let search_keyword = if case_sensitive {
        keyword.to_owned()
    } else {
        keyword.to_lowercase()
    };
    let search_text = if case_sensitive {
        text.to_owned()
    } else {
        text.to_lowercase()
    };

    if mode == KeywordMatchMode::Regex {
        return Ok(compile_regex(&search_keyword, case_sensitive)?.is_match(&search_text));
    }

    if let Some(prefix) = search_keyword.strip_suffix('*') {
        if prefix.is_empty() {
            return Ok(false);
        }
        let normalized_text = normalize_literal(&search_text);
        if contains_unsegmented_script(prefix) || contains_unsegmented_script(&normalized_text) {
            return Ok(normalized_text.contains(prefix));
        }
        return Ok(normalized_text
            .split_whitespace()
            .any(|word| word.starts_with(prefix)));
    }

    let normalized_keyword = normalize_literal(&search_keyword);
    let normalized_text = normalize_literal(&search_text);
    if contains_unsegmented_script(&normalized_keyword)
        || contains_unsegmented_script(&normalized_text)
    {
        return Ok(normalized_text.contains(&normalized_keyword));
    }
    if normalized_keyword.contains(' ') {
        return Ok(normalized_text.contains(&normalized_keyword));
    }
    Ok(normalized_text
        .split_whitespace()
        .any(|word| word == normalized_keyword))
}

fn compile_regex(
    keyword: &str,
    case_sensitive: bool,
) -> Result<regex::Regex, LorebookValidationError> {
    RegexBuilder::new(keyword)
        .case_insensitive(!case_sensitive)
        .size_limit(REGEX_SIZE_LIMIT)
        .dfa_size_limit(REGEX_DFA_SIZE_LIMIT)
        .build()
        .map_err(|error| LorebookValidationError::InvalidRegex(error.to_string()))
}

pub(crate) fn validate_match_context(text: &str) -> Result<(), LorebookValidationError> {
    if text.len() > MAX_MATCH_CONTEXT_BYTES {
        Err(LorebookValidationError::MatchContextTooLarge)
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LorebookEntryMatch {
    pub entry: LorebookEntry,
    pub matched_keywords: Vec<String>,
    pub always_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LorebookActivation {
    pub entries: Vec<LorebookEntry>,
    pub matches: Vec<LorebookEntryMatch>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LorebookEntryExplanation {
    pub entry_id: LorebookEntryId,
    pub active: bool,
    pub reason: &'static str,
    pub matched_keywords: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LorebookExplanation {
    pub lorebook_id: LorebookId,
    pub policy: DetectionPolicy,
    pub entries: Vec<LorebookEntryExplanation>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct LorebookMatcher;

impl LorebookMatcher {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    pub fn activate(
        &self,
        book: &Lorebook,
        entries: &[LorebookEntry],
        recent_messages: &[String],
        latest_user_message: Option<&str>,
    ) -> Result<LorebookActivation, LorebookValidationError> {
        book.validate()?;
        validate_entries(book.id, entries)?;
        let context = context_owned(book.detection_policy, recent_messages, latest_user_message)?;
        let mut matches = Vec::new();
        for entry in entries.iter().filter(|entry| entry.enabled) {
            let matched_keywords = entry
                .keywords
                .iter()
                .filter(|keyword| {
                    keyword_matches_with_mode(
                        keyword,
                        &context,
                        entry.case_sensitive,
                        entry.match_mode,
                    )
                    .unwrap_or(false)
                })
                .cloned()
                .collect::<Vec<_>>();
            if entry.always_active || !matched_keywords.is_empty() && !entry.keywords.is_empty() {
                matches.push(LorebookEntryMatch {
                    entry: entry.clone(),
                    matched_keywords,
                    always_active: entry.always_active,
                });
            }
        }
        // Legacy V1 ignores priority. Deterministic V2 currently retains this
        // ordering until the unresolved product decision makes priority live.
        matches.sort_by(compare_matches);
        let entries = matches
            .iter()
            .map(|matched| matched.entry.clone())
            .collect();
        Ok(LorebookActivation { entries, matches })
    }

    pub fn explain(
        &self,
        book: &Lorebook,
        entries: &[LorebookEntry],
        recent_messages: &[String],
        latest_user_message: Option<&str>,
    ) -> Result<LorebookExplanation, LorebookValidationError> {
        book.validate()?;
        validate_entries(book.id, entries)?;
        let context = context_owned(book.detection_policy, recent_messages, latest_user_message)?;
        let entries = entries
            .iter()
            .map(|entry| {
                let matched_keywords = entry
                    .keywords
                    .iter()
                    .filter(|keyword| {
                        keyword_matches_with_mode(
                            keyword,
                            &context,
                            entry.case_sensitive,
                            entry.match_mode,
                        )
                        .unwrap_or(false)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                let (active, reason) = if !entry.enabled {
                    (false, "disabled")
                } else if entry.always_active {
                    (true, "always_active")
                } else if entry.keywords.is_empty() {
                    (false, "no_keywords")
                } else if matched_keywords.is_empty() {
                    (false, "no_keyword_match")
                } else {
                    (true, "keyword_match")
                };
                LorebookEntryExplanation {
                    entry_id: entry.id,
                    active,
                    reason,
                    matched_keywords,
                }
            })
            .collect();
        Ok(LorebookExplanation {
            lorebook_id: book.id,
            policy: book.detection_policy,
            entries,
        })
    }

    pub fn preview(
        &self,
        book: &Lorebook,
        entries: &[LorebookEntry],
        recent_messages: &[String],
        latest_user_message: Option<&str>,
    ) -> Result<LorebookPreview, LorebookValidationError> {
        let activation = self.activate(book, entries, recent_messages, latest_user_message)?;
        let content = format_lorebook_for_prompt(&activation.entries);
        Ok(LorebookPreview {
            activation,
            content,
        })
    }
}

fn context_owned(
    policy: DetectionPolicy,
    recent_messages: &[String],
    latest_user_message: Option<&str>,
) -> Result<String, LorebookValidationError> {
    match policy {
        DetectionPolicy::LatestUserMessage => {
            let message = latest_user_message.unwrap_or_default();
            validate_match_context(message)?;
            Ok(message.to_owned())
        }
        DetectionPolicy::RecentMessageWindow => {
            let messages = recent_messages
                .iter()
                .rev()
                .take(LEGACY_RECENT_MESSAGE_LIMIT)
                .collect::<Vec<_>>();
            let bytes = messages.iter().map(|message| message.len()).sum::<usize>()
                + messages.len().saturating_sub(1);
            if bytes > MAX_MATCH_CONTEXT_BYTES {
                return Err(LorebookValidationError::MatchContextTooLarge);
            }
            Ok(messages
                .into_iter()
                .rev()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join("\n"))
        }
    }
}

fn compare_matches(left: &LorebookEntryMatch, right: &LorebookEntryMatch) -> Ordering {
    left.entry
        .ordinal
        .cmp(&right.entry.ordinal)
        .then_with(|| left.entry.created_at.cmp(&right.entry.created_at))
}

pub fn format_lorebook_for_prompt(entries: &[LorebookEntry]) -> String {
    entries
        .iter()
        .map(|entry| entry.content.trim())
        .filter(|content| !content.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LorebookPreview {
    pub activation: LorebookActivation,
    pub content: String,
}

pub trait LorebookRepository: Send + Sync {
    fn create(
        &self,
        book: Lorebook,
        now: TimestampMillis,
    ) -> Result<Lorebook, LorebookRepositoryError>;
    fn get(&self, id: LorebookId) -> Result<Option<Lorebook>, LorebookRepositoryError>;
    fn list(&self) -> Result<Vec<Lorebook>, LorebookRepositoryError>;
    fn replace_entry_set(
        &self,
        book: LorebookId,
        expected_revision: Revision,
        entries: Vec<LorebookEntry>,
        now: TimestampMillis,
    ) -> Result<Lorebook, LorebookRepositoryError>;
    fn archive(
        &self,
        id: LorebookId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<(), LorebookRepositoryError>;
    fn restore(
        &self,
        id: LorebookId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<(), LorebookRepositoryError>;
}

/// Entry lifecycle is separate from book lifecycle so entry mutations can
/// carry the book revision CAS and never masquerade as a whole-book upsert.
pub trait LorebookEntryRepository: Send + Sync {
    fn create_entry(
        &self,
        book: LorebookId,
        expected_book_revision: Revision,
        entry: LorebookEntry,
        now: TimestampMillis,
    ) -> Result<LorebookEntry, LorebookRepositoryError>;
    fn update_entry(
        &self,
        book: LorebookId,
        expected_book_revision: Revision,
        entry: LorebookEntry,
        now: TimestampMillis,
    ) -> Result<LorebookEntry, LorebookRepositoryError>;
    fn remove_entry(
        &self,
        book: LorebookId,
        expected_book_revision: Revision,
        entry: LorebookEntryId,
        now: TimestampMillis,
    ) -> Result<(), LorebookRepositoryError>;
    fn reorder_entry(
        &self,
        book: LorebookId,
        expected_book_revision: Revision,
        entry: LorebookEntryId,
        target_ordinal: usize,
        now: TimestampMillis,
    ) -> Result<Vec<LorebookEntry>, LorebookRepositoryError>;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LorebookRepositoryError {
    #[error("lorebook validation failed: {0}")]
    Invalid(#[from] LorebookValidationError),
    #[error("lorebook repository conflict")]
    Conflict,
    #[error("lorebook not found")]
    NotFound,
    #[error("lorebook repository failure: {0}")]
    Failure(String),
}

/// Convenience function for callers that do not need to retain a matcher.
pub fn activate_lorebook_entries(
    book: &Lorebook,
    entries: &[LorebookEntry],
    recent_messages: &[String],
    latest_user_message: Option<&str>,
) -> Result<LorebookActivation, LorebookValidationError> {
    LorebookMatcher::new().activate(book, entries, recent_messages, latest_user_message)
}

pub fn preview_lorebook(
    book: &Lorebook,
    entries: &[LorebookEntry],
    recent_messages: &[String],
    latest_user_message: Option<&str>,
) -> Result<LorebookPreview, LorebookValidationError> {
    LorebookMatcher::new().preview(book, entries, recent_messages, latest_user_message)
}

pub fn explain_lorebook(
    book: &Lorebook,
    entries: &[LorebookEntry],
    recent_messages: &[String],
    latest_user_message: Option<&str>,
) -> Result<LorebookExplanation, LorebookValidationError> {
    LorebookMatcher::new().explain(book, entries, recent_messages, latest_user_message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book(policy: DetectionPolicy) -> Lorebook {
        Lorebook {
            id: LorebookId::new(),
            status: LifecycleStatus::Active,
            name: "World".into(),
            detection_policy: policy,
            icon_asset_id: None,
            behavior_version: LorebookBehaviorVersion::LegacyV1,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::UNIX_EPOCH,
            updated_at: TimestampMillis::UNIX_EPOCH,
        }
    }

    fn entry(book: &Lorebook, ordinal: u32, keyword: &str) -> LorebookEntry {
        LorebookEntry {
            id: LorebookEntryId::new(),
            lorebook_id: book.id,
            title: "Entry".into(),
            enabled: true,
            always_active: false,
            keywords: vec![keyword.into()],
            case_sensitive: false,
            match_mode: KeywordMatchMode::Literal,
            content: "Lore".into(),
            priority: 100,
            ordinal,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(i64::from(ordinal)),
            updated_at: TimestampMillis::UNIX_EPOCH,
        }
    }

    #[test]
    fn legacy_literal_matching_preserves_boundaries_and_scripts() {
        assert!(!keyword_matches("art", "party", false).expect("bounded match"));
        assert!(keyword_matches("art", "made art", false).expect("bounded match"));
        assert!(keyword_matches("dragon*", "dragonstone", false).expect("bounded match"));
        assert!(keyword_matches("東京", "今日は東京へ行く", false).expect("bounded match"));
        assert!(keyword_matches("กรุงเทพ", "ฉันอยู่กรุงเทพมหานคร", false).expect("bounded match"));
        assert!(
            keyword_matches_with_mode("東京|大阪", "大阪", false, KeywordMatchMode::Regex)
                .expect("bounded regex")
        );
    }

    #[test]
    fn recent_window_uses_latest_ten_and_priority_is_metadata() {
        let book = book(DetectionPolicy::RecentMessageWindow);
        let mut entries = vec![entry(&book, 0, "needle"), entry(&book, 1, "other")];
        entries[0].priority = 10_000;
        let mut messages = (0..11).map(|n| format!("message {n}")).collect::<Vec<_>>();
        messages[0] = "needle".into();
        assert!(
            LorebookMatcher::new()
                .activate(&book, &entries, &messages, None)
                .expect("valid lorebook")
                .entries
                .is_empty()
        );
        messages[1] = "needle".into();
        let activated = LorebookMatcher::new()
            .activate(&book, &entries, &messages, None)
            .expect("valid lorebook");
        assert_eq!(activated.entries.len(), 1);
        assert_eq!(activated.entries[0].ordinal, 0);
    }

    #[test]
    fn always_active_and_latest_user_work() {
        let book = book(DetectionPolicy::LatestUserMessage);
        let mut active = entry(&book, 0, "");
        active.always_active = true;
        let matched = entry(&book, 1, "needle");
        let result = LorebookMatcher::new()
            .activate(&book, &[active, matched], &["no".into()], Some("needle"))
            .expect("valid lorebook");
        assert_eq!(result.entries.len(), 2);
    }

    #[test]
    fn invalid_regex_and_order_are_rejected() {
        let book = book(DetectionPolicy::RecentMessageWindow);
        let mut bad = entry(&book, 0, "[");
        bad.match_mode = KeywordMatchMode::Regex;
        assert!(matches!(
            LorebookMatcher::new().activate(&book, &[bad], &[], None),
            Err(LorebookValidationError::InvalidRegex(_))
        ));
        let bad_order = entry(&book, 4, "x");
        assert_eq!(
            LorebookMatcher::new().activate(&book, &[bad_order], &[], None),
            Err(LorebookValidationError::InvalidOrdering)
        );
    }

    #[test]
    fn preview_and_explanation_are_pure() {
        let book = book(DetectionPolicy::LatestUserMessage);
        let entry = entry(&book, 0, "needle");
        let preview = LorebookMatcher::new()
            .preview(&book, &[entry], &[], Some("needle"))
            .expect("valid lorebook preview");
        assert_eq!(preview.content, "Lore");
        assert_eq!(
            preview.activation.matches[0].matched_keywords,
            vec!["needle"]
        );
    }

    #[test]
    fn legacy_recent_window_is_fixed_at_ten_messages() {
        let book = book(DetectionPolicy::RecentMessageWindow);
        let entries = vec![entry(&book, 0, "needle")];
        let mut messages = (0..11)
            .map(|index| format!("message {index}"))
            .collect::<Vec<_>>();
        messages[0] = "needle".into();
        assert!(
            LorebookMatcher::new()
                .activate(&book, &entries, &messages, None)
                .expect("valid lorebook")
                .entries
                .is_empty()
        );
    }

    #[test]
    fn lorebook_limits_regexes_and_context() {
        let book = book(DetectionPolicy::LatestUserMessage);
        let mut entry = entry(&book, 0, "x");
        entry.match_mode = KeywordMatchMode::Regex;
        entry.keywords = (0..=MAX_REGEX_KEYWORDS_PER_BOOK)
            .map(|_| "x".to_owned())
            .collect();
        assert_eq!(
            validate_entries(book.id, &[entry]),
            Err(LorebookValidationError::TooManyRegexKeywords)
        );
        assert!(matches!(
            LorebookMatcher::new().activate(
                &book,
                &[],
                &[],
                Some(&"x".repeat(MAX_MATCH_CONTEXT_BYTES + 1)),
            ),
            Err(LorebookValidationError::MatchContextTooLarge)
        ));
    }
}
