//! Canonical transfer, compatibility formats, backup, and restore.
//!
//! The intended ownership, boundaries, migration path, and acceptance gates are
//! specified in the crate PLAN.md. This crate starts behavior-empty so the
//! legacy monolith cannot leak in through premature compatibility APIs.

#![deny(unsafe_op_in_unsafe_fn)]

mod backup;
mod character_card;
mod character_plan;
mod chat_jsonl;
mod entity_package;
mod lorebook_transfer;
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
mod legacy_backup_companion_shared_memory;
mod legacy_backup_compatibility;
mod legacy_backup_json_values;
mod legacy_backup_configuration;
mod legacy_backup_creation_helper;
mod legacy_backup_group_sessions;
mod legacy_backup_media;
mod legacy_backup_memory_embeddings;
mod legacy_backup_model_settings;
mod legacy_backup_pricing;
mod legacy_backup_scheduled_notes;
mod legacy_backup_sessions;
mod legacy_backup_attachments;
mod legacy_backup_images;
mod legacy_backup_usage;
mod playground_history_backup;
mod legacy_import_backup;
mod memory_backup;
mod memory_projection_backup;
mod restore_admission;
mod restore_plan;
mod restore_workspace;
mod usage_backup;

pub use character_card::*;
pub use character_plan::*;
pub use chat_jsonl::*;
pub use entity_package::*;
pub use lorebook_transfer::*;
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
pub use legacy_backup_companion_shared_memory::*;
pub use legacy_backup_compatibility::*;
pub use legacy_backup_configuration::*;
pub use legacy_backup_creation_helper::*;
pub use legacy_backup_group_sessions::*;
pub use legacy_backup_media::*;
pub use legacy_backup_memory_embeddings::*;
pub use legacy_backup_model_settings::*;
pub use legacy_backup_pricing::*;
pub use legacy_backup_scheduled_notes::*;
pub use legacy_backup_sessions::*;
pub use legacy_backup_attachments::*;
pub use legacy_backup_images::*;
pub use legacy_backup_usage::*;
pub use playground_history_backup::*;
pub use legacy_import_backup::*;
pub use memory_backup::*;
pub use memory_projection_backup::*;
pub use restore_admission::*;
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

/// The legacy table layout the importer reads: schema 92, which releases up to
/// 2.2.5 kept (93-94 only repaired group columns 2.2.0 already created, 95
/// rewrote group session override encodings the importer reads in both forms,
/// 96 reordered columns and renamed tables outside the schema).
pub const LEGACY_DATABASE_SCHEMA_VERSION: u32 = 92;
/// Every legacy schema version the importer accepts (user decision
/// 2026-09-23): 2.2.0 stored 92 or 94, 2.2.1 stored 95, 2.2.2-2.2.5 96.
pub const LEGACY_DATABASE_SCHEMA_VERSIONS: std::ops::RangeInclusive<u32> = 92..=96;
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
pub const LEGACY_LOREBOOK_ENTRIES_PER_BOOK_LIMIT: u32 = 10_000;
pub const LEGACY_MEDIA_REFERENCE_LIMIT: u32 = 20_000;
pub const LEGACY_MEDIA_OBJECT_BYTES_LIMIT: u64 = 64 * 1024 * 1024;
pub const LEGACY_MEDIA_TOTAL_BYTES_LIMIT: u64 = crate::MAX_BACKUP_TOTAL_BYTES;
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
    pub skipped: Vec<LegacyImportSkip>,
}

