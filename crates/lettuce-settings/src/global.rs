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
    pub dynamic_memory_prompts: DynamicMemoryPromptSelection,
    #[serde(default = "default_true")]
    pub dynamic_memory_llama_sampler_overwrite_enabled: bool,
    #[serde(default)]
    pub help_me_reply: HelpMeReplySettings,
    #[serde(default)]
    pub embedding: EmbeddingSettings,
    #[serde(default)]
    pub image_generation: ImageGenerationSettings,
    #[serde(default)]
    pub creation_helper: CreationHelperSettings,
    #[serde(default)]
    pub lorebook_entry_generator: LorebookEntryGeneratorSettings,
    #[serde(default)]
    pub companion_soul_writer: CompanionSoulWriterSettings,
    #[serde(default, skip_serializing_if = "UiPreferences::is_empty")]
    pub ui_preferences: UiPreferences,
    /// Legacy `autoDownloadCharacterCardAvatars`: fetch http(s) avatars a
    /// character card links to while importing it.
    #[serde(default = "default_true")]
    pub auto_download_character_card_avatars: bool,
    #[serde(default = "default_manual_mode_context_window")]
    pub manual_mode_context_window: u32,
}

const fn default_manual_mode_context_window() -> u32 {
    50
}

const fn default_true() -> bool {
    true
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
            dynamic_memory_prompts: DynamicMemoryPromptSelection::default(),
            dynamic_memory_llama_sampler_overwrite_enabled: true,
            help_me_reply: HelpMeReplySettings::default(),
            embedding: EmbeddingSettings::default(),
            image_generation: ImageGenerationSettings::default(),
            creation_helper: CreationHelperSettings::default(),
            lorebook_entry_generator: LorebookEntryGeneratorSettings::default(),
            companion_soul_writer: CompanionSoulWriterSettings::default(),
            ui_preferences: UiPreferences::default(),
            auto_download_character_card_avatars: true,
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

    /// Every model profile the settings payload selects.
    #[must_use]
    pub fn selected_model_profiles(&self) -> [Option<ModelProfileId>; 10] {
        let image = &self.image_generation;
        [
            self.lorebook_generator.selection.model_profile_id,
            self.help_me_reply.model_profile_id,
            image.avatar_model_profile_id,
            image.scene_model_profile_id,
            image.scene_writer_model_profile_id,
            image.creation_helper_model_profile_id,
            self.creation_helper.model_profile_id,
            self.lorebook_entry_generator.model_profile_id,
            self.companion_soul_writer.model_profile_id,
            self.companion_soul_writer.fallback_model_profile_id,
        ]
    }

    /// Unselects every profile `removed` accepts; true when one was selected.
    pub fn clear_model_profiles(&mut self, removed: impl Fn(ModelProfileId) -> bool) -> bool {
        let image = &mut self.image_generation;
        let mut changed = false;
        for selection in [
            &mut self.lorebook_generator.selection.model_profile_id,
            &mut self.help_me_reply.model_profile_id,
            &mut image.avatar_model_profile_id,
            &mut image.scene_model_profile_id,
            &mut image.scene_writer_model_profile_id,
            &mut image.creation_helper_model_profile_id,
            &mut self.creation_helper.model_profile_id,
            &mut self.lorebook_entry_generator.model_profile_id,
            &mut self.companion_soul_writer.model_profile_id,
            &mut self.companion_soul_writer.fallback_model_profile_id,
        ] {
            if selection.is_some_and(&removed) {
                *selection = None;
                changed = true;
            }
        }
        changed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneGenerationMode {
    #[default]
    Auto,
    AskFirst,
    Manual,
}

/// Legacy `avatarGeneration*`, `sceneGeneration*`, `sceneWriterModelId` and
/// `creationHelperImageModelId` advanced settings; unset models mean the first
/// suitable model, as legacy chose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ImageGenerationSettings {
    pub avatar_enabled: bool,
    pub avatar_model_profile_id: Option<ModelProfileId>,
    pub scene_enabled: bool,
    pub scene_mode: SceneGenerationMode,
    pub scene_model_profile_id: Option<ModelProfileId>,
    pub scene_writer_model_profile_id: Option<ModelProfileId>,
    pub creation_helper_model_profile_id: Option<ModelProfileId>,
    /// Legacy `sdDefaultSize`: the scene image size when the scene model sets
    /// none; unset means 1024x1024.
    pub scene_default_size: Option<String>,
}

impl Default for ImageGenerationSettings {
    fn default() -> Self {
        Self {
            avatar_enabled: true,
            avatar_model_profile_id: None,
            scene_enabled: false,
            scene_mode: SceneGenerationMode::Auto,
            scene_model_profile_id: None,
            scene_writer_model_profile_id: None,
            creation_helper_model_profile_id: None,
            scene_default_size: None,
        }
    }
}

/// The largest serialized [`UiPreferences`] document.
pub const MAX_UI_PREFERENCES_BYTES: usize = 256 * 1024;

/// Preferences only the app shell reads (theme, colors, view modes,
/// navigation, sounds and haptics, the base chat appearance, saved sampler
/// orders), kept as the shell's own JSON document under legacy's key names.
/// The backend stores, syncs and bounds it but never interprets it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UiPreferences(pub serde_json::Map<String, serde_json::Value>);

impl UiPreferences {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn within_bounds(&self) -> bool {
        serde_json::to_vec(&self.0).is_ok_and(|bytes| bytes.len() <= MAX_UI_PREFERENCES_BYTES)
    }
}

/// How the creation helper asks for tool calls when a model has no native
/// tool calling (legacy `creationHelperToolFallback`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreationHelperToolFallback {
    #[default]
    Native,
    Json,
    Xml,
}

