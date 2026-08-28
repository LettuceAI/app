use std::borrow::Cow;

use lettuce_models::ProviderConfig;

use crate::anthropic_messages::AnthropicWireProvider;
use crate::common::{
    AdapterError, AuthPlan, RemoteModel, custom_auth_plan, custom_config, parse_custom_model_list,
};
use crate::descriptor::{
    ApiKeyRequirement, ParameterFlags, PromptCachingSupport, ProviderDescriptor, ReasoningSupport,
};

pub(crate) struct CustomAnthropic;

impl AnthropicWireProvider for CustomAnthropic {
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
            .unwrap_or_else(|| crate::anthropic::Anthropic.parse_models(payload, config))
    }

    fn merges_same_role(&self, config: &ProviderConfig) -> bool {
        custom_config(config).is_ok_and(|config| config.merge_same_role_messages)
    }

    fn roles(&self, config: &ProviderConfig) -> (Cow<'static, str>, Cow<'static, str>) {
        let roles = custom_config(config).map(|config| &config.roles).ok();
        let pick = |custom: Option<&lettuce_models::WireRole>, default: &'static str| {
            custom.map_or(Cow::Borrowed(default), |role| {
                Cow::Owned(role.as_str().to_owned())
            })
        };
        (
            pick(roles.and_then(|roles| roles.user.as_ref()), "user"),
            pick(
                roles.and_then(|roles| roles.assistant.as_ref()),
                "assistant",
            ),
        )
    }

    fn auth(&self, config: &ProviderConfig) -> Result<AuthPlan, AdapterError> {
        Ok(custom_auth_plan(&custom_config(config)?.auth))
    }

    fn supports_streaming(&self, config: &ProviderConfig) -> bool {
        custom_config(config).is_ok_and(|config| config.streaming)
    }
}

pub(crate) const DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    kind: "custom-anthropic",
    display_name: "Custom (Anthropic-format)",
    protocol: lettuce_models::ProviderProtocol::Anthropic,
    aliases: &[],
    default_endpoint: None,
    endpoint_editable: true,
    api_key: ApiKeyRequirement::Optional,
    auth_header: "",
    streaming: true,
    lists_models: true,
    verifies_key: false,
    reasoning: ReasoningSupport::BudgetOnly,
    prompt_caching: PromptCachingSupport::CacheControl,
    parameters: ParameterFlags {
        top_k: true,
        reasoning_budget: true,
        ..ParameterFlags::BASIC
    },
    extra_body_keys: &[],
};
