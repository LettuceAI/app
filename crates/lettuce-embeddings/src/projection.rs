use lettuce_types::{MemoryId, MemorySpaceId, TimestampMillis};

use crate::{EmbeddingDimensions, EmbeddingVector};

const MAX_SOURCE_REVISION_BYTES: usize = 128;
const MAX_SOURCE_TEXT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryEmbeddingProjection {
    pub space_id: MemorySpaceId,
    pub memory_id: MemoryId,
    pub source_text: String,
    pub vector: EmbeddingVector,
    pub dimensions: EmbeddingDimensions,
    pub updated_at: TimestampMillis,
}

impl MemoryEmbeddingProjection {
    pub fn validate(&self) -> Result<(), EmbeddingProjectionError> {
        validate_identity(&self.source_text, &self.vector.source_revision)?;
        if self.vector.values.len() != self.dimensions.get()
            || self.vector.values.iter().any(|value| !value.is_finite())
        {
            return Err(EmbeddingProjectionError::InvalidVector);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryEmbeddingRepair {
    pub space_id: MemorySpaceId,
    pub memory_id: MemoryId,
    pub source_text: String,
    pub source_revision: String,
    pub dimensions: EmbeddingDimensions,
    pub updated_at: TimestampMillis,
}

impl MemoryEmbeddingRepair {
    pub fn validate(&self) -> Result<(), EmbeddingProjectionError> {
        validate_identity(&self.source_text, &self.source_revision)
    }
}

fn validate_identity(
    source_text: &str,
    source_revision: &str,
) -> Result<(), EmbeddingProjectionError> {
    if source_text.trim().is_empty() || source_text.len() > MAX_SOURCE_TEXT_BYTES {
        return Err(EmbeddingProjectionError::InvalidSourceText);
    }
    if source_revision.trim().is_empty() || source_revision.len() > MAX_SOURCE_REVISION_BYTES {
        return Err(EmbeddingProjectionError::InvalidSourceRevision);
    }
    Ok(())
}

pub trait MemoryEmbeddingRepository: Send + Sync {
    fn list_ready(
        &self,
        space_id: MemorySpaceId,
        source_revision: &str,
        dimensions: EmbeddingDimensions,
    ) -> Result<Vec<MemoryEmbeddingProjection>, EmbeddingProjectionError>;

    /// Live memories whose vector is missing or repair needed.
    fn list_repairs(
        &self,
        space_id: MemorySpaceId,
        source_revision: &str,
        dimensions: EmbeddingDimensions,
    ) -> Result<Vec<MemoryEmbeddingRepair>, EmbeddingProjectionError>;

    /// Stores a vector only while its memory still has exactly the embedded
    /// text; otherwise nothing is written and the write is `Superseded`.
    fn put_ready(
        &self,
        projection: MemoryEmbeddingProjection,
    ) -> Result<ProjectionWrite, EmbeddingProjectionError>;

    /// `put_ready` for a re-embedded memory that also stores the memory's
    /// recounted tokens under the same text check.
    fn put_reembedded(
        &self,
        projection: MemoryEmbeddingProjection,
        token_count: u32,
    ) -> Result<ProjectionWrite, EmbeddingProjectionError>;

    fn mark_repair_needed(
        &self,
        repair: MemoryEmbeddingRepair,
    ) -> Result<(), EmbeddingProjectionError>;
}

/// Whether a vector was stored, or its memory's text changed after it was
/// embedded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionWrite {
    Stored,
    Superseded,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EmbeddingProjectionError {
    #[error("embedding projection source text is invalid")]
    InvalidSourceText,
    #[error("embedding projection source revision is invalid")]
    InvalidSourceRevision,
    #[error("embedding projection vector is invalid")]
    InvalidVector,
    #[error("embedding projection repository failed: {0}")]
    Repository(String),
}

#[cfg(test)]
mod tests {
    use lettuce_types::{MemoryId, MemorySpaceId, TimestampMillis};

    use super::{EmbeddingProjectionError, MemoryEmbeddingProjection};
    use crate::{EmbeddingDimensions, EmbeddingVector};

    #[test]
    fn projection_requires_the_declared_finite_dimensions() {
        let projection = MemoryEmbeddingProjection {
            space_id: MemorySpaceId::new(),
            memory_id: MemoryId::new(),
            source_text: "memory".to_owned(),
            vector: EmbeddingVector {
                source_revision: "v4".to_owned(),
                values: vec![0.0; 64],
            },
            dimensions: EmbeddingDimensions::D128,
            updated_at: TimestampMillis::new(1),
        };
        assert_eq!(
            projection.validate(),
            Err(EmbeddingProjectionError::InvalidVector)
        );
    }
}
