use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum DatabaseFileKind {
    Initial,
    Restore,
    LegacyRestore,
    Reset,
    Existing,
}

/// Why a listed database file needs attention: an unreadable file blocks
/// media collection until it is deleted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum DatabaseFileError {
    Unreadable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct DatabaseFileView {
    pub file: String,
    pub kind: DatabaseFileKind,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub created_at: Option<i64>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub modified_at: i64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub size: u64,
    pub active: bool,
    pub deletable: bool,
    pub error: Option<DatabaseFileError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct DatabaseFileDeleteRequest {
    pub file: String,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct StorageOptimizeRequest {
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AppDataResetRequest {
    pub client_operation_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum AppDataResetStage {
    Preflight,
    Workers,
    Database,
    WebviewStorage,
    Restart,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct StorageSize {
    pub kind: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct StorageSummary {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub active_database_bytes: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub kept_database_bytes: u64,
    pub media: Vec<StorageSize>,
    pub models: Vec<StorageSize>,
    /// Model files the catalog names that are not on disk.
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub missing_model_files: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub logs_bytes: u64,
}
