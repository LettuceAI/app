use lettuce_models::ProviderConfig;

use crate::descriptor::{
    ApiKeyRequirement, ParameterFlags, PromptCachingSupport, ProviderDescriptor, ReasoningSupport,
};
use crate::openai_compatible::{AdapterError, AuthPlan, OpenAiWireProvider};

pub(crate) struct LmStudio;

impl OpenAiWireProvider for LmStudio {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &DESCRIPTOR
    }

    fn auth(&self, _config: &ProviderConfig) -> Result<AuthPlan, AdapterError> {
        Ok(AuthPlan::OptionalBearer)
    }
}

pub(crate) const DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    kind: "lmstudio",
    display_name: "LM Studio (Local)",
    protocol: lettuce_models::ProviderProtocol::OpenAiCompatible,
    aliases: &[],
    default_endpoint: None,
    endpoint_editable: true,
    api_key: ApiKeyRequirement::Optional,
    auth_header: "Authorization",
    streaming: true,
    lists_models: true,
    verifies_key: false,
    reasoning: ReasoningSupport::Effort,
    prompt_caching: PromptCachingSupport::None,
    parameters: ParameterFlags::PENALTIES_TOP_K_BUDGET,
    extra_body_keys: &[],
};
