//! Lorebook records, legacy-compatible matching, activation previews, and
//! explanation data.

use std::cmp::Ordering;

use lettuce_types::{
    AssetId, CharacterId, ConversationStarterId, GroupId, LorebookEntryId, LorebookId, Page,
    PageRequest, PersonaId, Revision, TimestampMillis,
};
use regex::RegexBuilder;
use serde::{Deserialize, Serialize};

use crate::prompt::{
    LifecycleFilter, LifecycleStatus, MAX_AUTHORED_BYTES, MAX_LABEL_BYTES, validate_label,
    validate_prose,
};

/// The legacy runtime always inspected this many recent messages.
pub const LEGACY_RECENT_MESSAGE_LIMIT: usize = 10;
pub const MAX_LOREBOOK_ENTRIES: usize = 512;
pub const MAX_KEYWORDS_PER_ENTRY: usize = 128;
pub const MAX_REGEX_KEYWORDS_PER_BOOK: usize = 64;
pub const MAX_LOREBOOK_SOURCES: usize = 128;
pub const MAX_ACTIVE_LOREBOOK_ENTRIES: usize = MAX_LOREBOOK_ENTRIES;
pub const MAX_ACTIVE_LOREBOOK_CONTENT_BYTES: usize = 4 * 1024 * 1024;
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

/// Adapter-owned fields (identity, parent, ordinal, revision, and timestamps)
/// are intentionally absent from authored lorebook entry drafts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LorebookEntryDraft {
    pub title: String,
    pub enabled: bool,
    pub always_active: bool,
    pub keywords: Vec<String>,
    pub case_sensitive: bool,
    pub match_mode: KeywordMatchMode,
    pub content: String,
    pub priority: i32,
}

impl LorebookEntryDraft {
    pub fn validate(&self) -> Result<(), LorebookValidationError> {
        validate_entry_fields(
            &self.title,
            &self.keywords,
            self.match_mode,
            self.case_sensitive,
            &self.content,
        )
    }
}

