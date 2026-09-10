//! Canonical transfer, compatibility formats, backup, and restore.
//!
//! The intended ownership, boundaries, migration path, and acceptance gates are
//! specified in the crate PLAN.md. This crate starts behavior-empty so the
//! legacy monolith cannot leak in through premature compatibility APIs.

#![deny(unsafe_op_in_unsafe_fn)]

mod backup;
mod backup_graph;
mod companion_state_backup;
mod companion_effect_backup;
mod conversation_backup;
mod conversation_outbox_backup;
mod conversation_runtime_backup;
mod dynamic_memory_backup;
mod job_backup;
mod legacy_backup;
mod legacy_backup_asr;
mod legacy_backup_authored;
mod legacy_backup_configuration;
mod legacy_backup_group_sessions;
mod legacy_backup_media;
mod legacy_backup_pricing;
mod legacy_backup_sessions;
mod legacy_backup_usage;
mod memory_backup;
mod memory_projection_backup;
mod restore_plan;
mod restore_workspace;
mod usage_backup;

pub use backup::*;
pub use backup_graph::*;
pub use companion_state_backup::*;
pub use companion_effect_backup::*;
pub use conversation_backup::*;
pub use conversation_outbox_backup::*;
pub use conversation_runtime_backup::*;
pub use dynamic_memory_backup::*;
pub use job_backup::*;
pub use legacy_backup::*;
pub use legacy_backup_asr::*;
pub use legacy_backup_authored::*;
pub use legacy_backup_configuration::*;
pub use legacy_backup_group_sessions::*;
pub use legacy_backup_media::*;
pub use legacy_backup_pricing::*;
pub use legacy_backup_sessions::*;
pub use legacy_backup_usage::*;
pub use memory_backup::*;
pub use memory_projection_backup::*;
pub use restore_plan::*;
pub use restore_workspace::*;
pub use usage_backup::*;

use std::fmt;

use lettuce_context::{PromptEntryDraft, PromptPurpose};
use lettuce_media::{AssetKind, AssetOrigin, AssetProvenanceV1};
use lettuce_models::{ModelKind, ModelProfileConfig, ProviderConfig, ProviderProtocol};
use lettuce_settings::{HeaderName, SecretOwnerId, SecretRef, SecretValue};
use lettuce_speech::{AsrCorrectionRule, AsrIgnoredSuggestion, AsrVocabularyTerm, AsrVoiceExample};
use lettuce_types::{
    AsrCorrectionId, AsrIgnoredSuggestionId, AsrVocabularyTermId, AsrVoiceExampleId, AssetId,
    CharacterId, ContentHash, GroupId, LegacyImportRunId, LorebookEntryId, LorebookId,
    ModelProfileId, PersonaId, PromptDocumentId, ProviderAccountId, SceneId, TimestampMillis,
};
use serde::{Deserialize, Serialize};

