use std::sync::Mutex;

use lettuce_embeddings::{
    EmbeddingError, EmbeddingRequest, EmbeddingVector, EmotionClassifierError,
    OnnxEmbeddingRuntime, OnnxEmotionClassifier, OnnxRuntimeLink,
};
use lettuce_jobs::handle::CancellationToken;
use lettuce_model_hub::{
    InstalledCompanionEmotionManifest, InstalledEmbeddingManifest, ModelArtifactError,
};
use lettuce_types::MemoryId;

use lettuce_companions::EmotionClassification;

#[derive(Debug)]
pub struct EmbeddingService {
    source_revision: String,
    runtime: Mutex<OnnxEmbeddingRuntime>,
}

impl EmbeddingService {
    pub fn load(
        manifest: &InstalledEmbeddingManifest,
        runtime_link: &OnnxRuntimeLink,
    ) -> Result<Self, EmbeddingServiceError> {
        let artifacts = manifest.verify()?;
        let source_revision = artifacts.source_revision.clone();
        let runtime = OnnxEmbeddingRuntime::load(artifacts, runtime_link)?;
        Ok(Self {
            source_revision,
            runtime: Mutex::new(runtime),
        })
    }

    #[must_use]
    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }

    pub fn embed(
        &self,
        request: &EmbeddingRequest,
        cancellation: &CancellationToken,
    ) -> Result<EmbeddingVector, EmbeddingServiceError> {
        self.runtime
            .lock()
            .map_err(|_| EmbeddingServiceError::RuntimeLock)?
            .embed(request, cancellation)
            .map_err(Into::into)
    }

    pub fn count_tokens(&self, text: &str) -> Result<u32, EmbeddingServiceError> {
        self.runtime
            .lock()
            .map_err(|_| EmbeddingServiceError::RuntimeLock)?
            .count_tokens(text)
            .map_err(Into::into)
    }

    pub fn semantic_duplicate_evidence(
        candidate: &EmbeddingVector,
        existing: &[(MemoryId, EmbeddingVector)],
        threshold: lettuce_memory::Score,
    ) -> Option<lettuce_memory::SemanticDuplicateEvidence> {
        existing
            .iter()
            .filter_map(|(id, embedding)| {
                candidate
                    .cosine_similarity(embedding)
                    .map(|similarity| (*id, similarity))
            })
            .filter(|(_, similarity)| f64::from(*similarity) >= threshold.ratio())
            .max_by(|(_, left), (_, right)| left.total_cmp(right))
            .and_then(|(existing_id, similarity)| {
                let cosine_score =
                    lettuce_memory::Score::from_ratio(f64::from(similarity.clamp(0.0, 1.0)))
                        .ok()?;
                let dimensions = u16::try_from(candidate.values.len()).ok()?;
                Some(lettuce_memory::SemanticDuplicateEvidence {
                    existing_id,
                    source_revision: candidate.source_revision.clone(),
                    dimensions,
                    cosine_score,
                    threshold,
                })
            })
    }
}

pub trait MemoryEmbeddingEngine: Send + Sync {
    fn source_revision(&self) -> &str;

    fn count_tokens(&self, text: &str) -> Result<u32, EmbeddingGenerationError>;

    fn embed_memory(
        &self,
        request: &EmbeddingRequest,
        cancellation: &CancellationToken,
    ) -> Result<EmbeddingVector, EmbeddingGenerationError>;
}

impl MemoryEmbeddingEngine for EmbeddingService {
    fn source_revision(&self) -> &str {
        self.source_revision()
    }

    fn count_tokens(&self, text: &str) -> Result<u32, EmbeddingGenerationError> {
        self.count_tokens(text)
            .map_err(|_| EmbeddingGenerationError::Unavailable)
    }

