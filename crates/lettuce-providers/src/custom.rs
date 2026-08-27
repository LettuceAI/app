use std::borrow::Cow;

use lettuce_conversations::MessageRole;
use lettuce_models::ProviderConfig;

use crate::common::{
    RemoteModel, custom_auth_plan, custom_config, parse_custom_model_list, parse_openai_model_list,
};
use crate::descriptor::{
    ApiKeyRequirement, ParameterFlags, PromptCachingSupport, ProviderDescriptor, ReasoningSupport,
};
use crate::openai_compatible::{AdapterError, AuthPlan, OpenAiWireProvider, standard_role};

pub(crate) struct Custom;

impl OpenAiWireProvider for Custom {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &DESCRIPTOR
    }

    fn accepts(&self, config: &ProviderConfig) -> bool {
        matches!(config, ProviderConfig::Custom(_))
    }

    fn chat_path(
        &self,
        _endpoint: &str,
        config: &ProviderConfig,
    ) -> Result<Cow<'static, str>, AdapterError> {
        Ok(Cow::Owned(custom_config(config)?.chat_path.clone()))
    }

    fn models_path(&self, _endpoint: &str, config: &ProviderConfig) -> Option<Cow<'static, str>> {
        custom_config(config)
            .ok()?
            .models_path
            .clone()
            .map(Cow::Owned)
    }

    fn parse_models(
        &self,
        payload: &serde_json::Value,
        config: &ProviderConfig,
    ) -> Vec<RemoteModel> {
        custom_config(config)
            .ok()
            .and_then(|config| parse_custom_model_list(&config.model_list, payload))
            .unwrap_or_else(|| parse_openai_model_list(payload))
    }

    fn role(&self, role: MessageRole, config: &ProviderConfig) -> Option<Cow<'static, str>> {
        let roles = &custom_config(config).ok()?.roles;
        let remapped = match role {
            MessageRole::System | MessageRole::Scene => roles.system.as_ref(),
            MessageRole::User => roles.user.as_ref(),
            MessageRole::Assistant => roles.assistant.as_ref(),
        };
        match remapped {
            Some(custom) => Some(Cow::Owned(custom.as_str().to_owned())),
            None => standard_role(role).map(Cow::Borrowed),
        }
    }

    fn merges_same_role(&self, config: &ProviderConfig) -> bool {
        custom_config(config).is_ok_and(|config| config.merge_same_role_messages)
    }

    fn auth(&self, config: &ProviderConfig) -> Result<AuthPlan, AdapterError> {
        Ok(custom_auth_plan(&custom_config(config)?.auth))
    }
}

pub(crate) const DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    kind: "custom",
    display_name: "Custom (OpenAI-format)",
    protocol: lettuce_models::ProviderProtocol::OpenAiCompatible,
    aliases: &[],
    default_endpoint: None,
    endpoint_editable: true,
    api_key: ApiKeyRequirement::Optional,
    auth_header: "",
    streaming: true,
    lists_models: true,
    verifies_key: false,
    reasoning: ReasoningSupport::Effort,
    prompt_caching: PromptCachingSupport::None,
    parameters: ParameterFlags::PENALTIES_TOP_K_BUDGET,
    extra_body_keys: &[],
};
