//! Companion state exchanged by sync.
//!
//! Four entities: a character's Soul (`companion_soul`, facts only), the
//! relationship a character keeps with one persona (`companion_relationship`,
//! `<character>:<persona key>`), a companion conversation's session state with
//! its continuity episode (`companion_session`) and scheduled notes. Revisions,
//! apply receipts and turn effects stay on the device that wrote them, and an
//! episode's index and predecessor are numbered by each device, since two
//! devices may open companion sessions at the same time.

use lettuce_companions::{
    CompanionScheduledNote, EmotionalState, RelationshipState, SoulFact, SoulOwner,
};
use lettuce_conversations::ConversationRepositoryError;
use lettuce_types::{CharacterId, ConversationId, PersonaId, TimestampMillis};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use crate::{companion::soul_adapter, conversation::state_adapter, memory::scheduled_note_adapter};

fn storage(_: impl std::fmt::Debug) -> ConversationRepositoryError {
    ConversationRepositoryError::Storage
}

fn exists(
    transaction: &Transaction<'_>,
    sql: &str,
    value: &str,
) -> Result<bool, ConversationRepositoryError> {
    transaction
        .query_row(sql, [value], |row| row.get(0))
        .map_err(storage)
}

fn require_character(
    transaction: &Transaction<'_>,
    character_id: CharacterId,
) -> Result<(), ConversationRepositoryError> {
    if exists(
        transaction,
        "SELECT EXISTS(SELECT 1 FROM characters WHERE id = ?1)",
        &character_id.to_string(),
    )? {
        Ok(())
    } else {
        Err(ConversationRepositoryError::NotFound)
    }
}

fn ids(connection: &Connection, sql: &str) -> rusqlite::Result<Vec<String>> {
    connection
        .prepare(sql)?
        .query_map([], |row| row.get(0))?
        .collect()
}

fn persona_key(persona_id: Option<PersonaId>) -> String {
    persona_id.map_or_else(|| "__default__".to_owned(), |id| id.to_string())
}

pub(crate) fn sync_soul_ids(connection: &Connection) -> rusqlite::Result<Vec<String>> {
    ids(
        connection,
        "SELECT CASE WHEN scope = '' THEN character_id ELSE character_id || ':' || scope END
           FROM companion_soul_states ORDER BY 1",
    )
}

pub(crate) fn sync_load_soul(
    transaction: &Transaction<'_>,
    owner: SoulOwner,
) -> Result<Option<Vec<SoulFact>>, ConversationRepositoryError> {
    Ok(soul_adapter::get_in(transaction, owner)
        .map_err(storage)?
        .map(|state| state.facts))
}

pub(crate) fn sync_replace_soul(
    transaction: &Transaction<'_>,
    owner: SoulOwner,
    facts: &[SoulFact],
) -> Result<(), ConversationRepositoryError> {
    let character_id = owner.character_id();
    require_character(transaction, character_id)?;
    if let Some(conversation_id) = owner.conversation_id() {
        let present: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ?1)",
                [conversation_id.to_string()],
                |row| row.get(0),
            )
            .map_err(storage)?;
        if !present {
            return Err(ConversationRepositoryError::NotFound);
        }
    }
    if sync_load_soul(transaction, owner)?.as_deref() == Some(facts) {
        return Ok(());
    }
    let now = TimestampMillis::now().map_err(storage)?;
    soul_adapter::replace_facts_in(transaction, owner, facts, now).map_err(
        |error| match error {
            lettuce_companions::SoulRepositoryError::Invalid(_) => {
                ConversationRepositoryError::Invalid(
                    lettuce_conversations::ValidationError::InvalidValue { field: "sync.soul" },
                )
            }
            _ => ConversationRepositoryError::Storage,
        },
    )?;
    transaction
        .execute(
            "UPDATE companion_soul_states SET revision = revision + 1, updated_at = max(updated_at, ?2) WHERE character_id = ?1 AND scope = ?3",
            params![character_id.to_string(), now.get(), soul_adapter::scope(owner)],
        )
        .map_err(storage)?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SyncCompanionRelationship {
    pub character_id: CharacterId,
    pub persona_id: Option<PersonaId>,
    pub state: RelationshipState,
}

