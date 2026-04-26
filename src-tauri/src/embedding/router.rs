#![allow(dead_code)]

use super::{embedding_model_dir, ort_runtime, specs::COMPANION_ROUTER_MODEL_FILES_LOCAL};
use crate::utils::{log_info, log_warn};
use ort::{
    inputs,
    session::{builder::GraphOptimizationLevel, Session},
    value::Value,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::AppHandle;
use tokenizers::Tokenizer;
use tokio::sync::Mutex as TokioMutex;

const MAX_ROUTER_SEQ_LENGTH: usize = 512;

#[derive(Debug, Clone)]
pub(crate) struct RouterHypothesisScore {
    pub(crate) entailment: f32,
    pub(crate) neutral: f32,
    pub(crate) contradiction: f32,
}

struct LoadedRouterRuntime {
    model_path: PathBuf,
    tokenizer_path: PathBuf,
    config_path: PathBuf,
    labels: Vec<String>,
    session: Session,
    tokenizer: Tokenizer,
}

#[derive(Debug, Deserialize)]
struct RouterModelConfig {
    #[serde(default)]
    id2label: HashMap<String, String>,
}

lazy_static::lazy_static! {
    static ref LOADED_ROUTER_RUNTIME: Arc<TokioMutex<Option<LoadedRouterRuntime>>> =
        Arc::new(TokioMutex::new(None));
}

pub(crate) async fn clear_loaded_runtime_cache() {
    let mut cache = LOADED_ROUTER_RUNTIME.lock().await;
    *cache = None;
}

pub(crate) fn companion_router_model_paths(
    app: &AppHandle,
) -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let model_dir = embedding_model_dir(app)?;
    Ok((
        model_dir.join("companion-router/model.int8.onnx"),
        model_dir.join("companion-router/tokenizer.json"),
        model_dir.join("companion-router/config.json"),
    ))
}

pub(crate) fn is_companion_router_model_installed(app: &AppHandle) -> Result<bool, String> {
    let model_dir = embedding_model_dir(app)?;
    Ok(COMPANION_ROUTER_MODEL_FILES_LOCAL
        .iter()
        .all(|filename| model_dir.join(filename).exists()))
}

pub(crate) async fn score_hypotheses(
    app: &AppHandle,
    premise: &str,
    hypotheses: &[&str],
) -> Result<Option<Vec<RouterHypothesisScore>>, String> {
    let trimmed = premise.trim();
    if trimmed.is_empty() || hypotheses.is_empty() {
        return Ok(None);
    }

    if !is_companion_router_model_installed(app)? {
        return Ok(None);
    }

    let (model_path, tokenizer_path, config_path) = companion_router_model_paths(app)?;
    ort_runtime::ensure_ort_init(app).await?;

    let mut cache = LOADED_ROUTER_RUNTIME.lock().await;
    let reuse = cache.as_ref().is_some_and(|loaded| {
        loaded.model_path == model_path
            && loaded.tokenizer_path == tokenizer_path
            && loaded.config_path == config_path
    });

    if !reuse {
        let runtime = create_runtime(app, &model_path, &tokenizer_path, &config_path)?;
        log_info(
            app,
            "companion_router",
            format!("loaded router model={}", model_path.display()),
        );
        *cache = Some(runtime);
    }

    let loaded = cache.as_mut().ok_or_else(|| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            "Companion router runtime unavailable",
        )
    })?;

    let mut scores = Vec::new();
    for hypothesis in hypotheses {
        let (entailment, neutral, contradiction) = score_with_runtime(
            &mut loaded.session,
            &loaded.tokenizer,
            &loaded.labels,
            trimmed,
            hypothesis,
        )?;
        scores.push(RouterHypothesisScore {
            entailment,
            neutral,
            contradiction,
        });
    }

    Ok(Some(scores))
}

