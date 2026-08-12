use serde_json::Value;
use std::collections::HashMap;

use super::types::{GpuLayerAssignment, Model, Session, Settings};

const FALLBACK_MAX_OUTPUT_TOKENS: u32 = 4096;
const DEFAULT_LLAMA_SAMPLER_PROFILE: &str = "balanced";

#[derive(Clone, Copy)]
pub(super) struct LlamaSamplerProfileDefaults {
    pub(super) name: &'static str,
    pub(super) temperature: f64,
    pub(super) top_p: f64,
    pub(super) top_k: Option<u32>,
    pub(super) min_p: Option<f64>,
    pub(super) typical_p: Option<f64>,
    pub(super) frequency_penalty: Option<f64>,
    pub(super) presence_penalty: Option<f64>,
}

pub(super) fn is_llama_cpp_model(model: &Model) -> bool {
    model.provider_id.eq_ignore_ascii_case("llamacpp")
}

fn normalize_llama_sampler_profile(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "balanced" | "creative" | "stable" | "reasoning" => Some(normalized),
        _ => None,
    }
}

fn decode_llama_sequence_breaker(value: &str) -> String {
    match value.trim() {
        "\\n" => "\n".to_string(),
        "\\r" => "\r".to_string(),
        "\\t" => "\t".to_string(),
        "\\\"" => "\"".to_string(),
        "\\\\" => "\\".to_string(),
        other => other.to_string(),
    }
}

pub(super) fn llama_sampler_profile_defaults(profile: Option<&str>) -> LlamaSamplerProfileDefaults {
    match profile.unwrap_or(DEFAULT_LLAMA_SAMPLER_PROFILE) {
        "creative" => LlamaSamplerProfileDefaults {
            name: "creative",
            temperature: 0.95,
            top_p: 0.98,
            top_k: Some(80),
            min_p: Some(0.02),
            typical_p: None,
            frequency_penalty: Some(0.0),
            presence_penalty: Some(0.25),
        },
        "stable" => LlamaSamplerProfileDefaults {
            name: "stable",
            temperature: 0.55,
            top_p: 0.90,
            top_k: Some(32),
            min_p: Some(0.08),
            typical_p: Some(0.97),
            frequency_penalty: Some(0.2),
            presence_penalty: Some(0.0),
        },
        "reasoning" => LlamaSamplerProfileDefaults {
            name: "reasoning",
            temperature: 0.35,
            top_p: 0.90,
            top_k: Some(24),
            min_p: None,
            typical_p: Some(0.95),
            frequency_penalty: Some(0.1),
            presence_penalty: Some(0.0),
        },
        _ => LlamaSamplerProfileDefaults {
            name: "balanced",
            temperature: 0.8,
            top_p: 0.95,
            top_k: Some(40),
            min_p: Some(0.05),
            typical_p: None,
            frequency_penalty: Some(0.15),
            presence_penalty: Some(0.0),
        },
    }
}

pub(super) fn resolve_llama_sampler_profile(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<String> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_sampler_profile.clone())
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_sampler_profile.clone())
        })
        .or_else(|| {
            settings
                .advanced_model_settings
                .llama_sampler_profile
                .clone()
        })
        .and_then(|value| normalize_llama_sampler_profile(&value))
}

pub(super) fn resolve_llama_sampler_order(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<Vec<String>> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_sampler_order.clone())
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_sampler_order.clone())
        })
        .or_else(|| settings.advanced_model_settings.llama_sampler_order.clone())
}

pub(super) fn resolve_temperature(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<f64> {
    if let Some(value) = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.temperature)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.temperature)
        })
        .or(settings.advanced_model_settings.temperature)
    {
        return Some(value);
    }
    if is_llama_cpp_model(model) {
        return Some(
            llama_sampler_profile_defaults(
                resolve_llama_sampler_profile(session, model, settings).as_deref(),
            )
            .temperature,
        );
    }
    None
}

