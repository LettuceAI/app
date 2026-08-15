//! Private SQLite storage for protected conversation artifacts.
//!
//! This module intentionally implements only the artifact and trusted-transfer
//! ports. Conversation rows never contain bytes and no ordinary repository API
//! exposes a read-back handle.

use std::str::FromStr;

use blake3::Hash;
use lettuce_conversations::{
    ArtifactCodec, ArtifactError, ArtifactRetention, ConversationArtifactStore,
    ConversationArtifactTransferPort, ProtectedSnapshotRef, ReplayArtifactDraft, ReplayArtifactRef,
    ReplayCodec, ReplayRetention, SnapshotArtifactDraft, SnapshotSource, TrustedArtifactDescriptor,
    TrustedArtifactSink,
};
use lettuce_types::{ContentHash, ReplayArtifactId, Revision, SnapshotArtifactId};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use zeroize::Zeroizing;

use super::Database;

const CHUNK_SIZE: usize = 64 * 1024;

type SnapshotStoredRow = (
    String,
    String,
    i64,
    String,
    i64,
    i64,
    String,
    String,
    Vec<u8>,
);
type SnapshotExistingRow = (
    String,
    String,
    i64,
    String,
    i64,
    String,
    i64,
    String,
    Vec<u8>,
);

fn hash(bytes: &[u8]) -> ContentHash {
    let value: Hash = blake3::hash(bytes);
    ContentHash::parse(value.to_hex().to_string()).expect("blake3 hash is always valid")
}

fn db_error(_: rusqlite::Error) -> ArtifactError {
    ArtifactError::Storage
}

fn source_parts(source: SnapshotSource) -> (&'static str, String) {
    match source {
        SnapshotSource::Character(id) => ("character", id.to_string()),
        SnapshotSource::Persona(id) => ("persona", id.to_string()),
        SnapshotSource::Scene(id) => ("scene", id.to_string()),
        SnapshotSource::Starter(id) => ("starter", id.to_string()),
        SnapshotSource::Prompt(id) => ("prompt", id.to_string()),
        SnapshotSource::Lorebook(id) => ("lorebook", id.to_string()),
        SnapshotSource::Model(id) => ("model", id.to_string()),
        SnapshotSource::Voice(id) => ("voice", id.to_string()),
        SnapshotSource::Group(id) => ("group", id.to_string()),
        SnapshotSource::ProviderAccount(id) => ("provider_account", id.to_string()),
    }
}

pub(crate) fn source_from_parts(kind: &str, id: &str) -> Result<SnapshotSource, ArtifactError> {
    macro_rules! parse {
        ($ty:ty, $variant:path) => {
            <$ty>::from_str(id)
                .map($variant)
                .map_err(|_| ArtifactError::Storage)
        };
    }
    match kind {
        "character" => parse!(lettuce_types::CharacterId, SnapshotSource::Character),
        "persona" => parse!(lettuce_types::PersonaId, SnapshotSource::Persona),
        "scene" => parse!(lettuce_types::SceneId, SnapshotSource::Scene),
        "starter" => parse!(
            lettuce_types::ConversationStarterId,
            SnapshotSource::Starter
        ),
        "prompt" => parse!(lettuce_types::PromptDocumentId, SnapshotSource::Prompt),
        "lorebook" => parse!(lettuce_types::LorebookId, SnapshotSource::Lorebook),
        "model" => parse!(lettuce_types::ModelProfileId, SnapshotSource::Model),
        "voice" => parse!(lettuce_types::VoiceProfileId, SnapshotSource::Voice),
        "group" => parse!(lettuce_types::GroupId, SnapshotSource::Group),
        "provider_account" => parse!(
            lettuce_types::ProviderAccountId,
            SnapshotSource::ProviderAccount
        ),
        _ => Err(ArtifactError::Storage),
    }
}

fn codec_name(value: ArtifactCodec) -> &'static str {
    match value {
        ArtifactCodec::Json => "json",
        ArtifactCodec::Cbor => "cbor",
        ArtifactCodec::Binary => "binary",
    }
}
fn retention_name(value: ArtifactRetention) -> &'static str {
    match value {
        ArtifactRetention::Conversation => "conversation",
        ArtifactRetention::Ephemeral => "ephemeral",
    }
}

