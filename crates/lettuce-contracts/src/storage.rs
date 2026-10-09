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
