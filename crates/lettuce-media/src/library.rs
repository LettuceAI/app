use serde::{Deserialize, Serialize};

use crate::{MediaAsset, MediaBlob, MediaKind, ReleasedMediaObject};
use lettuce_types::{AssetId, Page, PageRequest, RequestId, TimestampMillis};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaReferenceKind {
    Character,
    Persona,
    Group,
    Scene,
    Conversation,
    Creation,
    Speech,
    Image,
    Memory,
    Companion,
    Model,
    Settings,
    Lorebook,
    Prompt,
    Job,
    LegacyImport,
    Sync,
    Usage,
    Transfer,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaReference {
    pub kind: MediaReferenceKind,
    pub owner_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaLibraryEntry {
    pub asset: MediaAsset,
    pub blob: MediaBlob,
    pub references: Vec<MediaReference>,
}

pub trait MediaLibraryRepository: Send + Sync {
    fn library_page(
        &self,
        role: Option<MediaKind>,
        request: PageRequest,
    ) -> Result<Page<MediaLibraryEntry>, MediaLibraryError>;
    fn retaining_references(
        &self,
        asset: AssetId,
    ) -> Result<Vec<MediaReference>, MediaLibraryError>;
    fn remove_library_asset(
        &self,
        asset: AssetId,
        key: RequestId,
        digest: &str,
        now: TimestampMillis,
    ) -> Result<Vec<ReleasedMediaObject>, MediaLibraryError>;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MediaLibraryError {
    #[error("media asset was not found")]
    NotFound,
    #[error("media asset is referenced")]
    InUse(Vec<MediaReference>),
    #[error("media operation identity conflicts with a previous request")]
    Conflict,
    #[error("media library cursor is invalid")]
    InvalidCursor,
    #[error("media reference data are invalid")]
    InvalidData,
    #[error("media library storage failed")]
    Storage,
}
