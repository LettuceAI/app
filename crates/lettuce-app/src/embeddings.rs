use std::sync::Mutex;

use lettuce_embeddings::{
    EmbeddingDimensions, EmbeddingError, EmbeddingRequest, EmbeddingVector, EmotionClassifierError,
    OnnxEmbeddingRuntime, OnnxEmotionClassifier, OnnxRuntimeLink, SimilarityCalibration,
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
    dimensions: EmbeddingDimensions,
    calibration: SimilarityCalibration,
    runtime: Mutex<OnnxEmbeddingRuntime>,
}

impl EmbeddingService {
    /// Loads a verified install. `max_tokens` is the device's
    /// `embeddingMaxTokens`; vectors carry the family's vector-space label.
    pub fn load(
        manifest: &InstalledEmbeddingManifest,
        runtime_link: &OnnxRuntimeLink,
        dimensions: EmbeddingDimensions,
        max_tokens: Option<u16>,
    ) -> Result<Self, EmbeddingServiceError> {
        let artifacts = manifest.verify()?;
        let runtime = OnnxEmbeddingRuntime::load(artifacts, runtime_link, max_tokens)?;
        Ok(Self {
            source_revision: runtime.vector_space().to_owned(),
            dimensions,
            calibration: *runtime.calibration(),
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

    /// The closest existing memory whose shown similarity exceeds
    /// `threshold`; the shown similarity is the model's calibrated score.
    pub fn semantic_duplicate_evidence(
        candidate: &EmbeddingVector,
        existing: &[(MemoryId, EmbeddingVector)],
        threshold: lettuce_memory::Score,
        calibration: &SimilarityCalibration,
    ) -> Option<lettuce_memory::SemanticDuplicateEvidence> {
        let dimensions = EmbeddingDimensions::from_len(candidate.values.len())?;
        existing
            .iter()
            .filter_map(|(id, embedding)| {
                candidate
                    .cosine_similarity(embedding)
                    .map(|similarity| (*id, calibration.score(similarity, dimensions)))
            })
            .filter(|(_, similarity)| f64::from(*similarity) > threshold.ratio())
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

    fn dimensions(&self) -> EmbeddingDimensions;

    /// How this model's raw cosine becomes the score thresholds apply to.
    fn calibration(&self) -> SimilarityCalibration {
        SimilarityCalibration::RawCosine
    }

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

    fn dimensions(&self) -> EmbeddingDimensions {
        self.dimensions
    }

    fn calibration(&self) -> SimilarityCalibration {
        self.calibration
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
    use lettuce_embeddings::{EmbeddingVector, SimilarityCalibration};
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
            &SimilarityCalibration::RawCosine,
        );
        assert!(evidence.is_some_and(|evidence| {
            evidence.existing_id == strongest
                && evidence.source_revision == "v4"
                && evidence.cosine_score == Score::FULL
        }));
    }

    #[test]
    fn a_similarity_equal_to_the_duplicate_threshold_is_not_a_duplicate() {
        let mut values = vec![0.0; 64];
        values[0] = 1.0;
        let candidate = EmbeddingVector {
            source_revision: "v4".to_owned(),
            values,
        };
        let existing = [(MemoryId::new(), candidate.clone())];
        assert!(
            EmbeddingService::semantic_duplicate_evidence(
                &candidate,
                &existing,
                Score::FULL,
                &SimilarityCalibration::RawCosine,
            )
            .is_none()
        );
        assert!(
            EmbeddingService::semantic_duplicate_evidence(
                &candidate,
                &existing,
                score(9_999),
                &SimilarityCalibration::RawCosine,
            )
            .is_some()
        );
    }

    fn unit(dimensions: usize, cosine: f32) -> Vec<f32> {
        let mut values = vec![0.0; dimensions];
        values[0] = cosine;
        values[1] = (1.0 - cosine * cosine).sqrt();
        values
    }

    #[test]
    fn eidos_duplicates_compare_the_calibrated_score_with_the_threshold() {
        let calibration = SimilarityCalibration::from_json(
            br#"{"default_threshold": 0.5, "fallback_threshold": 0.35, "dims": {
                "768": {"a": 2.381, "b": -1.381}, "512": {"a": 2.2901, "b": -1.2863},
                "256": {"a": 2.2388, "b": -1.244}, "128": {"a": 2.2388, "b": -1.2664},
                "64": {"a": 1.9481, "b": -1.0058}}}"#,
        )
        .expect("calibration");
        let near = MemoryId::new();
        for (dimensions, raw, shown) in [
            (768, 0.92, 2.381 * 0.92 - 1.381),
            (256, 0.92, 2.2388 * 0.92 - 1.244),
        ] {
            let candidate = EmbeddingVector {
                source_revision: "v5".to_owned(),
                values: unit(dimensions, 1.0),
            };
            let existing = [(
                near,
                EmbeddingVector {
                    source_revision: "v5".to_owned(),
                    values: unit(dimensions, raw),
                },
            )];
            let evidence = EmbeddingService::semantic_duplicate_evidence(
                &candidate,
                &existing,
                score(7_800),
                &calibration,
            )
            .expect("calibrated duplicate");
            assert!((evidence.cosine_score.ratio() - shown).abs() < 0.001);
            assert!(evidence.cosine_score >= evidence.threshold);
            assert!(
                EmbeddingService::semantic_duplicate_evidence(
                    &candidate,
                    &existing,
                    score(9_000),
                    &calibration,
                )
                .is_none()
            );
        }
        let candidate = EmbeddingVector {
            source_revision: "v5".to_owned(),
            values: unit(768, 1.0),
        };
        let unrelated = [(
            near,
            EmbeddingVector {
                source_revision: "v5".to_owned(),
                values: unit(768, 0.8),
            },
        )];
        assert!(
            EmbeddingService::semantic_duplicate_evidence(
                &candidate,
                &unrelated,
                score(7_800),
                &SimilarityCalibration::RawCosine,
            )
            .is_some()
        );
        assert!(
            EmbeddingService::semantic_duplicate_evidence(
                &candidate,
                &unrelated,
                score(7_800),
                &calibration,
            )
            .is_none()
        );
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
