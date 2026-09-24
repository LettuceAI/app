use std::collections::BTreeSet;

use lettuce_models::{
    ChatParameterOverrides, ChatParameterProfile, FeatureGenerationParameters, FeatureParameters,
    LlamaCppSettings, LlamaFlashAttention, LlamaGpuDistributionMode, LlamaGpuLayerAssignment,
    LlamaKvPlacement, LlamaKvType, LlamaMtpPlacement, LlamaSamplerProfile, LlamaSamplerSettings,
    LlamaSamplerStage, OllamaOptionOverrides, OllamaOptions, OpenRouterOptions, ParameterOverride,
    PromptCacheRetention, PromptCaching, ReasoningEffort, ReasoningMode, StableDiffusionCacheMode,
    StableDiffusionCppBinding, StableDiffusionLora, StableDiffusionOffloadMode,
    StableDiffusionSettings,
};
use serde_json::{Map, Value};

const SEED_MAX: u64 = i32::MAX as u64;

/// One legacy model's `advanced_model_settings`, moved into typed settings.
/// A value that is the wrong type or outside the legacy editor's range is left
/// out and listed in `lossy_fields`; keys legacy never defined are listed in
/// `unknown_fields`. Neither aborts the import.
#[derive(Debug, Clone, PartialEq)]
pub struct LegacyModelParameters {
    pub chat_parameters: ChatParameterProfile,
    pub feature_parameters: FeatureParameters,
    pub llama_cpp: LlamaCppSettings,
    pub stable_diffusion: StableDiffusionSettings,
    pub lossy_fields: Vec<String>,
    pub unknown_fields: Vec<String>,
}

struct Reader<'a> {
    object: &'a Map<String, Value>,
    prefix: String,
    read: BTreeSet<&'a str>,
    lossy: &'a mut Vec<String>,
}

impl<'a> Reader<'a> {
    fn new(object: &'a Map<String, Value>, prefix: String, lossy: &'a mut Vec<String>) -> Self {
        Self {
            object,
            prefix,
            read: BTreeSet::new(),
            lossy,
        }
    }

