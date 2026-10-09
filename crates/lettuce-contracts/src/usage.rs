use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct UsageClearBeforeRequest {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub before: i64,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct UsageCleared {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub removed: u64,
}
