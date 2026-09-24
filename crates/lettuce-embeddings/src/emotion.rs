use std::{
    ops::Range,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use lettuce_companions::{EmotionClassification, EmotionLabelScore};
use lettuce_jobs::{ResourceClass, handle::CancellationToken};
use lettuce_model_hub::VerifiedCompanionEmotionArtifacts;
use ort::{
    inputs,
    session::{RunOptions, Session, builder::GraphOptimizationLevel},
    tensor::TensorElementType,
    value::{Value, ValueType},
};
use tokenizers::Tokenizer;

use crate::onnx::{
    OnnxRuntimeLink, configure_execution_provider, initialize_onnx_runtime, spawn_canceller,
};

const MAX_INPUT_BYTES: usize = 1024 * 1024;
const WINDOWS_PER_RUN: usize = 8;
const PROBABILITIES_OUTPUT: &str = "probabilities";
const BOS_TOKEN: &str = "<s>";
const EOS_TOKEN: &str = "</s>";
const PAD_TOKEN: &str = "<pad>";
const UNK_TOKEN: &str = "<unk>";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SpecialTokens {
    bos: u32,
    eos: u32,
    pad: u32,
    unk: u32,
}

/// Lettuce Thymos multi-label emotion tagging. Long text is split into
/// overlapping token windows exactly as the upstream `inference.py` does and
/// each label keeps its highest window probability.
#[derive(Debug)]
pub struct OnnxEmotionClassifier {
    source_revision: String,
    labels: Vec<String>,
    thresholds: Vec<f32>,
    window: usize,
    stride: usize,
    special: SpecialTokens,
    session: Session,
    tokenizer: Tokenizer,
}

impl OnnxEmotionClassifier {
    pub fn load(
        artifacts: VerifiedCompanionEmotionArtifacts,
        runtime: &OnnxRuntimeLink,
    ) -> Result<Self, EmotionClassifierError> {
        initialize_onnx_runtime(runtime).map_err(|_| EmotionClassifierError::RuntimeUnavailable)?;
        let builder = Session::builder()
            .map_err(|_| EmotionClassifierError::RuntimeUnavailable)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|_| EmotionClassifierError::RuntimeUnavailable)?;
        let session = configure_execution_provider(builder, &artifacts.model_path)
            .map_err(|_| EmotionClassifierError::RuntimeUnavailable)?
            .commit_from_file(&artifacts.model_path)
            .map_err(|_| EmotionClassifierError::ModelLoad)?;
        let has_input = |name: &str| session.inputs.iter().any(|input| input.name == name);
        let label_count = i64::try_from(artifacts.labels.labels().len())
            .map_err(|_| EmotionClassifierError::InvalidOutput)?;
        let output_width = session
            .outputs
            .iter()
            .find(|output| output.name == PROBABILITIES_OUTPUT)
            .and_then(|output| match &output.output_type {
                ValueType::Tensor {
                    ty: TensorElementType::Float32,
                    shape,
                    ..
                } if shape.len() == 2 => Some(shape[1]),
                _ => None,
            });
        let Some(output_width) = output_width else {
            return Err(EmotionClassifierError::InvalidOutput);
        };
        if !has_input("input_ids")
            || !has_input("attention_mask")
            || session.inputs.len() != 2
            || (output_width >= 0 && output_width != label_count)
        {
            return Err(EmotionClassifierError::InvalidOutput);
        }
        let mut tokenizer = Tokenizer::from_file(&artifacts.tokenizer_path)
            .map_err(|_| EmotionClassifierError::TokenizerLoad)?;
        tokenizer
            .with_truncation(None)
            .map_err(|_| EmotionClassifierError::TokenizerLoad)?;
        tokenizer.with_padding(None);
        let token = |name: &str| {
            tokenizer
                .token_to_id(name)
                .ok_or(EmotionClassifierError::TokenizerLoad)
        };
        let special = SpecialTokens {
            bos: token(BOS_TOKEN)?,
            eos: token(EOS_TOKEN)?,
            pad: token(PAD_TOKEN)?,
            unk: token(UNK_TOKEN)?,
        };
        let mut classifier = Self {
            source_revision: artifacts.source_revision,
            labels: artifacts.labels.labels().to_vec(),
            thresholds: artifacts.labels.thresholds().to_vec(),
            window: artifacts.labels.window(),
            stride: artifacts.labels.stride(),
            special,
            session,
            tokenizer,
        };
        if output_width < 0 {
            let span = 0..1;
            let probe = WindowBatch::new(&[special.unk], std::slice::from_ref(&span), special);
            for run in probe.runs() {
                classifier.run(&run, &CancellationToken::new())?;
            }
        }
        Ok(classifier)
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
        &self.source_revision
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
            .encode(trimmed, false)
            .map_err(|_| EmotionClassifierError::Tokenization)?;
        let mut ids = encoding.get_ids().to_vec();
        if ids.is_empty() {
            ids.push(self.special.unk);
        }
        let spans = window_spans(ids.len(), self.window - 2, self.stride);
        let batch = WindowBatch::new(&ids, &spans, self.special);
        let mut pooled = vec![f32::NEG_INFINITY; self.labels.len()];
        for run in batch.runs() {
            let probabilities = self.run(&run, cancellation)?;
            max_pool(&mut pooled, &probabilities, run.rows)?;
        }
        classification_from_probabilities(&self.labels, &self.thresholds, &pooled).map(Some)
    }

    fn run(
        &mut self,
        run: &WindowBatch,
        cancellation: &CancellationToken,
    ) -> Result<Vec<f32>, EmotionClassifierError> {
        if cancellation.is_cancelled() {
            return Err(EmotionClassifierError::Cancelled);
        }
        let count = run.rows;
        let input_ids = Value::from_array(([count, run.width], run.input_ids.clone()))
            .map_err(|_| EmotionClassifierError::InvalidInputTensor)?;
        let attention_mask = Value::from_array(([count, run.width], run.attention_mask.clone()))
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
            .get(PROBABILITIES_OUTPUT)
            .ok_or(EmotionClassifierError::InvalidOutput)?;
        let (shape, values) = output
            .try_extract_tensor::<f32>()
            .map_err(|_| EmotionClassifierError::InvalidOutput)?;
        let expected = [
            i64::try_from(count).map_err(|_| EmotionClassifierError::InvalidOutput)?,
            i64::try_from(self.labels.len()).map_err(|_| EmotionClassifierError::InvalidOutput)?,
        ];
        if shape.len() != 2 || shape[0] != expected[0] || shape[1] != expected[1] {
            return Err(EmotionClassifierError::InvalidOutput);
        }
        Ok(values.to_vec())
    }
}

