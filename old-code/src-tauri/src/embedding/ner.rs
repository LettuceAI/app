#![allow(dead_code)]

use super::{embedding_model_dir, ort_runtime, specs::COMPANION_NER_MODEL_FILES_LOCAL};
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

const MAX_NER_SEQ_LENGTH: usize = 512;
const MIN_ENTITY_TOKEN_SCORE: f32 = 0.52;
const MIN_ENTITY_SPAN_SCORE: f32 = 0.56;

#[derive(Debug, Clone)]
pub(crate) struct NamedEntitySpan {
    pub(crate) label: String,
    pub(crate) text: String,
    pub(crate) score: f32,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

struct LoadedNerRuntime {
    model_path: PathBuf,
    tokenizer_path: PathBuf,
    config_path: PathBuf,
    labels: Vec<String>,
    session: Session,
    tokenizer: Tokenizer,
}

#[derive(Debug, Deserialize)]
struct NerModelConfig {
    #[serde(default)]
    id2label: HashMap<String, String>,
}

#[derive(Debug, Clone)]
struct SpanAccumulator {
    label: String,
    start: usize,
    end: usize,
    score_sum: f32,
    token_count: usize,
    last_word_id: Option<u32>,
}

lazy_static::lazy_static! {
    static ref LOADED_NER_RUNTIME: Arc<TokioMutex<Option<LoadedNerRuntime>>> =
        Arc::new(TokioMutex::new(None));
}

pub(crate) async fn clear_loaded_runtime_cache() {
    let mut cache = LOADED_NER_RUNTIME.lock().await;
    *cache = None;
}

pub(crate) fn companion_ner_model_paths(
    app: &AppHandle,
) -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let model_dir = embedding_model_dir(app)?;
    Ok((
        model_dir.join("companion-ner/model.int8.onnx"),
        model_dir.join("companion-ner/tokenizer.json"),
        model_dir.join("companion-ner/config.json"),
    ))
}

pub(crate) fn is_companion_ner_model_installed(app: &AppHandle) -> Result<bool, String> {
    let model_dir = embedding_model_dir(app)?;
    Ok(COMPANION_NER_MODEL_FILES_LOCAL
        .iter()
        .all(|filename| model_dir.join(filename).exists()))
}

pub(crate) async fn extract_entities(
    app: &AppHandle,
    text: &str,
) -> Result<Option<Vec<NamedEntitySpan>>, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    if !is_companion_ner_model_installed(app)? {
        return Ok(None);
    }

    let (model_path, tokenizer_path, config_path) = companion_ner_model_paths(app)?;
    ort_runtime::ensure_ort_init(app).await?;

    let mut cache = LOADED_NER_RUNTIME.lock().await;
    let reuse = cache.as_ref().is_some_and(|loaded| {
        loaded.model_path == model_path
            && loaded.tokenizer_path == tokenizer_path
            && loaded.config_path == config_path
    });

    if !reuse {
        let runtime = create_runtime(app, &model_path, &tokenizer_path, &config_path)?;
        log_info(
            app,
            "companion_ner",
            format!("loaded ner model={}", model_path.display()),
        );
        *cache = Some(runtime);
    }

    let loaded = cache.as_mut().ok_or_else(|| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            "Companion NER runtime cache unavailable",
        )
    })?;

    extract_with_runtime(
        &mut loaded.session,
        &loaded.tokenizer,
        &loaded.labels,
        trimmed,
    )
    .map(|entities| {
        if entities.is_empty() {
            None
        } else {
            Some(entities)
        }
    })
}

fn create_runtime(
    app: &AppHandle,
    model_path: &Path,
    tokenizer_path: &Path,
    config_path: &Path,
) -> Result<LoadedNerRuntime, String> {
    let labels = read_labels(config_path)?;
    let session = Session::builder()
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to create companion NER session builder: {}", e),
            )
        })?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to set companion NER optimization level: {}", e),
            )
        })?
        .commit_from_file(model_path)
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to load companion NER model: {}", e),
            )
        })?;

    let tokenizer = Tokenizer::from_file(tokenizer_path).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!(
                "Failed to load companion NER tokenizer from {:?}: {}",
                tokenizer_path, e
            ),
        )
    })?;

    if labels.is_empty() {
        log_warn(
            app,
            "companion_ner",
            "companion NER config has no labels; using default multilingual NER labels",
        );
    }

    Ok(LoadedNerRuntime {
        model_path: model_path.to_path_buf(),
        tokenizer_path: tokenizer_path.to_path_buf(),
        config_path: config_path.to_path_buf(),
        labels: if labels.is_empty() {
            default_ner_labels()
        } else {
            labels
        },
        session,
        tokenizer,
    })
}

