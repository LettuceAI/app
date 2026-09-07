//! Canonical transfer, compatibility formats, backup, and restore.
//!
//! The intended ownership, boundaries, migration path, and acceptance gates are
//! specified in the crate PLAN.md. This crate starts behavior-empty so the
//! legacy monolith cannot leak in through premature compatibility APIs.

#![deny(unsafe_op_in_unsafe_fn)]

use std::fmt;

use lettuce_types::{LorebookId, PersonaId, TimestampMillis};

pub const LEGACY_DATABASE_SCHEMA_VERSION: u32 = 92;
pub const LEGACY_PERSONA_PLAN_LIMIT: u32 = 10_000;

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
        }
    }
}

impl std::error::Error for LegacyDatabasePreflightError {}
