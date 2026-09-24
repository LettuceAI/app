use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use lettuce_jobs::{ResourceClass, handle::CancellationToken};
use lettuce_model_hub::{EmbeddingModelFamily, VerifiedEmbeddingArtifacts};
use ort::{
    inputs,
    session::{RunOptions, Session, builder::GraphOptimizationLevel},
    value::Value,
};
use tokenizers::{Encoding, PostProcessor, Tokenizer, TruncationDirection};

use crate::SimilarityCalibration;

const MAX_INPUT_BYTES: usize = 1024 * 1024;
const MAX_CALIBRATION_BYTES: u64 = 64 * 1024;
const MIN_EMBEDDING_TOKENS: usize = 512;
const MAX_EMBEDDING_TOKENS: usize = 4096;

/// The token budget one embedding may use: the user's `embeddingMaxTokens`
/// (unset means 4,096, clamped to 512..=4,096) bounded by what the installed
/// model was trained on.
#[must_use]
pub fn effective_max_sequence_length(model_max: usize, preference: Option<u16>) -> usize {
    let preferred = preference
        .map_or(MAX_EMBEDDING_TOKENS, usize::from)
        .clamp(MIN_EMBEDDING_TOKENS, MAX_EMBEDDING_TOKENS);
    model_max.min(preferred)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnnxRuntimeLink {
    Dynamic(PathBuf),
    Linked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddingDimensions {
    D64,
    D128,
    D256,
    D512,
    D768,
}

impl EmbeddingDimensions {
    #[must_use]
    pub const fn from_preference(preferred: Option<u16>) -> Self {
        match preferred {
            Some(64) => Self::D64,
            Some(128) => Self::D128,
            Some(256) => Self::D256,
            Some(512) => Self::D512,
            _ => Self::D768,
        }
    }

    /// The dimension a stored vector of `len` values has.
    #[must_use]
    pub const fn from_len(len: usize) -> Option<Self> {
        match len {
            64 => Some(Self::D64),
            128 => Some(Self::D128),
            256 => Some(Self::D256),
            512 => Some(Self::D512),
            768 => Some(Self::D768),
            _ => None,
        }
    }

    #[must_use]
    pub const fn get(self) -> usize {
        match self {
            Self::D64 => 64,
            Self::D128 => 128,
            Self::D256 => 256,
            Self::D512 => 512,
            Self::D768 => 768,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddingRequest {
    pub text: String,
    pub dimensions: EmbeddingDimensions,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddingVector {
    pub source_revision: String,
    pub values: Vec<f32>,
}

impl EmbeddingVector {
    #[must_use]
    pub fn cosine_similarity(&self, other: &Self) -> Option<f32> {
        if self.source_revision != other.source_revision
            || self.values.len() != other.values.len()
            || self.values.is_empty()
        {
            return None;
        }
        let (dot, left, right) = self
            .values
            .iter()
            .zip(&other.values)
            .fold((0.0f32, 0.0f32, 0.0f32), |(dot, left, right), (a, b)| {
                (dot + a * b, left + a * a, right + b * b)
            });
        let denominator = left.sqrt() * right.sqrt();
        (denominator > 0.0).then_some(dot / denominator)
    }
}

#[derive(Debug)]
pub struct OnnxEmbeddingRuntime {
    artifacts: VerifiedEmbeddingArtifacts,
    session: Session,
    tokenizer: Tokenizer,
    expects_token_type_ids: bool,
    max_sequence_length: usize,
    calibration: SimilarityCalibration,
}

/// The tensors one embedding run feeds the session, all of one length.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModelInput {
    pub(crate) input_ids: Vec<i64>,
    pub(crate) attention_mask: Vec<i64>,
    pub(crate) token_type_ids: Vec<i64>,
}

impl ModelInput {
    fn len(&self) -> usize {
        self.input_ids.len()
    }
}

/// Drops the truncation and padding a published `tokenizer.json` may carry
/// (v4's cuts every input at 128 tokens) so only the explicit budget applies.
pub(crate) fn prepare_tokenizer(tokenizer: &mut Tokenizer) -> Result<(), EmbeddingError> {
    tokenizer
        .with_truncation(None)
        .map_err(|_| EmbeddingError::TokenizerLoad)?;
    tokenizer.with_padding(None);
    Ok(())
}

/// Encodes `text` with the model's special tokens, cutting the text (never
/// the special tokens) so the whole sequence fits `max_sequence_length`.
pub(crate) fn encode_bounded(
    tokenizer: &Tokenizer,
    text: &str,
    max_sequence_length: usize,
) -> Result<Encoding, EmbeddingError> {
    let mut encoding = tokenizer
        .encode(text, false)
        .map_err(|_| EmbeddingError::Tokenization)?;
    let special = tokenizer
        .get_post_processor()
        .map_or(0, |processor| processor.added_tokens(false));
    let budget = max_sequence_length
        .checked_sub(special)
        .filter(|budget| *budget > 0)
        .ok_or(EmbeddingError::Tokenization)?;
    if encoding.len() > budget {
        encoding.truncate(budget, 0, TruncationDirection::Right);
    }
    tokenizer
        .post_process(encoding, None, true)
        .map_err(|_| EmbeddingError::Tokenization)
}

pub(crate) fn count_tokens(tokenizer: &Tokenizer, text: &str) -> Result<u32, EmbeddingError> {
    let encoding = tokenizer
        .encode(text, false)
        .map_err(|_| EmbeddingError::Tokenization)?;
    u32::try_from(encoding.len()).map_err(|_| EmbeddingError::InputTooLarge)
}

pub(crate) fn model_input(
    tokenizer: &Tokenizer,
    text: &str,
    max_sequence_length: usize,
) -> Result<ModelInput, EmbeddingError> {
    let encoding = encode_bounded(tokenizer, text, max_sequence_length)?;
    let sequence_length = encoding.len().min(max_sequence_length);
    if sequence_length == 0 {
        return Err(EmbeddingError::Tokenization);
    }
    let widen = |values: &[u32]| {
        values[..sequence_length]
            .iter()
            .map(|value| i64::from(*value))
            .collect::<Vec<_>>()
    };
    let type_ids = encoding.get_type_ids();
    Ok(ModelInput {
        input_ids: widen(encoding.get_ids()),
        attention_mask: widen(encoding.get_attention_mask()),
        token_type_ids: if type_ids.len() >= sequence_length {
            widen(type_ids)
        } else {
            vec![0; sequence_length]
        },
    })
}

fn load_calibration(
    artifacts: &VerifiedEmbeddingArtifacts,
) -> Result<SimilarityCalibration, EmbeddingError> {
    match (
        &artifacts.calibration_path,
        artifacts.family.requires_calibration(),
    ) {
        (None, false) => Ok(SimilarityCalibration::RawCosine),
        (Some(path), true) => {
            let file = std::fs::File::open(path).map_err(|_| EmbeddingError::InvalidCalibration)?;
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(
                &mut std::io::Read::take(file, MAX_CALIBRATION_BYTES + 1),
                &mut bytes,
            )
            .map_err(|_| EmbeddingError::InvalidCalibration)?;
            SimilarityCalibration::from_json(&bytes).map_err(|_| EmbeddingError::InvalidCalibration)
        }
        _ => Err(EmbeddingError::InvalidCalibration),
    }
}

impl OnnxEmbeddingRuntime {
    /// Loads a verified install. `max_tokens` is the user's
    /// `embeddingMaxTokens` preference.
    pub fn load(
        artifacts: VerifiedEmbeddingArtifacts,
        runtime: &OnnxRuntimeLink,
        max_tokens: Option<u16>,
    ) -> Result<Self, EmbeddingError> {
        if artifacts.native_dimensions != artifacts.family.native_dimensions()
            || artifacts.max_sequence_length == 0
            || artifacts.max_sequence_length > artifacts.family.max_positions()
        {
            return Err(EmbeddingError::UnsupportedModel);
        }
        let calibration = load_calibration(&artifacts)?;
        initialize_onnx_runtime(runtime)?;
        let builder = Session::builder()
            .map_err(|_| EmbeddingError::RuntimeUnavailable)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|_| EmbeddingError::RuntimeUnavailable)?;
        let session = configure_execution_provider(builder, &artifacts.model_path)?
            .commit_from_file(&artifacts.model_path)
            .map_err(|_| EmbeddingError::ModelLoad)?;
        let mut tokenizer = Tokenizer::from_file(&artifacts.tokenizer_path)
            .map_err(|_| EmbeddingError::TokenizerLoad)?;
        prepare_tokenizer(&mut tokenizer)?;
        let expects_token_type_ids = session
            .inputs
            .iter()
            .any(|input| input.name.contains("token_type_ids"));
        // Force metadata access during load so malformed sessions fail before a job starts.
        if session.outputs.is_empty()
            || artifacts
                .family
                .output_name()
                .is_some_and(|name| !session.outputs.iter().any(|output| output.name == name))
        {
            return Err(EmbeddingError::InvalidOutput);
        }
        let max_sequence_length =
            effective_max_sequence_length(artifacts.max_sequence_length, max_tokens);
        Ok(Self {
            artifacts,
            session,
            tokenizer,
            expects_token_type_ids,
            max_sequence_length,
            calibration,
        })
    }

    #[must_use]
    pub const fn family(&self) -> EmbeddingModelFamily {
        self.artifacts.family
    }

    /// The vector-space label every embedding from this runtime carries.
    #[must_use]
    pub const fn vector_space(&self) -> &'static str {
        self.artifacts.family.vector_space()
    }

    #[must_use]
    pub const fn max_sequence_length(&self) -> usize {
        self.max_sequence_length
    }

    #[must_use]
    pub const fn calibration(&self) -> &SimilarityCalibration {
        &self.calibration
    }

    #[must_use]
    pub const fn required_resources() -> [ResourceClass; 3] {
        [
            ResourceClass::ModelLoad,
            ResourceClass::DiskRead,
            ResourceClass::Cpu,
        ]
    }

    pub fn embed(
        &mut self,
        request: &EmbeddingRequest,
        cancellation: &CancellationToken,
    ) -> Result<EmbeddingVector, EmbeddingError> {
        if request.text.len() > MAX_INPUT_BYTES {
            return Err(EmbeddingError::InputTooLarge);
        }
        if cancellation.is_cancelled() {
            return Err(EmbeddingError::Cancelled);
        }
        let input = model_input(&self.tokenizer, &request.text, self.max_sequence_length)?;
        let sequence_length = input.len();
        if cancellation.is_cancelled() {
            return Err(EmbeddingError::Cancelled);
        }

        let input_ids = Value::from_array(([1, sequence_length], input.input_ids))
            .map_err(|_| EmbeddingError::InvalidInputTensor)?;
        let attention_mask = Value::from_array(([1, sequence_length], input.attention_mask))
            .map_err(|_| EmbeddingError::InvalidInputTensor)?;
        let token_type_ids = Value::from_array(([1, sequence_length], input.token_type_ids))
            .map_err(|_| EmbeddingError::InvalidInputTensor)?;
        let run_options = Arc::new(RunOptions::new().map_err(|_| EmbeddingError::Inference)?);
        let finished = Arc::new(AtomicBool::new(false));
        let canceller = spawn_canceller(
            Arc::clone(&run_options),
            Arc::clone(&finished),
            cancellation.clone(),
        );
        let result = if self.expects_token_type_ids {
            self.session.run_with_options(
                inputs![
                    "input_ids" => input_ids,
                    "attention_mask" => attention_mask,
                    "token_type_ids" => token_type_ids
                ],
                run_options.as_ref(),
            )
        } else {
            self.session.run_with_options(
                inputs![
                    "input_ids" => input_ids,
                    "attention_mask" => attention_mask
                ],
                run_options.as_ref(),
            )
        };
        finished.store(true, Ordering::Release);
        if canceller.join().is_err() {
            return Err(EmbeddingError::CancellationMonitor);
        }
        let outputs = result.map_err(|_| {
            if cancellation.is_cancelled() {
                EmbeddingError::Cancelled
            } else {
                EmbeddingError::Inference
            }
        })?;
        if cancellation.is_cancelled() {
            return Err(EmbeddingError::Cancelled);
        }
        let output = match self.artifacts.family.output_name() {
            Some(name) => outputs.get(name),
            None => (outputs.len() > 0).then(|| &outputs[0]),
        }
        .ok_or(EmbeddingError::InvalidOutput)?;
        let (_, values) = output
            .try_extract_tensor::<f32>()
            .map_err(|_| EmbeddingError::InvalidOutput)?;
        let target = request.dimensions.get();
        if target > self.artifacts.native_dimensions || values.len() < target {
            return Err(EmbeddingError::InvalidOutput);
        }
        let mut values = values[..target].to_vec();
        if values.iter().any(|value| !value.is_finite()) {
            return Err(EmbeddingError::InvalidOutput);
        }
        if target < self.artifacts.native_dimensions {
            l2_normalize(&mut values)?;
        } else if values.iter().all(|value| *value == 0.0) {
            return Err(EmbeddingError::InvalidOutput);
        }
        Ok(EmbeddingVector {
            source_revision: self.artifacts.family.vector_space().to_owned(),
            values,
        })
    }

    /// The text's tokens without special tokens and without any cut.
    pub fn count_tokens(&self, text: &str) -> Result<u32, EmbeddingError> {
        count_tokens(&self.tokenizer, text)
    }
}

pub(crate) fn spawn_canceller(
    run_options: Arc<RunOptions>,
    finished: Arc<AtomicBool>,
    cancellation: CancellationToken,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        while !finished.load(Ordering::Acquire) {
            if cancellation.is_cancelled() {
                let _ = run_options.terminate();
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    })
}

fn l2_normalize(values: &mut [f32]) -> Result<(), EmbeddingError> {
    let norm = values.iter().map(|value| value * value).sum::<f32>().sqrt();
    if !norm.is_finite() || norm <= 0.0 {
        return Err(EmbeddingError::InvalidOutput);
    }
    for value in values {
        *value /= norm;
    }
    Ok(())
}

pub(crate) fn initialize_onnx_runtime(runtime: &OnnxRuntimeLink) -> Result<(), EmbeddingError> {
    let binding = match runtime {
        OnnxRuntimeLink::Dynamic(path) => crate::OnnxRuntimeBinding::Library(path),
        OnnxRuntimeLink::Linked => crate::OnnxRuntimeBinding::Linked,
    };
    crate::initialize_process_onnx_runtime(binding, "lettuce-embeddings")
        .map(|_| ())
        .map_err(|_| EmbeddingError::RuntimeUnavailable)
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub(crate) fn configure_execution_provider(
    builder: ort::session::builder::SessionBuilder,
    model_path: &Path,
) -> Result<ort::session::builder::SessionBuilder, EmbeddingError> {
    use ort::execution_providers::coreml::{
        CoreMLComputeUnits, CoreMLExecutionProvider, CoreMLModelFormat,
        CoreMLSpecializationStrategy,
    };

    let cache = model_path
        .parent()
        .ok_or(EmbeddingError::ModelLoad)?
        .join("coreml-cache");
    std::fs::create_dir_all(cache).map_err(|_| EmbeddingError::ModelLoad)?;
    let provider = CoreMLExecutionProvider::default()
        .with_compute_units(CoreMLComputeUnits::CPUAndNeuralEngine)
        .with_model_format(CoreMLModelFormat::MLProgram)
        .with_specialization_strategy(CoreMLSpecializationStrategy::FastPrediction)
        .with_static_input_shapes(true)
        .build();
    match builder.with_execution_providers([provider]) {
        Ok(builder) => Ok(builder),
        Err(error) => {
            tracing::warn!(error = %error, "CoreML unavailable; using ONNX CPU fallback");
            Session::builder()
                .and_then(|builder| builder.with_optimization_level(GraphOptimizationLevel::Level3))
                .map_err(|_| EmbeddingError::RuntimeUnavailable)
        }
    }
}

#[cfg(not(any(target_os = "ios", target_os = "macos")))]
pub(crate) fn configure_execution_provider(
    builder: ort::session::builder::SessionBuilder,
    _model_path: &Path,
) -> Result<ort::session::builder::SessionBuilder, EmbeddingError> {
    Ok(builder)
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EmbeddingError {
    #[error("embedding model is unsupported")]
    UnsupportedModel,
    #[error("ONNX Runtime is unavailable")]
    RuntimeUnavailable,
    #[error("embedding model could not be loaded")]
    ModelLoad,
    #[error("embedding tokenizer could not be loaded")]
    TokenizerLoad,
    #[error("embedding input is too large")]
    InputTooLarge,
    #[error("embedding tokenization failed")]
    Tokenization,
    #[error("embedding input tensor is invalid")]
    InvalidInputTensor,
    #[error("embedding inference failed")]
    Inference,
    #[error("embedding output is invalid")]
    InvalidOutput,
    #[error("embedding inference was cancelled")]
    Cancelled,
    #[error("embedding cancellation monitor failed")]
    CancellationMonitor,
    #[error("embedding similarity calibration is missing or invalid")]
    InvalidCalibration,
}

#[cfg(test)]
mod tests {
    use lettuce_jobs::handle::CancellationToken;
    use lettuce_model_hub::{
        EmbeddingModelFamily, InstalledEmbeddingManifest, InstalledModelArtifact,
    };

    use std::str::FromStr;

    use tokenizers::Tokenizer;

    use super::{
        EmbeddingDimensions, EmbeddingRequest, EmbeddingVector, OnnxEmbeddingRuntime,
        OnnxRuntimeLink, effective_max_sequence_length, l2_normalize, model_input,
        prepare_tokenizer,
    };

    fn tokenizer_json(padding: &str) -> String {
        format!(
            r#"{{
            "version": "1.0",
            "truncation": {{"direction": "Right", "max_length": 128, "strategy": "LongestFirst", "stride": 0}},
            "padding": {padding},
            "added_tokens": [
                {{"id": 0, "content": "[PAD]", "single_word": false, "lstrip": false, "rstrip": false, "normalized": false, "special": true}},
                {{"id": 1, "content": "[CLS]", "single_word": false, "lstrip": false, "rstrip": false, "normalized": false, "special": true}},
                {{"id": 2, "content": "[SEP]", "single_word": false, "lstrip": false, "rstrip": false, "normalized": false, "special": true}},
                {{"id": 3, "content": "[UNK]", "single_word": false, "lstrip": false, "rstrip": false, "normalized": false, "special": true}}
            ],
            "normalizer": null,
            "pre_tokenizer": {{"type": "Whitespace"}},
            "post_processor": {{
                "type": "TemplateProcessing",
                "single": [{{"SpecialToken": {{"id": "[CLS]", "type_id": 0}}}}, {{"Sequence": {{"id": "A", "type_id": 0}}}}, {{"SpecialToken": {{"id": "[SEP]", "type_id": 0}}}}],
                "pair": [{{"SpecialToken": {{"id": "[CLS]", "type_id": 0}}}}, {{"Sequence": {{"id": "A", "type_id": 0}}}}, {{"SpecialToken": {{"id": "[SEP]", "type_id": 0}}}}, {{"Sequence": {{"id": "B", "type_id": 1}}}}, {{"SpecialToken": {{"id": "[SEP]", "type_id": 1}}}}],
                "special_tokens": {{
                    "[CLS]": {{"id": "[CLS]", "ids": [1], "tokens": ["[CLS]"]}},
                    "[SEP]": {{"id": "[SEP]", "ids": [2], "tokens": ["[SEP]"]}}
                }}
            }},
            "decoder": null,
            "model": {{"type": "WordLevel", "vocab": {{"[PAD]": 0, "[CLS]": 1, "[SEP]": 2, "[UNK]": 3, "memory": 4}}, "unk_token": "[UNK]"}}
        }}"#
        )
    }

    const BATCH_PADDING: &str = r#"{"strategy": "BatchLongest", "direction": "Right", "pad_to_multiple_of": null, "pad_id": 0, "pad_type_id": 0, "pad_token": "[PAD]"}"#;

    #[test]
    fn long_text_reaches_the_model_past_the_published_tokenizer_truncation() {
        let text = vec!["memory"; 300].join(" ");
        let published = Tokenizer::from_str(&tokenizer_json(BATCH_PADDING)).expect("tokenizer");
        assert_eq!(
            published.encode(text.as_str(), true).expect("encode").len(),
            128
        );
        let mut tokenizer = published;
        prepare_tokenizer(&mut tokenizer).expect("prepare");
        let full = model_input(&tokenizer, &text, 4096).expect("input");
        assert_eq!(full.len(), 302);
        assert_eq!(full.input_ids.first(), Some(&1));
        assert_eq!(full.input_ids.last(), Some(&2));
        assert!(full.attention_mask.iter().all(|value| *value == 1));
        assert_eq!(full.token_type_ids.len(), 302);

        assert_eq!(super::count_tokens(&tokenizer, &text), Ok(300));
        let bounded = model_input(&tokenizer, &text, 256).expect("bounded input");
        assert_eq!(bounded.len(), 256);
        assert_eq!(bounded.input_ids.first(), Some(&1));
        assert_eq!(bounded.input_ids.last(), Some(&2));
        assert!(bounded.input_ids[1..255].iter().all(|id| *id == 4));
    }

    #[test]
    fn published_fixed_padding_is_dropped() {
        let fixed = r#"{"strategy": {"Fixed": 1024}, "direction": "Right", "pad_to_multiple_of": null, "pad_id": 0, "pad_type_id": 0, "pad_token": "[PAD]"}"#;
        let mut tokenizer = Tokenizer::from_str(&tokenizer_json(fixed)).expect("tokenizer");
        assert_eq!(
            tokenizer.encode("memory", true).expect("encode").len(),
            1024
        );
        prepare_tokenizer(&mut tokenizer).expect("prepare");
        assert_eq!(
            model_input(&tokenizer, "memory", 4096)
                .expect("input")
                .input_ids,
            vec![1, 4, 2]
        );
    }

    #[test]
    fn token_budget_follows_the_setting_within_the_model_limit() {
        assert_eq!(effective_max_sequence_length(4096, None), 4096);
        assert_eq!(effective_max_sequence_length(2048, None), 2048);
        assert_eq!(effective_max_sequence_length(4096, Some(100)), 512);
        assert_eq!(effective_max_sequence_length(4096, Some(1000)), 1000);
        assert_eq!(effective_max_sequence_length(4096, Some(9000)), 4096);
        assert_eq!(effective_max_sequence_length(2048, Some(3000)), 2048);
    }

    #[test]
    fn preference_follows_the_legacy_v4_dimension_rule() {
        for (preferred, expected) in [
            (None, EmbeddingDimensions::D768),
            (Some(64), EmbeddingDimensions::D64),
            (Some(128), EmbeddingDimensions::D128),
            (Some(256), EmbeddingDimensions::D256),
            (Some(512), EmbeddingDimensions::D512),
            (Some(768), EmbeddingDimensions::D768),
            (Some(300), EmbeddingDimensions::D768),
        ] {
            assert_eq!(EmbeddingDimensions::from_preference(preferred), expected);
        }
    }

    #[test]
    fn matryoshka_slice_is_normalized() {
        let mut values = vec![3.0, 4.0];
        assert!(l2_normalize(&mut values).is_ok());
        assert!((values[0] - 0.6).abs() < f32::EPSILON);
        assert!((values[1] - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn cosine_requires_matching_embedding_identity() {
        let first = EmbeddingVector {
            source_revision: "one".to_owned(),
            values: vec![1.0, 0.0],
        };
        let same = first.clone();
        let different = EmbeddingVector {
            source_revision: "two".to_owned(),
            values: vec![1.0, 0.0],
        };
        assert_eq!(first.cosine_similarity(&same), Some(1.0));
        assert_eq!(first.cosine_similarity(&different), None);
    }

    #[test]
    #[ignore = "requires audited local v4 model and ONNX Runtime paths"]
    fn live_v4_model_produces_native_and_matryoshka_embeddings() {
        let model = std::env::var_os("LETTUCE_TEST_EMBEDDING_MODEL")
            .map(std::path::PathBuf::from)
            .expect("model path");
        let tokenizer = std::env::var_os("LETTUCE_TEST_EMBEDDING_TOKENIZER")
            .map(std::path::PathBuf::from)
            .expect("tokenizer path");
        let runtime = std::env::var_os("LETTUCE_TEST_ONNX_RUNTIME")
            .map(std::path::PathBuf::from)
            .expect("runtime path");
        let manifest = InstalledEmbeddingManifest {
            family: EmbeddingModelFamily::LettuceEmbV4,
            source_revision: "8fe12dc548f75865bfb120593fd5a514e9186ca0".to_owned(),
            model: InstalledModelArtifact::inspect(model).expect("model artifact"),
            tokenizer: InstalledModelArtifact::inspect(tokenizer).expect("tokenizer artifact"),
            calibration: None,
            max_sequence_length: 2048,
            native_dimensions: 768,
        };
        let artifacts = manifest.verify().expect("verified artifacts");
        let mut runtime =
            OnnxEmbeddingRuntime::load(artifacts, &OnnxRuntimeLink::Dynamic(runtime), None)
                .expect("runtime");
        let cancellation = CancellationToken::new();
        let native = runtime
            .embed(
                &EmbeddingRequest {
                    text: "Mira prefers tea by the harbor.".to_owned(),
                    dimensions: EmbeddingDimensions::D768,
                },
                &cancellation,
            )
            .expect("native embedding");
        let compact = runtime
            .embed(
                &EmbeddingRequest {
                    text: "Mira prefers tea by the harbor.".to_owned(),
                    dimensions: EmbeddingDimensions::D128,
                },
                &cancellation,
            )
            .expect("compact embedding");
        assert_eq!(native.values.len(), 768);
        assert_eq!(compact.values.len(), 128);
        let norm = compact
            .values
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            .sqrt();
        assert!((norm - 1.0).abs() < 0.0001);
    }

    fn embed(
        runtime: &mut OnnxEmbeddingRuntime,
        text: &str,
        dimensions: EmbeddingDimensions,
    ) -> EmbeddingVector {
        runtime
            .embed(
                &EmbeddingRequest {
                    text: text.to_owned(),
                    dimensions,
                },
                &CancellationToken::new(),
            )
            .expect("embedding")
    }

    #[test]
    #[ignore = "requires a downloaded Eidos install and ONNX Runtime paths"]
    fn live_eidos_model_embeds_long_text_with_calibrated_scores() {
        let root = std::env::var_os("LETTUCE_TEST_EIDOS_DIR")
            .map(std::path::PathBuf::from)
            .expect("Eidos directory");
        let runtime = std::env::var_os("LETTUCE_TEST_ONNX_RUNTIME")
            .map(std::path::PathBuf::from)
            .expect("runtime path");
        let manifest = InstalledEmbeddingManifest {
            family: EmbeddingModelFamily::LettuceEidosV5,
            source_revision: "f14e5de6ab468df6651b6505f59a7224e630ee79".to_owned(),
            model: InstalledModelArtifact::inspect(root.join("onnx/model_quantized.onnx"))
                .expect("model artifact"),
            tokenizer: InstalledModelArtifact::inspect(root.join("tokenizer.json"))
                .expect("tokenizer artifact"),
            calibration: Some(
                InstalledModelArtifact::inspect(root.join("calibration.json"))
                    .expect("calibration artifact"),
            ),
            max_sequence_length: 4096,
            native_dimensions: 768,
        };
        let artifacts = manifest.verify().expect("verified artifacts");
        let mut runtime =
            OnnxEmbeddingRuntime::load(artifacts, &OnnxRuntimeLink::Dynamic(runtime), None)
                .expect("runtime");
        assert_eq!(runtime.max_sequence_length(), 4096);
        let query =
            "Aria: The mill looks quiet tonight.\nUser: Do you remember where you hid the sword?";
        let related =
            "Elara hid the sword in the old stone well behind the mill, under a loose slab.";
        let unrelated = "Apple prices at the market went up again this week.";
        for dimensions in [EmbeddingDimensions::D768, EmbeddingDimensions::D256] {
            let query = embed(&mut runtime, query, dimensions);
            let related = embed(&mut runtime, related, dimensions);
            let unrelated = embed(&mut runtime, unrelated, dimensions);
            assert_eq!(query.source_revision, "v5");
            assert_eq!(query.values.len(), dimensions.get());
            let norm = query
                .values
                .iter()
                .map(|value| value * value)
                .sum::<f32>()
                .sqrt();
            assert!((norm - 1.0).abs() < 0.001);
            let calibration = *runtime.calibration();
            let related = calibration.score(
                query.cosine_similarity(&related).expect("cosine"),
                dimensions,
            );
            let unrelated = calibration.score(
                query.cosine_similarity(&unrelated).expect("cosine"),
                dimensions,
            );
            assert!(related > unrelated);
            assert!(related >= 0.5, "related score {related}");
            assert!(unrelated < 0.5, "unrelated score {unrelated}");
        }
        let long = (0..400)
            .map(|index| format!("detail{index}"))
            .collect::<Vec<_>>()
            .join(" ");
        let prefix = long.split(' ').take(40).collect::<Vec<_>>().join(" ");
        assert!(runtime.count_tokens(&long).expect("count") > 1000);
        let long = embed(&mut runtime, &long, EmbeddingDimensions::D768);
        let prefix = embed(&mut runtime, &prefix, EmbeddingDimensions::D768);
        assert!(long.cosine_similarity(&prefix).expect("cosine") < 0.999);
    }
}