fn codec_from_name(value: &str) -> Result<ArtifactCodec, ArtifactError> {
    match value {
        "json" => Ok(ArtifactCodec::Json),
        "cbor" => Ok(ArtifactCodec::Cbor),
        "binary" => Ok(ArtifactCodec::Binary),
        _ => Err(ArtifactError::Storage),
    }
}

fn retention_from_name(value: &str) -> Result<ArtifactRetention, ArtifactError> {
    match value {
        "conversation" => Ok(ArtifactRetention::Conversation),
        "ephemeral" => Ok(ArtifactRetention::Ephemeral),
        _ => Err(ArtifactError::Storage),
    }
}

fn sql_u64(value: u64) -> Result<i64, ArtifactError> {
    i64::try_from(value).map_err(|_| ArtifactError::Storage)
}

fn verify_payload(bytes: &[u8], digest: &ContentHash, size: u64) -> Result<(), ArtifactError> {
    if u64::try_from(bytes.len()).map_err(|_| ArtifactError::Storage)? != size {
        return Err(ArtifactError::SizeMismatch);
    }
    if &hash(bytes) != digest {
        return Err(ArtifactError::DigestMismatch);
    }
    Ok(())
}

/// Verifies a staged snapshot while the owning conversation mutation holds its
/// immediate transaction. Artifact bytes remain private to this adapter even
/// when a repository mutation needs an atomic reference check.
pub(crate) fn verify_snapshot_in_transaction(
    transaction: &Transaction<'_>,
    reference: &ProtectedSnapshotRef,
) -> Result<(), ArtifactError> {
    reference
        .validate()
        .map_err(ArtifactError::InvalidReference)?;
    let row: Option<SnapshotStoredRow> = transaction
        .query_row(
            "SELECT source_kind, source_id, source_revision, digest, schema_version, byte_size, codec, retention, bytes FROM conversation_snapshot_artifacts WHERE artifact_id = ?1",
            [reference.artifact_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?)),
        )
        .optional()
        .map_err(db_error)?;
    let Some((kind, source_id, source_revision, digest, schema, size, codec, retention, bytes)) =
        row
    else {
        return Err(ArtifactError::NotFound);
    };
    let bytes = Zeroizing::new(bytes);
    if source_revision < 1 || schema < 1 || size < 1 {
        return Err(ArtifactError::Storage);
    }
    codec_from_name(&codec)?;
    if retention_from_name(&retention)? != ArtifactRetention::Conversation {
        return Err(ArtifactError::Storage);
    }
    let stored_source = source_from_parts(&kind, &source_id)?;
    let stored_digest = ContentHash::parse(&digest).map_err(|_| ArtifactError::Storage)?;
    if stored_source != reference.source
        || source_revision != sql_u64(reference.source_revision.get())?
        || digest != reference.digest.as_str()
        || schema != i64::from(reference.schema_version)
        || size != sql_u64(reference.byte_size)?
    {
        return Err(ArtifactError::Storage);
    }
    verify_payload(&bytes, &stored_digest, reference.byte_size)
}

