//! Physical content identity metadata.

use lettuce_types::{ContentHash, MediaBlobId, TimestampMillis};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Image,
    Audio,
    Video,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlobState {
    Staged,
    Ready,
    Quarantined,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaBlob {
    pub id: MediaBlobId,
    pub content_hash: ContentHash,
    pub kind: MediaKind,
    pub mime_type: String,
    pub byte_size: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_ms: Option<u64>,
    pub validation_version: u32,
    pub state: BlobState,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MediaBlobRepositoryError {
    #[error("media blob was not found")]
    NotFound,
    #[error("stored media metadata are invalid")]
    InvalidData,
    #[error("media storage failed")]
    Storage,
}

pub trait MediaBlobRepository: Send + Sync {
    /// Inserts metadata or returns the existing blob with the same content hash.
    fn register(&self, blob: MediaBlob) -> Result<MediaBlob, MediaBlobRepositoryError>;
    fn get(&self, id: MediaBlobId) -> Result<Option<MediaBlob>, MediaBlobRepositoryError>;
    fn find_by_hash(
        &self,
        hash: &ContentHash,
    ) -> Result<Option<MediaBlob>, MediaBlobRepositoryError>;
}
