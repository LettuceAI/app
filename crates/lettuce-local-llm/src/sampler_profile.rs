//! The named llama.cpp sampler profiles and their default values, which a
//! request resolves on every target.

pub const DEFAULT_LLAMA_SAMPLER_PROFILE: &str = "balanced";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SamplerProfileDefaults {
    pub name: &'static str,
    pub temperature: f64,
    pub top_p: f64,
    pub top_k: Option<u32>,
    pub min_p: Option<f64>,
    pub typical_p: Option<f64>,
    pub frequency_penalty: Option<f64>,
    pub presence_penalty: Option<f64>,
}

pub fn normalize_sampler_profile(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "balanced" => Some("balanced"),
        "creative" => Some("creative"),
        "stable" => Some("stable"),
        "reasoning" => Some("reasoning"),
        _ => None,
    }
}

pub fn sampler_profile_defaults(profile: Option<&str>) -> SamplerProfileDefaults {
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