    fn value(&mut self, key: &'a str) -> Option<&'a Value> {
        self.read.insert(key);
        self.object.get(key).filter(|value| !value.is_null())
    }

    fn lose(&mut self, key: &str) {
        self.lossy.push(format!("{}{key}", self.prefix));
    }

    fn parse<T>(&mut self, key: &'a str, parse: impl FnOnce(&Value) -> Option<T>) -> Option<T> {
        let value = self.value(key)?;
        let parsed = parse(value);
        if parsed.is_none() {
            self.lose(key);
        }
        parsed
    }

    fn f64(&mut self, key: &'a str, min: f64, max: f64) -> Option<f64> {
        self.parse(key, |value| {
            value
                .as_f64()
                .filter(|value| value.is_finite() && (min..=max).contains(value))
        })
    }

    fn u64(&mut self, key: &'a str, min: u64, max: u64) -> Option<u64> {
        self.parse(key, |value| {
            value.as_u64().filter(|value| (min..=max).contains(value))
        })
    }

    fn u32(&mut self, key: &'a str, min: u32, max: u32) -> Option<u32> {
        self.u64(key, u64::from(min), u64::from(max))
            .and_then(|value| u32::try_from(value).ok())
    }

    fn i32(&mut self, key: &'a str, min: i32, max: i32) -> Option<i32> {
        self.parse(key, |value| {
            value
                .as_i64()
                .filter(|value| (i64::from(min)..=i64::from(max)).contains(value))
                .and_then(|value| i32::try_from(value).ok())
        })
    }

    fn bool(&mut self, key: &'a str) -> Option<bool> {
        self.parse(key, Value::as_bool)
    }

    fn text(&mut self, key: &'a str) -> Option<String> {
        self.parse(key, |value| {
            value
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
        })
    }

    fn texts(&mut self, key: &'a str, allow_empty: bool) -> Option<Vec<String>> {
        self.parse(key, |value| {
            value
                .as_array()?
                .iter()
                .map(|item| {
                    item.as_str()
                        .filter(|item| allow_empty || !item.trim().is_empty())
                        .map(str::to_owned)
                })
                .collect()
        })
    }

    /// Ollama stop sequences within the profile's bounds (256 items of at most
    /// 4096 bytes); a longer list is left out rather than aborting the import.
    fn stop(&mut self, key: &'a str) -> Option<Vec<String>> {
        self.parse(key, |value| {
            let items = value.as_array()?;
            if items.len() > 256 {
                return None;
            }
            items
                .iter()
                .map(|item| {
                    item.as_str()
                        .filter(|item| !item.is_empty() && item.len() <= 4_096)
                        .map(str::to_owned)
                })
                .collect()
        })
    }

    fn choice<T>(&mut self, key: &'a str, map: impl Fn(&str) -> Option<T>) -> Option<T> {
        self.parse(key, |value| value.as_str().and_then(&map))
    }

    fn unknown(&self) -> Vec<String> {
        self.object
            .iter()
            .filter(|(key, value)| !value.is_null() && !self.read.contains(key.as_str()))
            .map(|(key, _)| format!("{}{key}", self.prefix))
            .collect()
    }
}

fn set<T>(value: Option<T>) -> ParameterOverride<T> {
    value.map_or(ParameterOverride::Inherit, ParameterOverride::Set)
}

fn sampler_profile(value: &str) -> Option<LlamaSamplerProfile> {
    match value.trim().to_ascii_lowercase().as_str() {
        "balanced" => Some(LlamaSamplerProfile::Balanced),
        "creative" => Some(LlamaSamplerProfile::Creative),
        "stable" => Some(LlamaSamplerProfile::Stable),
        "reasoning" => Some(LlamaSamplerProfile::Reasoning),
        _ => None,
    }
}

fn sampler_stage(value: &str) -> Option<LlamaSamplerStage> {
    match value.trim().to_ascii_lowercase().as_str() {
        "penalties" => Some(LlamaSamplerStage::Penalties),
        "grammar" => Some(LlamaSamplerStage::Grammar),
        "top_k" | "topk" => Some(LlamaSamplerStage::TopK),
        "top_p" | "topp" => Some(LlamaSamplerStage::TopP),
        "min_p" | "minp" => Some(LlamaSamplerStage::MinP),
        "dry" => Some(LlamaSamplerStage::Dry),
        "typical" | "typ_p" | "typical_p" => Some(LlamaSamplerStage::Typical),
        "xtc" => Some(LlamaSamplerStage::Xtc),
        "temp" | "temperature" => Some(LlamaSamplerStage::Temp),
        "adaptive_p" | "adaptivep" | "adaptive" => Some(LlamaSamplerStage::AdaptiveP),
        _ => None,
    }
}

/// The stages the old runtime ran for a saved order: names normalized,
/// unknown or non-text entries skipped, repeats dropped.
fn sampler_order(value: &Value) -> Option<Vec<LlamaSamplerStage>> {
    let mut order = Vec::new();
    for stage in value.as_array()?.iter().filter_map(Value::as_str) {
        if let Some(stage) = sampler_stage(stage)
            && !order.contains(&stage)
        {
            order.push(stage);
        }
    }
    Some(order)
}

fn llama_sampler(reader: &mut Reader<'_>) -> LlamaSamplerSettings {
    LlamaSamplerSettings {
        profile: reader.choice("llamaSamplerProfile", sampler_profile),
        order: reader.parse("llamaSamplerOrder", sampler_order),
        min_p: reader.f64("llamaMinP", 0.0, 1.0),
        typical_p: reader.f64("llamaTypicalP", 0.0, 1.0),
        repeat_penalty: reader.f64("llamaRepeatPenalty", 0.0, 2.0),
        n_pen_range: reader.i32("llamaNPenRange", -1, 262_144),
        dry_multiplier: reader.f64("llamaDryMultiplier", 0.0, 10.0),
        dry_base: reader.f64("llamaDryBase", 0.0, 10.0),
        dry_allowed_length: reader.u32("llamaDryAllowedLength", 0, 128),
        dry_penalty_last_n: reader.i32("llamaDryPenaltyLastN", -1, 262_144),
        dry_sequence_breakers: reader.texts("llamaDrySequenceBreakers", true),
        xtc_probability: reader.f64("llamaXtcProbability", 0.0, 1.0),
        xtc_threshold: reader.f64("llamaXtcThreshold", 0.0, 1.0),
        seed: reader
            .u64("llamaSeed", 0, SEED_MAX)
            .and_then(|value| u32::try_from(value).ok()),
        adaptive_target: reader.f64("llamaAdaptiveTarget", 0.0, 1.0),
        adaptive_decay: reader.f64("llamaAdaptiveDecay", 0.0, 0.99),
    }
}

/// The common sampling and Ollama keys a legacy feature slot overrides; a
/// present field is `Set`, an absent one inherits (legacy
/// `feature_model_overrides`). Ollama repetition penalty must be positive.
fn feature_slot(reader: &mut Reader<'_>) -> FeatureGenerationParameters {
    FeatureGenerationParameters {
        parameters: ChatParameterOverrides {
            temperature: set(reader.f64("temperature", 0.0, 2.0)),
            top_p: set(reader.f64("topP", 0.0, 1.0)),
            top_k: set(reader.u32("topK", 1, u32::MAX)),
            max_output_tokens: set(reader.u32("maxOutputTokens", 1, u32::MAX)),
            frequency_penalty: set(reader.f64("frequencyPenalty", -2.0, 2.0)),
            presence_penalty: set(reader.f64("presencePenalty", -2.0, 2.0)),
            repetition_penalty: set(reader.f64("ollamaRepeatPenalty", f64::MIN_POSITIVE, 2.0)),
            ollama: OllamaOptionOverrides {
                tfs_z: set(reader.f64("ollamaTfsZ", 0.0, 1.0)),
                typical_p: set(reader.f64("ollamaTypicalP", 0.0, 1.0)),
                min_p: set(reader.f64("ollamaMinP", 0.0, 1.0)),
                mirostat: set(reader.u32("ollamaMirostat", 0, 2)),
                mirostat_tau: set(reader.f64("ollamaMirostatTau", 0.0, 10.0)),
                mirostat_eta: set(reader.f64("ollamaMirostatEta", 0.0, 1.0)),
                seed: set(reader.u32("ollamaSeed", 0, i32::MAX as u32)),
                stop: set(reader.stop("ollamaStop")),
                ..OllamaOptionOverrides::default()
            },
            ..ChatParameterOverrides::default()
        },
        llama_sampler: llama_sampler(reader),
    }
}

fn chat_parameters(reader: &mut Reader<'_>, provider_kind: &str) -> ChatParameterProfile {
    let reasoning_mode = reader.bool("reasoningEnabled").map(|enabled| {
        if enabled {
            ReasoningMode::Enabled
        } else {
            ReasoningMode::Disabled
        }
    });
    let reasoning_effort = reader.choice("reasoningEffort", |value| match value {
        "low" => Some(ReasoningEffort::Low),
        "medium" => Some(ReasoningEffort::Medium),
        "high" => Some(ReasoningEffort::High),
        _ => None,
    });
    let reasoning_budget_tokens = reader.u32("reasoningBudgetTokens", 1024, u32::MAX);
    let caching_enabled = reader.bool("promptCachingEnabled");
    let retention = reader.choice("promptCachingTtl", |value| match value {
        "in_memory" => Some(PromptCacheRetention::InMemory),
        "5min" => Some(PromptCacheRetention::FiveMinutes),
        "1h" => Some(PromptCacheRetention::OneHour),
        "24h" => Some(PromptCacheRetention::TwentyFourHours),
        _ => None,
    });
    let prompt_caching = caching_enabled.map(|enabled| {
        if enabled {
            PromptCaching::Enabled {
                retention: retention.unwrap_or(if provider_kind == "openai" {
                    PromptCacheRetention::InMemory
                } else {
                    PromptCacheRetention::FiveMinutes
                }),
            }
        } else {
            PromptCaching::Disabled
        }
    });
    let max_output_tokens = reader
        .u32("maxOutputTokens", 0, u32::MAX)
        .filter(|value| *value != 0);
    let num_predict = reader
        .u32("ollamaNumPredict", 0, 131_072)
        .filter(|value| *value != 0);
    let context_length = reader
        .u32("contextLength", 0, u32::MAX)
        .filter(|value| *value != 0);
    let num_ctx = reader
        .u32("ollamaNumCtx", 0, 262_144)
        .filter(|value| *value != 0);
    if max_output_tokens.is_some()
        && num_predict.is_some_and(|value| Some(value) != max_output_tokens)
    {
        reader.lose("ollamaNumPredict");
    }
    if context_length.is_some() && num_ctx.is_some_and(|value| Some(value) != context_length) {
        reader.lose("ollamaNumCtx");
    }
    ChatParameterProfile {
        temperature: reader.f64("temperature", 0.0, 2.0),
        top_p: reader.f64("topP", 0.0, 1.0),
        top_k: reader.u32("topK", 1, u32::MAX),
        max_output_tokens: max_output_tokens.or(num_predict),
        context_length: context_length.or(num_ctx),
        frequency_penalty: reader.f64("frequencyPenalty", -2.0, 2.0),
        presence_penalty: reader.f64("presencePenalty", -2.0, 2.0),
        repetition_penalty: reader.f64("ollamaRepeatPenalty", f64::MIN_POSITIVE, 2.0),
        reasoning_mode,
        reasoning_effort,
        reasoning_budget_tokens,
        prompt_caching,
        send_thinking_state: reader.bool("forceSendThinkingState"),
        ollama: OllamaOptions {
            num_keep: reader.u32("ollamaNumKeep", 0, 32_768),
            num_batch: reader.u32("ollamaNumBatch", 1, 16_384),
            num_gpu: reader.u32("ollamaNumGpu", 0, 512),
            num_thread: reader.u32("ollamaNumThread", 1, 256),
            tfs_z: reader.f64("ollamaTfsZ", 0.0, 1.0),
            typical_p: reader.f64("ollamaTypicalP", 0.0, 1.0),
            min_p: reader.f64("ollamaMinP", 0.0, 1.0),
            mirostat: reader.u32("ollamaMirostat", 0, 2),
            mirostat_tau: reader.f64("ollamaMirostatTau", 0.0, 10.0),
            mirostat_eta: reader.f64("ollamaMirostatEta", 0.0, 1.0),
            seed: reader.u32("ollamaSeed", 0, i32::MAX as u32),
            stop: reader.stop("ollamaStop"),
        },
        openrouter: OpenRouterOptions {
            pinned_provider: reader.parse("openRouterProvider", |value| {
                value
                    .get("id")?
                    .as_str()
                    .map(str::trim)
                    .filter(|id| {
                        !id.is_empty() && id.len() <= 256 && !id.chars().any(char::is_control)
                    })
                    .map(str::to_owned)
            }),
        },
    }
}

fn llama_cpp(reader: &mut Reader<'_>) -> LlamaCppSettings {
    LlamaCppSettings {
        gpu_layers: reader.u32("llamaGpuLayers", 0, 512),
        multi_gpu_enabled: reader.bool("llamaMultiGpuEnabled"),
        gpu_device_ids: reader.parse("llamaGpuDeviceIds", |value| {
            value
                .as_array()?
                .iter()
                .map(|id| id.as_u64().and_then(|id| u32::try_from(id).ok()))
                .collect()
        }),
        gpu_distribution_mode: reader.choice("llamaGpuDistributionMode", |value| match value {
            "balanced" => Some(LlamaGpuDistributionMode::Balanced),
            "proportional" => Some(LlamaGpuDistributionMode::Proportional),
            "priority" => Some(LlamaGpuDistributionMode::Priority),
            "manual" => Some(LlamaGpuDistributionMode::Manual),
            _ => None,
        }),
        gpu_manual_layers: reader.parse("llamaGpuManualLayers", |value| {
            value
                .as_array()?
                .iter()
                .map(|assignment| {
                    Some(LlamaGpuLayerAssignment {
                        device_id: u32::try_from(assignment.get("deviceId")?.as_u64()?).ok()?,
                        layers: u32::try_from(assignment.get("layers")?.as_u64()?)
                            .ok()
                            .filter(|layers| *layers <= 512)?,
                    })
                })
                .collect()
        }),
        cpu_layers: reader.u32("llamaCpuLayers", 0, 512),
        kv_placement: reader.choice("llamaKvPlacement", |value| match value {
            "auto" => Some(LlamaKvPlacement::Auto),
            "split" => Some(LlamaKvPlacement::Split),
            "systemRam" => Some(LlamaKvPlacement::SystemRam),
            "pin" => Some(LlamaKvPlacement::Pin),
            _ => None,
        }),
        main_gpu: reader.u32("llamaMainGpu", 0, u32::MAX),
        single_gpu_device_id: reader.u32("llamaSingleGpuDeviceId", 0, u32::MAX),
        priority_vram_limit_bytes: reader.u64("llamaPriorityVramLimitBytes", 0, u64::MAX),
        threads: reader.u32("llamaThreads", 1, 256),
        threads_batch: reader.u32("llamaThreadsBatch", 1, 256),
        rope_freq_base: reader.f64("llamaRopeFreqBase", 0.0, 1_000_000.0),
        rope_freq_scale: reader.f64("llamaRopeFreqScale", 0.0, 10.0),
        offload_kqv: reader.bool("llamaOffloadKqv"),
        batch_size: reader.u32("llamaBatchSize", 1, 8192),
        ubatch_size: reader.u32("llamaUbatchSize", 1, 8192),
        kv_type: reader.parse("llamaKvType", |value| {
            serde_json::from_value::<LlamaKvType>(value.clone()).ok()
        }),
        kv_type_k: None,
        kv_type_v: None,
        flash_attention: reader.choice("llamaFlashAttention", |value| match value {
            "auto" => Some(LlamaFlashAttention::Auto),
            "enabled" => Some(LlamaFlashAttention::Enabled),
            "disabled" => Some(LlamaFlashAttention::Disabled),
            _ => None,
        }),
        swa_full: reader.bool("llamaSwaFull"),
        chat_template_override: reader.text("llamaChatTemplateOverride"),
        chat_template_preset: reader.text("llamaChatTemplatePreset"),
        mmproj_path: reader.text("llamaMmprojPath"),
        raw_completion_fallback: reader.bool("llamaRawCompletionFallback"),
        strict_mode: reader.bool("llamaStrictMode"),
        mtp_enabled: reader.bool("llamaMtpEnabled"),
        mtp_placement: reader.choice("llamaMtpPlacement", |value| match value {
            "auto" => Some(LlamaMtpPlacement::Auto),
            "gpu" => Some(LlamaMtpPlacement::Gpu),
            "cpu" => Some(LlamaMtpPlacement::Cpu),
            _ => None,
        }),
        mtp_draft_tokens: reader.u32("llamaMtpDraftTokens", 1, 8),
        mtp_model_path: reader.text("llamaMtpModelPath"),
        streaming_enabled: reader.bool("llamaStreamingEnabled"),
        force_gemma4_reasoning: reader.bool("forceGemma4Reasoning"),
        sampler: llama_sampler(reader),
    }
}

fn stable_diffusion(reader: &mut Reader<'_>) -> StableDiffusionSettings {
    StableDiffusionSettings {
        steps: reader.u32("sdSteps", 1, 150),
        cfg_scale: reader.f64("sdCfgScale", 0.0, 30.0),
        sampler: reader.text("sdSampler"),
        scheduler: reader.text("sdScheduler"),
        seed: reader
            .u64("sdSeed", 0, SEED_MAX)
            .and_then(|value| u32::try_from(value).ok()),
        negative_prompt: reader.text("sdNegativePrompt"),
        denoising_strength: reader.f64("sdDenoisingStrength", 0.0, 1.0),
        image_cfg_scale: reader.f64("sdImageCfgScale", 0.0, 30.0),
        distilled_guidance: reader.f64("sdDistilledGuidance", 0.0, 30.0),
        eta: reader.f64("sdEta", 0.0, 10.0),
        flow_shift: reader.f64("sdFlowShift", -100.0, 100.0),
        size: reader.parse("sdSize", |value| {
            value
                .as_str()
                .map(str::trim)
                .filter(|size| size.len() >= 3)
                .map(str::to_owned)
        }),
        vae_tiling_enabled: reader.bool("sdVaeTilingEnabled"),
        vae_tile_size_x: reader.u32("sdVaeTileSizeX", 1, 8192),
        vae_tile_size_y: reader.u32("sdVaeTileSizeY", 1, 8192),
        vae_tile_overlap: reader.f64("sdVaeTileOverlap", 0.0, 1.0),
        auto_resize_reference_images: reader.bool("sdAutoResizeRefImages"),
        increase_reference_index: reader.bool("sdIncreaseRefIndex"),
        hires_enabled: reader.bool("sdHiresEnabled"),
        hires_upscaler: reader.text("sdHiresUpscaler"),
        hires_scale: reader.f64("sdHiresScale", 1.0, 8.0),
        hires_width: reader.u32("sdHiresWidth", 0, 16_384),
        hires_height: reader.u32("sdHiresHeight", 0, 16_384),
        hires_steps: reader.u32("sdHiresSteps", 0, 150),
        hires_denoising_strength: reader.f64("sdHiresDenoisingStrength", 0.01, 1.0),
        slg_scale: reader.f64("sdSlgScale", 0.0, 30.0),
        slg_layers: reader.text("sdSlgLayers"),
        slg_layer_start: reader.f64("sdSlgLayerStart", 0.0, 1.0),
        slg_layer_end: reader.f64("sdSlgLayerEnd", 0.0, 1.0),
        cache_mode: reader.choice("sdCacheMode", |value| match value {
            "disabled" => Some(StableDiffusionCacheMode::Disabled),
            "easycache" => Some(StableDiffusionCacheMode::Easycache),
            "ucache" => Some(StableDiffusionCacheMode::Ucache),
            "dbcache" => Some(StableDiffusionCacheMode::Dbcache),
            "taylorseer" => Some(StableDiffusionCacheMode::Taylorseer),
            "cache-dit" => Some(StableDiffusionCacheMode::CacheDit),
            "spectrum" => Some(StableDiffusionCacheMode::Spectrum),
            _ => None,
        }),
        cache_option: reader.text("sdCacheOption"),
        offload_mode: reader.choice("sdOffloadMode", |value| match value {
            "auto" => Some(StableDiffusionOffloadMode::Auto),
            "gpu" => Some(StableDiffusionOffloadMode::Gpu),
            "mixed" => Some(StableDiffusionOffloadMode::Mixed),
            _ => None,
        }),
        extra_prompt: reader.text("sdExtraPrompt"),
        prompt_writer_instructions: reader.text("sdPromptWriterInstructions"),
        base_loras: reader.parse("sdBaseLoras", |value| {
            value
                .as_array()?
                .iter()
                .map(|lora| {
                    let path = lora.get("path")?.as_str()?.trim();
                    let multiplier = lora.get("multiplier")?.as_f64()?;
                    let is_high_noise = match lora.get("isHighNoise") {
                        None | Some(Value::Null) => false,
                        Some(value) => value.as_bool()?,
                    };
                    let keywords = match lora.get("keywords").filter(|value| !value.is_null()) {
                        Some(keywords) => keywords
                            .as_array()?
                            .iter()
                            .map(|keyword| {
                                keyword
                                    .as_str()
                                    .map(str::trim)
                                    .filter(|keyword| !keyword.is_empty())
                                    .map(str::to_owned)
                            })
                            .collect::<Option<Vec<_>>>()?,
                        None => Vec::new(),
                    };
                    (!path.is_empty()
                        && multiplier.is_finite()
                        && (0.0..=2.0).contains(&multiplier))
                    .then(|| StableDiffusionLora {
                        path: path.to_owned(),
                        multiplier,
                        is_high_noise,
                        keywords,
                    })
                })
                .collect()
        }),
        cpp: StableDiffusionCppBinding {
            profile_id: reader.text("sdcppProfileId"),
            variant_id: reader.text("sdcppVariantId"),
            text_encoder_path: reader.text("sdcppTextEncoderPath"),
            vae_path: reader.text("sdcppVaePath"),
            vision_encoder_path: reader.text("sdcppVisionEncoderPath"),
            runtime_release: reader.text("sdcppRuntimeRelease"),
            runtime_asset: reader.text("sdcppRuntimeAsset"),
            runtime_backend: reader.text("sdcppRuntimeBackend"),
            max_reference_images: reader.u32("sdcppMaxReferenceImages", 0, u32::MAX),
            supports_lora: reader.bool("sdcppSupportsLora"),
            supports_text_to_image: reader.bool("sdcppSupportsTextToImage"),
            supports_image_edit: reader.bool("sdcppSupportsImageEdit"),
            recommended_for_scenes: reader.bool("sdcppRecommendedForScenes"),
            requires_reference_image: reader.bool("sdcppRequiresReferenceImage"),
        },
    }
}

fn feature_parameters(
    reader: &mut Reader<'_>,
    lossy: &mut Vec<String>,
    unknown: &mut Vec<String>,
) -> FeatureParameters {
    let mut slots = FeatureParameters::default();
    let Some(value) = reader.value("featureGenerationSettings") else {
        return slots;
    };
    let Some(map) = value.as_object() else {
        reader.lose("featureGenerationSettings");
        return slots;
    };
    for (name, slot) in map.iter().filter(|(_, slot)| !slot.is_null()) {
        let prefix = format!("featureGenerationSettings.{name}.");
        let Some(object) = slot.as_object() else {
            lossy.push(format!("featureGenerationSettings.{name}"));
            continue;
        };
        let target = match name.as_str() {
            "dynamicMemory" => &mut slots.dynamic_memory,
            "companionSoulWriter" => &mut slots.companion_soul_writer,
            "companionMemory" => &mut slots.companion_memory,
            "lorebookEntryGenerator" => &mut slots.lorebook_entry_generator,
            "lorebookGenerator" => &mut slots.lorebook_generator,
            "sceneWriter" => &mut slots.scene_writer,
            "helpMeReply" => &mut slots.help_me_reply,
            "groupSpeakerSelection" => &mut slots.group_speaker_selection,
            "creationHelper" => &mut slots.creation_helper,
            _ => {
                unknown.push(format!("featureGenerationSettings.{name}"));
                continue;
            }
        };
        let mut slot_reader = Reader::new(object, prefix, lossy);
        *target = feature_slot(&mut slot_reader);
        unknown.extend(slot_reader.unknown());
    }
    slots
}

/// Maps one legacy model's `advanced_model_settings` object into typed model
/// settings. Shared by the legacy backup planner and the live legacy database
/// preflight.
#[must_use]
pub fn legacy_model_parameters(
    provider_kind: &str,
    advanced: &Map<String, Value>,
) -> LegacyModelParameters {
    let mut lossy = Vec::new();
    let mut unknown = Vec::new();
    let (chat_parameters, llama_cpp, stable_diffusion, feature_parameters, top_unknown) = {
        let mut slot_lossy = Vec::new();
        let mut reader = Reader::new(advanced, String::new(), &mut lossy);
        let chat_parameters = chat_parameters(&mut reader, provider_kind);
        let llama_cpp = llama_cpp(&mut reader);
        let stable_diffusion = stable_diffusion(&mut reader);
        let feature_parameters = feature_parameters(&mut reader, &mut slot_lossy, &mut unknown);
        if reader.value("llamaLastRuntimeReport").is_some() {
            reader.lose("llamaLastRuntimeReport");
        }
        let top_unknown = reader.unknown();
        lossy_extend(&mut reader, slot_lossy);
        (
            chat_parameters,
            llama_cpp,
            stable_diffusion,
            feature_parameters,
            top_unknown,
        )
    };
    unknown.extend(top_unknown);
    lossy.sort();
    unknown.sort();
    LegacyModelParameters {
        chat_parameters,
        feature_parameters,
        llama_cpp,
        stable_diffusion,
        lossy_fields: lossy,
        unknown_fields: unknown,
    }
}

fn lossy_extend(reader: &mut Reader<'_>, fields: Vec<String>) {
    reader.lossy.extend(fields);
}

/// A legacy session's or the app's `advanced_model_settings` as a settings
/// layer. Legacy read fewer fields from these layers than from a model:
/// prompt caching and the OpenRouter pin only from the model, image generation
/// settings never, feature generation slots never, and the app layer never
/// supplied the thinking state, top-k, frequency and presence penalties or
/// reasoning. Such values are left out and returned with the
/// other lossy fields; unknown keys are returned separately.
pub(crate) fn legacy_settings_layer(
    object: &Map<String, Value>,
    app: bool,
) -> (lettuce_models::ModelSettingsLayer, Vec<String>, Vec<String>) {
    let parameters = legacy_model_parameters("", object);
    let present = |key: &str| object.get(key).is_some_and(|value| !value.is_null());
    let mut lossy = parameters.lossy_fields;
    let mut chat_parameters = parameters.chat_parameters;
    let mut llama_cpp = parameters.llama_cpp;
    let mut ignored = vec![
        "promptCachingEnabled",
        "promptCachingTtl",
        "openRouterProvider",
    ];
    chat_parameters.prompt_caching = None;
    chat_parameters.openrouter = OpenRouterOptions::default();
    if app {
        ignored.extend([
            "forceSendThinkingState",
            "topK",
            "frequencyPenalty",
            "presencePenalty",
            "reasoningEnabled",
            "reasoningEffort",
            "reasoningBudgetTokens",
            "forceGemma4Reasoning",
        ]);
        llama_cpp.force_gemma4_reasoning = None;
        chat_parameters.send_thinking_state = None;
        chat_parameters.top_k = None;
        chat_parameters.frequency_penalty = None;
        chat_parameters.presence_penalty = None;
        chat_parameters.reasoning_mode = None;
        chat_parameters.reasoning_effort = None;
        chat_parameters.reasoning_budget_tokens = None;
    }
    lossy.extend(
        ignored
            .into_iter()
            .chain(
                object
                    .keys()
                    .map(String::as_str)
                    .filter(|key| key.starts_with("sd")),
            )
            .chain(
                (!parameters.feature_parameters.is_empty()).then_some("featureGenerationSettings"),
            )
            .filter(|key| present(key) || *key == "featureGenerationSettings")
            .map(str::to_owned),
    );
    lossy.sort();
    lossy.dedup();
    (
        lettuce_models::ModelSettingsLayer {
            chat_parameters,
            llama_cpp,
            stable_diffusion: StableDiffusionSettings::default(),
        },
        lossy,
        parameters.unknown_fields,
    )
}

struct Writer(Map<String, Value>);

impl Writer {
    fn put(&mut self, key: &str, value: Option<impl Into<Value>>) {
        if let Some(value) = value {
            self.0.insert(key.to_owned(), value.into());
        }
    }

