//! Normalized M8 create/read vertical slice.
//!
//! The mutation surface is deliberately not exposed as `ConversationRepository`
//! until the remaining lifecycle methods are implemented. This slice proves
//! the relational ownership contract and strict read path without pretending
//! to provide the complete repository.

#![allow(dead_code)]

use std::str::FromStr;

use lettuce_conversations::{
    Conversation, ConversationAggregate, ConversationBranch, ConversationKind,
    ConversationLifecycle, ConversationParticipant, ConversationRepositoryError,
    CurrentConversationSettings, ParticipantRole, ParticipantSource, SettingProvenance,
    SnapshotSelection, SnapshotSource,
};
use lettuce_types::{ConversationId, Revision, TimestampMillis};
use rusqlite::{OptionalExtension, Row, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::Database;

#[cfg(test)]
use lettuce_conversations::{ConversationParticipantDraft, CreateConversationPlan};
#[cfg(test)]
use lettuce_types::ConversationBranchId;

const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document<T> {
    format_version: u32,
    value: T,
}

pub(crate) fn encode<T: Serialize>(value: &T) -> Result<String, ConversationRepositoryError> {
    serde_json::to_string(&Document {
        format_version: FORMAT_VERSION,
        value,
    })
    .map_err(|_| ConversationRepositoryError::Storage)
}

pub(crate) fn decode<T: DeserializeOwned>(value: &str) -> Result<T, ConversationRepositoryError> {
    let document: Document<T> =
        serde_json::from_str(value).map_err(|_| ConversationRepositoryError::Storage)?;
    if document.format_version != FORMAT_VERSION {
        return Err(ConversationRepositoryError::Storage);
    }
    Ok(document.value)
}

pub(crate) fn db(error: rusqlite::Error) -> ConversationRepositoryError {
    match error {
        rusqlite::Error::InvalidQuery => ConversationRepositoryError::Storage,
        rusqlite::Error::SqliteFailure(code, _) if code.extended_code == 787 => {
            ConversationRepositoryError::Storage
        }
        _ => ConversationRepositoryError::Storage,
    }
}

pub(crate) fn parse_id<T: FromStr>(value: String) -> Result<T, ConversationRepositoryError> {
    value
        .parse()
        .map_err(|_| ConversationRepositoryError::Storage)
}
pub(crate) fn rev(value: i64) -> Result<Revision, ConversationRepositoryError> {
    u64::try_from(value)
        .map(Revision::new)
        .map_err(|_| ConversationRepositoryError::Storage)
}

fn sql_revision(value: Revision) -> Result<i64, ConversationRepositoryError> {
    i64::try_from(value.get()).map_err(|_| ConversationRepositoryError::Storage)
}

fn provenance_name(value: SettingProvenance) -> &'static str {
    match value {
        SettingProvenance::LaunchInherited => "launch_inherited",
        SettingProvenance::CurrentOverride => "current_override",
        SettingProvenance::Disabled => "disabled",
    }
}

