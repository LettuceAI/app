use serde::{Deserialize, Serialize};

/// This install's UI state (onboarding progress, dismissed hints, the last
/// route): JSON the frontend owns, stored on this device only.
pub type UiState = serde_json::Map<String, serde_json::Value>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum BuildVariant {
    Normal,
    Cuda,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum AppPlatform {
    Windows,
    Macos,
    Linux,
    Android,
    Ios,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AppStatus {
    pub version: String,
    pub build_variant: BuildVariant,
    pub platform: AppPlatform,
    #[cfg_attr(
        feature = "specta",
        specta(type = std::collections::HashMap<String, specta_typescript::Unknown>)
    )]
    pub ui_state: UiState,
    /// A database from the previous app version was found on this device.
    pub legacy_database_detected: bool,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub unresolved_sync_conflicts: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub purge_notices: u64,
}

/// Keys to set; a `null` value removes the key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AppUiStateUpdateRequest {
    #[cfg_attr(
        feature = "specta",
        specta(type = std::collections::HashMap<String, specta_typescript::Unknown>)
    )]
    pub patch: UiState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AppUiStateView {
    #[cfg_attr(
        feature = "specta",
        specta(type = std::collections::HashMap<String, specta_typescript::Unknown>)
    )]
    pub state: UiState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum PurgeNoticeEntityDto {
    Conversation,
    Character,
    Group,
    DatabaseFile,
    MediaAsset,
    SyncEntity,
    UsageRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum PurgeNoticeReasonDto {
    KeptUnsentLocalChanges,
    RejournalIncomplete,
    RejournalDropped,
    DroppedAfterFailures,
    GroupBelowTwoMembers,
    MediaCollectionSkipped,
    NotSynced,
    ConflictCarried,
    UsageRecordUnreadable,
}

/// A delete that needs the user's attention; `entity_id` names the entity
/// (a database file name for `database_file`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PurgeNoticeView {
    pub id: String,
    pub entity: PurgeNoticeEntityDto,
    pub entity_id: String,
    pub reason: PurgeNoticeReasonDto,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub recorded_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PurgeNoticeList {
    pub items: Vec<PurgeNoticeView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PurgeNoticeDismissRequest {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AppUsageDayView {
    pub day: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub active_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AppUsageDaysView {
    pub days: Vec<AppUsageDayView>,
}
