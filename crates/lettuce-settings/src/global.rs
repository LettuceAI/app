use lettuce_types::{ModelProfileId, PromptDocumentId, Revision, TimestampMillis};
use serde::{Deserialize, Serialize};

pub const GLOBAL_SETTINGS_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GlobalSettings {
    pub pure_mode: PureMode,
    pub analytics_enabled: bool,
    pub update_checks_enabled: bool,
    #[serde(default)]
    pub lorebook_generator: LorebookGeneratorSettings,
    #[serde(default)]
    pub dynamic_memory: DynamicMemorySettings,
    #[serde(default)]
    pub group_dynamic_memory: Option<DynamicMemorySettings>,
    #[serde(default)]
    pub embedding: EmbeddingSettings,
    #[serde(default = "default_manual_mode_context_window")]
    pub manual_mode_context_window: u32,
}

const fn default_manual_mode_context_window() -> u32 {
    50
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            pure_mode: PureMode::Standard,
            analytics_enabled: true,
            update_checks_enabled: true,
            lorebook_generator: LorebookGeneratorSettings::default(),
            dynamic_memory: DynamicMemorySettings::default(),
            group_dynamic_memory: None,
            embedding: EmbeddingSettings::default(),
            manual_mode_context_window: default_manual_mode_context_window(),
        }
    }
}