fn provenance_from_name(value: &str) -> Result<SettingProvenance, rusqlite::Error> {
    match value {
        "launch_inherited" => Ok(SettingProvenance::LaunchInherited),
        "current_override" => Ok(SettingProvenance::CurrentOverride),
        "disabled" => Ok(SettingProvenance::Disabled),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn snapshot_source_columns(source: SnapshotSource) -> (&'static str, String) {
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

fn source_columns(source: ParticipantSource) -> (&'static str, Option<String>) {
    match source {
        ParticipantSource::User => ("user", None),
        ParticipantSource::Character(id) => ("character", Some(id.to_string())),
        ParticipantSource::System => ("system", None),
    }
}
fn source_from_columns(
    kind: &str,
    id: Option<String>,
) -> Result<ParticipantSource, ConversationRepositoryError> {
    match (kind, id) {
        ("user", None) => Ok(ParticipantSource::User),
        ("system", None) => Ok(ParticipantSource::System),
        ("character", Some(id)) => parse_id(id).map(ParticipantSource::Character),
        _ => Err(ConversationRepositoryError::Storage),
    }
}
fn role_name(role: ParticipantRole) -> &'static str {
    match role {
        ParticipantRole::User => "user",
        ParticipantRole::Character => "character",
        ParticipantRole::System => "system",
    }
}
fn role_from_name(value: &str) -> Result<ParticipantRole, ConversationRepositoryError> {
    match value {
        "user" => Ok(ParticipantRole::User),
        "character" => Ok(ParticipantRole::Character),
        "system" => Ok(ParticipantRole::System),
        _ => Err(ConversationRepositoryError::Storage),
    }
}
fn lifecycle_name(value: ConversationLifecycle) -> &'static str {
    match value {
        ConversationLifecycle::Active => "active",
        ConversationLifecycle::Archived => "archived",
        ConversationLifecycle::Tombstoned => "tombstoned",
    }
}
fn lifecycle_from_name(value: &str) -> Result<ConversationLifecycle, ConversationRepositoryError> {
    match value {
        "active" => Ok(ConversationLifecycle::Active),
        "archived" => Ok(ConversationLifecycle::Archived),
        "tombstoned" => Ok(ConversationLifecycle::Tombstoned),
        _ => Err(ConversationRepositoryError::Storage),
    }
}

fn save_participants(
    transaction: &Transaction<'_>,
    conversation: &Conversation,
) -> Result<(), ConversationRepositoryError> {
    for participant in &conversation.participants {
        let (source_kind, source_id) = source_columns(participant.source);
        let revision = sql_revision(participant.revision)?;
        transaction.execute("INSERT INTO conversation_participants (conversation_id, id, role, ordinal, source_kind, source_id, enabled, muted, display_name, authored_description, model_selection_json, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)", params![conversation.id.to_string(), participant.id.to_string(), role_name(participant.role), i64::from(participant.ordinal), source_kind, source_id, participant.enabled as i64, participant.muted as i64, participant.display_name, participant.authored_description, encode(&participant.model_selection)?, revision, participant.created_at.get(), participant.updated_at.get()]).map_err(db)?;
    }
    Ok(())
}

fn save_settings(
    transaction: &Transaction<'_>,
    conversation: &Conversation,
) -> Result<(), ConversationRepositoryError> {
    let Some(settings) = &conversation.current_settings else {
        return Ok(());
    };
    let revision = sql_revision(settings.revision)?;
    transaction.execute("INSERT INTO conversation_settings (conversation_id, revision, author_note, author_note_provenance, memory_json, memory_provenance, model_override_json, model_provenance, voice_json, voice_provenance, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)", params![conversation.id.to_string(), revision, settings.author_note, provenance_name(settings.author_note_provenance), settings.memory.as_ref().map(encode).transpose()?, provenance_name(settings.memory_provenance), settings.model_override.as_ref().map(encode).transpose()?, provenance_name(settings.model_provenance), settings.voice.as_ref().map(encode).transpose()?, provenance_name(settings.voice_provenance), conversation.created_at.get(), conversation.updated_at.get()]).map_err(db)?;
    Ok(())
}

fn save_conversation(
    transaction: &Transaction<'_>,
    conversation: &Conversation,
) -> Result<(), ConversationRepositoryError> {
    let revision = sql_revision(conversation.revision)?;
    transaction.execute("INSERT INTO conversations (id, kind, lifecycle, title, active_branch_id, kind_json, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)", params![conversation.id.to_string(), if conversation.kind.is_group() { "group" } else { "direct" }, lifecycle_name(conversation.lifecycle), conversation.title, conversation.active_branch_id.to_string(), encode(&conversation.kind)?, revision, conversation.created_at.get(), conversation.updated_at.get()]).map_err(db)?;
    save_participants(transaction, conversation)?;
    save_settings(transaction, conversation)
}

fn save_branch(
    transaction: &Transaction<'_>,
    branch: &ConversationBranch,
) -> Result<(), ConversationRepositoryError> {
    let revision = sql_revision(branch.revision)?;
    transaction.execute("INSERT INTO conversation_branches (conversation_id, id, parent_branch_id, fork_message_id, head_message_id, status, revision, created_at, updated_at) VALUES (?1, ?2, NULL, NULL, NULL, 'active', ?3, ?4, ?5)", params![branch.conversation_id.to_string(), branch.id.to_string(), revision, branch.created_at.get(), branch.updated_at.get()]).map_err(db)?;
    Ok(())
}

impl Database {
    /// Fixture-only helper for the M8 vertical-slice tests. It intentionally
    /// omits initial timeline persistence and must not become the production
    /// conversation creator; the atomic `ConversationCreator` adapter will
    /// replace it in the next slice.
    #[cfg(test)]
    pub(crate) fn create_conversation_record(
        &self,
        plan: &CreateConversationPlan,
        now: TimestampMillis,
    ) -> Result<ConversationAggregate, ConversationRepositoryError> {
        plan.validate()
            .map_err(ConversationRepositoryError::Invalid)?;
        let mut connection = self
            .connection()
            .map_err(|_| ConversationRepositoryError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db)?;
        if transaction
            .query_row(
                "SELECT 1 FROM conversations WHERE id = ?1",
                [plan.conversation_id.to_string()],
                |_| Ok(()),
            )
            .optional()
            .map_err(db)?
            .is_some()
        {
            return Err(ConversationRepositoryError::Conflict);
        }
        let root = ConversationBranch {
            id: ConversationBranchId::new(),
            conversation_id: plan.conversation_id,
            parent_branch_id: None,
            fork_message_id: None,
            head_message_id: None,
            status: lettuce_conversations::BranchStatus::Active,
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        };
        let participants = plan
            .participants
            .iter()
            .map(|p: &ConversationParticipantDraft| ConversationParticipant {
                id: p.id,
                role: p.role,
                ordinal: p.ordinal,
                enabled: p.enabled,
                muted: p.muted,
                source: p.source,
                display_name: p.display_name.clone(),
                authored_description: p.authored_description.clone(),
                model_selection: p.model_selection.clone(),
                revision: Revision::INITIAL,
                created_at: now,
                updated_at: now,
            })
            .collect();
        let conversation = Conversation {
            id: plan.conversation_id,
            lifecycle: ConversationLifecycle::Active,
            title: plan.title.clone(),
            kind: plan.kind.clone(),
            active_branch_id: root.id,
            participants,
            current_settings: None,
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        };
        let aggregate = ConversationAggregate {
            conversation: conversation.clone(),
            branches: vec![root.clone()],
        };
        aggregate
            .validate()
            .map_err(ConversationRepositoryError::Invalid)?;
        for reference in lettuce_conversations::conversation_snapshot_references(&plan.kind) {
            super::conversation_artifact_adapter::verify_snapshot_in_transaction(
                &transaction,
                reference,
            )
            .map_err(ConversationRepositoryError::ArtifactReference)?;
        }
        save_conversation(&transaction, &conversation)?;
        save_branch(&transaction, &root)?;
        let mut stored_artifacts = std::collections::BTreeSet::new();
        for reference in lettuce_conversations::conversation_snapshot_references(&plan.kind) {
            if !stored_artifacts.insert(reference.artifact_id.to_string()) {
                continue;
            }
            transaction
                .execute(
                    "INSERT INTO conversation_snapshot_refs (conversation_id, artifact_id) VALUES (?1, ?2)",
                    params![
                        conversation.id.to_string(),
                        reference.artifact_id.to_string(),
                    ],
                )
                .map_err(db)?;
        }
        transaction.commit().map_err(db)?;
        Ok(aggregate)
    }

    /// Reads all normalized aggregate rows and fails closed on any malformed
    /// or cross-owner value.
    pub(crate) fn get_conversation_record(
        &self,
        id: ConversationId,
    ) -> Result<ConversationAggregate, ConversationRepositoryError> {
        let mut connection = self
            .connection()
            .map_err(|_| ConversationRepositoryError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(db)?;
        let aggregate = hydrate_conversation(&transaction, id, || {})?;
        transaction.commit().map_err(db)?;
        Ok(aggregate)
    }

    /// Test-only entry point for proving that the aggregate hydrator keeps the
    /// read transaction's snapshot while another connection commits a related
    /// aggregate update.
    #[cfg(test)]
    pub(crate) fn get_conversation_record_with_snapshot_hook<F>(
        &self,
        id: ConversationId,
        after_snapshot: F,
    ) -> Result<ConversationAggregate, ConversationRepositoryError>
    where
        F: FnOnce(),
    {
        let mut connection = self
            .connection()
            .map_err(|_| ConversationRepositoryError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(db)?;
        let aggregate = hydrate_conversation(&transaction, id, after_snapshot)?;
        transaction.commit().map_err(db)?;
        Ok(aggregate)
    }
}

pub(crate) fn hydrate_conversation<F>(
    transaction: &Transaction<'_>,
    id: ConversationId,
    after_snapshot: F,
) -> Result<ConversationAggregate, ConversationRepositoryError>
where
    F: FnOnce(),
{
    let row = transaction
        .query_row("SELECT lifecycle, title, active_branch_id, kind_json, revision, created_at, updated_at, kind FROM conversations WHERE id = ?1", [id.to_string()], |row| read_conversation_row(row, id))
        .optional()
        .map_err(db)?
        .ok_or(ConversationRepositoryError::NotFound)?;
    let mut expected_artifacts: Vec<String> =
        lettuce_conversations::conversation_snapshot_references(&row.kind)
            .into_iter()
            .map(|reference| reference.artifact_id.to_string())
            .collect();
    expected_artifacts.sort();
    expected_artifacts.dedup();
    let actual_artifacts: Vec<String> = transaction
        .prepare("SELECT artifact_id FROM conversation_snapshot_refs WHERE conversation_id = ?1 ORDER BY artifact_id")
        .map_err(db)?
        .query_map([id.to_string()], |child| child.get(0))
        .map_err(db)?
        .collect::<rusqlite::Result<Vec<String>>>()
        .map_err(db)?;
    if actual_artifacts != expected_artifacts {
        return Err(ConversationRepositoryError::Storage);
    }
    for reference in lettuce_conversations::conversation_snapshot_references(&row.kind) {
        super::conversation_artifact_adapter::verify_snapshot_in_transaction(
            transaction,
            reference,
        )
        .map_err(|_| ConversationRepositoryError::Storage)?;
    }

    // The root row and exact reference set have been acquired on this read
    // transaction.  The callback exists only for the two-handle coherence
    // test; production reads pass a no-op.
    after_snapshot();

    let mut participants = Vec::new();
    let mut statement = transaction.prepare("SELECT id, role, ordinal, source_kind, source_id, enabled, muted, display_name, authored_description, model_selection_json, revision, created_at, updated_at FROM conversation_participants WHERE conversation_id = ?1 ORDER BY ordinal, id").map_err(db)?;
    for row in statement
        .query_map([id.to_string()], read_participant)
        .map_err(db)?
    {
        participants.push(row.map_err(db)?);
    }
    drop(statement);
    for participant in &participants {
        if participant.role == ParticipantRole::Character {
            if let SnapshotSelection::Inherited(model) | SnapshotSelection::Explicit(model) =
                &participant.model_selection
            {
                super::conversation_artifact_adapter::verify_snapshot_in_transaction(
                    transaction,
                    &model.snapshot_ref,
                )
                .map_err(|_| ConversationRepositoryError::Storage)?;
            }
        }
    }
    let settings = transaction
        .query_row("SELECT revision, author_note, author_note_provenance, memory_json, memory_provenance, model_override_json, model_provenance, voice_json, voice_provenance FROM conversation_settings WHERE conversation_id = ?1", [id.to_string()], read_settings)
        .optional()
        .map_err(db)?;
    let mut conversation = row;
    conversation.participants = participants;
    conversation.current_settings = settings;
    let mut branches = Vec::new();
    let mut statement = transaction.prepare("SELECT id, parent_branch_id, fork_message_id, head_message_id, status, revision, created_at, updated_at FROM conversation_branches WHERE conversation_id = ?1 ORDER BY created_at, id").map_err(db)?;
    for row in statement
        .query_map([id.to_string()], |row| read_branch(row, id))
        .map_err(db)?
    {
        branches.push(row.map_err(db)?);
    }
    let aggregate = ConversationAggregate {
        conversation,
        branches,
    };
    aggregate
        .validate()
        .map_err(|_| ConversationRepositoryError::Storage)?;
    drop(statement);
    Ok(aggregate)
}

pub(crate) fn read_conversation_row(
    row: &Row<'_>,
    id: ConversationId,
) -> Result<Conversation, rusqlite::Error> {
    let lifecycle: String = row.get(0)?;
    let kind_json: String = row.get(3)?;
    let kind: ConversationKind = decode(&kind_json).map_err(|_| rusqlite::Error::InvalidQuery)?;
    let kind_name: String = row.get(7)?;
    if (kind_name == "group") != kind.is_group() {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(Conversation {
        id,
        lifecycle: lifecycle_from_name(&lifecycle).map_err(|_| rusqlite::Error::InvalidQuery)?,
        title: row.get(1)?,
        active_branch_id: parse_id(row.get(2)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        participants: Vec::new(),
        current_settings: None,
        kind,
        revision: rev(row.get(4)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        created_at: TimestampMillis::new(row.get(5)?),
        updated_at: TimestampMillis::new(row.get(6)?),
    })
}

pub(crate) fn read_participant(row: &Row<'_>) -> Result<ConversationParticipant, rusqlite::Error> {
    Ok(ConversationParticipant {
        id: parse_id(row.get(0)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        role: role_from_name(&row.get::<_, String>(1)?)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        ordinal: row
            .get::<_, i64>(2)?
            .try_into()
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        source: source_from_columns(&row.get::<_, String>(3)?, row.get(4)?)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        enabled: row.get::<_, i64>(5)? != 0,
        muted: row.get::<_, i64>(6)? != 0,
        display_name: row.get(7)?,
        authored_description: row.get(8)?,
        model_selection: decode(&row.get::<_, String>(9)?)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        revision: rev(row.get(10)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        created_at: TimestampMillis::new(row.get(11)?),
        updated_at: TimestampMillis::new(row.get(12)?),
    })
}

pub(crate) fn read_settings(row: &Row<'_>) -> Result<CurrentConversationSettings, rusqlite::Error> {
    Ok(CurrentConversationSettings {
        revision: rev(row.get(0)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        author_note: row.get(1)?,
        author_note_provenance: provenance_from_name(&row.get::<_, String>(2)?)?,
        memory: row
            .get::<_, Option<String>>(3)?
            .map(|v| decode(&v))
            .transpose()
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        memory_provenance: provenance_from_name(&row.get::<_, String>(4)?)?,
        model_override: row
            .get::<_, Option<String>>(5)?
            .map(|v| decode(&v))
            .transpose()
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        model_provenance: provenance_from_name(&row.get::<_, String>(6)?)?,
        voice: row
            .get::<_, Option<String>>(7)?
            .map(|v| decode(&v))
            .transpose()
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        voice_provenance: provenance_from_name(&row.get::<_, String>(8)?)?,
    })
}

pub(crate) fn read_branch(
    row: &Row<'_>,
    conversation_id: ConversationId,
) -> Result<ConversationBranch, rusqlite::Error> {
    Ok(ConversationBranch {
        id: parse_id(row.get(0)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        conversation_id,
        parent_branch_id: row
            .get::<_, Option<String>>(1)?
            .map(parse_id)
            .transpose()
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        fork_message_id: row
            .get::<_, Option<String>>(2)?
            .map(parse_id)
            .transpose()
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        head_message_id: row
            .get::<_, Option<String>>(3)?
            .map(parse_id)
            .transpose()
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        status: match row.get::<_, String>(4)?.as_str() {
            "active" => lettuce_conversations::BranchStatus::Active,
            "archived" => lettuce_conversations::BranchStatus::Archived,
            "tombstoned" => lettuce_conversations::BranchStatus::Tombstoned,
            _ => return Err(rusqlite::Error::InvalidQuery),
        },
        revision: rev(row.get(5)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        created_at: TimestampMillis::new(row.get(6)?),
        updated_at: TimestampMillis::new(row.get(7)?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_conversations::{
        ArtifactCodec, ArtifactRetention, CharacterLaunchSnapshot, ConversationArtifactStore,
        ConversationParticipantDraft, DirectConversationDetails, GroupChatModeSnapshot,
        GroupConversationDetails, GroupLaunchSnapshot, GroupMemberLaunchSnapshot,
        GroupParticipantPolicyDocument, GroupParticipantPolicySnapshot,
        GroupSpeakerSelectionSnapshot, ModelProviderKind, ModelSelectionSnapshot, OperationToken,
        ProtectedArtifactBytes, ProtectedSnapshotRef, SnapshotArtifactDraft, SnapshotSelection,
    };
    use lettuce_types::{
        CharacterId, ContentHash, ConversationParticipantId, GenerationAttemptId, GenerationTurnId,
        MessageCandidateId, MessageId, SnapshotArtifactId,
    };

    fn put_test_snapshot(
        database: &Database,
        source: SnapshotSource,
        payload: &[u8],
    ) -> ProtectedSnapshotRef {
        let artifact_id = SnapshotArtifactId::new();
        let bytes = ProtectedArtifactBytes::new(payload.to_vec()).expect("bytes");
        let reference = ProtectedSnapshotRef {
            source,
            source_revision: Revision::INITIAL,
            artifact_id,
            digest: bytes.digest(),
            schema_version: 1,
            byte_size: u64::try_from(bytes.len()).expect("size"),
        };
        database
            .put_snapshot(SnapshotArtifactDraft {
                source,
                source_revision: Revision::INITIAL,
                artifact_id,
                digest: reference.digest.clone(),
                schema_version: 1,
                byte_size: reference.byte_size,
                codec: ArtifactCodec::Json,
                retention: ArtifactRetention::Conversation,
                bytes,
            })
            .expect("artifact");
        reference
    }

    struct TurnGraph {
        conversation_id: ConversationId,
        root_branch_id: ConversationBranchId,
        child_branch_id: ConversationBranchId,
        user_message_id: MessageId,
        root_head_message_id: MessageId,
        child_non_head_message_id: MessageId,
        child_head_message_id: MessageId,
        source_turn_id: GenerationTurnId,
        source_attempt_id: GenerationAttemptId,
        source_candidate_id: MessageCandidateId,
    }

    fn seed_turn_graph(database: &Database) -> TurnGraph {
        let graph = TurnGraph {
            conversation_id: ConversationId::new(),
            root_branch_id: ConversationBranchId::new(),
            child_branch_id: ConversationBranchId::new(),
            user_message_id: MessageId::new(),
            root_head_message_id: MessageId::new(),
            child_non_head_message_id: MessageId::new(),
            child_head_message_id: MessageId::new(),
            source_turn_id: GenerationTurnId::new(),
            source_attempt_id: GenerationAttemptId::new(),
            source_candidate_id: MessageCandidateId::new(),
        };
        let user_participant_id = ConversationParticipantId::new();
        let character_participant_id = ConversationParticipantId::new();
        let mut connection = database.connection().expect("connection");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("graph transaction");
        transaction
            .execute(
                "INSERT INTO conversations (id, kind, lifecycle, title, active_branch_id, kind_json, revision, created_at, updated_at) VALUES (?1, 'direct', 'active', 'raw', ?2, '{\"format_version\":1,\"value\":null}', 1, 0, 0)",
                params![graph.conversation_id.to_string(), graph.root_branch_id.to_string()],
            )
            .expect("conversation");
        transaction
            .execute(
                "INSERT INTO conversation_branches (conversation_id, id, status, revision, created_at, updated_at) VALUES (?1, ?2, 'active', 1, 0, 0)",
                params![graph.conversation_id.to_string(), graph.root_branch_id.to_string()],
            )
            .expect("root branch");
        for (id, role, source_kind, display_name) in [
            (user_participant_id, "user", "user", "User"),
            (
                character_participant_id,
                "character",
                "character",
                "Character",
            ),
        ] {
            let source_id = if role == "character" {
                Some("character-source")
            } else {
                None
            };
            transaction
                .execute(
                    "INSERT INTO conversation_participants (conversation_id, id, role, ordinal, source_kind, source_id, enabled, muted, display_name, model_selection_json, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, 0, ?7, '{\"format_version\":1,\"value\":null}', 1, 0, 0)",
                    params![
                        graph.conversation_id.to_string(),
                        id.to_string(),
                        role,
                        if role == "user" { 0 } else { 1 },
                        source_kind,
                        source_id,
                        display_name,
                    ],
                )
                .expect("participant");
        }
        let insert_message =
            |message_id: MessageId,
             branch_id: ConversationBranchId,
             revision_id: &str,
             ordinal: i64,
             role: &str,
             author_id: Option<ConversationParticipantId>| {
                transaction
                .execute(
                    "INSERT INTO conversation_message_revisions (conversation_id, id, message_id, branch_id, sequence, parts_json, authored_at) VALUES (?1, ?2, ?3, ?4, 1, '{\"format_version\":1,\"value\":null}', 0)",
                    params![
                        graph.conversation_id.to_string(),
                        revision_id,
                        message_id.to_string(),
                        branch_id.to_string(),
                    ],
                )
                .expect("revision");
                transaction
                .execute(
                    "INSERT INTO conversation_messages (conversation_id, id, branch_id, role, author_participant_id, timeline_ordinal, logical_time, effective_time, visibility, pinned, scene_edited, active_revision_id, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 0, 'visible', 0, 0, ?7, 1, 0, 0)",
                    params![
                        graph.conversation_id.to_string(),
                        message_id.to_string(),
                        branch_id.to_string(),
                        role,
                        author_id.map(|id| id.to_string()),
                        ordinal,
                        revision_id,
                    ],
                )
                .expect("message");
            };
        insert_message(
            graph.user_message_id,
            graph.root_branch_id,
            "revision-user",
            1,
            "user",
            Some(user_participant_id),
        );
        insert_message(
            graph.root_head_message_id,
            graph.root_branch_id,
            "revision-root-head",
            2,
            "assistant",
            Some(character_participant_id),
        );
        transaction
            .execute(
                "UPDATE conversation_branches SET head_message_id = ?1 WHERE conversation_id = ?2 AND id = ?3",
                params![
                    graph.root_head_message_id.to_string(),
                    graph.conversation_id.to_string(),
                    graph.root_branch_id.to_string(),
                ],
            )
            .expect("root head");
        transaction
            .execute(
                "INSERT INTO conversation_branches (conversation_id, id, parent_branch_id, fork_message_id, status, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 'active', 1, 1, 1)",
                params![
                    graph.conversation_id.to_string(),
                    graph.child_branch_id.to_string(),
                    graph.root_branch_id.to_string(),
                    graph.root_head_message_id.to_string(),
                ],
            )
            .expect("child branch");
        insert_message(
            graph.child_non_head_message_id,
            graph.child_branch_id,
            "revision-child-non-head",
            3,
            "assistant",
            Some(character_participant_id),
        );
        insert_message(
            graph.child_head_message_id,
            graph.child_branch_id,
            "revision-child-head",
            4,
            "assistant",
            Some(character_participant_id),
        );
        transaction
            .execute(
                "UPDATE conversation_branches SET head_message_id = ?1 WHERE conversation_id = ?2 AND id = ?3",
                params![
                    graph.child_head_message_id.to_string(),
                    graph.conversation_id.to_string(),
                    graph.child_branch_id.to_string(),
                ],
            )
            .expect("child head");
        transaction
            .execute(
                "INSERT INTO conversation_turns (conversation_id, id, branch_id, operation, input_kind, candidate_message_id, candidate_id, idempotency_key, status, target_kind, target_message_id, target_prior_candidate_id, revision, created_at, updated_at) VALUES (?1, ?2, ?3, 'regenerate', 'existing_candidate', ?4, ?5, 'source-turn', 'created', 'existing_candidate', ?4, ?5, 1, 0, 0)",
                params![
                    graph.conversation_id.to_string(),
                    graph.source_turn_id.to_string(),
                    graph.root_branch_id.to_string(),
                    graph.root_head_message_id.to_string(),
                    graph.source_candidate_id.to_string(),
                ],
            )
            .expect("source turn");
        transaction
            .execute(
                "INSERT INTO generation_attempts (conversation_id, turn_id, id, ordinal, status, job_idempotency_key) VALUES (?1, ?2, ?3, 0, 'created', ?4)",
                params![
                    graph.conversation_id.to_string(),
                    graph.source_turn_id.to_string(),
                    graph.source_attempt_id.to_string(),
                    format!("generation.{}.{}", graph.source_turn_id, graph.source_attempt_id),
                ],
            )
            .expect("source attempt");
        transaction
            .execute(
                "INSERT INTO conversation_message_candidates (conversation_id, id, message_id, branch_id, turn_id, attempt_id, ordinal, parts_json, model_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, '{\"format_version\":1,\"value\":null}', '{\"format_version\":1,\"value\":null}', 0)",
                params![
                    graph.conversation_id.to_string(),
                    graph.source_candidate_id.to_string(),
                    graph.root_head_message_id.to_string(),
                    graph.root_branch_id.to_string(),
                    graph.source_turn_id.to_string(),
                    graph.source_attempt_id.to_string(),
                ],
            )
            .expect("source candidate");
        transaction.commit().expect("graph commit");
        graph
    }

    fn create_direct_fixture(
        database: &Database,
    ) -> (
        ConversationId,
        ConversationParticipantId,
        ConversationBranchId,
    ) {
        let character_id = CharacterId::new();
        let snapshot = put_test_snapshot(
            database,
            SnapshotSource::Character(character_id),
            b"coherent character",
        );
        let plan = CreateConversationPlan {
            conversation_id: ConversationId::new(),
            title: "Coherent chat".into(),
            kind: ConversationKind::Direct(DirectConversationDetails {
                format_version: 1,
                character: CharacterLaunchSnapshot {
                    snapshot_ref: snapshot,
                    source_id: character_id,
                    source_revision: Revision::INITIAL,
                    name: "Ada".into(),
                    nickname: None,
                },
                persona: SnapshotSelection::Disabled,
                scene: SnapshotSelection::Disabled,
                starter: SnapshotSelection::Disabled,
                prompt: SnapshotSelection::Disabled,
                lorebooks: SnapshotSelection::Explicit(Vec::new()),
                model: SnapshotSelection::Disabled,
                memory: SnapshotSelection::Disabled,
                voice: SnapshotSelection::Disabled,
            }),
            participants: vec![
                ConversationParticipantDraft {
                    id: ConversationParticipantId::new(),
                    role: ParticipantRole::User,
                    ordinal: 0,
                    source: ParticipantSource::User,
                    enabled: true,
                    muted: false,
                    display_name: "User".into(),
                    authored_description: None,
                    model_selection: SnapshotSelection::Disabled,
                },
                ConversationParticipantDraft {
                    id: ConversationParticipantId::new(),
                    role: ParticipantRole::Character,
                    ordinal: 1,
                    source: ParticipantSource::Character(character_id),
                    enabled: true,
                    muted: false,
                    display_name: "Ada".into(),
                    authored_description: None,
                    model_selection: SnapshotSelection::Disabled,
                },
            ],
            initial_timeline: lettuce_conversations::InitialTimelineDraft {
                format_version: 1,
                entries: Vec::new(),
            },
            operation: OperationToken {
                key: lettuce_jobs::IdempotencyKey::new("coherent-chat").expect("key"),
                request_digest: ContentHash::parse("ef".repeat(32)).expect("digest"),
            },
        };
        let aggregate = database
            .create_conversation_record(&plan, TimestampMillis::UNIX_EPOCH)
            .expect("create");
        (
            plan.conversation_id,
            aggregate.conversation.participants[0].id,
            aggregate.conversation.active_branch_id,
        )
    }

    #[test]
    fn create_inserts_exact_snapshot_refs_and_cleanup_preserves_dependency() {
        let database = Database::open_in_memory().expect("database");
        let character_id = CharacterId::new();
        let artifact_id = SnapshotArtifactId::new();
        let bytes = ProtectedArtifactBytes::new(b"character snapshot".to_vec()).expect("bytes");
        let snapshot = ProtectedSnapshotRef {
            source: SnapshotSource::Character(character_id),
            source_revision: Revision::INITIAL,
            artifact_id,
            digest: bytes.digest(),
            schema_version: 1,
            byte_size: u64::try_from(bytes.len()).expect("size"),
        };
        database
            .put_snapshot(SnapshotArtifactDraft {
                source: snapshot.source,
                source_revision: snapshot.source_revision,
                artifact_id,
                digest: snapshot.digest.clone(),
                schema_version: snapshot.schema_version,
                byte_size: snapshot.byte_size,
                codec: ArtifactCodec::Json,
                retention: ArtifactRetention::Conversation,
                bytes,
            })
            .expect("artifact");
        let plan = CreateConversationPlan {
            conversation_id: ConversationId::new(),
            title: "Chat".into(),
            kind: ConversationKind::Direct(DirectConversationDetails {
                format_version: 1,
                character: CharacterLaunchSnapshot {
                    snapshot_ref: snapshot.clone(),
                    source_id: character_id,
                    source_revision: Revision::INITIAL,
                    name: "Ada".into(),
                    nickname: None,
                },
                persona: SnapshotSelection::Disabled,
                scene: SnapshotSelection::Disabled,
                starter: SnapshotSelection::Disabled,
                prompt: SnapshotSelection::Disabled,
                lorebooks: SnapshotSelection::Explicit(Vec::new()),
                model: SnapshotSelection::Disabled,
                memory: SnapshotSelection::Disabled,
                voice: SnapshotSelection::Disabled,
            }),
            participants: vec![
                ConversationParticipantDraft {
                    id: ConversationParticipantId::new(),
                    role: ParticipantRole::User,
                    ordinal: 0,
                    source: ParticipantSource::User,
                    enabled: true,
                    muted: false,
                    display_name: "User".into(),
                    authored_description: None,
                    model_selection: SnapshotSelection::Disabled,
                },
                ConversationParticipantDraft {
                    id: ConversationParticipantId::new(),
                    role: ParticipantRole::Character,
                    ordinal: 1,
                    source: ParticipantSource::Character(character_id),
                    enabled: true,
                    muted: false,
                    display_name: "Ada".into(),
                    authored_description: None,
                    model_selection: SnapshotSelection::Disabled,
                },
            ],
            initial_timeline: lettuce_conversations::InitialTimelineDraft {
                format_version: 1,
                entries: Vec::new(),
            },
            operation: OperationToken {
                key: lettuce_jobs::IdempotencyKey::new("create-chat").expect("key"),
                request_digest: ContentHash::parse("ab".repeat(32)).expect("digest"),
            },
        };
        let aggregate = database
            .create_conversation_record(&plan, TimestampMillis::UNIX_EPOCH)
            .expect("create");
        assert_eq!(aggregate.conversation.participants.len(), 2);
        let count: i64 = database
            .connection()
            .expect("lock")
            .query_row(
                "SELECT count(*) FROM conversation_snapshot_refs WHERE conversation_id = ?1",
                [plan.conversation_id.to_string()],
                |row| row.get(0),
            )
            .expect("reference count");
        assert_eq!(count, 1);
        database
            .cleanup_orphan_snapshot(artifact_id)
            .expect("referenced cleanup");
        database.verify_snapshot(&snapshot).expect("retained");
        assert_eq!(
            database
                .get_conversation_record(plan.conversation_id)
                .expect("read")
                .conversation
                .active_branch_id,
            aggregate.conversation.active_branch_id
        );
        let other_character_id = CharacterId::new();
        database
            .connection()
            .expect("lock")
            .execute(
                "UPDATE conversation_snapshot_artifacts SET source_id = ?1 WHERE artifact_id = ?2",
                params![other_character_id.to_string(), artifact_id.to_string()],
            )
            .expect("tamper source");
        assert_eq!(
            database.get_conversation_record(plan.conversation_id),
            Err(ConversationRepositoryError::Storage)
        );
        database
            .connection()
            .expect("lock")
            .execute(
                "UPDATE conversation_snapshot_artifacts SET source_id = ?1, bytes = ?2 WHERE artifact_id = ?3",
                params![character_id.to_string(), b"character tampered".to_vec(), artifact_id.to_string()],
            )
            .expect("tamper bytes");
        assert_eq!(
            database.get_conversation_record(plan.conversation_id),
            Err(ConversationRepositoryError::Storage)
        );
        database
            .connection()
            .expect("lock")
            .execute(
                "UPDATE conversation_snapshot_artifacts SET bytes = ?1, digest = ?2 WHERE artifact_id = ?3",
                params![b"character snapshot".to_vec(), "00".repeat(32), artifact_id.to_string()],
            )
            .expect("tamper digest");
        assert_eq!(
            database.get_conversation_record(plan.conversation_id),
            Err(ConversationRepositoryError::Storage)
        );
        database
            .connection()
            .expect("lock")
            .execute(
                "UPDATE conversation_snapshot_artifacts SET digest = ?1 WHERE artifact_id = ?2",
                params![snapshot.digest.as_str(), artifact_id.to_string()],
            )
            .expect("restore digest");
        let second_id = SnapshotArtifactId::new();
        let second_bytes = ProtectedArtifactBytes::new(b"other snapshot".to_vec()).expect("bytes");
        database
            .put_snapshot(SnapshotArtifactDraft {
                source: snapshot.source,
                source_revision: snapshot.source_revision,
                artifact_id: second_id,
                digest: second_bytes.digest(),
                schema_version: 1,
                byte_size: u64::try_from(second_bytes.len()).expect("size"),
                codec: ArtifactCodec::Json,
                retention: ArtifactRetention::Conversation,
                bytes: second_bytes,
            })
            .expect("second artifact");
        database
            .connection()
            .expect("lock")
            .execute(
                "UPDATE conversation_snapshot_refs SET artifact_id = ?1 WHERE conversation_id = ?2",
                params![second_id.to_string(), plan.conversation_id.to_string()],
            )
            .expect("corrupt set");
        assert_eq!(
            database.get_conversation_record(plan.conversation_id),
            Err(ConversationRepositoryError::Storage)
        );
    }

    #[test]
    fn turn_inputs_reject_wrong_role_cross_branch_and_non_head_sources() {
        let database = Database::open_in_memory().expect("database");
        let graph = seed_turn_graph(&database);
        let connection = database.connection().expect("connection");
        let insert = |id: &str,
                      branch_id: ConversationBranchId,
                      operation: &str,
                      input_kind: &str,
                      user_message_id: Option<MessageId>,
                      head_message_id: Option<MessageId>,
                      candidate_message_id: Option<MessageId>,
                      candidate_id: Option<MessageCandidateId>,
                      key: &str| {
            connection.execute(
                "INSERT INTO conversation_turns (conversation_id, id, branch_id, operation, input_kind, user_message_id, head_message_id, candidate_message_id, candidate_id, idempotency_key, status, target_kind, target_message_id, target_parent_message_id, target_prior_candidate_id, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'created', CASE WHEN ?4 = 'regenerate' THEN 'existing_candidate' ELSE 'new_assistant' END, CASE WHEN ?4 = 'regenerate' THEN ?8 ELSE 'target-' || ?2 END, coalesce(?6, ?7), CASE WHEN ?4 = 'regenerate' THEN ?9 ELSE NULL END, 1, 0, 0)",
                params![
                    graph.conversation_id.to_string(), id, branch_id.to_string(), operation,
                    input_kind, user_message_id.map(|value| value.to_string()),
                    head_message_id.map(|value| value.to_string()),
                    candidate_message_id.map(|value| value.to_string()),
                    candidate_id.map(|value| value.to_string()), key,
                ],
            )
        };

        assert!(
            insert(
                "wrong-role",
                graph.root_branch_id,
                "send",
                "user_message",
                Some(graph.root_head_message_id),
                None,
                None,
                None,
                "wrong-role-key",
            )
            .is_err()
        );
        assert!(
            insert(
                "cross-branch-user",
                graph.child_branch_id,
                "send",
                "user_message",
                Some(graph.user_message_id),
                None,
                None,
                None,
                "cross-branch-user-key",
            )
            .is_err()
        );
        assert!(
            insert(
                "cross-branch-head",
                graph.child_branch_id,
                "continue",
                "existing_head",
                None,
                Some(graph.root_head_message_id),
                None,
                None,
                "cross-branch-head-key",
            )
            .is_err()
        );
        assert!(
            insert(
                "wrong-non-head",
                graph.child_branch_id,
                "continue",
                "existing_head",
                None,
                Some(graph.child_non_head_message_id),
                None,
                None,
                "wrong-non-head-key",
            )
            .is_err()
        );
        assert!(
            insert(
                "cross-branch-candidate",
                graph.child_branch_id,
                "regenerate",
                "existing_candidate",
                None,
                None,
                Some(graph.root_head_message_id),
                Some(graph.source_candidate_id),
                "cross-branch-candidate-key",
            )
            .is_err()
        );
        insert(
            "valid-regenerate",
            graph.root_branch_id,
            "regenerate",
            "existing_candidate",
            None,
            None,
            Some(graph.root_head_message_id),
            Some(graph.source_candidate_id),
            "valid-regenerate-key",
        )
        .expect("existing candidate target is valid");
        connection
            .execute(
                "INSERT INTO generation_attempts (conversation_id, turn_id, id, ordinal, status, job_idempotency_key) VALUES (?1, 'valid-regenerate', 'valid-regenerate-attempt', 0, 'created', 'generation.valid-regenerate.valid-regenerate-attempt')",
                [graph.conversation_id.to_string()],
            )
            .expect("regenerate attempt");
        connection
            .execute(
                "INSERT INTO conversation_message_candidates (conversation_id, id, message_id, branch_id, turn_id, attempt_id, ordinal, parts_json, model_json, created_at) VALUES (?1, 'valid-regenerate-candidate', ?2, ?3, 'valid-regenerate', 'valid-regenerate-attempt', 1, '{\"format_version\":1,\"value\":null}', '{\"format_version\":1,\"value\":null}', 0)",
                params![
                    graph.conversation_id.to_string(),
                    graph.root_head_message_id.to_string(),
                    graph.root_branch_id.to_string(),
                ],
            )
            .expect("regenerate candidate target is valid");
        connection
            .execute(
                "UPDATE conversation_turns SET status = 'preparing' WHERE conversation_id = ?1 AND id = ?2",
                params![graph.conversation_id.to_string(), graph.source_turn_id.to_string()],
            )
            .expect("retry source preparing");
        connection
            .execute(
                "UPDATE conversation_turns SET status = 'context_prepared' WHERE conversation_id = ?1 AND id = ?2",
                params![graph.conversation_id.to_string(), graph.source_turn_id.to_string()],
            )
            .expect("retry source context prepared");
        connection
            .execute(
                "UPDATE conversation_turns SET status = 'running' WHERE conversation_id = ?1 AND id = ?2",
                params![graph.conversation_id.to_string(), graph.source_turn_id.to_string()],
            )
            .expect("retry source running");
        connection
            .execute(
                "UPDATE conversation_turns SET status = 'finalizing' WHERE conversation_id = ?1 AND id = ?2",
                params![graph.conversation_id.to_string(), graph.source_turn_id.to_string()],
            )
            .expect("retry source finalizing");
        connection
            .execute(
                "INSERT INTO conversation_usage_refs (conversation_id, turn_id, attempt_id, usage_event_id, outcome, created_at) VALUES (?1, ?2, ?3, 'retry-usage', 'failed', 0)",
                params![
                    graph.conversation_id.to_string(),
                    graph.source_turn_id.to_string(),
                    graph.source_attempt_id.to_string(),
                ],
            )
            .expect("retry usage ref");
        connection
            .execute(
                "UPDATE generation_attempts SET status = 'failed', usage_event_id = 'retry-usage', usage_outcome = 'failed', failure = 'internal', started_at = 1, finished_at = 1 WHERE conversation_id = ?1 AND turn_id = ?2",
                params![graph.conversation_id.to_string(), graph.source_turn_id.to_string()],
            )
            .expect("settle retry source attempt");
        connection
            .execute(
                "UPDATE conversation_turns SET status = 'failed', failure = 'internal' WHERE conversation_id = ?1 AND id = ?2",
                params![graph.conversation_id.to_string(), graph.source_turn_id.to_string()],
            )
            .expect("terminal retry source");
        connection
            .execute(
                "INSERT INTO conversation_turns (conversation_id, id, branch_id, operation, input_kind, candidate_message_id, candidate_id, idempotency_key, status, target_kind, target_message_id, target_prior_candidate_id, retry_of_turn_id, revision, created_at, updated_at) VALUES (?1, 'retry-child', ?2, 'regenerate', 'existing_candidate', ?3, ?4, 'retry-child-key', 'created', 'existing_candidate', ?3, ?4, ?5, 1, 0, 0)",
                params![
                    graph.conversation_id.to_string(),
                    graph.root_branch_id.to_string(),
                    graph.root_head_message_id.to_string(),
                    graph.source_candidate_id.to_string(),
                    graph.source_turn_id.to_string(),
                ],
            )
            .expect("matching retry child");
        assert!(
            connection
                .execute(
                    "DELETE FROM conversation_turns WHERE conversation_id = ?1 AND id = ?2",
                    params![
                        graph.conversation_id.to_string(),
                        graph.source_turn_id.to_string()
                    ],
                )
                .is_err()
        );
        insert(
            "valid-continue",
            graph.child_branch_id,
            "continue",
            "existing_head",
            None,
            Some(graph.child_head_message_id),
            None,
            None,
            "valid-continue-key",
        )
        .expect("current branch head is valid");
        assert!(connection
            .execute(
                "INSERT INTO conversation_turns (conversation_id, id, branch_id, operation, input_kind, user_message_id, idempotency_key, status, target_kind, target_message_id, target_parent_message_id, prompt_revision, revision, created_at, updated_at) VALUES (?1, 'prompt-pair', ?2, 'send', 'user_message', ?3, 'prompt-pair-key', 'created', 'new_assistant', 'prompt-target', ?3, 9, 1, 1, 0, 0)",
                params![
                    graph.conversation_id.to_string(),
                    graph.root_branch_id.to_string(),
                    graph.user_message_id.to_string(),
                ],
            )
            .is_err());
        assert!(connection
            .execute(
                "INSERT INTO conversation_turns (conversation_id, id, branch_id, operation, input_kind, user_message_id, idempotency_key, status, target_kind, target_message_id, target_parent_message_id, selected_speaker_participant_id, selected_speaker_details_json, revision, created_at, updated_at) VALUES (?1, 'direct-speaker', ?2, 'send', 'user_message', ?3, 'direct-speaker-key', 'created', 'new_assistant', 'speaker-target', ?3, 'missing-speaker', '{\"format_version\":1}', 1, 0, 0)",
                params![
                    graph.conversation_id.to_string(),
                    graph.root_branch_id.to_string(),
                    graph.user_message_id.to_string(),
                ],
            )
            .is_err());
    }

    #[test]
    fn terminal_turns_require_settled_attempts_on_insert_and_update() {
        let database = Database::open_in_memory().expect("database");
        let graph = seed_turn_graph(&database);
        let connection = database.connection().expect("connection");
        assert!(connection
            .execute(
                "INSERT INTO conversation_turns (conversation_id, id, branch_id, operation, input_kind, user_message_id, idempotency_key, status, target_kind, target_message_id, target_parent_message_id, failure, revision, created_at, updated_at) VALUES (?1, 'terminal-insert', ?2, 'send', 'user_message', ?3, 'terminal-insert-key', 'failed', 'new_assistant', 'terminal-target', ?3, 'internal', 1, 0, 0)",
                params![
                    graph.conversation_id.to_string(),
                    graph.root_branch_id.to_string(),
                    graph.user_message_id.to_string(),
                ],
            )
            .is_err());
        connection
            .execute(
                "INSERT INTO conversation_turns (conversation_id, id, branch_id, operation, input_kind, user_message_id, idempotency_key, status, target_kind, target_message_id, target_parent_message_id, revision, created_at, updated_at) VALUES (?1, 'terminal-update', ?2, 'send', 'user_message', ?3, 'terminal-update-key', 'created', 'new_assistant', 'terminal-update-target', ?3, 1, 0, 0)",
                params![
                    graph.conversation_id.to_string(),
                    graph.root_branch_id.to_string(),
                    graph.user_message_id.to_string(),
                ],
            )
            .expect("turn");
        connection
            .execute(
                "INSERT INTO generation_attempts (conversation_id, turn_id, id, ordinal, status, job_idempotency_key, started_at) VALUES (?1, 'terminal-update', 'live-attempt', 0, 'running', 'generation.terminal-update.live-attempt', 1)",
                [graph.conversation_id.to_string()],
            )
            .expect("live attempt");
        for status in ["preparing", "context_prepared", "running", "finalizing"] {
            connection
                .execute(
                    "UPDATE conversation_turns SET status = ?1 WHERE conversation_id = ?2 AND id = 'terminal-update'",
                    params![status, graph.conversation_id.to_string()],
                )
                .expect("turn transition");
        }
        assert!(connection
            .execute(
                "UPDATE conversation_turns SET status = 'failed', failure = 'internal' WHERE conversation_id = ?1 AND id = 'terminal-update'",
                [graph.conversation_id.to_string()],
            )
            .is_err());
    }

    #[test]
    fn group_create_and_get_round_trip_uses_group_snapshot_refs() {
        let database = Database::open_in_memory().expect("database");
        let group_id = lettuce_types::GroupId::new();
        let character_id = lettuce_types::CharacterId::new();
        let second_character_id = lettuce_types::CharacterId::new();
        let now = TimestampMillis::UNIX_EPOCH;
        let put = |source: SnapshotSource| {
            let artifact_id = SnapshotArtifactId::new();
            let bytes = ProtectedArtifactBytes::new(b"group snapshot".to_vec()).expect("bytes");
            let reference = ProtectedSnapshotRef {
                source,
                source_revision: Revision::INITIAL,
                artifact_id,
                digest: bytes.digest(),
                schema_version: 1,
                byte_size: u64::try_from(bytes.len()).expect("size"),
            };
            database
                .put_snapshot(SnapshotArtifactDraft {
                    source,
                    source_revision: Revision::INITIAL,
                    artifact_id,
                    digest: reference.digest.clone(),
                    schema_version: 1,
                    byte_size: reference.byte_size,
                    codec: ArtifactCodec::Json,
                    retention: ArtifactRetention::Conversation,
                    bytes,
                })
                .expect("artifact");
            reference
        };
        let group_ref = put(SnapshotSource::Group(group_id));
        let character_ref = put(SnapshotSource::Character(character_id));
        let second_character_ref = put(SnapshotSource::Character(second_character_id));
        let model_id = lettuce_types::ModelProfileId::new();
        let model_ref = put(SnapshotSource::Model(model_id));
        let model = ModelSelectionSnapshot {
            snapshot_ref: model_ref,
            source_id: model_id,
            source_revision: Revision::INITIAL,
            provider_kind: ModelProviderKind::Other,
            external_model_id: "model".into(),
            display_name: "Model".into(),
            context_length: None,
            max_output_tokens: None,
        };
        let participant_id = ConversationParticipantId::new();
        let second_participant_id = ConversationParticipantId::new();
        let plan = CreateConversationPlan {
            conversation_id: ConversationId::new(),
            title: "Group chat".into(),
            kind: ConversationKind::Group(GroupConversationDetails {
                format_version: 1,
                group: GroupLaunchSnapshot {
                    snapshot_ref: group_ref,
                    source_id: group_id,
                    source_revision: Revision::INITIAL,
                    name: "Group".into(),
                    members: vec![
                        GroupMemberLaunchSnapshot {
                            character: CharacterLaunchSnapshot {
                                snapshot_ref: character_ref,
                                source_id: character_id,
                                source_revision: Revision::INITIAL,
                                name: "Ada".into(),
                                nickname: None,
                            },
                            ordinal: 0,
                            enabled: true,
                            muted: false,
                            model_override: SnapshotSelection::Explicit(model.clone()),
                            lorebooks: SnapshotSelection::Explicit(Vec::new()),
                        },
                        GroupMemberLaunchSnapshot {
                            character: CharacterLaunchSnapshot {
                                snapshot_ref: second_character_ref,
                                source_id: second_character_id,
                                source_revision: Revision::INITIAL,
                                name: "Bea".into(),
                                nickname: None,
                            },
                            ordinal: 1,
                            enabled: true,
                            muted: false,
                            model_override: SnapshotSelection::Disabled,
                            lorebooks: SnapshotSelection::Explicit(Vec::new()),
                        },
                    ],
                    chat_mode: GroupChatModeSnapshot::Conversation,
                    speaker_selection: GroupSpeakerSelectionSnapshot::RoundRobin,
                    memory: SnapshotSelection::Disabled,
                    disable_character_lorebook: false,
                    persona: SnapshotSelection::Disabled,
                    scene: SnapshotSelection::Disabled,
                    prompt: SnapshotSelection::Disabled,
                    lorebooks: SnapshotSelection::Explicit(Vec::new()),
                    model: SnapshotSelection::Explicit(model.clone()),
                },
                initial_participant_policy: GroupParticipantPolicyDocument {
                    members: vec![
                        GroupParticipantPolicySnapshot {
                            participant_id,
                            enabled: true,
                            muted: false,
                            model_override: SnapshotSelection::Explicit(model.clone()),
                        },
                        GroupParticipantPolicySnapshot {
                            participant_id: second_participant_id,
                            enabled: true,
                            muted: false,
                            model_override: SnapshotSelection::Disabled,
                        },
                    ],
                    revision: Revision::INITIAL,
                    created_at: now,
                    updated_at: now,
                },
            }),
            participants: vec![
                ConversationParticipantDraft {
                    id: ConversationParticipantId::new(),
                    role: ParticipantRole::User,
                    ordinal: 0,
                    source: ParticipantSource::User,
                    enabled: true,
                    muted: false,
                    display_name: "User".into(),
                    authored_description: None,
                    model_selection: SnapshotSelection::Disabled,
                },
                ConversationParticipantDraft {
                    id: participant_id,
                    role: ParticipantRole::Character,
                    ordinal: 1,
                    source: ParticipantSource::Character(character_id),
                    enabled: true,
                    muted: false,
                    display_name: "Ada".into(),
                    authored_description: None,
                    model_selection: SnapshotSelection::Explicit(model.clone()),
                },
                ConversationParticipantDraft {
                    id: second_participant_id,
                    role: ParticipantRole::Character,
                    ordinal: 2,
                    source: ParticipantSource::Character(second_character_id),
                    enabled: true,
                    muted: false,
                    display_name: "Bea".into(),
                    authored_description: None,
                    model_selection: SnapshotSelection::Disabled,
                },
            ],
            initial_timeline: lettuce_conversations::InitialTimelineDraft {
                format_version: 1,
                entries: Vec::new(),
            },
            operation: OperationToken {
                key: lettuce_jobs::IdempotencyKey::new("create-group").expect("key"),
                request_digest: ContentHash::parse("cd".repeat(32)).expect("digest"),
            },
        };
        let created = database
            .create_conversation_record(&plan, now)
            .expect("create group");
        let loaded = database
            .get_conversation_record(plan.conversation_id)
            .expect("get group");
        assert!(loaded.conversation.kind.is_group());
        assert_eq!(loaded, created);
        let reference_count: i64 = database
            .connection()
            .expect("lock")
            .query_row(
                "SELECT count(*) FROM conversation_snapshot_refs WHERE conversation_id = ?1",
                [plan.conversation_id.to_string()],
                |row| row.get(0),
            )
            .expect("reference count");
        assert_eq!(reference_count, 4);
        for reference in lettuce_conversations::conversation_snapshot_references(&plan.kind) {
            database
                .verify_snapshot(reference)
                .expect("reference verifies");
        }
    }

    #[test]
    fn aggregate_hydration_verifies_current_models_and_rejects_invalid_non_character_models() {
        let database = Database::open_in_memory().expect("database");
        let group_id = lettuce_types::GroupId::new();
        let character_id = lettuce_types::CharacterId::new();
        let second_character_id = lettuce_types::CharacterId::new();
        let model_id = lettuce_types::ModelProfileId::new();
        let put = |source: SnapshotSource| {
            put_test_snapshot(&database, source, b"aggregate model snapshot")
        };
        let group_ref = put(SnapshotSource::Group(group_id));
        let character_ref = put(SnapshotSource::Character(character_id));
        let second_character_ref = put(SnapshotSource::Character(second_character_id));
        let model_ref = put(SnapshotSource::Model(model_id));
        let model = ModelSelectionSnapshot {
            snapshot_ref: model_ref.clone(),
            source_id: model_id,
            source_revision: Revision::INITIAL,
            provider_kind: ModelProviderKind::Other,
            external_model_id: "model".into(),
            display_name: "Model".into(),
            context_length: None,
            max_output_tokens: None,
        };
        let participant_id = ConversationParticipantId::new();
        let second_participant_id = ConversationParticipantId::new();
        let plan = CreateConversationPlan {
            conversation_id: ConversationId::new(),
            title: "Aggregate model drift".into(),
            kind: ConversationKind::Group(GroupConversationDetails {
                format_version: 1,
                group: GroupLaunchSnapshot {
                    snapshot_ref: group_ref,
                    source_id: group_id,
                    source_revision: Revision::INITIAL,
                    name: "Group".into(),
                    members: vec![
                        GroupMemberLaunchSnapshot {
                            character: CharacterLaunchSnapshot {
                                snapshot_ref: character_ref,
                                source_id: character_id,
                                source_revision: Revision::INITIAL,
                                name: "Ada".into(),
                                nickname: None,
                            },
                            ordinal: 0,
                            enabled: true,
                            muted: false,
                            model_override: SnapshotSelection::Explicit(model.clone()),
                            lorebooks: SnapshotSelection::Explicit(Vec::new()),
                        },
                        GroupMemberLaunchSnapshot {
                            character: CharacterLaunchSnapshot {
                                snapshot_ref: second_character_ref,
                                source_id: second_character_id,
                                source_revision: Revision::INITIAL,
                                name: "Bea".into(),
                                nickname: None,
                            },
                            ordinal: 1,
                            enabled: true,
                            muted: false,
                            model_override: SnapshotSelection::Disabled,
                            lorebooks: SnapshotSelection::Explicit(Vec::new()),
                        },
                    ],
                    chat_mode: GroupChatModeSnapshot::Conversation,
                    speaker_selection: GroupSpeakerSelectionSnapshot::RoundRobin,
                    memory: SnapshotSelection::Disabled,
                    disable_character_lorebook: false,
                    persona: SnapshotSelection::Disabled,
                    scene: SnapshotSelection::Disabled,
                    prompt: SnapshotSelection::Disabled,
                    lorebooks: SnapshotSelection::Explicit(Vec::new()),
                    model: SnapshotSelection::Explicit(model.clone()),
                },
                initial_participant_policy: GroupParticipantPolicyDocument {
                    members: vec![
                        GroupParticipantPolicySnapshot {
                            participant_id,
                            enabled: true,
                            muted: false,
                            model_override: SnapshotSelection::Explicit(model.clone()),
                        },
                        GroupParticipantPolicySnapshot {
                            participant_id: second_participant_id,
                            enabled: true,
                            muted: false,
                            model_override: SnapshotSelection::Disabled,
                        },
                    ],
                    revision: Revision::INITIAL,
                    created_at: TimestampMillis::UNIX_EPOCH,
                    updated_at: TimestampMillis::UNIX_EPOCH,
                },
            }),
            participants: vec![
                ConversationParticipantDraft {
                    id: ConversationParticipantId::new(),
                    role: ParticipantRole::User,
                    ordinal: 0,
                    source: ParticipantSource::User,
                    enabled: true,
                    muted: false,
                    display_name: "User".into(),
                    authored_description: None,
                    model_selection: SnapshotSelection::Disabled,
                },
                ConversationParticipantDraft {
                    id: participant_id,
                    role: ParticipantRole::Character,
                    ordinal: 1,
                    source: ParticipantSource::Character(character_id),
                    enabled: true,
                    muted: false,
                    display_name: "Ada".into(),
                    authored_description: None,
                    model_selection: SnapshotSelection::Explicit(model.clone()),
                },
                ConversationParticipantDraft {
                    id: second_participant_id,
                    role: ParticipantRole::Character,
                    ordinal: 2,
                    source: ParticipantSource::Character(second_character_id),
                    enabled: true,
                    muted: false,
                    display_name: "Bea".into(),
                    authored_description: None,
                    model_selection: SnapshotSelection::Disabled,
                },
            ],
            initial_timeline: lettuce_conversations::InitialTimelineDraft {
                format_version: 1,
                entries: Vec::new(),
            },
            operation: OperationToken {
                key: lettuce_jobs::IdempotencyKey::new("aggregate-model-drift").expect("key"),
                request_digest: ContentHash::parse("ab".repeat(32)).expect("digest"),
            },
        };
        database
            .create_conversation_record(&plan, TimestampMillis::UNIX_EPOCH)
            .expect("create");
        database
            .verify_snapshot(&model_ref)
            .expect("model artifact exists");

        let override_model_id = lettuce_types::ModelProfileId::new();
        let override_model_ref = put(SnapshotSource::Model(override_model_id));
        let override_model = ModelSelectionSnapshot {
            snapshot_ref: override_model_ref.clone(),
            source_id: override_model_id,
            source_revision: Revision::INITIAL,
            provider_kind: ModelProviderKind::Other,
            external_model_id: "override-model".into(),
            display_name: "Override model".into(),
            context_length: None,
            max_output_tokens: None,
        };
        database
            .connection()
            .expect("lock")
            .execute(
                "UPDATE conversation_participants SET model_selection_json = ?1 WHERE conversation_id = ?2 AND id = ?3",
                params![encode(&SnapshotSelection::Explicit(override_model.clone())).expect("override model"), plan.conversation_id.to_string(), participant_id.to_string()],
            )
            .expect("set current character override");
        database
            .get_conversation_record(plan.conversation_id)
            .expect("valid current character override hydrates");

        let user_id = plan.participants[0].id;
        database
            .connection()
            .expect("lock")
            .execute(
                "UPDATE conversation_participants SET model_selection_json = ?1 WHERE conversation_id = ?2 AND id = ?3",
                params![encode(&SnapshotSelection::Explicit(override_model.clone())).expect("override model"), plan.conversation_id.to_string(), user_id.to_string()],
            )
            .expect("tamper non-character model");
        assert_eq!(
            database.get_conversation_record(plan.conversation_id),
            Err(ConversationRepositoryError::Storage)
        );

        let disabled: SnapshotSelection<ModelSelectionSnapshot> = SnapshotSelection::Disabled;
        let disabled_json = encode(&disabled).expect("disabled model");
        database
            .connection()
            .expect("lock")
            .execute(
                "UPDATE conversation_participants SET model_selection_json = ?1 WHERE conversation_id = ?2 AND id = ?3",
                params![disabled_json, plan.conversation_id.to_string(), user_id.to_string()],
            )
            .expect("restore non-character model");

        let missing_model_id = lettuce_types::ModelProfileId::new();
        let missing_model = ModelSelectionSnapshot {
            snapshot_ref: ProtectedSnapshotRef {
                source: SnapshotSource::Model(missing_model_id),
                source_revision: Revision::INITIAL,
                artifact_id: SnapshotArtifactId::new(),
                digest: model_ref.digest.clone(),
                schema_version: model_ref.schema_version,
                byte_size: model_ref.byte_size,
            },
            source_id: missing_model_id,
            source_revision: Revision::INITIAL,
            provider_kind: ModelProviderKind::Other,
            external_model_id: "missing-model".into(),
            display_name: "Missing model".into(),
            context_length: None,
            max_output_tokens: None,
        };
        database
            .connection()
            .expect("lock")
            .execute(
                "UPDATE conversation_participants SET model_selection_json = ?1 WHERE conversation_id = ?2 AND id = ?3",
                params![
                    encode(&SnapshotSelection::Explicit(missing_model)).expect("missing model"),
                    plan.conversation_id.to_string(),
                    participant_id.to_string()
                ],
            )
            .expect("tamper current model reference");
        assert_eq!(
            database.get_conversation_record(plan.conversation_id),
            Err(ConversationRepositoryError::Storage)
        );
        database
            .verify_snapshot(&model_ref)
            .expect("artifact verification remains independent");
        database
            .verify_snapshot(&override_model_ref)
            .expect("current override artifact remains available");
    }

    #[test]
    fn two_file_handles_keep_aggregate_reads_coherent_across_commit() {
        let path =
            std::env::temp_dir().join(format!("lettuce-m8-coherent-{}.db", ConversationId::new()));
        let reader = Database::open(&path).expect("reader database");
        let writer = Database::open(&path).expect("writer database");
        let (conversation_id, participant_id, branch_id) = create_direct_fixture(&reader);

        let old = reader
            .get_conversation_record_with_snapshot_hook(conversation_id, || {
                let mut connection = writer.connection().expect("writer connection");
                let transaction = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .expect("writer transaction");
                transaction
                    .execute(
                        "UPDATE conversation_participants SET display_name = 'Writer User', revision = 2, updated_at = 1 WHERE conversation_id = ?1 AND id = ?2",
                        params![conversation_id.to_string(), participant_id.to_string()],
                    )
                    .expect("participant update");
                transaction
                    .execute(
                        "UPDATE conversation_branches SET revision = 2, updated_at = 1 WHERE conversation_id = ?1 AND id = ?2",
                        params![conversation_id.to_string(), branch_id.to_string()],
                    )
                    .expect("branch update");
                transaction
                    .execute(
                        "UPDATE conversations SET revision = 2, updated_at = 1 WHERE id = ?1",
                        [conversation_id.to_string()],
                    )
                    .expect("root update");
                transaction.commit().expect("writer commit");
            })
            .expect("old coherent read");
        assert_eq!(old.conversation.participants[0].display_name, "User");
        assert_eq!(old.conversation.revision, Revision::INITIAL);
        assert_eq!(old.branches[0].revision, Revision::INITIAL);

        let new = reader
            .get_conversation_record(conversation_id)
            .expect("new read");
        assert_eq!(new.conversation.participants[0].display_name, "Writer User");
        assert_eq!(new.conversation.revision, Revision::new(2));
        assert_eq!(new.branches[0].revision, Revision::new(2));

        drop(writer);
        drop(reader);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(format!("{}-wal", path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", path.display()));
    }

    #[test]
    fn m8_schema_exposes_typed_constraints_and_recovery_triggers() {
        let database = Database::open_in_memory().expect("database");
        let mut connection = database.connection().expect("lock");
        let branch_sql: String = connection
            .query_row(
                "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = 'conversation_branches'",
                [],
                |row| row.get(0),
            )
            .expect("branch schema");
        assert!(branch_sql.contains("parent_branch_id IS NULL) = (fork_message_id IS NULL)"));
        let message_sql: String = connection
            .query_row(
                "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = 'conversation_messages'",
                [],
                |row| row.get(0),
            )
            .expect("message schema");
        assert!(message_sql.contains("active_revision_id"));
        assert!(message_sql.contains("author_participant_id IS NULL"));
        for trigger in [
            "generation_attempt_parent_ordinal",
            "generation_checkpoint_contiguous",
            "conversation_turn_final_insert_contract",
            "conversation_turn_final_update_contract",
            "conversation_branch_head_same_branch_insert",
            "conversation_branch_fork_message_parent_insert",
            "conversation_message_parent_topology_insert",
        ] {
            let present: i64 = connection
                .query_row(
                    "SELECT count(*) FROM sqlite_schema WHERE type = 'trigger' AND name = ?1",
                    [trigger],
                    |row| row.get(0),
                )
                .expect("trigger lookup");
            assert_eq!(present, 1, "missing trigger {trigger}");
        }
        for index in [
            "conversation_turns_id_idx",
            "conversation_message_revisions_id_idx",
            "conversation_message_candidates_id_idx",
        ] {
            let present: i64 = connection
                .query_row(
                    "SELECT count(*) FROM sqlite_schema WHERE type = 'index' AND name = ?1",
                    [index],
                    |row| row.get(0),
                )
                .expect("index lookup");
            assert_eq!(present, 1, "missing index {index}");
        }
        let fk_count: i64 = connection
            .query_row(
                "SELECT count(*) FROM pragma_foreign_key_list('conversation_messages')",
                [],
                |row| row.get(0),
            )
            .expect("message fks");
        assert!(fk_count >= 5);
        let origin_sql: String = connection
            .query_row(
                "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = 'conversation_initial_message_origins'",
                [],
                |row| row.get(0),
            )
            .expect("initial origin schema");
        assert!(origin_sql.contains("source_kind IN ('scene', 'starter')"));
        assert!(origin_sql.contains("starter_message_id IS NULL"));
        assert!(origin_sql.contains("conversation_snapshot_artifacts"));
        assert!(origin_sql.contains("conversation_snapshot_refs"));
        let origin_fks: i64 = connection
            .query_row(
                "SELECT count(*) FROM pragma_foreign_key_list('conversation_initial_message_origins')",
                [],
                |row| row.get(0),
            )
            .expect("initial origin fks");
        assert!(origin_fks >= 3);

        let conversation_id = ConversationId::new();
        let branch_id = ConversationBranchId::new();
        let setup_transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("raw setup transaction");
        setup_transaction
            .execute(
                "INSERT INTO conversations (id, kind, lifecycle, title, active_branch_id, kind_json, revision, created_at, updated_at) VALUES (?1, 'direct', 'active', 'raw', ?2, '{\"format_version\":1,\"value\":null}', 1, 0, 0)",
                params![conversation_id.to_string(), branch_id.to_string()],
            )
            .expect("raw conversation");
        setup_transaction
            .execute(
                "INSERT INTO conversation_branches (conversation_id, id, status, revision, created_at, updated_at) VALUES (?1, ?2, 'active', 1, 0, 0)",
                params![conversation_id.to_string(), branch_id.to_string()],
            )
            .expect("raw branch");
        setup_transaction.commit().expect("raw setup commit");
        assert!(connection
            .execute(
                "INSERT INTO conversation_participants (conversation_id, id, role, ordinal, source_kind, source_id, enabled, muted, display_name, model_selection_json, revision, created_at, updated_at) VALUES (?1, 'participant', 'user', 0, 'character', 'forged', 1, 0, 'User', '{\"format_version\":1,\"value\":null}', 1, 0, 0)",
                [conversation_id.to_string()],
            )
            .is_err());
        assert!(connection
            .execute(
                "INSERT INTO conversation_settings (conversation_id, revision, author_note, author_note_provenance, memory_provenance, model_provenance, voice_provenance, created_at, updated_at) VALUES (?1, 1, 'forged', 'disabled', 'disabled', 'disabled', 'disabled', 0, 0)",
                [conversation_id.to_string()],
            )
            .is_err());
        assert!(connection
            .execute(
                "INSERT INTO conversation_settings (conversation_id, revision, author_note_provenance, memory_provenance, model_provenance, voice_provenance, created_at, updated_at) VALUES (?1, 1, 'current_override', 'launch_inherited', 'launch_inherited', 'launch_inherited', 0, 0)",
                [conversation_id.to_string()],
            )
            .is_err());
        assert!(connection
            .execute(
                "INSERT INTO conversation_settings (conversation_id, revision, author_note, author_note_provenance, memory_provenance, model_provenance, voice_provenance, created_at, updated_at) VALUES (?1, 1, 'forged', 'launch_inherited', 'launch_inherited', 'launch_inherited', 'launch_inherited', 0, 0)",
                [conversation_id.to_string()],
            )
            .is_err());
        assert!(connection
            .execute(
                "INSERT INTO conversation_snapshot_refs (conversation_id, artifact_id) VALUES (?1, 'missing')",
                [conversation_id.to_string()],
            )
            .is_err());
        let graph_transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("raw graph transaction");
        graph_transaction
            .execute(
                "INSERT INTO conversation_participants (conversation_id, id, role, ordinal, source_kind, enabled, muted, display_name, model_selection_json, revision, created_at, updated_at) VALUES (?1, 'user-participant', 'user', 0, 'user', 1, 0, 'User', '{\"format_version\":1,\"value\":null}', 1, 0, 0)",
                [conversation_id.to_string()],
            )
            .expect("raw user participant");
        graph_transaction
            .execute(
                "INSERT INTO conversation_message_revisions (conversation_id, id, message_id, branch_id, sequence, parts_json, authored_at) VALUES (?1, 'revision', 'message', ?2, 1, '{\"format_version\":1,\"value\":null}', 0)",
                params![conversation_id.to_string(), branch_id.to_string()],
            )
            .expect("raw revision");
        graph_transaction
            .execute(
                "INSERT INTO conversation_messages (conversation_id, id, branch_id, role, author_participant_id, timeline_ordinal, logical_time, effective_time, visibility, pinned, scene_edited, active_revision_id, revision, created_at, updated_at) VALUES (?1, 'message', ?2, 'user', 'user-participant', 1, 0, 0, 'visible', 0, 0, 'revision', 1, 0, 0)",
                params![conversation_id.to_string(), branch_id.to_string()],
            )
            .expect("raw message");
        graph_transaction.commit().expect("raw graph commit");
        let other_transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("other message transaction");
        other_transaction
            .execute(
                "INSERT INTO conversation_message_revisions (conversation_id, id, message_id, branch_id, sequence, parts_json, authored_at) VALUES (?1, 'other-revision', 'other-message', ?2, 1, '{\"format_version\":1,\"value\":null}', 0)",
                params![conversation_id.to_string(), branch_id.to_string()],
            )
            .expect("raw other revision");
        other_transaction
            .execute(
                "INSERT INTO conversation_messages (conversation_id, id, branch_id, role, timeline_ordinal, logical_time, effective_time, visibility, pinned, scene_edited, active_revision_id, revision, created_at, updated_at) VALUES (?1, 'other-message', ?2, 'system', 2, 0, 0, 'visible', 0, 0, 'other-revision', 1, 0, 0)",
                params![conversation_id.to_string(), branch_id.to_string()],
            )
            .expect("raw other message");
        other_transaction.commit().expect("other message commit");
        let fresh_branch_id = ConversationBranchId::new();
        let child_branch_id = ConversationBranchId::new();
        connection
            .execute(
                "INSERT INTO conversation_branches (conversation_id, id, parent_branch_id, fork_message_id, status, revision, created_at, updated_at) VALUES (?1, ?2, ?3, 'message', 'active', 1, 0, 0)",
                params![
                    conversation_id.to_string(),
                    child_branch_id.to_string(),
                    branch_id.to_string()
                ],
            )
            .expect("raw child branch");
        assert!(connection
            .execute(
                "UPDATE conversation_branches SET head_message_id = 'message' WHERE conversation_id = ?1 AND id = ?2",
                params![conversation_id.to_string(), child_branch_id.to_string()],
            )
            .is_err());
        assert!(connection
            .execute(
                "UPDATE conversation_branches SET parent_branch_id = 'missing-parent' WHERE conversation_id = ?1 AND id = ?2",
                params![conversation_id.to_string(), child_branch_id.to_string()],
            )
            .is_err());
        let bad_child_transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("bad child transaction");
        bad_child_transaction
            .execute(
                "INSERT INTO conversation_message_revisions (conversation_id, id, message_id, branch_id, sequence, parts_json, authored_at) VALUES (?1, 'bad-child-revision', 'bad-child-message', ?2, 1, '{\"format_version\":1,\"value\":null}', 0)",
                params![conversation_id.to_string(), child_branch_id.to_string()],
            )
            .expect("bad child revision");
        assert!(bad_child_transaction
            .execute(
                "INSERT INTO conversation_messages (conversation_id, id, branch_id, parent_message_id, role, timeline_ordinal, logical_time, effective_time, visibility, pinned, scene_edited, active_revision_id, revision, created_at, updated_at) VALUES (?1, 'bad-child-message', ?2, 'other-message', 'system', 3, 0, 0, 'visible', 0, 0, 'bad-child-revision', 1, 0, 0)",
                params![conversation_id.to_string(), child_branch_id.to_string()],
            )
            .is_err());
        assert!(bad_child_transaction.rollback().is_ok());
        let self_parent_error = connection
            .execute(
                "INSERT INTO conversation_branches (conversation_id, id, parent_branch_id, fork_message_id, status, revision, created_at, updated_at) VALUES (?1, ?2, ?2, 'message', 'active', 1, 0, 0)",
                params![conversation_id.to_string(), fresh_branch_id.to_string()],
            )
            .expect_err("self-parent check");
        assert!(
            self_parent_error
                .to_string()
                .contains("CHECK constraint failed")
        );
        connection
            .execute(
                "INSERT INTO conversation_turns (conversation_id, id, branch_id, operation, input_kind, user_message_id, idempotency_key, status, target_kind, target_message_id, target_parent_message_id, revision, created_at, updated_at) VALUES (?1, 'turn', ?2, 'send', 'user_message', 'message', 'turn-key', 'created', 'new_assistant', 'turn-target', 'message', 1, 0, 0)",
                params![conversation_id.to_string(), branch_id.to_string()],
            )
            .expect("raw turn");
        connection
            .execute(
                "INSERT INTO conversation_turns (conversation_id, id, branch_id, operation, input_kind, user_message_id, idempotency_key, status, target_kind, target_message_id, target_parent_message_id, revision, created_at, updated_at) VALUES (?1, 'final-turn', ?2, 'send', 'user_message', 'message', 'final-turn-key', 'created', 'new_assistant', 'new-assistant', 'message', 1, 0, 0)",
                params![conversation_id.to_string(), branch_id.to_string()],
            )
            .expect("valid final target turn");
        connection
            .execute(
                "INSERT INTO conversation_participants (conversation_id, id, role, ordinal, source_kind, source_id, enabled, muted, display_name, model_selection_json, revision, created_at, updated_at) VALUES (?1, 'character-participant', 'character', 1, 'character', 'character-source', 1, 0, 'Character', '{\"format_version\":1,\"value\":null}', 1, 0, 0)",
                [conversation_id.to_string()],
            )
            .expect("raw character participant");
        let target_message_transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("target message transaction");
        for (message_id, revision_id) in [
            ("new-assistant", "new-assistant-revision"),
            ("wrong-assistant", "wrong-assistant-revision"),
        ] {
            target_message_transaction
                .execute(
                    "INSERT INTO conversation_message_revisions (conversation_id, id, message_id, branch_id, sequence, parts_json, authored_at) VALUES (?1, ?2, ?3, ?4, 1, '{\"format_version\":1,\"value\":null}', 0)",
                    params![conversation_id.to_string(), revision_id, message_id, branch_id.to_string()],
                )
                .expect("target revision");
            target_message_transaction
                .execute(
                    "INSERT INTO conversation_messages (conversation_id, id, branch_id, parent_message_id, author_participant_id, role, timeline_ordinal, logical_time, effective_time, visibility, pinned, scene_edited, active_revision_id, revision, created_at, updated_at) VALUES (?1, ?2, ?3, 'message', 'character-participant', 'assistant', ?4, 0, 0, 'visible', 0, 0, ?5, 1, 0, 0)",
                    params![conversation_id.to_string(), message_id, branch_id.to_string(), if message_id == "new-assistant" { 5 } else { 6 }, revision_id],
                )
                .expect("target message");
        }
        target_message_transaction
            .execute(
                "INSERT INTO generation_attempts (conversation_id, turn_id, id, ordinal, status, job_idempotency_key) VALUES (?1, 'final-turn', 'final-attempt', 0, 'created', 'generation.final-turn.final-attempt')",
                [conversation_id.to_string()],
            )
            .expect("final attempt");
        target_message_transaction
            .commit()
            .expect("target message commit");
        assert!(connection
            .execute(
                "INSERT INTO conversation_message_candidates (conversation_id, id, message_id, branch_id, turn_id, attempt_id, ordinal, parts_json, model_json, created_at) VALUES (?1, 'wrong-candidate', 'wrong-assistant', ?2, 'final-turn', 'final-attempt', 0, '{\"format_version\":1,\"value\":null}', '{\"format_version\":1,\"value\":null}', 0)",
                params![conversation_id.to_string(), branch_id.to_string()],
            )
            .is_err());
        connection
            .execute(
                "INSERT INTO conversation_message_candidates (conversation_id, id, message_id, branch_id, turn_id, attempt_id, ordinal, parts_json, model_json, created_at) VALUES (?1, 'valid-candidate', 'new-assistant', ?2, 'final-turn', 'final-attempt', 0, '{\"format_version\":1,\"value\":null}', '{\"format_version\":1,\"value\":null}', 0)",
                params![conversation_id.to_string(), branch_id.to_string()],
            )
            .expect("valid final candidate target");
        assert!(connection
            .execute(
                "INSERT INTO conversation_turns (conversation_id, id, branch_id, operation, input_kind, user_message_id, idempotency_key, status, revision, created_at, updated_at) VALUES (?1, 'missing-target', ?2, 'send', 'user_message', 'message', 'missing-target-key', 'created', 1, 0, 0)",
                params![conversation_id.to_string(), branch_id.to_string()],
            )
            .is_err());
        assert!(connection
            .execute(
                "INSERT INTO conversation_turns (conversation_id, id, branch_id, operation, input_kind, user_message_id, idempotency_key, status, revision, created_at, updated_at) VALUES (?1, 'rejected-targetless', ?2, 'send', 'user_message', 'message', 'rejected-targetless-key', 'created', 1, 0, 0)",
                params![conversation_id.to_string(), branch_id.to_string()],
            )
            .is_err());
        assert!(connection
            .execute(
                "INSERT INTO conversation_turns (conversation_id, id, branch_id, operation, input_kind, user_message_id, idempotency_key, status, target_kind, target_message_id, target_parent_message_id, forced_speaker_participant_id, revision, created_at, updated_at) VALUES (?1, 'direct-speaker', ?2, 'send', 'user_message', 'message', 'direct-speaker-key', 'created', 'new_assistant', 'new-assistant-2', 'message', 'user-participant', 1, 0, 0)",
                params![conversation_id.to_string(), branch_id.to_string()],
            )
            .is_err());
        assert!(connection
            .execute(
                "INSERT INTO conversation_turns (conversation_id, id, branch_id, operation, input_kind, user_message_id, idempotency_key, status, target_kind, target_message_id, target_parent_message_id, retry_of_turn_id, revision, created_at, updated_at) VALUES (?1, 'bad-retry', ?2, 'send', 'user_message', 'message', 'bad-retry-key', 'created', 'new_assistant', 'new-assistant-3', 'message', 'turn', 1, 0, 0)",
                params![conversation_id.to_string(), branch_id.to_string()],
            )
            .is_err());
        assert!(connection
            .execute(
                "INSERT INTO conversation_turns (conversation_id, id, branch_id, operation, input_kind, user_message_id, idempotency_key, status, target_kind, target_message_id, target_parent_message_id, guidance, revision, created_at, updated_at) VALUES (?1, 'blank-guidance', ?2, 'send', 'user_message', 'message', 'blank-guidance-key', 'created', 'new_assistant', 'new-assistant-4', 'message', ' ', 1, 0, 0)",
                params![conversation_id.to_string(), branch_id.to_string()],
            )
            .is_err());
        for statement in [
            "UPDATE conversation_turns SET target_message_id = 'coherent-new-target' WHERE conversation_id = ?1 AND id = 'turn'",
            "UPDATE conversation_turns SET guidance = 'coherent guidance' WHERE conversation_id = ?1 AND id = 'turn'",
            "UPDATE conversation_turns SET requested_model_override_json = '{\"format_version\":1,\"value\":null}' WHERE conversation_id = ?1 AND id = 'turn'",
            "UPDATE conversation_turns SET forced_speaker_participant_id = 'character-participant' WHERE conversation_id = ?1 AND id = 'turn'",
            "UPDATE conversation_turns SET retry_of_turn_id = 'turn' WHERE conversation_id = ?1 AND id = 'turn'",
            "UPDATE conversation_turns SET swap_roles = 1 WHERE conversation_id = ?1 AND id = 'turn'",
        ] {
            assert!(
                connection
                    .execute(statement, [conversation_id.to_string()])
                    .is_err()
            );
        }
        connection
            .execute(
                "UPDATE conversations SET kind = 'group', kind_json = '{\"format_version\":1,\"value\":null}' WHERE id = ?1",
                [conversation_id.to_string()],
            )
            .expect("group speaker fixture");
        connection
            .execute(
                "INSERT INTO conversation_participants (conversation_id, id, role, ordinal, source_kind, source_id, enabled, muted, display_name, model_selection_json, revision, created_at, updated_at) VALUES (?1, 'wrong-group-character', 'character', 2, 'character', 'wrong-source', 1, 0, 'Wrong', '{\"format_version\":1,\"value\":null}', 1, 0, 0)",
                [conversation_id.to_string()],
            )
            .expect("wrong group character");
        let target_author: String = connection
            .query_row(
                "SELECT author_participant_id FROM conversation_messages WHERE conversation_id = ?1 AND id = ?2",
                params![conversation_id.to_string(), "new-assistant"],
                |row| row.get(0),
            )
            .expect("group target author");
        connection
            .execute(
                "INSERT INTO conversation_turns (conversation_id, id, branch_id, operation, input_kind, candidate_message_id, candidate_id, idempotency_key, status, target_kind, target_message_id, target_prior_candidate_id, revision, created_at, updated_at) VALUES (?1, 'group-regenerate', ?2, 'regenerate', 'existing_candidate', ?3, ?4, 'group-regenerate-key', 'created', 'existing_candidate', ?3, ?4, 1, 0, 0)",
                params![
                    conversation_id.to_string(),
                    branch_id.to_string(),
                    "new-assistant",
                    "valid-candidate",
                ],
            )
            .expect("group regenerate may resolve speaker later");
        connection
            .execute(
                "UPDATE conversation_turns SET selected_speaker_participant_id = ?1, selected_speaker_details_json = '{\"format_version\":1}' WHERE conversation_id = ?2 AND id = 'group-regenerate'",
                params![target_author, conversation_id.to_string()],
            )
            .expect("group regenerate target author speaker");
        assert!(connection
            .execute(
                "UPDATE conversation_turns SET selected_speaker_participant_id = 'wrong-group-character', selected_speaker_details_json = '{\"format_version\":1}' WHERE conversation_id = ?1 AND id = 'group-regenerate'",
                [conversation_id.to_string()],
            )
            .is_err());
        assert!(connection
            .execute(
                "INSERT INTO generation_attempts (conversation_id, turn_id, id, ordinal, status, job_idempotency_key) VALUES (?1, 'turn', 'attempt', 0, 'created', 'wrong')",
                [conversation_id.to_string()],
            )
            .is_err());
        connection
            .execute(
                "INSERT INTO generation_attempts (conversation_id, turn_id, id, ordinal, status, job_idempotency_key) VALUES (?1, 'turn', 'attempt', 0, 'created', 'generation.turn.attempt')",
                [conversation_id.to_string()],
            )
            .expect("raw attempt");
        assert!(connection
            .execute(
                "INSERT INTO generation_checkpoints (conversation_id, turn_id, attempt_id, sequence, event_json, created_at) VALUES (?1, 'turn', 'attempt', 2, '{\"format_version\":1,\"value\":null}', 0)",
                [conversation_id.to_string()],
            )
            .is_err());
        let message_transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("message transaction");
        message_transaction
            .execute(
                "INSERT INTO conversation_messages (conversation_id, id, branch_id, role, timeline_ordinal, logical_time, effective_time, visibility, pinned, scene_edited, active_revision_id, revision, created_at, updated_at) VALUES (?1, 'corrupt-message', ?2, 'system', 4, 0, 0, 'visible', 0, 0, 'missing-revision', 1, 0, 0)",
                params![conversation_id.to_string(), branch_id.to_string()],
            )
            .expect("raw message");
        assert!(message_transaction.commit().is_err());
        assert!(connection
            .execute(
                "INSERT INTO conversation_outbox (conversation_id, id, sequence, conversation_revision, operation_record_id, at, event_json) VALUES (?1, 'event', 1, 1, 'missing-operation', 0, '{\"format_version\":1,\"value\":null}')",
                [conversation_id.to_string()],
            )
            .is_err());
    }
}
