use serde::{Deserialize, Serialize};

use lettuce_types::{ModelProfileId, ProviderAccountId, Revision};

use crate::{
    CapabilityStatus, ChatParameterOverrides, ChatParameterProfile, ModelCapabilities, ModelKind,
    ModelProfile, ParameterOverride, ParameterSupport, ParameterValidationError, PromptCaching,
    ProviderAccount, ProviderConfig, ProviderProtocol, ReasoningEffort, ReasoningMode,
    SecretHeader, validate_provider_connection,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedModelIdentity {
    pub model_profile_id: ModelProfileId,
    pub model_revision: Revision,
    pub provider_account_id: ProviderAccountId,
    pub provider_account_revision: Revision,
    pub external_model_id: String,
    pub display_name: String,
    pub provider_protocol: ProviderProtocol,
    pub model_kind: ModelKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ChatParameterResolutionInput {
    pub global: ChatParameterProfile,
    pub session: ChatParameterOverrides,
    pub operation: ChatParameterOverrides,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequiredCapability {
    Streaming,
    Tools,
    StructuredOutput,
    Reasoning,
    PromptCache,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModalityDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatRequirements {
    pub input_modalities: Vec<crate::Modality>,
    pub output_modalities: Vec<crate::Modality>,
    pub require_streaming: bool,
    pub require_tools: bool,
    pub require_structured_output: bool,
    pub require_reasoning: bool,
    pub require_prompt_cache: bool,
}

impl Default for ChatRequirements {
    fn default() -> Self {
        Self {
            input_modalities: vec![crate::Modality::Text],
            output_modalities: vec![crate::Modality::Text],
            require_streaming: false,
            require_tools: false,
            require_structured_output: false,
            require_reasoning: false,
            require_prompt_cache: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedChatParameters {
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub top_k: Option<u32>,
    pub visible_max_output_tokens: Option<u32>,
    pub context_length: Option<u32>,
    pub frequency_penalty: Option<f64>,
    pub presence_penalty: Option<f64>,
    pub repetition_penalty: Option<f64>,
    pub reasoning_mode: Option<ReasoningMode>,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub reasoning_budget_tokens: Option<u32>,
    pub prompt_caching: Option<PromptCaching>,
    pub total_completion_allowance: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatProfileWarning {
    ContextLimitUnverified,
    VisibleOutputLimitUnverified,
    TotalOutputLimitUnverified,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedChatProfile {
    pub model_profile_id: ModelProfileId,
    pub model_revision: Revision,
    pub provider_account_id: ProviderAccountId,
    pub provider_account_revision: Revision,
    pub secret_owner_id: lettuce_settings::SecretOwnerId,
    pub external_model_id: String,
    pub provider_kind: String,
    pub provider_protocol: ProviderProtocol,
    pub endpoint: Option<String>,
    pub provider_config: ProviderConfig,
    pub capabilities: ModelCapabilities,
    pub parameters: ResolvedChatParameters,
    pub api_key_ref: Option<lettuce_settings::SecretRef>,
    pub secret_headers: Vec<SecretHeader>,
    pub warnings: Vec<ChatProfileWarning>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityField {
    ModelProfileId,
    ModelRevision,
    ProviderAccountId,
    ProviderAccountRevision,
    ExternalModelId,
    DisplayName,
    ProviderProtocol,
    ModelKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialRequirement {
    ApiKey,
    SecretHeader,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatParameter {
    Temperature,
    TopP,
    TopK,
    FrequencyPenalty,
    PresencePenalty,
    RepetitionPenalty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ChatProfileResolutionError {
    #[error("selected model identity does not match live model data: {field:?}")]
    IdentityMismatch { field: IdentityField },
    #[error("provider account is disabled")]
    AccountDisabled,
    #[error("selected model is not a chat model")]
    NonChatModel,
    #[error("chat protocol is not supported by the remote chat resolver")]
    UnsupportedProtocol,
    #[error("required credential is missing: {requirement:?}")]
    MissingCredential { requirement: CredentialRequirement },
    #[error("capability {capability:?} is unsupported")]
    CapabilityUnsupported { capability: RequiredCapability },
    #[error("capability {capability:?} is unknown")]
    CapabilityUnknown { capability: RequiredCapability },
    #[error("required {direction:?} modality {modality:?} is unsupported")]
    ModalityUnsupported {
        direction: ModalityDirection,
        modality: crate::Modality,
    },
    #[error("required {direction:?} modality {modality:?} is unknown")]
    ModalityUnknown {
        direction: ModalityDirection,
        modality: crate::Modality,
    },
    #[error("parameter {parameter:?} is unsupported")]
    ParameterUnsupported { parameter: ChatParameter },
    #[error("parameter {parameter:?} support is unknown")]
    ParameterUnknown { parameter: ChatParameter },
    #[error("resolved context length {requested} exceeds capability limit {available}")]
    ContextLimitExceeded { requested: u32, available: u32 },
    #[error("resolved visible output limit {requested} exceeds capability limit {available}")]
    VisibleOutputLimitExceeded { requested: u32, available: u32 },
    #[error("resolved total output allowance {requested} exceeds capability limit {available}")]
    TotalOutputLimitExceeded { requested: u32, available: u32 },
    #[error("reasoning is disabled but effort or budget is configured")]
    ContradictoryReasoning,
    #[error("chat parameter is invalid: {field}")]
    InvalidParameter { field: &'static str },
    #[error("model capabilities are invalid")]
    InvalidCapabilities,
    #[error("provider connection metadata is invalid")]
    InvalidConnection,
    #[error("completion allowance overflowed")]
    CompletionAllowanceOverflow,
}

pub fn resolve_chat_profile(
    expected: &ExpectedModelIdentity,
    profile: &ModelProfile,
    account: &ProviderAccount,
    input: &ChatParameterResolutionInput,
    requirements: &ChatRequirements,
) -> Result<ResolvedChatProfile, ChatProfileResolutionError> {
    validate_identity(expected, profile, account)?;
    validate_provider_connection(account)
        .map_err(|_| ChatProfileResolutionError::InvalidConnection)?;
    if !account.enabled {
        return Err(ChatProfileResolutionError::AccountDisabled);
    }
    if profile.kind != ModelKind::Chat {
        return Err(ChatProfileResolutionError::NonChatModel);
    }
    if account.protocol == ProviderProtocol::StableDiffusion {
        return Err(ChatProfileResolutionError::UnsupportedProtocol);
    }
    validate_credentials(account)?;
    profile
        .config
        .capabilities
        .validate()
        .map_err(|_| ChatProfileResolutionError::InvalidCapabilities)?;
    let capabilities = &profile.config.capabilities;
    validate_modalities(capabilities, requirements)?;
    validate_required_capabilities(capabilities, requirements)?;
    input.global.validate().map_err(parameter_error)?;
    input.session.validate().map_err(parameter_error)?;
    input.operation.validate().map_err(parameter_error)?;
    let parameters = resolve_parameters(&profile.config.chat_parameters, input)?;
    validate_parameter_support(capabilities.parameter_support, &parameters)?;
    validate_configured_capabilities(capabilities, &parameters)?;
    let mut warnings = Vec::new();
    validate_limits(capabilities, &parameters, &mut warnings)?;
    Ok(ResolvedChatProfile {
        model_profile_id: profile.id,
        model_revision: profile.revision,
        provider_account_id: account.id,
        provider_account_revision: account.revision,
        secret_owner_id: account.secret_owner_id,
        external_model_id: profile.external_model_id.clone(),
        provider_kind: account.provider_kind.clone(),
        provider_protocol: account.protocol,
        endpoint: account.endpoint.clone(),
        provider_config: account.config.clone(),
        capabilities: capabilities.clone(),
        parameters,
        api_key_ref: account.api_key_ref,
        secret_headers: account.secret_headers.clone(),
        warnings,
    })
}

fn validate_identity(
    expected: &ExpectedModelIdentity,
    profile: &ModelProfile,
    account: &ProviderAccount,
) -> Result<(), ChatProfileResolutionError> {
    let checks = [
        (
            expected.model_profile_id == profile.id,
            IdentityField::ModelProfileId,
        ),
        (
            expected.model_revision == profile.revision,
            IdentityField::ModelRevision,
        ),
        (
            expected.provider_account_id == profile.provider_account_id,
            IdentityField::ProviderAccountId,
        ),
        (
            expected.provider_account_id == account.id,
            IdentityField::ProviderAccountId,
        ),
        (
            expected.provider_account_revision == account.revision,
            IdentityField::ProviderAccountRevision,
        ),
        (
            expected.external_model_id == profile.external_model_id,
            IdentityField::ExternalModelId,
        ),
        (
            expected.display_name == profile.display_name,
            IdentityField::DisplayName,
        ),
        (
            expected.provider_protocol == account.protocol,
            IdentityField::ProviderProtocol,
        ),
        (
            expected.model_kind == profile.kind,
            IdentityField::ModelKind,
        ),
    ];
    checks
        .into_iter()
        .find_map(|(matches, field)| (!matches).then_some(field))
        .map_or(Ok(()), |field| {
            Err(ChatProfileResolutionError::IdentityMismatch { field })
        })
}

fn validate_credentials(account: &ProviderAccount) -> Result<(), ChatProfileResolutionError> {
    let required = match &account.config {
        ProviderConfig::Standard => matches!(
            account.protocol,
            ProviderProtocol::OpenAiCompatible
                | ProviderProtocol::Anthropic
                | ProviderProtocol::Gemini
        )
        .then_some(CredentialRequirement::ApiKey),
        ProviderConfig::Custom { auth, .. } => match auth {
            crate::CustomAuth::None => None,
            crate::CustomAuth::Bearer | crate::CustomAuth::Query => {
                Some(CredentialRequirement::ApiKey)
            }
            crate::CustomAuth::Header => Some(CredentialRequirement::SecretHeader),
        },
    };
    match required {
        Some(CredentialRequirement::ApiKey) if account.api_key_ref.is_none() => {
            Err(ChatProfileResolutionError::MissingCredential {
                requirement: CredentialRequirement::ApiKey,
            })
        }
        Some(CredentialRequirement::SecretHeader) if account.secret_headers.is_empty() => {
            Err(ChatProfileResolutionError::MissingCredential {
                requirement: CredentialRequirement::SecretHeader,
            })
        }
        _ => Ok(()),
    }
}

fn validate_modalities(
    capabilities: &ModelCapabilities,
    requirements: &ChatRequirements,
) -> Result<(), ChatProfileResolutionError> {
    for &modality in &requirements.input_modalities {
        require_modality(
            ModalityDirection::Input,
            modality,
            capabilities.input_modalities.get(modality),
        )?;
    }
    for &modality in &requirements.output_modalities {
        require_modality(
            ModalityDirection::Output,
            modality,
            capabilities.output_modalities.get(modality),
        )?;
    }
    Ok(())
}

fn validate_required_capabilities(
    capabilities: &ModelCapabilities,
    requirements: &ChatRequirements,
) -> Result<(), ChatProfileResolutionError> {
    for (required, capability, status) in [
        (
            requirements.require_streaming,
            RequiredCapability::Streaming,
            capabilities.streaming,
        ),
        (
            requirements.require_tools,
            RequiredCapability::Tools,
            capabilities.tools,
        ),
        (
            requirements.require_structured_output,
            RequiredCapability::StructuredOutput,
            capabilities.structured_output,
        ),
        (
            requirements.require_reasoning,
            RequiredCapability::Reasoning,
            capabilities.reasoning,
        ),
        (
            requirements.require_prompt_cache,
            RequiredCapability::PromptCache,
            capabilities.prompt_cache,
        ),
    ] {
        if required {
            require_capability(capability, status)?;
        }
    }
    Ok(())
}

fn require_capability(
    capability: RequiredCapability,
    status: CapabilityStatus,
) -> Result<(), ChatProfileResolutionError> {
    match status {
        CapabilityStatus::Supported => Ok(()),
        CapabilityStatus::Unsupported => {
            Err(ChatProfileResolutionError::CapabilityUnsupported { capability })
        }
        CapabilityStatus::Unknown => {
            Err(ChatProfileResolutionError::CapabilityUnknown { capability })
        }
    }
}

fn require_modality(
    direction: ModalityDirection,
    modality: crate::Modality,
    status: CapabilityStatus,
) -> Result<(), ChatProfileResolutionError> {
    match status {
        CapabilityStatus::Supported => Ok(()),
        CapabilityStatus::Unsupported => Err(ChatProfileResolutionError::ModalityUnsupported {
            direction,
            modality,
        }),
        CapabilityStatus::Unknown => Err(ChatProfileResolutionError::ModalityUnknown {
            direction,
            modality,
        }),
    }
}

fn resolve_parameters(
    profile: &ChatParameterProfile,
    input: &ChatParameterResolutionInput,
) -> Result<ResolvedChatParameters, ChatProfileResolutionError> {
    let global = &input.global;
    let session = &input.session;
    let operation = &input.operation;
    let reasoning_mode = resolve_model_only(
        &operation.reasoning_mode,
        &session.reasoning_mode,
        profile.reasoning_mode,
    );
    let reasoning_effort = resolve_model_only(
        &operation.reasoning_effort,
        &session.reasoning_effort,
        profile.reasoning_effort,
    );
    let reasoning_budget_tokens = resolve_model_only(
        &operation.reasoning_budget_tokens,
        &session.reasoning_budget_tokens,
        profile.reasoning_budget_tokens,
    );
    let reasoning_mode = if reasoning_mode.is_none()
        && (reasoning_effort.is_some() || reasoning_budget_tokens.is_some())
    {
        Some(ReasoningMode::Enabled)
    } else {
        reasoning_mode
    };
    let reasoning_budget_tokens =
        if reasoning_mode == Some(ReasoningMode::Enabled) && reasoning_budget_tokens.is_none() {
            reasoning_effort.map(reasoning_budget)
        } else {
            reasoning_budget_tokens
        };
    if reasoning_mode == Some(ReasoningMode::Disabled)
        && (reasoning_effort.is_some() || reasoning_budget_tokens.is_some())
    {
        return Err(ChatProfileResolutionError::ContradictoryReasoning);
    }
    let visible = resolve_common(
        &operation.max_output_tokens,
        &session.max_output_tokens,
        profile.max_output_tokens,
        global.max_output_tokens,
    );
    let total = match (visible, reasoning_budget_tokens) {
        (Some(visible), Some(reasoning)) => Some(
            visible
                .checked_add(reasoning)
                .ok_or(ChatProfileResolutionError::CompletionAllowanceOverflow)?,
        ),
        (Some(visible), None) => Some(visible),
        _ => None,
    };
    Ok(ResolvedChatParameters {
        temperature: resolve_common(
            &operation.temperature,
            &session.temperature,
            profile.temperature,
            global.temperature,
        ),
        top_p: resolve_common(
            &operation.top_p,
            &session.top_p,
            profile.top_p,
            global.top_p,
        ),
        top_k: resolve_model_only(&operation.top_k, &session.top_k, profile.top_k),
        visible_max_output_tokens: visible,
        context_length: resolve_common(
            &operation.context_length,
            &session.context_length,
            profile.context_length,
            global.context_length,
        ),
        frequency_penalty: resolve_model_only(
            &operation.frequency_penalty,
            &session.frequency_penalty,
            profile.frequency_penalty,
        ),
        presence_penalty: resolve_model_only(
            &operation.presence_penalty,
            &session.presence_penalty,
            profile.presence_penalty,
        ),
        repetition_penalty: resolve_model_only(
            &operation.repetition_penalty,
            &session.repetition_penalty,
            profile.repetition_penalty,
        ),
        reasoning_mode,
        reasoning_effort,
        reasoning_budget_tokens,
        prompt_caching: profile.prompt_caching,
        total_completion_allowance: total,
    })
}

fn resolve_common<T: Clone>(
    operation: &ParameterOverride<T>,
    session: &ParameterOverride<T>,
    profile: Option<T>,
    global: Option<T>,
) -> Option<T> {
    match operation {
        ParameterOverride::Set(value) => Some(value.clone()),
        ParameterOverride::Clear => None,
        ParameterOverride::Inherit => match session {
            ParameterOverride::Set(value) => Some(value.clone()),
            ParameterOverride::Clear => None,
            ParameterOverride::Inherit => profile.or(global),
        },
    }
}

fn resolve_model_only<T: Clone>(
    operation: &ParameterOverride<T>,
    session: &ParameterOverride<T>,
    profile: Option<T>,
) -> Option<T> {
    match operation {
        ParameterOverride::Set(value) => Some(value.clone()),
        ParameterOverride::Clear => None,
        ParameterOverride::Inherit => match session {
            ParameterOverride::Set(value) => Some(value.clone()),
            ParameterOverride::Clear => None,
            ParameterOverride::Inherit => profile,
        },
    }
}

fn reasoning_budget(effort: ReasoningEffort) -> u32 {
    match effort {
        ReasoningEffort::Low => 2_048,
        ReasoningEffort::Medium => 8_192,
        ReasoningEffort::High => 16_384,
    }
}

fn validate_parameter_support(
    support: ParameterSupport,
    parameters: &ResolvedChatParameters,
) -> Result<(), ChatProfileResolutionError> {
    for (configured, parameter, status) in [
        (
            parameters.temperature.is_some(),
            ChatParameter::Temperature,
            support.temperature,
        ),
        (
            parameters.top_p.is_some(),
            ChatParameter::TopP,
            support.top_p,
        ),
        (
            parameters.top_k.is_some(),
            ChatParameter::TopK,
            support.top_k,
        ),
        (
            parameters.frequency_penalty.is_some(),
            ChatParameter::FrequencyPenalty,
            support.frequency_penalty,
        ),
        (
            parameters.presence_penalty.is_some(),
            ChatParameter::PresencePenalty,
            support.presence_penalty,
        ),
        (
            parameters.repetition_penalty.is_some(),
            ChatParameter::RepetitionPenalty,
            support.repetition_penalty,
        ),
    ] {
        if configured {
            match status {
                CapabilityStatus::Supported => {}
                CapabilityStatus::Unsupported => {
                    return Err(ChatProfileResolutionError::ParameterUnsupported { parameter });
                }
                CapabilityStatus::Unknown => {
                    return Err(ChatProfileResolutionError::ParameterUnknown { parameter });
                }
            }
        }
    }
    Ok(())
}

fn validate_configured_capabilities(
    capabilities: &ModelCapabilities,
    parameters: &ResolvedChatParameters,
) -> Result<(), ChatProfileResolutionError> {
    if parameters.reasoning_mode == Some(ReasoningMode::Enabled)
        || parameters.reasoning_effort.is_some()
        || parameters.reasoning_budget_tokens.is_some()
    {
        require_capability(RequiredCapability::Reasoning, capabilities.reasoning)?;
    }
    if parameters.prompt_caching == Some(PromptCaching::Enabled { ttl_seconds: None })
        || matches!(
            parameters.prompt_caching,
            Some(PromptCaching::Enabled {
                ttl_seconds: Some(_)
            })
        )
    {
        require_capability(RequiredCapability::PromptCache, capabilities.prompt_cache)?;
    }
    Ok(())
}

fn validate_limits(
    capabilities: &ModelCapabilities,
    parameters: &ResolvedChatParameters,
    warnings: &mut Vec<ChatProfileWarning>,
) -> Result<(), ChatProfileResolutionError> {
    if let Some(requested) = parameters.context_length {
        if let Some(available) = capabilities.context_length {
            if requested > available {
                return Err(ChatProfileResolutionError::ContextLimitExceeded {
                    requested,
                    available,
                });
            }
        }
    }
    if let Some(requested) = parameters.visible_max_output_tokens {
        if let Some(available) = capabilities.max_visible_output_tokens {
            if requested > available {
                return Err(ChatProfileResolutionError::VisibleOutputLimitExceeded {
                    requested,
                    available,
                });
            }
        }
    }
    if let Some(requested) = parameters.total_completion_allowance {
        if let Some(available) = capabilities.max_total_completion_tokens {
            if requested > available {
                return Err(ChatProfileResolutionError::TotalOutputLimitExceeded {
                    requested,
                    available,
                });
            }
        }
    }
    if parameters.context_length.is_some() && capabilities.context_length.is_none() {
        warnings.push(ChatProfileWarning::ContextLimitUnverified);
    }
    if parameters.visible_max_output_tokens.is_some()
        && capabilities.max_visible_output_tokens.is_none()
    {
        warnings.push(ChatProfileWarning::VisibleOutputLimitUnverified);
    }
    if parameters.total_completion_allowance.is_some()
        && capabilities.max_total_completion_tokens.is_none()
    {
        warnings.push(ChatProfileWarning::TotalOutputLimitUnverified);
    }
    Ok(())
}

fn parameter_error(error: ParameterValidationError) -> ChatProfileResolutionError {
    match error {
        ParameterValidationError::InvalidValue(field) => {
            ChatProfileResolutionError::InvalidParameter { field }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CapabilityEvidence, CapabilityEvidenceSource, ModalityCapabilities, ProviderConfig,
        SecretHeader,
    };
    use lettuce_settings::{HeaderName, SecretOwnerId, SecretRef};
    use lettuce_types::{Revision, TimestampMillis};

    fn fixture() -> (ExpectedModelIdentity, ModelProfile, ProviderAccount) {
        let model_profile_id = ModelProfileId::new();
        let provider_account_id = ProviderAccountId::new();
        let mut capabilities = ModelCapabilities::unknown(CapabilityEvidence {
            source: CapabilityEvidenceSource::ProviderReported,
            source_version: 1,
            observed_at: TimestampMillis::new(3),
        });
        capabilities.input_modalities = ModalityCapabilities {
            text: CapabilityStatus::Supported,
            image: CapabilityStatus::Unsupported,
            audio: CapabilityStatus::Unsupported,
        };
        capabilities.output_modalities = capabilities.input_modalities;
        capabilities.tools = CapabilityStatus::Supported;
        capabilities.streaming = CapabilityStatus::Supported;
        capabilities.reasoning = CapabilityStatus::Supported;
        capabilities.prompt_cache = CapabilityStatus::Supported;
        capabilities.context_length = Some(2_000);
        capabilities.max_visible_output_tokens = Some(200);
        capabilities.max_total_completion_tokens = Some(20_000);
        capabilities.parameter_support = ParameterSupport {
            temperature: CapabilityStatus::Supported,
            top_p: CapabilityStatus::Supported,
            top_k: CapabilityStatus::Supported,
            frequency_penalty: CapabilityStatus::Supported,
            presence_penalty: CapabilityStatus::Supported,
            repetition_penalty: CapabilityStatus::Supported,
        };
        let profile = ModelProfile {
            id: model_profile_id,
            provider_account_id,
            external_model_id: "remote-model".into(),
            display_name: "Remote model".into(),
            kind: ModelKind::Chat,
            config: crate::ModelProfileConfig {
                chat_parameters: ChatParameterProfile {
                    temperature: Some(0.7),
                    max_output_tokens: Some(100),
                    context_length: Some(1_000),
                    ..ChatParameterProfile::default()
                },
                capabilities,
            },
            revision: Revision::new(2),
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(2),
        };
        let account = ProviderAccount {
            id: provider_account_id,
            secret_owner_id: SecretOwnerId::new(),
            provider_kind: "test".into(),
            protocol: ProviderProtocol::OpenAiCompatible,
            label: "Test account".into(),
            endpoint: Some("https://example.invalid".into()),
            enabled: true,
            api_key_ref: Some(SecretRef::new()),
            secret_headers: vec![SecretHeader {
                name: HeaderName::new("x-test").expect("header"),
                secret_ref: SecretRef::new(),
            }],
            config: ProviderConfig::Standard,
            revision: Revision::new(3),
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(2),
        };
        let expected = ExpectedModelIdentity {
            model_profile_id,
            model_revision: profile.revision,
            provider_account_id,
            provider_account_revision: account.revision,
            external_model_id: profile.external_model_id.clone(),
            display_name: profile.display_name.clone(),
            provider_protocol: account.protocol,
            model_kind: profile.kind,
        };
        (expected, profile, account)
    }

    #[test]
    fn resolution_is_stable_secret_free_and_derives_reasoning_budget() {
        let (expected, profile, account) = fixture();
        let input = ChatParameterResolutionInput {
            operation: ChatParameterOverrides {
                temperature: ParameterOverride::Set(0.9),
                reasoning_effort: ParameterOverride::Set(ReasoningEffort::Low),
                ..ChatParameterOverrides::default()
            },
            ..ChatParameterResolutionInput::default()
        };
        let resolved = resolve_chat_profile(
            &expected,
            &profile,
            &account,
            &input,
            &ChatRequirements::default(),
        )
        .expect("profile resolves");
        assert_eq!(resolved.parameters.temperature, Some(0.9));
        assert_eq!(resolved.parameters.reasoning_budget_tokens, Some(2_048));
        assert_eq!(resolved.parameters.total_completion_allowance, Some(2_148));
        assert_eq!(resolved.provider_kind, account.provider_kind);
        assert_eq!(resolved.endpoint, account.endpoint);
        assert_eq!(resolved.provider_config, account.config);
        assert_eq!(resolved.secret_owner_id, account.secret_owner_id);
        let encoded = serde_json::to_string(&resolved).expect("resolved encode");
        assert!(encoded.contains("example.invalid"));
        assert!(encoded.contains("api_key_ref"));
        assert!(encoded.contains("secret_owner_id"));
        assert!(encoded.contains("provider_kind"));
        assert!(encoded.contains("provider_config"));
        assert!(format!("{resolved:?}").contains(&account.api_key_ref.expect("key").to_string()));
        assert!(!encoded.contains("canary-secret-value"));
        assert!(!format!("{resolved:?}").contains("canary-secret-value"));
        assert_eq!(
            encoded,
            serde_json::to_string(&resolved).expect("stable encode")
        );
    }

    #[test]
    fn unknown_and_unsupported_parameters_are_distinct() {
        let (expected, mut profile, account) = fixture();
        profile.config.capabilities.parameter_support.temperature = CapabilityStatus::Unknown;
        assert!(matches!(
            resolve_chat_profile(
                &expected,
                &profile,
                &account,
                &ChatParameterResolutionInput::default(),
                &ChatRequirements::default()
            ),
            Err(ChatProfileResolutionError::ParameterUnknown {
                parameter: ChatParameter::Temperature
            })
        ));
        profile.config.capabilities.parameter_support.temperature = CapabilityStatus::Unsupported;
        assert!(matches!(
            resolve_chat_profile(
                &expected,
                &profile,
                &account,
                &ChatParameterResolutionInput::default(),
                &ChatRequirements::default()
            ),
            Err(ChatProfileResolutionError::ParameterUnsupported {
                parameter: ChatParameter::Temperature
            })
        ));
    }

    #[test]
    fn frozen_identity_and_credentials_are_checked_without_secret_loading() {
        let (mut expected, profile, account) = fixture();
        expected.provider_account_revision = Revision::new(99);
        assert!(matches!(
            resolve_chat_profile(
                &expected,
                &profile,
                &account,
                &ChatParameterResolutionInput::default(),
                &ChatRequirements::default()
            ),
            Err(ChatProfileResolutionError::IdentityMismatch {
                field: IdentityField::ProviderAccountRevision
            })
        ));
        let mut missing = account.clone();
        missing.api_key_ref = None;
        expected.provider_account_revision = account.revision;
        assert!(matches!(
            resolve_chat_profile(
                &expected,
                &profile,
                &missing,
                &ChatParameterResolutionInput::default(),
                &ChatRequirements::default()
            ),
            Err(ChatProfileResolutionError::MissingCredential {
                requirement: CredentialRequirement::ApiKey
            })
        ));
    }

    #[test]
    fn connection_metadata_rejects_endpoint_credentials_queries_and_bad_paths() {
        let (expected, profile, account) = fixture();
        for endpoint in [
            "https://user:password@example.invalid",
            "https://example.invalid/api?token=canary-secret-value",
            "https://example.invalid/api#fragment",
            "ftp://example.invalid/api",
        ] {
            let mut invalid = account.clone();
            invalid.endpoint = Some(endpoint.into());
            let error = resolve_chat_profile(
                &expected,
                &profile,
                &invalid,
                &ChatParameterResolutionInput::default(),
                &ChatRequirements::default(),
            )
            .expect_err("invalid endpoint must be rejected");
            assert!(matches!(
                error,
                ChatProfileResolutionError::InvalidConnection
            ));
            assert!(!format!("{error:?}").contains("canary-secret-value"));
        }

        let mut accepted = account.clone();
        accepted.endpoint = Some("https://example.invalid/api/@v1".into());
        assert!(
            resolve_chat_profile(
                &expected,
                &profile,
                &accepted,
                &ChatParameterResolutionInput::default(),
                &ChatRequirements::default(),
            )
            .is_ok()
        );
        accepted.config = ProviderConfig::Custom {
            chat_path: "/v1/users/@me".into(),
            models_path: Some("/models/@all".into()),
            streaming: true,
            auth: crate::CustomAuth::Bearer,
        };
        assert!(
            resolve_chat_profile(
                &expected,
                &profile,
                &accepted,
                &ChatParameterResolutionInput::default(),
                &ChatRequirements::default(),
            )
            .is_ok()
        );

        let mut invalid = account;
        invalid.config = ProviderConfig::Custom {
            chat_path: "/chat/completions?token=canary-secret-value".into(),
            models_path: Some("/models".into()),
            streaming: true,
            auth: crate::CustomAuth::Bearer,
        };
        let error = resolve_chat_profile(
            &expected,
            &profile,
            &invalid,
            &ChatParameterResolutionInput::default(),
            &ChatRequirements::default(),
        )
        .expect_err("invalid provider path must be rejected");
        assert!(matches!(
            error,
            ChatProfileResolutionError::InvalidConnection
        ));
        assert!(!format!("{error:?}").contains("canary-secret-value"));
    }

    #[test]
    fn clear_stops_at_the_authored_layer_boundary() {
        let (expected, profile, account) = fixture();
        let input = ChatParameterResolutionInput {
            global: ChatParameterProfile {
                temperature: Some(0.2),
                context_length: Some(500),
                ..ChatParameterProfile::default()
            },
            session: ChatParameterOverrides {
                temperature: ParameterOverride::Set(0.8),
                context_length: ParameterOverride::Set(900),
                frequency_penalty: ParameterOverride::Set(0.1),
                ..ChatParameterOverrides::default()
            },
            operation: ChatParameterOverrides {
                temperature: ParameterOverride::Clear,
                context_length: ParameterOverride::Inherit,
                frequency_penalty: ParameterOverride::Clear,
                ..ChatParameterOverrides::default()
            },
        };
        let resolved = resolve_chat_profile(
            &expected,
            &profile,
            &account,
            &input,
            &ChatRequirements::default(),
        )
        .expect("profile resolves");
        assert_eq!(resolved.parameters.temperature, None);
        assert_eq!(resolved.parameters.context_length, Some(900));
        assert_eq!(resolved.parameters.frequency_penalty, None);

        let input = ChatParameterResolutionInput {
            session: ChatParameterOverrides {
                temperature: ParameterOverride::Clear,
                ..ChatParameterOverrides::default()
            },
            ..ChatParameterResolutionInput::default()
        };
        let resolved = resolve_chat_profile(
            &expected,
            &profile,
            &account,
            &input,
            &ChatRequirements::default(),
        )
        .expect("profile resolves");
        assert_eq!(resolved.parameters.temperature, None);
    }

    #[test]
    fn authored_reasoning_and_prompt_cache_require_supported_capabilities() {
        let (expected, mut profile, account) = fixture();
        profile.config.chat_parameters.reasoning_mode = Some(ReasoningMode::Enabled);
        profile.config.capabilities.reasoning = CapabilityStatus::Unknown;
        assert!(matches!(
            resolve_chat_profile(
                &expected,
                &profile,
                &account,
                &ChatParameterResolutionInput::default(),
                &ChatRequirements::default()
            ),
            Err(ChatProfileResolutionError::CapabilityUnknown {
                capability: RequiredCapability::Reasoning
            })
        ));

        profile.config.chat_parameters.reasoning_mode = None;
        profile.config.chat_parameters.prompt_caching = Some(PromptCaching::Enabled {
            ttl_seconds: Some(60),
        });
        profile.config.capabilities.reasoning = CapabilityStatus::Supported;
        profile.config.capabilities.prompt_cache = CapabilityStatus::Unsupported;
        assert!(matches!(
            resolve_chat_profile(
                &expected,
                &profile,
                &account,
                &ChatParameterResolutionInput::default(),
                &ChatRequirements::default()
            ),
            Err(ChatProfileResolutionError::CapabilityUnsupported {
                capability: RequiredCapability::PromptCache
            })
        ));
    }
}
