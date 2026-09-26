//! The llama.cpp sampler chain: the named profiles and their defaults, the
//! stage order (default or the user's, deduplicated, an explicit empty list
//! meaning no stages), the penalties/DRY/XTC/typical/min-p parameters, the
//! template's (lazy) grammar forced to the front, and last adaptive-p when
//! the order asks for it and a target is set, else `dist` when the
//! temperature is positive, `greedy` otherwise.

use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;
use serde_json::{Value, json};

use crate::offload::FlashAttentionPolicy;
pub use crate::sampler_profile::{
    DEFAULT_LLAMA_SAMPLER_PROFILE, SamplerProfileDefaults, normalize_sampler_profile,
    sampler_profile_defaults,
};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct SamplerError(pub String);

pub const DEFAULT_LLAMA_SAMPLER_ORDER: [&str; 10] = [
    "penalties",
    "grammar",
    "top_k",
    "top_p",
    "min_p",
    "dry",
    "typical",
    "xtc",
    "temp",
    "adaptive_p",
];

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedSamplerConfig {
    pub profile: &'static str,
    pub order: Option<Vec<String>>,
    pub temperature: f64,
    pub top_p: f64,
    pub top_k: Option<u32>,
    pub min_p: Option<f64>,
    pub typical_p: Option<f64>,
    pub repeat_penalty: Option<f64>,
    pub n_pen_range: Option<i32>,
    pub context_size: u32,
    pub dry_multiplier: Option<f64>,
    pub dry_base: Option<f64>,
    pub dry_allowed_length: Option<u32>,
    pub dry_penalty_last_n: Option<i32>,
    pub dry_sequence_breakers: Option<Vec<String>>,
    pub xtc_probability: Option<f64>,
    pub xtc_threshold: Option<f64>,
    pub adaptive_target: Option<f64>,
    pub adaptive_decay: Option<f64>,
    pub frequency_penalty: Option<f64>,
    pub presence_penalty: Option<f64>,
    pub seed: Option<u32>,
}

pub struct BuiltSampler {
    pub sampler: LlamaSampler,
    pub order: Vec<&'static str>,
    pub active_params: Value,
}

impl std::fmt::Debug for BuiltSampler {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BuiltSampler")
            .field("order", &self.order)
            .field("active_params", &self.active_params)
            .finish_non_exhaustive()
    }
}

#[must_use]
pub const fn flash_attention_policy_label(policy: FlashAttentionPolicy) -> &'static str {
    match policy {
        FlashAttentionPolicy::Auto => "auto",
        FlashAttentionPolicy::Disabled => "disabled",
        FlashAttentionPolicy::Enabled => "enabled",
    }
}

pub fn kv_type_label(llama_kv_type_raw: Option<&str>) -> &str {
    llama_kv_type_raw.unwrap_or("llama.cpp default")
}

pub fn offload_kqv_mode_label(resolved_offload_kqv: Option<bool>) -> &'static str {
    match resolved_offload_kqv {
        Some(true) => "enabled",
        Some(false) => "disabled",
        None => "llama.cpp default",
    }
}

fn regex_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

fn anchor_pattern(value: &str) -> String {
    if value.is_empty() {
        return "^$".to_string();
    }

    let mut anchored = String::new();
    if !value.starts_with('^') {
        anchored.push('^');
    }
    anchored.push_str(value);
    if !value.ends_with('$') {
        anchored.push('$');
    }
    anchored
}

fn normalize_sampler_stage(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "penalties" => Some("penalties"),
        "grammar" => Some("grammar"),
        "top_k" | "topk" => Some("top_k"),
        "top_p" | "topp" => Some("top_p"),
        "min_p" | "minp" => Some("min_p"),
        "dry" => Some("dry"),
        "typical" | "typ_p" | "typical_p" => Some("typical"),
        "xtc" => Some("xtc"),
        "temp" | "temperature" => Some("temp"),
        "adaptive_p" | "adaptivep" | "adaptive" => Some("adaptive_p"),
        _ => None,
    }
}