/// Verifies a replay artifact while the read transaction still owns the same
/// SQLite snapshot used to hydrate its referencing revision or candidate.
pub(crate) fn verify_replay_in_transaction(
    transaction: &Transaction<'_>,
    reference: &ReplayArtifactRef,
) -> Result<(), ArtifactError> {
    reference
        .validate()
        .map_err(ArtifactError::InvalidReference)?;
    let row: Option<(String, i64, i64, String, String, Vec<u8>)> = transaction
        .query_row(
            "SELECT digest, schema_version, byte_size, codec, retention, bytes FROM conversation_replay_artifacts WHERE artifact_id = ?1",
            [reference.artifact_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
        )
        .optional()
        .map_err(db_error)?;
    let Some((digest, schema, size, codec, retention, bytes)) = row else {
        return Err(ArtifactError::NotFound);
    };
    let bytes = Zeroizing::new(bytes);
    if schema < 1 || size < 1 {
        return Err(ArtifactError::Storage);
    }
    let stored_digest = ContentHash::parse(&digest).map_err(|_| ArtifactError::Storage)?;
    let expected_codec = match reference.codec {
        ReplayCodec::Json => ArtifactCodec::Json,
        ReplayCodec::Cbor => ArtifactCodec::Cbor,
        ReplayCodec::Binary => ArtifactCodec::Binary,
    };
    let expected_retention = match reference.retention {
        ReplayRetention::Conversation => ArtifactRetention::Conversation,
        ReplayRetention::Ephemeral => ArtifactRetention::Ephemeral,
    };
    if codec_from_name(&codec)? != expected_codec
        || retention_from_name(&retention)? != expected_retention
        || digest != reference.digest.as_str()
        || schema != i64::from(reference.schema_version)
        || size != sql_u64(reference.byte_size)?
    {
        return Err(ArtifactError::Storage);
    }
    verify_payload(&bytes, &stored_digest, reference.byte_size)
}

impl ConversationArtifactStore for Database {
    fn put_snapshot(
        &self,
        draft: SnapshotArtifactDraft,
    ) -> Result<ProtectedSnapshotRef, ArtifactError> {
        draft.validate()?;
        let reference = draft.reference();
        let (source_kind, source_id) = source_parts(draft.source);
        let artifact_codec = draft.codec;
        let source_revision = sql_u64(draft.source_revision.get())?;
        let byte_size = sql_u64(reference.byte_size)?;
        let bytes = Zeroizing::new(draft.bytes.into_store_bytes());
        let mut connection = self.connection().map_err(|_| ArtifactError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let existing: Option<SnapshotExistingRow> = transaction.query_row("SELECT source_kind, source_id, source_revision, digest, schema_version, codec, byte_size, retention, bytes FROM conversation_snapshot_artifacts WHERE artifact_id = ?1", [reference.artifact_id.to_string()], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?))).optional().map_err(db_error)?;
        if let Some((kind, id, revision, digest, schema, codec, size, retention, stored_bytes)) =
            existing
        {
            let stored_bytes = Zeroizing::new(stored_bytes);
            if revision < 1 || schema < 1 || size < 1 {
                return Err(ArtifactError::Storage);
            }
            ContentHash::parse(&digest).map_err(|_| ArtifactError::Storage)?;
            if codec_from_name(&codec)? != artifact_codec
                || retention_from_name(&retention)? != ArtifactRetention::Conversation
                || kind != source_kind
                || id != source_id
                || revision != source_revision
                || digest != reference.digest.as_str()
                || schema != i64::from(reference.schema_version)
                || size != byte_size
            {
                return Err(ArtifactError::ImmutableConflict);
            }
            verify_payload(&stored_bytes, &reference.digest, reference.byte_size)?;
            transaction.commit().map_err(db_error)?;
            return Ok(reference);
        }
        transaction.execute("INSERT INTO conversation_snapshot_artifacts (artifact_id, source_kind, source_id, source_revision, digest, schema_version, byte_size, codec, retention, bytes, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'conversation', ?9, ?10)", params![reference.artifact_id.to_string(), source_kind, source_id, source_revision, reference.digest.as_str(), i64::from(reference.schema_version), byte_size, codec_name(artifact_codec), bytes.as_slice(), lettuce_types::TimestampMillis::now().map_err(|_| ArtifactError::Storage)?.get()]).map_err(db_error)?;
        transaction.commit().map_err(db_error)?;
        Ok(reference)
    }

    fn verify_snapshot(&self, reference: &ProtectedSnapshotRef) -> Result<(), ArtifactError> {
        reference
            .validate()
            .map_err(ArtifactError::InvalidReference)?;
        let connection = self.connection().map_err(|_| ArtifactError::Storage)?;
        let row: Option<SnapshotStoredRow> = connection.query_row("SELECT source_kind, source_id, source_revision, digest, schema_version, byte_size, codec, retention, bytes FROM conversation_snapshot_artifacts WHERE artifact_id = ?1", [reference.artifact_id.to_string()], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?))).optional().map_err(db_error)?;
        let Some((kind, source_id, source_revision, digest, schema, size, codec, retention, bytes)) =
            row
        else {
            return Err(ArtifactError::NotFound);
        };
        let bytes = Zeroizing::new(bytes);
        if source_revision < 1 || schema < 1 || size < 1 {
            return Err(ArtifactError::Storage);
        }
        codec_from_name(&codec)?;
        ContentHash::parse(&digest).map_err(|_| ArtifactError::Storage)?;
        if retention_from_name(&retention)? != ArtifactRetention::Conversation
            || source_from_parts(&kind, &source_id)? != reference.source
            || source_revision != sql_u64(reference.source_revision.get())?
            || digest != reference.digest.as_str()
            || schema != i64::from(reference.schema_version)
            || size != sql_u64(reference.byte_size)?
        {
            return Err(ArtifactError::ImmutableConflict);
        }
        verify_payload(&bytes, &reference.digest, reference.byte_size)
    }

    fn cleanup_orphan_snapshot(
        &self,
        artifact_id: SnapshotArtifactId,
    ) -> Result<(), ArtifactError> {
        let mut connection = self.connection().map_err(|_| ArtifactError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        transaction.execute("DELETE FROM conversation_snapshot_artifacts WHERE artifact_id = ?1 AND NOT EXISTS (SELECT 1 FROM conversation_snapshot_refs WHERE artifact_id = ?1)", [artifact_id.to_string()]).map_err(db_error)?;
        transaction.commit().map_err(db_error)?;
        Ok(())
    }

    fn put_replay(&self, draft: ReplayArtifactDraft) -> Result<ReplayArtifactRef, ArtifactError> {
        draft.validate()?;
        let reference = draft.reference();
        let artifact_codec = draft.codec;
        let artifact_retention = draft.retention;
        let byte_size = sql_u64(reference.byte_size)?;
        let bytes = Zeroizing::new(draft.bytes.into_store_bytes());
        let mut connection = self.connection().map_err(|_| ArtifactError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let existing: Option<(String, i64, i64, String, String, Vec<u8>)> = transaction.query_row("SELECT digest, schema_version, byte_size, codec, retention, bytes FROM conversation_replay_artifacts WHERE artifact_id = ?1", [reference.artifact_id.to_string()], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))).optional().map_err(db_error)?;
        if let Some((digest, schema, size, codec, retention, stored_bytes)) = existing {
            let stored_bytes = Zeroizing::new(stored_bytes);
            if schema < 1 || size < 1 {
                return Err(ArtifactError::Storage);
            }
            ContentHash::parse(&digest).map_err(|_| ArtifactError::Storage)?;
            if codec_from_name(&codec)? != artifact_codec
                || retention_from_name(&retention)? != artifact_retention
                || digest != reference.digest.as_str()
                || schema != i64::from(reference.schema_version)
                || size != byte_size
            {
                return Err(ArtifactError::ImmutableConflict);
            }
            verify_payload(&stored_bytes, &reference.digest, reference.byte_size)?;
            transaction.commit().map_err(db_error)?;
            return Ok(reference);
        }
        transaction.execute("INSERT INTO conversation_replay_artifacts (artifact_id, digest, schema_version, byte_size, codec, retention, bytes, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)", params![reference.artifact_id.to_string(), reference.digest.as_str(), i64::from(reference.schema_version), byte_size, codec_name(artifact_codec), retention_name(artifact_retention), bytes.as_slice(), lettuce_types::TimestampMillis::now().map_err(|_| ArtifactError::Storage)?.get()]).map_err(db_error)?;
        transaction.commit().map_err(db_error)?;
        Ok(reference)
    }

    fn verify_replay(&self, reference: &ReplayArtifactRef) -> Result<(), ArtifactError> {
        reference
            .validate()
            .map_err(ArtifactError::InvalidReference)?;
        let connection = self.connection().map_err(|_| ArtifactError::Storage)?;
        let row: Option<(String, i64, i64, String, String, Vec<u8>)> = connection.query_row("SELECT digest, schema_version, byte_size, codec, retention, bytes FROM conversation_replay_artifacts WHERE artifact_id = ?1", [reference.artifact_id.to_string()], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))).optional().map_err(db_error)?;
        let Some((digest, schema, size, codec, retention, bytes)) = row else {
            return Err(ArtifactError::NotFound);
        };
        let bytes = Zeroizing::new(bytes);
        if schema < 1 || size < 1 {
            return Err(ArtifactError::Storage);
        }
        ContentHash::parse(&digest).map_err(|_| ArtifactError::Storage)?;
        if codec_from_name(&codec)?
            != match reference.codec {
                ReplayCodec::Json => ArtifactCodec::Json,
                ReplayCodec::Cbor => ArtifactCodec::Cbor,
                ReplayCodec::Binary => ArtifactCodec::Binary,
            }
            || retention_from_name(&retention)?
                != match reference.retention {
                    ReplayRetention::Conversation => ArtifactRetention::Conversation,
                    ReplayRetention::Ephemeral => ArtifactRetention::Ephemeral,
                }
            || digest != reference.digest.as_str()
            || schema != i64::from(reference.schema_version)
            || size != sql_u64(reference.byte_size)?
        {
            return Err(ArtifactError::ImmutableConflict);
        }
        verify_payload(&bytes, &reference.digest, reference.byte_size)
    }

    fn cleanup_orphan_replay(&self, artifact_id: ReplayArtifactId) -> Result<(), ArtifactError> {
        let mut connection = self.connection().map_err(|_| ArtifactError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        transaction.execute("DELETE FROM conversation_replay_artifacts WHERE artifact_id = ?1 AND NOT EXISTS (SELECT 1 FROM conversation_message_revisions WHERE provider_replay_artifact_id = ?1) AND NOT EXISTS (SELECT 1 FROM conversation_message_candidates WHERE provider_replay_artifact_id = ?1)", [artifact_id.to_string()]).map_err(db_error)?;
        transaction.commit().map_err(db_error)?;
        Ok(())
    }
}

