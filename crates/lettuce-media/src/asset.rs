//! Logical media identities and their lifecycle invariants.

use lettuce_types::{
    AssetId, CharacterId, ConversationId, GroupId, JobId, LorebookEntryId, LorebookId, MediaBlobId,
    MessageId, ModelProfileId, PersonaId, PromptDocumentId, Revision, SceneAssetLinkId, SceneId,
    SceneVariantId, TimestampMillis,
};
use serde::{Deserialize, Serialize};

use crate::MediaKind;

/// The format version of [`AssetProvenanceV1`].
pub const ASSET_PROVENANCE_FORMAT_VERSION: u32 = 1;

const MAX_SOURCE_LABEL_SCALARS: usize = 256;
const MAX_SOURCE_URI_SCALARS: usize = 2_048;
const MAX_IMPORTED_FORMAT_SCALARS: usize = 128;

/// The user-facing role of a logical asset. This is deliberately narrower
/// than the physical blob kinds: one blob can back several logical assets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    AvatarOriginal,
    BackgroundImage,
    Illustration,
    LorebookIcon,
    MessageImage,
    MessageAudio,
    GeneratedImage,
    SynthesizedSpeech,
    OtherImage,
    OtherAudio,
}

impl AssetKind {
    /// Returns the physical kind required by this logical asset.
    #[must_use]
    pub const fn blob_kind(self) -> MediaKind {
        match self {
            Self::MessageAudio | Self::SynthesizedSpeech | Self::OtherAudio => MediaKind::Audio,
            Self::AvatarOriginal
            | Self::BackgroundImage
            | Self::Illustration
            | Self::LorebookIcon
            | Self::MessageImage
            | Self::GeneratedImage
            | Self::OtherImage => MediaKind::Image,
        }
    }
}

/// How a logical asset entered the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetOrigin {
    Upload,
    Import,
    RemoteFetch,
    Generated,
    Synthesized,
    Legacy,
}

/// Retention is a value rather than an optional expiry so the temporary
/// invariant cannot be represented as `Temporary { expires_at: None }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetentionClass {
    Persistent,
    Library,
    Temporary { expires_at: TimestampMillis },
}

impl RetentionClass {
    #[must_use]
    pub const fn expires_at(self) -> Option<TimestampMillis> {
        match self {
            Self::Temporary { expires_at } => Some(expires_at),
            Self::Persistent | Self::Library => None,
        }
    }

    #[must_use]
    pub const fn is_temporary(self) -> bool {
        matches!(self, Self::Temporary { .. })
    }
}

/// Logical lifecycle state. Physical deletion is intentionally not exposed
/// by the repository port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetState {
    Staged,
    Ready,
    Quarantined,
    Missing,
    Corrupt,
    TrashPending,
    Deleted,
}

/// Versioned, deliberately redacted provenance. It excludes bytes, native
/// paths, credentials, prompts and provider request bodies by construction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetProvenanceV1 {
    pub format_version: u32,
    pub source_label: Option<String>,
    pub source_uri_redacted: Option<String>,
    pub producing_job_id: Option<JobId>,
    pub model_profile_id: Option<ModelProfileId>,
    pub imported_format: Option<String>,
}

impl Default for AssetProvenanceV1 {
    fn default() -> Self {
        Self {
            format_version: ASSET_PROVENANCE_FORMAT_VERSION,
            source_label: None,
            source_uri_redacted: None,
            producing_job_id: None,
            model_profile_id: None,
            imported_format: None,
        }
    }
}

impl AssetProvenanceV1 {
    /// Validates the bounded strings and redacted URI shape.
    pub fn validate(&self) -> Result<(), AssetProvenanceError> {
        if self.format_version != ASSET_PROVENANCE_FORMAT_VERSION {
            return Err(AssetProvenanceError::UnsupportedFormatVersion(
                self.format_version,
            ));
        }
        if let Some(label) = &self.source_label {
            validate_bounded_text(label, "source label", MAX_SOURCE_LABEL_SCALARS)?;
        }
        if let Some(uri) = &self.source_uri_redacted {
            validate_redacted_uri(uri)?;
        }
        if let Some(format) = &self.imported_format {
            validate_imported_format(format)?;
        }
        Ok(())
    }
}

/// A catalog record with logical identity independent of blob identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaAsset {
    pub id: AssetId,
    pub blob_id: MediaBlobId,
    pub kind: AssetKind,
    pub origin: AssetOrigin,
    pub retention: RetentionClass,
    pub state: AssetState,
    pub provenance: AssetProvenanceV1,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