impl From<LorebookEntry> for LorebookEntryDraft {
    fn from(entry: LorebookEntry) -> Self {
        Self {
            title: entry.title,
            enabled: entry.enabled,
            always_active: entry.always_active,
            keywords: entry.keywords,
            case_sensitive: entry.case_sensitive,
            match_mode: entry.match_mode,
            content: entry.content,
            priority: entry.priority,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LorebookMetadataDraft {
    pub name: String,
    pub detection_policy: DetectionPolicy,
    pub icon_asset_id: Option<AssetId>,
    pub behavior_version: LorebookBehaviorVersion,
}

impl LorebookMetadataDraft {
    pub fn validate(&self) -> Result<(), LorebookValidationError> {
        validate_label(&self.name, "lorebook name")?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LorebookDetails {
    pub book: Lorebook,
    pub entries: Vec<LorebookEntry>,
}

impl LorebookDetails {
    pub fn validate(&self) -> Result<(), LorebookValidationError> {
        self.book.validate()?;
        validate_entries(self.book.id, &self.entries)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum LorebookEntryInsertionTarget {
    Append,
    At(usize),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum LorebookEntryMutation {
    Add {
        draft: LorebookEntryDraft,
        target: LorebookEntryInsertionTarget,
    },
    Update {
        entry_id: LorebookEntryId,
        draft: LorebookEntryDraft,
    },
    Remove {
        entry_id: LorebookEntryId,
    },
    Replace {
        drafts: Vec<LorebookEntryDraft>,
    },
    Reorder {
        entry_id: LorebookEntryId,
        target_index: usize,
    },
}

impl LorebookEntryMutation {
    pub fn validate(&self) -> Result<(), LorebookValidationError> {
        match self {
            Self::Add { draft, .. } | Self::Update { draft, .. } => draft.validate(),
            Self::Replace { drafts } => validate_entry_drafts(drafts),
            Self::Remove { .. } | Self::Reorder { .. } => Ok(()),
        }
    }
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
    #[error("lorebook entry insertion target is outside the list")]
    InvalidTarget,
    #[error("lorebook revision must be at least one")]
    ZeroRevision,
    #[error("lorebook created_at must not be later than updated_at")]
    InvalidTimestampOrder,
    #[error("lorebook match context exceeds the 1 MiB limit")]
    MatchContextTooLarge,
}

impl Lorebook {
    pub fn validate(&self) -> Result<(), LorebookValidationError> {
        validate_label(&self.name, "lorebook name")?;
        if self.revision.get() == 0 {
            return Err(LorebookValidationError::ZeroRevision);
        }
        if self.created_at > self.updated_at {
            return Err(LorebookValidationError::InvalidTimestampOrder);
        }
        Ok(())
    }
}

impl LorebookEntry {
    pub fn validate(&self) -> Result<(), LorebookValidationError> {
        if self.revision.get() == 0 {
            return Err(LorebookValidationError::ZeroRevision);
        }
        if self.created_at > self.updated_at {
            return Err(LorebookValidationError::InvalidTimestampOrder);
        }
        validate_entry_fields(
            &self.title,
            &self.keywords,
            self.match_mode,
            self.case_sensitive,
            &self.content,
        )
    }
}

fn validate_entry_fields(
    title: &str,
    keywords: &[String],
    match_mode: KeywordMatchMode,
    case_sensitive: bool,
    content: &str,
) -> Result<(), LorebookValidationError> {
    validate_label(title, "lorebook entry title")?;
    validate_prose(content, "lorebook entry content")?;
    if keywords.len() > MAX_KEYWORDS_PER_ENTRY {
        return Err(LorebookValidationError::TooManyKeywords);
    }
    if keywords
        .iter()
        .any(|keyword| keyword.len() > MAX_LABEL_BYTES)
    {
        return Err(LorebookValidationError::KeywordTooLarge);
    }
    if match_mode == KeywordMatchMode::Regex {
        for keyword in keywords {
            compile_regex(keyword, case_sensitive)?;
        }
    }
    Ok(())
}

fn validate_entry_drafts(drafts: &[LorebookEntryDraft]) -> Result<(), LorebookValidationError> {
    if drafts.len() > MAX_LOREBOOK_ENTRIES {
        return Err(LorebookValidationError::TooManyEntries);
    }
    let mut authored_bytes = 0_usize;
    let mut regex_keywords = 0_usize;
    for draft in drafts {
        draft.validate()?;
        authored_bytes = authored_bytes
            .checked_add(draft.title.len())
            .and_then(|value| value.checked_add(draft.content.len()))
            .and_then(|value| {
                draft
                    .keywords
                    .iter()
                    .try_fold(value, |total, keyword| total.checked_add(keyword.len()))
            })
            .ok_or(LorebookValidationError::AuthoredPayloadTooLarge)?;
        if authored_bytes > MAX_AUTHORED_BYTES {
            return Err(LorebookValidationError::AuthoredPayloadTooLarge);
        }
        if draft.match_mode == KeywordMatchMode::Regex {
            regex_keywords = regex_keywords
                .checked_add(draft.keywords.len())
                .ok_or(LorebookValidationError::TooManyRegexKeywords)?;
            if regex_keywords > MAX_REGEX_KEYWORDS_PER_BOOK {
                return Err(LorebookValidationError::TooManyRegexKeywords);
            }
        }
    }
    Ok(())
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

/// Provenance is typed here instead of referring to character/group crates;
/// this keeps context independent while still identifying every source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum LorebookSourceProvenance {
    Character {
        id: CharacterId,
    },
    Persona {
        id: PersonaId,
    },
    Group {
        id: GroupId,
    },
    Starter {
        character_id: CharacterId,
        starter_id: ConversationStarterId,
    },
}

/// Sources must already be in the caller's binding/source order. The resolver
/// preserves that order only as the final tie-break after legacy ordinal and
/// creation-time ordering. `details: None` is an unresolved ID, not an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LorebookActivationSource {
    pub provenance: LorebookSourceProvenance,
    pub lorebook_id: LorebookId,
    pub details: Option<LorebookDetails>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLorebookSource {
    pub provenance: LorebookSourceProvenance,
    pub lorebook_id: LorebookId,
    pub book_revision: Revision,
    pub source_order: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LorebookSourceSkipReason {
    Missing,
    Archived,
    Duplicate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedLorebookSource {
    pub provenance: LorebookSourceProvenance,
    pub lorebook_id: LorebookId,
    pub reason: LorebookSourceSkipReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLorebookEntry {
    pub entry: LorebookEntry,
    pub source: ResolvedLorebookSource,
    pub matched_keywords: Vec<String>,
    pub always_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiLorebookActivation {
    pub entries: Vec<ResolvedLorebookEntry>,
    pub sources: Vec<ResolvedLorebookSource>,
    pub skipped: Vec<SkippedLorebookSource>,
}

/// The immutable entry shape carried by a conversation lorebook snapshot.
///
/// Snapshot entries intentionally do not contain a live entry revision or
/// timestamps: the snapshot envelope's root revision is the only revision
/// available for this document.  The fields mirror `LorebookEntryV1` in the
/// conversations crate so context can remain independent of that crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LorebookSnapshotActivationEntry {
    pub entry_id: LorebookEntryId,
    pub title: String,
    pub enabled: bool,
    pub always_active: bool,
    pub keywords: Vec<String>,
    pub case_sensitive: bool,
    pub match_mode: KeywordMatchMode,
    pub content: String,
    pub priority: i32,
    pub ordinal: u32,
}

/// One ordered lorebook document supplied by a conversation snapshot.
///
/// `source_order` is supplied by the binding resolver.  It is deliberately
/// not inferred from the vector position because an application may combine
/// ordered sources from more than one snapshot selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LorebookSnapshotActivationSource {
    pub lorebook_id: LorebookId,
    pub root_revision: Revision,
    pub source_order: usize,
    pub detection_policy: DetectionPolicy,
    pub behavior_version: LorebookBehaviorVersion,
    pub entries: Vec<LorebookSnapshotActivationEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLorebookSnapshotSource {
    pub lorebook_id: LorebookId,
    pub root_revision: Revision,
    pub source_order: usize,
    pub detection_policy: DetectionPolicy,
    pub behavior_version: LorebookBehaviorVersion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLorebookSnapshotEntry {
    pub entry: LorebookSnapshotActivationEntry,
    pub source: ResolvedLorebookSnapshotSource,
    pub matched_keywords: Vec<String>,
    pub always_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiLorebookSnapshotActivation {
    pub entries: Vec<ResolvedLorebookSnapshotEntry>,
    pub sources: Vec<ResolvedLorebookSnapshotSource>,
    /// IDs of books which contributed at least one activated entry, in the
    /// same deterministic order as `entries`.
    pub activated_lorebook_ids: Vec<LorebookId>,
    /// Exact snapshot entry IDs which contributed to this activation.
    pub activated_entry_ids: Vec<LorebookEntryId>,
}

fn validate_snapshot_entry(
    entry: &LorebookSnapshotActivationEntry,
) -> Result<(), LorebookValidationError> {
    validate_entry_fields(
        &entry.title,
        &entry.keywords,
        entry.match_mode,
        entry.case_sensitive,
        &entry.content,
    )
}

fn validate_snapshot_source(
    source: &LorebookSnapshotActivationSource,
) -> Result<(), LorebookValidationError> {
    if source.root_revision.get() == 0 {
        return Err(LorebookValidationError::ZeroRevision);
    }
    if source.entries.len() > MAX_LOREBOOK_ENTRIES {
        return Err(LorebookValidationError::TooManyEntries);
    }
    let mut ids = std::collections::HashSet::with_capacity(source.entries.len());
    let mut authored_bytes = 0_usize;
    let mut regex_keywords = 0_usize;
    for (ordinal, entry) in source.entries.iter().enumerate() {
        validate_snapshot_entry(entry)?;
        if !ids.insert(entry.entry_id) {
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

/// Resolve lorebook activation directly from frozen conversation documents.
///
/// This is intentionally separate from `resolve_lorebook_activation`: a
/// conversation document has no entry timestamps or live lifecycle state, so
/// manufacturing `Lorebook`/`LorebookEntry` values would make replay depend on
/// data which was not actually captured.  Matching itself still goes through
/// the same legacy keyword helper and context-window rules.
pub fn resolve_lorebook_snapshot_activation(
    sources: &[LorebookSnapshotActivationSource],
    recent_messages: &[String],
    latest_user_message: Option<&str>,
) -> Result<MultiLorebookSnapshotActivation, MultiLorebookActivationError> {
    if sources.len() > MAX_LOREBOOK_SOURCES {
        return Err(MultiLorebookActivationError::TooManySources);
    }

    let mut seen_books = std::collections::HashSet::with_capacity(sources.len());
    let mut resolved_sources = Vec::new();
    let mut active_entries = Vec::new();
    let mut active_content_bytes = 0_usize;
    for source in sources {
        if !seen_books.insert(source.lorebook_id) {
            continue;
        }
        validate_snapshot_source(source)?;
        let resolved_source = ResolvedLorebookSnapshotSource {
            lorebook_id: source.lorebook_id,
            root_revision: source.root_revision,
            source_order: source.source_order,
            detection_policy: source.detection_policy,
            behavior_version: source.behavior_version,
        };
        let context = context_owned(
            source.detection_policy,
            recent_messages,
            latest_user_message,
        )?;
        for entry in source.entries.iter().filter(|entry| entry.enabled) {
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
            if !entry.always_active && (matched_keywords.is_empty() || entry.keywords.is_empty()) {
                continue;
            }
            if active_entries.len() >= MAX_ACTIVE_LOREBOOK_ENTRIES {
                return Err(MultiLorebookActivationError::TooManyActiveEntries);
            }
            if active_content_bytes.saturating_add(entry.content.len())
                > MAX_ACTIVE_LOREBOOK_CONTENT_BYTES
            {
                return Err(MultiLorebookActivationError::ActiveContentTooLarge);
            }
            active_content_bytes += entry.content.len();
            active_entries.push((
                resolved_source.clone(),
                ResolvedLorebookSnapshotEntry {
                    entry: entry.clone(),
                    source: resolved_source.clone(),
                    matched_keywords,
                    always_active: entry.always_active,
                },
            ));
        }
        resolved_sources.push(resolved_source);
    }

    // Snapshot entries have no created_at. Source order is the binding order;
    // ordinal is the stable order captured inside each book. IDs only break a
    // malformed-but-accepted source-order tie deterministically.
    active_entries.sort_by(|left, right| {
        left.0
            .source_order
            .cmp(&right.0.source_order)
            .then_with(|| left.1.entry.ordinal.cmp(&right.1.entry.ordinal))
            .then_with(|| left.0.lorebook_id.cmp(&right.0.lorebook_id))
            .then_with(|| left.1.entry.entry_id.cmp(&right.1.entry.entry_id))
    });
    let entries = active_entries
        .into_iter()
        .map(|(_, entry)| entry)
        .collect::<Vec<_>>();
    let mut activated_lorebook_ids = Vec::new();
    let mut activated_entry_ids = Vec::with_capacity(entries.len());
    for entry in &entries {
        activated_entry_ids.push(entry.entry.entry_id);
        if activated_lorebook_ids.last() != Some(&entry.source.lorebook_id) {
            activated_lorebook_ids.push(entry.source.lorebook_id);
        }
    }
    Ok(MultiLorebookSnapshotActivation {
        entries,
        sources: resolved_sources,
        activated_lorebook_ids,
        activated_entry_ids,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MultiLorebookActivationError {
    #[error("too many lorebook sources")]
    TooManySources,
    #[error("too many active lorebook entries")]
    TooManyActiveEntries,
    #[error("active lorebook content exceeds the 4 MiB bound")]
    ActiveContentTooLarge,
    #[error("lorebook validation failed: {0}")]
    Invalid(#[from] LorebookValidationError),
}

/// Activates an ordered set of books without knowing anything about an
/// inference engine or conversation. Missing and archived books are reported
/// as skipped sources. Duplicate book IDs keep the first source deterministically.
/// LegacyV1 ordering is global: entry ordinal, creation time, then source order;
/// priority is metadata and never participates in this function.
pub fn resolve_lorebook_activation(
    sources: &[LorebookActivationSource],
    recent_messages: &[String],
    latest_user_message: Option<&str>,
) -> Result<MultiLorebookActivation, MultiLorebookActivationError> {
    if sources.len() > MAX_LOREBOOK_SOURCES {
        return Err(MultiLorebookActivationError::TooManySources);
    }
    let matcher = LorebookMatcher::new();
    let mut seen_books = std::collections::HashSet::with_capacity(sources.len());
    let mut resolved_sources = Vec::new();
    let mut skipped = Vec::new();
    let mut active_entries = Vec::new();

    for (source_order, source) in sources.iter().enumerate() {
        if !seen_books.insert(source.lorebook_id) {
            skipped.push(SkippedLorebookSource {
                provenance: source.provenance,
                lorebook_id: source.lorebook_id,
                reason: LorebookSourceSkipReason::Duplicate,
            });
            continue;
        }
        let Some(details) = source.details.as_ref() else {
            skipped.push(SkippedLorebookSource {
                provenance: source.provenance,
                lorebook_id: source.lorebook_id,
                reason: LorebookSourceSkipReason::Missing,
            });
            continue;
        };
        if details.book.id != source.lorebook_id {
            return Err(MultiLorebookActivationError::Invalid(
                LorebookValidationError::WrongBook,
            ));
        }
        if details.book.status == LifecycleStatus::Archived {
            skipped.push(SkippedLorebookSource {
                provenance: source.provenance,
                lorebook_id: source.lorebook_id,
                reason: LorebookSourceSkipReason::Archived,
            });
            continue;
        }
        details.validate()?;
        let resolved = ResolvedLorebookSource {
            provenance: source.provenance,
            lorebook_id: source.lorebook_id,
            book_revision: details.book.revision,
            source_order,
        };
        let activation = matcher.activate(
            &details.book,
            &details.entries,
            recent_messages,
            latest_user_message,
        )?;
        for matched in activation.matches {
            if active_entries.len() >= MAX_ACTIVE_LOREBOOK_ENTRIES {
                return Err(MultiLorebookActivationError::TooManyActiveEntries);
            }
            let active_content_bytes = active_entries
                .iter()
                .map(
                    |(_, _, _, item): &(u32, TimestampMillis, usize, ResolvedLorebookEntry)| {
                        item.entry.content.len()
                    },
                )
                .sum::<usize>();
            if active_content_bytes.saturating_add(matched.entry.content.len())
                > MAX_ACTIVE_LOREBOOK_CONTENT_BYTES
            {
                return Err(MultiLorebookActivationError::ActiveContentTooLarge);
            }
            active_entries.push((
                matched.entry.ordinal,
                matched.entry.created_at,
                source_order,
                ResolvedLorebookEntry {
                    entry: matched.entry,
                    source: resolved.clone(),
                    matched_keywords: matched.matched_keywords,
                    always_active: matched.always_active,
                },
            ));
        }
        resolved_sources.push(resolved);
    }

    active_entries.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    Ok(MultiLorebookActivation {
        entries: active_entries
            .into_iter()
            .map(|(_, _, _, entry)| entry)
            .collect(),
        sources: resolved_sources,
        skipped,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LorebookMutationResult {
    pub details: LorebookDetails,
    /// Explicitly repeated so callers can feed it to their next CAS even when
    /// they do not retain the nested book snapshot.
    pub book_revision: Revision,
}

/// A bounded lorebook library query. Adapters filter by status first, then use
/// the opaque keyset cursor and deterministic `updated_at DESC, id ASC` order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LorebookLibraryQuery {
    pub page: PageRequest,
    pub status: LifecycleFilter,
}

impl Default for LorebookLibraryQuery {
    fn default() -> Self {
        Self {
            page: PageRequest::default(),
            status: LifecycleFilter::Active,
        }
    }
}

pub trait LorebookRepository: Send + Sync {
    fn create(
        &self,
        metadata: LorebookMetadataDraft,
        entries: Vec<LorebookEntryDraft>,
        now: TimestampMillis,
    ) -> Result<LorebookDetails, LorebookRepositoryError>;
    fn get(&self, id: LorebookId) -> Result<Option<LorebookDetails>, LorebookRepositoryError>;
    fn page(&self, query: LorebookLibraryQuery) -> Result<Page<Lorebook>, LorebookRepositoryError>;
    fn revise_metadata(
        &self,
        id: LorebookId,
        expected_revision: Revision,
        metadata: LorebookMetadataDraft,
        now: TimestampMillis,
    ) -> Result<LorebookMutationResult, LorebookRepositoryError>;
    fn mutate_entries(
        &self,
        book: LorebookId,
        expected_revision: Revision,
        mutation: LorebookEntryMutation,
        now: TimestampMillis,
    ) -> Result<LorebookMutationResult, LorebookRepositoryError>;
    /// Returns the complete post-archive aggregate and new book CAS revision.
    fn archive(
        &self,
        id: LorebookId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<LorebookMutationResult, LorebookRepositoryError>;
    fn restore(
        &self,
        id: LorebookId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<LorebookMutationResult, LorebookRepositoryError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum LorebookReference {
    Character {
        id: CharacterId,
    },
    Persona {
        id: PersonaId,
    },
    Group {
        id: GroupId,
    },
    Starter {
        character_id: CharacterId,
        starter_id: ConversationStarterId,
    },
}

pub trait LorebookDependencyReader: Send + Sync {
    fn references_to(
        &self,
        lorebook_id: LorebookId,
    ) -> Result<Vec<LorebookReference>, LorebookDependencyError>;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LorebookDependencyError {
    #[error("lorebook dependency failure: {0}")]
    Failure(String),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LorebookRepositoryError {
    #[error("lorebook validation failed: {0}")]
    Invalid(#[from] LorebookValidationError),
    #[error("lorebook repository conflict")]
    Conflict,
    #[error("lorebook not found")]
    NotFound,
    #[error("lorebook entry was not found")]
    EntryNotFound,
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
            updated_at: TimestampMillis::new(i64::from(ordinal)),
        }
    }

    fn snapshot_entry(ordinal: u32, keyword: &str) -> LorebookSnapshotActivationEntry {
        LorebookSnapshotActivationEntry {
            entry_id: LorebookEntryId::new(),
            title: "Entry".into(),
            enabled: true,
            always_active: false,
            keywords: vec![keyword.into()],
            case_sensitive: false,
            match_mode: KeywordMatchMode::Literal,
            content: "Lore".into(),
            priority: 100,
            ordinal,
        }
    }

    fn snapshot_source(
        lorebook_id: LorebookId,
        root_revision: Revision,
        source_order: usize,
        detection_policy: DetectionPolicy,
        entries: Vec<LorebookSnapshotActivationEntry>,
    ) -> LorebookSnapshotActivationSource {
        LorebookSnapshotActivationSource {
            lorebook_id,
            root_revision,
            source_order,
            detection_policy,
            behavior_version: LorebookBehaviorVersion::LegacyV1,
            entries,
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
    fn snapshot_activation_preserves_recent_ten_and_latest_user_detection() {
        let recent_id = LorebookId::new();
        let latest_id = LorebookId::new();
        let recent_entry = snapshot_entry(0, "needle");
        let latest_entry = snapshot_entry(0, "needle");
        let sources = vec![
            snapshot_source(
                recent_id,
                Revision::new(7),
                0,
                DetectionPolicy::RecentMessageWindow,
                vec![recent_entry.clone()],
            ),
            snapshot_source(
                latest_id,
                Revision::new(8),
                1,
                DetectionPolicy::LatestUserMessage,
                vec![latest_entry.clone()],
            ),
        ];
        let mut messages = (0..11).map(|n| format!("message {n}")).collect::<Vec<_>>();
        messages[0] = "needle".into();
        let result = resolve_lorebook_snapshot_activation(&sources, &messages, Some("other"))
            .expect("valid snapshot sources");
        assert!(result.entries.is_empty());

        messages[1] = "needle".into();
        let result = resolve_lorebook_snapshot_activation(&sources, &messages, Some("other"))
            .expect("valid snapshot sources");
        assert_eq!(result.activated_lorebook_ids, vec![recent_id]);
        assert_eq!(result.activated_entry_ids, vec![recent_entry.entry_id]);

        let result = resolve_lorebook_snapshot_activation(&sources, &[], Some("needle"))
            .expect("valid snapshot sources");
        assert_eq!(
            result.activated_lorebook_ids,
            vec![latest_id],
            "latest-user detection does not inspect recent-message context"
        );
        assert_eq!(result.activated_entry_ids, vec![latest_entry.entry_id]);
    }

    #[test]
    fn snapshot_activation_keeps_always_active_and_skips_disabled_entries() {
        let lorebook_id = LorebookId::new();
        let mut always_active = snapshot_entry(0, "");
        always_active.always_active = true;
        let mut disabled = snapshot_entry(1, "needle");
        disabled.enabled = false;
        let ordinary = snapshot_entry(2, "needle");
        let result = resolve_lorebook_snapshot_activation(
            &[snapshot_source(
                lorebook_id,
                Revision::INITIAL,
                0,
                DetectionPolicy::LatestUserMessage,
                vec![always_active.clone(), disabled, ordinary.clone()],
            )],
            &[],
            Some("needle"),
        )
        .expect("valid snapshot source");
        assert_eq!(result.activated_lorebook_ids, vec![lorebook_id]);
        assert_eq!(
            result.activated_entry_ids,
            vec![always_active.entry_id, ordinary.entry_id]
        );
    }

    #[test]
    fn snapshot_activation_deduplicates_books_and_orders_by_source_then_ordinal() {
        let first_id = LorebookId::new();
        let second_id = LorebookId::new();
        let first_entry = snapshot_entry(0, "needle");
        let mut second_entry = snapshot_entry(0, "needle");
        second_entry.content = "Second".into();
        let duplicate_entry = snapshot_entry(0, "needle");
        let sources = vec![
            snapshot_source(
                first_id,
                Revision::new(11),
                9,
                DetectionPolicy::LatestUserMessage,
                vec![first_entry.clone()],
            ),
            snapshot_source(
                second_id,
                Revision::new(12),
                2,
                DetectionPolicy::LatestUserMessage,
                vec![second_entry.clone()],
            ),
            snapshot_source(
                first_id,
                Revision::new(99),
                0,
                DetectionPolicy::LatestUserMessage,
                vec![duplicate_entry],
            ),
        ];
        let result = resolve_lorebook_snapshot_activation(&sources, &[], Some("needle"))
            .expect("valid snapshot sources");
        assert_eq!(result.sources.len(), 2);
        assert_eq!(
            result.entries[0].source.root_revision,
            Revision::new(12),
            "the first source is ordered by its supplied binding order"
        );
        assert_eq!(
            result.activated_lorebook_ids,
            vec![second_id, first_id],
            "duplicate book IDs do not contribute a second activation"
        );
        assert_eq!(
            result.activated_entry_ids,
            vec![second_entry.entry_id, first_entry.entry_id]
        );
    }

    #[test]
    fn snapshot_activation_enforces_active_entry_and_content_bounds() {
        let first_id = LorebookId::new();
        let second_id = LorebookId::new();
        let first_entries = (0..MAX_LOREBOOK_ENTRIES)
            .map(|ordinal| snapshot_entry(ordinal as u32, "needle"))
            .collect::<Vec<_>>();
        let second_entries = (0..MAX_LOREBOOK_ENTRIES)
            .map(|ordinal| snapshot_entry(ordinal as u32, "needle"))
            .collect::<Vec<_>>();
        let result = resolve_lorebook_snapshot_activation(
            &[
                snapshot_source(
                    first_id,
                    Revision::INITIAL,
                    0,
                    DetectionPolicy::LatestUserMessage,
                    first_entries,
                ),
                snapshot_source(
                    second_id,
                    Revision::INITIAL,
                    1,
                    DetectionPolicy::LatestUserMessage,
                    second_entries,
                ),
            ],
            &[],
            Some("needle"),
        );
        assert_eq!(
            result,
            Err(MultiLorebookActivationError::TooManyActiveEntries)
        );

        let oversized = (0..5)
            .map(|ordinal| {
                let mut entry = snapshot_entry(ordinal, "");
                entry.always_active = true;
                entry.content = "x".repeat(crate::MAX_PROSE_BYTES);
                entry
            })
            .collect::<Vec<_>>();
        let result = resolve_lorebook_snapshot_activation(
            &[snapshot_source(
                LorebookId::new(),
                Revision::INITIAL,
                0,
                DetectionPolicy::LatestUserMessage,
                oversized,
            )],
            &[],
            None,
        );
        assert_eq!(
            result,
            Err(MultiLorebookActivationError::ActiveContentTooLarge)
        );
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

    #[test]
    fn lorebook_and_entry_reject_reversed_timestamps() {
        let mut value = book(DetectionPolicy::LatestUserMessage);
        value.created_at = TimestampMillis::new(2);
        value.updated_at = TimestampMillis::new(1);
        assert_eq!(
            value.validate(),
            Err(LorebookValidationError::InvalidTimestampOrder)
        );
        let valid_book = book(DetectionPolicy::LatestUserMessage);
        let mut value = entry(&valid_book, 0, "needle");
        value.created_at = TimestampMillis::new(2);
        value.updated_at = TimestampMillis::new(1);
        assert_eq!(
            value.validate(),
            Err(LorebookValidationError::InvalidTimestampOrder)
        );
    }

    #[test]
    fn entry_drafts_are_closed_and_replace_preserves_limits() {
        let book = book(DetectionPolicy::LatestUserMessage);
        let authored = LorebookEntryDraft::from(entry(&book, 0, "needle"));
        let mut value = serde_json::to_value(&authored).expect("draft value");
        value["id"] = serde_json::json!(LorebookEntryId::new());
        assert!(serde_json::from_value::<LorebookEntryDraft>(value).is_err());
        let provenance = LorebookSourceProvenance::Starter {
            character_id: CharacterId::new(),
            starter_id: ConversationStarterId::new(),
        };
        let mut encoded = serde_json::to_value(provenance).expect("provenance value");
        encoded["extra"] = serde_json::json!(true);
        assert!(serde_json::from_value::<LorebookSourceProvenance>(encoded).is_err());
        assert!(
            LorebookEntryMutation::Replace {
                drafts: vec![authored.clone()]
            }
            .validate()
            .is_ok()
        );
        assert_eq!(
            LorebookEntryMutation::Replace {
                drafts: vec![authored; MAX_LOREBOOK_ENTRIES + 1]
            }
            .validate(),
            Err(LorebookValidationError::TooManyEntries)
        );
    }

    #[test]
    fn multi_book_resolution_skips_archived_missing_and_duplicates() {
        let first = book(DetectionPolicy::LatestUserMessage);
        let second = book(DetectionPolicy::LatestUserMessage);
        let mut archived = book(DetectionPolicy::LatestUserMessage);
        archived.status = LifecycleStatus::Archived;
        let first_entry = entry(&first, 0, "needle");
        let mut second_entry = entry(&second, 0, "needle");
        second_entry.created_at = TimestampMillis::new(-1);
        let sources = vec![
            LorebookActivationSource {
                provenance: LorebookSourceProvenance::Character {
                    id: CharacterId::new(),
                },
                lorebook_id: first.id,
                details: Some(LorebookDetails {
                    book: first.clone(),
                    entries: vec![first_entry],
                }),
            },
            LorebookActivationSource {
                provenance: LorebookSourceProvenance::Group { id: GroupId::new() },
                lorebook_id: second.id,
                details: Some(LorebookDetails {
                    book: second.clone(),
                    entries: vec![second_entry],
                }),
            },
            LorebookActivationSource {
                provenance: LorebookSourceProvenance::Persona {
                    id: PersonaId::new(),
                },
                lorebook_id: archived.id,
                details: Some(LorebookDetails {
                    book: archived,
                    entries: vec![],
                }),
            },
            LorebookActivationSource {
                provenance: LorebookSourceProvenance::Starter {
                    character_id: CharacterId::new(),
                    starter_id: ConversationStarterId::new(),
                },
                lorebook_id: LorebookId::new(),
                details: None,
            },
            LorebookActivationSource {
                provenance: LorebookSourceProvenance::Group { id: GroupId::new() },
                lorebook_id: first.id,
                details: None,
            },
        ];
        let resolved = resolve_lorebook_activation(&sources, &[], Some("needle"))
            .expect("all supplied active books are valid");
        assert_eq!(resolved.sources.len(), 2);
        assert_eq!(resolved.entries.len(), 2);
        assert_eq!(resolved.entries[0].source.source_order, 1);
        assert_eq!(resolved.entries[1].source.source_order, 0);
        assert_eq!(resolved.entries[0].source.book_revision, second.revision);
        assert_eq!(
            resolved
                .skipped
                .iter()
                .map(|item| item.reason)
                .collect::<Vec<_>>(),
            vec![
                LorebookSourceSkipReason::Archived,
                LorebookSourceSkipReason::Missing,
                LorebookSourceSkipReason::Duplicate
            ]
        );
    }

    #[test]
    fn multi_book_resolution_enforces_active_entry_bound() {
        let first_book = book(DetectionPolicy::LatestUserMessage);
        let entries = (0..MAX_LOREBOOK_ENTRIES)
            .map(|ordinal| entry(&first_book, ordinal as u32, "needle"))
            .collect::<Vec<_>>();
        let details = LorebookDetails {
            book: first_book.clone(),
            entries,
        };
        let second = book(DetectionPolicy::LatestUserMessage);
        let second_details = LorebookDetails {
            book: second.clone(),
            entries: (0..MAX_LOREBOOK_ENTRIES)
                .map(|ordinal| entry(&second, ordinal as u32, "needle"))
                .collect(),
        };
        assert_eq!(
            resolve_lorebook_activation(
                &[
                    LorebookActivationSource {
                        provenance: LorebookSourceProvenance::Character {
                            id: CharacterId::new()
                        },
                        lorebook_id: first_book.id,
                        details: Some(details),
                    },
                    LorebookActivationSource {
                        provenance: LorebookSourceProvenance::Group { id: GroupId::new() },
                        lorebook_id: second.id,
                        details: Some(second_details),
                    }
                ],
                &[],
                Some("needle"),
            ),
            Err(MultiLorebookActivationError::TooManyActiveEntries)
        );
    }

    #[test]
    fn multi_book_resolution_enforces_active_content_bound() {
        let value = "x".repeat(crate::MAX_PROSE_BYTES);
        let make_book = || book(DetectionPolicy::LatestUserMessage);
        let mut sources = Vec::new();
        for _ in 0..5 {
            let current = make_book();
            let mut current_entry = entry(&current, 0, "");
            current_entry.always_active = true;
            current_entry.content = value.clone();
            sources.push(LorebookActivationSource {
                provenance: LorebookSourceProvenance::Character {
                    id: CharacterId::new(),
                },
                lorebook_id: current.id,
                details: Some(LorebookDetails {
                    book: current,
                    entries: vec![current_entry],
                }),
            });
        }
        assert_eq!(
            resolve_lorebook_activation(&sources, &[], None),
            Err(MultiLorebookActivationError::ActiveContentTooLarge)
        );
    }

    #[test]
    fn lorebook_library_query_is_bounded_and_closed() {
        let query = LorebookLibraryQuery::default();
        assert_eq!(query.status, LifecycleFilter::Active);
        assert_eq!(query.page.limit.get(), 50);
        let mut encoded = serde_json::to_value(&query).expect("query value");
        encoded["listAll"] = serde_json::json!(true);
        assert!(serde_json::from_value::<LorebookLibraryQuery>(encoded).is_err());
    }
}