fn extract_with_runtime(
    session: &mut Session,
    tokenizer: &Tokenizer,
    labels: &[String],
    text: &str,
) -> Result<Vec<NamedEntitySpan>, String> {
    let encoding = tokenizer.encode(text, true).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Companion NER tokenization failed: {}", e),
        )
    })?;

    let input_ids = encoding.get_ids();
    let attention_mask = encoding.get_attention_mask();
    let special_tokens_mask = encoding.get_special_tokens_mask();
    let offsets = encoding.get_offsets();
    let word_ids = encoding.get_word_ids();
    let seq_len = input_ids.len().min(MAX_NER_SEQ_LENGTH);

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
            format!("Failed to create NER input_ids tensor: {}", e),
        )
    })?;
    let attention_mask_value =
        Value::from_array(([1, seq_len], attention_mask_i64)).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to create NER attention_mask tensor: {}", e),
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
                format!("Companion NER inference failed: {}", e),
            )
        })?;

    let logits_value = &outputs[0];
    let (_, logits) = logits_value.try_extract_tensor::<f32>().map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to extract NER logits: {}", e),
        )
    })?;

    let label_count = labels.len();
    if label_count == 0 {
        return Ok(Vec::new());
    }

    let expected_len = seq_len.saturating_mul(label_count);
    if logits.len() < expected_len {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!(
                "Unexpected NER logits shape: expected at least {} values, got {}",
                expected_len,
                logits.len()
            ),
        ));
    }

    let mut spans = Vec::new();
    let mut current: Option<SpanAccumulator> = None;

    for index in 0..seq_len {
        if special_tokens_mask.get(index).copied().unwrap_or(0) != 0 {
            flush_entity(text, &mut current, &mut spans);
            continue;
        }

        let (token_start, token_end) = offsets.get(index).copied().unwrap_or((0, 0));
        if token_end <= token_start || token_end > text.len() {
            flush_entity(text, &mut current, &mut spans);
            continue;
        }

        let token_logits = &logits[index * label_count..(index + 1) * label_count];
        let probabilities = softmax(token_logits);
        let Some((label_index, token_score)) = probabilities
            .iter()
            .copied()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        else {
            flush_entity(text, &mut current, &mut spans);
            continue;
        };

        let Some(label) = labels.get(label_index) else {
            flush_entity(text, &mut current, &mut spans);
            continue;
        };

        let Some((prefix, entity_type)) = split_entity_label(label) else {
            flush_entity(text, &mut current, &mut spans);
            continue;
        };

        if token_score < MIN_ENTITY_TOKEN_SCORE {
            flush_entity(text, &mut current, &mut spans);
            continue;
        }

        let word_id = word_ids.get(index).copied().flatten();
        let should_extend = current.as_ref().is_some_and(|active| {
            active.label == entity_type
                && token_start <= active.end.saturating_add(1)
                && (prefix == "I"
                    || (prefix == "B"
                        && active.last_word_id.is_some()
                        && active.last_word_id == word_id)
                    || (word_id.is_some() && active.last_word_id == word_id))
        });

        if should_extend {
            if let Some(active) = current.as_mut() {
                active.end = token_end;
                active.score_sum += token_score;
                active.token_count += 1;
                active.last_word_id = word_id;
            }
            continue;
        }

        flush_entity(text, &mut current, &mut spans);
        current = Some(SpanAccumulator {
            label: entity_type.to_string(),
            start: token_start,
            end: token_end,
            score_sum: token_score,
            token_count: 1,
            last_word_id: word_id,
        });
    }

    flush_entity(text, &mut current, &mut spans);
    Ok(spans)
}

fn flush_entity(
    text: &str,
    current: &mut Option<SpanAccumulator>,
    spans: &mut Vec<NamedEntitySpan>,
) {
    let Some(active) = current.take() else {
        return;
    };

    let score = active.score_sum / active.token_count.max(1) as f32;
    if score < MIN_ENTITY_SPAN_SCORE {
        return;
    }

    let Some((start, end, span_text)) = normalize_span_text(text, active.start, active.end) else {
        return;
    };

    spans.push(NamedEntitySpan {
        label: active.label,
        text: span_text,
        score,
        start,
        end,
    });
}

fn normalize_span_text(text: &str, start: usize, end: usize) -> Option<(usize, usize, String)> {
    let raw = text.get(start..end)?;
    let trimmed = raw.trim_matches(|ch: char| ch.is_whitespace() || ",;:()[]{}".contains(ch));
    if trimmed.is_empty() {
        return None;
    }

    let leading_trim = raw.len().saturating_sub(raw.trim_start().len());
    let trailing_trim = raw.len().saturating_sub(raw.trim_end().len());
    let mut normalized_start = start + leading_trim;
    let mut normalized_end = end.saturating_sub(trailing_trim);

    while normalized_start < normalized_end {
        let Some(ch) = text[normalized_start..normalized_end].chars().next() else {
            break;
        };
        if !(ch.is_whitespace() || ",;:()[]{}".contains(ch)) {
            break;
        }
        normalized_start += ch.len_utf8();
    }

    while normalized_end > normalized_start {
        let Some(ch) = text[..normalized_end].chars().next_back() else {
            break;
        };
        if !(ch.is_whitespace() || ",;:()[]{}".contains(ch)) {
            break;
        }
        normalized_end = normalized_end.saturating_sub(ch.len_utf8());
    }

    let normalized = text.get(normalized_start..normalized_end)?.trim();
    if normalized.is_empty() {
        return None;
    }

    Some((normalized_start, normalized_end, normalized.to_string()))
}

fn read_labels(config_path: &Path) -> Result<Vec<String>, String> {
    let raw = fs::read_to_string(config_path).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!(
                "Failed to read companion NER config {}: {}",
                config_path.display(),
                e
            ),
        )
    })?;
    let config: NerModelConfig = serde_json::from_str(&raw).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to parse companion NER config: {}", e),
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

fn split_entity_label(label: &str) -> Option<(&str, &str)> {
    if label == "O" {
        return None;
    }
    let (prefix, entity) = label.split_once('-')?;
    if prefix != "B" && prefix != "I" {
        return None;
    }
    Some((prefix, entity))
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

fn default_ner_labels() -> Vec<String> {
    [
        "O", "B-DATE", "I-DATE", "B-PER", "I-PER", "B-ORG", "I-ORG", "B-LOC", "I-LOC",
    ]
    .iter()
    .map(|label| label.to_string())
    .collect()
}