pub(crate) fn sync_relationship_ids(connection: &Connection) -> rusqlite::Result<Vec<String>> {
    ids(
        connection,
        "SELECT character_id || ':' || persona_key FROM companion_relationship_states ORDER BY 1",
    )
}

pub(crate) fn sync_load_relationship(
    transaction: &Transaction<'_>,
    id: &str,
) -> Result<Option<SyncCompanionRelationship>, ConversationRepositoryError> {
    let (character, key) = id
        .split_once(':')
        .ok_or(ConversationRepositoryError::Storage)?;
    transaction
        .query_row(
            "SELECT persona_id, closeness, trust, affection, tension, stability, interaction_count, last_interaction_at
               FROM companion_relationship_states WHERE character_id = ?1 AND persona_key = ?2",
            params![character, key],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    RelationshipState {
                        closeness: row.get(1)?,
                        trust: row.get(2)?,
                        affection: row.get(3)?,
                        tension: row.get(4)?,
                        stability: row.get(5)?,
                        interaction_count: row.get(6)?,
                        last_interaction_at: TimestampMillis::new(row.get(7)?),
                    },
                ))
            },
        )
        .optional()
        .map_err(storage)?
        .map(|(persona, state)| {
            Ok(SyncCompanionRelationship {
                character_id: character.parse().map_err(storage)?,
                persona_id: persona.map(|id| id.parse()).transpose().map_err(storage)?,
                state,
            })
        })
        .transpose()
}

pub(crate) fn relationship_id(relationship: &SyncCompanionRelationship) -> String {
    format!(
        "{}:{}",
        relationship.character_id,
        persona_key(relationship.persona_id)
    )
}

pub(crate) fn sync_replace_relationship(
    transaction: &Transaction<'_>,
    relationship: &SyncCompanionRelationship,
) -> Result<(), ConversationRepositoryError> {
    require_character(transaction, relationship.character_id)?;
    let state = &relationship.state;
    transaction
        .execute(
            "INSERT INTO companion_relationship_states (
               character_id, persona_key, persona_id, closeness, trust, affection, tension,
               stability, interaction_count, last_interaction_at, revision, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?10, ?10)
             ON CONFLICT(character_id, persona_key) DO UPDATE SET
               closeness = excluded.closeness, trust = excluded.trust,
               affection = excluded.affection, tension = excluded.tension,
               stability = excluded.stability, interaction_count = excluded.interaction_count,
               last_interaction_at = excluded.last_interaction_at,
               revision = companion_relationship_states.revision + 1,
               updated_at = max(companion_relationship_states.updated_at, excluded.last_interaction_at)",
            params![
                relationship.character_id.to_string(),
                persona_key(relationship.persona_id),
                relationship.persona_id.map(|id| id.to_string()),
                state.closeness,
                state.trust,
                state.affection,
                state.tension,
                state.stability,
                i64::from(state.interaction_count),
                state.last_interaction_at.get(),
            ],
        )
        .map_err(storage)?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SyncCompanionSession {
    pub character_id: CharacterId,
    pub persona_id: Option<PersonaId>,
    pub initial_hash: String,
    pub emotional_state: EmotionalState,
    pub active_signals: Vec<String>,
    pub updated_at: TimestampMillis,
    pub episode_started_at: Option<TimestampMillis>,
    pub episode_ended_at: Option<TimestampMillis>,
}

pub(crate) fn sync_session_ids(connection: &Connection) -> rusqlite::Result<Vec<String>> {
    ids(
        connection,
        "SELECT conversation_id FROM companion_session_states ORDER BY conversation_id",
    )
}