fn create_runtime(
    app: &AppHandle,
    model_path: &Path,
    tokenizer_path: &Path,
    config_path: &Path,
) -> Result<LoadedRouterRuntime, String> {
    let labels = read_labels(config_path)?;
    let session = Session::builder()
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to create companion router session builder: {}", e),
            )
        })?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to set companion router optimization level: {}", e),
            )
        })?
        .commit_from_file(model_path)
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to load companion router model: {}", e),
            )
        })?;

    let tokenizer = Tokenizer::from_file(tokenizer_path).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!(
                "Failed to load companion router tokenizer from {:?}: {}",
                tokenizer_path, e
            ),
        )
    })?;

    if labels.is_empty() {
        log_warn(
            app,
            "companion_router",
            "companion router config has no labels; using default MNLI labels",
        );
    }

    Ok(LoadedRouterRuntime {
        model_path: model_path.to_path_buf(),
        tokenizer_path: tokenizer_path.to_path_buf(),
        config_path: config_path.to_path_buf(),
        labels: if labels.is_empty() {
            default_router_labels()
        } else {
            labels
        },
        session,
        tokenizer,
    })
}

fn score_with_runtime(
    session: &mut Session,
    tokenizer: &Tokenizer,
    labels: &[String],
    premise: &str,
    hypothesis: &str,
) -> Result<(f32, f32, f32), String> {
    let encoding = tokenizer.encode((premise, hypothesis), true).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Companion router tokenization failed: {}", e),
        )
    })?;

    let input_ids = encoding.get_ids();
    let attention_mask = encoding.get_attention_mask();
    let seq_len = input_ids.len().min(MAX_ROUTER_SEQ_LENGTH);
    let input_ids_i64 = input_ids[..seq_len]
        .iter()
        .map(|&value| value as i64)
        .collect::<Vec<_>>();
    let attention_mask_i64 = attention_mask[..seq_len]
        .iter()
        .map(|&value| value as i64)
        .collect::<Vec<_>>();

    let input_ids_value = Value::from_array(([1, seq_len], input_ids_i64)).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to create router input_ids tensor: {}", e),
        )
    })?;
    let attention_mask_value =
        Value::from_array(([1, seq_len], attention_mask_i64)).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to create router attention_mask tensor: {}", e),
            )
        })?;

    let outputs = session
        .run(inputs![
            "input_ids" => input_ids_value,
            "attention_mask" => attention_mask_value
        ])
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Companion router inference failed: {}", e),
            )
        })?;

    let logits_value = &outputs[0];
    let (_, logits) = logits_value.try_extract_tensor::<f32>().map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to extract router logits: {}", e),
        )
    })?;

    let probabilities = softmax(logits);
    Ok((
        label_probability(labels, &probabilities, "ENTAILMENT"),
        label_probability(labels, &probabilities, "NEUTRAL"),
        label_probability(labels, &probabilities, "CONTRADICTION"),
    ))
}

fn label_probability(labels: &[String], probabilities: &[f32], target: &str) -> f32 {
    labels
        .iter()
        .position(|label| label.eq_ignore_ascii_case(target))
        .and_then(|index| probabilities.get(index).copied())
        .unwrap_or(0.0)
}

fn read_labels(config_path: &Path) -> Result<Vec<String>, String> {
    let raw = fs::read_to_string(config_path).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!(
                "Failed to read companion router config {}: {}",
                config_path.display(),
                e
            ),
        )
    })?;
    let config: RouterModelConfig = serde_json::from_str(&raw).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to parse companion router config: {}", e),
        )
    })?;

    let mut labels = config
        .id2label
        .into_iter()
        .filter_map(|(key, label)| key.parse::<usize>().ok().map(|index| (index, label)))
        .collect::<Vec<_>>();
    labels.sort_by_key(|(index, _)| *index);
    Ok(labels.into_iter().map(|(_, label)| label).collect())
}

fn softmax(logits: &[f32]) -> Vec<f32> {
    let max_logit = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exps = logits
        .iter()
        .map(|value| (value - max_logit).exp())
        .collect::<Vec<_>>();
    let sum = exps.iter().copied().sum::<f32>().max(f32::EPSILON);
    exps.into_iter().map(|value| value / sum).collect()
}

fn default_router_labels() -> Vec<String> {
    ["ENTAILMENT", "NEUTRAL", "CONTRADICTION"]
        .iter()
        .map(|label| label.to_string())
        .collect()
}
