use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingsGlobalSettings {
    pub pure_mode: SettingsPureMode,
    pub analytics_enabled: bool,
    pub update_checks_enabled: bool,
    pub developer_mode_enabled: bool,
    pub lorebook_generator: SettingsLorebookGeneratorSettings,
    pub dynamic_memory: SettingsDynamicMemorySettings,
    pub group_dynamic_memory: Option<SettingsDynamicMemorySettings>,
    pub dynamic_memory_prompts: SettingsDynamicMemoryPromptSelection,
    pub dynamic_memory_llama_sampler_overwrite_enabled: bool,
    pub help_me_reply: SettingsHelpMeReplySettings,
    pub embedding: SettingsEmbeddingSettings,
    pub image_generation: SettingsImageGenerationSettings,
    pub creation_helper: SettingsCreationHelperSettings,
    pub lorebook_entry_generator: SettingsLorebookEntryGeneratorSettings,
    pub companion_soul_writer: SettingsCompanionSoulWriterSettings,
    #[cfg_attr(feature = "specta", specta(type = std::collections::HashMap<String, specta_typescript::Unknown>))]
    pub ui_preferences: serde_json::Value,
    pub auto_download_character_card_avatars: bool,
    pub manual_mode_context_window: u32,
    pub lorebook_scan_depth: u8,
    pub llama_default_context_length: Option<u32>,
    pub llama_default_kv_cache_type: Option<SettingsLlamaDefaultKvCacheType>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsLlamaDefaultKvCacheType {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "f16")]
    F16,
    #[serde(rename = "q8_0")]
    Q8_0,
    #[serde(rename = "q4_0")]
    Q4_0,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SettingsPureMode {
    Off,
    Standard,
    Strict,

    Low,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SettingsSceneGenerationMode {
    Auto,
    AskFirst,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingsImageGenerationSettings {
    pub avatar_enabled: bool,
    pub avatar_model_profile_id: Option<String>,
    pub scene_enabled: bool,
    pub scene_mode: SettingsSceneGenerationMode,
    pub scene_model_profile_id: Option<String>,
    pub scene_writer_model_profile_id: Option<String>,
    pub creation_helper_model_profile_id: Option<String>,

    pub scene_default_size: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SettingsCreationHelperToolFallback {
    Native,
    Json,
    Xml,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingsCreationHelperSettings {
    pub model_profile_id: Option<String>,
    pub streaming: bool,
    pub enabled_tools: Option<Vec<String>>,
    pub tool_fallback: SettingsCreationHelperToolFallback,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingsLorebookEntryGeneratorSettings {
    pub model_profile_id: Option<String>,
    pub entry_prompt_id: Option<String>,
    pub keyword_prompt_id: Option<String>,
    pub structured_fallback_format: SettingsMemoryStructuredFallbackFormat,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingsCompanionSoulWriterSettings {
    pub model_profile_id: Option<String>,
    pub fallback_model_profile_id: Option<String>,
    pub prompt_id: Option<String>,
    pub structured_fallback_format: SettingsMemoryStructuredFallbackFormat,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SettingsMemoryRetrievalStrategy {
    Smart,
    Cosine,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingsEmbeddingSettings {
    pub dimensions: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SettingsMemoryRunMode {
    Auto,
    AskFirst,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingsDynamicMemoryPromptSelection {
    pub summarizer_prompt_id: Option<String>,
    pub manager_prompt_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SettingsHelpMeReplyStyle {
    Roleplay,
    Conversational,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingsHelpMeReplySettings {
    pub enabled: bool,
    pub model_profile_id: Option<String>,
    pub streaming: bool,
    pub max_output_tokens: u32,
    pub history_count: u32,
    pub style: SettingsHelpMeReplyStyle,
    pub roleplay_prompt_id: Option<String>,
    pub conversational_prompt_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SettingsMemoryStructuredFallbackFormat {
    Json,
    Xml,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingsDynamicMemorySettings {
    pub enabled: bool,
    pub summary_message_interval: u32,
    pub run_mode: SettingsMemoryRunMode,
    pub max_entries: u32,

    pub min_similarity_basis_points: Option<u16>,
    pub retrieval_limit: u16,
    pub retrieval_strategy: SettingsMemoryRetrievalStrategy,
    pub hot_memory_token_budget: u32,
    pub cold_threshold_basis_points: u16,
    pub delete_confidence_basis_points: u16,
    pub max_hard_delete_ratio_basis_points: u16,
    pub duplicate_threshold_basis_points: u16,
    pub context_enrichment_enabled: bool,
    pub decay_rate_basis_points: u16,
    pub recursive_memory_loops: bool,
    pub recursive_memory_loop_hard_cap: u32,
    pub structured_fallback_format: SettingsMemoryStructuredFallbackFormat,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingsLorebookGeneratorSettings {
    pub selection: SettingsLorebookGeneratorSelection,
    pub structured_fallback_format: SettingsMemoryStructuredFallbackFormat,
    pub default_target_count: Option<u32>,
    pub max_output_tokens: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingsLorebookGeneratorSelection {
    pub model_profile_id: Option<String>,
    pub planner_prompt_id: Option<String>,
    pub writer_prompt_id: Option<String>,
    pub refine_prompt_id: Option<String>,
    pub coherence_prompt_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingsChatParameterProfile {
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub top_k: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub context_length: Option<u32>,
    pub frequency_penalty: Option<f64>,
    pub presence_penalty: Option<f64>,
    pub repetition_penalty: Option<f64>,
    pub reasoning_mode: Option<SettingsReasoningMode>,
    pub reasoning_effort: Option<SettingsReasoningEffort>,
    pub reasoning_budget_tokens: Option<u32>,
    pub prompt_caching: Option<SettingsPromptCaching>,
    pub send_thinking_state: Option<bool>,
    pub ollama: SettingsOllamaOptions,
    pub openrouter: SettingsOpenRouterOptions,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingsOpenRouterOptions {
    pub pinned_provider: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingsOllamaOptions {
    pub num_ctx: Option<u32>,
    pub num_predict: Option<u32>,
    pub num_keep: Option<u32>,
    pub num_batch: Option<u32>,
    pub num_gpu: Option<u32>,
    pub num_thread: Option<u32>,
    pub tfs_z: Option<f64>,
    pub typical_p: Option<f64>,
    pub min_p: Option<f64>,
    pub mirostat: Option<u32>,
    pub mirostat_tau: Option<f64>,
    pub mirostat_eta: Option<f64>,
    pub seed: Option<u32>,
    pub stop: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SettingsReasoningMode {
    Disabled,
    Enabled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SettingsReasoningEffort {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SettingsPromptCaching {
    Disabled,
    Enabled {
        retention: SettingsPromptCacheRetention,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SettingsPromptCacheRetention {
    InMemory,
    FiveMinutes,
    OneHour,
    TwentyFourHours,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SettingsLlamaSamplerProfile {
    Balanced,
    Creative,
    Stable,
    Reasoning,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SettingsLlamaSamplerStage {
    Penalties,
    Grammar,
    TopK,
    TopP,
    MinP,
    Dry,
    Typical,
    Xtc,
    Temp,
    AdaptiveP,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingsLlamaSamplerSettings {
    pub profile: Option<SettingsLlamaSamplerProfile>,
    pub order: Option<Vec<SettingsLlamaSamplerStage>>,
    pub min_p: Option<f64>,
    pub typical_p: Option<f64>,
    pub repeat_penalty: Option<f64>,
    pub n_pen_range: Option<i32>,
    pub dry_multiplier: Option<f64>,
    pub dry_base: Option<f64>,
    pub dry_allowed_length: Option<u32>,
    pub dry_penalty_last_n: Option<i32>,
    pub dry_sequence_breakers: Option<Vec<String>>,
    pub xtc_probability: Option<f64>,
    pub xtc_threshold: Option<f64>,
    pub seed: Option<u32>,
    pub adaptive_target: Option<f64>,
    pub adaptive_decay: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SettingsLlamaGpuDistributionMode {
    Balanced,
    Proportional,
    Priority,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingsLlamaGpuLayerAssignment {
    pub device_id: u32,
    pub layers: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SettingsLlamaKvPlacement {
    Auto,
    Split,
    SystemRam,
    Pin,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum SettingsLlamaKvType {
    #[serde(rename = "f32")]
    F32,
    #[serde(rename = "f16")]
    F16,
    #[serde(rename = "q8_1")]
    Q81,
    #[serde(rename = "q8_0")]
    Q80,
    #[serde(rename = "q6_k")]
    Q6K,
    #[serde(rename = "q5_k")]
    Q5K,
    #[serde(rename = "q5_1")]
    Q51,
    #[serde(rename = "q5_0")]
    Q50,
    #[serde(rename = "q4_k")]
    Q4K,
    #[serde(rename = "q4_1")]
    Q41,
    #[serde(rename = "q4_0")]
    Q40,
    #[serde(rename = "q3_k")]
    Q3K,
    #[serde(rename = "q2_k")]
    Q2K,
    #[serde(rename = "iq4_nl")]
    Iq4Nl,
    #[serde(rename = "iq3_s")]
    Iq3S,
    #[serde(rename = "iq3_xxs")]
    Iq3Xxs,
    #[serde(rename = "iq2_xs")]
    Iq2Xs,
    #[serde(rename = "iq2_xxs")]
    Iq2Xxs,
    #[serde(rename = "iq1_s")]
    Iq1S,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SettingsLlamaFlashAttention {
    Auto,
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SettingsLlamaMtpPlacement {
    Auto,
    Gpu,
    Cpu,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingsLlamaCppSettings {
    pub gpu_layers: Option<u32>,
    pub multi_gpu_enabled: Option<bool>,
    pub gpu_device_ids: Option<Vec<u32>>,
    pub gpu_distribution_mode: Option<SettingsLlamaGpuDistributionMode>,
    pub gpu_manual_layers: Option<Vec<SettingsLlamaGpuLayerAssignment>>,
    pub cpu_layers: Option<u32>,
    pub kv_placement: Option<SettingsLlamaKvPlacement>,
    pub main_gpu: Option<u32>,
    pub single_gpu_device_id: Option<u32>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub priority_vram_limit_bytes: Option<u64>,
    pub threads: Option<u32>,
    pub threads_batch: Option<u32>,
    pub rope_freq_base: Option<f64>,
    pub rope_freq_scale: Option<f64>,
    pub offload_kqv: Option<bool>,
    pub batch_size: Option<u32>,
    pub ubatch_size: Option<u32>,
    pub kv_type: Option<SettingsLlamaKvType>,
    pub kv_type_k: Option<SettingsLlamaKvType>,
    pub kv_type_v: Option<SettingsLlamaKvType>,
    pub flash_attention: Option<SettingsLlamaFlashAttention>,
    pub swa_full: Option<bool>,
    pub chat_template_override: Option<String>,
    pub chat_template_preset: Option<String>,
    pub mmproj_path: Option<String>,
    pub raw_completion_fallback: Option<bool>,
    pub strict_mode: Option<bool>,
    pub mtp_enabled: Option<bool>,
    pub mtp_placement: Option<SettingsLlamaMtpPlacement>,
    pub mtp_draft_tokens: Option<u32>,
    pub mtp_model_path: Option<String>,
    pub dflash_enabled: Option<bool>,
    pub dflash_draft_tokens: Option<u32>,
    pub dflash_min_probability: Option<f64>,
    pub dflash_model_path: Option<String>,
    pub streaming_enabled: Option<bool>,
    pub force_gemma4_reasoning: Option<bool>,
    pub sampler: SettingsLlamaSamplerSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SettingsStableDiffusionCacheMode {
    Disabled,
    Easycache,
    Ucache,
    Dbcache,
    Taylorseer,
    CacheDit,
    Spectrum,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SettingsStableDiffusionOffloadMode {
    Auto,
    Gpu,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingsStableDiffusionLora {
    pub path: String,
    pub multiplier: f64,
    pub is_high_noise: bool,
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingsStableDiffusionCppBinding {
    pub profile_id: Option<String>,
    pub variant_id: Option<String>,
    pub text_encoder_path: Option<String>,
    pub vae_path: Option<String>,
    pub vision_encoder_path: Option<String>,
    pub runtime_release: Option<String>,
    pub runtime_asset: Option<String>,
    pub runtime_backend: Option<String>,
    pub max_reference_images: Option<u32>,
    pub supports_lora: Option<bool>,
    pub supports_text_to_image: Option<bool>,
    pub supports_image_edit: Option<bool>,
    pub recommended_for_scenes: Option<bool>,
    pub requires_reference_image: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingsStableDiffusionSettings {
    pub steps: Option<u32>,
    pub cfg_scale: Option<f64>,
    pub sampler: Option<String>,
    pub scheduler: Option<String>,
    pub seed: Option<u32>,
    pub negative_prompt: Option<String>,
    pub denoising_strength: Option<f64>,
    pub image_cfg_scale: Option<f64>,
    pub distilled_guidance: Option<f64>,
    pub eta: Option<f64>,
    pub flow_shift: Option<f64>,
    pub size: Option<String>,
    pub vae_tiling_enabled: Option<bool>,
    pub vae_tile_size_x: Option<u32>,
    pub vae_tile_size_y: Option<u32>,
    pub vae_tile_overlap: Option<f64>,
    pub auto_resize_reference_images: Option<bool>,
    pub increase_reference_index: Option<bool>,
    pub hires_enabled: Option<bool>,
    pub hires_upscaler: Option<String>,
    pub hires_scale: Option<f64>,
    pub hires_width: Option<u32>,
    pub hires_height: Option<u32>,
    pub hires_steps: Option<u32>,
    pub hires_denoising_strength: Option<f64>,
    pub slg_scale: Option<f64>,
    pub slg_layers: Option<String>,
    pub slg_layer_start: Option<f64>,
    pub slg_layer_end: Option<f64>,
    pub cache_mode: Option<SettingsStableDiffusionCacheMode>,
    pub cache_option: Option<String>,
    pub offload_mode: Option<SettingsStableDiffusionOffloadMode>,
    pub extra_prompt: Option<String>,
    pub prompt_writer_instructions: Option<String>,
    pub base_loras: Option<Vec<SettingsStableDiffusionLora>>,
    pub cpp: SettingsStableDiffusionCppBinding,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingsModelSettingsLayer {
    pub chat_parameters: SettingsChatParameterProfile,
    pub llama_cpp: SettingsLlamaCppSettings,
    pub stable_diffusion: SettingsStableDiffusionSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingsView {
    pub device: SettingsDeviceView,
    pub global: SettingsGlobalSettings,
    pub sampler_defaults: SettingsModelSettingsLayer,
    pub default_model_profile_id: Option<String>,
    pub default_prompt_document_id: Option<String>,
    pub dynamic_memory_model_profile_id: Option<String>,
    pub group_speaker_model_profile_id: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "section", rename_all = "snake_case", deny_unknown_fields)]
pub enum SettingsPatch {
    General {
        pure_mode: Option<SettingsPureMode>,
        analytics_enabled: Option<bool>,
        update_checks_enabled: Option<bool>,
        developer_mode_enabled: Option<bool>,
        auto_download_character_card_avatars: Option<bool>,
        manual_mode_context_window: Option<u32>,
        lorebook_scan_depth: Option<u8>,
    },
    DynamicMemory {
        value: SettingsDynamicMemorySettings,
    },
    GroupDynamicMemory {
        value: Option<SettingsDynamicMemorySettings>,
    },
    DynamicMemoryPrompts {
        value: SettingsDynamicMemoryPromptSelection,
    },
    DynamicMemorySampler {
        enabled: bool,
    },
    HelpMeReply {
        value: SettingsHelpMeReplySettings,
    },
    LorebookGenerator {
        value: SettingsLorebookGeneratorSettings,
    },
    LorebookEntryGenerator {
        value: SettingsLorebookEntryGeneratorSettings,
    },
    CompanionSoulWriter {
        value: SettingsCompanionSoulWriterSettings,
    },
    ImageGeneration {
        value: SettingsImageGenerationSettings,
    },
    Embedding {
        value: SettingsEmbeddingSettings,
    },
    DeviceEmbedding {
        model_version: Option<SettingsEmbeddingVersion>,
        max_tokens: Option<u16>,
        keep_model_loaded: bool,
    },
    LocalRuntime {
        context_length: Option<u32>,
        kv_cache_type: Option<SettingsLlamaDefaultKvCacheType>,
    },
    UiPreferences {
        changes: Vec<crate::UiPreferenceChange>,
    },
    Selections {
        default_model: Option<crate::IdChange>,
        default_prompt: Option<crate::IdChange>,
        dynamic_memory_model: Option<crate::IdChange>,
        group_speaker_model: Option<crate::IdChange>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingsUpdateRequest {
    pub patch: SettingsPatch,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingsSamplerDefaultsUpdateRequest {
    pub value: SettingsModelSettingsLayer,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SettingsDeviceView {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub revision: u64,
    pub embedding_model_version: Option<SettingsEmbeddingVersion>,
    pub embedding_max_tokens: Option<u16>,
    pub embedding_keep_model_loaded: bool,
    pub llm_models_dir: Option<String>,
    pub dictation_model_id: Option<String>,
    pub trusted_certificates: Vec<crate::TrustedCertificateView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SettingsEmbeddingVersion {
    V3,
    V4,
    V5,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ContentFilterLogView {
    pub entries: Vec<ContentFilterLogEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ContentFilterLogEntry {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub timestamp_ms: u64,
    pub text_snippet: String,
    pub score: f32,
    pub blocked: bool,
    pub matched_terms: Vec<String>,
    pub level: SettingsPureMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SettingsFailureReason {
    InvalidData,
    StaleRevision,
    ModelProfileMissing,
    Storage,
    DeveloperModeRequired,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "specta", specta(transparent))]
pub struct SettingsCommandInput<T>(
    #[cfg_attr(feature = "specta", specta(type = T))] Result<T, String>,
);

impl<T> From<T> for SettingsCommandInput<T> {
    fn from(value: T) -> Self {
        Self(Ok(value))
    }
}

impl<T> SettingsCommandInput<T> {
    pub fn into_result(self) -> Result<T, String> {
        self.0
    }
}

impl<'de, T: serde::de::DeserializeOwned> Deserialize<'de> for SettingsCommandInput<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        Ok(Self(
            serde_json::from_value(value).map_err(|error| error.to_string()),
        ))
    }
}

impl<T: Serialize> Serialize for SettingsCommandInput<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0
            .as_ref()
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}
