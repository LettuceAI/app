use std::{
    collections::HashMap,
    fs,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use lettuce_companions::{EmotionClassification, EmotionLabelScore};
use lettuce_jobs::{ResourceClass, handle::CancellationToken};
use lettuce_model_hub::{COMPANION_EMOTION_MAX_SEQUENCE_LENGTH, VerifiedCompanionEmotionArtifacts};
use ort::{
    inputs,
    session::{RunOptions, Session, builder::GraphOptimizationLevel},
    value::Value,
};
use serde::Deserialize;
use tokenizers::Tokenizer;

use crate::onnx::{
    OnnxRuntimeLink, configure_execution_provider, initialize_onnx_runtime, spawn_canceller,
};

const MAX_INPUT_BYTES: usize = 1024 * 1024;

#[derive(Debug)]
pub struct OnnxEmotionClassifier {
    artifacts: VerifiedCompanionEmotionArtifacts,
    labels: Vec<String>,
    session: Session,
    tokenizer: Tokenizer,
}

impl OnnxEmotionClassifier {
    pub fn load(
        artifacts: VerifiedCompanionEmotionArtifacts,
        runtime: &OnnxRuntimeLink,
    ) -> Result<Self, EmotionClassifierError> {
        initialize_onnx_runtime(runtime).map_err(|_| EmotionClassifierError::RuntimeUnavailable)?;
        let labels = read_labels(&artifacts.config_path)?;
        let builder = Session::builder()
            .map_err(|_| EmotionClassifierError::RuntimeUnavailable)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|_| EmotionClassifierError::RuntimeUnavailable)?;
        let session = configure_execution_provider(builder, &artifacts.model_path)
            .map_err(|_| EmotionClassifierError::RuntimeUnavailable)?
            .commit_from_file(&artifacts.model_path)
            .map_err(|_| EmotionClassifierError::ModelLoad)?;
        if session.outputs.is_empty() {
            return Err(EmotionClassifierError::InvalidOutput);
        }
        let tokenizer = Tokenizer::from_file(&artifacts.tokenizer_path)
            .map_err(|_| EmotionClassifierError::TokenizerLoad)?;
        Ok(Self {
            artifacts,
            labels: if labels.is_empty() {
                default_go_emotions_labels()
            } else {
                labels
            },
            session,
            tokenizer,
        })
    }

    #[must_use]
    pub const fn required_resources() -> [ResourceClass; 3] {
        [
            ResourceClass::ModelLoad,
            ResourceClass::DiskRead,
            ResourceClass::Cpu,
        ]
    }

    #[must_use]
    pub fn source_revision(&self) -> &str {
        &self.artifacts.source_revision
    }

    pub fn classify(
        &mut self,
        text: &str,
        cancellation: &CancellationToken,
    ) -> Result<Option<EmotionClassification>, EmotionClassifierError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        if text.len() > MAX_INPUT_BYTES {
            return Err(EmotionClassifierError::InputTooLarge);
        }
        if cancellation.is_cancelled() {
            return Err(EmotionClassifierError::Cancelled);
        }
        let encoding = self
            .tokenizer
            .encode(trimmed, true)
            .map_err(|_| EmotionClassifierError::Tokenization)?;
        let sequence_length = encoding.len().min(COMPANION_EMOTION_MAX_SEQUENCE_LENGTH);
        if sequence_length == 0 {
            return Err(EmotionClassifierError::Tokenization);
        }
        let input_ids = encoding.get_ids()[..sequence_length]
            .iter()
            .map(|value| i64::from(*value))
            .collect::<Vec<_>>();
        let attention_mask = encoding.get_attention_mask()[..sequence_length]
            .iter()
            .map(|value| i64::from(*value))
            .collect::<Vec<_>>();
        if cancellation.is_cancelled() {
            return Err(EmotionClassifierError::Cancelled);
        }
        let input_ids = Value::from_array(([1, sequence_length], input_ids))
            .map_err(|_| EmotionClassifierError::InvalidInputTensor)?;
        let attention_mask = Value::from_array(([1, sequence_length], attention_mask))
            .map_err(|_| EmotionClassifierError::InvalidInputTensor)?;
        let run_options =
            Arc::new(RunOptions::new().map_err(|_| EmotionClassifierError::Inference)?);
        let finished = Arc::new(AtomicBool::new(false));
        let canceller = spawn_canceller(
            Arc::clone(&run_options),
            Arc::clone(&finished),
            cancellation.clone(),
        );
        let result = self.session.run_with_options(
            inputs![
                "input_ids" => input_ids,
                "attention_mask" => attention_mask
            ],
            run_options.as_ref(),
        );
        finished.store(true, Ordering::Release);
        if canceller.join().is_err() {
            return Err(EmotionClassifierError::CancellationMonitor);
        }
        let outputs = result.map_err(|_| {
            if cancellation.is_cancelled() {
                EmotionClassifierError::Cancelled
            } else {
                EmotionClassifierError::Inference
            }
        })?;
        if cancellation.is_cancelled() {
            return Err(EmotionClassifierError::Cancelled);
        }
        let output = outputs
            .values()
            .next()
            .ok_or(EmotionClassifierError::InvalidOutput)?;
        let (_, logits) = output
            .try_extract_tensor::<f32>()
            .map_err(|_| EmotionClassifierError::InvalidOutput)?;
        classification_from_logits(&self.labels, logits).map(Some)
    }
}

#[derive(Debug, Deserialize)]
struct EmotionModelConfig {
    #[serde(default)]
    id2label: HashMap<String, String>,
}