pub(super) fn resolve_top_p(session: &Session, model: &Model, settings: &Settings) -> Option<f64> {
    if let Some(value) = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.top_p)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.top_p)
        })
        .or(settings.advanced_model_settings.top_p)
    {
        return Some(value);
    }
    if is_llama_cpp_model(model) {
        return Some(
            llama_sampler_profile_defaults(
                resolve_llama_sampler_profile(session, model, settings).as_deref(),
            )
            .top_p,
        );
    }
    None
}

pub(super) fn resolve_max_tokens(session: &Session, model: &Model, settings: &Settings) -> u32 {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.max_output_tokens)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.max_output_tokens)
        })
        .or(settings.advanced_model_settings.max_output_tokens)
        .unwrap_or(FALLBACK_MAX_OUTPUT_TOKENS)
}

pub(super) fn resolve_context_length(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<u32> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.context_length)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.context_length)
        })
        .or(settings.advanced_model_settings.context_length)
        .filter(|v| *v > 0)
}

pub(super) fn resolve_frequency_penalty(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<f64> {
    let configured = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.frequency_penalty)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.frequency_penalty)
        });
    if configured.is_some() {
        return configured;
    }
    if is_llama_cpp_model(model) {
        return llama_sampler_profile_defaults(
            resolve_llama_sampler_profile(session, model, settings).as_deref(),
        )
        .frequency_penalty;
    }
    None
}

pub(super) fn resolve_presence_penalty(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<f64> {
    let configured = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.presence_penalty)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.presence_penalty)
        });
    if configured.is_some() {
        return configured;
    }
    if is_llama_cpp_model(model) {
        return llama_sampler_profile_defaults(
            resolve_llama_sampler_profile(session, model, settings).as_deref(),
        )
        .presence_penalty;
    }
    None
}

pub(super) fn resolve_top_k(session: &Session, model: &Model, settings: &Settings) -> Option<u32> {
    let configured = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.top_k)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.top_k)
        });
    if configured.is_some() {
        return configured;
    }
    if is_llama_cpp_model(model) {
        return llama_sampler_profile_defaults(
            resolve_llama_sampler_profile(session, model, settings).as_deref(),
        )
        .top_k;
    }
    None
}

pub(super) fn resolve_llama_gpu_layers(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<u32> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_gpu_layers)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_gpu_layers)
        })
        .or(settings.advanced_model_settings.llama_gpu_layers)
}

pub(super) fn resolve_llama_multi_gpu_enabled(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<bool> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_multi_gpu_enabled)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_multi_gpu_enabled)
        })
        .or(settings.advanced_model_settings.llama_multi_gpu_enabled)
}

pub(super) fn resolve_llama_gpu_device_ids(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<Vec<usize>> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_gpu_device_ids.clone())
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_gpu_device_ids.clone())
        })
        .or_else(|| {
            settings
                .advanced_model_settings
                .llama_gpu_device_ids
                .clone()
        })
        .map(|ids| {
            let mut deduped = Vec::new();
            for id in ids {
                if !deduped.contains(&id) {
                    deduped.push(id);
                }
            }
            deduped
        })
        .filter(|ids| !ids.is_empty())
}

pub(super) fn resolve_llama_gpu_distribution_mode(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<String> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_gpu_distribution_mode.clone())
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_gpu_distribution_mode.clone())
        })
        .or_else(|| {
            settings
                .advanced_model_settings
                .llama_gpu_distribution_mode
                .clone()
        })
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| {
            matches!(
                v.as_str(),
                "balanced" | "proportional" | "priority" | "manual"
            )
        })
}

pub(super) fn resolve_llama_gpu_manual_layers(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<Vec<GpuLayerAssignment>> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_gpu_manual_layers.clone())
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_gpu_manual_layers.clone())
        })
        .or_else(|| {
            settings
                .advanced_model_settings
                .llama_gpu_manual_layers
                .clone()
        })
        .filter(|layers| !layers.is_empty())
}