/// Legacy `creationHelper*` advanced settings: the chat model (unset means
/// the default model), streaming, the tools it may call (unset means all)
/// and its tool-call fallback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CreationHelperSettings {
    pub model_profile_id: Option<ModelProfileId>,
    pub streaming: bool,
    pub enabled_tools: Option<Vec<String>>,
    pub tool_fallback: CreationHelperToolFallback,
}

impl Default for CreationHelperSettings {
    fn default() -> Self {
        Self {
            model_profile_id: None,
            streaming: true,
            enabled_tools: None,
            tool_fallback: CreationHelperToolFallback::Native,
        }
    }
}

/// Legacy `lorebookEntryGenerator*` and
/// `lorebookKeywordGeneratorPromptTemplateId` advanced settings: the model
/// (unset means the first text model), the entry and keyword prompts (unset
/// means the built-in ones) and the structured fallback format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LorebookEntryGeneratorSettings {
    pub model_profile_id: Option<ModelProfileId>,
    pub entry_prompt_id: Option<PromptDocumentId>,
    pub keyword_prompt_id: Option<PromptDocumentId>,
    pub structured_fallback_format: MemoryStructuredFallbackFormat,
}

impl Default for LorebookEntryGeneratorSettings {
    fn default() -> Self {
        Self {
            model_profile_id: None,
            entry_prompt_id: None,
            keyword_prompt_id: None,
            structured_fallback_format: MemoryStructuredFallbackFormat::Json,
        }
    }
}

/// Legacy `companionSoulWriter*` advanced settings: the model (unset means
/// the default model, then the first text model), its fallback model, the
/// prompt (unset means the built-in one) and the structured fallback format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CompanionSoulWriterSettings {
    pub model_profile_id: Option<ModelProfileId>,
    pub fallback_model_profile_id: Option<ModelProfileId>,
    pub prompt_id: Option<PromptDocumentId>,
    pub structured_fallback_format: MemoryStructuredFallbackFormat,
}

impl Default for CompanionSoulWriterSettings {
    fn default() -> Self {
        Self {
            model_profile_id: None,
            fallback_model_profile_id: None,
            prompt_id: None,
            structured_fallback_format: MemoryStructuredFallbackFormat::Json,
        }
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

/// The user's dynamic-memory prompt overrides (legacy
/// `dynamicMemorySummarizerPromptTemplateId` /
/// `dynamicMemoryManagerPromptTemplateId`), shared by direct and group chats;
/// unset means the built-in document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DynamicMemoryPromptSelection {
    pub summarizer_prompt_id: Option<PromptDocumentId>,
    pub manager_prompt_id: Option<PromptDocumentId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HelpMeReplyStyle {
    Roleplay,
    Conversational,
}

/// Legacy `helpMeReply*` advanced settings: the feature toggle, its model,
/// streaming, output cap, history window, style and the per-style prompt
/// overrides; unset ids mean the default model and the built-in documents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HelpMeReplySettings {
    pub enabled: bool,
    pub model_profile_id: Option<ModelProfileId>,
    pub streaming: bool,
    pub max_output_tokens: u32,
    pub history_count: u32,
    pub style: HelpMeReplyStyle,
    pub roleplay_prompt_id: Option<PromptDocumentId>,
    pub conversational_prompt_id: Option<PromptDocumentId>,
}

impl Default for HelpMeReplySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            model_profile_id: None,
            streaming: true,
            max_output_tokens: 150,
            history_count: 10,
            style: HelpMeReplyStyle::Roleplay,
            roleplay_prompt_id: None,
            conversational_prompt_id: None,
        }
    }
}

impl HelpMeReplySettings {
    /// Legacy ignored a zero history count and used ten messages.
    #[must_use]
    pub fn history_count(&self) -> usize {
        if self.history_count == 0 {
            10
        } else {
            self.history_count as usize
        }
    }
}

/// The document format the dynamic-memory cycle asks for when a model cannot
/// call tools; legacy defaulted to XML.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryStructuredFallbackFormat {
    Json,
    Xml,
}

