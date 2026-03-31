use serde_json::{json, Map, Value};
use std::collections::HashMap;

use crate::chat_manager::types::{Model, Session, Settings};

use super::{
    is_llama_cpp_model, llama_sampler_profile_defaults, resolve_context_length,
    resolve_frequency_penalty, resolve_llama_batch_size, resolve_llama_chat_template_override,
    resolve_llama_chat_template_preset, resolve_llama_flash_attention, resolve_llama_gpu_layers,
    resolve_llama_kv_type, resolve_llama_mmproj_path, resolve_llama_offload_kqv,
    resolve_llama_profile_min_p, resolve_llama_profile_typical_p,
    resolve_llama_raw_completion_fallback, resolve_llama_rope_freq_base,
    resolve_llama_rope_freq_scale, resolve_llama_sampler_profile, resolve_llama_seed,
    resolve_llama_threads, resolve_llama_threads_batch, resolve_max_tokens,
    resolve_presence_penalty, resolve_temperature, resolve_top_k, resolve_top_p,
};

fn build_llama_extra_fields(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<HashMap<String, Value>> {
    let mut extra = HashMap::new();
    let sampler_profile = if is_llama_cpp_model(model) {
        Some(
            llama_sampler_profile_defaults(
                resolve_llama_sampler_profile(session, model, settings).as_deref(),
            )
            .name
            .to_string(),
        )
    } else {
        None
    };
    if let Some(v) = resolve_llama_gpu_layers(session, model, settings) {
        extra.insert("llamaGpuLayers".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_threads(session, model, settings) {
        extra.insert("llamaThreads".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_threads_batch(session, model, settings) {
        extra.insert("llamaThreadsBatch".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_seed(session, model, settings) {
        extra.insert("llamaSeed".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_rope_freq_base(session, model, settings) {
        extra.insert("llamaRopeFreqBase".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_rope_freq_scale(session, model, settings) {
        extra.insert("llamaRopeFreqScale".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_offload_kqv(session, model, settings) {
        extra.insert("llamaOffloadKqv".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_batch_size(session, model, settings) {
        extra.insert("llamaBatchSize".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_kv_type(session, model, settings) {
        extra.insert("llamaKvType".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_flash_attention(session, model, settings) {
        extra.insert("llamaFlashAttentionPolicy".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_chat_template_override(session, model, settings) {
        extra.insert("llamaChatTemplateOverride".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_mmproj_path(session, model, settings) {
        extra.insert("llamaMmprojPath".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_chat_template_preset(session, model, settings) {
        extra.insert("llamaChatTemplatePreset".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_raw_completion_fallback(session, model, settings) {
        extra.insert("llamaRawCompletionFallback".to_string(), json!(v));
    }
    if let Some(v) = sampler_profile {
        extra.insert("llamaSamplerProfile".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_profile_min_p(session, model, settings) {
        extra.insert("llamaMinP".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_profile_typical_p(session, model, settings) {
        extra.insert("llamaTypicalP".to_string(), json!(v));
    }

    if extra.is_empty() {
        None
    } else {
        Some(extra)
    }
}

fn build_ollama_extra_fields(
    session: &Session,
    model: &Model,
    settings: &Settings,
    request_settings: &RequestSettings,
) -> Option<HashMap<String, Value>> {
    let mut options = Map::new();

    let num_ctx = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_num_ctx)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_num_ctx)
        })
        .or(settings.advanced_model_settings.ollama_num_ctx)
        .or(request_settings.context_length);
    let num_predict = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_num_predict)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_num_predict)
        })
        .or(settings.advanced_model_settings.ollama_num_predict)
        .or(Some(request_settings.max_tokens));
    let num_keep = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_num_keep)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_num_keep)
        })
        .or(settings.advanced_model_settings.ollama_num_keep);
    let num_batch = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_num_batch)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_num_batch)
        })
        .or(settings.advanced_model_settings.ollama_num_batch);
    let num_gpu = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_num_gpu)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_num_gpu)
        })
        .or(settings.advanced_model_settings.ollama_num_gpu);
    let num_thread = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_num_thread)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_num_thread)
        })
        .or(settings.advanced_model_settings.ollama_num_thread);
    let tfs_z = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_tfs_z)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_tfs_z)
        })
        .or(settings.advanced_model_settings.ollama_tfs_z);
    let typical_p = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_typical_p)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_typical_p)
        })
        .or(settings.advanced_model_settings.ollama_typical_p);
    let min_p = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_min_p)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_min_p)
        })
        .or(settings.advanced_model_settings.ollama_min_p);
    let mirostat = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_mirostat)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_mirostat)
        })
        .or(settings.advanced_model_settings.ollama_mirostat);
    let mirostat_tau = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_mirostat_tau)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_mirostat_tau)
        })
        .or(settings.advanced_model_settings.ollama_mirostat_tau);
    let mirostat_eta = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_mirostat_eta)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_mirostat_eta)
        })
        .or(settings.advanced_model_settings.ollama_mirostat_eta);
    let repeat_penalty = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_repeat_penalty)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_repeat_penalty)
        })
        .or(settings.advanced_model_settings.ollama_repeat_penalty);
    let seed = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_seed)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_seed)
        })
        .or(settings.advanced_model_settings.ollama_seed);
    let stop = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_stop.clone())
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_stop.clone())
        })
        .or(settings.advanced_model_settings.ollama_stop.clone());

    if let Some(v) = request_settings.temperature {
        options.insert("temperature".into(), json!(v));
    }
    if let Some(v) = request_settings.top_p {
        options.insert("top_p".into(), json!(v));
    }
    if let Some(v) = request_settings.top_k {
        options.insert("top_k".into(), json!(v));
    }
    if let Some(v) = request_settings.frequency_penalty {
        options.insert("frequency_penalty".into(), json!(v));
    }
    if let Some(v) = request_settings.presence_penalty {
        options.insert("presence_penalty".into(), json!(v));
    }
    if let Some(v) = num_ctx {
        options.insert("num_ctx".into(), json!(v));
    }
    if let Some(v) = num_predict {
        options.insert("num_predict".into(), json!(v));
    }
    if let Some(v) = num_keep {
        options.insert("num_keep".into(), json!(v));
    }
    if let Some(v) = num_batch {
        options.insert("num_batch".into(), json!(v));
    }
    if let Some(v) = num_gpu {
        options.insert("num_gpu".into(), json!(v));
    }
    if let Some(v) = num_thread {
        options.insert("num_thread".into(), json!(v));
    }
    if let Some(v) = tfs_z {
        options.insert("tfs_z".into(), json!(v));
    }
    if let Some(v) = typical_p {
        options.insert("typical_p".into(), json!(v));
    }
    if let Some(v) = min_p {
        options.insert("min_p".into(), json!(v));
    }
    if let Some(v) = mirostat {
        options.insert("mirostat".into(), json!(v));
    }
    if let Some(v) = mirostat_tau {
        options.insert("mirostat_tau".into(), json!(v));
    }
    if let Some(v) = mirostat_eta {
        options.insert("mirostat_eta".into(), json!(v));
    }
    if let Some(v) = repeat_penalty {
        options.insert("repeat_penalty".into(), json!(v));
    }
    if let Some(v) = seed {
        options.insert("seed".into(), json!(v));
    }
    if let Some(v) = stop {
        options.insert("stop".into(), json!(v));
    }

    let mut extra = HashMap::new();
    extra.insert("options".to_string(), Value::Object(options));
    Some(extra)
}

