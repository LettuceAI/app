use super::*;

pub(super) const DEFAULT_LLAMA_SAMPLER_PROFILE: &str = "balanced";

#[derive(Clone, Copy)]
pub(super) struct SamplerProfileDefaults {
    pub(super) name: &'static str,
    pub(super) temperature: f64,
    pub(super) top_p: f64,
    pub(super) top_k: Option<u32>,
    pub(super) min_p: Option<f64>,
    pub(super) typical_p: Option<f64>,
    pub(super) frequency_penalty: Option<f64>,
    pub(super) presence_penalty: Option<f64>,
}

pub(super) struct ResolvedSamplerConfig {
    pub(super) profile: &'static str,
    pub(super) temperature: f64,
    pub(super) top_p: f64,
    pub(super) top_k: Option<u32>,
    pub(super) min_p: Option<f64>,
    pub(super) typical_p: Option<f64>,
    pub(super) frequency_penalty: Option<f64>,
    pub(super) presence_penalty: Option<f64>,
    pub(super) seed: Option<u32>,
}

pub(super) struct BuiltSampler {
    pub(super) sampler: LlamaSampler,
    pub(super) order: Vec<&'static str>,
    pub(super) active_params: Value,
}

pub(super) fn flash_attention_policy_label(policy: llama_flash_attn_type) -> &'static str {
    match policy {
        LLAMA_FLASH_ATTN_TYPE_AUTO => "auto",
        LLAMA_FLASH_ATTN_TYPE_DISABLED => "disabled",
        LLAMA_FLASH_ATTN_TYPE_ENABLED => "enabled",
        _ => "unknown",
    }
}

pub(super) fn kv_type_label(llama_kv_type_raw: Option<&str>) -> &str {
    llama_kv_type_raw.unwrap_or("llama.cpp default")
}

pub(super) fn offload_kqv_mode_label(resolved_offload_kqv: Option<bool>) -> &'static str {
    match resolved_offload_kqv {
        Some(true) => "enabled",
        Some(false) => "disabled",
        None => "llama.cpp default",
    }
}

pub(super) fn normalize_sampler_profile(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "balanced" => Some("balanced"),
        "creative" => Some("creative"),
        "stable" => Some("stable"),
        "reasoning" => Some("reasoning"),
        _ => None,
    }
}

pub(super) fn sampler_profile_defaults(profile: Option<&str>) -> SamplerProfileDefaults {
    match profile
        .and_then(normalize_sampler_profile)
        .unwrap_or(DEFAULT_LLAMA_SAMPLER_PROFILE)
    {
        "creative" => SamplerProfileDefaults {
            name: "creative",
            temperature: 0.95,
            top_p: 0.98,
            top_k: Some(80),
            min_p: Some(0.02),
            typical_p: None,
            frequency_penalty: Some(0.0),
            presence_penalty: Some(0.25),
        },
        "stable" => SamplerProfileDefaults {
            name: "stable",
            temperature: 0.55,
            top_p: 0.90,
            top_k: Some(32),
            min_p: Some(0.08),
            typical_p: Some(0.97),
            frequency_penalty: Some(0.2),
            presence_penalty: Some(0.0),
        },
        "reasoning" => SamplerProfileDefaults {
            name: "reasoning",
            temperature: 0.35,
            top_p: 0.90,
            top_k: Some(24),
            min_p: None,
            typical_p: Some(0.95),
            frequency_penalty: Some(0.1),
            presence_penalty: Some(0.0),
        },
        _ => SamplerProfileDefaults {
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

pub(super) fn build_sampler(config: &ResolvedSamplerConfig) -> BuiltSampler {
    let mut samplers = Vec::new();
    let mut order = Vec::new();
    let mut active_params = serde_json::Map::new();
    active_params.insert("profile".to_string(), json!(config.profile));
    active_params.insert("temperature".to_string(), json!(config.temperature));
    active_params.insert("top_p".to_string(), json!(config.top_p));
    if let Some(seed) = config.seed {
        active_params.insert("seed".to_string(), json!(seed));
    }
    let penalty_freq = config.frequency_penalty.unwrap_or(0.0);
    let penalty_present = config.presence_penalty.unwrap_or(0.0);
    if penalty_freq != 0.0 || penalty_present != 0.0 {
        order.push("penalties");
        samplers.push(LlamaSampler::penalties(
            -1,
            1.0,
            penalty_freq as f32,
            penalty_present as f32,
        ));
        active_params.insert("frequency_penalty".to_string(), json!(penalty_freq));
        active_params.insert("presence_penalty".to_string(), json!(penalty_present));
    }

    let k = config.top_k.unwrap_or(40) as i32;
    order.push("top_k");
    samplers.push(LlamaSampler::top_k(k));
    active_params.insert("top_k".to_string(), json!(k));

    let p = if config.top_p > 0.0 {
        config.top_p
    } else {
        1.0
    };
    order.push("top_p");
    samplers.push(LlamaSampler::top_p(p as f32, 1));
    if let Some(mp) = config.min_p {
        if mp > 0.0 {
            order.push("min_p");
            samplers.push(LlamaSampler::min_p(mp as f32, 1));
            active_params.insert("min_p".to_string(), json!(mp));
        }
    }
    if let Some(tp) = config.typical_p {
        if tp > 0.0 && tp < 1.0 {
            order.push("typical");
            samplers.push(LlamaSampler::typical(tp as f32, 1));
            active_params.insert("typical_p".to_string(), json!(tp));
        }
    }

    if config.temperature > 0.0 {
        order.push("temp");
        samplers.push(LlamaSampler::temp(config.temperature as f32));
        order.push("dist");
        samplers.push(LlamaSampler::dist(
            config.seed.unwrap_or_else(rand::random::<u32>),
        ));
    } else {
        order.push("greedy");
        samplers.push(LlamaSampler::greedy());
    }

    BuiltSampler {
        sampler: LlamaSampler::chain(samplers, false),
        order,
        active_params: Value::Object(active_params),
    }
}
