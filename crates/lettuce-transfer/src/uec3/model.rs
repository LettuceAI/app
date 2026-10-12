use super::model_serde::*;
use lettuce_models::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortableLlamaCppSettings {
    pub gpu_layers: Option<u32>,
    pub multi_gpu_enabled: Option<bool>,
    pub gpu_device_ids: Option<Vec<u32>>,
    pub gpu_distribution_mode: Option<LlamaGpuDistributionMode>,
    pub gpu_manual_layers: Option<Vec<PortableLlamaGpuLayerAssignment>>,
    pub cpu_layers: Option<u32>,
    pub kv_placement: Option<LlamaKvPlacement>,
    pub main_gpu: Option<u32>,
    pub single_gpu_device_id: Option<u32>,
    pub priority_vram_limit_bytes: Option<u64>,
    pub threads: Option<u32>,
    pub threads_batch: Option<u32>,
    pub rope_freq_base: Option<f64>,
    pub rope_freq_scale: Option<f64>,
    pub offload_kqv: Option<bool>,
    pub batch_size: Option<u32>,
    pub ubatch_size: Option<u32>,
    pub kv_type: Option<LlamaKvType>,
    pub kv_type_k: Option<LlamaKvType>,
    pub kv_type_v: Option<LlamaKvType>,
    pub flash_attention: Option<LlamaFlashAttention>,
    pub swa_full: Option<bool>,
    pub chat_template_override: Option<String>,
    pub chat_template_preset: Option<String>,
    pub raw_completion_fallback: Option<bool>,
    pub strict_mode: Option<bool>,
    pub mtp_enabled: Option<bool>,
    pub mtp_placement: Option<LlamaMtpPlacement>,
    pub mtp_draft_tokens: Option<u32>,
    pub dflash_enabled: Option<bool>,
    pub dflash_draft_tokens: Option<u32>,
    pub dflash_min_probability: Option<f64>,
    pub streaming_enabled: Option<bool>,
    pub force_gemma4_reasoning: Option<bool>,
    #[serde(with = "WireLlamaSamplerSettings")]
    pub sampler: LlamaSamplerSettings,
}

impl From<&LlamaCppSettings> for PortableLlamaCppSettings {
    fn from(value: &LlamaCppSettings) -> Self {
        Self {
            gpu_layers: value.gpu_layers,
            multi_gpu_enabled: value.multi_gpu_enabled,
            gpu_device_ids: value.gpu_device_ids.clone(),
            gpu_distribution_mode: value.gpu_distribution_mode,
            gpu_manual_layers: value
                .gpu_manual_layers
                .as_ref()
                .map(|layers| layers.iter().map(Into::into).collect()),
            cpu_layers: value.cpu_layers,
            kv_placement: value.kv_placement,
            main_gpu: value.main_gpu,
            single_gpu_device_id: value.single_gpu_device_id,
            priority_vram_limit_bytes: value.priority_vram_limit_bytes,
            threads: value.threads,
            threads_batch: value.threads_batch,
            rope_freq_base: value.rope_freq_base,
            rope_freq_scale: value.rope_freq_scale,
            offload_kqv: value.offload_kqv,
            batch_size: value.batch_size,
            ubatch_size: value.ubatch_size,
            kv_type: value.kv_type,
            kv_type_k: value.kv_type_k,
            kv_type_v: value.kv_type_v,
            flash_attention: value.flash_attention,
            swa_full: value.swa_full,
            chat_template_override: value.chat_template_override.clone(),
            chat_template_preset: value.chat_template_preset.clone(),
            raw_completion_fallback: value.raw_completion_fallback,
            strict_mode: value.strict_mode,
            mtp_enabled: value.mtp_enabled,
            mtp_placement: value.mtp_placement,
            mtp_draft_tokens: value.mtp_draft_tokens,
            dflash_enabled: value.dflash_enabled,
            dflash_draft_tokens: value.dflash_draft_tokens,
            dflash_min_probability: value.dflash_min_probability,
            streaming_enabled: value.streaming_enabled,
            force_gemma4_reasoning: value.force_gemma4_reasoning,
            sampler: value.sampler.clone(),
        }
    }
}