pub(super) fn resolve_llama_kv_placement(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<String> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_kv_placement.clone())
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_kv_placement.clone())
        })
        .or_else(|| settings.advanced_model_settings.llama_kv_placement.clone())
        .map(|v| v.trim().to_string())
        .filter(|v| matches!(v.as_str(), "auto" | "split" | "systemRam" | "pin"))
}

pub(super) fn resolve_llama_main_gpu(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<i32> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_main_gpu)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_main_gpu)
        })
        .or(settings.advanced_model_settings.llama_main_gpu)
}

pub(super) fn resolve_llama_multi_gpu_enabled_leveled(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<(bool, u8)> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_multi_gpu_enabled)
        .map(|value| (value, 2))
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_multi_gpu_enabled)
                .map(|value| (value, 1))
        })
        .or(settings
            .advanced_model_settings
            .llama_multi_gpu_enabled
            .map(|value| (value, 0)))
}

pub(super) fn resolve_llama_single_gpu_device_id_leveled(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<(usize, u8)> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_single_gpu_device_id)
        .map(|value| (value, 2))
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_single_gpu_device_id)
                .map(|value| (value, 1))
        })
        .or(settings
            .advanced_model_settings
            .llama_single_gpu_device_id
            .map(|value| (value, 0)))
}

pub(crate) fn llama_pin_overridden_by_multi_gpu(
    multi_gpu: Option<(bool, u8)>,
    pin: Option<(usize, u8)>,
) -> bool {
    matches!(
        (multi_gpu, pin),
        (Some((true, multi_level)), Some((_, pin_level))) if multi_level >= pin_level
    )
}

pub(super) fn resolve_llama_priority_vram_limit_bytes(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<u64> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_priority_vram_limit_bytes)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_priority_vram_limit_bytes)
        })
        .or(settings
            .advanced_model_settings
            .llama_priority_vram_limit_bytes)
        .filter(|v| *v > 0)
}

pub(super) fn resolve_llama_threads(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<u32> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_threads)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_threads)
        })
        .or(settings.advanced_model_settings.llama_threads)
}

pub(super) fn resolve_llama_threads_batch(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<u32> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_threads_batch)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_threads_batch)
        })
        .or(settings.advanced_model_settings.llama_threads_batch)
}

pub(super) fn resolve_llama_seed(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<u32> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_seed)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_seed)
        })
        .or(settings.advanced_model_settings.llama_seed)
}

pub(super) fn resolve_llama_rope_freq_base(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<f64> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_rope_freq_base)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_rope_freq_base)
        })
        .or(settings.advanced_model_settings.llama_rope_freq_base)
}

pub(super) fn resolve_llama_rope_freq_scale(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<f64> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_rope_freq_scale)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_rope_freq_scale)
        })
        .or(settings.advanced_model_settings.llama_rope_freq_scale)
}

pub(super) fn resolve_llama_offload_kqv(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<bool> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_offload_kqv)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_offload_kqv)
        })
        .or(settings.advanced_model_settings.llama_offload_kqv)
}

pub(super) fn resolve_llama_swa_full(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<bool> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_swa_full)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_swa_full)
        })
        .or(settings.advanced_model_settings.llama_swa_full)
}

pub(super) fn resolve_llama_batch_size(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<u32> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_batch_size)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_batch_size)
        })
        .or(settings.advanced_model_settings.llama_batch_size)
        .filter(|v| *v > 0)
}

pub(super) fn resolve_llama_ubatch_size(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<u32> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_ubatch_size)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_ubatch_size)
        })
        .or(settings.advanced_model_settings.llama_ubatch_size)
        .filter(|v| *v > 0)
}

pub(super) fn resolve_llama_kv_type(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<String> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_kv_type.clone())
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_kv_type.clone())
        })
        .or_else(|| settings.advanced_model_settings.llama_kv_type.clone())
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
}

