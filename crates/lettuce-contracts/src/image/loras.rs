use serde::{Deserialize, Serialize};

use crate::FileSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LoraKeywordSource {
    None,
    Metadata,
    Civitai,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LoraArchitectureSource {
    None,
    Metadata,
    Civitai,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LoraCompatibility {
    Compatible,
    Incompatible,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct InstalledLora {
    pub filename: String,
    pub path: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub bytes_on_disk: u64,
    pub keywords: Vec<String>,
    pub keyword_source: LoraKeywordSource,
    pub architecture: Option<String>,
    pub architecture_source: LoraArchitectureSource,
    pub compatibility: LoraCompatibility,
}

/// Every LoRA in the library, with its compatibility with `profile_id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorasListRequest {
    pub profile_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LoraList {
    pub loras: Vec<InstalledLora>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorasImportRequest {
    pub source: FileSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorasDeleteRequest {
    pub path: String,
}

/// The LoRA is removed even when a cache file cannot be; `left_behind` lists
/// those files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LoraDeleted {
    pub left_behind: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorasUpdateKeywordsRequest {
    pub path: String,
    pub keywords: Vec<String>,
    pub profile_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LoraKeywordDiscovery {
    pub keywords: Vec<String>,
    pub source: LoraKeywordSource,
    pub sha256: Option<String>,
    pub architecture: Option<String>,
    pub architecture_source: LoraArchitectureSource,
    pub compatibility: LoraCompatibility,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LoraKeywordsDiscoverRequest {
    pub path: String,
    pub profile_id: Option<String>,
    pub client_operation_id: String,
}
