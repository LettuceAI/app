use lettuce_models::{
    CapabilityStatus, ChatParameterOverrides, ChatParameterProfile, ChatParameterResolutionInput,
    FeatureGenerationParameters, ParameterOverride, ParameterSupport, ReasoningMode,
};

/// Legacy `FeatureSamplingDefaults`: the temperature, top_p and output cap a
/// feature request uses where the model's feature slot leaves them unset.
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

fn declared<T>(value: &mut ParameterOverride<T>, status: CapabilityStatus) {
    if status != CapabilityStatus::Supported && matches!(value, ParameterOverride::Set(_)) {
        *value = ParameterOverride::Clear;
    }
}

/// Legacy `feature_model_overrides` + `prepare_feature_request` for one app
/// feature: the model's feature slot overrides the model, unset temperature
/// and top_p (and the output cap when the feature has one) come from the
/// feature defaults, reasoning is off, and a conversation's own model settings
/// do not apply (legacy replaced them with the feature slot). The app layer
/// still fills what the model leaves unset. A gated sampling parameter the
/// model does not declare is left out: legacy sent it for the provider to
/// ignore, the resolver rejects it.
#[must_use]
pub fn feature_parameter_input(
    slot: &FeatureGenerationParameters,
    defaults: FeatureSamplingDefaults,
    global: &ChatParameterProfile,
    support: ParameterSupport,
) -> ChatParameterResolutionInput {
    let mut operation: ChatParameterOverrides = slot.parameters.clone();
    if operation.temperature == ParameterOverride::Inherit {
        operation.temperature = ParameterOverride::Set(defaults.temperature);
    }
    if operation.top_p == ParameterOverride::Inherit {
        operation.top_p = ParameterOverride::Set(defaults.top_p);
    }
    if let Some(max_output_tokens) = defaults.max_output_tokens
        && operation.max_output_tokens == ParameterOverride::Inherit
    {
        operation.max_output_tokens = ParameterOverride::Set(max_output_tokens);
    }
    operation.reasoning_mode = ParameterOverride::Set(ReasoningMode::Disabled);
    operation.reasoning_effort = ParameterOverride::Clear;
    operation.reasoning_budget_tokens = ParameterOverride::Clear;
    declared(&mut operation.temperature, support.temperature);
    declared(&mut operation.top_p, support.top_p);
    declared(&mut operation.top_k, support.top_k);
    declared(&mut operation.frequency_penalty, support.frequency_penalty);
    declared(&mut operation.presence_penalty, support.presence_penalty);
    declared(
        &mut operation.repetition_penalty,
        support.repetition_penalty,
    );
    ChatParameterResolutionInput {
        global: global.clone(),
        session: ChatParameterOverrides::default(),
        operation,
    }
}

#[cfg(test)]
mod tests {
    use lettuce_models::{CapabilityStatus, ParameterOverride::*, ParameterSupport};

    use super::*;

    #[test]
    fn slot_values_win_and_defaults_fill_the_rest_with_reasoning_off() {
        let mut slot = FeatureGenerationParameters::default();
        slot.parameters.temperature = Set(0.25);
        slot.parameters.frequency_penalty = Set(0.5);
        let support = ParameterSupport {
            temperature: CapabilityStatus::Supported,
            top_p: CapabilityStatus::Supported,
            ..ParameterSupport::default()
        };
        let input = feature_parameter_input(
            &slot,
            GROUP_SPEAKER_SELECTION_DEFAULTS,
            &ChatParameterProfile::default(),
            support,
        );
        assert_eq!(input.operation.temperature, Set(0.25));
        assert_eq!(input.operation.top_p, Set(1.0));
        assert_eq!(input.operation.max_output_tokens, Set(500));
        assert_eq!(input.operation.frequency_penalty, Clear);
        assert_eq!(input.operation.reasoning_mode, Set(ReasoningMode::Disabled));
        assert_eq!(input.session, ChatParameterOverrides::default());
    }
}