/// The content-token ranges of each window, as `inference.py`'s `_chunks`
/// builds them: windows start every `stride` tokens, hold up to `content`
/// tokens, and stop once a window reaches the last token.
fn window_spans(token_count: usize, content: usize, stride: usize) -> Vec<Range<usize>> {
    let mut spans = Vec::new();
    let mut start = 0;
    while start < token_count.max(1) {
        let end = token_count.min(start + content);
        if end <= start {
            break;
        }
        spans.push(start..end);
        if start + content >= token_count {
            break;
        }
        start += stride;
    }
    spans
}

/// Every window wrapped in its own bos/eos, right-padded to the longest
/// window with the pad token and a zero attention mask.
#[derive(Debug, Clone, PartialEq, Eq)]
struct WindowBatch {
    rows: usize,
    width: usize,
    input_ids: Vec<i64>,
    attention_mask: Vec<i64>,
}

impl WindowBatch {
    fn new(ids: &[u32], spans: &[Range<usize>], special: SpecialTokens) -> Self {
        let width = spans.iter().map(ExactSizeIterator::len).max().unwrap_or(0) + 2;
        let mut input_ids = vec![i64::from(special.pad); spans.len() * width];
        let mut attention_mask = vec![0_i64; spans.len() * width];
        for (row, span) in spans.iter().enumerate() {
            let tokens = std::iter::once(special.bos)
                .chain(ids[span.clone()].iter().copied())
                .chain(std::iter::once(special.eos));
            for (column, token) in tokens.enumerate() {
                input_ids[row * width + column] = i64::from(token);
                attention_mask[row * width + column] = 1;
            }
        }
        Self {
            rows: spans.len(),
            width,
            input_ids,
            attention_mask,
        }
    }

