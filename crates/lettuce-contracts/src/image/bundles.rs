use serde::{Deserialize, Serialize};

use crate::{HfModelSummary, HfSort, ImageBundleProfile, ImageComponentRole};

/// A repository file checked against a role of an image architecture and
/// pinned to a revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ImageBundleAsset {
    pub selection_id: String,
    pub profile_id: String,
    pub role: ImageComponentRole,
    pub model_id: String,
    pub revision: String,
    pub relative_path: String,
    pub format: String,
    pub quantization: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub size: u64,
    pub sha256: String,
    pub architecture: Option<String>,
    pub gated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ImageBundleProfiles {
    pub profiles: Vec<ImageBundleProfile>,
}

/// A search for repositories that can fill one role of an architecture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfImageBundleSearchRequest {
    pub profile_id: String,
    pub role: ImageComponentRole,
    pub query: String,
    pub sort: Option<HfSort>,
    pub author: Option<String>,
    /// `safetensors`, else GGUF.
    pub format: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfImageBundleSearchResults {
    pub models: Vec<HfModelSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfImageBundleFilesRequest {
    pub profile_id: String,
    pub model_id: String,
    pub role: ImageComponentRole,
}

/// The files of a repository that can fill the role, smallest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfImageBundleFiles {
    pub assets: Vec<ImageBundleAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfImageBundleInstallRequest {
    pub profile_id: String,
    pub display_name: String,
    pub runtime_release: String,
    pub runtime_asset: String,
    pub assets: Vec<ImageBundleAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ImageBundleAccepted {
    pub job_id: String,
    pub bundle_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct HfImageBundleRetryRequest {
    pub bundle_id: String,
}

/// The job that fetches what is missing; `None` when every file is in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ImageBundleRetried {
    pub job_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ImageBundleRegistered {
    pub model_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum ImageBundleState {
    Downloading,
    Registered,
    SetupFailed,
}
