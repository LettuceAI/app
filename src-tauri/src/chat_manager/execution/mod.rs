use serde_json::Value;
use std::collections::HashMap;

use super::types::{Model, Session, Settings};

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

pub(crate) fn prepare_sampling_request(
    provider_id: &str,
    session: &Session,
    model: &Model,
    settings: &Settings,
    max_tokens: u32,
    temperature: f64,
    top_p: f64,
    top_k: Option<u32>,
    frequency_penalty: Option<f64>,
    presence_penalty: Option<f64>,
) -> (RequestSettings, Option<HashMap<String, Value>>) {
    let model_request_settings = RequestSettings::resolve(session, model, settings);
    let request_settings = RequestSettings::for_sampling(
        model_request_settings.context_length,
        max_tokens,
        temperature,
        top_p,
        top_k,
        frequency_penalty,
        presence_penalty,
        model_request_settings.prompt_caching_enabled,
    );
    let extra_body_fields =
        build_provider_extra_fields(provider_id, session, model, settings, &request_settings);

    (request_settings, extra_body_fields)
}

pub(crate) fn prepare_default_sampling_request(
    provider_id: &str,
    session: &Session,
    model: &Model,
    settings: &Settings,
    temperature: f64,
    top_p: f64,
    top_k: Option<u32>,
    frequency_penalty: Option<f64>,
    presence_penalty: Option<f64>,
) -> (RequestSettings, Option<HashMap<String, Value>>) {
    let model_request_settings = RequestSettings::resolve(session, model, settings);
    prepare_sampling_request(
        provider_id,
        session,
        model,
        settings,
        model_request_settings.max_tokens,
        temperature,
        top_p,
        top_k,
        frequency_penalty,
        presence_penalty,
    )
}

mod fallback;

pub(crate) use fallback::{
    build_model_attempts, emit_fallback_retry_toast, find_model_with_credential,
};

mod provider_fields;
pub(crate) use provider_fields::{build_provider_extra_fields, RequestSettings};