/// A legacy record the import leaves out, sealed with the run as evidence.
/// Only records the legacy app itself already ignored are skipped.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LegacyImportSkip {
    pub kind: LegacyImportSkipKind,
    pub source_key: String,
    pub reason: LegacyImportSkipReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LegacyImportSkipKind {
    SettingsDefaultProviderAccount,
    SettingsDefaultModelProfile,
    PersonaAvatar,
    PersonaDesignReference,
    LorebookAvatar,
    PersonaLorebookBinding,
    LorebookEntryKeyword,
    LegacyValue,
    ModelReference,
    PromptReference,
    SceneReference,
    ChatTemplateReference,
    LorebookReference,
    CharacterReference,
    PersonaReference,
    MessageVariantReference,
    ModelProfile,
    GroupProfile,
    GroupReference,
    CharacterMedia,
    GroupMedia,
    VoiceReference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LegacyImportSkipReason {
    MissingProviderAccount,
    MissingModelProfile,
    MissingMediaFile,
    MissingLorebook,
    InvalidRegex,
    MalformedLegacyValue,
    UnknownLegacyValue,
    MissingPrompt,
    MissingScene,
    MissingSceneVariant,
    MissingChatTemplate,
    MissingCharacter,
    MissingPersona,
    MissingMessageVariant,
    UndersizedGroup,
    MissingGroup,
    MissingUserVoice,
    IncompatibleReference,
}

pub fn legacy_value_skip(
    field: &str,
    row_id: &str,
    reason: LegacyImportSkipReason,
) -> LegacyImportSkip {
    LegacyImportSkip {
        kind: LegacyImportSkipKind::LegacyValue,
        source_key: format!("{field}:{row_id}"),
        reason,
    }
}

/// Reads a legacy JSON column the old settings reader parsed leniently: JSON
/// `null` is absent, and an unparseable or rejected value is absent and recorded.
pub fn lenient_legacy_json(
    value: Option<&str>,
    field: &str,
    row_id: &str,
    skipped: &mut Vec<LegacyImportSkip>,
    accept: fn(&serde_json::Value) -> bool,
) -> Option<serde_json::Value> {
    match serde_json::from_str::<serde_json::Value>(value?) {
        Ok(serde_json::Value::Null) => None,
        Ok(value) if accept(&value) => Some(value),
        _ => {
            skipped.push(legacy_value_skip(
                field,
                row_id,
                LegacyImportSkipReason::MalformedLegacyValue,
            ));
            None
        }
    }
}

pub fn reconcile_legacy_persona_lorebooks(
    personas: &mut LegacyPersonaPlan,
    lorebooks: &LegacyLorebookPlan,
) {
    let known = lorebooks
        .lorebooks
        .iter()
        .map(|lorebook| lorebook.id)
        .collect::<std::collections::BTreeSet<_>>();
    let mut skipped = Vec::new();
    for persona in &mut personas.personas {
        let persona_id = persona.id;
        let mut seen = std::collections::BTreeSet::new();
        persona.active_lorebook_ids.retain(|lorebook_id| {
            if !seen.insert(*lorebook_id) {
                return false;
            }
            let present = known.contains(lorebook_id);
            if !present {
                skipped.push(LegacyImportSkip {
                    kind: LegacyImportSkipKind::PersonaLorebookBinding,
                    source_key: format!("{persona_id}:{lorebook_id}"),
                    reason: LegacyImportSkipReason::MissingLorebook,
                });
            }
            present
        });
    }
    personas.skipped.extend(skipped);
    personas.skipped.sort();
    personas.skipped.dedup();
}

pub fn reconcile_legacy_lorebook_keywords(lorebooks: &mut LegacyLorebookPlan) {
    let mut skipped = Vec::new();
    for lorebook in &mut lorebooks.lorebooks {
        for entry in &mut lorebook.entries {
            if entry.match_mode != LegacyKeywordMatchMode::Regex {
                continue;
            }
            let entry_id = entry.id;
            let case_sensitive = entry.case_sensitive;
            let mut index = 0_usize;
            entry.keywords.retain(|keyword| {
                let keep = lettuce_context::validate_regex_keyword(keyword, case_sensitive).is_ok();
                if !keep {
                    skipped.push(LegacyImportSkip {
                        kind: LegacyImportSkipKind::LorebookEntryKeyword,
                        source_key: format!("{entry_id}:{index}"),
                        reason: LegacyImportSkipReason::InvalidRegex,
                    });
                }
                index += 1;
                keep
            });
        }
    }
    lorebooks.skipped.extend(skipped);
    lorebooks.skipped.sort();
    lorebooks.skipped.dedup();
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
    pub skipped: Vec<LegacyImportSkip>,
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
    pub skipped: Vec<LegacyImportSkip>,
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
    pub skipped: Vec<LegacyImportSkip>,
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
    /// A chat message or group variant attachment; `audio` is what its bytes
    /// hold, `label` its legacy filename (the prompt of a generated image).
    MessageAttachment {
        attachment_id: String,
        audio: bool,
        label: Option<String>,
    },
    /// Image `ordinal` of a legacy playground history entry.
    PlaygroundImage { generation_id: String, ordinal: u32 },
    /// The background a direct (`group` false) or group session set for
    /// itself.
    ConversationBackground { session_id: String, group: bool },
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
    pub skipped: Vec<LegacyImportSkip>,
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
        minimum: u32,
        maximum: u32,
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
            Self::UnsupportedVersion {
                found,
                minimum,
                maximum,
            } => write!(
                formatter,
                "legacy schema version {found} is unsupported; expected {minimum} to {maximum}"
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
    /// Reserved for a run that imported every legacy domain.
    Completed,
    /// Every planned stage finished while some legacy domains are not imported
    /// by any stage yet, so the run must not be treated as a full migration.
    Partial,
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
    pub source_fingerprint: Option<ContentHash>,
    pub sources: LegacyImportSources,
    pub skips: Vec<LegacyImportSkip>,
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
    /// The sealed legacy compatibility plan every later import stage binds to.
    pub source_fingerprint: Option<ContentHash>,
    /// Skips for domains later import stages write, sealed at admission.
    pub later_skips: Vec<LegacyImportSkip>,
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
    pub source_fingerprint: Option<ContentHash>,
    pub status: LegacyImportRunStatus,
    pub assignments: Vec<LegacyImportAssignment>,
    pub skips: Vec<LegacyImportSkip>,
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

/// A legacy import stage that writes one domain after the provider, prompt,
/// persona, lorebook and media stages of the same admitted run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LegacyImportStage {
    Characters,
    Groups,
    Audio,
    Settings,
    DirectConversations,
    GroupConversations,
    UsageRecords,
    CreationHelper,
    Images,
}

impl LegacyImportStage {
    /// Every stage a legacy run needs before it counts as a completed import.
    pub const ALL: [Self; 9] = [
        Self::Characters,
        Self::Groups,
        Self::Audio,
        Self::Settings,
        Self::DirectConversations,
        Self::GroupConversations,
        Self::UsageRecords,
        Self::CreationHelper,
        Self::Images,
    ];
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyCharacterMaterializationRequest {
    pub run_id: LegacyImportRunId,
    pub plan_fingerprint: ContentHash,
    pub source_fingerprint: ContentHash,
    pub media: LegacyMediaPlan,
    pub characters: Vec<LegacyBackupCharacterCandidate>,
    pub character_lorebooks: Vec<BackupLorebookBindings<CharacterId>>,
    pub completed_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyGroupMaterializationRequest {
    pub run_id: LegacyImportRunId,
    pub plan_fingerprint: ContentHash,
    pub source_fingerprint: ContentHash,
    pub media: LegacyMediaPlan,
    pub groups: Vec<LegacyBackupGroupCandidate>,
    pub group_lorebooks: Vec<BackupLorebookBindings<GroupId>>,
    pub completed_at: TimestampMillis,
}

#[derive(Debug, Clone)]
pub struct LegacyAudioMaterializationRequest {
    pub run_id: LegacyImportRunId,
    pub plan_fingerprint: ContentHash,
    pub source_fingerprint: ContentHash,
    pub audio_providers: Vec<lettuce_speech::AudioProvider>,
    pub user_voices: Vec<lettuce_speech::UserVoice>,
    pub completed_at: TimestampMillis,
}

/// One finished legacy conversation in the backup v2 history shapes, with the
/// launch snapshot drafts it references.
#[derive(Debug)]
pub struct LegacyConversationRecord {
    pub history: BackupConversation,
    pub turns: Vec<lettuce_conversations::GenerationTurn>,
    pub usage: Vec<lettuce_usage::UsageEvent>,
    pub snapshots: Vec<lettuce_conversations::SnapshotArtifactDraft>,
    /// The conversation's own memory.
    pub memory: Option<BackupMemorySpace>,
    /// The companion memory pool, on each of the character's conversations.
    pub pool: Option<BackupMemorySpace>,
    pub memory_projections: Vec<BackupMemoryProjection>,
    pub companion: Option<LegacyCompanionConversation>,
}

/// The companion runtime state a legacy companion session seeds, with its
/// exact legacy continuity episode when the legacy data kept one.
#[derive(Debug, Clone)]
pub struct LegacyCompanionConversation {
    pub owner: lettuce_companions::CompanionStateOwner,
    pub initial: lettuce_companions::CompanionRuntimeState,
    pub episode: LegacyCompanionEpisodeRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyCompanionEpisodeRecord {
    pub episode_index: u32,
    pub previous_conversation_id: Option<lettuce_types::ConversationId>,
    pub started_at: TimestampMillis,
    pub ended_at: Option<TimestampMillis>,
    pub updated_at: TimestampMillis,
}

#[derive(Debug)]
pub struct LegacyDirectConversationMaterializationRequest {
    pub run_id: LegacyImportRunId,
    pub plan_fingerprint: ContentHash,
    pub source_fingerprint: ContentHash,
    pub conversations: Vec<LegacyConversationRecord>,
    pub companion_souls: Vec<(CharacterId, Vec<lettuce_companions::SoulFact>)>,
    pub scheduled_notes: Vec<lettuce_companions::CompanionScheduledNote>,
    pub completed_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyCreationMaterializationRequest {
    pub run_id: LegacyImportRunId,
    pub plan_fingerprint: ContentHash,
    pub source_fingerprint: ContentHash,
    pub sessions: Vec<LegacyBackupCreationHelperSession>,
    pub completed_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyUsageMaterializationRequest {
    pub run_id: LegacyImportRunId,
    pub plan_fingerprint: ContentHash,
    pub source_fingerprint: ContentHash,
    pub records: Vec<LegacyBackupUsageRecord>,
    pub completed_at: TimestampMillis,
}

/// The legacy image-generation rows of one admitted run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyImageMaterializationRequest {
    pub run_id: LegacyImportRunId,
    pub plan_fingerprint: ContentHash,
    pub source_fingerprint: ContentHash,
    pub loras: Vec<LegacyImageLoraRecord>,
    pub playground: Vec<LegacyPlaygroundImport>,
    pub completed_at: TimestampMillis,
}

/// A legacy playground entry with the imported asset of each image (none
/// when the media plan recorded the file as missing or unusable).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyPlaygroundImport {
    /// The playground history id the entry gets, derived from the source.
    pub id: String,
    pub generation: LegacyPlaygroundGeneration,
    pub assets: Vec<Option<AssetId>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacySettingsMaterializationRequest {
    pub run_id: LegacyImportRunId,
    pub plan_fingerprint: ContentHash,
    pub source_fingerprint: ContentHash,
    pub settings: LegacyBackupSettingsCandidate,
    pub completed_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyImportStageReceipt {
    pub run_id: LegacyImportRunId,
    pub stage: LegacyImportStage,
    pub record_count: u64,
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

    fn materialize_characters(
        &self,
        request: LegacyCharacterMaterializationRequest,
    ) -> Result<LegacyImportStageReceipt, LegacyImportRepositoryError>;

    fn materialize_groups(
        &self,
        request: LegacyGroupMaterializationRequest,
    ) -> Result<LegacyImportStageReceipt, LegacyImportRepositoryError>;

    fn materialize_audio(
        &self,
        request: LegacyAudioMaterializationRequest,
    ) -> Result<LegacyImportStageReceipt, LegacyImportRepositoryError>;

    fn materialize_settings(
        &self,
        request: LegacySettingsMaterializationRequest,
    ) -> Result<LegacyImportStageReceipt, LegacyImportRepositoryError>;

    /// The committed receipt of one later stage, so a replay can return it
    /// before rebuilding stage input from sources that may have changed since.
    fn stage_receipt(
        &self,
        run_id: LegacyImportRunId,
        stage: LegacyImportStage,
        fingerprints: (&ContentHash, &ContentHash),
    ) -> Result<Option<LegacyImportStageReceipt>, LegacyImportRepositoryError>;

    fn materialize_direct_conversations(
        &self,
        request: LegacyDirectConversationMaterializationRequest,
    ) -> Result<LegacyImportStageReceipt, LegacyImportRepositoryError>;

    fn materialize_group_conversations(
        &self,
        request: LegacyDirectConversationMaterializationRequest,
    ) -> Result<LegacyImportStageReceipt, LegacyImportRepositoryError>;

    fn materialize_usage_records(
        &self,
        request: LegacyUsageMaterializationRequest,
    ) -> Result<LegacyImportStageReceipt, LegacyImportRepositoryError>;

    fn materialize_images(
        &self,
        request: LegacyImageMaterializationRequest,
    ) -> Result<LegacyImportStageReceipt, LegacyImportRepositoryError>;

    fn materialize_creation_helper(
        &self,
        request: LegacyCreationMaterializationRequest,
    ) -> Result<LegacyImportStageReceipt, LegacyImportRepositoryError>;

    /// Marks a partial run completed once every later stage has its result.
    fn complete_legacy_import_run(
        &self,
        run_id: LegacyImportRunId,
        completed_at: TimestampMillis,
    ) -> Result<LegacyImportRunStatus, LegacyImportRepositoryError>;
}