impl From<PortableLlamaCppSettings> for LlamaCppSettings {
    fn from(value: PortableLlamaCppSettings) -> Self {
        Self {
            gpu_layers: value.gpu_layers,
            multi_gpu_enabled: value.multi_gpu_enabled,
            gpu_device_ids: value.gpu_device_ids,
            gpu_distribution_mode: value.gpu_distribution_mode,
            gpu_manual_layers: value
                .gpu_manual_layers
                .map(|layers| layers.into_iter().map(Into::into).collect()),
            cpu_layers: value.cpu_layers,
            kv_placement: value.kv_placement,
            main_gpu: value.main_gpu,
            single_gpu_device_id: value.single_gpu_device_id,
            priority_vram_limit_bytes: value.priority_vram_limit_bytes,
            threads: value.threads,
            threads_batch: value.threads_batch,
            rope_freq_base: value.rope_freq_base,
            rope_freq_scale: value.rope_freq_scale,
            offload_kqv: value.offload_kqv,
            batch_size: value.batch_size,
            ubatch_size: value.ubatch_size,
            kv_type: value.kv_type,
            kv_type_k: value.kv_type_k,
            kv_type_v: value.kv_type_v,
            flash_attention: value.flash_attention,
            swa_full: value.swa_full,
            chat_template_override: value.chat_template_override,
            chat_template_preset: value.chat_template_preset,
            raw_completion_fallback: value.raw_completion_fallback,
            strict_mode: value.strict_mode,
            mtp_enabled: value.mtp_enabled,
            mtp_placement: value.mtp_placement,
            mtp_draft_tokens: value.mtp_draft_tokens,
            dflash_enabled: value.dflash_enabled,
            dflash_draft_tokens: value.dflash_draft_tokens,
            dflash_min_probability: value.dflash_min_probability,
            streaming_enabled: value.streaming_enabled,
            force_gemma4_reasoning: value.force_gemma4_reasoning,
            sampler: value.sampler,
            mmproj_path: None,
            mtp_model_path: None,
            dflash_model_path: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortableStableDiffusionCppBinding {
    pub profile_id: Option<String>,
    pub variant_id: Option<String>,
    pub max_reference_images: Option<u32>,
    pub supports_lora: Option<bool>,
    pub supports_text_to_image: Option<bool>,
    pub supports_image_edit: Option<bool>,
    pub recommended_for_scenes: Option<bool>,
    pub requires_reference_image: Option<bool>,
}

impl From<&StableDiffusionCppBinding> for PortableStableDiffusionCppBinding {
    fn from(value: &StableDiffusionCppBinding) -> Self {
        Self {
            profile_id: value.profile_id.clone(),
            variant_id: value.variant_id.clone(),
            max_reference_images: value.max_reference_images,
            supports_lora: value.supports_lora,
            supports_text_to_image: value.supports_text_to_image,
            supports_image_edit: value.supports_image_edit,
            recommended_for_scenes: value.recommended_for_scenes,
            requires_reference_image: value.requires_reference_image,
        }
    }
}

impl From<PortableStableDiffusionCppBinding> for StableDiffusionCppBinding {
    fn from(value: PortableStableDiffusionCppBinding) -> Self {
        Self {
            max_reference_images: value.max_reference_images,
            supports_lora: value.supports_lora,
            supports_text_to_image: value.supports_text_to_image,
            supports_image_edit: value.supports_image_edit,
            recommended_for_scenes: value.recommended_for_scenes,
            requires_reference_image: value.requires_reference_image,
            profile_id: value.profile_id,
            variant_id: value.variant_id,
            text_encoder_path: None,
            vae_path: None,
            vision_encoder_path: None,
            runtime_release: None,
            runtime_asset: None,
            runtime_backend: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PortableStableDiffusionSettings {
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
    pub cache_mode: Option<StableDiffusionCacheMode>,
    pub cache_option: Option<String>,
    pub offload_mode: Option<StableDiffusionOffloadMode>,
    pub extra_prompt: Option<String>,
    pub prompt_writer_instructions: Option<String>,
    pub cpp: PortableStableDiffusionCppBinding,
}

impl From<&StableDiffusionSettings> for PortableStableDiffusionSettings {
    fn from(value: &StableDiffusionSettings) -> Self {
        Self {
            steps: value.steps,
            cfg_scale: value.cfg_scale,
            sampler: value.sampler.clone(),
            scheduler: value.scheduler.clone(),
            seed: value.seed,
            negative_prompt: value.negative_prompt.clone(),
            denoising_strength: value.denoising_strength,
            image_cfg_scale: value.image_cfg_scale,
            distilled_guidance: value.distilled_guidance,
            eta: value.eta,
            flow_shift: value.flow_shift,
            size: value.size.clone(),
            vae_tiling_enabled: value.vae_tiling_enabled,
            vae_tile_size_x: value.vae_tile_size_x,
            vae_tile_size_y: value.vae_tile_size_y,
            vae_tile_overlap: value.vae_tile_overlap,
            auto_resize_reference_images: value.auto_resize_reference_images,
            increase_reference_index: value.increase_reference_index,
            hires_enabled: value.hires_enabled,
            hires_upscaler: value.hires_upscaler.clone(),
            hires_scale: value.hires_scale,
            hires_width: value.hires_width,
            hires_height: value.hires_height,
            hires_steps: value.hires_steps,
            hires_denoising_strength: value.hires_denoising_strength,
            slg_scale: value.slg_scale,
            slg_layers: value.slg_layers.clone(),
            slg_layer_start: value.slg_layer_start,
            slg_layer_end: value.slg_layer_end,
            cache_mode: value.cache_mode,
            cache_option: value.cache_option.clone(),
            offload_mode: value.offload_mode,
            extra_prompt: value.extra_prompt.clone(),
            prompt_writer_instructions: value.prompt_writer_instructions.clone(),
            cpp: (&value.cpp).into(),
        }
    }
}

impl From<PortableStableDiffusionSettings> for StableDiffusionSettings {
    fn from(value: PortableStableDiffusionSettings) -> Self {
        Self {
            steps: value.steps,
            cfg_scale: value.cfg_scale,
            sampler: value.sampler,
            scheduler: value.scheduler,
            seed: value.seed,
            negative_prompt: value.negative_prompt,
            denoising_strength: value.denoising_strength,
            image_cfg_scale: value.image_cfg_scale,
            distilled_guidance: value.distilled_guidance,
            eta: value.eta,
            flow_shift: value.flow_shift,
            size: value.size,
            vae_tiling_enabled: value.vae_tiling_enabled,
            vae_tile_size_x: value.vae_tile_size_x,
            vae_tile_size_y: value.vae_tile_size_y,
            vae_tile_overlap: value.vae_tile_overlap,
            auto_resize_reference_images: value.auto_resize_reference_images,
            increase_reference_index: value.increase_reference_index,
            hires_enabled: value.hires_enabled,
            hires_upscaler: value.hires_upscaler,
            hires_scale: value.hires_scale,
            hires_width: value.hires_width,
            hires_height: value.hires_height,
            hires_steps: value.hires_steps,
            hires_denoising_strength: value.hires_denoising_strength,
            slg_scale: value.slg_scale,
            slg_layers: value.slg_layers,
            slg_layer_start: value.slg_layer_start,
            slg_layer_end: value.slg_layer_end,
            cache_mode: value.cache_mode,
            cache_option: value.cache_option,
            offload_mode: value.offload_mode,
            extra_prompt: value.extra_prompt,
            prompt_writer_instructions: value.prompt_writer_instructions,
            cpp: value.cpp.into(),
            base_loras: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProvider {
    pub kind: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelParameters {
    #[serde(with = "WireChatParameterProfile")]
    pub chat: ChatParameterProfile,
    #[serde(with = "WireFeatureParameters")]
    pub features: FeatureParameters,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProfile {
    pub display_name: String,
    pub kind: ModelKind,
    pub provider: ModelProvider,
    pub external_id: String,
    pub parameters: ModelParameters,
    #[serde(with = "WireModelCapabilities")]
    pub capabilities: ModelCapabilities,
    pub llama_cpp: PortableLlamaCppSettings,
    pub stable_diffusion: PortableStableDiffusionSettings,
}

impl ModelProfile {
    pub fn from_stored(profile: &lettuce_models::ModelProfile, account: &ProviderAccount) -> Self {
        Self {
            display_name: profile.display_name.clone(),
            kind: profile.kind,
            provider: ModelProvider {
                kind: account.provider_kind.clone(),
                label: account.label.clone(),
            },
            external_id: if matches!(
                account.protocol,
                ProviderProtocol::LlamaCpp | ProviderProtocol::StableDiffusion
            ) {
                UNPICKED_LOCAL_MODEL_FILE.to_owned()
            } else {
                profile.external_model_id.clone()
            },
            parameters: ModelParameters {
                chat: profile.config.chat_parameters.clone(),
                features: profile.config.feature_parameters.clone(),
            },
            capabilities: profile.config.capabilities.clone(),
            llama_cpp: (&profile.config.llama_cpp).into(),
            stable_diffusion: (&profile.config.stable_diffusion).into(),
        }
    }

    pub fn into_stored(
        self,
        account: lettuce_types::ProviderAccountId,
        now: lettuce_types::TimestampMillis,
    ) -> Result<lettuce_models::ModelProfile, super::Error> {
        let config = ModelProfileConfig {
            chat_parameters: self.parameters.chat,
            feature_parameters: self.parameters.features,
            capabilities: self.capabilities,
            llama_cpp: self.llama_cpp.into(),
            stable_diffusion: self.stable_diffusion.into(),
        };
        config.validate_parameters().map_err(super::format)?;
        config.capabilities.validate().map_err(super::format)?;
        if self.display_name.trim().is_empty()
            || self.external_id.trim().is_empty()
            || self.provider.kind.trim().is_empty()
        {
            return Err(super::Error::Format(
                "model name and provider are required".into(),
            ));
        }
        Ok(lettuce_models::ModelProfile {
            id: lettuce_types::ModelProfileId::new(),
            provider_account_id: account,
            external_model_id: self.external_id,
            display_name: self.display_name,
            kind: self.kind,
            config,
            revision: lettuce_types::Revision::INITIAL,
            created_at: now,
            updated_at: now,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableLlamaGpuLayerAssignment {
    pub device_id: u32,
    pub layers: u32,
}

impl From<&LlamaGpuLayerAssignment> for PortableLlamaGpuLayerAssignment {
    fn from(value: &LlamaGpuLayerAssignment) -> Self {
        Self {
            device_id: value.device_id,
            layers: value.layers,
        }
    }
}

impl From<PortableLlamaGpuLayerAssignment> for LlamaGpuLayerAssignment {
    fn from(value: PortableLlamaGpuLayerAssignment) -> Self {
        Self {
            device_id: value.device_id,
            layers: value.layers,
        }
    }
}
