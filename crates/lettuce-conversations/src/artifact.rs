//! Opaque, immutable payloads used by conversation-owned launch snapshots and
//! provider replay records.
//!
//! Conversation rows contain only [`ProtectedSnapshotRef`] and
//! [`ReplayArtifactRef`].  The bytes live behind [`ConversationArtifactStore`]
//! so repositories, IPC DTOs, logs, backups, and sync code cannot accidentally
//! move a provider payload through the ordinary conversation path.

use std::fmt;

use blake3::Hash;
use lettuce_types::{ContentHash, ReplayArtifactId, SnapshotArtifactId};
use thiserror::Error;
use zeroize::{Zeroize, Zeroizing};

use crate::{
    ConversationKind, ProtectedSnapshotRef, ReplayArtifactRef, ReplayCodec, ReplayRetention,
    SnapshotSelection, SnapshotSource, ValidationError,
};

pub(crate) const MAX_PROTECTED_ARTIFACT_BYTES: usize = 16 * 1024 * 1024;

/// Secret-adjacent or provider-returned bytes which must not be serialized or
/// displayed.  The backing allocation is zeroized when this value is dropped.
///
/// This type deliberately has no `Serialize` implementation and its `Debug`
/// implementation contains only the byte count. Application and IPC code
/// should pass the value through to the store without opening it.
pub struct ProtectedArtifactBytes(Zeroizing<Vec<u8>>);

impl ProtectedArtifactBytes {
    pub fn new(bytes: impl Into<Vec<u8>>) -> Result<Self, ArtifactError> {
        let bytes = bytes.into();
        if bytes.is_empty() {
            return Err(ArtifactError::Empty);
        }
        if bytes.len() > MAX_PROTECTED_ARTIFACT_BYTES {
            return Err(ArtifactError::TooLarge {
                size: bytes.len(),
                max: MAX_PROTECTED_ARTIFACT_BYTES,
            });
        }
        Ok(Self(Zeroizing::new(bytes)))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn digest(&self) -> ContentHash {
        content_hash(self.0.as_slice())
    }

    /// Consumes the protected value for a concrete artifact-store adapter.
    /// There is intentionally no borrowing/read-back API: ordinary domain,
    /// repository, backup, and IPC code cannot inspect payload bytes.
    pub fn into_store_bytes(self) -> Vec<u8> {
        self.0.to_vec()
    }
}

impl fmt::Debug for ProtectedArtifactBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProtectedArtifactBytes")
            .field("byte_len", &self.len())
            .finish()
    }
}

impl PartialEq for ProtectedArtifactBytes {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_slice() == other.0.as_slice()
    }
}

impl Eq for ProtectedArtifactBytes {}