fn read_labels(path: &std::path::Path) -> Result<Vec<String>, EmotionClassifierError> {
    let raw = fs::read_to_string(path).map_err(|_| EmotionClassifierError::ConfigLoad)?;
    let config: EmotionModelConfig =
        serde_json::from_str(&raw).map_err(|_| EmotionClassifierError::ConfigLoad)?;
    let mut labels = config
        .id2label
        .into_iter()
        .filter_map(|(key, label)| key.parse::<usize>().ok().map(|index| (index, label)))
        .collect::<Vec<_>>();
    labels.sort_by_key(|(index, _)| *index);
    Ok(labels.into_iter().map(|(_, label)| label).collect())
}

fn classification_from_logits(
    labels: &[String],
    logits: &[f32],
) -> Result<EmotionClassification, EmotionClassifierError> {
    if logits.is_empty() || logits.iter().any(|value| !value.is_finite()) {
        return Err(EmotionClassifierError::InvalidOutput);
    }
    let mut scored = logits
        .iter()
        .enumerate()
        .map(|(index, logit)| EmotionLabelScore {
            label: labels
                .get(index)
                .cloned()
                .unwrap_or_else(|| format!("label_{index}")),
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
        .map(|item| f64::from(item.score))
        .fold(0.0, f64::max)
        .clamp(0.0, 1.0);
    Ok(EmotionClassification {
        labels: scored,
        confidence,
    })
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

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EmotionClassifierError {
    #[error("ONNX Runtime is unavailable")]
    RuntimeUnavailable,
    #[error("emotion model could not be loaded")]
    ModelLoad,
    #[error("emotion tokenizer could not be loaded")]
    TokenizerLoad,
    #[error("emotion model config could not be loaded")]
    ConfigLoad,
    #[error("emotion classifier input is too large")]
    InputTooLarge,
    #[error("emotion tokenization failed")]
    Tokenization,
    #[error("emotion input tensor is invalid")]
    InvalidInputTensor,
    #[error("emotion inference failed")]
    Inference,
    #[error("emotion output is invalid")]
    InvalidOutput,
    #[error("emotion inference was cancelled")]
    Cancelled,
    #[error("emotion cancellation monitor failed")]
    CancellationMonitor,
}

#[cfg(test)]
mod tests {
    use lettuce_model_hub::{InstalledCompanionEmotionManifest, InstalledModelArtifact};
    use lettuce_types::OperationId;

    use super::*;

    #[test]
    fn config_labels_are_sorted_numerically_and_nonnumeric_keys_are_ignored() {
        let path = std::env::temp_dir().join(format!("emotion-config-{}.json", OperationId::new()));
        std::fs::write(
            &path,
            br#"{"id2label":{"10":"ten","2":"two","bad":"ignored","0":"zero"}}"#,
        )
        .expect("config");
        assert_eq!(read_labels(&path).expect("labels"), ["zero", "two", "ten"]);
        std::fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn empty_config_uses_the_exact_legacy_go_emotions_order() {
        let labels = default_go_emotions_labels();
        assert_eq!(labels.len(), 28);
        assert_eq!(labels[0], "admiration");
        assert_eq!(labels[18], "love");
        assert_eq!(labels[27], "neutral");
    }

    #[test]
    fn logits_use_sigmoid_descending_sort_fallback_names_and_top_three_confidence() {
        let labels = vec!["first".into(), "second".into()];
        let classification =
            classification_from_logits(&labels, &[-1.0, 2.0, 0.0]).expect("classification");
        assert_eq!(classification.labels[0].label, "second");
        assert_eq!(classification.labels[1].label, "label_2");
        assert_eq!(classification.labels[2].label, "first");
        assert!((classification.labels[0].score - sigmoid(2.0)).abs() < f32::EPSILON);
        assert_eq!(
            classification.confidence,
            f64::from(classification.labels[0].score)
        );
    }

    #[test]
    fn invalid_logits_fail_closed() {
        assert_eq!(
            classification_from_logits(&[], &[]),
            Err(EmotionClassifierError::InvalidOutput)
        );
        assert_eq!(
            classification_from_logits(&[], &[f32::NAN]),
            Err(EmotionClassifierError::InvalidOutput)
        );
    }

    fn artifact(path: std::path::PathBuf) -> InstalledModelArtifact {
        InstalledModelArtifact::inspect(path).expect("artifact")
    }

    #[test]
    #[ignore = "requires audited local companion-emotion model and ONNX Runtime paths"]
    fn live_companion_emotion_model_classifies_text() {
        let model = std::env::var_os("LETTUCE_TEST_EMOTION_MODEL")
            .map(std::path::PathBuf::from)
            .expect("model path");
        let tokenizer = std::env::var_os("LETTUCE_TEST_EMOTION_TOKENIZER")
            .map(std::path::PathBuf::from)
            .expect("tokenizer path");
        let config = std::env::var_os("LETTUCE_TEST_EMOTION_CONFIG")
            .map(std::path::PathBuf::from)
            .expect("config path");
        let runtime = std::env::var_os("LETTUCE_TEST_ONNX_RUNTIME")
            .map(std::path::PathBuf::from)
            .expect("runtime path");
        let manifest = InstalledCompanionEmotionManifest {
            source_revision: "legacy-live-test".into(),
            model: artifact(model),
            tokenizer: artifact(tokenizer),
            config: artifact(config),
        };
        let verified = manifest.verify().expect("verified artifacts");
        let mut classifier =
            OnnxEmotionClassifier::load(verified, &OnnxRuntimeLink::Dynamic(runtime))
                .expect("classifier");
        let classification = classifier
            .classify("I love spending time with you.", &CancellationToken::new())
            .expect("inference")
            .expect("nonblank");
        assert_eq!(classification.labels.len(), 28);
        assert!((0.0..=1.0).contains(&classification.confidence));
    }
}