impl GlobalSettings {
    #[must_use]
    pub fn effective_group_dynamic_memory(&self) -> &DynamicMemorySettings {
        self.group_dynamic_memory
            .as_ref()
            .unwrap_or(&self.dynamic_memory)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryRetrievalStrategy {
    Smart,
    Cosine,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EmbeddingSettings {
    pub dimensions: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryRunMode {
    Auto,
    AskFirst,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DynamicMemorySettings {
    pub enabled: bool,
    pub summary_message_interval: u32,
    pub run_mode: MemoryRunMode,
    pub max_entries: u32,
    pub min_similarity_basis_points: u16,
    pub retrieval_limit: u16,
    pub retrieval_strategy: MemoryRetrievalStrategy,
    pub hot_memory_token_budget: u32,
    pub cold_threshold_basis_points: u16,
    pub delete_confidence_basis_points: u16,
    pub max_hard_delete_ratio_basis_points: u16,
    pub duplicate_threshold_basis_points: u16,
    pub context_enrichment_enabled: bool,
    pub decay_rate_basis_points: u16,
    pub recursive_memory_loops: bool,
    pub recursive_memory_loop_hard_cap: u32,
}

impl Default for DynamicMemorySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            summary_message_interval: 20,
            run_mode: MemoryRunMode::Auto,
            max_entries: 50,
            min_similarity_basis_points: 3_500,
            retrieval_limit: 5,
            retrieval_strategy: MemoryRetrievalStrategy::Smart,
            hot_memory_token_budget: 2_000,
            cold_threshold_basis_points: 3_000,
            delete_confidence_basis_points: 5_000,
            max_hard_delete_ratio_basis_points: 5_000,
            duplicate_threshold_basis_points: 7_800,
            context_enrichment_enabled: true,
            decay_rate_basis_points: 800,
            recursive_memory_loops: false,
            recursive_memory_loop_hard_cap: 20,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LorebookGeneratorSettings {
    pub selection: LorebookGeneratorSelection,
    pub default_target_count: Option<u32>,
    pub max_output_tokens: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LorebookGeneratorSelection {
    pub model_profile_id: Option<ModelProfileId>,
    pub planner_prompt_id: Option<PromptDocumentId>,
    pub writer_prompt_id: Option<PromptDocumentId>,
    pub refine_prompt_id: Option<PromptDocumentId>,
    pub coherence_prompt_id: Option<PromptDocumentId>,
}

impl LorebookGeneratorSettings {
    #[must_use]
    pub fn target_count(&self) -> u32 {
        self.default_target_count
            .map(|value| value.clamp(5, 50))
            .unwrap_or(12)
    }

    #[must_use]
    pub fn output_tokens(&self) -> u32 {
        self.max_output_tokens
            .map(|value| value.clamp(256, 32768))
            .unwrap_or(4096)
    }

    #[must_use]
    pub fn select(
        &self,
        overrides: &LorebookGeneratorSelection,
        default_model: Option<ModelProfileId>,
        builtins: &LorebookGeneratorSelection,
    ) -> LorebookGeneratorSelection {
        LorebookGeneratorSelection {
            model_profile_id: overrides
                .model_profile_id
                .or(self.selection.model_profile_id)
                .or(default_model),
            planner_prompt_id: overrides
                .planner_prompt_id
                .or(self.selection.planner_prompt_id)
                .or(builtins.planner_prompt_id),
            writer_prompt_id: overrides
                .writer_prompt_id
                .or(self.selection.writer_prompt_id)
                .or(builtins.writer_prompt_id),
            refine_prompt_id: overrides
                .refine_prompt_id
                .or(self.selection.refine_prompt_id)
                .or(builtins.refine_prompt_id),
            coherence_prompt_id: overrides
                .coherence_prompt_id
                .or(self.selection.coherence_prompt_id)
                .or(builtins.coherence_prompt_id),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PureMode {
    Off,
    Standard,
    Strict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredGlobalSettings {
    pub settings: GlobalSettings,
    pub default_model_profile_id: Option<ModelProfileId>,
    pub default_prompt_document_id: Option<PromptDocumentId>,
    pub dynamic_memory_model_profile_id: Option<ModelProfileId>,
    pub group_speaker_model_profile_id: Option<ModelProfileId>,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GlobalSettingsStoreError {
    #[error("settings update has a stale revision")]
    StaleRevision,
    #[error("the selected model profile does not exist")]
    ModelProfileMissing,
    #[error("stored settings are invalid")]
    InvalidData,
    #[error("settings storage failed")]
    Storage,
}

pub trait GlobalSettingsStore: Send + Sync {
    fn load(&self) -> Result<StoredGlobalSettings, GlobalSettingsStoreError>;

    fn save(
        &self,
        settings: GlobalSettings,
        default_model_profile_id: Option<ModelProfileId>,
        expected_revision: Revision,
    ) -> Result<StoredGlobalSettings, GlobalSettingsStoreError>;

    fn set_dynamic_memory_model_profile(
        &self,
        model_profile_id: Option<ModelProfileId>,
        expected_revision: Revision,
    ) -> Result<StoredGlobalSettings, GlobalSettingsStoreError>;

    fn set_group_speaker_model_profile(
        &self,
        model_profile_id: Option<ModelProfileId>,
        expected_revision: Revision,
    ) -> Result<StoredGlobalSettings, GlobalSettingsStoreError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_memory_defaults_match_the_legacy_schema() {
        let settings = DynamicMemorySettings::default();
        assert!(!settings.enabled);
        assert_eq!(settings.summary_message_interval, 20);
        assert_eq!(settings.run_mode, MemoryRunMode::Auto);
        assert_eq!(settings.decay_rate_basis_points, 800);
        assert!(!settings.recursive_memory_loops);
        assert_eq!(settings.recursive_memory_loop_hard_cap, 20);
        let stored: DynamicMemorySettings =
            serde_json::from_str(r#"{"max_entries":12,"run_mode":"ask_first"}"#)
                .expect("partial settings document");
        assert_eq!(stored.max_entries, 12);
        assert_eq!(stored.run_mode, MemoryRunMode::AskFirst);
        assert_eq!(stored.summary_message_interval, 20);
    }

    #[test]
    fn legacy_generator_defaults_and_selection_precedence() {
        let legacy =
            r#"{"pure_mode":"standard","analytics_enabled":true,"update_checks_enabled":true}"#;
        let settings: GlobalSettings = serde_json::from_str(legacy).expect("old settings document");
        assert_eq!(settings.lorebook_generator.target_count(), 12);
        assert_eq!(settings.manual_mode_context_window, 50);
        assert_eq!(settings.lorebook_generator.output_tokens(), 4096);
        assert_eq!(settings.dynamic_memory, DynamicMemorySettings::default());
        assert_eq!(settings.group_dynamic_memory, None);
        assert_eq!(
            settings.effective_group_dynamic_memory(),
            &settings.dynamic_memory
        );
        let mut generator = settings.lorebook_generator;
        generator.default_target_count = Some(0);
        generator.max_output_tokens = Some(u32::MAX);
        assert_eq!(generator.target_count(), 5);
        assert_eq!(generator.output_tokens(), 32768);
        generator.default_target_count = Some(100);
        generator.max_output_tokens = Some(0);
        assert_eq!(generator.target_count(), 50);
        assert_eq!(generator.output_tokens(), 256);
        let default_model = ModelProfileId::new();
        let configured_model = ModelProfileId::new();
        let override_model = ModelProfileId::new();
        let builtin = PromptDocumentId::new();
        let configured = PromptDocumentId::new();
        let explicit = PromptDocumentId::new();
        let builtins = LorebookGeneratorSelection {
            planner_prompt_id: Some(builtin),
            ..Default::default()
        };
        assert_eq!(
            generator
                .select(&Default::default(), Some(default_model), &builtins)
                .model_profile_id,
            Some(default_model)
        );
        generator.selection.model_profile_id = Some(configured_model);
        generator.selection.planner_prompt_id = Some(configured);
        let selected = generator.select(&Default::default(), Some(default_model), &builtins);
        assert_eq!(selected.model_profile_id, Some(configured_model));
        assert_eq!(selected.planner_prompt_id, Some(configured));
        let selected = generator.select(
            &LorebookGeneratorSelection {
                model_profile_id: Some(override_model),
                planner_prompt_id: Some(explicit),
                ..Default::default()
            },
            Some(default_model),
            &builtins,
        );
        assert_eq!(selected.model_profile_id, Some(override_model));
        assert_eq!(selected.planner_prompt_id, Some(explicit));
    }

    #[test]
    fn settings_document_rejects_unknown_fields() {
        let value = r#"{"pure_mode":"standard","analytics_enabled":true,"update_checks_enabled":true,"api_key":"no"}"#;
        assert!(serde_json::from_str::<GlobalSettings>(value).is_err());
    }

    #[test]
    fn group_dynamic_memory_override_replaces_the_direct_policy() {
        let mut settings = GlobalSettings::default();
        settings.dynamic_memory.max_entries = 17;
        let mut group = settings.dynamic_memory.clone();
        group.max_entries = 29;
        group.retrieval_limit = 7;
        settings.group_dynamic_memory = Some(group.clone());

        assert_eq!(settings.effective_group_dynamic_memory(), &group);
    }
}
