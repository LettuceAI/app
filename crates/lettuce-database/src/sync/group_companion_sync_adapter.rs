//! Group companion message state exchanged by sync.
//!
//! One entity holds every checkpoint of a message with its effects and the
//! persona applications recorded on that message. The entity id is
//! `<conversation>:<message>`, so it follows the message's own lifetime.

use std::collections::HashSet;

use lettuce_companions::{
    EmotionalState, GroupCompanionCheckpoint, GroupCompanionEffect, GroupCompanionMessageSource,
    GroupCompanionRepositoryError, GroupMemberRelationship,
};
use lettuce_conversations::{ConversationRepositoryError, ValidationError};
use lettuce_sync::{
    CanonicalPayload, GROUP_COMPANION_MESSAGE_SYNC_SCHEMA, GROUP_COMPANION_MESSAGE_SYNC_VERSION,
};
use lettuce_types::{CharacterId, ConversationId, MessageId, TimestampMillis};
use rusqlite::{Connection, Transaction, params};
use serde::{Deserialize, Serialize};

fn storage(error: impl std::fmt::Debug) -> ConversationRepositoryError {
    ConversationRepositoryError::Storage {
        reason: format!("{error:?}"),
    }
}

fn invalid() -> ConversationRepositoryError {
    ConversationRepositoryError::Invalid(ValidationError::InvalidValue {
        field: "sync.group_companion_message",
    })
}

fn parse_entity_id(id: &str) -> Result<(ConversationId, MessageId), ConversationRepositoryError> {
    let (conversation, message) = id.split_once(':').ok_or_else(invalid)?;
    Ok((
        conversation.parse().map_err(|_| invalid())?,
        message.parse().map_err(|_| invalid())?,
    ))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SyncGroupCompanionCheckpoint {
    pub character_id: CharacterId,
    pub source: GroupCompanionMessageSource,
    pub emotional_state: EmotionalState,
    pub member_relationships: Vec<GroupMemberRelationship>,
    pub effects: Vec<GroupCompanionEffect>,
    pub created_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SyncGroupCompanionPersonaApplication {
    pub character_id: CharacterId,
    pub persona_key: String,
    pub applied_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SyncGroupCompanionMessage {
    pub checkpoints: Vec<SyncGroupCompanionCheckpoint>,
    pub persona_applications: Vec<SyncGroupCompanionPersonaApplication>,
}

pub(crate) fn canonical_group_companion_message_payload(
    message: &SyncGroupCompanionMessage,
) -> Result<CanonicalPayload, ConversationRepositoryError> {
    CanonicalPayload::new(
        GROUP_COMPANION_MESSAGE_SYNC_SCHEMA,
        GROUP_COMPANION_MESSAGE_SYNC_VERSION,
        serde_json::to_vec(message).map_err(storage)?,
    )
    .map_err(storage)
}

fn checkpoint_from_wire(
    message_id: MessageId,
    checkpoint: &SyncGroupCompanionCheckpoint,
) -> Result<GroupCompanionCheckpoint, ConversationRepositoryError> {
    let checkpoint = GroupCompanionCheckpoint {
        message_id,
        character_id: checkpoint.character_id,
        source: checkpoint.source.clone(),
        emotional_state: checkpoint.emotional_state.clone(),
        member_relationships: checkpoint.member_relationships.clone(),
        effects: checkpoint.effects.clone(),
        created_at: checkpoint.created_at,
    };
    checkpoint.validate().map_err(|error| match error {
        GroupCompanionRepositoryError::Invalid { .. } => invalid(),
        _ => storage(error),
    })?;
    Ok(checkpoint)
}

fn message_from_wire(
    message_id: MessageId,
    message: &SyncGroupCompanionMessage,
) -> Result<
    (
        Vec<GroupCompanionCheckpoint>,
        Vec<SyncGroupCompanionPersonaApplication>,
    ),
    ConversationRepositoryError,
> {
    let checkpoints = message
        .checkpoints
        .iter()
        .map(|checkpoint| checkpoint_from_wire(message_id, checkpoint))
        .collect::<Result<Vec<_>, _>>()?;
    for application in &message.persona_applications {
        if application.persona_key.trim().is_empty() {
            return Err(invalid());
        }
    }
    Ok((checkpoints, message.persona_applications.clone()))
}

pub(crate) fn sync_group_companion_message_ids(
    connection: &Connection,
) -> rusqlite::Result<Vec<String>> {
    connection
        .prepare(
            "SELECT id FROM (
               SELECT conversation_id || ':' || message_id AS id
                 FROM group_companion_checkpoints
               UNION
               SELECT conversation_id || ':' || message_id AS id
                 FROM group_companion_persona_applications
             ) ORDER BY id",
        )?
        .query_map([], |row| row.get(0))?
        .collect()
}

fn load_message(
    connection: &Connection,
    conversation_id: ConversationId,
    message_id: MessageId,
) -> Result<Option<SyncGroupCompanionMessage>, ConversationRepositoryError> {
    let present: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM conversation_messages WHERE conversation_id = ?1 AND id = ?2)",
            params![conversation_id.to_string(), message_id.to_string()],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if !present {
        return Ok(None);
    }
    let checkpoints =
        crate::conversation::group_companion_adapter::load_checkpoints_for_message_in(
            connection,
            conversation_id,
            message_id,
        )
        .map_err(storage)?;
    let applications =
        crate::conversation::group_companion_adapter::load_persona_applications_for_message_in(
            connection,
            conversation_id,
            message_id,
        )
        .map_err(storage)?;
    if checkpoints.is_empty() && applications.is_empty() {
        return Ok(None);
    }
    Ok(Some(SyncGroupCompanionMessage {
        checkpoints: checkpoints
            .into_iter()
            .map(|checkpoint| SyncGroupCompanionCheckpoint {
                character_id: checkpoint.character_id,
                source: checkpoint.source,
                emotional_state: checkpoint.emotional_state,
                member_relationships: checkpoint.member_relationships,
                effects: checkpoint.effects,
                created_at: checkpoint.created_at,
            })
            .collect(),
        persona_applications: applications
            .into_iter()
            .map(
                |(character_id, persona_key, applied_at)| SyncGroupCompanionPersonaApplication {
                    character_id,
                    persona_key,
                    applied_at,
                },
            )
            .collect(),
    }))
}

pub(crate) fn sync_load_group_companion_message(
    connection: &Connection,
    id: &str,
) -> Result<Option<SyncGroupCompanionMessage>, ConversationRepositoryError> {
    let (conversation_id, message_id) = parse_entity_id(id)?;
    load_message(connection, conversation_id, message_id)
}

pub(crate) fn decode_group_companion_message(
    id: &str,
    bytes: &[u8],
) -> Result<(), ConversationRepositoryError> {
    let (_, message_id) = parse_entity_id(id)?;
    let message: SyncGroupCompanionMessage = serde_json::from_slice(bytes).map_err(storage)?;
    message_from_wire(message_id, &message).map(|_| ())
}

fn require_character(
    transaction: &Transaction<'_>,
    character_id: CharacterId,
) -> Result<(), ConversationRepositoryError> {
    let present: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM characters WHERE id = ?1)",
            [character_id.to_string()],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if present {
        Ok(())
    } else {
        Err(ConversationRepositoryError::NotFound)
    }
}

