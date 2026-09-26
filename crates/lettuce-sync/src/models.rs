use lettuce_models::{ModelProfile, ProviderAccount};
use lettuce_types::{ModelProfileId, ProviderAccountId};

use crate::{CanonicalPayload, SyncChangeError, SyncEntity};

pub const PROVIDER_ACCOUNT_SYNC_KIND: &str = "provider_account";
pub const PROVIDER_ACCOUNT_SYNC_SCHEMA: &str = "provider_account.snapshot";
pub const PROVIDER_ACCOUNT_SYNC_VERSION: u32 = 1;
pub const MODEL_PROFILE_SYNC_KIND: &str = "model_profile";
pub const MODEL_PROFILE_SYNC_SCHEMA: &str = "model_profile.snapshot";
pub const MODEL_PROFILE_SYNC_VERSION: u32 = 1;

/// The complete provider account row. Secret references travel as opaque
/// identifiers; secret values stay in each device's secret store.
pub fn canonical_provider_account_payload(
    account: &ProviderAccount,
) -> Result<CanonicalPayload, SyncChangeError> {
    let bytes = serde_json::to_vec(account).map_err(|_| SyncChangeError::PayloadEncoding)?;
    CanonicalPayload::new(
        PROVIDER_ACCOUNT_SYNC_SCHEMA,
        PROVIDER_ACCOUNT_SYNC_VERSION,
        bytes,
    )
}

pub fn provider_account_sync_entity(id: ProviderAccountId) -> Result<SyncEntity, SyncChangeError> {
    SyncEntity::new(PROVIDER_ACCOUNT_SYNC_KIND, id.to_string())
}

pub fn canonical_model_profile_payload(
    profile: &ModelProfile,
) -> Result<CanonicalPayload, SyncChangeError> {
    let bytes = serde_json::to_vec(profile).map_err(|_| SyncChangeError::PayloadEncoding)?;
    CanonicalPayload::new(MODEL_PROFILE_SYNC_SCHEMA, MODEL_PROFILE_SYNC_VERSION, bytes)
}

pub fn model_profile_sync_entity(id: ModelProfileId) -> Result<SyncEntity, SyncChangeError> {
    SyncEntity::new(MODEL_PROFILE_SYNC_KIND, id.to_string())
}

pub const CHARACTER_SYNC_KIND: &str = "character";
pub const CHARACTER_SYNC_SCHEMA: &str = "character.snapshot";
pub const CHARACTER_SYNC_VERSION: u32 = 1;

/// The complete character aggregate: root, scenes, variants and starters.
pub fn canonical_character_payload(
    details: &lettuce_characters::CharacterDetails,
) -> Result<CanonicalPayload, SyncChangeError> {
    let bytes = serde_json::to_vec(details).map_err(|_| SyncChangeError::PayloadEncoding)?;
    CanonicalPayload::new(CHARACTER_SYNC_SCHEMA, CHARACTER_SYNC_VERSION, bytes)
}

pub const LOREBOOK_SYNC_KIND: &str = "lorebook";
pub const LOREBOOK_SYNC_SCHEMA: &str = "lorebook.snapshot";
pub const LOREBOOK_SYNC_VERSION: u32 = 1;
pub const CHARACTER_LOREBOOK_BINDINGS_SYNC_KIND: &str = "character_lorebook_bindings";
pub const PERSONA_LOREBOOK_BINDINGS_SYNC_KIND: &str = "persona_lorebook_bindings";
pub const LOREBOOK_BINDINGS_SYNC_SCHEMA: &str = "lorebook.bindings";
pub const LOREBOOK_BINDINGS_SYNC_VERSION: u32 = 1;

/// The complete lorebook: book and entries with their keywords.
pub fn canonical_lorebook_payload(
    details: &lettuce_context::LorebookDetails,
) -> Result<CanonicalPayload, SyncChangeError> {
    let bytes = serde_json::to_vec(details).map_err(|_| SyncChangeError::PayloadEncoding)?;
    CanonicalPayload::new(LOREBOOK_SYNC_SCHEMA, LOREBOOK_SYNC_VERSION, bytes)
}

