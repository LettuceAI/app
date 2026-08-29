use std::sync::Mutex;

use lettuce_embeddings::{
    EmbeddingError, EmbeddingRequest, EmbeddingVector, OnnxEmbeddingRuntime, OnnxRuntimeLink,
};
use lettuce_jobs::handle::CancellationToken;
use lettuce_model_hub::{InstalledEmbeddingManifest, ModelArtifactError};
use lettuce_types::MemoryId;

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