type SessionRow = (String, Option<String>, Vec<u8>, f64, i64, i64);

pub(crate) fn sync_load_session(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
) -> Result<Option<SyncCompanionSession>, ConversationRepositoryError> {
    let row: Option<SessionRow> = transaction
        .query_row(
            "SELECT character_id, persona_id, initial_hash, confidence, emotional_updated_at, state_updated_at
               FROM companion_session_states WHERE conversation_id = ?1",
            [conversation_id.to_string()],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .optional()
        .map_err(storage)?;
    let Some((character, persona, initial_hash, confidence, emotional_at, state_at)) = row else {
        return Ok(None);
    };
    let episode: Option<(i64, Option<i64>)> = transaction
        .query_row(
            "SELECT started_at, ended_at FROM companion_continuity_episodes WHERE conversation_id = ?1",
            [conversation_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(storage)?;
    Ok(Some(SyncCompanionSession {
        character_id: character.parse().map_err(storage)?,
        persona_id: persona.map(|id| id.parse()).transpose().map_err(storage)?,
        initial_hash: blake3::Hash::from_slice(&initial_hash)
            .map_err(storage)?
            .to_hex()
            .to_string(),
        emotional_state: EmotionalState {
            felt: state_adapter::read_vector(transaction, conversation_id, "felt")
                .map_err(storage)?,
            expressed: state_adapter::read_vector(transaction, conversation_id, "expressed")
                .map_err(storage)?,
            blocked: state_adapter::read_vector(transaction, conversation_id, "blocked")
                .map_err(storage)?,
            momentum: state_adapter::read_vector(transaction, conversation_id, "momentum")
                .map_err(storage)?,
            active_drivers: state_adapter::read_signals(transaction, conversation_id, "driver")
                .map_err(storage)?,
            confidence,
            updated_at: TimestampMillis::new(emotional_at),
        },
        active_signals: state_adapter::read_signals(transaction, conversation_id, "active")
            .map_err(storage)?,
        updated_at: TimestampMillis::new(state_at),
        episode_started_at: episode.map(|(started, _)| TimestampMillis::new(started)),
        episode_ended_at: episode
            .and_then(|(_, ended)| ended)
            .map(TimestampMillis::new),
    }))
}

/// Writes a synced companion session once its conversation and relationship
/// exist. A new continuity episode takes the next local index after the
/// latest episode of the same character and persona.
pub(crate) fn sync_replace_session(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    session: &SyncCompanionSession,
) -> Result<(), ConversationRepositoryError> {
    let key = persona_key(session.persona_id);
    if !exists(
        transaction,
        "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ?1)",
        &conversation_id.to_string(),
    )? || !transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM companion_relationship_states WHERE character_id = ?1 AND persona_key = ?2)",
            params![session.character_id.to_string(), key],
            |row| row.get::<_, bool>(0),
        )
        .map_err(storage)?
    {
        return Err(ConversationRepositoryError::NotFound);
    }
    if sync_load_session(transaction, conversation_id)?.as_ref() == Some(session) {
        return Ok(());
    }
    let initial_hash = blake3::Hash::from_hex(&session.initial_hash).map_err(storage)?;
    let emotional = &session.emotional_state;
    transaction
        .execute(
            "INSERT INTO companion_session_states (
               conversation_id, character_id, persona_key, persona_id, initial_hash, confidence,
               emotional_updated_at, state_updated_at, revision, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?8, ?8)
             ON CONFLICT(conversation_id) DO UPDATE SET
               confidence = excluded.confidence,
               emotional_updated_at = excluded.emotional_updated_at,
               state_updated_at = excluded.state_updated_at,
               revision = companion_session_states.revision + 1,
               updated_at = max(companion_session_states.updated_at, excluded.state_updated_at)",
            params![
                conversation_id.to_string(),
                session.character_id.to_string(),
                key,
                session.persona_id.map(|id| id.to_string()),
                initial_hash.as_bytes().as_slice(),
                emotional.confidence,
                emotional.updated_at.get(),
                session.updated_at.get(),
            ],
        )
        .map_err(storage)?;
    state_adapter::replace_vectors(transaction, conversation_id, emotional).map_err(storage)?;
    state_adapter::replace_signals(
        transaction,
        conversation_id,
        emotional,
        &session.active_signals,
    )
    .map_err(storage)?;
    let Some(started_at) = session.episode_started_at else {
        return Ok(());
    };
    let updated_at = session
        .episode_ended_at
        .unwrap_or(started_at)
        .max(started_at);
    if exists(
        transaction,
        "SELECT EXISTS(SELECT 1 FROM companion_continuity_episodes WHERE conversation_id = ?1)",
        &conversation_id.to_string(),
    )? {
        transaction
            .execute(
                "UPDATE companion_continuity_episodes SET ended_at = ?2, updated_at = max(updated_at, ?3) WHERE conversation_id = ?1",
                params![
                    conversation_id.to_string(),
                    session.episode_ended_at.map(TimestampMillis::get),
                    updated_at.get(),
                ],
            )
            .map_err(storage)?;
        return Ok(());
    }
    let previous: Option<String> = transaction
        .query_row(
            "SELECT conversation_id FROM companion_continuity_episodes
              WHERE character_id = ?1 AND persona_key = ?2 AND started_at <= ?3
              ORDER BY started_at DESC, episode_index DESC LIMIT 1",
            params![session.character_id.to_string(), key, started_at.get()],
            |row| row.get(0),
        )
        .optional()
        .map_err(storage)?;
    let index: i64 = transaction
        .query_row(
            "SELECT COALESCE(MAX(episode_index), 0) + 1 FROM companion_continuity_episodes
              WHERE character_id = ?1 AND persona_key = ?2",
            params![session.character_id.to_string(), key],
            |row| row.get(0),
        )
        .map_err(storage)?;
    transaction
        .execute(
            "INSERT INTO companion_continuity_episodes (
               conversation_id, character_id, persona_key, persona_id, episode_index,
               previous_conversation_id, started_at, ended_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                conversation_id.to_string(),
                session.character_id.to_string(),
                key,
                session.persona_id.map(|id| id.to_string()),
                index,
                previous,
                started_at.get(),
                session.episode_ended_at.map(TimestampMillis::get),
                updated_at.get(),
            ],
        )
        .map_err(storage)?;
    Ok(())
}

