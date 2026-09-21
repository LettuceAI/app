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