pub const LEGACY_DATABASE_SCHEMA_VERSION: u32 = 92;
pub const ASR_LEARNING_DOCUMENT_VERSION: u32 = 3;
pub const ASR_LEARNING_RECORD_LIMIT: usize = 40_000;
pub const ASR_LEARNING_TABLE_LIMIT: usize = 10_000;
pub const LEGACY_ASR_LEARNING_DOCUMENT_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LegacyAsrLearningDocument {
    pub version: u32,
    #[serde(default)]
    pub vocabulary: Vec<LegacyAsrVocabularyRecord>,
    #[serde(default)]
    pub corrections: Vec<LegacyAsrCorrectionRecord>,
    #[serde(default)]
    pub voice_examples: Vec<LegacyAsrVoiceExampleRecord>,
    #[serde(default)]
    pub ignored_suggestions: Vec<LegacyAsrIgnoredSuggestionRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LegacyAsrVocabularyRecord {
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

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LegacyAsrCorrectionRecord {
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LegacyAsrVoiceExampleRecord {
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LegacyAsrIgnoredSuggestionRecord {
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

impl LegacyAsrLearningDocument {
    #[must_use]
    pub fn within_bounds(&self) -> bool {
        self.version == LEGACY_ASR_LEARNING_DOCUMENT_VERSION
            && self.vocabulary.len() <= ASR_LEARNING_TABLE_LIMIT
            && self.corrections.len() <= ASR_LEARNING_TABLE_LIMIT
            && self.ignored_suggestions.len() <= ASR_LEARNING_TABLE_LIMIT
            && self.voice_examples.len() <= ASR_LEARNING_TABLE_LIMIT
            && self
                .vocabulary
                .len()
                .checked_add(self.corrections.len())
                .and_then(|count| count.checked_add(self.ignored_suggestions.len()))
                .and_then(|count| count.checked_add(self.voice_examples.len()))
                .is_some_and(|count| count <= ASR_LEARNING_RECORD_LIMIT)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AsrLearningDocument {
    pub version: u32,
    pub vocabulary: Vec<AsrVocabularyTerm>,
    pub corrections: Vec<AsrCorrectionRule>,
    pub ignored_suggestions: Vec<AsrIgnoredSuggestion>,
    pub voice_examples: Vec<AsrVoiceExample>,
    pub audio_assets: Vec<AsrLearningAudioAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AsrLearningAudioAsset {
    pub asset_id: AssetId,
    pub kind: AssetKind,
    pub origin: AssetOrigin,
    pub provenance: AssetProvenanceV1,
    pub content_hash: ContentHash,
    pub byte_size: u64,
    pub mime_type: String,
    pub duration_ms: Option<u64>,
}

impl AsrLearningDocument {
    #[must_use]
    pub fn within_bounds(&self) -> bool {
        self.version == ASR_LEARNING_DOCUMENT_VERSION
            && self.vocabulary.len() <= ASR_LEARNING_TABLE_LIMIT
            && self.corrections.len() <= ASR_LEARNING_TABLE_LIMIT
            && self.ignored_suggestions.len() <= ASR_LEARNING_TABLE_LIMIT
            && self.voice_examples.len() <= ASR_LEARNING_TABLE_LIMIT
            && self.audio_assets.len() <= ASR_LEARNING_TABLE_LIMIT
            && self
                .vocabulary
                .len()
                .checked_add(self.corrections.len())
                .and_then(|count| count.checked_add(self.ignored_suggestions.len()))
                .and_then(|count| count.checked_add(self.voice_examples.len()))
                .is_some_and(|count| count <= ASR_LEARNING_RECORD_LIMIT)
    }

    pub fn validate(&self) -> Result<(), lettuce_speech::AsrLearningError> {
        if !self.within_bounds() {
            return Err(lettuce_speech::AsrLearningError::InvalidData);
        }
        let vocabulary_ids = self
            .vocabulary
            .iter()
            .map(|term| term.id)
            .collect::<std::collections::BTreeSet<_>>();
        let correction_ids = self
            .corrections
            .iter()
            .map(|rule| rule.id)
            .collect::<std::collections::BTreeSet<_>>();
        let ignored_ids = self
            .ignored_suggestions
            .iter()
            .map(|ignored| ignored.id)
            .collect::<std::collections::BTreeSet<_>>();
        let voice_ids = self
            .voice_examples
            .iter()
            .map(|example| example.id)
            .collect::<std::collections::BTreeSet<_>>();
        let audio_asset_ids = self
            .audio_assets
            .iter()
            .map(|asset| asset.asset_id)
            .collect::<std::collections::BTreeSet<_>>();
        if vocabulary_ids.len() != self.vocabulary.len()
            || correction_ids.len() != self.corrections.len()
            || ignored_ids.len() != self.ignored_suggestions.len()
            || voice_ids.len() != self.voice_examples.len()
            || audio_asset_ids.len() != self.audio_assets.len()
        {
            return Err(lettuce_speech::AsrLearningError::InvalidData);
        }
        for term in &self.vocabulary {
            term.validate()?;
        }
        for correction in &self.corrections {
            correction.validate()?;
        }
        for ignored in &self.ignored_suggestions {
            ignored.validate()?;
        }
        for example in &self.voice_examples {
            example.validate()?;
            if example
                .vocabulary_term_id
                .is_some_and(|id| !vocabulary_ids.contains(&id))
                || example
                    .correction_id
                    .is_some_and(|id| !correction_ids.contains(&id))
            {
                return Err(lettuce_speech::AsrLearningError::InvalidData);
            }
        }
        let referenced_audio_ids = self
            .voice_examples
            .iter()
            .map(|example| example.audio_asset_id)
            .collect::<std::collections::BTreeSet<_>>();
        if referenced_audio_ids != audio_asset_ids
            || self.audio_assets.iter().any(|asset| {
                asset.kind.blob_kind() != lettuce_media::MediaKind::Audio
                    || asset.mime_type.trim().is_empty()
                    || asset.mime_type.trim() != asset.mime_type
                    || asset.mime_type.chars().count() > 256
                    || asset.mime_type.chars().any(char::is_control)
                    || !asset.mime_type.is_ascii()
                    || i64::try_from(asset.byte_size).is_err()
                    || asset
                        .duration_ms
                        .is_some_and(|duration| i64::try_from(duration).is_err())
                    || asset.provenance.validate().is_err()
            })
        {
            return Err(lettuce_speech::AsrLearningError::InvalidData);
        }
        Ok(())
    }
}
pub const LEGACY_PERSONA_PLAN_LIMIT: u32 = 10_000;
pub const LEGACY_LOREBOOK_PLAN_LIMIT: u32 = 10_000;
pub const LEGACY_LOREBOOK_ENTRY_PLAN_LIMIT: u32 = 100_000;
pub const LEGACY_LOREBOOK_ENTRIES_PER_BOOK_LIMIT: u32 = 512;
pub const LEGACY_MEDIA_REFERENCE_LIMIT: u32 = 20_000;
pub const LEGACY_MEDIA_OBJECT_BYTES_LIMIT: u64 = 64 * 1024 * 1024;
pub const LEGACY_MEDIA_TOTAL_BYTES_LIMIT: u64 = 512 * 1024 * 1024;
pub const LEGACY_PROVIDER_ACCOUNT_PLAN_LIMIT: u32 = 256;
pub const LEGACY_MODEL_PROFILE_PLAN_LIMIT: u32 = 10_000;
pub const LEGACY_PROMPT_PLAN_LIMIT: u32 = 10_000;
pub const LEGACY_ASR_TABLE_PLAN_LIMIT: u32 = 10_000;
pub const LEGACY_ASR_RECORD_PLAN_LIMIT: u32 = 40_000;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyProviderAccountOrigin {
    Stored,
    BuiltInLlamaCpp,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum LegacyPendingProviderSecret {
    ApiKey,
    Header { name: HeaderName },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyProviderAccountCandidate {
    pub id: ProviderAccountId,
    pub origin: LegacyProviderAccountOrigin,
    pub secret_owner_id: SecretOwnerId,
    pub provider_kind: String,
    pub protocol: ProviderProtocol,
    pub label: String,
    pub endpoint: Option<String>,
    pub enabled: bool,
    pub streaming_enabled: bool,
    pub allow_invalid_tls: bool,
    pub default_model: Option<String>,
    pub config: ProviderConfig,
    pub pending_secrets: Vec<LegacyPendingProviderSecret>,
    pub deferred_config_fields: Vec<String>,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyModelProfileCandidate {
    pub id: ModelProfileId,
    pub provider_account_id: ProviderAccountId,
    pub source_provider_kind: String,
    pub source_provider_label: String,
    pub external_model_id: String,
    pub display_name: String,
    pub kind: ModelKind,
    pub config: ModelProfileConfig,
    pub prompt_template_id: Option<String>,
    pub deprecated_system_prompt: Option<String>,
    pub deferred_advanced_fields: Vec<String>,
    pub created_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyProviderModelPlan {
    pub provider_accounts: Vec<LegacyProviderAccountCandidate>,
    pub model_profiles: Vec<LegacyModelProfileCandidate>,
    pub default_provider_account_id: Option<ProviderAccountId>,
    pub default_model_profile_id: Option<ModelProfileId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyPromptEntryCandidate {
    pub source_id: String,
    pub draft: PromptEntryDraft,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyPromptCandidate {
    pub source_id: String,
    pub name: String,
    pub purpose: PromptPurpose,
    pub entries: Vec<LegacyPromptEntryCandidate>,
    pub condense: bool,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyPromptPlan {
    pub prompts: Vec<LegacyPromptCandidate>,
    pub default_prompt_source_id: Option<String>,
    pub deprecated_system_prompt: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyAsrVocabularyCandidate {
    pub source_id: i64,
    pub term: String,
    pub normalized_term: String,
    pub language: Option<String>,
    pub category: Option<String>,
    pub scope: String,
    pub priority: i64,
    pub use_count: u64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyAsrCorrectionCandidate {
    pub source_id: i64,
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
    pub last_seen_at: Option<String>,
    pub user_approved: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyAsrIgnoredSuggestionCandidate {
    pub source_id: i64,
    pub wrong: String,
    pub normalized_wrong: String,
    pub correct: String,
    pub normalized_correct: String,
    pub language: Option<String>,
    pub scope: String,
    pub ignored_count: u64,
    pub last_ignored_at: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyAsrVoiceExampleCandidate {
    pub source_id: i64,
    pub audio: LegacyMediaReference,
    pub expected_text: String,
    pub normalized_expected_text: String,
    pub whisper_output: Option<String>,
    pub normalized_whisper_output: Option<String>,
    pub language: Option<String>,
    pub scope: String,
    pub vocabulary_source_id: Option<i64>,
    pub correction_source_id: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyAsrPlan {
    pub vocabulary: Vec<LegacyAsrVocabularyCandidate>,
    pub corrections: Vec<LegacyAsrCorrectionCandidate>,
    pub ignored_suggestions: Vec<LegacyAsrIgnoredSuggestionCandidate>,
    pub voice_examples: Vec<LegacyAsrVoiceExampleCandidate>,
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
    CharacterAvatar { character_id: CharacterId },
    CharacterBackground { character_id: CharacterId },
    CharacterDesignReference { character_id: CharacterId, ordinal: u32 },
    CharacterSceneBackground { character_id: CharacterId, scene_id: SceneId },
    GroupBackground { group_id: GroupId },
    GroupSceneBackground { group_id: GroupId, scene_id: SceneId },
    AsrVoiceExample { source_id: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyMediaCandidate {
    pub relative_path: String,
    pub source_locator: String,
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
    pub provider_account_ids: Vec<ProviderAccountId>,
    pub model_profile_ids: Vec<ModelProfileId>,
    pub prompt_ids: Vec<String>,
    pub provider_secrets: Vec<LegacyImportProviderSecretSource>,
    pub persona_ids: Vec<PersonaId>,
    pub lorebook_ids: Vec<LorebookId>,
    pub lorebook_entry_ids: Vec<LorebookEntryId>,
    pub asr_vocabulary_ids: Vec<i64>,
    pub asr_correction_ids: Vec<i64>,
    pub asr_ignored_suggestion_ids: Vec<i64>,
    pub asr_voice_example_ids: Vec<i64>,
    pub media: Vec<LegacyImportMediaSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LegacyImportProviderSecretSource {
    pub provider_account_id: ProviderAccountId,
    pub secret: LegacyPendingProviderSecret,
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

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyImportPlan {
    pub provider_models: LegacyProviderModelPlan,
    pub prompts: LegacyPromptPlan,
    pub personas: LegacyPersonaPlan,
    pub lorebooks: LegacyLorebookPlan,
    pub asr: LegacyAsrPlan,
    pub media: LegacyMediaPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyImportAssignment {
    ProviderAccount {
        legacy_id: ProviderAccountId,
        destination_id: ProviderAccountId,
        secret_owner_id: SecretOwnerId,
    },
    ModelProfile {
        legacy_id: ModelProfileId,
        destination_id: ModelProfileId,
    },
    Prompt {
        legacy_id: String,
        destination_id: PromptDocumentId,
    },
    ProviderSecret {
        source: LegacyImportProviderSecretSource,
        destination_ref: SecretRef,
    },
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
    AsrVocabulary {
        legacy_id: i64,
        destination_id: AsrVocabularyTermId,
    },
    AsrCorrection {
        legacy_id: i64,
        destination_id: AsrCorrectionId,
    },
    AsrIgnoredSuggestion {
        legacy_id: i64,
        destination_id: AsrIgnoredSuggestionId,
    },
    AsrVoiceExample {
        legacy_id: i64,
        destination_id: AsrVoiceExampleId,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyImportSecretCompletionRequest {
    pub run_id: LegacyImportRunId,
    pub source: LegacyImportProviderSecretSource,
    pub destination_ref: SecretRef,
    pub generation: u64,
    pub completed_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyImportSecretCompletion {
    pub run_id: LegacyImportRunId,
    pub source: LegacyImportProviderSecretSource,
    pub destination_ref: SecretRef,
    pub generation: u64,
    pub completed_at: TimestampMillis,
    pub replayed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyProviderSecretSourceError {
    Unavailable,
    Missing,
    Invalid,
}

pub trait LegacyProviderSecretSource: Send + Sync {
    fn sources(
        &self,
    ) -> Result<Vec<LegacyImportProviderSecretSource>, LegacyProviderSecretSourceError>;

    fn load(
        &self,
        source: &LegacyImportProviderSecretSource,
    ) -> Result<SecretValue, LegacyProviderSecretSourceError>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyImportExecutionRequest {
    pub run_id: LegacyImportRunId,
    pub plan_fingerprint: ContentHash,
    pub provider_models: LegacyProviderModelPlan,
    pub prompts: LegacyPromptPlan,
    pub personas: LegacyPersonaPlan,
    pub lorebooks: LegacyLorebookPlan,
    pub media: LegacyMediaPlan,
    pub completed_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyImportReceipt {
    pub run_id: LegacyImportRunId,
    pub persona_count: u64,
    pub lorebook_count: u64,
    pub lorebook_entry_count: u64,
    pub completed_at: TimestampMillis,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyAsrMaterializationRequest {
    pub run_id: LegacyImportRunId,
    pub plan_fingerprint: ContentHash,
    pub asr: LegacyAsrPlan,
    pub media: LegacyMediaPlan,
    pub completed_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyAsrReceipt {
    pub run_id: LegacyImportRunId,
    pub vocabulary_count: u64,
    pub correction_count: u64,
    pub ignored_suggestion_count: u64,
    pub voice_example_count: u64,
    pub completed_at: TimestampMillis,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyProviderModelMaterializationRequest {
    pub run_id: LegacyImportRunId,
    pub plan_fingerprint: ContentHash,
    pub provider_models: LegacyProviderModelPlan,
    pub prompts: LegacyPromptPlan,
    pub completed_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyProviderModelReceipt {
    pub run_id: LegacyImportRunId,
    pub provider_account_count: u64,
    pub model_profile_count: u64,
    pub prompt_count: u64,
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

    fn get_secret_completion(
        &self,
        run_id: LegacyImportRunId,
        source: &LegacyImportProviderSecretSource,
    ) -> Result<Option<LegacyImportSecretCompletion>, LegacyImportRepositoryError>;

    fn complete_secret(
        &self,
        request: LegacyImportSecretCompletionRequest,
    ) -> Result<LegacyImportSecretCompletion, LegacyImportRepositoryError>;

    fn materialize(
        &self,
        request: LegacyImportExecutionRequest,
    ) -> Result<LegacyImportReceipt, LegacyImportRepositoryError>;

    fn materialize_asr(
        &self,
        request: LegacyAsrMaterializationRequest,
    ) -> Result<LegacyAsrReceipt, LegacyImportRepositoryError>;

    fn materialize_provider_models(
        &self,
        request: LegacyProviderModelMaterializationRequest,
    ) -> Result<LegacyProviderModelReceipt, LegacyImportRepositoryError>;
}
