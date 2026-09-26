use lettuce_models::{
    ChatParameterOverrides, ChatParameterResolutionInput, FeatureGenerationParameters,
    ParameterOverride, ProviderProtocol, ReasoningMode,
};

/// The temperature, top_p and output cap a feature request uses where the
/// model's feature slot leaves them unset.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeatureSamplingDefaults {
    pub temperature: f64,
    pub top_p: f64,
    pub max_output_tokens: Option<u32>,
}

impl FeatureSamplingDefaults {
    #[must_use]
    pub const fn new(temperature: f64) -> Self {
        Self {
            temperature,
            top_p: 1.0,
            max_output_tokens: None,
        }
    }

    #[must_use]
    pub const fn with_max_tokens(temperature: f64, max_output_tokens: u32) -> Self {
        Self {
            temperature,
            top_p: 1.0,
            max_output_tokens: Some(max_output_tokens),
        }
    }
}

pub const DYNAMIC_MEMORY_DEFAULTS: FeatureSamplingDefaults = FeatureSamplingDefaults::new(0.4);
pub const COMPANION_SOUL_WRITER_DEFAULTS: FeatureSamplingDefaults =
    FeatureSamplingDefaults::new(0.4);
pub const COMPANION_MEMORY_DEFAULTS: FeatureSamplingDefaults = FeatureSamplingDefaults::new(0.3);
pub const LOREBOOK_ENTRY_GENERATOR_DEFAULTS: FeatureSamplingDefaults =
    FeatureSamplingDefaults::new(0.2);
pub const LOREBOOK_GENERATOR_DEFAULTS: FeatureSamplingDefaults = FeatureSamplingDefaults::new(0.3);
pub const SCENE_WRITER_DEFAULTS: FeatureSamplingDefaults =
    FeatureSamplingDefaults::with_max_tokens(0.7, 1280);
pub const SCENE_DESIGN_REFERENCE_DEFAULTS: FeatureSamplingDefaults =
    FeatureSamplingDefaults::new(0.4);
pub const HELP_ME_REPLY_DEFAULTS: FeatureSamplingDefaults = FeatureSamplingDefaults::new(0.8);
pub const GROUP_SPEAKER_SELECTION_DEFAULTS: FeatureSamplingDefaults =
    FeatureSamplingDefaults::with_max_tokens(0.3, 500);
pub const CREATION_HELPER_DEFAULTS: FeatureSamplingDefaults =
    FeatureSamplingDefaults::with_max_tokens(0.7, 20480);

/// Which request fields a feature passes besides temperature, top_p, the
/// output cap and the context length. Ollama always receives top_k and the
/// penalties through its request options; llama.cpp gets them only for
/// `Full` features and otherwise samples with its sampler-profile defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureRequestFields {
    Sampling,
    SamplingAndPromptCache,
    Full,
}

fn unset<T>(value: &ParameterOverride<T>) -> bool {
    !matches!(value, ParameterOverride::Set(_))
}