pub(crate) fn sync_note_ids(connection: &Connection) -> rusqlite::Result<Vec<String>> {
    ids(
        connection,
        "SELECT id FROM companion_scheduled_notes ORDER BY id",
    )
}

pub(crate) fn sync_load_note(
    transaction: &Transaction<'_>,
    id: uuid::Uuid,
) -> Result<Option<CompanionScheduledNote>, ConversationRepositoryError> {
    scheduled_note_adapter::load_in(transaction, id).map_err(storage)
}

pub(crate) fn sync_replace_note(
    transaction: &Transaction<'_>,
    note: &CompanionScheduledNote,
) -> Result<(), ConversationRepositoryError> {
    require_character(transaction, note.character_id)?;
    note.validate().map_err(|_| {
        ConversationRepositoryError::Invalid(lettuce_conversations::ValidationError::InvalidValue {
            field: "sync.scheduled_note",
        })
    })?;
    transaction
        .execute(
            "DELETE FROM companion_scheduled_notes WHERE id = ?1",
            [note.id.to_string()],
        )
        .map_err(storage)?;
    scheduled_note_adapter::insert_restored_note_in(transaction, note).map_err(storage)
}

pub(crate) fn sync_delete_note(
    transaction: &Transaction<'_>,
    id: &str,
) -> Result<bool, ConversationRepositoryError> {
    transaction
        .execute("DELETE FROM companion_scheduled_notes WHERE id = ?1", [id])
        .map_err(storage)?;
    Ok(true)
}