    fn embed_memory(
        &self,
        request: &EmbeddingRequest,
        cancellation: &CancellationToken,
    ) -> Result<EmbeddingVector, EmbeddingGenerationError> {
        self.embed(request, cancellation).map_err(|error| {
            if matches!(
                error,
                EmbeddingServiceError::Runtime(EmbeddingError::Cancelled)
            ) {
                EmbeddingGenerationError::Cancelled
            } else {
                EmbeddingGenerationError::Unavailable
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EmbeddingGenerationError {
    #[error("embedding generation was cancelled")]
    Cancelled,
    #[error("embedding generation is unavailable")]
    Unavailable,
}

#[derive(Debug)]
pub struct CompanionEmotionService {
    runtime: Mutex<OnnxEmotionClassifier>,
}

impl CompanionEmotionService {
    pub fn load(
        manifest: &InstalledCompanionEmotionManifest,
        runtime_link: &OnnxRuntimeLink,
    ) -> Result<Self, CompanionEmotionServiceError> {
        let artifacts = manifest.verify()?;
        let runtime = OnnxEmotionClassifier::load(artifacts, runtime_link)?;
        Ok(Self {
            runtime: Mutex::new(runtime),
        })
    }
}

pub trait CompanionEmotionEngine: Send + Sync {
    fn classify_emotion(
        &self,
        text: &str,
        cancellation: &CancellationToken,
    ) -> Result<Option<EmotionClassification>, CompanionEmotionGenerationError>;
}

impl CompanionEmotionEngine for CompanionEmotionService {
    fn classify_emotion(
        &self,
        text: &str,
        cancellation: &CancellationToken,
    ) -> Result<Option<EmotionClassification>, CompanionEmotionGenerationError> {
        self.runtime
            .lock()
            .map_err(|_| CompanionEmotionGenerationError::Unavailable)?
            .classify(text, cancellation)
            .map_err(|error| {
                if matches!(error, EmotionClassifierError::Cancelled) {
                    CompanionEmotionGenerationError::Cancelled
                } else {
                    CompanionEmotionGenerationError::Unavailable
                }
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CompanionEmotionGenerationError {
    #[error("companion emotion classification was cancelled")]
    Cancelled,
    #[error("companion emotion classification is unavailable")]
    Unavailable,
}

#[derive(Debug, thiserror::Error)]
pub enum CompanionEmotionServiceError {
    #[error("companion emotion artifacts are unavailable: {0}")]
    Artifacts(#[from] ModelArtifactError),
    #[error("companion emotion runtime failed: {0}")]
    Runtime(#[from] EmotionClassifierError),
}

#[cfg(test)]
mod tests {
    use lettuce_embeddings::EmbeddingVector;
    use lettuce_memory::Score;
    use lettuce_types::MemoryId;

    use super::EmbeddingService;

    fn score(value: u16) -> Score {
        match Score::from_basis_points(value) {
            Some(score) => score,
            None => panic!("test score must be valid"),
        }
    }

    #[test]
    fn semantic_duplicate_evidence_uses_matching_identity_and_best_score() {
        let mut candidate_values = vec![0.0; 64];
        candidate_values[0] = 1.0;
        let candidate = EmbeddingVector {
            source_revision: "v4".to_owned(),
            values: candidate_values.clone(),
        };
        let wrong_identity = MemoryId::new();
        let weaker = MemoryId::new();
        let strongest = MemoryId::new();
        let evidence = EmbeddingService::semantic_duplicate_evidence(
            &candidate,
            &[
                (
                    wrong_identity,
                    EmbeddingVector {
                        source_revision: "v3".to_owned(),
                        values: candidate_values.clone(),
                    },
                ),
                (
                    weaker,
                    EmbeddingVector {
                        source_revision: "v4".to_owned(),
                        values: {
                            let mut values = vec![0.0; 64];
                            values[0] = 0.91;
                            values[1] = 0.41;
                            values
                        },
                    },
                ),
                (
                    strongest,
                    EmbeddingVector {
                        source_revision: "v4".to_owned(),
                        values: candidate_values,
                    },
                ),
            ],
            score(9_000),
        );
        assert!(evidence.is_some_and(|evidence| {
            evidence.existing_id == strongest
                && evidence.source_revision == "v4"
                && evidence.cosine_score == Score::FULL
        }));
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EmbeddingServiceError {
    #[error("embedding artifacts are unavailable: {0}")]
    Artifacts(#[from] ModelArtifactError),
    #[error("embedding runtime failed: {0}")]
    Runtime(#[from] EmbeddingError),
    #[error("embedding runtime lock failed")]
    RuntimeLock,
}
