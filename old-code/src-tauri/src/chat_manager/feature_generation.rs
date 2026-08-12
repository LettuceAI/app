use super::types::{
    AdvancedModelSettings, FeatureGenerationSettings, FeatureGenerationSettingsMap, Model, Session,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LlmFeature {
    DynamicMemory,
    CompanionSoulWriter,
    CompanionMemory,
    LorebookEntryGenerator,
    LorebookGenerator,
    SceneWriter,
    HelpMeReply,
    GroupSpeakerSelection,
    CreationHelper,
}

impl LlmFeature {
    pub fn config<'a>(&self, model: &'a Model) -> Option<&'a FeatureGenerationSettings> {
        let map: &'a FeatureGenerationSettingsMap = model
            .advanced_model_settings
            .as_ref()?
            .feature_generation_settings
            .as_ref()?;
        match self {
            LlmFeature::DynamicMemory => map.dynamic_memory.as_ref(),
            LlmFeature::CompanionSoulWriter => map.companion_soul_writer.as_ref(),
            LlmFeature::CompanionMemory => map.companion_memory.as_ref(),
            LlmFeature::LorebookEntryGenerator => map.lorebook_entry_generator.as_ref(),
            LlmFeature::LorebookGenerator => map.lorebook_generator.as_ref(),
            LlmFeature::SceneWriter => map.scene_writer.as_ref(),
            LlmFeature::HelpMeReply => map.help_me_reply.as_ref(),
            LlmFeature::GroupSpeakerSelection => map.group_speaker_selection.as_ref(),
            LlmFeature::CreationHelper => map.creation_helper.as_ref(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FeatureSamplingDefaults {
    pub temperature: f64,
    pub top_p: f64,
    pub max_output_tokens: Option<u32>,
}

impl FeatureSamplingDefaults {
    pub const fn new(temperature: f64) -> Self {
        Self {
            temperature,
            top_p: 1.0,
            max_output_tokens: None,
        }
    }

    pub const fn with_max_tokens(temperature: f64, max_output_tokens: u32) -> Self {
        Self {
            temperature,
            top_p: 1.0,
            max_output_tokens: Some(max_output_tokens),
        }
    }
}

pub const DYNAMIC_MEMORY_MANAGER_DEFAULTS: FeatureSamplingDefaults =
    FeatureSamplingDefaults::new(0.4);
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

impl FeatureGenerationSettings {
    pub fn to_advanced_model_settings(&self) -> AdvancedModelSettings {
        AdvancedModelSettings {
            temperature: self.temperature,
            top_p: self.top_p,
            top_k: self.top_k,
            max_output_tokens: self.max_output_tokens,
            frequency_penalty: self.frequency_penalty,
            presence_penalty: self.presence_penalty,
            llama_sampler_profile: self.llama_sampler_profile.clone(),
            llama_sampler_order: self.llama_sampler_order.clone(),
            llama_min_p: self.llama_min_p,
            llama_typical_p: self.llama_typical_p,
            llama_repeat_penalty: self.llama_repeat_penalty,
            llama_n_pen_range: self.llama_n_pen_range,
            llama_xtc_probability: self.llama_xtc_probability,
            llama_xtc_threshold: self.llama_xtc_threshold,
            llama_dry_multiplier: self.llama_dry_multiplier,
            llama_dry_base: self.llama_dry_base,
            llama_dry_allowed_length: self.llama_dry_allowed_length,
            llama_dry_penalty_last_n: self.llama_dry_penalty_last_n,
            llama_dry_sequence_breakers: self.llama_dry_sequence_breakers.clone(),
            llama_seed: self.llama_seed,
            ollama_min_p: self.ollama_min_p,
            ollama_typical_p: self.ollama_typical_p,
            ollama_tfs_z: self.ollama_tfs_z,
            ollama_repeat_penalty: self.ollama_repeat_penalty,
            ollama_mirostat: self.ollama_mirostat,
            ollama_mirostat_tau: self.ollama_mirostat_tau,
            ollama_mirostat_eta: self.ollama_mirostat_eta,
            ollama_seed: self.ollama_seed,
            ollama_stop: self.ollama_stop.clone(),
            ..AdvancedModelSettings::empty()
        }
    }

    pub fn has_llama_sampler_override(&self) -> bool {
        self.llama_sampler_profile.is_some()
            || self.llama_sampler_order.is_some()
            || self.llama_min_p.is_some()
            || self.llama_typical_p.is_some()
            || self.llama_repeat_penalty.is_some()
            || self.llama_n_pen_range.is_some()
            || self.llama_xtc_probability.is_some()
            || self.llama_xtc_threshold.is_some()
            || self.llama_dry_multiplier.is_some()
            || self.llama_dry_base.is_some()
            || self.llama_dry_allowed_length.is_some()
            || self.llama_dry_penalty_last_n.is_some()
            || self.llama_dry_sequence_breakers.is_some()
            || self.llama_seed.is_some()
    }
}

pub fn feature_model_overrides(
    model: &Model,
    feature: LlmFeature,
    defaults: FeatureSamplingDefaults,
) -> AdvancedModelSettings {
    let mut overrides = feature
        .config(model)
        .map(FeatureGenerationSettings::to_advanced_model_settings)
        .unwrap_or_else(AdvancedModelSettings::empty);
    if overrides.temperature.is_none() {
        overrides.temperature = Some(defaults.temperature);
    }
    if overrides.top_p.is_none() {
        overrides.top_p = Some(defaults.top_p);
    }
    if overrides.max_output_tokens.is_none() {
        overrides.max_output_tokens = defaults.max_output_tokens;
    }
    overrides
}

pub fn feature_llama_sampler_override_present(model: &Model, feature: LlmFeature) -> bool {
    feature
        .config(model)
        .map(FeatureGenerationSettings::has_llama_sampler_override)
        .unwrap_or(false)
}

pub fn synthetic_feature_session(id: &str, overrides: AdvancedModelSettings) -> Session {
    Session {
        id: id.to_string(),
        character_id: String::new(),
        title: id.to_string(),
        parent_session_id: None,
        branched_from_message_id: None,
        root_session_id: None,
        background_image_path: None,
        system_prompt: None,
        mode: "roleplay".to_string(),
        selected_scene_id: None,
        prompt_template_id: None,
        lorebook_ids_override: None,
        author_note: None,
        persona_id: None,
        persona_disabled: false,
        voice_autoplay: None,
        advanced_model_settings: Some(overrides),
        companion_state: None,
        memories: Vec::new(),
        memory_embeddings: Vec::new(),
        memory_summary: None,
        memory_summary_token_count: 0,
        memory_tool_events: Vec::new(),
        memory_status: None,
        memory_error: None,
        memory_progress_step: None,
        messages: Vec::new(),
        archived: false,
        created_at: 0,
        updated_at: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model_fixture(map: Option<FeatureGenerationSettingsMap>) -> Model {
        Model {
            id: "model".to_string(),
            name: "test".to_string(),
            provider_id: "llamacpp".to_string(),
            provider_credential_id: None,
            provider_label: "llama.cpp".to_string(),
            display_name: "Test Model".to_string(),
            created_at: 0,
            input_scopes: vec!["text".to_string()],
            output_scopes: vec!["text".to_string()],
            advanced_model_settings: Some(AdvancedModelSettings {
                feature_generation_settings: map,
                ..AdvancedModelSettings::empty()
            }),
            prompt_template_id: None,
            voice_config: None,
            system_prompt: None,
        }
    }

    #[test]
    fn empty_advanced_model_settings_has_no_values() {
        let value = serde_json::to_value(AdvancedModelSettings::empty()).expect("serialize");
        let map = value.as_object().expect("object");
        for (key, entry) in map {
            assert!(entry.is_null(), "expected {key} to be null, got {entry}");
        }
    }

    #[test]
    fn feature_overrides_backfill_defaults_without_config() {
        let model = model_fixture(None);
        let overrides = feature_model_overrides(
            &model,
            LlmFeature::DynamicMemory,
            DYNAMIC_MEMORY_MANAGER_DEFAULTS,
        );
        assert_eq!(overrides.temperature, Some(0.4));
        assert_eq!(overrides.top_p, Some(1.0));
        assert_eq!(overrides.max_output_tokens, None);
        assert!(overrides.llama_sampler_order.is_none());
    }

    #[test]
    fn feature_overrides_keep_configured_values() {
        let map = FeatureGenerationSettingsMap {
            help_me_reply: Some(FeatureGenerationSettings {
                temperature: Some(1.1),
                llama_min_p: Some(0.05),
                ..FeatureGenerationSettings::default()
            }),
            ..FeatureGenerationSettingsMap::default()
        };
        let model = model_fixture(Some(map));

        let overrides =
            feature_model_overrides(&model, LlmFeature::HelpMeReply, HELP_ME_REPLY_DEFAULTS);
        assert_eq!(overrides.temperature, Some(1.1));
        assert_eq!(overrides.top_p, Some(1.0));
        assert_eq!(overrides.llama_min_p, Some(0.05));
        assert!(overrides.feature_generation_settings.is_none());
        assert!(feature_llama_sampler_override_present(
            &model,
            LlmFeature::HelpMeReply
        ));
        assert!(!feature_llama_sampler_override_present(
            &model,
            LlmFeature::DynamicMemory
        ));
    }
}