fn normalize_sampler_order(value: Option<&[String]>) -> Vec<&'static str> {
    let mut seen = std::collections::HashSet::new();
    let mut order = Vec::new();

    let Some(value) = value else {
        return DEFAULT_LLAMA_SAMPLER_ORDER.to_vec();
    };

    for stage in value {
        let Some(stage) = normalize_sampler_stage(stage) else {
            continue;
        };
        if seen.insert(stage) {
            order.push(stage);
        }
    }

    order
}

fn resolve_n_pen_range(value: Option<i32>, context_size: u32) -> i32 {
    match value.unwrap_or(-1) {
        -1 => i32::try_from(context_size).unwrap_or(i32::MAX),
        value => value,
    }
}

pub fn build_sampler(
    model: &LlamaModel,
    config: &ResolvedSamplerConfig,
    chat_template_result: Option<&llama_cpp_2::model::ChatTemplateResult>,
) -> Result<BuiltSampler, SamplerError> {
    let mut samplers = Vec::new();
    let mut order = Vec::new();
    let mut active_params = serde_json::Map::new();
    active_params.insert("profile".to_string(), json!(config.profile));
    let requested_order = normalize_sampler_order(config.order.as_deref());
    active_params.insert("sampler_order".to_string(), json!(requested_order));
    active_params.insert("temperature".to_string(), json!(config.temperature));
    active_params.insert("top_p".to_string(), json!(config.top_p));
    if let Some(seed) = config.seed {
        active_params.insert("seed".to_string(), json!(seed));
    }
    let penalty_freq = config.frequency_penalty.unwrap_or(0.0);
    let penalty_present = config.presence_penalty.unwrap_or(0.0);
    let repeat_penalty = config.repeat_penalty.unwrap_or(1.0);
    let n_pen_range = resolve_n_pen_range(config.n_pen_range, config.context_size);
    let mut penalties_sampler =
        if repeat_penalty != 1.0 || penalty_freq != 0.0 || penalty_present != 0.0 {
            active_params.insert("repeat_penalty".to_string(), json!(repeat_penalty));
            active_params.insert("n_pen_range".to_string(), json!(n_pen_range));
            active_params.insert("frequency_penalty".to_string(), json!(penalty_freq));
            active_params.insert("presence_penalty".to_string(), json!(penalty_present));
            Some(LlamaSampler::penalties(
                model,
                n_pen_range,
                repeat_penalty as f32,
                penalty_freq as f32,
                penalty_present as f32,
            ))
        } else {
            None
        };

    let mut grammar_sampler = None;
    if let Some(template_result) = chat_template_result {
        if let Some(grammar) = template_result.grammar.as_deref() {
            grammar_sampler = Some(if template_result.grammar_lazy {
                let mut preserved = std::collections::HashSet::new();
                for token_str in &template_result.preserved_tokens {
                    let tokens = model.str_to_token(token_str, AddBos::Never).map_err(|e| {
                        SamplerError(format!(
                                "Failed to tokenize preserved grammar token '{}': {e}",
                                token_str
                            ),
                        )
                    })?;
                    if tokens.len() == 1 {
                        preserved.insert(tokens[0]);
                    }
                }

                let mut trigger_patterns = Vec::new();
                let mut trigger_tokens = Vec::new();

                for trigger in &template_result.grammar_triggers {
                    match trigger.trigger_type {
                        llama_cpp_2::model::GrammarTriggerType::Token => {
                            if let Some(token) = trigger.token {
                                trigger_tokens.push(token);
                            }
                        }
                        llama_cpp_2::model::GrammarTriggerType::Word => {
                            let tokens =
                                model.str_to_token(&trigger.value, AddBos::Never).map_err(|e| {
                                    SamplerError(format!(
                                            "Failed to tokenize grammar trigger word '{}': {e}",
                                            trigger.value
                                        ),
                                    )
                                })?;
                            if tokens.len() == 1 {
                                if !preserved.contains(&tokens[0]) {
                                    return Err(SamplerError(format!(
                                            "Grammar trigger word '{}' was not preserved as a single token",
                                            trigger.value
                                        ),
                                    ));
                                }
                                trigger_tokens.push(tokens[0]);
                            } else {
                                trigger_patterns.push(regex_escape(&trigger.value));
                            }
                        }
                        llama_cpp_2::model::GrammarTriggerType::Pattern => {
                            trigger_patterns.push(trigger.value.clone());
                        }
                        llama_cpp_2::model::GrammarTriggerType::PatternFull => {
                            trigger_patterns.push(anchor_pattern(&trigger.value));
                        }
                    }
                }

                LlamaSampler::grammar_lazy_patterns(
                    model,
                    grammar,
                    "root",
                    &trigger_patterns,
                    &trigger_tokens,
                )
            } else {
                LlamaSampler::grammar(model, grammar, "root")
            }
            .map_err(|e| {
                SamplerError(format!("Failed to initialize llama.cpp grammar sampler: {e}"),
                )
            })?);
            active_params.insert(
                "grammar".to_string(),
                json!({
                    "lazy": template_result.grammar_lazy,
                    "trigger_count": template_result.grammar_triggers.len(),
                    "preserved_token_count": template_result.preserved_tokens.len(),
                }),
            );
        }
    }

    let k = config.top_k.unwrap_or(40) as i32;
    active_params.insert("top_k".to_string(), json!(k));
    let mut top_k_sampler = Some(LlamaSampler::top_k(k));

    let p = if config.top_p > 0.0 {
        config.top_p
    } else {
        1.0
    };
    let mut top_p_sampler = Some(LlamaSampler::top_p(p as f32, 1));
    if let Some(mp) = config.min_p {
        if mp > 0.0 {
            active_params.insert("min_p".to_string(), json!(mp));
        }
    }
    let mut min_p_sampler = config
        .min_p
        .filter(|mp| *mp > 0.0)
        .map(|mp| LlamaSampler::min_p(mp as f32, 1));
    let mut dry_sampler = config
        .dry_multiplier
        .filter(|multiplier| *multiplier > 0.0)
        .map(|multiplier| {
            let base = config.dry_base.unwrap_or(1.75).max(0.0);
            let allowed_length = config
                .dry_allowed_length
                .and_then(|value| i32::try_from(value).ok())
                .unwrap_or(2);
            let penalty_last_n = config.dry_penalty_last_n.unwrap_or(-1);
            let seq_breakers = config.dry_sequence_breakers.clone().unwrap_or_else(|| {
                vec![
                    "\n".to_string(),
                    ":".to_string(),
                    "\"".to_string(),
                    "*".to_string(),
                ]
            });
            active_params.insert("dry_multiplier".to_string(), json!(multiplier));
            active_params.insert("dry_base".to_string(), json!(base));
            active_params.insert("dry_allowed_length".to_string(), json!(allowed_length));
            active_params.insert("dry_penalty_last_n".to_string(), json!(penalty_last_n));
            active_params.insert("dry_sequence_breakers".to_string(), json!(seq_breakers));
            LlamaSampler::dry(
                model,
                multiplier as f32,
                base as f32,
                allowed_length,
                penalty_last_n,
                seq_breakers,
            )
        });
    if let Some(tp) = config.typical_p {
        if tp > 0.0 && tp < 1.0 {
            active_params.insert("typical_p".to_string(), json!(tp));
        }
    }
    let mut typical_sampler = config
        .typical_p
        .filter(|tp| *tp > 0.0 && *tp < 1.0)
        .map(|tp| LlamaSampler::typical(tp as f32, 1));
    let mut xtc_sampler = config
        .xtc_probability
        .filter(|probability| *probability > 0.0)
        .map(|probability| {
            let threshold = config.xtc_threshold.unwrap_or(0.1);
            active_params.insert("xtc_probability".to_string(), json!(probability));
            active_params.insert("xtc_threshold".to_string(), json!(threshold));
            LlamaSampler::xtc(
                probability as f32,
                threshold as f32,
                1,
                config.seed.unwrap_or_else(rand::random::<u32>),
            )
        });

    let mut adaptive_requested = false;
    for stage in requested_order {
        match stage {
            "penalties" => {
                if let Some(sampler) = penalties_sampler.take() {
                    order.push("penalties");
                    samplers.push(sampler);
                }
            }
            "grammar" => {
                if let Some(sampler) = grammar_sampler.take() {
                    order.push("grammar");
                    samplers.push(sampler);
                }
            }
            "top_k" => {
                if let Some(sampler) = top_k_sampler.take() {
                    order.push("top_k");
                    samplers.push(sampler);
                }
            }
            "top_p" => {
                if let Some(sampler) = top_p_sampler.take() {
                    order.push("top_p");
                    samplers.push(sampler);
                }
            }
            "min_p" => {
                if let Some(sampler) = min_p_sampler.take() {
                    order.push("min_p");
                    samplers.push(sampler);
                }
            }
            "dry" => {
                if let Some(sampler) = dry_sampler.take() {
                    order.push("dry");
                    samplers.push(sampler);
                }
            }
            "typical" => {
                if let Some(sampler) = typical_sampler.take() {
                    order.push("typical");
                    samplers.push(sampler);
                }
            }
            "xtc" => {
                if let Some(sampler) = xtc_sampler.take() {
                    order.push("xtc");
                    samplers.push(sampler);
                }
            }
            "temp" if config.temperature > 0.0 => {
                order.push("temp");
                samplers.push(LlamaSampler::temp(config.temperature as f32));
            }
            "adaptive_p" => adaptive_requested = true,
            _ => {}
        }
    }

    if let Some(sampler) = grammar_sampler {
        order.insert(0, "grammar");
        samplers.insert(0, sampler);
    }

    let adaptive_target = config
        .adaptive_target
        .filter(|_| adaptive_requested)
        .filter(|target| *target > 0.0 && *target <= 1.0);
    if let Some(target) = adaptive_target {
        let decay = config.adaptive_decay.unwrap_or(0.95).clamp(0.0, 0.99);
        active_params.insert("adaptive_target".to_string(), json!(target));
        active_params.insert("adaptive_decay".to_string(), json!(decay));
        order.push("adaptive_p");
        samplers.push(LlamaSampler::adaptive_p(
            target as f32,
            decay as f32,
            config.seed.unwrap_or_else(rand::random::<u32>),
        ));
    } else if config.temperature > 0.0 {
        order.push("dist");
        samplers.push(LlamaSampler::dist(
            config.seed.unwrap_or_else(rand::random::<u32>),
        ));
    } else {
        order.push("greedy");
        samplers.push(LlamaSampler::greedy());
    }

    Ok(BuiltSampler {
        sampler: LlamaSampler::chain(samplers, false),
        order,
        active_params: Value::Object(active_params),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sampler_order_preserves_explicit_subsets() {
        let configured = vec![
            "min_p".to_string(),
            "temp".to_string(),
            "min_p".to_string(),
            "unknown".to_string(),
        ];

        assert_eq!(
            normalize_sampler_order(Some(&configured)),
            vec!["min_p", "temp"]
        );
    }

    #[test]
    fn sampler_order_preserves_an_explicit_empty_chain() {
        let configured = Vec::<String>::new();

        assert!(normalize_sampler_order(Some(&configured)).is_empty());
        assert_eq!(normalize_sampler_order(None), DEFAULT_LLAMA_SAMPLER_ORDER);
    }

    #[test]
    fn adaptive_p_is_part_of_the_default_order_and_accepts_its_aliases() {
        assert_eq!(DEFAULT_LLAMA_SAMPLER_ORDER.last(), Some(&"adaptive_p"));
        let configured = vec!["Adaptive".to_string(), "adaptivep".to_string()];
        assert_eq!(
            normalize_sampler_order(Some(&configured)),
            vec!["adaptive_p"]
        );
    }

    #[test]
    fn penalty_range_resolves_full_context_and_preserves_explicit_values() {
        assert_eq!(resolve_n_pen_range(None, 8_192), 8_192);
        assert_eq!(resolve_n_pen_range(Some(-1), 4_096), 4_096);
        assert_eq!(resolve_n_pen_range(Some(0), 4_096), 0);
        assert_eq!(resolve_n_pen_range(Some(64), 4_096), 64);
    }
}
