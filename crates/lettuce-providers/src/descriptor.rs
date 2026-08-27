use lettuce_models::{CapabilityStatus, ParameterSupport, ProviderProtocol};

/// Static, user-facing facts about one provider kind. This is the
/// replacement for the legacy `get_provider_configs` catalog and the
/// frontend `PROVIDER_PARAMETER_SUPPORT` / reasoning tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderDescriptor {
    pub kind: &'static str,
    pub display_name: &'static str,
    pub protocol: ProviderProtocol,
    pub aliases: &'static [&'static str],
    pub default_endpoint: Option<&'static str>,
    pub endpoint_editable: bool,
    pub api_key: ApiKeyRequirement,
    pub auth_header: &'static str,
    pub streaming: bool,
    /// The provider has a catalog endpoint; custom accounts still need a
    /// configured models path.
    pub lists_models: bool,
    /// Whether the legacy settings page verified the key on save. A probe
    /// can still be requested explicitly for any kind.
    pub verifies_key: bool,
    pub reasoning: ReasoningSupport,
    pub prompt_caching: PromptCachingSupport,
    pub parameters: ParameterFlags,
    /// User-supplied extra request-body keys the legacy request builder let
    /// through for this provider (not keys the adapter itself emits).
    pub extra_body_keys: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiKeyRequirement {
    Required,
    Optional,
    NotUsed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasoningSupport {
    None,
    Effort,
    BudgetOnly,
    Dynamic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptCachingSupport {
    None,
    Supported,
    Automatic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParameterFlags {
    pub temperature: bool,
    pub top_p: bool,
    pub max_output_tokens: bool,
    pub context_length: bool,
    pub frequency_penalty: bool,
    pub presence_penalty: bool,
    pub top_k: bool,
    pub reasoning_budget: bool,
}

impl ParameterFlags {
    pub const BASIC: Self = Self {
        temperature: true,
        top_p: true,
        max_output_tokens: true,
        context_length: false,
        frequency_penalty: false,
        presence_penalty: false,
        top_k: false,
        reasoning_budget: false,
    };
    pub const PENALTIES: Self = Self {
        frequency_penalty: true,
        presence_penalty: true,
        ..Self::BASIC
    };
    pub const PENALTIES_BUDGET: Self = Self {
        reasoning_budget: true,
        ..Self::PENALTIES
    };
    pub const PENALTIES_TOP_K_BUDGET: Self = Self {
        top_k: true,
        ..Self::PENALTIES_BUDGET
    };

    /// Capability evidence for `lettuce-models`; remote providers never
    /// take a repetition penalty (that was a llama.cpp-only knob).
    pub fn parameter_support(self) -> ParameterSupport {
        let status = |flag: bool| {
            if flag {
                CapabilityStatus::Supported
            } else {
                CapabilityStatus::Unsupported
            }
        };
        ParameterSupport {
            temperature: status(self.temperature),
            top_p: status(self.top_p),
            top_k: status(self.top_k),
            frequency_penalty: status(self.frequency_penalty),
            presence_penalty: status(self.presence_penalty),
            repetition_penalty: CapabilityStatus::Unsupported,
        }
    }
}

/// A model advertised by a provider's catalog endpoint (legacy `ModelInfo`).
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteModel {
    pub id: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub context_length: Option<u64>,
    pub input_modalities: Option<Vec<String>>,
    pub output_modalities: Option<Vec<String>>,
    pub supported_endpoints: Option<Vec<String>>,
    pub input_price: Option<f64>,
    pub output_price: Option<f64>,
}

/// Result of probing an account's credential against its provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyVerification {
    pub valid: bool,
    pub status: Option<u16>,
}

/// Failure categories for catalog and verification requests. Bodies and
/// secrets are never carried.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProviderRequestError {
    #[error("provider does not support this operation")]
    Unsupported,
    #[error("request was rejected before or by the provider")]
    Rejected,
    #[error("provider rejected the credential")]
    CredentialRejected,
    #[error("provider or secret store is unavailable")]
    Unavailable,
    #[error("provider response was malformed")]
    Malformed,
}

impl From<crate::common::AdapterError> for ProviderRequestError {
    fn from(error: crate::common::AdapterError) -> Self {
        use crate::common::AdapterError;
        match error {
            AdapterError::Rejected => Self::Rejected,
            AdapterError::CredentialRejected => Self::CredentialRejected,
            AdapterError::SecretUnavailable | AdapterError::Transport => Self::Unavailable,
            AdapterError::MalformedResponse | AdapterError::EmptyResponse => Self::Malformed,
            AdapterError::Provider(failure) => match failure.kind {
                lettuce_conversations::ProviderFailureKind::CredentialRejected => {
                    Self::CredentialRejected
                }
                lettuce_conversations::ProviderFailureKind::RequestRejected => Self::Rejected,
                lettuce_conversations::ProviderFailureKind::Unavailable => Self::Unavailable,
            },
        }
    }
}