    fn put_override<T: Into<Value> + Clone>(&mut self, key: &str, value: &ParameterOverride<T>) {
        if let ParameterOverride::Set(value) = value {
            self.0.insert(key.to_owned(), value.clone().into());
        }
    }
}

fn sampler_profile_name(profile: LlamaSamplerProfile) -> &'static str {
    match profile {
        LlamaSamplerProfile::Balanced => "balanced",
        LlamaSamplerProfile::Creative => "creative",
        LlamaSamplerProfile::Stable => "stable",
        LlamaSamplerProfile::Reasoning => "reasoning",
    }
}

fn sampler_stage_name(stage: LlamaSamplerStage) -> &'static str {
    match stage {
        LlamaSamplerStage::Penalties => "penalties",
        LlamaSamplerStage::Grammar => "grammar",
        LlamaSamplerStage::TopK => "top_k",
        LlamaSamplerStage::TopP => "top_p",
        LlamaSamplerStage::MinP => "min_p",
        LlamaSamplerStage::Dry => "dry",
        LlamaSamplerStage::Typical => "typical",
        LlamaSamplerStage::Xtc => "xtc",
        LlamaSamplerStage::Temp => "temp",
        LlamaSamplerStage::AdaptiveP => "adaptive_p",
    }
}

fn write_sampler(writer: &mut Writer, sampler: &LlamaSamplerSettings) {
    writer.put(
        "llamaSamplerProfile",
        sampler.profile.map(sampler_profile_name),
    );
    writer.put(
        "llamaSamplerOrder",
        sampler.order.as_ref().map(|order| {
            order
                .iter()
                .map(|stage| Value::from(sampler_stage_name(*stage)))
                .collect::<Vec<_>>()
        }),
    );
    writer.put("llamaMinP", sampler.min_p);
    writer.put("llamaTypicalP", sampler.typical_p);
    writer.put("llamaRepeatPenalty", sampler.repeat_penalty);
    writer.put("llamaNPenRange", sampler.n_pen_range);
    writer.put("llamaDryMultiplier", sampler.dry_multiplier);
    writer.put("llamaDryBase", sampler.dry_base);
    writer.put("llamaDryAllowedLength", sampler.dry_allowed_length);
    writer.put("llamaDryPenaltyLastN", sampler.dry_penalty_last_n);
    writer.put(
        "llamaDrySequenceBreakers",
        sampler.dry_sequence_breakers.clone(),
    );
    writer.put("llamaXtcProbability", sampler.xtc_probability);
    writer.put("llamaXtcThreshold", sampler.xtc_threshold);
    writer.put("llamaSeed", sampler.seed);
    writer.put("llamaAdaptiveTarget", sampler.adaptive_target);
    writer.put("llamaAdaptiveDecay", sampler.adaptive_decay);
}