/// One owner's ordered lorebook bindings; an owner without bindings has no
/// snapshot (its entity is deleted).
pub fn canonical_lorebook_bindings_payload(
    bindings: &[lettuce_context::LorebookBinding],
) -> Result<CanonicalPayload, SyncChangeError> {
    let bytes = serde_json::to_vec(bindings).map_err(|_| SyncChangeError::PayloadEncoding)?;
    CanonicalPayload::new(
        LOREBOOK_BINDINGS_SYNC_SCHEMA,
        LOREBOOK_BINDINGS_SYNC_VERSION,
        bytes,
    )
}

pub const PROMPT_SYNC_KIND: &str = "prompt";
pub const PROMPT_SYNC_SCHEMA: &str = "prompt.snapshot";
pub const PROMPT_SYNC_VERSION: u32 = 1;

/// The complete prompt document with its ordered entries.
pub fn canonical_prompt_payload(
    document: &lettuce_context::PromptDocument,
) -> Result<CanonicalPayload, SyncChangeError> {
    let bytes = serde_json::to_vec(document).map_err(|_| SyncChangeError::PayloadEncoding)?;
    CanonicalPayload::new(PROMPT_SYNC_SCHEMA, PROMPT_SYNC_VERSION, bytes)
}

pub const GROUP_SYNC_KIND: &str = "group";
pub const GROUP_SYNC_SCHEMA: &str = "group.snapshot";
pub const GROUP_SYNC_VERSION: u32 = 1;
pub const GROUP_LOREBOOK_BINDINGS_SYNC_KIND: &str = "group_lorebook_bindings";

/// The complete reusable group: profile, members and starting scene.
pub fn canonical_group_payload(
    details: &lettuce_characters::GroupDetails,
) -> Result<CanonicalPayload, SyncChangeError> {
    let bytes = serde_json::to_vec(details).map_err(|_| SyncChangeError::PayloadEncoding)?;
    CanonicalPayload::new(GROUP_SYNC_SCHEMA, GROUP_SYNC_VERSION, bytes)
}

pub const APP_SETTINGS_SYNC_KIND: &str = "app_settings";
pub const APP_SETTINGS_SYNC_SCHEMA: &str = "app_settings.snapshot";
pub const APP_SETTINGS_SYNC_VERSION: u32 = 2;

pub const CONVERSATION_SNAPSHOT_SYNC_KIND: &str = "conversation_snapshot";
pub const CONVERSATION_SNAPSHOT_SYNC_SCHEMA: &str = "conversation.snapshot_artifact";
pub const CONVERSATION_SNAPSHOT_SYNC_VERSION: u32 = 1;

pub const CONVERSATION_SYNC_KIND: &str = "conversation";
pub const CONVERSATION_SYNC_SCHEMA: &str = "conversation.root";
pub const CONVERSATION_SYNC_VERSION: u32 = 2;

pub const CONVERSATION_MESSAGE_SYNC_KIND: &str = "conversation_message";
pub const CONVERSATION_MESSAGE_SYNC_SCHEMA: &str = "conversation.message";
pub const CONVERSATION_MESSAGE_SYNC_VERSION: u32 = 2;

pub const CONVERSATION_BRANCH_SYNC_KIND: &str = "conversation_branch";
pub const CONVERSATION_BRANCH_SYNC_SCHEMA: &str = "conversation.branch";
pub const CONVERSATION_BRANCH_SYNC_VERSION: u32 = 1;

pub const MEMORY_ITEM_SYNC_KIND: &str = "memory_item";
pub const MEMORY_ITEM_SYNC_SCHEMA: &str = "memory.item";
pub const MEMORY_ITEM_SYNC_VERSION: u32 = 1;

pub const MEMORY_SUMMARY_SYNC_KIND: &str = "memory_summary";
pub const MEMORY_SUMMARY_SYNC_SCHEMA: &str = "memory.summary";
pub const MEMORY_SUMMARY_SYNC_VERSION: u32 = 1;

pub const MEMORY_CURSOR_SYNC_KIND: &str = "memory_cursor";
pub const MEMORY_CURSOR_SYNC_SCHEMA: &str = "memory.cursor";
pub const MEMORY_CURSOR_SYNC_VERSION: u32 = 1;

pub const COMPANION_SOUL_SYNC_KIND: &str = "companion_soul";
pub const COMPANION_SOUL_SYNC_SCHEMA: &str = "companion.soul";
pub const COMPANION_SOUL_SYNC_VERSION: u32 = 1;

