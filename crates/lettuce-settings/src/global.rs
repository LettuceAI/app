use lettuce_types::{ModelProfileId, Revision, TimestampMillis};
use serde::{Deserialize, Serialize};

pub const GLOBAL_SETTINGS_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GlobalSettings {
    pub pure_mode: PureMode,
    pub analytics_enabled: bool,
    pub update_checks_enabled: bool,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            pure_mode: PureMode::Standard,
            analytics_enabled: true,
            update_checks_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PureMode {
    Off,
    Standard,
    Strict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredGlobalSettings {
    pub settings: GlobalSettings,
    pub default_model_profile_id: Option<ModelProfileId>,
    pub dynamic_memory_model_profile_id: Option<ModelProfileId>,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GlobalSettingsStoreError {
    #[error("settings update has a stale revision")]
    StaleRevision,
    #[error("the selected model profile does not exist")]
    ModelProfileMissing,
    #[error("stored settings are invalid")]
    InvalidData,
    #[error("settings storage failed")]
    Storage,
}

pub trait GlobalSettingsStore: Send + Sync {
    fn load(&self) -> Result<StoredGlobalSettings, GlobalSettingsStoreError>;

    fn save(
        &self,
        settings: GlobalSettings,
        default_model_profile_id: Option<ModelProfileId>,
        expected_revision: Revision,
    ) -> Result<StoredGlobalSettings, GlobalSettingsStoreError>;

    fn set_dynamic_memory_model_profile(
        &self,
        model_profile_id: Option<ModelProfileId>,
        expected_revision: Revision,
    ) -> Result<StoredGlobalSettings, GlobalSettingsStoreError>;
}

#[cfg(test)]
mod tests {
    use super::GlobalSettings;

    #[test]
    fn settings_document_rejects_unknown_fields() {
        let value = r#"{"pure_mode":"standard","analytics_enabled":true,"update_checks_enabled":true,"api_key":"no"}"#;
        assert!(serde_json::from_str::<GlobalSettings>(value).is_err());
    }
}