fn feature_slot_value(slot: &FeatureGenerationParameters) -> Option<Value> {
    let mut writer = Writer(Map::new());
    let parameters = &slot.parameters;
    writer.put_override("temperature", &parameters.temperature);
    writer.put_override("topP", &parameters.top_p);
    writer.put_override("topK", &parameters.top_k);
    writer.put_override("maxOutputTokens", &parameters.max_output_tokens);
    writer.put_override("frequencyPenalty", &parameters.frequency_penalty);
    writer.put_override("presencePenalty", &parameters.presence_penalty);
    writer.put_override("ollamaRepeatPenalty", &parameters.repetition_penalty);
    let ollama = &parameters.ollama;
    writer.put_override("ollamaTfsZ", &ollama.tfs_z);
    writer.put_override("ollamaTypicalP", &ollama.typical_p);
    writer.put_override("ollamaMinP", &ollama.min_p);
    writer.put_override("ollamaMirostat", &ollama.mirostat);
    writer.put_override("ollamaMirostatTau", &ollama.mirostat_tau);
    writer.put_override("ollamaMirostatEta", &ollama.mirostat_eta);
    writer.put_override("ollamaSeed", &ollama.seed);
    writer.put_override("ollamaStop", &ollama.stop);
    write_sampler(&mut writer, &slot.llama_sampler);
    (!writer.0.is_empty()).then_some(Value::Object(writer.0))
}