pub const COMPANION_RELATIONSHIP_SYNC_KIND: &str = "companion_relationship";
pub const COMPANION_RELATIONSHIP_SYNC_SCHEMA: &str = "companion.relationship";
pub const COMPANION_RELATIONSHIP_SYNC_VERSION: u32 = 1;

pub const COMPANION_SESSION_SYNC_KIND: &str = "companion_session";
pub const COMPANION_SESSION_SYNC_SCHEMA: &str = "companion.session";
pub const COMPANION_SESSION_SYNC_VERSION: u32 = 1;

pub const COMPANION_NOTE_SYNC_KIND: &str = "companion_note";
pub const COMPANION_NOTE_SYNC_SCHEMA: &str = "companion.scheduled_note";
pub const COMPANION_NOTE_SYNC_VERSION: u32 = 1;

pub const USAGE_COST_SYNC_KIND: &str = "usage_cost";
pub const USAGE_COST_SYNC_SCHEMA: &str = "usage.cost_basis";
pub const USAGE_COST_SYNC_VERSION: u32 = 1;

pub const JOB_USAGE_SYNC_KIND: &str = "job_usage";
pub const JOB_USAGE_SYNC_SCHEMA: &str = "usage.job_inference";
pub const JOB_USAGE_SYNC_VERSION: u32 = 1;

pub const JOB_USAGE_COST_SYNC_KIND: &str = "job_usage_cost";
pub const JOB_USAGE_COST_SYNC_SCHEMA: &str = "usage.job_cost_basis";
pub const JOB_USAGE_COST_SYNC_VERSION: u32 = 1;

pub const LEGACY_USAGE_SYNC_KIND: &str = "legacy_usage";
pub const LEGACY_USAGE_SYNC_SCHEMA: &str = "usage.legacy_record";
pub const LEGACY_USAGE_SYNC_VERSION: u32 = 1;

pub const AUDIO_PROVIDER_SYNC_KIND: &str = "audio_provider";
pub const AUDIO_PROVIDER_SYNC_SCHEMA: &str = "audio.provider";
pub const AUDIO_PROVIDER_SYNC_VERSION: u32 = 1;

pub const USER_VOICE_SYNC_KIND: &str = "user_voice";
pub const USER_VOICE_SYNC_SCHEMA: &str = "audio.user_voice";
pub const USER_VOICE_SYNC_VERSION: u32 = 1;

pub const ASR_VOCABULARY_TERM_SYNC_KIND: &str = "asr_vocabulary_term";
pub const ASR_VOCABULARY_TERM_SYNC_SCHEMA: &str = "asr.vocabulary_term";
pub const ASR_VOCABULARY_TERM_SYNC_VERSION: u32 = 1;

pub const ASR_CORRECTION_SYNC_KIND: &str = "asr_correction";
pub const ASR_CORRECTION_SYNC_SCHEMA: &str = "asr.correction";
pub const ASR_CORRECTION_SYNC_VERSION: u32 = 1;

pub const ASR_IGNORED_SUGGESTION_SYNC_KIND: &str = "asr_ignored_suggestion";
pub const ASR_IGNORED_SUGGESTION_SYNC_SCHEMA: &str = "asr.ignored_suggestion";
pub const ASR_IGNORED_SUGGESTION_SYNC_VERSION: u32 = 1;

pub const ASR_VOICE_EXAMPLE_SYNC_KIND: &str = "asr_voice_example";
pub const ASR_VOICE_EXAMPLE_SYNC_SCHEMA: &str = "asr.voice_example";
pub const ASR_VOICE_EXAMPLE_SYNC_VERSION: u32 = 1;

pub const PLAYGROUND_HISTORY_SYNC_KIND: &str = "playground_history";
pub const PLAYGROUND_HISTORY_SYNC_SCHEMA: &str = "playground.history";
pub const PLAYGROUND_HISTORY_SYNC_VERSION: u32 = 1;

pub const PLAYGROUND_IMAGE_SYNC_KIND: &str = "playground_history_image";
pub const PLAYGROUND_IMAGE_SYNC_SCHEMA: &str = "playground.history_image";
pub const PLAYGROUND_IMAGE_SYNC_VERSION: u32 = 1;

pub const CREATION_WORKFLOW_SYNC_KIND: &str = "creation_workflow";
pub const CREATION_WORKFLOW_SYNC_SCHEMA: &str = "creation.workflow";
pub const CREATION_WORKFLOW_SYNC_VERSION: u32 = 1;