    /// Consecutive runs of at most eight windows, as `inference.py`
    /// batches them; every run keeps the width of the whole batch.
    fn runs(&self) -> Vec<Self> {
        (0..self.rows)
            .step_by(WINDOWS_PER_RUN)
            .map(|start| {
                let rows = WINDOWS_PER_RUN.min(self.rows - start);
                let cells = start * self.width..(start + rows) * self.width;
                Self {
                    rows,
                    width: self.width,
                    input_ids: self.input_ids[cells.clone()].to_vec(),
                    attention_mask: self.attention_mask[cells].to_vec(),
                }
            })
            .collect()
    }
}

/// Folds one run's `[rows, labels]` probabilities into the per-label
/// maximum. The model output is already sigmoided.
fn max_pool(
    pooled: &mut [f32],
    probabilities: &[f32],
    rows: usize,
) -> Result<(), EmotionClassifierError> {
    if probabilities.len() != rows * pooled.len()
        || probabilities
            .iter()
            .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
    {
        return Err(EmotionClassifierError::InvalidOutput);
    }
    for row in probabilities.chunks_exact(pooled.len()) {
        for (best, value) in pooled.iter_mut().zip(row) {
            *best = best.max(*value);
        }
    }
    Ok(())
}

/// Scores sorted by descending probability, each with its calibrated
/// threshold; confidence is the best of the top three scores.
fn classification_from_probabilities(
    labels: &[String],
    thresholds: &[f32],
    probabilities: &[f32],
) -> Result<EmotionClassification, EmotionClassifierError> {
    if probabilities.is_empty()
        || probabilities.len() != labels.len()
        || thresholds.len() != labels.len()
        || probabilities
            .iter()
            .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
    {
        return Err(EmotionClassifierError::InvalidOutput);
    }
    let mut scored = labels
        .iter()
        .zip(thresholds)
        .zip(probabilities)
        .map(|((label, threshold), score)| EmotionLabelScore {
            label: label.clone(),
            score: *score,
            threshold: *threshold,
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

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EmotionClassifierError {
    #[error("ONNX Runtime is unavailable")]
    RuntimeUnavailable,
    #[error("emotion model could not be loaded")]
    ModelLoad,
    #[error("emotion tokenizer could not be loaded")]
    TokenizerLoad,
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
    use super::*;

    const SPECIAL: SpecialTokens = SpecialTokens {
        bos: 0,
        eos: 2,
        pad: 1,
        unk: 3,
    };

    #[test]
    fn windows_match_inference_py_boundaries() {
        let starts = |tokens: usize| {
            window_spans(tokens, 94, 70)
                .into_iter()
                .map(|span| (span.start, span.len()))
                .collect::<Vec<_>>()
        };
        assert_eq!(starts(1), [(0, 1)]);
        assert_eq!(starts(7), [(0, 7)]);
        assert_eq!(starts(94), [(0, 94)]);
        assert_eq!(starts(95), [(0, 94), (70, 25)]);
        assert_eq!(starts(164), [(0, 94), (70, 94)]);
        assert_eq!(starts(165), [(0, 94), (70, 94), (140, 25)]);
        assert_eq!(starts(267), [(0, 94), (70, 94), (140, 94), (210, 57)]);
        assert_eq!(window_spans(0, 94, 70), Vec::<Range<usize>>::new());
        assert_eq!(
            window_spans(5, 2, 1),
            [0..2, 1..3, 2..4, 3..5],
            "the smallest valid window"
        );
    }

    #[test]
    fn every_window_carries_its_own_bos_and_eos_and_pads_to_the_longest() {
        let ids = (10..17).collect::<Vec<u32>>();
        let spans = window_spans(ids.len(), 4, 3);
        assert_eq!(spans, [0..4, 3..7]);
        let batch = WindowBatch::new(&ids, &spans, SPECIAL);
        assert_eq!((batch.rows, batch.width), (2, 6));
        assert_eq!(
            batch.input_ids,
            [0, 10, 11, 12, 13, 2, 0, 13, 14, 15, 16, 2]
        );
        assert_eq!(batch.attention_mask, [1; 12]);
        let spans = window_spans(6, 4, 3);
        assert_eq!(spans, [0..4, 3..6]);
        let batch = WindowBatch::new(&ids, &spans, SPECIAL);
        assert_eq!(batch.input_ids, [0, 10, 11, 12, 13, 2, 0, 13, 14, 15, 2, 1]);
        assert_eq!(batch.attention_mask, [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0]);
    }

    #[test]
    fn more_than_eight_windows_run_in_slices_of_eight_with_one_shared_width() {
        let ids = (100..300).collect::<Vec<u32>>();
        let spans = window_spans(ids.len(), 10, 7);
        assert_eq!(spans.len(), 29);
        assert_eq!(spans.last(), Some(&(196..200)));
        let short = [
            0..2,
            2..14,
            14..26,
            26..38,
            38..50,
            50..62,
            62..74,
            74..86,
            86..98,
            98..99,
        ];
        let batch = WindowBatch::new(&ids, &short, SPECIAL);
        assert_eq!((batch.rows, batch.width), (10, 14));
        let runs = batch.runs();
        assert_eq!(
            runs.iter()
                .map(|run| (run.rows, run.width))
                .collect::<Vec<_>>(),
            [(8, 14), (2, 14)]
        );
        assert_eq!(runs[0].input_ids.len(), 8 * 14);
        assert_eq!(runs[1].input_ids.len(), 2 * 14);
        assert_eq!(
            runs[0].input_ids[..14],
            [0, 100, 101, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1]
        );
        assert_eq!(
            runs[0].attention_mask[..14],
            [1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        let row =
            |run: &WindowBatch, index: usize| run.input_ids[index * 14..(index + 1) * 14].to_vec();
        assert_eq!(
            row(&runs[1], 0),
            [
                0, 186, 187, 188, 189, 190, 191, 192, 193, 194, 195, 196, 197, 2
            ]
        );
        assert_eq!(
            row(&runs[1], 1),
            [0, 198, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1]
        );
        assert_eq!(
            runs.iter()
                .flat_map(|run| run.input_ids.clone())
                .collect::<Vec<_>>(),
            batch.input_ids
        );
        assert_eq!(
            runs.iter()
                .flat_map(|run| run.attention_mask.clone())
                .collect::<Vec<_>>(),
            batch.attention_mask
        );
        let exact = WindowBatch::new(&ids, &spans[..16], SPECIAL).runs();
        assert_eq!(exact.iter().map(|run| run.rows).collect::<Vec<_>>(), [8, 8]);
    }

    #[test]
    fn windows_pool_by_the_per_label_maximum() {
        let mut pooled = vec![f32::NEG_INFINITY; 3];
        max_pool(&mut pooled, &[0.1, 0.9, 0.2, 0.7, 0.3, 0.2], 2).expect("first run");
        max_pool(&mut pooled, &[0.05, 0.1, 0.6], 1).expect("second run");
        assert_eq!(pooled, [0.7, 0.9, 0.6]);
        assert_eq!(
            max_pool(&mut pooled, &[0.1, 0.2], 1),
            Err(EmotionClassifierError::InvalidOutput)
        );
        assert_eq!(
            max_pool(&mut pooled, &[0.1, f32::NAN, 0.2], 1),
            Err(EmotionClassifierError::InvalidOutput)
        );
        assert_eq!(
            max_pool(&mut pooled, &[0.1, 1.5, 0.2], 1),
            Err(EmotionClassifierError::InvalidOutput)
        );
    }

    #[test]
    fn probabilities_are_used_as_is_with_their_thresholds_and_top_three_confidence() {
        let labels = vec!["love".into(), "anger".into(), "neutral".into()];
        let classification =
            classification_from_probabilities(&labels, &[0.438, 0.176, 0.249], &[0.2, 0.9, 0.0])
                .expect("classification");
        assert_eq!(
            classification.labels,
            [
                EmotionLabelScore {
                    label: "anger".into(),
                    score: 0.9,
                    threshold: 0.176,
                },
                EmotionLabelScore {
                    label: "love".into(),
                    score: 0.2,
                    threshold: 0.438,
                },
                EmotionLabelScore {
                    label: "neutral".into(),
                    score: 0.0,
                    threshold: 0.249,
                },
            ]
        );
        assert_eq!(classification.confidence, f64::from(0.9_f32));
    }

    #[test]
    fn invalid_probabilities_fail_closed() {
        let labels = vec!["love".into()];
        for probabilities in [&[][..], &[f32::NAN], &[-0.1], &[1.1], &[0.5, 0.5]] {
            assert_eq!(
                classification_from_probabilities(&labels, &[0.4], probabilities),
                Err(EmotionClassifierError::InvalidOutput)
            );
        }
        assert_eq!(
            classification_from_probabilities(&labels, &[], &[0.5]),
            Err(EmotionClassifierError::InvalidOutput)
        );
    }

    #[test]
    #[ignore = "requires a downloaded Thymos install and ONNX Runtime 1.22 (the reference values were taken with it; 1.23+ differs up to 1.4e-3 on the long text)"]
    fn live_thymos_matches_the_upstream_reference() {
        let root = std::env::var_os("LETTUCE_TEST_THYMOS_ROOT")
            .map(std::path::PathBuf::from)
            .expect("Thymos install root");
        let runtime = std::env::var_os("LETTUCE_TEST_ONNX_RUNTIME")
            .map(std::path::PathBuf::from)
            .expect("runtime path");
        let manifest = lettuce_model_hub::CompanionEmotionInstallStore::open(&root)
            .expect("store")
            .installed()
            .expect("manifest")
            .expect("installed");
        let verified = manifest.verify().expect("verified artifacts");
        assert_eq!(
            (verified.labels.window(), verified.labels.stride()),
            (96, 70)
        );
        let mut classifier =
            OnnxEmotionClassifier::load(verified, &OnnxRuntimeLink::Dynamic(runtime))
                .expect("classifier");
        let score = |classification: &EmotionClassification, label: &str| {
            classification
                .labels
                .iter()
                .find(|item| item.label == label)
                .map(|item| item.score)
                .expect("label")
        };
        let short = classifier
            .classify("I love spending time with you.", &CancellationToken::new())
            .expect("inference")
            .expect("nonblank");
        assert_eq!(short.labels.len(), 28);
        assert_eq!(short.labels[0].label, "love");
        assert!((score(&short, "love") - 0.90752).abs() < 1e-4);
        assert!((score(&short, "joy") - 0.0674).abs() < 1e-4);
        assert!((short.confidence - 0.90752).abs() < 1e-4);

        let long = format!(
            "{}But then you called this morning and explained everything, and honestly I feel \
             relieved and grateful that you are safe. Thank you for telling me the truth.",
            "I waited all night for you and you never came. I am so angry and hurt, I can't \
             believe you would do this to me again after everything we talked about. "
                .repeat(6)
        );
        let long = classifier
            .classify(&long, &CancellationToken::new())
            .expect("inference")
            .expect("nonblank");
        for (label, expected) in [
            ("gratitude", 0.95087),
            ("anger", 0.66684),
            ("annoyance", 0.33348),
            ("sadness", 0.24214),
            ("disappointment", 0.19362),
        ] {
            assert!(
                (score(&long, label) - expected).abs() < 1e-4,
                "{label}: {}",
                score(&long, label)
            );
        }
        let cancelled = CancellationToken::new();
        cancelled.cancel();
        assert_eq!(
            classifier.classify("hello", &cancelled),
            Err(EmotionClassifierError::Cancelled)
        );
    }
}