/// The minimum raw-cosine similarity when none is set.
pub const DEFAULT_MIN_SIMILARITY_BASIS_POINTS: u16 = 3_500;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DynamicMemorySettings {
    pub enabled: bool,
    pub summary_message_interval: u32,
    pub run_mode: MemoryRunMode,
    pub max_entries: u32,
    /// The minimum shown similarity a retrieved memory needs. `None` means
    /// the embedding model's own default: the thresholds a calibrated model
    /// publishes, or `DEFAULT_MIN_SIMILARITY_BASIS_POINTS` on raw cosine.
    pub min_similarity_basis_points: Option<u16>,
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
    pub structured_fallback_format: MemoryStructuredFallbackFormat,
}

impl Default for DynamicMemorySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            summary_message_interval: 20,
            run_mode: MemoryRunMode::Auto,
            max_entries: 50,
            min_similarity_basis_points: None,
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
            structured_fallback_format: MemoryStructuredFallbackFormat::Xml,
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
    /// Blocks explicit sexual content and slurs only.
    Low,
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

/// The app shell's state about this install (legacy onboarding progress,
/// dismissed hints, the last version seen, active-usage counters): a JSON
/// object under legacy's key names that never syncs or enters backups.
pub trait DeviceUiStateStore: Send + Sync {
    fn load_device_ui_state(
        &self,
    ) -> Result<serde_json::Map<String, serde_json::Value>, GlobalSettingsStoreError>;

    fn save_device_ui_state(
        &self,
        state: serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), GlobalSettingsStoreError>;
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
    fn unset_min_similarity_round_trips_apart_from_a_set_one() {
        let unset = GlobalSettings::default();
        assert_eq!(unset.dynamic_memory.min_similarity_basis_points, None);
        let json = serde_json::to_string(&unset).expect("json");
        let restored: GlobalSettings = serde_json::from_str(&json).expect("restored");
        assert_eq!(restored.dynamic_memory.min_similarity_basis_points, None);
        let mut set = GlobalSettings::default();
        set.dynamic_memory.min_similarity_basis_points = Some(3_500);
        let restored: GlobalSettings =
            serde_json::from_str(&serde_json::to_string(&set).expect("json")).expect("restored");
        assert_eq!(
            restored.dynamic_memory.min_similarity_basis_points,
            Some(3_500)
        );
        let without_key: DynamicMemorySettings =
            serde_json::from_str(r#"{"max_entries": 60}"#).expect("older payload");
        assert_eq!(without_key.min_similarity_basis_points, None);
    }

    #[test]
    fn image_generation_defaults_match_the_legacy_schema_and_older_payloads() {
        let defaults = ImageGenerationSettings::default();
        assert!(defaults.avatar_enabled && !defaults.scene_enabled);
        assert_eq!(defaults.scene_mode, SceneGenerationMode::Auto);
        let mut older = serde_json::to_value(GlobalSettings::default()).expect("settings");
        older
            .as_object_mut()
            .expect("object")
            .remove("image_generation");
        let older: GlobalSettings = serde_json::from_value(older).expect("older payload");
        assert_eq!(older.image_generation, defaults);
        let mut settings = GlobalSettings::default();
        let kept = ModelProfileId::new();
        let removed = ModelProfileId::new();
        settings.image_generation.scene_model_profile_id = Some(removed);
        settings.image_generation.scene_writer_model_profile_id = Some(kept);
        settings.help_me_reply.model_profile_id = Some(removed);
        assert!(settings.clear_model_profiles(|id| id == removed));
        assert_eq!(
            settings
                .selected_model_profiles()
                .into_iter()
                .flatten()
                .collect::<Vec<_>>(),
            vec![kept]
        );
        assert!(!settings.clear_model_profiles(|id| id == removed));
    }

    #[test]
    fn dynamic_memory_defaults_match_the_legacy_schema() {
        let settings = DynamicMemorySettings::default();
        assert!(!settings.enabled);
        assert_eq!(settings.summary_message_interval, 20);
        assert_eq!(settings.run_mode, MemoryRunMode::Auto);
        assert_eq!(settings.decay_rate_basis_points, 800);
        assert!(!settings.recursive_memory_loops);
        assert_eq!(settings.recursive_memory_loop_hard_cap, 20);
        assert_eq!(
            settings.structured_fallback_format,
            MemoryStructuredFallbackFormat::Xml
        );
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
            settings.dynamic_memory_prompts,
            DynamicMemoryPromptSelection::default()
        );
        assert!(settings.dynamic_memory_llama_sampler_overwrite_enabled);
        assert_eq!(settings.help_me_reply, HelpMeReplySettings::default());
        assert!(settings.help_me_reply.enabled && settings.help_me_reply.streaming);
        assert_eq!(settings.help_me_reply.max_output_tokens, 150);
        assert_eq!(settings.help_me_reply.history_count(), 10);
        assert_eq!(settings.help_me_reply.style, HelpMeReplyStyle::Roleplay);
        assert_eq!(
            HelpMeReplySettings {
                history_count: 0,
                ..HelpMeReplySettings::default()
            }
            .history_count(),
            10
        );
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
