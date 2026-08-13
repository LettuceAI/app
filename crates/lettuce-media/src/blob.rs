//! Physical content identity metadata.

use lettuce_types::{ContentHash, MediaBlobId, TimestampMillis};
use serde::{Deserialize, Serialize};

const MAX_MIME_TYPE_SCALARS: usize = 256;

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

impl MediaBlob {
    /// Validates the metadata that can be represented by the M1 SQLite row.
    ///
    /// Dimensions and duration are decoder metadata, so they may be absent
    /// for legacy or partially validated rows. If one dimension is present,
    /// the other must be present too; all numeric values must fit the M1
    /// SQLite representation.
    pub fn validate(&self) -> Result<(), MediaBlobValidationError> {
        if self.mime_type.trim().is_empty()
            || self.mime_type.trim() != self.mime_type
            || self.mime_type.chars().count() > MAX_MIME_TYPE_SCALARS
            || self.mime_type.chars().any(char::is_control)
            || !self.mime_type.is_ascii()
        {
            return Err(MediaBlobValidationError::InvalidMimeType);
        }
        if i64::try_from(self.byte_size).is_err() {
            return Err(MediaBlobValidationError::OutOfRange("byte_size"));
        }
        for (field, value) in [("width", self.width), ("height", self.height)] {
            if value == Some(0) {
                return Err(MediaBlobValidationError::NonPositiveDimension { field });
            }
        }
        if self
            .duration_ms
            .is_some_and(|duration| i64::try_from(duration).is_err())
        {
            return Err(MediaBlobValidationError::OutOfRange("duration_ms"));
        }
        if self.validation_version == 0 {
            return Err(MediaBlobValidationError::ZeroValidationVersion);
        }
        if self.width.is_some() != self.height.is_some() {
            return Err(MediaBlobValidationError::IncoherentMetadata { kind: self.kind });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MediaBlobValidationError {
    #[error("media blob MIME type is blank, unsafe, or too long")]
    InvalidMimeType,
    #[error("media blob {0} is outside SQLite's signed 64-bit range")]
    OutOfRange(&'static str),
    #[error("media blob {field} must be positive")]
    NonPositiveDimension { field: &'static str },
    #[error("media blob validation version must be at least one")]
    ZeroValidationVersion,
    #[error("media blob metadata are incoherent for {kind:?}")]
    IncoherentMetadata { kind: MediaKind },
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

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_types::{ContentHash, MediaBlobId, TimestampMillis};

    fn blob(kind: MediaKind) -> MediaBlob {
        MediaBlob {
            id: MediaBlobId::new(),
            content_hash: ContentHash::parse("ab".repeat(32)).expect("hash"),
            kind,
            mime_type: "application/octet-stream".into(),
            byte_size: 1,
            width: None,
            height: None,
            duration_ms: None,
            validation_version: 1,
            state: BlobState::Staged,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        }
    }

    #[test]
    fn legacy_optional_media_metadata_remains_valid() {
        for kind in [MediaKind::Image, MediaKind::Audio, MediaKind::Video] {
            assert!(blob(kind).validate().is_ok());
        }
        let mut with_duration = blob(MediaKind::Image);
        with_duration.duration_ms = Some(42);
        assert!(with_duration.validate().is_ok());
    }

    #[test]
    fn dimensions_must_be_positive_and_paired() {
        let mut zero = blob(MediaKind::Image);
        zero.width = Some(0);
        zero.height = Some(1);
        assert!(matches!(
            zero.validate(),
            Err(MediaBlobValidationError::NonPositiveDimension { field: "width" })
        ));

        let mut partial = blob(MediaKind::Image);
        partial.width = Some(1);
        assert!(matches!(
            partial.validate(),
            Err(MediaBlobValidationError::IncoherentMetadata { .. })
        ));
    }

    #[test]
    fn mime_version_and_sqlite_ranges_are_checked() {
        let mut invalid = blob(MediaKind::Image);
        invalid.mime_type = "   ".into();
        assert_eq!(
            invalid.validate(),
            Err(MediaBlobValidationError::InvalidMimeType)
        );

        let mut parameterized = blob(MediaKind::Audio);
        parameterized.mime_type = "audio/webm; codecs=opus".into();
        assert!(parameterized.validate().is_ok());
        parameterized.mime_type = " audio/webm".into();
        assert_eq!(
            parameterized.validate(),
            Err(MediaBlobValidationError::InvalidMimeType)
        );
        parameterized.mime_type = "audio/webm ".into();
        assert_eq!(
            parameterized.validate(),
            Err(MediaBlobValidationError::InvalidMimeType)
        );

        invalid = blob(MediaKind::Image);
        invalid.mime_type = "image/\u{7f}png".into();
        assert_eq!(
            invalid.validate(),
            Err(MediaBlobValidationError::InvalidMimeType)
        );

        invalid = blob(MediaKind::Image);
        invalid.mime_type = "x".repeat(MAX_MIME_TYPE_SCALARS + 1);
        assert_eq!(
            invalid.validate(),
            Err(MediaBlobValidationError::InvalidMimeType)
        );

        invalid = blob(MediaKind::Image);
        invalid.validation_version = 0;
        assert_eq!(
            invalid.validate(),
            Err(MediaBlobValidationError::ZeroValidationVersion)
        );

        invalid = blob(MediaKind::Image);
        invalid.byte_size = u64::MAX;
        assert_eq!(
            invalid.validate(),
            Err(MediaBlobValidationError::OutOfRange("byte_size"))
        );

        invalid = blob(MediaKind::Image);
        invalid.duration_ms = Some(u64::MAX);
        assert_eq!(
            invalid.validate(),
            Err(MediaBlobValidationError::OutOfRange("duration_ms"))
        );
    }
}