impl ConversationArtifactTransferPort for Database {
    fn export_snapshot(
        &self,
        artifact_id: SnapshotArtifactId,
        sink: &mut dyn TrustedArtifactSink,
    ) -> Result<(), lettuce_conversations::ArtifactTransferError> {
        export_snapshot(self, artifact_id, sink)
    }
    fn export_replay(
        &self,
        artifact_id: ReplayArtifactId,
        sink: &mut dyn TrustedArtifactSink,
    ) -> Result<(), lettuce_conversations::ArtifactTransferError> {
        export_replay(self, artifact_id, sink)
    }
}

fn export_snapshot(
    database: &Database,
    artifact_id: SnapshotArtifactId,
    sink: &mut dyn TrustedArtifactSink,
) -> Result<(), lettuce_conversations::ArtifactTransferError> {
    let connection = database
        .connection()
        .map_err(|_| lettuce_conversations::ArtifactTransferError::Storage)?;
    let row: Option<SnapshotStoredRow> = connection.query_row("SELECT source_kind, source_id, source_revision, digest, schema_version, byte_size, codec, retention, bytes FROM conversation_snapshot_artifacts WHERE artifact_id = ?1", [artifact_id.to_string()], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?))).optional().map_err(|_| lettuce_conversations::ArtifactTransferError::Storage)?;
    let Some((kind, source_id, source_revision, digest, schema, size, codec, retention, bytes)) =
        row
    else {
        return Err(lettuce_conversations::ArtifactTransferError::NotFound);
    };
    if source_revision < 1 || schema < 1 || size < 1 || retention != "conversation" {
        return Err(lettuce_conversations::ArtifactTransferError::Storage);
    }
    let _codec = codec_from_name(&codec)
        .map_err(|_| lettuce_conversations::ArtifactTransferError::Storage)?;
    let digest = ContentHash::parse(digest)
        .map_err(|_| lettuce_conversations::ArtifactTransferError::Storage)?;
    let byte_size =
        u64::try_from(size).map_err(|_| lettuce_conversations::ArtifactTransferError::Storage)?;
    let bytes = Zeroizing::new(bytes);
    verify_payload(&bytes, &digest, byte_size)
        .map_err(|_| lettuce_conversations::ArtifactTransferError::Storage)?;
    let descriptor = TrustedArtifactDescriptor::Snapshot(ProtectedSnapshotRef {
        source: source_from_parts(&kind, &source_id)
            .map_err(|_| lettuce_conversations::ArtifactTransferError::Storage)?,
        source_revision: Revision::new(
            source_revision
                .try_into()
                .map_err(|_| lettuce_conversations::ArtifactTransferError::Storage)?,
        ),
        artifact_id,
        digest,
        schema_version: schema
            .try_into()
            .map_err(|_| lettuce_conversations::ArtifactTransferError::Storage)?,
        byte_size,
    });
    if let TrustedArtifactDescriptor::Snapshot(reference) = &descriptor {
        reference
            .validate()
            .map_err(|_| lettuce_conversations::ArtifactTransferError::Storage)?;
    }
    sink.begin(&descriptor)?;
    for chunk in bytes.chunks(CHUNK_SIZE) {
        sink.chunk(chunk)?;
    }
    sink.finish()
}