impl Drop for ProtectedArtifactBytes {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactCodec {
    Json,
    Cbor,
    Binary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactRetention {
    Conversation,
    Ephemeral,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ArtifactError {
    #[error("artifact payload must not be empty")]
    Empty,
    #[error("artifact payload is {size} bytes, maximum is {max}")]
    TooLarge { size: usize, max: usize },
    #[error("artifact schema version must be non-zero")]
    ZeroSchemaVersion,
    #[error("artifact metadata byte size does not match its payload")]
    SizeMismatch,
    #[error("artifact metadata digest does not match its payload")]
    DigestMismatch,
    #[error("artifact identity is immutable and already has different metadata")]
    ImmutableConflict,
    #[error("artifact was not found")]
    NotFound,
    #[error("artifact storage is unavailable")]
    Storage,
    #[error("artifact reference is invalid: {0}")]
    InvalidReference(ValidationError),
}

#[derive(Debug, PartialEq, Eq)]
pub struct SnapshotArtifactDraft {
    pub source: SnapshotSource,
    pub source_revision: lettuce_types::Revision,
    pub artifact_id: SnapshotArtifactId,
    pub digest: ContentHash,
    pub schema_version: u32,
    pub byte_size: u64,
    pub codec: ArtifactCodec,
    pub retention: ArtifactRetention,
    pub bytes: ProtectedArtifactBytes,
}

impl SnapshotArtifactDraft {
    pub fn validate(&self) -> Result<(), ArtifactError> {
        validate_metadata(
            &self.digest,
            self.schema_version,
            self.byte_size,
            self.bytes.len(),
            self.bytes.digest(),
        )?;
        if self.source_revision.get() == 0 {
            return Err(ArtifactError::InvalidReference(
                ValidationError::ZeroRevision,
            ));
        }
        if self.retention == ArtifactRetention::Ephemeral {
            return Err(ArtifactError::InvalidReference(
                ValidationError::InvalidValue {
                    field: "snapshot_artifact.retention",
                },
            ));
        }
        Ok(())
    }

    #[must_use]
    pub fn reference(&self) -> ProtectedSnapshotRef {
        ProtectedSnapshotRef {
            source: self.source,
            source_revision: self.source_revision,
            artifact_id: self.artifact_id,
            digest: self.digest.clone(),
            schema_version: self.schema_version,
            byte_size: self.byte_size,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ReplayArtifactDraft {
    pub artifact_id: ReplayArtifactId,
    pub digest: ContentHash,
    pub schema_version: u32,
    pub byte_size: u64,
    pub codec: ArtifactCodec,
    pub retention: ArtifactRetention,
    pub bytes: ProtectedArtifactBytes,
}

impl ReplayArtifactDraft {
    pub fn validate(&self) -> Result<(), ArtifactError> {
        validate_metadata(
            &self.digest,
            self.schema_version,
            self.byte_size,
            self.bytes.len(),
            self.bytes.digest(),
        )?;
        Ok(())
    }

    #[must_use]
    pub fn reference(&self) -> ReplayArtifactRef {
        ReplayArtifactRef {
            artifact_id: self.artifact_id,
            digest: self.digest.clone(),
            schema_version: self.schema_version,
            byte_size: self.byte_size,
            retention: match self.retention {
                ArtifactRetention::Conversation => ReplayRetention::Conversation,
                ArtifactRetention::Ephemeral => ReplayRetention::Ephemeral,
            },
            codec: match self.codec {
                ArtifactCodec::Json => ReplayCodec::Json,
                ArtifactCodec::Cbor => ReplayCodec::Cbor,
                ArtifactCodec::Binary => ReplayCodec::Binary,
            },
        }
    }
}

fn validate_metadata(
    digest: &ContentHash,
    schema_version: u32,
    byte_size: u64,
    actual_byte_len: usize,
    actual_digest: ContentHash,
) -> Result<(), ArtifactError> {
    if schema_version == 0 {
        return Err(ArtifactError::ZeroSchemaVersion);
    }
    if byte_size > MAX_PROTECTED_ARTIFACT_BYTES as u64 {
        return Err(ArtifactError::TooLarge {
            size: actual_byte_len,
            max: MAX_PROTECTED_ARTIFACT_BYTES,
        });
    }
    if byte_size != actual_byte_len as u64 {
        return Err(ArtifactError::SizeMismatch);
    }
    if digest != &actual_digest {
        return Err(ArtifactError::DigestMismatch);
    }
    Ok(())
}

fn content_hash(bytes: &[u8]) -> ContentHash {
    let hash: Hash = blake3::hash(bytes);
    ContentHash::parse(hash.to_hex().to_string()).expect("blake3 always produces a valid hash")
}

/// Synchronous persistence boundary for immutable snapshot and replay payloads.
///
/// Implementations must make `put_*` immutable by artifact identity and digest:
/// the same identity may be deduplicated, but a different payload or metadata
/// must return [`ArtifactError::ImmutableConflict`].  Cleanup of payloads that
/// were inserted before a failed conversation mutation belongs to the store
/// adapter. Backup/sync adapters use the separate trusted transfer port below
/// instead of reading ordinary conversation rows.
pub trait ConversationArtifactStore: Send + Sync {
    fn put_snapshot(
        &self,
        draft: SnapshotArtifactDraft,
    ) -> Result<ProtectedSnapshotRef, ArtifactError>;
    fn verify_snapshot(&self, reference: &ProtectedSnapshotRef) -> Result<(), ArtifactError>;
    /// Removes an artifact that was staged before its owning conversation
    /// mutation committed. Implementations must treat this as an orphan-only,
    /// idempotent cleanup operation and never delete a referenced artifact.
    fn cleanup_orphan_snapshot(&self, artifact_id: SnapshotArtifactId)
    -> Result<(), ArtifactError>;
    fn put_replay(&self, draft: ReplayArtifactDraft) -> Result<ReplayArtifactRef, ArtifactError>;
    fn verify_replay(&self, reference: &ReplayArtifactRef) -> Result<(), ArtifactError>;
    fn cleanup_orphan_replay(&self, artifact_id: ReplayArtifactId) -> Result<(), ArtifactError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustedArtifactDescriptor {
    Snapshot(ProtectedSnapshotRef),
    Replay(ReplayArtifactRef),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ArtifactTransferError {
    #[error("trusted artifact was not found")]
    NotFound,
    #[error("trusted artifact sink rejected a chunk")]
    SinkRejected,
    #[error("trusted artifact transfer storage is unavailable")]
    Storage,
}

/// Sink owned by an encrypted backup or sync adapter. It receives metadata
/// and borrowed chunks only; it never creates an ordinary serializable
/// artifact payload object.
pub trait TrustedArtifactSink {
    fn begin(
        &mut self,
        descriptor: &TrustedArtifactDescriptor,
    ) -> Result<(), ArtifactTransferError>;
    fn chunk(&mut self, bytes: &[u8]) -> Result<(), ArtifactTransferError>;
    fn finish(&mut self) -> Result<(), ArtifactTransferError>;
}

/// Separate trusted transfer capability for encrypted backup/sync composition.
/// This trait is intentionally not a supertrait of or field on
/// [`ConversationArtifactStore`] and is not reachable through
/// [`crate::ConversationRepository`].
pub trait ConversationArtifactTransferPort: Send + Sync {
    fn export_snapshot(
        &self,
        artifact_id: SnapshotArtifactId,
        sink: &mut dyn TrustedArtifactSink,
    ) -> Result<(), ArtifactTransferError>;
    fn export_replay(
        &self,
        artifact_id: ReplayArtifactId,
        sink: &mut dyn TrustedArtifactSink,
    ) -> Result<(), ArtifactTransferError>;
}

/// Returns every immutable snapshot reference embedded in a direct/group
/// launch document. Repositories use this exact traversal before creating the
/// conversation so a forged or missing artifact cannot enter the aggregate.
pub fn conversation_snapshot_references(kind: &ConversationKind) -> Vec<&ProtectedSnapshotRef> {
    let mut refs = Vec::new();
    match kind {
        ConversationKind::Direct(details) => {
            refs.push(&details.character.snapshot_ref);
            if let SnapshotSelection::Inherited(value) | SnapshotSelection::Explicit(value) =
                &details.persona
            {
                refs.push(&value.snapshot_ref);
            }
            if let SnapshotSelection::Inherited(value) | SnapshotSelection::Explicit(value) =
                &details.scene
            {
                refs.push(&value.snapshot_ref);
            }
            if let SnapshotSelection::Inherited(value) | SnapshotSelection::Explicit(value) =
                &details.starter
            {
                refs.push(&value.snapshot_ref);
            }
            if let SnapshotSelection::Inherited(value) | SnapshotSelection::Explicit(value) =
                &details.prompt
            {
                refs.push(&value.snapshot_ref);
            }
            if let SnapshotSelection::Inherited(values) | SnapshotSelection::Explicit(values) =
                &details.lorebooks
            {
                refs.extend(values.iter().map(|value| &value.snapshot_ref));
            }
            if let SnapshotSelection::Inherited(value) | SnapshotSelection::Explicit(value) =
                &details.model
            {
                refs.push(&value.snapshot_ref);
            }
            if let SnapshotSelection::Inherited(value) | SnapshotSelection::Explicit(value) =
                &details.memory
            {
                if let Some(reference) = &value.policy_ref {
                    refs.push(reference);
                }
            }
            if let SnapshotSelection::Inherited(value) | SnapshotSelection::Explicit(value) =
                &details.voice
            {
                refs.push(&value.snapshot_ref);
            }
        }
        ConversationKind::Group(details) => {
            let group = &details.group;
            refs.push(&group.snapshot_ref);
            for member in &group.members {
                refs.push(&member.character.snapshot_ref);
                if let SnapshotSelection::Inherited(value) | SnapshotSelection::Explicit(value) =
                    &member.model_override
                {
                    refs.push(&value.snapshot_ref);
                }
            }
            if let SnapshotSelection::Inherited(value) | SnapshotSelection::Explicit(value) =
                &group.memory
            {
                if let Some(reference) = &value.policy_ref {
                    refs.push(reference);
                }
            }
            if let SnapshotSelection::Inherited(value) | SnapshotSelection::Explicit(value) =
                &group.persona
            {
                refs.push(&value.snapshot_ref);
            }
            if let SnapshotSelection::Inherited(value) | SnapshotSelection::Explicit(value) =
                &group.scene
            {
                refs.push(&value.snapshot_ref);
            }
            if let SnapshotSelection::Inherited(value) | SnapshotSelection::Explicit(value) =
                &group.prompt
            {
                refs.push(&value.snapshot_ref);
            }
            if let SnapshotSelection::Inherited(values) | SnapshotSelection::Explicit(values) =
                &group.lorebooks
            {
                refs.extend(values.iter().map(|value| &value.snapshot_ref));
            }
            if let SnapshotSelection::Inherited(value) | SnapshotSelection::Explicit(value) =
                &group.model
            {
                refs.push(&value.snapshot_ref);
            }
        }
    }
    refs
}