pub(super) fn resolve_llama_mtp_enabled(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<bool> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_mtp_enabled)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_mtp_enabled)
        })
        .or(settings.advanced_model_settings.llama_mtp_enabled)
}

pub(super) fn resolve_llama_mtp_placement(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<String> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_mtp_placement.clone())
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_mtp_placement.clone())
        })
        .or_else(|| settings.advanced_model_settings.llama_mtp_placement.clone())
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| matches!(value.as_str(), "auto" | "gpu" | "cpu"))
}

pub(super) fn resolve_llama_mtp_draft_tokens(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<u32> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_mtp_draft_tokens)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_mtp_draft_tokens)
        })
        .or(settings.advanced_model_settings.llama_mtp_draft_tokens)
        .filter(|v| *v > 0)
}

pub(super) fn resolve_llama_mtp_model_path(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<String> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_mtp_model_path.clone())
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_mtp_model_path.clone())
        })
        .or_else(|| {
            settings
                .advanced_model_settings
                .llama_mtp_model_path
                .clone()
        })
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

pub(super) fn resolve_llama_flash_attention(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<String> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_flash_attention.clone())
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_flash_attention.clone())
        })
        .or_else(|| {
            settings
                .advanced_model_settings
                .llama_flash_attention
                .clone()
        })
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
}

pub(super) fn resolve_llama_chat_template_override(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<String> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_chat_template_override.clone())
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_chat_template_override.clone())
        })
        .or_else(|| {
            settings
                .advanced_model_settings
                .llama_chat_template_override
                .clone()
        })
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

pub(super) fn resolve_llama_mmproj_path(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<String> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_mmproj_path.clone())
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_mmproj_path.clone())
        })
        .or_else(|| settings.advanced_model_settings.llama_mmproj_path.clone())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

pub(super) fn resolve_llama_chat_template_preset(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<String> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_chat_template_preset.clone())
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_chat_template_preset.clone())
        })
        .or_else(|| {
            settings
                .advanced_model_settings
                .llama_chat_template_preset
                .clone()
        })
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

pub(super) fn resolve_llama_raw_completion_fallback(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<bool> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_raw_completion_fallback)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_raw_completion_fallback)
        })
        .or(settings
            .advanced_model_settings
            .llama_raw_completion_fallback)
}

pub(super) fn resolve_llama_streaming_enabled(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<bool> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_streaming_enabled)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_streaming_enabled)
        })
        .or(settings.advanced_model_settings.llama_streaming_enabled)
}

pub(super) fn resolve_llama_strict_mode(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<bool> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_strict_mode)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_strict_mode)
        })
        .or(settings.advanced_model_settings.llama_strict_mode)
}

pub(super) fn resolve_llama_profile_min_p(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<f64> {
    if let Some(value) = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_min_p)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_min_p)
        })
        .or(settings.advanced_model_settings.llama_min_p)
    {
        return Some(value);
    }
    if !is_llama_cpp_model(model) {
        return None;
    }
    llama_sampler_profile_defaults(
        resolve_llama_sampler_profile(session, model, settings).as_deref(),
    )
    .min_p
}

pub(super) fn resolve_llama_profile_typical_p(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<f64> {
    if let Some(value) = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_typical_p)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_typical_p)
        })
        .or(settings.advanced_model_settings.llama_typical_p)
    {
        return Some(value);
    }
    if !is_llama_cpp_model(model) {
        return None;
    }
    llama_sampler_profile_defaults(
        resolve_llama_sampler_profile(session, model, settings).as_deref(),
    )
    .typical_p
}

pub(super) fn resolve_llama_repeat_penalty(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<f64> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_repeat_penalty)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_repeat_penalty)
        })
        .or(settings.advanced_model_settings.llama_repeat_penalty)
}