fn resolve_reasoning_enabled(session: &Session, model: &Model, _settings: &Settings) -> bool {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.reasoning_enabled)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.reasoning_enabled)
        })
        .unwrap_or(false)
}

fn resolve_reasoning_effort(
    session: &Session,
    model: &Model,
    _settings: &Settings,
) -> Option<String> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.reasoning_effort.clone())
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.reasoning_effort.clone())
        })
}

fn resolve_reasoning_budget(
    session: &Session,
    model: &Model,
    _settings: &Settings,
    reasoning_effort: Option<&str>,
) -> Option<u32> {
    let explicit_budget = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.reasoning_budget_tokens)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.reasoning_budget_tokens)
        });

    if explicit_budget.is_some() {
        return explicit_budget;
    }

    reasoning_effort.map(|effort| match effort {
        "low" => 2048,
        "medium" => 8192,
        "high" => 16384,
        _ => 4096,
    })
}

#[derive(Clone, Debug)]
pub(crate) struct RequestSettings {
    pub(crate) temperature: Option<f64>,
    pub(crate) top_p: Option<f64>,
    pub(crate) max_tokens: u32,
    pub(crate) context_length: Option<u32>,
    pub(crate) frequency_penalty: Option<f64>,
    pub(crate) presence_penalty: Option<f64>,
    pub(crate) top_k: Option<u32>,
    pub(crate) reasoning_enabled: bool,
    pub(crate) reasoning_effort: Option<String>,
    pub(crate) reasoning_budget: Option<u32>,
    pub(crate) prompt_caching_enabled: Option<bool>,
}