fn export_replay(
    database: &Database,
    artifact_id: ReplayArtifactId,
    sink: &mut dyn TrustedArtifactSink,
) -> Result<(), lettuce_conversations::ArtifactTransferError> {
    let connection = database
        .connection()
        .map_err(|_| lettuce_conversations::ArtifactTransferError::Storage)?;
    let row: Option<(String, i64, i64, String, String, Vec<u8>)> = connection.query_row("SELECT digest, schema_version, byte_size, codec, retention, bytes FROM conversation_replay_artifacts WHERE artifact_id = ?1", [artifact_id.to_string()], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))).optional().map_err(|_| lettuce_conversations::ArtifactTransferError::Storage)?;
    let Some((digest, schema, size, codec, retention, bytes)) = row else {
        return Err(lettuce_conversations::ArtifactTransferError::NotFound);
    };
    if schema < 1 || size < 1 {
        return Err(lettuce_conversations::ArtifactTransferError::Storage);
    }
    let digest = ContentHash::parse(digest)
        .map_err(|_| lettuce_conversations::ArtifactTransferError::Storage)?;
    let byte_size =
        u64::try_from(size).map_err(|_| lettuce_conversations::ArtifactTransferError::Storage)?;
    let bytes = Zeroizing::new(bytes);
    verify_payload(&bytes, &digest, byte_size)
        .map_err(|_| lettuce_conversations::ArtifactTransferError::Storage)?;
    let descriptor = TrustedArtifactDescriptor::Replay(ReplayArtifactRef {
        artifact_id,
        digest,
        schema_version: schema
            .try_into()
            .map_err(|_| lettuce_conversations::ArtifactTransferError::Storage)?,
        byte_size,
        codec: match codec.as_str() {
            "json" => ReplayCodec::Json,
            "cbor" => ReplayCodec::Cbor,
            "binary" => ReplayCodec::Binary,
            _ => return Err(lettuce_conversations::ArtifactTransferError::Storage),
        },
        retention: match retention.as_str() {
            "conversation" => ReplayRetention::Conversation,
            "ephemeral" => ReplayRetention::Ephemeral,
            _ => return Err(lettuce_conversations::ArtifactTransferError::Storage),
        },
    });
    if let TrustedArtifactDescriptor::Replay(reference) = &descriptor {
        reference
            .validate()
            .map_err(|_| lettuce_conversations::ArtifactTransferError::Storage)?;
    }
    sink.begin(&descriptor)?;
    for chunk in bytes.chunks(CHUNK_SIZE) {
        sink.chunk(chunk)?;
    }
    sink.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_conversations::{
        ConversationArtifactStore, ConversationArtifactTransferPort, ProtectedArtifactBytes,
        TrustedArtifactDescriptor,
    };
    use lettuce_types::{CharacterId, ConversationBranchId, ConversationId};

    fn replay_draft(artifact_id: ReplayArtifactId, payload: &[u8]) -> ReplayArtifactDraft {
        let bytes = ProtectedArtifactBytes::new(payload.to_vec()).expect("payload");
        ReplayArtifactDraft {
            artifact_id,
            digest: bytes.digest(),
            schema_version: 1,
            byte_size: u64::try_from(bytes.len()).expect("size"),
            codec: ArtifactCodec::Json,
            retention: ArtifactRetention::Conversation,
            bytes,
        }
    }

    fn snapshot_draft(artifact_id: SnapshotArtifactId, payload: &[u8]) -> SnapshotArtifactDraft {
        let bytes = ProtectedArtifactBytes::new(payload.to_vec()).expect("payload");
        SnapshotArtifactDraft {
            source: SnapshotSource::Character(CharacterId::new()),
            source_revision: Revision::INITIAL,
            artifact_id,
            digest: bytes.digest(),
            schema_version: 1,
            byte_size: u64::try_from(bytes.len()).expect("size"),
            codec: ArtifactCodec::Json,
            retention: ArtifactRetention::Conversation,
            bytes,
        }
    }

    #[derive(Default)]
    struct CollectingSink {
        descriptor: Option<TrustedArtifactDescriptor>,
        bytes: Vec<u8>,
        finished: bool,
    }

    impl TrustedArtifactSink for CollectingSink {
        fn begin(
            &mut self,
            descriptor: &TrustedArtifactDescriptor,
        ) -> Result<(), lettuce_conversations::ArtifactTransferError> {
            self.descriptor = Some(descriptor.clone());
            Ok(())
        }

        fn chunk(
            &mut self,
            bytes: &[u8],
        ) -> Result<(), lettuce_conversations::ArtifactTransferError> {
            self.bytes.extend_from_slice(bytes);
            Ok(())
        }

        fn finish(&mut self) -> Result<(), lettuce_conversations::ArtifactTransferError> {
            self.finished = true;
            Ok(())
        }
    }

    struct RejectingSink;

    impl TrustedArtifactSink for RejectingSink {
        fn begin(
            &mut self,
            _: &TrustedArtifactDescriptor,
        ) -> Result<(), lettuce_conversations::ArtifactTransferError> {
            Ok(())
        }
        fn chunk(&mut self, _: &[u8]) -> Result<(), lettuce_conversations::ArtifactTransferError> {
            Err(lettuce_conversations::ArtifactTransferError::SinkRejected)
        }
        fn finish(&mut self) -> Result<(), lettuce_conversations::ArtifactTransferError> {
            Ok(())
        }
    }

    #[test]
    fn replay_roundtrip_deduplicates_and_rejects_immutable_rewrites() {
        let database = Database::open_in_memory().expect("database");
        let id = ReplayArtifactId::new();
        let first = database
            .put_replay(replay_draft(id, b"replay"))
            .expect("insert");
        assert_eq!(
            database
                .put_replay(replay_draft(id, b"replay"))
                .expect("dedupe"),
            first
        );
        assert!(matches!(
            database.put_replay(replay_draft(id, b"changed")),
            Err(ArtifactError::ImmutableConflict)
        ));
        database.verify_replay(&first).expect("verify");
    }

    #[test]
    fn snapshot_roundtrip_deduplicates_and_rejects_immutable_rewrites() {
        let database = Database::open_in_memory().expect("database");
        let id = SnapshotArtifactId::new();
        let source = SnapshotSource::Character(CharacterId::new());
        let make = |payload: &[u8]| {
            let bytes = ProtectedArtifactBytes::new(payload.to_vec()).expect("payload");
            SnapshotArtifactDraft {
                source,
                source_revision: Revision::INITIAL,
                artifact_id: id,
                digest: bytes.digest(),
                schema_version: 1,
                byte_size: u64::try_from(bytes.len()).expect("size"),
                codec: ArtifactCodec::Json,
                retention: ArtifactRetention::Conversation,
                bytes,
            }
        };
        let first = database.put_snapshot(make(b"snapshot")).expect("insert");
        assert_eq!(
            database.put_snapshot(make(b"snapshot")).expect("dedupe"),
            first
        );
        assert!(matches!(
            database.put_snapshot(make(b"changed")),
            Err(ArtifactError::ImmutableConflict)
        ));
        database.verify_snapshot(&first).expect("verify");
    }

    #[test]
    fn corrupt_bytes_fail_closed_and_orphan_cleanup_is_idempotent() {
        let database = Database::open_in_memory().expect("database");
        let id = ReplayArtifactId::new();
        let reference = database
            .put_replay(replay_draft(id, b"payload"))
            .expect("insert");
        let connection = database.connection().expect("lock");
        connection
            .pragma_update(None, "ignore_check_constraints", true)
            .expect("ignore checks");
        connection
            .execute(
                "UPDATE conversation_replay_artifacts SET bytes = X'00' WHERE artifact_id = ?1",
                [id.to_string()],
            )
            .expect("corrupt");
        connection
            .pragma_update(None, "ignore_check_constraints", false)
            .expect("restore checks");
        drop(connection);
        assert!(matches!(
            database.verify_replay(&reference),
            Err(ArtifactError::DigestMismatch | ArtifactError::SizeMismatch)
        ));
        database.cleanup_orphan_replay(id).expect("cleanup");
        database
            .cleanup_orphan_replay(id)
            .expect("idempotent cleanup");
        assert_eq!(
            database.verify_replay(&reference),
            Err(ArtifactError::NotFound)
        );
    }

    #[test]
    fn referenced_snapshot_is_never_orphan_cleaned() {
        let database = Database::open_in_memory().expect("database");
        let id = SnapshotArtifactId::new();
        let draft = snapshot_draft(id, b"snapshot");
        let reference = database.put_snapshot(draft).expect("insert");
        let conversation_id = ConversationId::new();
        let branch_id = ConversationBranchId::new();
        let mut connection = database.connection().expect("lock");
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("transaction");
        tx.execute("INSERT INTO conversations (id, kind, lifecycle, title, active_branch_id, kind_json, revision, created_at, updated_at) VALUES (?1, 'direct', 'active', 'test', ?2, '{\"format_version\":1,\"value\":null}', 1, 0, 0)", params![conversation_id.to_string(), branch_id.to_string()]).expect("conversation");
        tx.execute("INSERT INTO conversation_branches (conversation_id, id, status, revision, created_at, updated_at) VALUES (?1, ?2, 'active', 1, 0, 0)", params![conversation_id.to_string(), branch_id.to_string()]).expect("branch");
        tx.execute(
            "INSERT INTO conversation_snapshot_refs (conversation_id, artifact_id) VALUES (?1, ?2)",
            params![conversation_id.to_string(), id.to_string()],
        )
        .expect("reference");
        tx.commit().expect("commit");
        drop(connection);
        database
            .cleanup_orphan_snapshot(id)
            .expect("guarded cleanup");
        database
            .verify_snapshot(&reference)
            .expect("still referenced");
    }

    #[test]
    fn trusted_transfer_verifies_before_sink_and_streams_chunks() {
        let database = Database::open_in_memory().expect("database");
        let id = ReplayArtifactId::new();
        let reference = database
            .put_replay(replay_draft(id, b"transfer payload"))
            .expect("insert");
        let mut sink = CollectingSink::default();
        database.export_replay(id, &mut sink).expect("export");
        assert_eq!(sink.bytes, b"transfer payload");
        assert!(sink.finished);
        assert_eq!(
            sink.descriptor,
            Some(TrustedArtifactDescriptor::Replay(reference))
        );
        let mut rejecting = RejectingSink;
        assert_eq!(
            database.export_replay(id, &mut rejecting),
            Err(lettuce_conversations::ArtifactTransferError::SinkRejected)
        );
    }

    #[test]
    fn two_handles_same_artifact_put_is_a_safe_dedupe() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        let path = std::env::temp_dir().join(format!(
            "lettuce-m8-artifact-race-{}.db",
            ReplayArtifactId::new()
        ));
        let setup = Database::open(&path).expect("setup");
        drop(setup);
        let first = Database::open(&path).expect("first handle");
        let second = Database::open(&path).expect("second handle");
        let artifact_id = ReplayArtifactId::new();
        let barrier = Arc::new(Barrier::new(2));
        let first_barrier = Arc::clone(&barrier);
        let first_thread = thread::spawn(move || {
            first_barrier.wait();
            first.put_replay(replay_draft(artifact_id, b"same payload"))
        });
        let second_barrier = Arc::clone(&barrier);
        let second_thread = thread::spawn(move || {
            second_barrier.wait();
            second.put_replay(replay_draft(artifact_id, b"same payload"))
        });
        let first_result = first_thread.join().expect("first put");
        let second_result = second_thread.join().expect("second put");
        assert!(first_result.is_ok(), "first result: {first_result:?}");
        assert!(second_result.is_ok(), "second result: {second_result:?}");
        assert_eq!(first_result, second_result);
        let _ = std::fs::remove_file(path);
    }
}