pub(super) fn resolve_llama_n_pen_range(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<i32> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_n_pen_range)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_n_pen_range)
        })
        .or(settings.advanced_model_settings.llama_n_pen_range)
}

pub(super) fn resolve_llama_dry_multiplier(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<f64> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_dry_multiplier)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_dry_multiplier)
        })
        .or(settings.advanced_model_settings.llama_dry_multiplier)
}

pub(super) fn resolve_llama_dry_base(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<f64> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_dry_base)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_dry_base)
        })
        .or(settings.advanced_model_settings.llama_dry_base)
}

pub(super) fn resolve_llama_xtc_probability(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<f64> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_xtc_probability)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_xtc_probability)
        })
        .or(settings.advanced_model_settings.llama_xtc_probability)
}

pub(super) fn resolve_llama_xtc_threshold(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<f64> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_xtc_threshold)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_xtc_threshold)
        })
        .or(settings.advanced_model_settings.llama_xtc_threshold)
}

pub(super) fn resolve_llama_dry_allowed_length(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<u32> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_dry_allowed_length)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_dry_allowed_length)
        })
        .or(settings.advanced_model_settings.llama_dry_allowed_length)
}

pub(super) fn resolve_llama_dry_penalty_last_n(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<i32> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_dry_penalty_last_n)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_dry_penalty_last_n)
        })
        .or(settings.advanced_model_settings.llama_dry_penalty_last_n)
}

pub(super) fn resolve_llama_dry_sequence_breakers(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<Vec<String>> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_dry_sequence_breakers.clone())
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_dry_sequence_breakers.clone())
        })
        .or_else(|| {
            settings
                .advanced_model_settings
                .llama_dry_sequence_breakers
                .clone()
        })
        .map(|values| {
            values
                .into_iter()
                .map(|value| decode_llama_sequence_breaker(&value))
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .filter(|values| !values.is_empty())
}

pub(crate) fn prepare_feature_request(
    provider_id: &str,
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> (RequestSettings, Option<HashMap<String, Value>>) {
    let mut request_settings = RequestSettings::resolve(session, model, settings);
    request_settings.reasoning_enabled = false;
    request_settings.reasoning_effort = None;
    request_settings.reasoning_budget = None;
    let extra_body_fields =
        build_provider_extra_fields(provider_id, session, model, settings, &request_settings);

    (request_settings, extra_body_fields)
}

mod model_resolution;

pub(crate) use model_resolution::find_model_with_credential;

mod provider_fields;
pub(crate) use provider_fields::{build_provider_extra_fields, RequestSettings};

#[cfg(test)]
mod gpu_pin_tests {
    use super::llama_pin_overridden_by_multi_gpu;

    #[test]
    fn multi_gpu_at_same_or_higher_level_suppresses_pin() {
        assert!(llama_pin_overridden_by_multi_gpu(
            Some((true, 0)),
            Some((1, 0))
        ));
        assert!(llama_pin_overridden_by_multi_gpu(
            Some((true, 1)),
            Some((1, 0))
        ));
        assert!(llama_pin_overridden_by_multi_gpu(
            Some((true, 2)),
            Some((1, 1))
        ));
    }

    #[test]
    fn more_specific_pin_beats_multi_gpu() {
        assert!(!llama_pin_overridden_by_multi_gpu(
            Some((true, 0)),
            Some((1, 1))
        ));
        assert!(!llama_pin_overridden_by_multi_gpu(
            Some((true, 1)),
            Some((1, 2))
        ));
    }

    #[test]
    fn disabled_or_absent_multi_gpu_never_suppresses() {
        assert!(!llama_pin_overridden_by_multi_gpu(
            Some((false, 2)),
            Some((1, 0))
        ));
        assert!(!llama_pin_overridden_by_multi_gpu(None, Some((1, 0))));
        assert!(!llama_pin_overridden_by_multi_gpu(Some((true, 2)), None));
    }
}
