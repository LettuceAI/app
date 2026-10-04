use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CivitaiImage {
    pub url: String,
    pub nsfw_level: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CivitaiLoraSummary {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub id: u64,
    pub name: String,
    pub nsfw: bool,
    pub nsfw_level: u32,
    pub creator_username: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub download_count: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub thumbs_up_count: u64,
    pub preview_image: Option<CivitaiImage>,
    pub base_models: Vec<String>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub latest_version_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CivitaiFile {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub id: u64,
    pub name: String,
    pub size_kb: f64,
    pub primary: bool,
    pub format: Option<String>,
    pub fp: Option<String>,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CivitaiVersion {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub id: u64,
    pub name: String,
    pub base_model: Option<String>,
    pub architecture: Option<String>,
    pub published_at: Option<String>,
    pub trained_words: Vec<String>,
    pub images: Vec<CivitaiImage>,
    pub files: Vec<CivitaiFile>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CivitaiModelDetail {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub id: u64,
    pub name: String,
    pub description: Option<String>,
    pub nsfw: bool,
    pub nsfw_level: u32,
    pub creator_username: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub download_count: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub thumbs_up_count: u64,
    pub tags: Vec<String>,
    pub versions: Vec<CivitaiVersion>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CivitaiSearchRequest {
    pub query: Option<String>,
    /// `Highest Rated`, `Most Downloaded` or `Newest`.
    pub sort: Option<String>,
    /// `AllTime`, `Year`, `Month`, `Week` or `Day`.
    pub period: Option<String>,
    pub base_models: Vec<String>,
    pub cursor: Option<String>,
    pub limit: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CivitaiSearchPage {
    pub items: Vec<CivitaiLoraSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CivitaiModelRequest {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub model_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum CivitaiAuthErrorKind {
    MissingToken,
    /// CivitAI answered, but not in a way that proves the token works.
    Unverified,
    InvalidOrExpired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CivitaiAuthStatus {
    pub saved: bool,
    pub valid: bool,
    pub error_kind: Option<CivitaiAuthErrorKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CivitaiAuthSaveRequest {
    pub token: String,
}

/// A LoRA file of a CivitAI model version to put into the library; its
/// trained words and base model are read from CivitAI again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CivitaiLoraDownloadRequest {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub model_id: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub version_id: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub file_id: u64,
}
