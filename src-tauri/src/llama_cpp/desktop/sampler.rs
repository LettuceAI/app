use super::*;

pub(super) const DEFAULT_LLAMA_SAMPLER_PROFILE: &str = "balanced";
pub(super) const DEFAULT_LLAMA_SAMPLER_ORDER: [&str; 7] = [
    "penalties",
    "grammar",
    "top_k",
    "top_p",
    "min_p",
    "typical",
    "temp",
];

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
    pub(super) order: Option<Vec<String>>,
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
        "typical" | "typ_p" | "typical_p" => Some("typical"),
        "temp" | "temperature" => Some("temp"),
        _ => None,
    }
}

fn normalize_sampler_order(value: Option<&[String]>) -> Vec<&'static str> {
    let mut seen = std::collections::HashSet::new();
    let mut order = Vec::new();

    if let Some(value) = value {
        for stage in value {
            let Some(stage) = normalize_sampler_stage(stage) else {
                continue;
            };
            if seen.insert(stage) {
                order.push(stage);
            }
        }
    }

    if order.is_empty() {
        return DEFAULT_LLAMA_SAMPLER_ORDER.to_vec();
    }

    for stage in DEFAULT_LLAMA_SAMPLER_ORDER {
        if seen.insert(stage) {
            order.push(stage);
        }
    }

    order
}

pub(super) fn build_sampler(
    model: &LlamaModel,
    config: &ResolvedSamplerConfig,
    chat_template_result: Option<&llama_cpp_2::model::ChatTemplateResult>,
) -> Result<BuiltSampler, String> {
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
    let mut penalties_sampler = if penalty_freq != 0.0 || penalty_present != 0.0 {
        active_params.insert("frequency_penalty".to_string(), json!(penalty_freq));
        active_params.insert("presence_penalty".to_string(), json!(penalty_present));
        Some(LlamaSampler::penalties(
            -1,
            1.0,
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
                        crate::utils::err_msg(
                            module_path!(),
                            line!(),
                            format!(
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
                                    crate::utils::err_msg(
                                        module_path!(),
                                        line!(),
                                        format!(
                                            "Failed to tokenize grammar trigger word '{}': {e}",
                                            trigger.value
                                        ),
                                    )
                                })?;
                            if tokens.len() == 1 {
                                if !preserved.contains(&tokens[0]) {
                                    return Err(crate::utils::err_msg(
                                        module_path!(),
                                        line!(),
                                        format!(
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
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to initialize llama.cpp grammar sampler: {e}"),
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
    if let Some(tp) = config.typical_p {
        if tp > 0.0 && tp < 1.0 {
            active_params.insert("typical_p".to_string(), json!(tp));
        }
    }
    let mut typical_sampler = config
        .typical_p
        .filter(|tp| *tp > 0.0 && *tp < 1.0)
        .map(|tp| LlamaSampler::typical(tp as f32, 1));

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
            "typical" => {
                if let Some(sampler) = typical_sampler.take() {
                    order.push("typical");
                    samplers.push(sampler);
                }
            }
            "temp" => {
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
            }
            _ => {}
        }
    }

    Ok(BuiltSampler {
        sampler: LlamaSampler::chain(samplers, false),
        order,
        active_params: Value::Object(active_params),
    })
}
