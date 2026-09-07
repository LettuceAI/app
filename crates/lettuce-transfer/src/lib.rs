//! Canonical transfer, compatibility formats, backup, and restore.
//!
//! The intended ownership, boundaries, migration path, and acceptance gates are
//! specified in the crate PLAN.md. This crate starts behavior-empty so the
//! legacy monolith cannot leak in through premature compatibility APIs.

#![deny(unsafe_op_in_unsafe_fn)]

use std::fmt;

use lettuce_types::{
    AssetId, ContentHash, LegacyImportRunId, LorebookEntryId, LorebookId, PersonaId,
    TimestampMillis,
};

pub const LEGACY_DATABASE_SCHEMA_VERSION: u32 = 92;
pub const LEGACY_PERSONA_PLAN_LIMIT: u32 = 10_000;
pub const LEGACY_LOREBOOK_PLAN_LIMIT: u32 = 10_000;
pub const LEGACY_LOREBOOK_ENTRY_PLAN_LIMIT: u32 = 100_000;
pub const LEGACY_LOREBOOK_ENTRIES_PER_BOOK_LIMIT: u32 = 512;
pub const LEGACY_MEDIA_REFERENCE_LIMIT: u32 = 20_000;
pub const LEGACY_MEDIA_OBJECT_BYTES_LIMIT: u64 = 64 * 1024 * 1024;
pub const LEGACY_MEDIA_TOTAL_BYTES_LIMIT: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyDatabaseInventory {
    pub schema_version: u32,
    pub provider_accounts: u64,
    pub models: u64,
    pub prompts: u64,
    pub personas: u64,
    pub characters: u64,
    pub lorebooks: u64,
    pub chat_templates: u64,
    pub direct_conversations: u64,
    pub group_profiles: u64,
    pub group_conversations: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyMediaReference {
    pub locator: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LegacyCrop {
    pub x: f64,
    pub y: f64,
    pub scale: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyImageRecommendation {
    pub model_name: String,
    pub strength: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyPersonaCandidate {
    pub id: PersonaId,
    pub title: String,
    pub description: String,
    pub nickname: Option<String>,
    pub avatar: Option<LegacyMediaReference>,
    pub avatar_crop: Option<LegacyCrop>,
    pub design_description: Option<String>,
    pub design_references: Vec<LegacyMediaReference>,
    pub image_recommendation: Option<LegacyImageRecommendation>,
    pub active_lorebook_ids: Vec<LorebookId>,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyPersonaPlan {
    pub personas: Vec<LegacyPersonaCandidate>,
    pub default_persona_id: Option<PersonaId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyLorebookDetectionPolicy {
    RecentMessageWindow,
    LatestUserMessage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyKeywordMatchMode {
    Literal,
    Regex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyLorebookEntryCandidate {
    pub id: LorebookEntryId,
    pub title: String,
    pub enabled: bool,
    pub always_active: bool,
    pub keywords: Vec<String>,
    pub case_sensitive: bool,
    pub match_mode: LegacyKeywordMatchMode,
    pub content: String,
    pub priority: i32,
    pub display_order: i32,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyLorebookCandidate {
    pub id: LorebookId,
    pub name: String,
    pub avatar: Option<LegacyMediaReference>,
    pub detection_policy: LegacyLorebookDetectionPolicy,
    pub entries: Vec<LegacyLorebookEntryCandidate>,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyLorebookPlan {
    pub lorebooks: Vec<LegacyLorebookCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum LegacyMediaUse {
    PersonaAvatar { persona_id: PersonaId },
    PersonaDesignReference { persona_id: PersonaId, ordinal: u32 },
    LorebookAvatar { lorebook_id: LorebookId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyMediaCandidate {
    pub relative_path: String,
    pub byte_len: u64,
    pub content_hash: ContentHash,
    pub uses: Vec<LegacyMediaUse>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyMediaPlan {
    pub media: Vec<LegacyMediaCandidate>,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyDatabasePreflightError {
    Unavailable,
    MissingTable {
        table: &'static str,
    },
    MissingSettings,
    InvalidSchema,
    UnsupportedVersion {
        found: i64,
        supported: u32,
    },
    CountOutOfRange {
        table: &'static str,
    },
    LimitExceeded {
        table: &'static str,
        limit: u32,
    },
    MalformedRecord {
        table: &'static str,
        field: &'static str,
    },
    OrphanRecord {
        table: &'static str,
        parent_table: &'static str,
    },
    MediaReferenceLimitExceeded {
        limit: u32,
    },
    MissingMedia {
        locator: String,
    },
    UnsafeMediaReference {
        locator: String,
    },
    ConflictingMediaReference {
        locator: String,
    },
    MediaObjectTooLarge {
        locator: String,
        limit: u64,
    },
    MediaTotalTooLarge {
        limit: u64,
    },
    MediaReadFailed {
        locator: String,
    },
}

impl fmt::Display for LegacyDatabasePreflightError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => formatter.write_str("legacy database is unavailable"),
            Self::MissingTable { table } => write!(formatter, "legacy table is missing: {table}"),
            Self::MissingSettings => formatter.write_str("legacy settings row is missing"),
            Self::InvalidSchema => formatter.write_str("legacy database schema is invalid"),
            Self::UnsupportedVersion { found, supported } => write!(
                formatter,
                "legacy schema version {found} is unsupported; expected {supported}"
            ),
            Self::CountOutOfRange { table } => {
                write!(formatter, "legacy record count is out of range: {table}")
            }
            Self::LimitExceeded { table, limit } => {
                write!(
                    formatter,
                    "legacy table exceeds the {limit}-record limit: {table}"
                )
            }
            Self::MalformedRecord { table, field } => {
                write!(formatter, "legacy record is malformed: {table}.{field}")
            }
            Self::OrphanRecord {
                table,
                parent_table,
            } => write!(
                formatter,
                "legacy {table} record has no matching {parent_table} parent"
            ),
            Self::MediaReferenceLimitExceeded { limit } => {
                write!(
                    formatter,
                    "legacy media exceeds the {limit}-reference limit"
                )
            }
            Self::MissingMedia { locator } => {
                write!(formatter, "legacy media is missing: {locator}")
            }
            Self::UnsafeMediaReference { locator } => {
                write!(formatter, "legacy media reference is unsafe: {locator}")
            }
            Self::ConflictingMediaReference { locator } => {
                write!(formatter, "legacy media reference is ambiguous: {locator}")
            }
            Self::MediaObjectTooLarge { locator, limit } => write!(
                formatter,
                "legacy media exceeds the {limit}-byte object limit: {locator}"
            ),
            Self::MediaTotalTooLarge { limit } => {
                write!(
                    formatter,
                    "legacy media exceeds the {limit}-byte total limit"
                )
            }
            Self::MediaReadFailed { locator } => {
                write!(formatter, "legacy media could not be read: {locator}")
            }
        }
    }
}

impl std::error::Error for LegacyDatabasePreflightError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyImportRunStatus {
    Admitted,
    Importing,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyImportSources {
    pub persona_ids: Vec<PersonaId>,
    pub lorebook_ids: Vec<LorebookId>,
    pub lorebook_entry_ids: Vec<LorebookEntryId>,
    pub media: Vec<LegacyImportMediaSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LegacyImportMediaSource {
    pub relative_path: String,
    pub byte_len: u64,
    pub content_hash: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyImportAdmissionRequest {
    pub run_id: LegacyImportRunId,
    pub source_schema_version: u32,
    pub inventory_fingerprint: ContentHash,
    pub plan_fingerprint: ContentHash,
    pub sources: LegacyImportSources,
    pub admitted_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyImportAssignment {
    Persona {
        legacy_id: PersonaId,
        destination_id: PersonaId,
    },
    Lorebook {
        legacy_id: LorebookId,
        destination_id: LorebookId,
    },
    LorebookEntry {
        legacy_id: LorebookEntryId,
        destination_id: LorebookEntryId,
    },
    Media {
        relative_path: String,
        destination_id: AssetId,
        byte_len: u64,
        content_hash: ContentHash,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyImportAdmission {
    pub run_id: LegacyImportRunId,
    pub source_schema_version: u32,
    pub inventory_fingerprint: ContentHash,
    pub plan_fingerprint: ContentHash,
    pub status: LegacyImportRunStatus,
    pub assignments: Vec<LegacyImportAssignment>,
    pub admitted_at: TimestampMillis,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyImportMediaCompletionRequest {
    pub run_id: LegacyImportRunId,
    pub relative_path: String,
    pub destination_asset_id: AssetId,
    pub blob_id: lettuce_types::MediaBlobId,
    pub byte_len: u64,
    pub content_hash: ContentHash,
    pub completed_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyImportMediaCompletion {
    pub run_id: LegacyImportRunId,
    pub relative_path: String,
    pub destination_asset_id: AssetId,
    pub blob_id: lettuce_types::MediaBlobId,
    pub byte_len: u64,
    pub content_hash: ContentHash,
    pub completed_at: TimestampMillis,
    pub replayed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyImportRepositoryError {
    InvalidInput,
    Conflict,
    Storage,
}

impl fmt::Display for LegacyImportRepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput => formatter.write_str("legacy import admission is invalid"),
            Self::Conflict => formatter.write_str("legacy import admission conflicts"),
            Self::Storage => formatter.write_str("legacy import storage is unavailable"),
        }
    }
}

impl std::error::Error for LegacyImportRepositoryError {}

pub trait LegacyImportRepository: Send + Sync {
    fn admit(
        &self,
        request: LegacyImportAdmissionRequest,
    ) -> Result<LegacyImportAdmission, LegacyImportRepositoryError>;

    fn complete_media(
        &self,
        request: LegacyImportMediaCompletionRequest,
    ) -> Result<LegacyImportMediaCompletion, LegacyImportRepositoryError>;
}