/// The parameters of one app feature request: the model's feature slot
/// overrides the model, unset temperature and top_p (and the output cap when
/// the feature has one) come from the feature defaults, reasoning is off, and
/// a conversation's own model settings do not apply (the feature slot takes
/// their place). The app layer still fills what the model leaves unset.
#[must_use]
pub fn feature_parameter_input(
    slot: &FeatureGenerationParameters,
    defaults: FeatureSamplingDefaults,
    fields: FeatureRequestFields,
    protocol: ProviderProtocol,
    global: &lettuce_models::ModelSettingsLayer,
) -> ChatParameterResolutionInput {
    let mut operation: ChatParameterOverrides = slot.parameters.clone();
    if unset(&operation.temperature) {
        operation.temperature = ParameterOverride::Set(defaults.temperature);
    }
    if unset(&operation.top_p) {
        operation.top_p = ParameterOverride::Set(defaults.top_p);
    }
    if let Some(max_output_tokens) = defaults.max_output_tokens
        && unset(&operation.max_output_tokens)
    {
        operation.max_output_tokens = ParameterOverride::Set(max_output_tokens);
    }
    operation.reasoning_mode = ParameterOverride::Set(ReasoningMode::Disabled);
    operation.reasoning_effort = ParameterOverride::Clear;
    operation.reasoning_budget_tokens = ParameterOverride::Clear;
    if protocol != ProviderProtocol::Ollama {
        operation.repetition_penalty = ParameterOverride::Clear;
        if fields != FeatureRequestFields::Full {
            operation.top_k = ParameterOverride::Clear;
            operation.frequency_penalty = ParameterOverride::Clear;
            operation.presence_penalty = ParameterOverride::Clear;
        }
    }
    if fields == FeatureRequestFields::Sampling {
        operation.prompt_caching = ParameterOverride::Clear;
    }
    ChatParameterResolutionInput {
        global: global.chat_parameters.clone(),
        session: ChatParameterOverrides::default(),
        operation,
        llama_cpp: Box::new(lettuce_models::LlamaResolutionInput {
            global: global.llama_cpp.clone(),
            session: lettuce_models::LlamaCppSettings {
                sampler: slot.llama_sampler.clone(),
                ..lettuce_models::LlamaCppSettings::default()
            },
            memory_sampler: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use lettuce_models::ChatParameterProfile;
    use lettuce_models::ParameterOverride::*;

    use super::*;

    fn slot() -> FeatureGenerationParameters {
        let mut slot = FeatureGenerationParameters::default();
        slot.parameters.temperature = Set(0.25);
        slot.parameters.top_p = Clear;
        slot.parameters.frequency_penalty = Set(0.5);
        slot.parameters.top_k = Set(20);
        slot.parameters.repetition_penalty = Set(1.1);
        slot.parameters.reasoning_effort = Set(lettuce_models::ReasoningEffort::High);
        slot
    }

    #[test]
    fn slot_values_win_and_defaults_fill_the_rest_with_reasoning_off() {
        let global = ChatParameterProfile {
            context_length: Some(4096),
            ..ChatParameterProfile::default()
        };
        let input = feature_parameter_input(
            &slot(),
            GROUP_SPEAKER_SELECTION_DEFAULTS,
            FeatureRequestFields::Sampling,
            ProviderProtocol::OpenAiCompatible,
            &lettuce_models::ModelSettingsLayer {
                chat_parameters: global.clone(),
                ..lettuce_models::ModelSettingsLayer::default()
            },
        );
        assert_eq!(input.operation.temperature, Set(0.25));
        assert_eq!(input.operation.top_p, Set(1.0));
        assert_eq!(input.operation.max_output_tokens, Set(500));
        assert_eq!(input.operation.reasoning_mode, Set(ReasoningMode::Disabled));
        assert_eq!(input.operation.reasoning_effort, Clear);
        assert_eq!(input.operation.reasoning_budget_tokens, Clear);
        assert_eq!(input.operation.frequency_penalty, Clear);
        assert_eq!(input.operation.top_k, Clear);
        assert_eq!(input.operation.repetition_penalty, Clear);
        assert_eq!(input.operation.prompt_caching, Clear);
        assert_eq!(input.session, ChatParameterOverrides::default());
        assert_eq!(input.global, global);
    }

    #[test]
    fn request_options_and_full_features_keep_penalties() {
        let local = feature_parameter_input(
            &slot(),
            DYNAMIC_MEMORY_DEFAULTS,
            FeatureRequestFields::Sampling,
            ProviderProtocol::Ollama,
            &lettuce_models::ModelSettingsLayer::default(),
        );
        assert_eq!(local.operation.frequency_penalty, Set(0.5));
        assert_eq!(local.operation.top_k, Set(20));
        assert_eq!(local.operation.repetition_penalty, Set(1.1));
        assert_eq!(local.operation.max_output_tokens, Inherit);
        let full = feature_parameter_input(
            &slot(),
            HELP_ME_REPLY_DEFAULTS,
            FeatureRequestFields::Full,
            ProviderProtocol::Anthropic,
            &lettuce_models::ModelSettingsLayer::default(),
        );
        assert_eq!(full.operation.frequency_penalty, Set(0.5));
        assert_eq!(full.operation.top_k, Set(20));
        assert_eq!(full.operation.repetition_penalty, Clear);
        assert_eq!(full.operation.prompt_caching, Inherit);
    }

    #[test]
    fn llama_cpp_features_leave_top_k_and_penalties_to_the_sampler_profile() {
        let feature = feature_parameter_input(
            &slot(),
            DYNAMIC_MEMORY_DEFAULTS,
            FeatureRequestFields::Sampling,
            ProviderProtocol::LlamaCpp,
            &lettuce_models::ModelSettingsLayer::default(),
        );
        assert_eq!(feature.operation.top_k, Clear);
        assert_eq!(feature.operation.frequency_penalty, Clear);
        assert_eq!(feature.operation.presence_penalty, Clear);
        let reply = feature_parameter_input(
            &slot(),
            HELP_ME_REPLY_DEFAULTS,
            FeatureRequestFields::Full,
            ProviderProtocol::LlamaCpp,
            &lettuce_models::ModelSettingsLayer::default(),
        );
        assert_eq!(reply.operation.top_k, Set(20));
        assert_eq!(reply.operation.frequency_penalty, Set(0.5));
    }
}
