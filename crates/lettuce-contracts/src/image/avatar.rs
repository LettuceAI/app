use serde::{Deserialize, Serialize};

use crate::AvatarPromptKind;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AvatarGenerationModel {
    pub id: String,
    pub name: String,
    pub provider_kind: String,
    pub supports_editing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AvatarGenerationOptions {
    pub enabled: bool,
    pub model: Option<AvatarGenerationModel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AvatarGenerateRequest {
    pub request_id: String,
    pub model_id: Option<String>,
    pub kind: AvatarPromptKind,
    pub source_asset_id: Option<String>,
    pub character_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AvatarGenerationAccepted {
    pub job_id: String,
    pub model: AvatarGenerationModel,
    pub prompt: String,
}