pub(crate) fn sync_replace_group_companion_message(
    transaction: &Transaction<'_>,
    id: &str,
    message: &SyncGroupCompanionMessage,
) -> Result<(), ConversationRepositoryError> {
    let (conversation_id, message_id) = parse_entity_id(id)?;
    let present: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM conversation_messages WHERE conversation_id = ?1 AND id = ?2)",
            params![conversation_id.to_string(), message_id.to_string()],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if !present {
        return Err(ConversationRepositoryError::NotFound);
    }
    let (checkpoints, applications) = message_from_wire(message_id, message)?;
    let mut characters = HashSet::new();
    for checkpoint in &checkpoints {
        characters.insert(checkpoint.character_id);
    }
    for application in &applications {
        characters.insert(application.character_id);
    }
    for character_id in characters {
        require_character(transaction, character_id)?;
    }
    crate::conversation::group_companion_adapter::replace_message_checkpoints_in(
        transaction,
        conversation_id,
        message_id,
        &checkpoints,
    )
    .map_err(storage)?;
    for application in &applications {
        transaction
            .execute(
                "INSERT OR IGNORE INTO group_companion_persona_applications (
                   conversation_id, message_id, character_id, persona_key, applied_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    conversation_id.to_string(),
                    message_id.to_string(),
                    application.character_id.to_string(),
                    application.persona_key,
                    application.applied_at.get()
                ],
            )
            .map_err(storage)?;
    }
    Ok(())
}