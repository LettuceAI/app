use super::{embedding_model_dir, ort_runtime, specs::COMPANION_EMOTION_MODEL_FILES_LOCAL};
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

const MAX_EMOTION_SEQ_LENGTH: usize = 512;

#[derive(Debug, Clone)]
pub(crate) struct EmotionLabelScore {
    pub(crate) label: String,
    pub(crate) score: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct EmotionClassification {
    pub(crate) labels: Vec<EmotionLabelScore>,
    pub(crate) confidence: f64,
}

struct LoadedEmotionRuntime {
    model_path: PathBuf,
    tokenizer_path: PathBuf,
    config_path: PathBuf,
    labels: Vec<String>,
    session: Session,
    tokenizer: Tokenizer,
}

#[derive(Debug, Deserialize)]
struct EmotionModelConfig {
    #[serde(default)]
    id2label: HashMap<String, String>,
}

lazy_static::lazy_static! {
    static ref LOADED_EMOTION_RUNTIME: Arc<TokioMutex<Option<LoadedEmotionRuntime>>> =
        Arc::new(TokioMutex::new(None));
}

pub(crate) async fn clear_loaded_runtime_cache() {
    let mut cache = LOADED_EMOTION_RUNTIME.lock().await;
    *cache = None;
}

pub(crate) fn companion_emotion_model_paths(
    app: &AppHandle,
) -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let model_dir = embedding_model_dir(app)?;
    Ok((
        model_dir.join("companion-emotion/model.int8.onnx"),
        model_dir.join("companion-emotion/tokenizer.json"),
        model_dir.join("companion-emotion/config.json"),
    ))
}

pub(crate) fn is_companion_emotion_model_installed(app: &AppHandle) -> Result<bool, String> {
    let model_dir = embedding_model_dir(app)?;
    Ok(COMPANION_EMOTION_MODEL_FILES_LOCAL
        .iter()
        .all(|filename| model_dir.join(filename).exists()))
}

pub(crate) async fn classify_text(
    app: &AppHandle,
    text: &str,
) -> Result<Option<EmotionClassification>, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    if !is_companion_emotion_model_installed(app)? {
        return Ok(None);
    }

    let (model_path, tokenizer_path, config_path) = companion_emotion_model_paths(app)?;
    ort_runtime::ensure_ort_init(app).await?;

    let mut cache = LOADED_EMOTION_RUNTIME.lock().await;
    let reuse = cache.as_ref().is_some_and(|loaded| {
        loaded.model_path == model_path
            && loaded.tokenizer_path == tokenizer_path
            && loaded.config_path == config_path
    });

    if !reuse {
        let runtime = create_runtime(app, &model_path, &tokenizer_path, &config_path)?;
        log_info(
            app,
            "companion_emotion",
            format!("loaded emotion classifier model={}", model_path.display()),
        );
        *cache = Some(runtime);
    }

    let loaded = cache.as_mut().ok_or_else(|| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            "Companion emotion runtime cache unavailable",
        )
    })?;

    classify_with_runtime(
        &mut loaded.session,
        &loaded.tokenizer,
        &loaded.labels,
        trimmed,
    )
    .map(Some)
}

fn create_runtime(
    app: &AppHandle,
    model_path: &Path,
    tokenizer_path: &Path,
    config_path: &Path,
) -> Result<LoadedEmotionRuntime, String> {
    let labels = read_labels(config_path)?;
    let session = Session::builder()
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to create emotion classifier session builder: {}", e),
            )
        })?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to set emotion classifier optimization level: {}", e),
            )
        })?
        .commit_from_file(model_path)
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to load emotion classifier model: {}", e),
            )
        })?;

    let tokenizer = Tokenizer::from_file(tokenizer_path).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!(
                "Failed to load emotion classifier tokenizer from {:?}: {}",
                tokenizer_path, e
            ),
        )
    })?;

    if labels.is_empty() {
        log_warn(
            app,
            "companion_emotion",
            "emotion classifier config has no labels; using GoEmotions fallback labels",
        );
    }

    Ok(LoadedEmotionRuntime {
        model_path: model_path.to_path_buf(),
        tokenizer_path: tokenizer_path.to_path_buf(),
        config_path: config_path.to_path_buf(),
        labels: if labels.is_empty() {
            default_go_emotions_labels()
        } else {
            labels
        },
        session,
        tokenizer,
    })
}

fn classify_with_runtime(
    session: &mut Session,
    tokenizer: &Tokenizer,
    labels: &[String],
    text: &str,
) -> Result<EmotionClassification, String> {
    let encoding = tokenizer.encode(text, true).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Emotion classifier tokenization failed: {}", e),
        )
    })?;

    let input_ids = encoding.get_ids();
    let attention_mask = encoding.get_attention_mask();
    let seq_len = input_ids.len().min(MAX_EMOTION_SEQ_LENGTH);
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
            format!("Failed to create emotion input_ids tensor: {}", e),
        )
    })?;
    let attention_mask_value =
        Value::from_array(([1, seq_len], attention_mask_i64)).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to create emotion attention_mask tensor: {}", e),
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
                format!("Emotion classifier inference failed: {}", e),
            )
        })?;

    let logits_value = &outputs[0];
    let (_, logits) = logits_value.try_extract_tensor::<f32>().map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to extract emotion logits: {}", e),
        )
    })?;

    let mut scored = logits
        .iter()
        .enumerate()
        .map(|(index, logit)| EmotionLabelScore {
            label: labels
                .get(index)
                .cloned()
                .unwrap_or_else(|| format!("label_{}", index)),
            score: sigmoid(*logit),
        })
        .collect::<Vec<_>>();

    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let confidence = scored
        .iter()
        .take(3)
        .map(|item| item.score as f64)
        .fold(0.0, f64::max)
        .clamp(0.0, 1.0);

    Ok(EmotionClassification {
        labels: scored,
        confidence,
    })
}

fn read_labels(config_path: &Path) -> Result<Vec<String>, String> {
    let raw = fs::read_to_string(config_path).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!(
                "Failed to read emotion classifier config {}: {}",
                config_path.display(),
                e
            ),
        )
    })?;
    let config: EmotionModelConfig = serde_json::from_str(&raw).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to parse emotion classifier config: {}", e),
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

fn sigmoid(value: f32) -> f32 {
    1.0 / (1.0 + (-value).exp())
}

fn default_go_emotions_labels() -> Vec<String> {
    [
        "admiration",
        "amusement",
        "anger",
        "annoyance",
        "approval",
        "caring",
        "confusion",
        "curiosity",
        "desire",
        "disappointment",
        "disapproval",
        "disgust",
        "embarrassment",
        "excitement",
        "fear",
        "gratitude",
        "grief",
        "joy",
        "love",
        "nervousness",
        "optimism",
        "pride",
        "realization",
        "relief",
        "remorse",
        "sadness",
        "surprise",
        "neutral",
    ]
    .iter()
    .map(|label| label.to_string())
    .collect()
}
