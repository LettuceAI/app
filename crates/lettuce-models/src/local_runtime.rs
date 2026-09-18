use serde::{Deserialize, Serialize};

use crate::{ChatParameterOverrides, ParameterValidationError};

const SEED_MAX: u32 = i32::MAX as u32;

fn invalid(field: &'static str) -> ParameterValidationError {
    ParameterValidationError::InvalidValue(field)
}

fn check_f64(
    field: &'static str,
    value: Option<f64>,
    min: f64,
    max: f64,
) -> Result<(), ParameterValidationError> {
    match value {
        Some(value) if !value.is_finite() || value < min || value > max => Err(invalid(field)),
        _ => Ok(()),
    }
}

fn check_u32(
    field: &'static str,
    value: Option<u32>,
    min: u32,
    max: u32,
) -> Result<(), ParameterValidationError> {
    match value {
        Some(value) if value < min || value > max => Err(invalid(field)),
        _ => Ok(()),
    }
}

fn check_i32(
    field: &'static str,
    value: Option<i32>,
    min: i32,
    max: i32,
) -> Result<(), ParameterValidationError> {
    match value {
        Some(value) if value < min || value > max => Err(invalid(field)),
        _ => Ok(()),
    }
}

fn check_text(field: &'static str, value: Option<&str>) -> Result<(), ParameterValidationError> {
    match value {
        Some(value) if value.trim().is_empty() || value.trim() != value => Err(invalid(field)),
        _ => Ok(()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlamaSamplerProfile {
    Balanced,
    Creative,
    Stable,
    Reasoning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlamaSamplerStage {
    Penalties,
    Grammar,
    TopK,
    TopP,
    MinP,
    Dry,
    Typical,
    Xtc,
    Temp,
}

/// llama.cpp sampler settings. Every field is optional; an unset field leaves
/// the sampler profile default in place.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LlamaSamplerSettings {
    #[serde(default)]
    pub profile: Option<LlamaSamplerProfile>,
    #[serde(default)]
    pub order: Option<Vec<LlamaSamplerStage>>,
    #[serde(default)]
    pub min_p: Option<f64>,
    #[serde(default)]
    pub typical_p: Option<f64>,
    #[serde(default)]
    pub repeat_penalty: Option<f64>,
    #[serde(default)]
    pub n_pen_range: Option<i32>,
    #[serde(default)]
    pub dry_multiplier: Option<f64>,
    #[serde(default)]
    pub dry_base: Option<f64>,
    #[serde(default)]
    pub dry_allowed_length: Option<u32>,
    #[serde(default)]
    pub dry_penalty_last_n: Option<i32>,
    #[serde(default)]
    pub dry_sequence_breakers: Option<Vec<String>>,
    #[serde(default)]
    pub xtc_probability: Option<f64>,
    #[serde(default)]
    pub xtc_threshold: Option<f64>,
    #[serde(default)]
    pub seed: Option<u32>,
}

impl LlamaSamplerSettings {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    pub fn validate(&self) -> Result<(), ParameterValidationError> {
        check_f64("llama_min_p", self.min_p, 0.0, 1.0)?;
        check_f64("llama_typical_p", self.typical_p, 0.0, 1.0)?;
        check_f64("llama_repeat_penalty", self.repeat_penalty, 0.0, 2.0)?;
        check_i32("llama_n_pen_range", self.n_pen_range, -1, 262_144)?;
        check_f64("llama_dry_multiplier", self.dry_multiplier, 0.0, 10.0)?;
        check_f64("llama_dry_base", self.dry_base, 0.0, 10.0)?;
        check_u32("llama_dry_allowed_length", self.dry_allowed_length, 0, 128)?;
        check_i32(
            "llama_dry_penalty_last_n",
            self.dry_penalty_last_n,
            -1,
            262_144,
        )?;
        check_f64("llama_xtc_probability", self.xtc_probability, 0.0, 1.0)?;
        check_f64("llama_xtc_threshold", self.xtc_threshold, 0.0, 1.0)?;
        check_u32("llama_seed", self.seed, 0, SEED_MAX)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlamaGpuDistributionMode {
    Balanced,
    Proportional,
    Priority,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LlamaGpuLayerAssignment {
    pub device_id: u32,
    pub layers: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlamaKvPlacement {
    Auto,
    Split,
    SystemRam,
    Pin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LlamaKvType {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlamaFlashAttention {
    Auto,
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlamaMtpPlacement {
    Auto,
    Gpu,
    Cpu,
}

/// Per-model llama.cpp load, placement and template settings, owned by the
/// model editor. Runtime sizing formulas consume them unchanged.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LlamaCppSettings {
    #[serde(default)]
    pub gpu_layers: Option<u32>,
    #[serde(default)]
    pub multi_gpu_enabled: Option<bool>,
    #[serde(default)]
    pub gpu_device_ids: Option<Vec<u32>>,
    #[serde(default)]
    pub gpu_distribution_mode: Option<LlamaGpuDistributionMode>,
    #[serde(default)]
    pub gpu_manual_layers: Option<Vec<LlamaGpuLayerAssignment>>,
    #[serde(default)]
    pub cpu_layers: Option<u32>,
    #[serde(default)]
    pub kv_placement: Option<LlamaKvPlacement>,
    #[serde(default)]
    pub main_gpu: Option<u32>,
    #[serde(default)]
    pub single_gpu_device_id: Option<u32>,
    #[serde(default)]
    pub priority_vram_limit_bytes: Option<u64>,
    #[serde(default)]
    pub threads: Option<u32>,
    #[serde(default)]
    pub threads_batch: Option<u32>,
    #[serde(default)]
    pub rope_freq_base: Option<f64>,
    #[serde(default)]
    pub rope_freq_scale: Option<f64>,
    #[serde(default)]
    pub offload_kqv: Option<bool>,
    #[serde(default)]
    pub batch_size: Option<u32>,
    #[serde(default)]
    pub ubatch_size: Option<u32>,
    #[serde(default)]
    pub kv_type: Option<LlamaKvType>,
    #[serde(default)]
    pub flash_attention: Option<LlamaFlashAttention>,
    #[serde(default)]
    pub swa_full: Option<bool>,
    #[serde(default)]
    pub chat_template_override: Option<String>,
    #[serde(default)]
    pub chat_template_preset: Option<String>,
    #[serde(default)]
    pub mmproj_path: Option<String>,
    #[serde(default)]
    pub raw_completion_fallback: Option<bool>,
    #[serde(default)]
    pub strict_mode: Option<bool>,
    #[serde(default)]
    pub mtp_enabled: Option<bool>,
    #[serde(default)]
    pub mtp_placement: Option<LlamaMtpPlacement>,
    #[serde(default)]
    pub mtp_draft_tokens: Option<u32>,
    #[serde(default)]
    pub mtp_model_path: Option<String>,
    #[serde(default)]
    pub streaming_enabled: Option<bool>,
    #[serde(default)]
    pub sampler: LlamaSamplerSettings,
}

impl LlamaCppSettings {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    pub fn validate(&self) -> Result<(), ParameterValidationError> {
        check_u32("llama_gpu_layers", self.gpu_layers, 0, 512)?;
        if self
            .gpu_manual_layers
            .iter()
            .flatten()
            .any(|assignment| assignment.layers > 512)
        {
            return Err(invalid("llama_gpu_manual_layers"));
        }
        check_u32("llama_cpu_layers", self.cpu_layers, 0, 512)?;
        check_u32("llama_threads", self.threads, 1, 256)?;
        check_u32("llama_threads_batch", self.threads_batch, 1, 256)?;
        check_f64(
            "llama_rope_freq_base",
            self.rope_freq_base,
            0.0,
            1_000_000.0,
        )?;
        check_f64("llama_rope_freq_scale", self.rope_freq_scale, 0.0, 10.0)?;
        check_u32("llama_batch_size", self.batch_size, 1, 8192)?;
        check_u32("llama_ubatch_size", self.ubatch_size, 1, 8192)?;
        check_u32("llama_mtp_draft_tokens", self.mtp_draft_tokens, 1, 8)?;
        check_text(
            "llama_chat_template_override",
            self.chat_template_override.as_deref(),
        )?;
        check_text(
            "llama_chat_template_preset",
            self.chat_template_preset.as_deref(),
        )?;
        check_text("llama_mmproj_path", self.mmproj_path.as_deref())?;
        check_text("llama_mtp_model_path", self.mtp_model_path.as_deref())?;
        self.sampler.validate()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StableDiffusionCacheMode {
    Disabled,
    Easycache,
    Ucache,
    Dbcache,
    Taylorseer,
    CacheDit,
    Spectrum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StableDiffusionOffloadMode {
    Auto,
    Gpu,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StableDiffusionLora {
    pub path: String,
    pub multiplier: f64,
    #[serde(default)]
    pub is_high_noise: bool,
    #[serde(default)]
    pub keywords: Vec<String>,
}

/// The stable-diffusion.cpp model binding the model catalog installed.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StableDiffusionCppBinding {
    #[serde(default)]
    pub profile_id: Option<String>,
    #[serde(default)]
    pub variant_id: Option<String>,
    #[serde(default)]
    pub text_encoder_path: Option<String>,
    #[serde(default)]
    pub vae_path: Option<String>,
    #[serde(default)]
    pub vision_encoder_path: Option<String>,
    #[serde(default)]
    pub runtime_release: Option<String>,
    #[serde(default)]
    pub runtime_asset: Option<String>,
    #[serde(default)]
    pub runtime_backend: Option<String>,
    #[serde(default)]
    pub max_reference_images: Option<u32>,
    #[serde(default)]
    pub supports_lora: Option<bool>,
    #[serde(default)]
    pub supports_text_to_image: Option<bool>,
    #[serde(default)]
    pub supports_image_edit: Option<bool>,
    #[serde(default)]
    pub recommended_for_scenes: Option<bool>,
    #[serde(default)]
    pub requires_reference_image: Option<bool>,
}

/// Per-model image generation defaults and the stable-diffusion.cpp binding.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StableDiffusionSettings {
    #[serde(default)]
    pub steps: Option<u32>,
    #[serde(default)]
    pub cfg_scale: Option<f64>,
    #[serde(default)]
    pub sampler: Option<String>,
    #[serde(default)]
    pub scheduler: Option<String>,
    #[serde(default)]
    pub seed: Option<u32>,
    #[serde(default)]
    pub negative_prompt: Option<String>,
    #[serde(default)]
    pub denoising_strength: Option<f64>,
    #[serde(default)]
    pub image_cfg_scale: Option<f64>,
    #[serde(default)]
    pub distilled_guidance: Option<f64>,
    #[serde(default)]
    pub eta: Option<f64>,
    #[serde(default)]
    pub flow_shift: Option<f64>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub vae_tiling_enabled: Option<bool>,
    #[serde(default)]
    pub vae_tile_size_x: Option<u32>,
    #[serde(default)]
    pub vae_tile_size_y: Option<u32>,
    #[serde(default)]
    pub vae_tile_overlap: Option<f64>,
    #[serde(default)]
    pub auto_resize_reference_images: Option<bool>,
    #[serde(default)]
    pub increase_reference_index: Option<bool>,
    #[serde(default)]
    pub hires_enabled: Option<bool>,
    #[serde(default)]
    pub hires_upscaler: Option<String>,
    #[serde(default)]
    pub hires_scale: Option<f64>,
    #[serde(default)]
    pub hires_width: Option<u32>,
    #[serde(default)]
    pub hires_height: Option<u32>,
    #[serde(default)]
    pub hires_steps: Option<u32>,
    #[serde(default)]
    pub hires_denoising_strength: Option<f64>,
    #[serde(default)]
    pub slg_scale: Option<f64>,
    #[serde(default)]
    pub slg_layers: Option<String>,
    #[serde(default)]
    pub slg_layer_start: Option<f64>,
    #[serde(default)]
    pub slg_layer_end: Option<f64>,
    #[serde(default)]
    pub cache_mode: Option<StableDiffusionCacheMode>,
    #[serde(default)]
    pub cache_option: Option<String>,
    #[serde(default)]
    pub offload_mode: Option<StableDiffusionOffloadMode>,
    #[serde(default)]
    pub extra_prompt: Option<String>,
    #[serde(default)]
    pub prompt_writer_instructions: Option<String>,
    #[serde(default)]
    pub base_loras: Option<Vec<StableDiffusionLora>>,
    #[serde(default)]
    pub cpp: StableDiffusionCppBinding,
}

impl StableDiffusionSettings {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    pub fn validate(&self) -> Result<(), ParameterValidationError> {
        check_u32("sd_steps", self.steps, 1, 150)?;
        check_f64("sd_cfg_scale", self.cfg_scale, 0.0, 30.0)?;
        check_u32("sd_seed", self.seed, 0, SEED_MAX)?;
        check_f64("sd_denoising_strength", self.denoising_strength, 0.0, 1.0)?;
        check_f64("sd_image_cfg_scale", self.image_cfg_scale, 0.0, 30.0)?;
        check_f64("sd_distilled_guidance", self.distilled_guidance, 0.0, 30.0)?;
        check_f64("sd_eta", self.eta, 0.0, 10.0)?;
        check_f64("sd_flow_shift", self.flow_shift, -100.0, 100.0)?;
        if self
            .size
            .as_deref()
            .is_some_and(|size| size.trim() != size || size.len() < 3)
        {
            return Err(invalid("sd_size"));
        }
        check_u32("sd_vae_tile_size_x", self.vae_tile_size_x, 1, 8192)?;
        check_u32("sd_vae_tile_size_y", self.vae_tile_size_y, 1, 8192)?;
        check_f64("sd_vae_tile_overlap", self.vae_tile_overlap, 0.0, 1.0)?;
        check_f64("sd_hires_scale", self.hires_scale, 1.0, 8.0)?;
        check_u32("sd_hires_width", self.hires_width, 0, 16_384)?;
        check_u32("sd_hires_height", self.hires_height, 0, 16_384)?;
        check_u32("sd_hires_steps", self.hires_steps, 0, 150)?;
        check_f64(
            "sd_hires_denoising_strength",
            self.hires_denoising_strength,
            0.01,
            1.0,
        )?;
        check_f64("sd_slg_scale", self.slg_scale, 0.0, 30.0)?;
        check_f64("sd_slg_layer_start", self.slg_layer_start, 0.0, 1.0)?;
        check_f64("sd_slg_layer_end", self.slg_layer_end, 0.0, 1.0)?;
        for (field, value) in [
            ("sd_sampler", &self.sampler),
            ("sd_scheduler", &self.scheduler),
            ("sd_negative_prompt", &self.negative_prompt),
            ("sd_hires_upscaler", &self.hires_upscaler),
            ("sd_slg_layers", &self.slg_layers),
            ("sd_cache_option", &self.cache_option),
            ("sd_extra_prompt", &self.extra_prompt),
            (
                "sd_prompt_writer_instructions",
                &self.prompt_writer_instructions,
            ),
            ("sdcpp_profile_id", &self.cpp.profile_id),
            ("sdcpp_variant_id", &self.cpp.variant_id),
            ("sdcpp_text_encoder_path", &self.cpp.text_encoder_path),
            ("sdcpp_vae_path", &self.cpp.vae_path),
            ("sdcpp_vision_encoder_path", &self.cpp.vision_encoder_path),
            ("sdcpp_runtime_release", &self.cpp.runtime_release),
            ("sdcpp_runtime_asset", &self.cpp.runtime_asset),
            ("sdcpp_runtime_backend", &self.cpp.runtime_backend),
        ] {
            check_text(field, value.as_deref())?;
        }
        if self.base_loras.iter().flatten().any(|lora| {
            lora.path.trim().is_empty()
                || !lora.multiplier.is_finite()
                || !(0.0..=2.0).contains(&lora.multiplier)
                || lora
                    .keywords
                    .iter()
                    .any(|keyword| keyword.trim().is_empty())
        }) {
            return Err(invalid("sd_base_loras"));
        }
        Ok(())
    }
}

/// Sampling for one app feature that runs inference on this model: chat
/// parameter overrides plus llama.cpp sampler overrides.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeatureGenerationParameters {
    #[serde(default)]
    pub parameters: ChatParameterOverrides,
    #[serde(default)]
    pub llama_sampler: LlamaSamplerSettings,
}

impl FeatureGenerationParameters {
    pub fn validate(&self) -> Result<(), ParameterValidationError> {
        self.parameters.validate()?;
        self.llama_sampler.validate()
    }
}

/// Per-model sampling overrides for each app feature (legacy
/// `featureGenerationSettings`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeatureParameters {
    #[serde(default)]
    pub dynamic_memory: FeatureGenerationParameters,
    #[serde(default)]
    pub companion_soul_writer: FeatureGenerationParameters,
    #[serde(default)]
    pub companion_memory: FeatureGenerationParameters,
    #[serde(default)]
    pub lorebook_entry_generator: FeatureGenerationParameters,
    #[serde(default)]
    pub lorebook_generator: FeatureGenerationParameters,
    #[serde(default)]
    pub scene_writer: FeatureGenerationParameters,
    #[serde(default)]
    pub help_me_reply: FeatureGenerationParameters,
    #[serde(default)]
    pub group_speaker_selection: FeatureGenerationParameters,
    #[serde(default)]
    pub creation_helper: FeatureGenerationParameters,
}

impl FeatureParameters {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    pub fn validate(&self) -> Result<(), ParameterValidationError> {
        for slot in [
            &self.dynamic_memory,
            &self.companion_soul_writer,
            &self.companion_memory,
            &self.lorebook_entry_generator,
            &self.lorebook_generator,
            &self.scene_writer,
            &self.help_me_reply,
            &self.group_speaker_selection,
            &self.creation_helper,
        ] {
            slot.validate()?;
        }
        Ok(())
    }
}

/// Model settings a conversation or the app sets on top of (conversation) or
/// underneath (app) a model's own settings; legacy resolved every field as
/// conversation, then model, then app. An unset field defers to the next
/// layer.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelSettingsLayer {
    #[serde(default)]
    pub chat_parameters: crate::ChatParameterProfile,
    #[serde(default)]
    pub llama_cpp: LlamaCppSettings,
    #[serde(default)]
    pub stable_diffusion: StableDiffusionSettings,
}

/// Validated layers hold only finite numbers, so equality is total.
impl Eq for ModelSettingsLayer {}

impl ModelSettingsLayer {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// The chat parameters as overrides on top of a model: a set field
    /// overrides, an unset field inherits.
    #[must_use]
    pub fn chat_overrides(&self) -> ChatParameterOverrides {
        fn set<T: Clone>(value: &Option<T>) -> crate::ParameterOverride<T> {
            value.clone().map_or(
                crate::ParameterOverride::Inherit,
                crate::ParameterOverride::Set,
            )
        }
        let chat = &self.chat_parameters;
        let ollama = &chat.ollama;
        ChatParameterOverrides {
            temperature: set(&chat.temperature),
            top_p: set(&chat.top_p),
            top_k: set(&chat.top_k),
            max_output_tokens: set(&chat.max_output_tokens),
            context_length: set(&chat.context_length),
            frequency_penalty: set(&chat.frequency_penalty),
            presence_penalty: set(&chat.presence_penalty),
            repetition_penalty: set(&chat.repetition_penalty),
            reasoning_mode: set(&chat.reasoning_mode),
            reasoning_effort: set(&chat.reasoning_effort),
            reasoning_budget_tokens: set(&chat.reasoning_budget_tokens),
            prompt_caching: set(&chat.prompt_caching),
            ollama: crate::OllamaOptionOverrides {
                num_keep: set(&ollama.num_keep),
                num_batch: set(&ollama.num_batch),
                num_gpu: set(&ollama.num_gpu),
                num_thread: set(&ollama.num_thread),
                tfs_z: set(&ollama.tfs_z),
                typical_p: set(&ollama.typical_p),
                min_p: set(&ollama.min_p),
                mirostat: set(&ollama.mirostat),
                mirostat_tau: set(&ollama.mirostat_tau),
                mirostat_eta: set(&ollama.mirostat_eta),
                seed: set(&ollama.seed),
                stop: set(&ollama.stop),
            },
        }
    }

    pub fn validate(&self) -> Result<(), ParameterValidationError> {
        self.chat_parameters.validate()?;
        self.llama_cpp.validate()?;
        self.stable_diffusion.validate()
    }
}
