use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ProviderVerifyDraft {
    pub provider_kind: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = std::collections::HashMap<String, specta_typescript::Unknown>))]
    pub config: serde_json::Value,
}

impl std::fmt::Debug for ProviderVerifyDraft {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ProviderVerifyDraft([REDACTED])")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProviderVerifyRequest {
    Saved { account_id: String },
    Draft { draft: ProviderVerifyDraft },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ProviderVerified {
    pub valid: bool,
    pub status: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ProviderAccountView {
    pub id: String,
    pub provider_kind: String,
    pub protocol: crate::ProviderProtocolContract,
    pub label: String,
    pub base_url: Option<String>,
    pub enabled: bool,
    pub streaming_enabled: bool,
    pub allow_invalid_tls: bool,
    pub api_key_set: bool,
    #[cfg_attr(feature = "specta", specta(type = std::collections::HashMap<String, specta_typescript::Unknown>))]
    pub config: serde_json::Value,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub revision: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ProviderOpenRouterEndpointsRequest {
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ProviderOpenRouterEndpoint {
    pub id: String,
    pub name: String,
    pub prompt_price: String,
    pub completion_price: String,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub context_length: Option<u64>,
    pub uptime_last_30m: Option<f64>,
    pub supports_prompt_caching: bool,
    pub cache_read_price: Option<String>,
    pub cache_write_price: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum CertificateInvalidReason {
    InvalidPem,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct TrustedCertificateView {
    pub valid: bool,
    pub reason: Option<CertificateInvalidReason>,
    pub id: String,
    pub name: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub imported_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ProviderAccountInput {
    pub id: Option<String>,
    pub provider_kind: String,
    pub label: String,
    pub base_url: Option<String>,
    pub enabled: bool,
    pub streaming_enabled: bool,
    pub allow_invalid_tls: bool,
    #[cfg_attr(feature = "specta", specta(type = std::collections::HashMap<String, specta_typescript::Unknown>))]
    pub config: serde_json::Value,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ProviderAccountSaveRequest {
    pub account: ProviderAccountInput,
    pub api_key: Option<String>,
    pub clear_api_key: bool,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub expected_revision: Option<u64>,
    pub client_operation_id: String,
}
impl std::fmt::Debug for ProviderAccountSaveRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ProviderAccountSaveRequest([REDACTED])")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ProviderAccountDeleteRequest {
    pub account_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub delete_models: bool,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CertificatesView {
    pub certificates: Vec<TrustedCertificateView>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub revision: u64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CertificatesImportRequest {
    pub source: crate::FileSource,
    pub client_operation_id: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CertificatesRemoveRequest {
    pub certificate_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ProviderModelsRequest {
    pub account_id: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ProviderModelVerifyRequest {
    pub account_id: String,
    pub model: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ProviderModelVerified {
    pub exists: bool,
}
