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
use tokenizers::{Tokenizer, utils::truncation::TruncationParams};

const MAX_INPUT_BYTES: usize = 1024 * 1024;

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
}

impl OnnxEmbeddingRuntime {
    pub fn load(
        artifacts: VerifiedEmbeddingArtifacts,
        runtime: &OnnxRuntimeLink,
    ) -> Result<Self, EmbeddingError> {
        if artifacts.family != EmbeddingModelFamily::LettuceEmbV4
            || artifacts.native_dimensions != 768
        {
            return Err(EmbeddingError::UnsupportedModel);
        }
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
        tokenizer
            .with_truncation(Some(TruncationParams {
                max_length: artifacts.max_sequence_length,
                ..TruncationParams::default()
            }))
            .map_err(|_| EmbeddingError::TokenizerLoad)?;
        let expects_token_type_ids = session
            .inputs
            .iter()
            .any(|input| input.name.contains("token_type_ids"));
        // Force metadata access during load so malformed sessions fail before a job starts.
        if session.outputs.is_empty() {
            return Err(EmbeddingError::InvalidOutput);
        }
        Ok(Self {
            artifacts,
            session,
            tokenizer,
            expects_token_type_ids,
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
        let encoding = self
            .tokenizer
            .encode(request.text.as_str(), true)
            .map_err(|_| EmbeddingError::Tokenization)?;
        let sequence_length = encoding.len().min(self.artifacts.max_sequence_length);
        if sequence_length == 0 {
            return Err(EmbeddingError::Tokenization);
        }
        let input_ids = encoding.get_ids()[..sequence_length]
            .iter()
            .map(|value| i64::from(*value))
            .collect::<Vec<_>>();
        let attention_mask = encoding.get_attention_mask()[..sequence_length]
            .iter()
            .map(|value| i64::from(*value))
            .collect::<Vec<_>>();
        let type_ids = encoding.get_type_ids();
        let token_type_ids = if type_ids.len() >= sequence_length {
            type_ids[..sequence_length]
                .iter()
                .map(|value| i64::from(*value))
                .collect::<Vec<_>>()
        } else {
            vec![0; sequence_length]
        };
        if cancellation.is_cancelled() {
            return Err(EmbeddingError::Cancelled);
        }

        let input_ids = Value::from_array(([1, sequence_length], input_ids))
            .map_err(|_| EmbeddingError::InvalidInputTensor)?;
        let attention_mask = Value::from_array(([1, sequence_length], attention_mask))
            .map_err(|_| EmbeddingError::InvalidInputTensor)?;
        let token_type_ids = Value::from_array(([1, sequence_length], token_type_ids))
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
        let output = outputs
            .values()
            .next()
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
            source_revision: self.artifacts.source_revision.clone(),
            values,
        })
    }
}

fn spawn_canceller(
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

fn initialize_onnx_runtime(runtime: &OnnxRuntimeLink) -> Result<(), EmbeddingError> {
    let result = match runtime {
        OnnxRuntimeLink::Dynamic(path) => {
            let path = path.to_str().ok_or(EmbeddingError::RuntimeUnavailable)?;
            ort::init_from(path)
                .with_name("lettuce-embeddings")
                .commit()
        }
        OnnxRuntimeLink::Linked => ort::init().with_name("lettuce-embeddings").commit(),
    };
    result
        .map(|_| ())
        .map_err(|_| EmbeddingError::RuntimeUnavailable)
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
fn configure_execution_provider(
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
fn configure_execution_provider(
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
}

#[cfg(test)]
mod tests {
    use lettuce_jobs::handle::CancellationToken;
    use lettuce_model_hub::{
        EmbeddingModelFamily, InstalledEmbeddingManifest, InstalledModelArtifact,
    };

    use super::{
        EmbeddingDimensions, EmbeddingRequest, EmbeddingVector, OnnxEmbeddingRuntime,
        OnnxRuntimeLink, l2_normalize,
    };

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
            max_sequence_length: 2048,
            native_dimensions: 768,
        };
        let artifacts = manifest.verify().expect("verified artifacts");
        let mut runtime = OnnxEmbeddingRuntime::load(artifacts, &OnnxRuntimeLink::Dynamic(runtime))
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
}