/// A model's typed settings back in the shape of legacy
/// `advancedModelSettings`: the reverse of [`legacy_model_parameters`] for
/// every value legacy could hold.
#[must_use]
pub fn legacy_advanced_model_settings(
    chat: &ChatParameterProfile,
    features: &FeatureParameters,
    llama: &LlamaCppSettings,
    sd: &StableDiffusionSettings,
) -> Map<String, Value> {
    let mut writer = Writer(Map::new());
    for key in [
        "temperature",
        "topP",
        "maxOutputTokens",
        "contextLength",
        "frequencyPenalty",
        "presencePenalty",
        "topK",
    ] {
        writer.0.insert(key.to_owned(), Value::Null);
    }
    writer.put("temperature", chat.temperature);
    writer.put("topP", chat.top_p);
    writer.put("topK", chat.top_k);
    writer.put("maxOutputTokens", chat.max_output_tokens);
    writer.put("contextLength", chat.context_length);
    writer.put("frequencyPenalty", chat.frequency_penalty);
    writer.put("presencePenalty", chat.presence_penalty);
    writer.put("ollamaRepeatPenalty", chat.repetition_penalty);
    writer.put(
        "reasoningEnabled",
        chat.reasoning_mode
            .map(|mode| mode == ReasoningMode::Enabled),
    );
    writer.put(
        "reasoningEffort",
        chat.reasoning_effort.map(|effort| match effort {
            ReasoningEffort::Low => "low",
            ReasoningEffort::Medium => "medium",
            ReasoningEffort::High => "high",
        }),
    );
    writer.put("reasoningBudgetTokens", chat.reasoning_budget_tokens);
    match chat.prompt_caching {
        Some(PromptCaching::Enabled { retention }) => {
            writer.put("promptCachingEnabled", Some(true));
            writer.put(
                "promptCachingTtl",
                Some(match retention {
                    PromptCacheRetention::InMemory => "in_memory",
                    PromptCacheRetention::FiveMinutes => "5min",
                    PromptCacheRetention::OneHour => "1h",
                    PromptCacheRetention::TwentyFourHours => "24h",
                }),
            );
        }
        Some(PromptCaching::Disabled) => writer.put("promptCachingEnabled", Some(false)),
        None => {}
    }
    writer.put("forceSendThinkingState", chat.send_thinking_state);
    let ollama = &chat.ollama;
    writer.put("ollamaNumKeep", ollama.num_keep);
    writer.put("ollamaNumBatch", ollama.num_batch);
    writer.put("ollamaNumGpu", ollama.num_gpu);
    writer.put("ollamaNumThread", ollama.num_thread);
    writer.put("ollamaTfsZ", ollama.tfs_z);
    writer.put("ollamaTypicalP", ollama.typical_p);
    writer.put("ollamaMinP", ollama.min_p);
    writer.put("ollamaMirostat", ollama.mirostat);
    writer.put("ollamaMirostatTau", ollama.mirostat_tau);
    writer.put("ollamaMirostatEta", ollama.mirostat_eta);
    writer.put("ollamaSeed", ollama.seed);
    writer.put("ollamaStop", ollama.stop.clone());
    writer.put(
        "openRouterProvider",
        chat.openrouter
            .pinned_provider
            .as_ref()
            .map(|id| serde_json::json!({ "id": id, "name": id })),
    );
    writer.put("llamaGpuLayers", llama.gpu_layers);
    writer.put("llamaMultiGpuEnabled", llama.multi_gpu_enabled);
    writer.put("llamaGpuDeviceIds", llama.gpu_device_ids.clone());
    writer.put(
        "llamaGpuDistributionMode",
        llama.gpu_distribution_mode.map(|mode| match mode {
            LlamaGpuDistributionMode::Balanced => "balanced",
            LlamaGpuDistributionMode::Proportional => "proportional",
            LlamaGpuDistributionMode::Priority => "priority",
            LlamaGpuDistributionMode::Manual => "manual",
        }),
    );
    writer.put(
        "llamaGpuManualLayers",
        llama.gpu_manual_layers.as_ref().map(|layers| {
            layers
                .iter()
                .map(|layer| serde_json::json!({"deviceId": layer.device_id, "layers": layer.layers}))
                .collect::<Vec<_>>()
        }),
    );
    writer.put("llamaCpuLayers", llama.cpu_layers);
    writer.put(
        "llamaKvPlacement",
        llama.kv_placement.map(|placement| match placement {
            LlamaKvPlacement::Auto => "auto",
            LlamaKvPlacement::Split => "split",
            LlamaKvPlacement::SystemRam => "systemRam",
            LlamaKvPlacement::Pin => "pin",
        }),
    );
    writer.put("llamaMainGpu", llama.main_gpu);
    writer.put("llamaSingleGpuDeviceId", llama.single_gpu_device_id);
    writer.put(
        "llamaPriorityVramLimitBytes",
        llama.priority_vram_limit_bytes,
    );
    writer.put("llamaThreads", llama.threads);
    writer.put("llamaThreadsBatch", llama.threads_batch);
    writer.put("llamaRopeFreqBase", llama.rope_freq_base);
    writer.put("llamaRopeFreqScale", llama.rope_freq_scale);
    writer.put("llamaOffloadKqv", llama.offload_kqv);
    writer.put("llamaBatchSize", llama.batch_size);
    writer.put("llamaUbatchSize", llama.ubatch_size);
    writer.put(
        "llamaKvType",
        llama
            .kv_type
            .and_then(|kv_type| serde_json::to_value(kv_type).ok()),
    );
    writer.put(
        "llamaFlashAttention",
        llama.flash_attention.map(|mode| match mode {
            LlamaFlashAttention::Auto => "auto",
            LlamaFlashAttention::Enabled => "enabled",
            LlamaFlashAttention::Disabled => "disabled",
        }),
    );
    writer.put("llamaSwaFull", llama.swa_full);
    writer.put(
        "llamaChatTemplateOverride",
        llama.chat_template_override.clone(),
    );
    writer.put(
        "llamaChatTemplatePreset",
        llama.chat_template_preset.clone(),
    );
    writer.put("llamaMmprojPath", llama.mmproj_path.clone());
    writer.put("llamaRawCompletionFallback", llama.raw_completion_fallback);
    writer.put("llamaStrictMode", llama.strict_mode);
    writer.put("llamaMtpEnabled", llama.mtp_enabled);
    writer.put(
        "llamaMtpPlacement",
        llama.mtp_placement.map(|placement| match placement {
            LlamaMtpPlacement::Auto => "auto",
            LlamaMtpPlacement::Gpu => "gpu",
            LlamaMtpPlacement::Cpu => "cpu",
        }),
    );
    writer.put("llamaMtpDraftTokens", llama.mtp_draft_tokens);
    writer.put("llamaMtpModelPath", llama.mtp_model_path.clone());
    writer.put("llamaStreamingEnabled", llama.streaming_enabled);
    writer.put("forceGemma4Reasoning", llama.force_gemma4_reasoning);
    write_sampler(&mut writer, &llama.sampler);
    writer.put("sdSteps", sd.steps);
    writer.put("sdCfgScale", sd.cfg_scale);
    writer.put("sdSampler", sd.sampler.clone());
    writer.put("sdScheduler", sd.scheduler.clone());
    writer.put("sdSeed", sd.seed);
    writer.put("sdNegativePrompt", sd.negative_prompt.clone());
    writer.put("sdDenoisingStrength", sd.denoising_strength);
    writer.put("sdImageCfgScale", sd.image_cfg_scale);
    writer.put("sdDistilledGuidance", sd.distilled_guidance);
    writer.put("sdEta", sd.eta);
    writer.put("sdFlowShift", sd.flow_shift);
    writer.put("sdSize", sd.size.clone());
    writer.put("sdVaeTilingEnabled", sd.vae_tiling_enabled);
    writer.put("sdVaeTileSizeX", sd.vae_tile_size_x);
    writer.put("sdVaeTileSizeY", sd.vae_tile_size_y);
    writer.put("sdVaeTileOverlap", sd.vae_tile_overlap);
    writer.put("sdAutoResizeRefImages", sd.auto_resize_reference_images);
    writer.put("sdIncreaseRefIndex", sd.increase_reference_index);
    writer.put("sdHiresEnabled", sd.hires_enabled);
    writer.put("sdHiresUpscaler", sd.hires_upscaler.clone());
    writer.put("sdHiresScale", sd.hires_scale);
    writer.put("sdHiresWidth", sd.hires_width);
    writer.put("sdHiresHeight", sd.hires_height);
    writer.put("sdHiresSteps", sd.hires_steps);
    writer.put("sdHiresDenoisingStrength", sd.hires_denoising_strength);
    writer.put("sdSlgScale", sd.slg_scale);
    writer.put("sdSlgLayers", sd.slg_layers.clone());
    writer.put("sdSlgLayerStart", sd.slg_layer_start);
    writer.put("sdSlgLayerEnd", sd.slg_layer_end);
    writer.put(
        "sdCacheMode",
        sd.cache_mode.map(|mode| match mode {
            StableDiffusionCacheMode::Disabled => "disabled",
            StableDiffusionCacheMode::Easycache => "easycache",
            StableDiffusionCacheMode::Ucache => "ucache",
            StableDiffusionCacheMode::Dbcache => "dbcache",
            StableDiffusionCacheMode::Taylorseer => "taylorseer",
            StableDiffusionCacheMode::CacheDit => "cache-dit",
            StableDiffusionCacheMode::Spectrum => "spectrum",
        }),
    );
    writer.put("sdCacheOption", sd.cache_option.clone());
    writer.put(
        "sdOffloadMode",
        sd.offload_mode.map(|mode| match mode {
            StableDiffusionOffloadMode::Auto => "auto",
            StableDiffusionOffloadMode::Gpu => "gpu",
            StableDiffusionOffloadMode::Mixed => "mixed",
        }),
    );
    writer.put("sdExtraPrompt", sd.extra_prompt.clone());
    writer.put(
        "sdPromptWriterInstructions",
        sd.prompt_writer_instructions.clone(),
    );
    writer.put(
        "sdBaseLoras",
        sd.base_loras.as_ref().map(|loras| {
            loras
                .iter()
                .map(|lora| {
                    serde_json::json!({
                        "path": lora.path,
                        "multiplier": lora.multiplier,
                        "isHighNoise": lora.is_high_noise,
                        "keywords": lora.keywords,
                    })
                })
                .collect::<Vec<_>>()
        }),
    );
    let cpp = &sd.cpp;
    writer.put("sdcppProfileId", cpp.profile_id.clone());
    writer.put("sdcppVariantId", cpp.variant_id.clone());
    writer.put("sdcppTextEncoderPath", cpp.text_encoder_path.clone());
    writer.put("sdcppVaePath", cpp.vae_path.clone());
    writer.put("sdcppVisionEncoderPath", cpp.vision_encoder_path.clone());
    writer.put("sdcppRuntimeRelease", cpp.runtime_release.clone());
    writer.put("sdcppRuntimeAsset", cpp.runtime_asset.clone());
    writer.put("sdcppRuntimeBackend", cpp.runtime_backend.clone());
    writer.put("sdcppMaxReferenceImages", cpp.max_reference_images);
    writer.put("sdcppSupportsLora", cpp.supports_lora);
    writer.put("sdcppSupportsTextToImage", cpp.supports_text_to_image);
    writer.put("sdcppSupportsImageEdit", cpp.supports_image_edit);
    writer.put("sdcppRecommendedForScenes", cpp.recommended_for_scenes);
    writer.put("sdcppRequiresReferenceImage", cpp.requires_reference_image);
    let slots = [
        ("dynamicMemory", &features.dynamic_memory),
        ("companionSoulWriter", &features.companion_soul_writer),
        ("companionMemory", &features.companion_memory),
        ("lorebookEntryGenerator", &features.lorebook_entry_generator),
        ("lorebookGenerator", &features.lorebook_generator),
        ("sceneWriter", &features.scene_writer),
        ("helpMeReply", &features.help_me_reply),
        ("groupSpeakerSelection", &features.group_speaker_selection),
        ("creationHelper", &features.creation_helper),
    ]
    .into_iter()
    .filter_map(|(name, slot)| Some((name.to_owned(), feature_slot_value(slot)?)))
    .collect::<Map<_, _>>();
    if !slots.is_empty() {
        writer
            .0
            .insert("featureGenerationSettings".to_owned(), Value::Object(slots));
    }
    writer.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn written_settings_read_back_as_the_same_settings() {
        let legacy: Value = serde_json::from_str(
            r#"{
            "temperature": 0.7, "topP": 0.9, "topK": 40, "maxOutputTokens": 1024,
            "contextLength": 8192, "frequencyPenalty": 0.2, "presencePenalty": -0.1,
            "ollamaRepeatPenalty": 1.1, "reasoningEnabled": true, "reasoningEffort": "high",
            "reasoningBudgetTokens": 2048, "promptCachingEnabled": true, "promptCachingTtl": "1h",
            "forceSendThinkingState": false, "ollamaNumKeep": 4, "ollamaNumBatch": 512,
            "ollamaNumGpu": 20, "ollamaNumThread": 8, "ollamaTfsZ": 0.9, "ollamaTypicalP": 0.8,
            "ollamaMinP": 0.05, "ollamaMirostat": 2, "ollamaMirostatTau": 5.0,
            "ollamaMirostatEta": 0.1, "ollamaSeed": 7, "ollamaStop": ["</s>"],
            "openRouterProvider": {"id": "Together"},
            "llamaGpuLayers": 33, "llamaMultiGpuEnabled": true, "llamaGpuDeviceIds": [0, 1],
            "llamaGpuDistributionMode": "manual",
            "llamaGpuManualLayers": [{"deviceId": 0, "layers": 20}],
            "llamaCpuLayers": 2, "llamaKvPlacement": "split", "llamaMainGpu": 0,
            "llamaSingleGpuDeviceId": 1, "llamaPriorityVramLimitBytes": 1000,
            "llamaThreads": 8, "llamaThreadsBatch": 8, "llamaRopeFreqBase": 10000.0,
            "llamaRopeFreqScale": 1.0, "llamaOffloadKqv": true, "llamaBatchSize": 512,
            "llamaUbatchSize": 256, "llamaKvType": "q8_0", "llamaFlashAttention": "auto",
            "llamaSwaFull": false, "llamaChatTemplateOverride": "{{x}}",
            "llamaChatTemplatePreset": "chatml", "llamaMmprojPath": "/m.gguf",
            "llamaRawCompletionFallback": true, "llamaStrictMode": false,
            "llamaMtpEnabled": true, "llamaMtpPlacement": "gpu", "llamaMtpDraftTokens": 3,
            "llamaMtpModelPath": "/mtp.gguf", "llamaStreamingEnabled": true,
            "llamaSamplerProfile": "creative", "llamaSamplerOrder": ["top_k", "temp"],
            "llamaMinP": 0.1, "llamaDryMultiplier": 0.8, "llamaDrySequenceBreakers": ["\\n"],
            "llamaSeed": 42,
            "sdSteps": 20, "sdCfgScale": 7.0, "sdSampler": "euler", "sdSeed": 3,
            "sdSize": "512x512", "sdCacheMode": "cache-dit", "sdOffloadMode": "mixed",
            "sdBaseLoras": [{"path": "/l.safetensors", "multiplier": 0.8, "isHighNoise": false, "keywords": ["x"]}],
            "sdcppProfileId": "flux", "sdcppSupportsLora": true,
            "featureGenerationSettings": {
                "dynamicMemory": {"temperature": 0.4, "llamaXtcProbability": 0.2, "ollamaStop": ["x"]},
                "creationHelper": {"topP": 0.5}
            }
        }"#,
        )
        .expect("legacy settings");
        let object = legacy.as_object().expect("object");
        let first = legacy_model_parameters("openai", object);
        assert!(first.lossy_fields.is_empty(), "{:?}", first.lossy_fields);
        assert!(
            first.unknown_fields.is_empty(),
            "{:?}",
            first.unknown_fields
        );
        let written = legacy_advanced_model_settings(
            &first.chat_parameters,
            &first.feature_parameters,
            &first.llama_cpp,
            &first.stable_diffusion,
        );
        let second = legacy_model_parameters("openai", &written);
        assert!(second.lossy_fields.is_empty(), "{:?}", second.lossy_fields);
        assert!(
            second.unknown_fields.is_empty(),
            "{:?}",
            second.unknown_fields
        );
        assert_eq!(second, first);
    }
}
