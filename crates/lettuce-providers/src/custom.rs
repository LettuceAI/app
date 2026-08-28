use std::borrow::Cow;

use lettuce_conversations::{MessageRole, ToolChoice};
use lettuce_models::{CustomToolChoiceMode, ProviderConfig};

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

    fn supports_streaming(&self, config: &ProviderConfig) -> bool {
        custom_config(config).is_ok_and(|config| config.streaming)
    }

    fn tool_choice(
        &self,
        choice: &ToolChoice,
        config: &ProviderConfig,
    ) -> Result<Option<serde_json::Value>, AdapterError> {
        let mode = custom_config(config)?.tool_choice_mode;
        Ok(match mode {
            CustomToolChoiceMode::Auto => Some("auto".into()),
            CustomToolChoiceMode::Required => Some("required".into()),
            CustomToolChoiceMode::None => Some("none".into()),
            CustomToolChoiceMode::Omit => None,
            CustomToolChoiceMode::Passthrough => Some(match choice {
                ToolChoice::Auto => "auto".into(),
                ToolChoice::Required => "required".into(),
                ToolChoice::Named { name } => serde_json::json!({
                    "type": "function",
                    "function": { "name": name }
                }),
            }),
        })
    }

    fn extend_config_body(
        &self,
        config: &ProviderConfig,
        parameters: &lettuce_models::ResolvedChatParameters,
        body: &mut serde_json::Map<String, serde_json::Value>,
    ) {
        if custom_config(config).is_ok_and(|config| config.send_chat_template_kwargs) {
            body.insert(
                "chat_template_kwargs".to_owned(),
                serde_json::json!({
                    "enable_thinking": parameters.reasoning_mode
                        == Some(lettuce_models::ReasoningMode::Enabled)
                }),
            );
        }
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
