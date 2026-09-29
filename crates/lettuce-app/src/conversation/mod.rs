//! Conversation management use cases that turn ids into the snapshot values
//! the conversation commands need: settings changes and group membership.

mod participants;
mod settings;

pub(crate) use participants::conversation_model;
pub use participants::{
    ParticipantChange, add_group_member, ensure_group_members, member_operation,
    member_participant_id, update_group_participant,
};
pub use settings::{
    BackgroundChange, Change, Choice, ConversationSettingsChange, apply_settings_change,
    prepare_scene_selection,
};

use lettuce_conversations::{ConversationRepositoryError, IdempotencyKey, OperationToken};
use lettuce_types::{ContentHash, ConversationId, SnapshotArtifactId};

/// Why a conversation edit was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConversationEditError {
    #[error("{field} does not name a usable source")]
    InvalidInput { field: &'static str },
    #[error(transparent)]
    Conversation(#[from] ConversationRepositoryError),
    #[error("a source could not be read")]
    Source,
    #[error("a snapshot could not be built")]
    Snapshot,
}

/// An artifact id derived from the conversation and what it snapshots, so a
/// retried edit stages the same artifact.
pub(crate) fn snapshot_artifact_id(
    conversation_id: ConversationId,
    name: &str,
) -> SnapshotArtifactId {
    SnapshotArtifactId::from_uuid(uuid::Uuid::new_v5(
        &conversation_id.as_uuid(),
        name.as_bytes(),
    ))
}

/// An operation token under `key` whose digest covers `parts`.
pub fn edit_operation(
    key: String,
    parts: &[&[u8]],
) -> Result<OperationToken, ConversationEditError> {
    let mut hasher = blake3::Hasher::new();
    for part in parts {
        hasher.update(&(part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    Ok(OperationToken {
        key: IdempotencyKey::new(key).map_err(|_| ConversationEditError::InvalidInput {
            field: "client_operation_id",
        })?,
        request_digest: ContentHash::parse(hasher.finalize().to_hex().as_str())
            .map_err(|_| ConversationEditError::Snapshot)?,
    })
}

pub(crate) use participants::ensure_group_members_at;