impl RequestSettings {
    pub(crate) fn resolve(session: &Session, model: &Model, settings: &Settings) -> Self {
        let reasoning_effort = resolve_reasoning_effort(session, model, settings);
        Self {
            temperature: resolve_temperature(session, model, settings),
            top_p: resolve_top_p(session, model, settings),
            max_tokens: resolve_max_tokens(session, model, settings),
            context_length: resolve_context_length(session, model, settings),
            frequency_penalty: resolve_frequency_penalty(session, model, settings),
            presence_penalty: resolve_presence_penalty(session, model, settings),
            top_k: resolve_top_k(session, model, settings),
            reasoning_enabled: resolve_reasoning_enabled(session, model, settings),
            reasoning_budget: resolve_reasoning_budget(
                session,
                model,
                settings,
                reasoning_effort.as_deref(),
            ),
            reasoning_effort,
            prompt_caching_enabled: model
                .advanced_model_settings
                .as_ref()
                .and_then(|s| s.prompt_caching_enabled),
        }
    }

    pub(crate) fn for_sampling(
        context_length: Option<u32>,
        max_tokens: u32,
        temperature: f64,
        top_p: f64,
        top_k: Option<u32>,
        frequency_penalty: Option<f64>,
        presence_penalty: Option<f64>,
        prompt_caching_enabled: Option<bool>,
    ) -> Self {
        Self {
            temperature: Some(temperature),
            top_p: Some(top_p),
            max_tokens,
            context_length,
            frequency_penalty,
            presence_penalty,
            top_k,
            reasoning_enabled: false,
            reasoning_effort: None,
            reasoning_budget: None,
            prompt_caching_enabled,
        }
    }
}

pub(crate) fn build_provider_extra_fields(
    provider_id: &str,
    session: &Session,
    model: &Model,
    settings: &Settings,
    request_settings: &RequestSettings,
) -> Option<HashMap<String, Value>> {
    let mut extra = HashMap::new();

    // Keep all your existing provider-specific logic
    if provider_id == "llamacpp" {
        if let Some(llama_extra) = build_llama_extra_fields(session, model, settings) {
            extra.extend(llama_extra);
        }
    } else if provider_id == "ollama" {
        if let Some(ollama_extra) =
            build_ollama_extra_fields(session, model, settings, request_settings)
        {
            extra.extend(ollama_extra);
        }
    }

    // ─────────────────────────────────────────────────────────────
    // NEW: Prompt caching TTL (used by Claude, Bedrock, Vertex, Gemini, etc.)
    // ─────────────────────────────────────────────────────────────
    let ttl_val = model
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.prompt_caching_ttl.clone())
        .or_else(|| {
            session
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.prompt_caching_ttl.clone())
        })
        // Global settings fallback (in case you ever add it there)
        .or_else(|| {
            // settings.advanced_model_settings.prompt_caching_ttl.clone()  // uncomment if the field exists
            None
        });

    if let Some(ttl) = ttl_val {
        extra.insert("promptCachingTtl".to_string(), json!(ttl));
    }

    if extra.is_empty() {
        None
    } else {
        Some(extra)
    }
}
