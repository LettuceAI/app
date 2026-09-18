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
    match value {
        "penalties" => Some(LlamaSamplerStage::Penalties),
        "grammar" => Some(LlamaSamplerStage::Grammar),
        "top_k" => Some(LlamaSamplerStage::TopK),
        "top_p" => Some(LlamaSamplerStage::TopP),
        "min_p" => Some(LlamaSamplerStage::MinP),
        "dry" => Some(LlamaSamplerStage::Dry),
        "typical" => Some(LlamaSamplerStage::Typical),
        "xtc" => Some(LlamaSamplerStage::Xtc),
        "temp" => Some(LlamaSamplerStage::Temp),
        _ => None,
    }
}

fn llama_sampler(reader: &mut Reader<'_>) -> LlamaSamplerSettings {
    LlamaSamplerSettings {
        profile: reader.choice("llamaSamplerProfile", sampler_profile),
        order: reader.parse("llamaSamplerOrder", |value| {
            value
                .as_array()?
                .iter()
                .map(|stage| stage.as_str().and_then(sampler_stage))
                .collect()
        }),
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
                stop: set(reader.texts("ollamaStop", false)),
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
    let reasoning_budget_tokens = reader.u32("reasoningBudgetTokens", 1, u32::MAX);
    let enabled_reasoning = reasoning_mode != Some(ReasoningMode::Disabled);
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
    let max_output_tokens = reader.u32("maxOutputTokens", 0, u32::MAX);
    let num_predict = reader.u32("ollamaNumPredict", 0, u32::MAX);
    let context_length = reader.u32("contextLength", 0, u32::MAX);
    let num_ctx = reader.u32("ollamaNumCtx", 0, u32::MAX);
    ChatParameterProfile {
        temperature: reader.f64("temperature", 0.0, 2.0),
        top_p: reader.f64("topP", 0.0, 1.0),
        top_k: reader.u32("topK", 1, u32::MAX),
        max_output_tokens: max_output_tokens
            .or(num_predict)
            .filter(|value| *value != 0),
        context_length: context_length.or(num_ctx).filter(|value| *value != 0),
        frequency_penalty: reader.f64("frequencyPenalty", -2.0, 2.0),
        presence_penalty: reader.f64("presencePenalty", -2.0, 2.0),
        repetition_penalty: reader.f64("ollamaRepeatPenalty", f64::MIN_POSITIVE, 2.0),
        reasoning_mode,
        reasoning_effort: reasoning_effort.filter(|_| enabled_reasoning),
        reasoning_budget_tokens: reasoning_budget_tokens.filter(|_| enabled_reasoning),
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
            stop: reader.texts("ollamaStop", false),
        },
        openrouter: OpenRouterOptions {
            pinned_provider: reader.parse("openRouterProvider", |value| {
                value
                    .get("id")?
                    .as_str()
                    .map(str::trim)
                    .filter(|id| !id.is_empty() && id.len() <= 256)
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
                        is_high_noise: lora
                            .get("isHighNoise")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
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