impl MediaAsset {
    /// Constructs and validates a logical asset record.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: AssetId,
        blob_id: MediaBlobId,
        kind: AssetKind,
        origin: AssetOrigin,
        retention: RetentionClass,
        state: AssetState,
        provenance: AssetProvenanceV1,
        revision: Revision,
        created_at: TimestampMillis,
        updated_at: TimestampMillis,
    ) -> Result<Self, MediaAssetValidationError> {
        let asset = Self {
            id,
            blob_id,
            kind,
            origin,
            retention,
            state,
            provenance,
            revision,
            created_at,
            updated_at,
        };
        asset.validate()?;
        Ok(asset)
    }

    /// Alias for callers that prefer an explicitly fallible constructor.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        id: AssetId,
        blob_id: MediaBlobId,
        kind: AssetKind,
        origin: AssetOrigin,
        retention: RetentionClass,
        state: AssetState,
        provenance: AssetProvenanceV1,
        revision: Revision,
        created_at: TimestampMillis,
        updated_at: TimestampMillis,
    ) -> Result<Self, MediaAssetValidationError> {
        Self::new(
            id, blob_id, kind, origin, retention, state, provenance, revision, created_at,
            updated_at,
        )
    }

    /// Validates representation invariants independent of a blob lookup.
    pub fn validate(&self) -> Result<(), MediaAssetValidationError> {
        if self.revision.get() == 0 {
            return Err(MediaAssetValidationError::ZeroRevision);
        }
        self.provenance
            .validate()
            .map_err(MediaAssetValidationError::InvalidProvenance)
    }

    /// Validates the cross-record logical/physical kind invariant.
    pub fn validate_for_blob_kind(
        &self,
        blob_kind: MediaKind,
    ) -> Result<(), MediaAssetValidationError> {
        self.validate()?;
        if self.kind.blob_kind() != blob_kind {
            return Err(MediaAssetValidationError::IncompatibleBlobKind {
                asset_kind: self.kind,
                blob_kind,
            });
        }
        Ok(())
    }

    /// Returns the next revision for an optimistic mutation.
    pub fn next_revision(&self) -> Result<Revision, MediaAssetValidationError> {
        self.revision
            .next()
            .map_err(|_| MediaAssetValidationError::RevisionOverflow)
    }

    /// Applies a retention change using an optimistic-concurrency token.
    pub fn update_retention(
        &mut self,
        expected_revision: Revision,
        retention: RetentionClass,
        updated_at: TimestampMillis,
    ) -> Result<(), MediaAssetMutationError> {
        self.validate()
            .map_err(MediaAssetMutationError::InvalidAsset)?;
        self.require_revision(expected_revision)?;
        let next_revision = self.next_revision()?;
        self.retention = retention;
        self.revision = next_revision;
        self.updated_at = updated_at;
        Ok(())
    }

    /// Marks an asset as missing or corrupt using an optimistic-concurrency
    /// token. Other lifecycle states require a separate named operation.
    pub fn mark_missing_or_corrupt(
        &mut self,
        expected_revision: Revision,
        state: AssetState,
        updated_at: TimestampMillis,
    ) -> Result<(), MediaAssetMutationError> {
        self.validate()
            .map_err(MediaAssetMutationError::InvalidAsset)?;
        self.require_revision(expected_revision)?;
        if !matches!(state, AssetState::Missing | AssetState::Corrupt) {
            return Err(MediaAssetMutationError::InvalidIntegrityState(state));
        }
        let next_revision = self.next_revision()?;
        self.state = state;
        self.revision = next_revision;
        self.updated_at = updated_at;
        Ok(())
    }

    fn require_revision(&self, expected_revision: Revision) -> Result<(), MediaAssetMutationError> {
        if self.revision != expected_revision {
            return Err(MediaAssetMutationError::StaleRevision {
                expected: expected_revision,
                actual: self.revision,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AssetProvenanceError {
    #[error("unsupported asset provenance format version {0}")]
    UnsupportedFormatVersion(u32),
    #[error("{field} is empty")]
    Empty { field: &'static str },
    #[error("{field} is too long")]
    TooLong { field: &'static str },
    #[error("{field} contains a control character")]
    ControlCharacter { field: &'static str },
    #[error("redacted source URI must be an HTTP(S) URI without credentials or a query")]
    UnsafeSourceUri,
    #[error("imported format contains an unsafe character")]
    UnsafeImportedFormat,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MediaAssetValidationError {
    #[error("revision must be at least one")]
    ZeroRevision,
    #[error("revision cannot be incremented")]
    RevisionOverflow,
    #[error("invalid provenance: {0}")]
    InvalidProvenance(AssetProvenanceError),
    #[error("asset kind {asset_kind:?} is incompatible with blob kind {blob_kind:?}")]
    IncompatibleBlobKind {
        asset_kind: AssetKind,
        blob_kind: MediaKind,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MediaAssetMutationError {
    #[error("the media asset update has a stale revision (expected {expected}, actual {actual})")]
    StaleRevision {
        expected: Revision,
        actual: Revision,
    },
    #[error("media asset revision cannot be incremented")]
    RevisionOverflow,
    #[error("media asset is invalid: {0}")]
    InvalidAsset(#[source] MediaAssetValidationError),
    #[error("integrity marking only accepts missing or corrupt, not {0:?}")]
    InvalidIntegrityState(AssetState),
}

impl From<MediaAssetValidationError> for MediaAssetMutationError {
    fn from(error: MediaAssetValidationError) -> Self {
        match error {
            MediaAssetValidationError::RevisionOverflow => Self::RevisionOverflow,
            other => Self::InvalidAsset(other),
        }
    }
}

fn validate_bounded_text(
    value: &str,
    field: &'static str,
    max_scalars: usize,
) -> Result<(), AssetProvenanceError> {
    if value.trim().is_empty() {
        return Err(AssetProvenanceError::Empty { field });
    }
    if value.chars().count() > max_scalars {
        return Err(AssetProvenanceError::TooLong { field });
    }
    if value.chars().any(char::is_control) {
        return Err(AssetProvenanceError::ControlCharacter { field });
    }
    Ok(())
}

fn validate_redacted_uri(value: &str) -> Result<(), AssetProvenanceError> {
    if value.chars().count() > MAX_SOURCE_URI_SCALARS
        || value.is_empty()
        || value.chars().any(char::is_control)
        || value.chars().any(char::is_whitespace)
        || value.contains('?')
        || value.contains('@')
        || value.contains("..")
        || value.starts_with('/')
        || value.starts_with('\\')
        || (value.len() >= 3
            && value.as_bytes()[0].is_ascii_alphabetic()
            && value.as_bytes()[1] == b':'
            && (value.as_bytes()[2] == b'/' || value.as_bytes()[2] == b'\\'))
    {
        return Err(AssetProvenanceError::UnsafeSourceUri);
    }
    let (scheme, authority_and_path) = value
        .split_once("://")
        .ok_or(AssetProvenanceError::UnsafeSourceUri)?;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return Err(AssetProvenanceError::UnsafeSourceUri);
    }
    let authority_end = authority_and_path
        .find(['/', '#'])
        .unwrap_or(authority_and_path.len());
    let authority = &authority_and_path[..authority_end];
    if authority.is_empty()
        || !authority
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || ".-:[]".contains(character))
    {
        return Err(AssetProvenanceError::UnsafeSourceUri);
    }
    Ok(())
}

fn validate_imported_format(value: &str) -> Result<(), AssetProvenanceError> {
    if value.is_empty() || value.chars().count() > MAX_IMPORTED_FORMAT_SCALARS {
        return Err(AssetProvenanceError::UnsafeImportedFormat);
    }
    if value.chars().any(|character| {
        !character.is_ascii()
            || character.is_control()
            || character.is_whitespace()
            || matches!(character, '\\' | ':' | ';' | '?' | '#')
            || !matches!(character, 'a'..='z' | 'A'..='Z' | '0'..='9' | '/' | '+' | '.' | '-')
    }) {
        return Err(AssetProvenanceError::UnsafeImportedFormat);
    }
    Ok(())
}

/// A typed retaining owner. No caller-supplied owner-kind string exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum AssetRetainer {
    Character(CharacterId),
    Persona(PersonaId),
    Group(GroupId),
    Scene(SceneId),
    SceneVariant(SceneVariantId),
    SceneAssetLink(SceneAssetLinkId),
    Conversation(ConversationId),
    Message(MessageId),
    PromptDocument(PromptDocumentId),
    Lorebook(LorebookId),
    LorebookEntry(LorebookEntryId),
}

/// An alias emphasizing that this list is used for reachability/retention.
pub type AssetReference = AssetRetainer;

/// Errors while reading typed asset associations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AssetReferenceReaderError {
    #[error("asset reference data are invalid")]
    InvalidData,
    #[error("asset reference storage failed")]
    Storage,
}

/// Reads domain-owned references that keep an asset reachable.
pub trait AssetReferenceReader: Send + Sync {
    fn retaining_owners(
        &self,
        asset_id: AssetId,
    ) -> Result<Vec<AssetRetainer>, AssetReferenceReaderError>;

    fn references_for(
        &self,
        asset_id: AssetId,
    ) -> Result<Vec<AssetRetainer>, AssetReferenceReaderError> {
        self.retaining_owners(asset_id)
    }
}

/// A narrow reachability query for retention and repair workflows.
pub trait AssetRetentionReader: Send + Sync {
    fn is_retained(&self, asset_id: AssetId) -> Result<bool, AssetReferenceReaderError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(
        kind: AssetKind,
        retention: RetentionClass,
        provenance: AssetProvenanceV1,
    ) -> MediaAsset {
        MediaAsset::new(
            AssetId::new(),
            MediaBlobId::new(),
            kind,
            AssetOrigin::Upload,
            retention,
            AssetState::Ready,
            provenance,
            Revision::INITIAL,
            TimestampMillis::new(10),
            TimestampMillis::new(10),
        )
        .expect("test asset should be valid")
    }

    #[test]
    fn temporary_retention_always_carries_an_expiry() {
        let retention = RetentionClass::Temporary {
            expires_at: TimestampMillis::new(42),
        };
        assert_eq!(retention.expires_at(), Some(TimestampMillis::new(42)));
        assert!(retention.is_temporary());
        assert_eq!(RetentionClass::Library.expires_at(), None);
    }

    #[test]
    fn logical_assets_can_share_a_blob_without_sharing_retention_or_provenance() {
        let blob_id = MediaBlobId::new();
        let first = MediaAsset::new(
            AssetId::new(),
            blob_id,
            AssetKind::AvatarOriginal,
            AssetOrigin::Upload,
            RetentionClass::Library,
            AssetState::Ready,
            AssetProvenanceV1 {
                source_label: Some("avatar".to_owned()),
                ..AssetProvenanceV1::default()
            },
            Revision::INITIAL,
            TimestampMillis::new(1),
            TimestampMillis::new(1),
        )
        .expect("first asset should be valid");
        let second = MediaAsset::new(
            AssetId::new(),
            blob_id,
            AssetKind::GeneratedImage,
            AssetOrigin::Generated,
            RetentionClass::Temporary {
                expires_at: TimestampMillis::new(100),
            },
            AssetState::Ready,
            AssetProvenanceV1 {
                producing_job_id: Some(JobId::new()),
                ..AssetProvenanceV1::default()
            },
            Revision::INITIAL,
            TimestampMillis::new(2),
            TimestampMillis::new(2),
        )
        .expect("second asset should be valid");

        assert_eq!(first.blob_id, second.blob_id);
        assert_ne!(first.id, second.id);
        assert_ne!(first.retention, second.retention);
        assert_ne!(first.provenance, second.provenance);
    }

    #[test]
    fn zero_revision_is_rejected() {
        let result = MediaAsset::new(
            AssetId::new(),
            MediaBlobId::new(),
            AssetKind::OtherImage,
            AssetOrigin::Import,
            RetentionClass::Persistent,
            AssetState::Ready,
            AssetProvenanceV1::default(),
            Revision::new(0),
            TimestampMillis::new(1),
            TimestampMillis::new(1),
        );

        assert_eq!(result, Err(MediaAssetValidationError::ZeroRevision));
    }

    #[test]
    fn blob_kind_compatibility_is_explicit() {
        let image = asset(
            AssetKind::Illustration,
            RetentionClass::Persistent,
            AssetProvenanceV1::default(),
        );
        assert!(image.validate_for_blob_kind(MediaKind::Image).is_ok());
        assert!(matches!(
            image.validate_for_blob_kind(MediaKind::Audio),
            Err(MediaAssetValidationError::IncompatibleBlobKind { .. })
        ));

        let audio = asset(
            AssetKind::MessageAudio,
            RetentionClass::Persistent,
            AssetProvenanceV1::default(),
        );
        assert!(audio.validate_for_blob_kind(MediaKind::Audio).is_ok());
    }

    #[test]
    fn retention_and_integrity_mutations_are_revision_checked_and_increment_once() {
        let mut image = asset(
            AssetKind::Illustration,
            RetentionClass::Library,
            AssetProvenanceV1::default(),
        );
        image
            .update_retention(
                Revision::INITIAL,
                RetentionClass::Temporary {
                    expires_at: TimestampMillis::new(99),
                },
                TimestampMillis::new(20),
            )
            .expect("matching retention revision");
        assert_eq!(image.revision, Revision::new(2));
        assert_eq!(image.updated_at, TimestampMillis::new(20));
        assert!(matches!(
            image.update_retention(
                Revision::INITIAL,
                RetentionClass::Persistent,
                TimestampMillis::new(21)
            ),
            Err(MediaAssetMutationError::StaleRevision { .. })
        ));

        image
            .mark_missing_or_corrupt(
                Revision::new(2),
                AssetState::Corrupt,
                TimestampMillis::new(22),
            )
            .expect("matching integrity revision");
        assert_eq!(image.state, AssetState::Corrupt);
        assert_eq!(image.revision, Revision::new(3));
        assert!(matches!(
            image.mark_missing_or_corrupt(
                Revision::new(3),
                AssetState::Deleted,
                TimestampMillis::new(23)
            ),
            Err(MediaAssetMutationError::InvalidIntegrityState(
                AssetState::Deleted
            ))
        ));
        assert_eq!(image.revision, Revision::new(3));
    }

    #[test]
    fn provenance_rejects_paths_queries_credentials_and_unbounded_shapes() {
        for uri in [
            "/tmp/image.png",
            "C:\\Users\\me\\image.png",
            "file:///tmp/image.png",
            "data:image/png;base64,AAAA",
            "https://user:secret@example.com/image.png",
            "https://example.com/image.png?token=secret",
        ] {
            let provenance = AssetProvenanceV1 {
                source_uri_redacted: Some(uri.to_owned()),
                ..AssetProvenanceV1::default()
            };
            assert!(matches!(
                provenance.validate(),
                Err(AssetProvenanceError::UnsafeSourceUri)
            ));
        }

        let valid = AssetProvenanceV1 {
            source_uri_redacted: Some("https://example.com/image.png#blake3-abc".to_owned()),
            imported_format: Some("image/png".to_owned()),
            ..AssetProvenanceV1::default()
        };
        assert!(valid.validate().is_ok());

        let label = AssetProvenanceV1 {
            source_label: Some("   \n".to_owned()),
            ..AssetProvenanceV1::default()
        };
        assert!(matches!(
            label.validate(),
            Err(AssetProvenanceError::Empty { .. })
        ));

        let too_long = AssetProvenanceV1 {
            source_label: Some("x".repeat(MAX_SOURCE_LABEL_SCALARS + 1)),
            ..AssetProvenanceV1::default()
        };
        assert!(matches!(
            too_long.validate(),
            Err(AssetProvenanceError::TooLong { .. })
        ));
    }

    #[test]
    fn versioned_closed_shapes_round_trip_and_reject_unknown_fields() {
        let provenance = AssetProvenanceV1 {
            source_label: Some("legacy import".to_owned()),
            imported_format: Some("image/webp".to_owned()),
            ..AssetProvenanceV1::default()
        };
        let encoded = serde_json::to_string(&provenance).expect("serialize provenance");
        assert_eq!(
            serde_json::from_str::<AssetProvenanceV1>(&encoded).expect("deserialize provenance"),
            provenance
        );
        assert!(
            serde_json::from_str::<AssetProvenanceV1>(
                r#"{"source_label":"import","unexpected":"value"}"#
            )
            .is_err()
        );

        let temporary = serde_json::to_value(RetentionClass::Temporary {
            expires_at: TimestampMillis::new(9),
        })
        .expect("serialize retention");
        assert_eq!(temporary["temporary"]["expires_at"], 9);
    }

    #[test]
    fn provenance_requires_the_current_wire_format_version() {
        assert_eq!(
            AssetProvenanceV1 {
                format_version: 2,
                ..AssetProvenanceV1::default()
            }
            .validate(),
            Err(AssetProvenanceError::UnsupportedFormatVersion(2))
        );
        assert!(serde_json::from_str::<AssetProvenanceV1>(r#"{"source_label":"old"}"#).is_err());
    }
}
