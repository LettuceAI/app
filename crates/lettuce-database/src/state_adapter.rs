use std::str::FromStr;

use blake3::Hasher;
use lettuce_companions::{
    CompanionContinueRepositoryError, CompanionContinuityEpisode, CompanionConversationContinuer,
    CompanionConversationCreator, CompanionConversationSender, CompanionEffectSourceWindow,
    CompanionEmotionDelta, CompanionLaunchRepositoryError, CompanionMemoryChanges,
    CompanionRuntimeState, CompanionSendRepositoryError, CompanionSignalChanges,
    CompanionStateApplyReceipt, CompanionStateOwner, CompanionStateReplacement,
    CompanionStateRepository, CompanionStateRepositoryError, CompanionStateSnapshot,
    CompanionTurnEffect, CompanionTurnEffectOutcome, CompanionTurnEffectRepository,
    CompanionTurnEffectRepositoryError, CompanionTurnEffectSeed, CompanionTurnEffectStatus,
    EmotionVector, EmotionalState, PreparedCompanionContinue, PreparedCompanionLaunch,
    PreparedCompanionSend, RelationshipDelta, RelationshipState, validate_runtime_state,
};
use lettuce_types::{
    CharacterId, CompanionEffectId, ConversationId, GenerationTurnId, MemoryId, MessageId,
    OperationRecordId, PersonaId, Revision, TimestampMillis,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use crate::Database;

type Error = CompanionStateRepositoryError;

fn conversation_state_error(error: Error) -> lettuce_conversations::ConversationRepositoryError {
    match error {
        Error::AlreadyExists | Error::Conflict | Error::OperationMismatch => {
            lettuce_conversations::ConversationRepositoryError::Conflict
        }
        _ => lettuce_conversations::ConversationRepositoryError::Storage,
    }
}

fn failure(_: impl std::fmt::Debug) -> Error {
    Error::Failure
}

fn corrupt(_: impl std::fmt::Debug) -> Error {
    Error::Corrupt
}

fn persona_key(owner: CompanionStateOwner) -> String {
    owner
        .persona_id
        .map(|value| value.to_string())
        .unwrap_or_else(|| "__default__".to_owned())
}

fn parse_companion_id<T: FromStr>(value: String) -> Result<T, Error> {
    value.parse().map_err(|_| Error::Corrupt)
}

fn sql_revision(value: Revision) -> Result<i64, Error> {
    i64::try_from(value.get()).map_err(corrupt)
}

fn parse_revision(value: i64) -> Result<Revision, Error> {
    let value = u64::try_from(value).map_err(corrupt)?;
    (value > 0)
        .then(|| Revision::new(value))
        .ok_or(Error::Corrupt)
}

fn vector_values(vector: &EmotionVector) -> [f64; 10] {
    [
        vector.warmth,
        vector.trust,
        vector.calm,
        vector.vulnerability,
        vector.longing,
        vector.hurt,
        vector.tension,
        vector.irritation,
        vector.affection_intensity,
        vector.reassurance_need,
    ]
}

fn insert_vector(
    tx: &Transaction<'_>,
    conversation_id: ConversationId,
    kind: &str,
    vector: &EmotionVector,
) -> Result<(), Error> {
    let values = vector_values(vector);
    tx.execute(
        "INSERT INTO companion_emotion_vectors (
           conversation_id, kind, warmth, trust, calm, vulnerability, longing,
           hurt, tension, irritation, affection_intensity, reassurance_need
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            conversation_id.to_string(),
            kind,
            values[0],
            values[1],
            values[2],
            values[3],
            values[4],
            values[5],
            values[6],
            values[7],
            values[8],
            values[9]
        ],
    )
    .map_err(failure)?;
    Ok(())
}

fn replace_vectors(
    tx: &Transaction<'_>,
    conversation_id: ConversationId,
    emotional: &EmotionalState,
) -> Result<(), Error> {
    tx.execute(
        "DELETE FROM companion_emotion_vectors WHERE conversation_id = ?1",
        [conversation_id.to_string()],
    )
    .map_err(failure)?;
    for (kind, vector) in [
        ("felt", &emotional.felt),
        ("expressed", &emotional.expressed),
        ("blocked", &emotional.blocked),
        ("momentum", &emotional.momentum),
    ] {
        insert_vector(tx, conversation_id, kind, vector)?;
    }
    Ok(())
}

fn replace_signals(
    tx: &Transaction<'_>,
    conversation_id: ConversationId,
    emotional: &EmotionalState,
    active_signals: &[String],
) -> Result<(), Error> {
    tx.execute(
        "DELETE FROM companion_state_signals WHERE conversation_id = ?1",
        [conversation_id.to_string()],
    )
    .map_err(failure)?;
    for (scope, values) in [
        ("driver", emotional.active_drivers.as_slice()),
        ("active", active_signals),
    ] {
        for (ordinal, value) in values.iter().enumerate() {
            tx.execute(
                "INSERT INTO companion_state_signals (conversation_id, scope, ordinal, value)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    conversation_id.to_string(),
                    scope,
                    i64::try_from(ordinal).map_err(corrupt)?,
                    value
                ],
            )
            .map_err(failure)?;
        }
    }
    Ok(())
}

fn read_vector(
    tx: &Transaction<'_>,
    conversation_id: ConversationId,
    kind: &str,
) -> Result<EmotionVector, Error> {
    tx.query_row(
        "SELECT warmth, trust, calm, vulnerability, longing, hurt, tension,
                irritation, affection_intensity, reassurance_need
         FROM companion_emotion_vectors WHERE conversation_id = ?1 AND kind = ?2",
        params![conversation_id.to_string(), kind],
        |row| {
            Ok(EmotionVector {
                warmth: row.get(0)?,
                trust: row.get(1)?,
                calm: row.get(2)?,
                vulnerability: row.get(3)?,
                longing: row.get(4)?,
                hurt: row.get(5)?,
                tension: row.get(6)?,
                irritation: row.get(7)?,
                affection_intensity: row.get(8)?,
                reassurance_need: row.get(9)?,
            })
        },
    )
    .map_err(corrupt)
}

fn read_signals(
    tx: &Transaction<'_>,
    conversation_id: ConversationId,
    scope: &str,
) -> Result<Vec<String>, Error> {
    let mut statement = tx
        .prepare(
            "SELECT value FROM companion_state_signals
             WHERE conversation_id = ?1 AND scope = ?2 ORDER BY ordinal",
        )
        .map_err(corrupt)?;
    statement
        .query_map(params![conversation_id.to_string(), scope], |row| {
            row.get(0)
        })
        .map_err(corrupt)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(corrupt)
}

fn get_in(
    tx: &Transaction<'_>,
    owner: CompanionStateOwner,
) -> Result<Option<CompanionStateSnapshot>, Error> {
    let row = tx
        .query_row(
            "SELECT character_id, persona_id, confidence, emotional_updated_at,
                    state_updated_at, revision
             FROM companion_session_states WHERE conversation_id = ?1",
            [owner.conversation_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, f64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            },
        )
        .optional()
        .map_err(corrupt)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let stored_character = CharacterId::from_str(&row.0).map_err(corrupt)?;
    let stored_persona = row
        .1
        .map(|value| PersonaId::from_str(&value).map_err(corrupt))
        .transpose()?;
    if stored_character != owner.character_id || stored_persona != owner.persona_id {
        return Err(Error::Corrupt);
    }
    let key = persona_key(owner);
    let relationship = tx
        .query_row(
            "SELECT closeness, trust, affection, tension, stability, interaction_count,
                    last_interaction_at, revision
             FROM companion_relationship_states
             WHERE character_id = ?1 AND persona_key = ?2",
            params![owner.character_id.to_string(), key],
            |row| {
                Ok((
                    RelationshipState {
                        closeness: row.get(0)?,
                        trust: row.get(1)?,
                        affection: row.get(2)?,
                        tension: row.get(3)?,
                        stability: row.get(4)?,
                        interaction_count: row.get(5)?,
                        last_interaction_at: TimestampMillis::new(row.get(6)?),
                    },
                    row.get::<_, i64>(7)?,
                ))
            },
        )
        .optional()
        .map_err(corrupt)?
        .ok_or(Error::Corrupt)?;
    let state = CompanionRuntimeState {
        emotional_state: EmotionalState {
            felt: read_vector(tx, owner.conversation_id, "felt")?,
            expressed: read_vector(tx, owner.conversation_id, "expressed")?,
            blocked: read_vector(tx, owner.conversation_id, "blocked")?,
            momentum: read_vector(tx, owner.conversation_id, "momentum")?,
            active_drivers: read_signals(tx, owner.conversation_id, "driver")?,
            confidence: row.2,
            updated_at: TimestampMillis::new(row.3),
        },
        relationship_state: relationship.0,
        active_signals: read_signals(tx, owner.conversation_id, "active")?,
        updated_at: TimestampMillis::new(row.4),
    };
    validate_runtime_state(&state).map_err(|_| Error::Corrupt)?;
    Ok(Some(CompanionStateSnapshot {
        owner,
        session_revision: parse_revision(row.5)?,
        relationship_revision: parse_revision(relationship.1)?,
        state,
    }))
}

fn put_hash_part(hasher: &mut Hasher, value: &[u8]) {
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value);
}

fn state_hash(state: &CompanionRuntimeState) -> [u8; 32] {
    let mut hasher = Hasher::new();
    for vector in [
        &state.emotional_state.felt,
        &state.emotional_state.expressed,
        &state.emotional_state.blocked,
        &state.emotional_state.momentum,
    ] {
        for value in vector_values(vector) {
            hasher.update(&value.to_bits().to_le_bytes());
        }
    }
    for values in [
        state.emotional_state.active_drivers.as_slice(),
        state.active_signals.as_slice(),
    ] {
        hasher.update(&(values.len() as u64).to_le_bytes());
        for value in values {
            put_hash_part(&mut hasher, value.as_bytes());
        }
    }
    hasher.update(&state.emotional_state.confidence.to_bits().to_le_bytes());
    hasher.update(&state.emotional_state.updated_at.get().to_le_bytes());
    let relationship = &state.relationship_state;
    for value in [
        relationship.closeness,
        relationship.trust,
        relationship.affection,
        relationship.tension,
        relationship.stability,
    ] {
        hasher.update(&value.to_bits().to_le_bytes());
    }
    hasher.update(&relationship.interaction_count.to_le_bytes());
    hasher.update(&relationship.last_interaction_at.get().to_le_bytes());
    hasher.update(&state.updated_at.get().to_le_bytes());
    *hasher.finalize().as_bytes()
}

fn replacement_hash(replacement: &CompanionStateReplacement) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(&replacement.expected_session_revision.get().to_le_bytes());
    hasher.update(
        &replacement
            .expected_relationship_revision
            .get()
            .to_le_bytes(),
    );
    hasher.update(&replacement.applied_at.get().to_le_bytes());
    hasher.update(&state_hash(&replacement.state));
    *hasher.finalize().as_bytes()
}

fn load_receipt(
    tx: &Transaction<'_>,
    operation_id: OperationRecordId,
) -> Result<Option<(CompanionStateApplyReceipt, Vec<u8>)>, Error> {
    tx.query_row(
        "SELECT conversation_id, character_id, persona_key, expected_session_revision,
                resulting_session_revision, expected_relationship_revision,
                resulting_relationship_revision, applied_at, change_hash
         FROM companion_state_apply_receipts WHERE operation_id = ?1",
        [operation_id.to_string()],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, Vec<u8>>(8)?,
            ))
        },
    )
    .optional()
    .map_err(corrupt)?
    .map(|row| {
        let persona_id = (row.2 != "__default__")
            .then(|| PersonaId::from_str(&row.2).map_err(corrupt))
            .transpose()?;
        let receipt = CompanionStateApplyReceipt {
            operation_id,
            owner: CompanionStateOwner {
                conversation_id: ConversationId::from_str(&row.0).map_err(corrupt)?,
                character_id: CharacterId::from_str(&row.1).map_err(corrupt)?,
                persona_id,
            },
            expected_session_revision: parse_revision(row.3)?,
            resulting_session_revision: parse_revision(row.4)?,
            expected_relationship_revision: parse_revision(row.5)?,
            resulting_relationship_revision: parse_revision(row.6)?,
            applied_at: TimestampMillis::new(row.7),
        };
        if receipt.resulting_session_revision
            != receipt.expected_session_revision.next().map_err(corrupt)?
            || receipt.resulting_relationship_revision
                != receipt
                    .expected_relationship_revision
                    .next()
                    .map_err(corrupt)?
            || row.8.len() != 32
        {
            return Err(Error::Corrupt);
        }
        Ok((receipt, row.8))
    })
    .transpose()
}

pub(crate) fn create_in(
    tx: &Transaction<'_>,
    owner: CompanionStateOwner,
    initial: &CompanionRuntimeState,
    now: TimestampMillis,
) -> Result<CompanionStateSnapshot, Error> {
    validate_runtime_state(initial)?;
    let initial_hash = state_hash(initial);
    let valid_owner = tx
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM conversations AS conversation
               JOIN conversation_participants AS participant
                 ON participant.conversation_id = conversation.id
               WHERE conversation.id = ?1 AND conversation.kind = 'direct'
                 AND participant.role = 'character'
                 AND participant.source_kind = 'character' AND participant.source_id = ?2
             )",
            params![
                owner.conversation_id.to_string(),
                owner.character_id.to_string()
            ],
            |row| row.get::<_, bool>(0),
        )
        .map_err(corrupt)?;
    if !valid_owner {
        return Err(Error::Invalid);
    }
    if let Some(existing) = get_in(tx, owner)? {
        let stored_hash = tx
            .query_row(
                "SELECT initial_hash FROM companion_session_states WHERE conversation_id = ?1",
                [owner.conversation_id.to_string()],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .map_err(corrupt)?;
        return if stored_hash.as_slice() == initial_hash {
            Ok(existing)
        } else {
            Err(Error::AlreadyExists)
        };
    }
    let key = persona_key(owner);
    let relationship = &initial.relationship_state;
    tx.execute(
        "INSERT OR IGNORE INTO companion_relationship_states (
           character_id, persona_key, persona_id, closeness, trust, affection, tension,
           stability, interaction_count, last_interaction_at, revision, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11, ?11)",
        params![
            owner.character_id.to_string(),
            key,
            owner.persona_id.map(|id| id.to_string()),
            relationship.closeness,
            relationship.trust,
            relationship.affection,
            relationship.tension,
            relationship.stability,
            i64::from(relationship.interaction_count),
            relationship.last_interaction_at.get(),
            now.get()
        ],
    )
    .map_err(failure)?;
    let emotional = &initial.emotional_state;
    tx.execute(
        "INSERT INTO companion_session_states (
           conversation_id, character_id, persona_key, persona_id, initial_hash, confidence,
           emotional_updated_at, state_updated_at, revision, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?9)",
        params![
            owner.conversation_id.to_string(),
            owner.character_id.to_string(),
            key,
            owner.persona_id.map(|id| id.to_string()),
            initial_hash.as_slice(),
            emotional.confidence,
            emotional.updated_at.get(),
            initial.updated_at.get(),
            now.get()
        ],
    )
    .map_err(failure)?;
    replace_vectors(tx, owner.conversation_id, emotional)?;
    replace_signals(
        tx,
        owner.conversation_id,
        emotional,
        &initial.active_signals,
    )?;
    get_in(tx, owner)?.ok_or(Error::Corrupt)
}

fn ensure_continuity_episode_in(
    tx: &Transaction<'_>,
    owner: CompanionStateOwner,
    now: TimestampMillis,
) -> Result<(), Error> {
    let conversation_id = owner.conversation_id.to_string();
    let exists = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM companion_continuity_episodes WHERE conversation_id = ?1)",
            [&conversation_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(failure)?;
    if exists {
        return Ok(());
    }

    let key = persona_key(owner);
    let previous = tx
        .query_row(
            "SELECT conversation_id, episode_index
             FROM companion_continuity_episodes
             WHERE character_id = ?1 AND persona_key = ?2
             ORDER BY episode_index DESC, started_at DESC
             LIMIT 1",
            params![owner.character_id.to_string(), key],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(failure)?;
    let (previous_conversation_id, episode_index) = match previous {
        Some((previous_conversation_id, previous_index)) => {
            tx.execute(
                "UPDATE companion_continuity_episodes
                 SET ended_at = ?1, updated_at = ?1
                 WHERE conversation_id = ?2 AND ended_at IS NULL",
                params![now.get(), previous_conversation_id],
            )
            .map_err(failure)?;
            (
                Some(previous_conversation_id),
                previous_index.checked_add(1).ok_or(Error::Corrupt)?,
            )
        }
        None => (None, 1),
    };
    tx.execute(
        "INSERT INTO companion_continuity_episodes (
           conversation_id, character_id, persona_key, persona_id, episode_index,
           previous_conversation_id, started_at, ended_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?7)",
        params![
            conversation_id,
            owner.character_id.to_string(),
            key,
            owner.persona_id.map(|id| id.to_string()),
            episode_index,
            previous_conversation_id,
            now.get(),
        ],
    )
    .map_err(failure)?;
    Ok(())
}

pub(crate) fn replace_in(
    tx: &Transaction<'_>,
    owner: CompanionStateOwner,
    replacement: &CompanionStateReplacement,
) -> Result<(Revision, Revision), Error> {
    validate_runtime_state(&replacement.state)?;
    let current = get_in(tx, owner)?.ok_or(Error::NotFound)?;
    if current.session_revision != replacement.expected_session_revision
        || current.relationship_revision != replacement.expected_relationship_revision
    {
        return Err(Error::Conflict);
    }
    let next_session = current.session_revision.next().map_err(corrupt)?;
    let next_relationship = current.relationship_revision.next().map_err(corrupt)?;
    let relationship = &replacement.state.relationship_state;
    let key = persona_key(owner);
    let relationship_updated = tx
        .execute(
            "UPDATE companion_relationship_states SET
               closeness = ?3, trust = ?4, affection = ?5, tension = ?6, stability = ?7,
               interaction_count = ?8, last_interaction_at = ?9, revision = ?10, updated_at = ?11
             WHERE character_id = ?1 AND persona_key = ?2 AND revision = ?12",
            params![
                owner.character_id.to_string(),
                key,
                relationship.closeness,
                relationship.trust,
                relationship.affection,
                relationship.tension,
                relationship.stability,
                i64::from(relationship.interaction_count),
                relationship.last_interaction_at.get(),
                sql_revision(next_relationship)?,
                replacement.applied_at.get(),
                sql_revision(current.relationship_revision)?
            ],
        )
        .map_err(failure)?;
    let emotional = &replacement.state.emotional_state;
    let session_updated = tx
        .execute(
            "UPDATE companion_session_states SET confidence = ?2, emotional_updated_at = ?3,
               state_updated_at = ?4, revision = ?5, updated_at = ?6
             WHERE conversation_id = ?1 AND revision = ?7",
            params![
                owner.conversation_id.to_string(),
                emotional.confidence,
                emotional.updated_at.get(),
                replacement.state.updated_at.get(),
                sql_revision(next_session)?,
                replacement.applied_at.get(),
                sql_revision(current.session_revision)?
            ],
        )
        .map_err(failure)?;
    if relationship_updated != 1 || session_updated != 1 {
        return Err(Error::Conflict);
    }
    replace_vectors(tx, owner.conversation_id, emotional)?;
    replace_signals(
        tx,
        owner.conversation_id,
        emotional,
        &replacement.state.active_signals,
    )?;
    Ok((next_session, next_relationship))
}

impl CompanionStateRepository for Database {
    fn create(
        &self,
        owner: CompanionStateOwner,
        initial: CompanionRuntimeState,
        now: TimestampMillis,
    ) -> Result<CompanionStateSnapshot, Error> {
        let mut connection = self.connection().map_err(failure)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let snapshot = create_in(&tx, owner, &initial, now)?;
        tx.commit().map_err(failure)?;
        Ok(snapshot)
    }

    fn get(&self, owner: CompanionStateOwner) -> Result<Option<CompanionStateSnapshot>, Error> {
        let mut connection = self.connection().map_err(failure)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(failure)?;
        let state = get_in(&tx, owner)?;
        tx.commit().map_err(failure)?;
        Ok(state)
    }

    fn get_continuity_episode(
        &self,
        conversation_id: ConversationId,
    ) -> Result<Option<CompanionContinuityEpisode>, Error> {
        let connection = self.connection().map_err(failure)?;
        let row = connection
            .query_row(
                "SELECT conversation_id, character_id, persona_id, episode_index,
                        previous_conversation_id, started_at, ended_at, updated_at
                 FROM companion_continuity_episodes WHERE conversation_id = ?1",
                [conversation_id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, Option<i64>>(6)?,
                        row.get::<_, i64>(7)?,
                    ))
                },
            )
            .optional()
            .map_err(failure)?;
        row.map(
            |(
                conversation_id,
                character_id,
                persona_id,
                episode_index,
                previous_conversation_id,
                started_at,
                ended_at,
                updated_at,
            )| {
                Ok(CompanionContinuityEpisode {
                    conversation_id: parse_companion_id(conversation_id)?,
                    character_id: parse_companion_id(character_id)?,
                    persona_id: persona_id.map(parse_companion_id).transpose()?,
                    episode_index: u32::try_from(episode_index).map_err(corrupt)?,
                    previous_conversation_id: previous_conversation_id
                        .map(parse_companion_id)
                        .transpose()?,
                    started_at: TimestampMillis::new(started_at),
                    ended_at: ended_at.map(TimestampMillis::new),
                    updated_at: TimestampMillis::new(updated_at),
                })
            },
        )
        .transpose()
    }

    fn replace(
        &self,
        owner: CompanionStateOwner,
        operation_id: OperationRecordId,
        replacement: CompanionStateReplacement,
    ) -> Result<CompanionStateApplyReceipt, Error> {
        validate_runtime_state(&replacement.state)?;
        let hash = replacement_hash(&replacement);
        let mut connection = self.connection().map_err(failure)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        if let Some((receipt, stored_hash)) = load_receipt(&tx, operation_id)? {
            if receipt.owner == owner
                && receipt.expected_session_revision == replacement.expected_session_revision
                && receipt.expected_relationship_revision
                    == replacement.expected_relationship_revision
                && receipt.applied_at == replacement.applied_at
                && stored_hash.as_slice() == hash
            {
                tx.commit().map_err(failure)?;
                return Ok(receipt);
            }
            return Err(Error::OperationMismatch);
        }
        let (next_session, next_relationship) = replace_in(&tx, owner, &replacement)?;
        let key = persona_key(owner);
        tx.execute(
            "INSERT INTO companion_state_apply_receipts (
               operation_id, conversation_id, character_id, persona_key,
               expected_session_revision, resulting_session_revision,
               expected_relationship_revision, resulting_relationship_revision,
               applied_at, change_hash
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                operation_id.to_string(),
                owner.conversation_id.to_string(),
                owner.character_id.to_string(),
                key,
                sql_revision(replacement.expected_session_revision)?,
                sql_revision(next_session)?,
                sql_revision(replacement.expected_relationship_revision)?,
                sql_revision(next_relationship)?,
                replacement.applied_at.get(),
                hash.as_slice()
            ],
        )
        .map_err(failure)?;
        let receipt = CompanionStateApplyReceipt {
            operation_id,
            owner,
            expected_session_revision: replacement.expected_session_revision,
            resulting_session_revision: next_session,
            expected_relationship_revision: replacement.expected_relationship_revision,
            resulting_relationship_revision: next_relationship,
            applied_at: replacement.applied_at,
        };
        tx.commit().map_err(failure)?;
        Ok(receipt)
    }
}

impl CompanionConversationCreator for Database {
    fn create_companion_conversation(
        &self,
        launch: PreparedCompanionLaunch,
        now: TimestampMillis,
    ) -> Result<lettuce_conversations::CreateConversationResult, CompanionLaunchRepositoryError>
    {
        let (conversation, owner, initial) = launch.into_parts();
        crate::conversation_creator::create_with_hook(self, conversation, now, |tx, _| {
            create_in(tx, owner, &initial, now).map_err(conversation_state_error)?;
            ensure_continuity_episode_in(tx, owner, now).map_err(conversation_state_error)
        })
        .map_err(CompanionLaunchRepositoryError::Conversation)
    }
}

impl CompanionConversationSender for Database {
    fn begin_companion_send(
        &self,
        prepared: PreparedCompanionSend,
        now: TimestampMillis,
    ) -> Result<lettuce_conversations::SendConversationResult, CompanionSendRepositoryError> {
        let (command, owner, replacement, effect_seed) = prepared.into_parts();
        crate::conversation_mutations::begin_send_with_hook(
            self,
            &command,
            now,
            |tx, turn_id, user_message_id| {
                replace_in(tx, owner, &replacement).map_err(conversation_state_error)?;
                if let Some(effect_seed) = &effect_seed {
                    insert_effect_draft_in(
                        tx,
                        owner.conversation_id,
                        turn_id,
                        Some(user_message_id),
                        effect_seed,
                        now,
                    )
                    .map_err(effect_conversation_error)?;
                }
                Ok(())
            },
        )
        .map_err(CompanionSendRepositoryError::Conversation)
    }
}

impl CompanionConversationContinuer for Database {
    fn begin_companion_continue(
        &self,
        prepared: PreparedCompanionContinue,
        now: TimestampMillis,
    ) -> Result<lettuce_conversations::ContinueConversationResult, CompanionContinueRepositoryError>
    {
        let command = prepared.into_command();
        crate::conversation_mutations::begin_continue_with_hook(
            self,
            &command,
            now,
            |tx, turn_id| {
                insert_effect_draft_in(
                    tx,
                    command.conversation_id,
                    turn_id,
                    None,
                    &CompanionTurnEffectSeed::default(),
                    now,
                )
                .map_err(effect_conversation_error)
            },
        )
        .map_err(CompanionContinueRepositoryError::Conversation)
    }
}

fn effect_failure(_: impl std::fmt::Debug) -> CompanionTurnEffectRepositoryError {
    CompanionTurnEffectRepositoryError::Failure
}

fn effect_corrupt(_: impl std::fmt::Debug) -> CompanionTurnEffectRepositoryError {
    CompanionTurnEffectRepositoryError::Corrupt
}

fn effect_conversation_error(
    error: CompanionTurnEffectRepositoryError,
) -> lettuce_conversations::ConversationRepositoryError {
    match error {
        CompanionTurnEffectRepositoryError::Conflict => {
            lettuce_conversations::ConversationRepositoryError::Conflict
        }
        _ => lettuce_conversations::ConversationRepositoryError::Storage,
    }
}

fn insert_effect_vector(
    tx: &Transaction<'_>,
    conversation_id: ConversationId,
    turn_id: GenerationTurnId,
    kind: &str,
    vector: &EmotionVector,
) -> Result<(), CompanionTurnEffectRepositoryError> {
    let values = vector_values(vector);
    tx.execute(
        "INSERT INTO companion_turn_effect_emotion_deltas (
           conversation_id, turn_id, kind, warmth, trust, calm, vulnerability, longing,
           hurt, tension, irritation, affection_intensity, reassurance_need
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            conversation_id.to_string(),
            turn_id.to_string(),
            kind,
            values[0],
            values[1],
            values[2],
            values[3],
            values[4],
            values[5],
            values[6],
            values[7],
            values[8],
            values[9]
        ],
    )
    .map_err(effect_failure)?;
    Ok(())
}

pub(crate) fn insert_effect_draft_in(
    tx: &Transaction<'_>,
    conversation_id: ConversationId,
    turn_id: GenerationTurnId,
    user_message_id: Option<MessageId>,
    seed: &CompanionTurnEffectSeed,
    now: TimestampMillis,
) -> Result<(), CompanionTurnEffectRepositoryError> {
    seed.validate()?;
    let effect_id = CompanionEffectId::new();
    let delta = &seed.relationship_delta;
    tx.execute(
        "INSERT INTO companion_turn_effect_drafts (
           conversation_id, turn_id, effect_id, user_message_id, closeness_delta,
           trust_delta, affection_delta, tension_delta, stability_delta, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            conversation_id.to_string(),
            turn_id.to_string(),
            effect_id.to_string(),
            user_message_id.map(|value| value.to_string()),
            delta.closeness,
            delta.trust,
            delta.affection,
            delta.tension,
            delta.stability,
            now.get()
        ],
    )
    .map_err(effect_failure)?;
    for (kind, vector) in [
        ("felt", &seed.emotion_delta.felt),
        ("expressed", &seed.emotion_delta.expressed),
        ("blocked", &seed.emotion_delta.blocked),
    ] {
        insert_effect_vector(tx, conversation_id, turn_id, kind, vector)?;
    }
    for (change_kind, values) in [
        ("added", &seed.signal_changes.added),
        ("removed", &seed.signal_changes.removed),
    ] {
        for (ordinal, value) in values.iter().enumerate() {
            tx.execute(
                "INSERT INTO companion_turn_effect_signal_changes
                   (conversation_id, turn_id, change_kind, ordinal, value)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    conversation_id.to_string(),
                    turn_id.to_string(),
                    change_kind,
                    i64::try_from(ordinal).map_err(effect_corrupt)?,
                    value
                ],
            )
            .map_err(effect_failure)?;
        }
    }
    Ok(())
}

pub(crate) fn finalize_turn_effect_in(
    tx: &Transaction<'_>,
    conversation_id: ConversationId,
    turn_id: GenerationTurnId,
    assistant_message_id: MessageId,
    now: TimestampMillis,
) -> Result<(), lettuce_conversations::ConversationRepositoryError> {
    let draft = tx
        .query_row(
            "SELECT effect_id, user_message_id FROM companion_turn_effect_drafts
             WHERE conversation_id = ?1 AND turn_id = ?2",
            params![conversation_id.to_string(), turn_id.to_string()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .optional()
        .map_err(|_| lettuce_conversations::ConversationRepositoryError::Storage)?;
    let Some((effect_id, user_message_id)) = draft else {
        return Ok(());
    };
    tx.execute(
        "INSERT INTO companion_turn_effects (
           id, conversation_id, turn_id, user_message_id, assistant_message_id,
           status, summary, enqueued_at, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, 'processing', NULL, NULL, ?6, ?6)",
        params![
            effect_id,
            conversation_id.to_string(),
            turn_id.to_string(),
            user_message_id,
            assistant_message_id.to_string(),
            now.get()
        ],
    )
    .map_err(|_| lettuce_conversations::ConversationRepositoryError::Storage)?;
    Ok(())
}

pub(crate) fn discard_turn_effect_draft_in(
    tx: &Transaction<'_>,
    conversation_id: ConversationId,
    turn_id: GenerationTurnId,
) -> Result<(), lettuce_conversations::ConversationRepositoryError> {
    tx.execute(
        "DELETE FROM companion_turn_effect_drafts
         WHERE conversation_id = ?1 AND turn_id = ?2
           AND NOT EXISTS (
             SELECT 1 FROM companion_turn_effects
             WHERE conversation_id = ?1 AND turn_id = ?2
           )",
        params![conversation_id.to_string(), turn_id.to_string()],
    )
    .map_err(|_| lettuce_conversations::ConversationRepositoryError::Storage)?;
    Ok(())
}

fn load_effect_vector(
    connection: &Connection,
    conversation_id: ConversationId,
    turn_id: GenerationTurnId,
    kind: &str,
) -> Result<EmotionVector, CompanionTurnEffectRepositoryError> {
    connection
        .query_row(
            "SELECT warmth, trust, calm, vulnerability, longing, hurt, tension,
                    irritation, affection_intensity, reassurance_need
             FROM companion_turn_effect_emotion_deltas
             WHERE conversation_id = ?1 AND turn_id = ?2 AND kind = ?3",
            params![conversation_id.to_string(), turn_id.to_string(), kind],
            |row| {
                Ok(EmotionVector {
                    warmth: row.get(0)?,
                    trust: row.get(1)?,
                    calm: row.get(2)?,
                    vulnerability: row.get(3)?,
                    longing: row.get(4)?,
                    hurt: row.get(5)?,
                    tension: row.get(6)?,
                    irritation: row.get(7)?,
                    affection_intensity: row.get(8)?,
                    reassurance_need: row.get(9)?,
                })
            },
        )
        .map_err(effect_corrupt)
}

fn load_effect_strings(
    connection: &Connection,
    sql: &str,
    id: &str,
    kind: &str,
) -> Result<Vec<String>, CompanionTurnEffectRepositoryError> {
    let mut statement = connection.prepare(sql).map_err(effect_failure)?;
    statement
        .query_map(params![id, kind], |row| row.get(0))
        .map_err(effect_failure)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(effect_corrupt)
}

fn load_source_messages(
    connection: &Connection,
    effect_id: CompanionEffectId,
) -> Result<Vec<String>, CompanionTurnEffectRepositoryError> {
    let mut statement = connection
        .prepare(
            "SELECT message_id FROM companion_turn_effect_source_messages
             WHERE effect_id = ?1 ORDER BY ordinal",
        )
        .map_err(effect_failure)?;
    statement
        .query_map([effect_id.to_string()], |row| row.get(0))
        .map_err(effect_failure)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(effect_corrupt)
}

fn load_effect(
    connection: &Connection,
    conversation_id: ConversationId,
    assistant_message_id: MessageId,
) -> Result<Option<CompanionTurnEffect>, CompanionTurnEffectRepositoryError> {
    let row = connection
        .query_row(
            "SELECT e.id, e.turn_id, e.user_message_id, e.status, e.summary,
                    e.enqueued_at, e.created_at, e.updated_at,
                    d.closeness_delta, d.trust_delta, d.affection_delta,
                    d.tension_delta, d.stability_delta
             FROM companion_turn_effects e
             JOIN companion_turn_effect_drafts d
               ON d.conversation_id = e.conversation_id AND d.turn_id = e.turn_id
              AND d.effect_id = e.id
             WHERE e.conversation_id = ?1 AND e.assistant_message_id = ?2",
            params![
                conversation_id.to_string(),
                assistant_message_id.to_string()
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, f64>(8)?,
                    row.get::<_, f64>(9)?,
                    row.get::<_, f64>(10)?,
                    row.get::<_, f64>(11)?,
                    row.get::<_, f64>(12)?,
                ))
            },
        )
        .optional()
        .map_err(effect_failure)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let id = CompanionEffectId::from_str(&row.0).map_err(effect_corrupt)?;
    let turn_id = GenerationTurnId::from_str(&row.1).map_err(effect_corrupt)?;
    let user_message_id = row
        .2
        .map(|value| MessageId::from_str(&value).map_err(effect_corrupt))
        .transpose()?;
    let stored_status = match row.3.as_str() {
        "processing" => CompanionTurnEffectStatus::Processing,
        "ready" => CompanionTurnEffectStatus::Ready,
        "failed" => CompanionTurnEffectStatus::Failed,
        _ => return Err(CompanionTurnEffectRepositoryError::Corrupt),
    };
    let signal_sql = "SELECT value FROM companion_turn_effect_signal_changes
                      WHERE conversation_id = ?1 AND turn_id = ?2 AND change_kind = ?3
                      ORDER BY ordinal";
    let mut signal_statement = connection.prepare(signal_sql).map_err(effect_failure)?;
    let mut load_signals = |kind: &str| -> Result<Vec<String>, CompanionTurnEffectRepositoryError> {
        signal_statement
            .query_map(
                params![conversation_id.to_string(), turn_id.to_string(), kind],
                |row| row.get(0),
            )
            .map_err(effect_failure)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(effect_corrupt)
    };
    let added_signals = load_signals("added")?;
    let removed_signals = load_signals("removed")?;
    drop(signal_statement);
    let memory_sql = "SELECT memory_id FROM companion_turn_effect_memory_changes
                      WHERE effect_id = ?1 AND change_kind = ?2 ORDER BY ordinal";
    let parse_memories = |kind: &str| -> Result<Vec<MemoryId>, CompanionTurnEffectRepositoryError> {
        load_effect_strings(connection, memory_sql, &id.to_string(), kind)?
            .into_iter()
            .map(|value| MemoryId::from_str(&value).map_err(effect_corrupt))
            .collect()
    };
    let source_window = row
        .5
        .map(|enqueued_at| {
            load_source_messages(connection, id)?
                .into_iter()
                .map(|value| MessageId::from_str(&value).map_err(effect_corrupt))
                .collect::<Result<Vec<_>, _>>()
                .map(|message_ids| CompanionEffectSourceWindow {
                    message_ids,
                    enqueued_at: TimestampMillis::new(enqueued_at),
                })
        })
        .transpose()?;
    let mut effect = CompanionTurnEffect {
        id,
        conversation_id,
        turn_id,
        user_message_id,
        assistant_message_id,
        status: stored_status,
        summary: row.4,
        seed: CompanionTurnEffectSeed {
            relationship_delta: RelationshipDelta {
                closeness: row.8,
                trust: row.9,
                affection: row.10,
                tension: row.11,
                stability: row.12,
            },
            emotion_delta: CompanionEmotionDelta {
                felt: load_effect_vector(connection, conversation_id, turn_id, "felt")?,
                expressed: load_effect_vector(connection, conversation_id, turn_id, "expressed")?,
                blocked: load_effect_vector(connection, conversation_id, turn_id, "blocked")?,
            },
            signal_changes: CompanionSignalChanges {
                added: added_signals,
                removed: removed_signals,
            },
        },
        memory_changes: CompanionMemoryChanges {
            added: parse_memories("added")?,
            updated: parse_memories("updated")?,
            superseded: parse_memories("superseded")?,
        },
        source_window,
        created_at: TimestampMillis::new(row.6),
        updated_at: TimestampMillis::new(row.7),
    };
    effect.seed.validate()?;
    if effect.updated_at < effect.created_at {
        return Err(CompanionTurnEffectRepositoryError::Corrupt);
    }
    match stored_status {
        CompanionTurnEffectStatus::Processing
            if effect.summary.is_none()
                && effect.memory_changes == CompanionMemoryChanges::default()
                && effect.source_window.is_none() => {}
        CompanionTurnEffectStatus::Ready => CompanionTurnEffectOutcome::Ready {
            summary: effect.summary.clone(),
            memory_changes: effect.memory_changes.clone(),
            source_window: effect
                .source_window
                .clone()
                .ok_or(CompanionTurnEffectRepositoryError::Corrupt)?,
        }
        .validate()?,
        CompanionTurnEffectStatus::Failed
            if effect.memory_changes == CompanionMemoryChanges::default()
                && effect.source_window.is_none() =>
        {
            CompanionTurnEffectOutcome::Failed {
                summary: effect
                    .summary
                    .clone()
                    .ok_or(CompanionTurnEffectRepositoryError::Corrupt)?,
            }
            .validate()?;
        }
        _ => return Err(CompanionTurnEffectRepositoryError::Corrupt),
    }
    if connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM companion_turn_effect_invalidations WHERE effect_id=?1)",
            [id.to_string()],
            |row| row.get::<_, bool>(0),
        )
        .map_err(effect_failure)?
    {
        effect.status = CompanionTurnEffectStatus::Invalidated;
    }
    Ok(Some(effect))
}

impl CompanionTurnEffectRepository for Database {
    fn get_for_message(
        &self,
        conversation_id: ConversationId,
        assistant_message_id: MessageId,
    ) -> Result<Option<CompanionTurnEffect>, CompanionTurnEffectRepositoryError> {
        let connection = self.connection().map_err(effect_failure)?;
        load_effect(&connection, conversation_id, assistant_message_id)
    }

    fn list_processing(
        &self,
        limit: u16,
    ) -> Result<Vec<CompanionTurnEffect>, CompanionTurnEffectRepositoryError> {
        if limit == 0 || limit > 512 {
            return Err(CompanionTurnEffectRepositoryError::Invalid);
        }
        let connection = self.connection().map_err(effect_failure)?;
        let identities = {
            let mut statement = connection
                .prepare(
                    "SELECT conversation_id, assistant_message_id
                     FROM companion_turn_effects
                     WHERE status = 'processing'
                       AND NOT EXISTS (
                           SELECT 1 FROM companion_turn_effect_invalidations invalidation
                           WHERE invalidation.effect_id = companion_turn_effects.id
                       )
                     ORDER BY conversation_id, created_at, id
                     LIMIT ?1",
                )
                .map_err(effect_failure)?;
            statement
                .query_map([i64::from(limit)], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(effect_failure)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(effect_failure)?
        };
        identities
            .into_iter()
            .map(|(conversation_id, assistant_message_id)| {
                let conversation_id =
                    ConversationId::from_str(&conversation_id).map_err(effect_corrupt)?;
                let assistant_message_id =
                    MessageId::from_str(&assistant_message_id).map_err(effect_corrupt)?;
                load_effect(&connection, conversation_id, assistant_message_id)?
                    .ok_or(CompanionTurnEffectRepositoryError::Corrupt)
            })
            .collect()
    }

    fn list_for_conversation(
        &self,
        conversation_id: ConversationId,
        limit: u16,
    ) -> Result<Vec<CompanionTurnEffect>, CompanionTurnEffectRepositoryError> {
        if limit == 0 || limit > 512 {
            return Err(CompanionTurnEffectRepositoryError::Invalid);
        }
        let connection = self.connection().map_err(effect_failure)?;
        let assistant_ids = {
            let mut statement = connection
                .prepare(
                    "SELECT assistant_message_id FROM companion_turn_effects
                     WHERE conversation_id=?1 ORDER BY created_at, id LIMIT ?2",
                )
                .map_err(effect_failure)?;
            statement
                .query_map(
                    params![conversation_id.to_string(), i64::from(limit)],
                    |row| row.get::<_, String>(0),
                )
                .map_err(effect_failure)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(effect_failure)?
        };
        assistant_ids
            .into_iter()
            .map(|assistant_message_id| {
                let assistant_message_id =
                    MessageId::from_str(&assistant_message_id).map_err(effect_corrupt)?;
                load_effect(&connection, conversation_id, assistant_message_id)?
                    .ok_or(CompanionTurnEffectRepositoryError::Corrupt)
            })
            .collect()
    }

    fn settle(
        &self,
        effect_id: CompanionEffectId,
        outcome: CompanionTurnEffectOutcome,
        now: TimestampMillis,
    ) -> Result<CompanionTurnEffect, CompanionTurnEffectRepositoryError> {
        outcome.validate()?;
        let mut connection = self.connection().map_err(effect_failure)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(effect_failure)?;
        let (conversation_id, user_message_id, assistant_message_id, created_at, current_status) =
            tx.query_row(
                "SELECT conversation_id, user_message_id, assistant_message_id, created_at, status
                 FROM companion_turn_effects
                 WHERE id = ?1",
                [effect_id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .optional()
            .map_err(effect_failure)?
            .ok_or(CompanionTurnEffectRepositoryError::NotFound)?;
        if tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM companion_turn_effect_invalidations WHERE effect_id=?1)",
                [effect_id.to_string()],
                |row| row.get::<_, bool>(0),
            )
            .map_err(effect_failure)?
        {
            return Err(CompanionTurnEffectRepositoryError::Conflict);
        }
        if current_status != "processing" {
            let conversation_id =
                ConversationId::from_str(&conversation_id).map_err(effect_corrupt)?;
            let assistant_message_id =
                MessageId::from_str(&assistant_message_id).map_err(effect_corrupt)?;
            let existing = load_effect(&tx, conversation_id, assistant_message_id)?
                .ok_or(CompanionTurnEffectRepositoryError::Corrupt)?;
            let exact = match (&existing.status, &outcome) {
                (
                    CompanionTurnEffectStatus::Ready,
                    CompanionTurnEffectOutcome::Ready {
                        summary,
                        memory_changes,
                        source_window,
                    },
                ) => {
                    existing.summary == *summary
                        && existing.memory_changes == *memory_changes
                        && existing.source_window.as_ref() == Some(source_window)
                }
                (
                    CompanionTurnEffectStatus::Failed,
                    CompanionTurnEffectOutcome::Failed { summary },
                ) => existing.summary.as_ref() == Some(summary),
                _ => false,
            };
            if !exact {
                return Err(CompanionTurnEffectRepositoryError::Conflict);
            }
            tx.commit().map_err(effect_failure)?;
            return Ok(existing);
        }
        if now < TimestampMillis::new(created_at) {
            return Err(CompanionTurnEffectRepositoryError::Invalid);
        }
        let (status, summary, memory_changes, source_window) = match outcome {
            CompanionTurnEffectOutcome::Ready {
                summary,
                memory_changes,
                source_window,
            } => ("ready", summary, Some(memory_changes), Some(source_window)),
            CompanionTurnEffectOutcome::Failed { summary } => ("failed", Some(summary), None, None),
        };
        if let Some(window) = &source_window {
            let expected = user_message_id
                .iter()
                .cloned()
                .chain(std::iter::once(assistant_message_id.clone()))
                .collect::<std::collections::BTreeSet<_>>();
            let actual = window
                .message_ids
                .iter()
                .map(ToString::to_string)
                .collect::<std::collections::BTreeSet<_>>();
            if actual.len() != window.message_ids.len()
                || actual != expected
                || window.enqueued_at > now
            {
                return Err(CompanionTurnEffectRepositoryError::Invalid);
            }
        }
        let enqueued_at = source_window.as_ref().map(|value| value.enqueued_at.get());
        if let Some(changes) = memory_changes {
            for (kind, values) in [
                ("added", changes.added),
                ("updated", changes.updated),
                ("superseded", changes.superseded),
            ] {
                for (ordinal, memory_id) in values.into_iter().enumerate() {
                    tx.execute(
                        "INSERT INTO companion_turn_effect_memory_changes
                           (effect_id, change_kind, ordinal, memory_id) VALUES (?1, ?2, ?3, ?4)",
                        params![
                            effect_id.to_string(),
                            kind,
                            i64::try_from(ordinal).map_err(effect_corrupt)?,
                            memory_id.to_string()
                        ],
                    )
                    .map_err(effect_failure)?;
                }
            }
        }
        if let Some(window) = source_window {
            for (ordinal, message_id) in window.message_ids.into_iter().enumerate() {
                tx.execute(
                    "INSERT INTO companion_turn_effect_source_messages
                       (effect_id, ordinal, message_id) VALUES (?1, ?2, ?3)",
                    params![
                        effect_id.to_string(),
                        i64::try_from(ordinal).map_err(effect_corrupt)?,
                        message_id.to_string()
                    ],
                )
                .map_err(effect_failure)?;
            }
        }
        tx.execute(
            "UPDATE companion_turn_effects
             SET status = ?2, summary = ?3, enqueued_at = ?4, updated_at = ?5
             WHERE id = ?1 AND status = 'processing'",
            params![
                effect_id.to_string(),
                status,
                summary,
                enqueued_at,
                now.get()
            ],
        )
        .map_err(effect_failure)?;
        let conversation_id = ConversationId::from_str(&conversation_id).map_err(effect_corrupt)?;
        let assistant_message_id =
            MessageId::from_str(&assistant_message_id).map_err(effect_corrupt)?;
        let effect = load_effect(&tx, conversation_id, assistant_message_id)?
            .ok_or(CompanionTurnEffectRepositoryError::Corrupt)?;
        tx.commit().map_err(effect_failure)?;
        Ok(effect)
    }
}

#[cfg(test)]
mod tests {
    use lettuce_companions::{
        CompanionStateOwner, CompanionStateReplacement, CompanionStateRepository,
        CompanionStateRepositoryError, EmotionVector, RegulationStyle, RelationshipDefaults,
        initial_runtime_state,
    };
    use lettuce_types::{
        CharacterId, ConversationBranchId, ConversationId, OperationRecordId, PersonaId, Revision,
        TimestampMillis,
    };
    use rusqlite::{TransactionBehavior, params};

    use crate::Database;

    fn insert_character(database: &Database, character_id: CharacterId) {
        let connection = database.connection().expect("database lock");
        connection
            .execute(
                "INSERT INTO characters (
                   id, status, name, normalized_name, profile_json, provenance_json,
                   defaults_json, interaction_mode, memory_policy, voice_autoplay,
                   presentation_json, revision, created_at, updated_at
                 ) VALUES (?1, 'active', 'Companion', 'companion', '{}', '{}', '{}',
                           'companion', 'dynamic', 0, '{}', 1, 1, 1)",
                [character_id.to_string()],
            )
            .expect("insert character");
    }

    fn insert_conversation(
        database: &Database,
        conversation_id: ConversationId,
        character_id: CharacterId,
    ) {
        let mut connection = database.connection().expect("database lock");
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("conversation transaction");
        let branch_id = ConversationBranchId::new();
        tx.execute(
            "INSERT INTO conversations (
               id, kind, lifecycle, title, active_branch_id, kind_json, revision,
               created_at, updated_at
             ) VALUES (?1, 'direct', 'active', 'Companion', ?2,
                       '{\"format_version\":1,\"value\":null}', 1, 1, 1)",
            params![conversation_id.to_string(), branch_id.to_string()],
        )
        .expect("insert conversation");
        tx.execute(
            "INSERT INTO conversation_branches (
               conversation_id, id, status, revision, created_at, updated_at
             ) VALUES (?1, ?2, 'active', 1, 1, 1)",
            params![conversation_id.to_string(), branch_id.to_string()],
        )
        .expect("insert branch");
        tx.execute(
            "INSERT INTO conversation_participants (
               conversation_id, id, role, ordinal, source_kind, source_id, enabled, muted,
               display_name, model_selection_json, revision, created_at, updated_at
             ) VALUES (?1, ?2, 'character', 0, 'character', ?3, 1, 0, 'Companion',
                       '{\"format_version\":1,\"value\":null}', 1, 1, 1)",
            params![
                conversation_id.to_string(),
                lettuce_types::ConversationParticipantId::new().to_string(),
                character_id.to_string()
            ],
        )
        .expect("insert participant");
        tx.commit().expect("commit conversation");
    }

    fn owner(
        conversation_id: ConversationId,
        character_id: CharacterId,
        persona_id: Option<PersonaId>,
    ) -> CompanionStateOwner {
        CompanionStateOwner {
            conversation_id,
            character_id,
            persona_id,
        }
    }

    fn initial(
        emotional_calm: f64,
        relationship_trust: f64,
    ) -> lettuce_companions::CompanionRuntimeState {
        let baseline = EmotionVector {
            calm: emotional_calm,
            ..EmotionVector::default()
        };
        let relationship = RelationshipDefaults {
            trust: relationship_trust,
            ..RelationshipDefaults::default()
        };
        initial_runtime_state(&baseline, &RegulationStyle::default(), &relationship)
    }

    #[test]
    fn relationship_carries_across_sessions_per_persona_while_emotion_stays_local() {
        let database = Database::open_in_memory().expect("open database");
        let character_id = CharacterId::new();
        let persona_a = PersonaId::new();
        let persona_b = PersonaId::new();
        let first_id = ConversationId::new();
        let second_id = ConversationId::new();
        let other_id = ConversationId::new();
        insert_character(&database, character_id);
        for id in [first_id, second_id, other_id] {
            insert_conversation(&database, id, character_id);
        }

        let first = database
            .create(
                owner(first_id, character_id, Some(persona_a)),
                initial(0.2, 0.8),
                TimestampMillis::new(10),
            )
            .expect("create first state");
        assert_eq!(
            database.create(
                owner(first_id, character_id, Some(persona_a)),
                initial(0.2, 0.8),
                TimestampMillis::new(10),
            ),
            Ok(first.clone())
        );
        assert_eq!(
            database.create(
                owner(first_id, character_id, Some(persona_a)),
                initial(0.3, 0.8),
                TimestampMillis::new(10),
            ),
            Err(CompanionStateRepositoryError::AlreadyExists)
        );
        let second = database
            .create(
                owner(second_id, character_id, Some(persona_a)),
                initial(0.9, 0.1),
                TimestampMillis::new(20),
            )
            .expect("create second state");
        let other = database
            .create(
                owner(other_id, character_id, Some(persona_b)),
                initial(0.7, -0.3),
                TimestampMillis::new(30),
            )
            .expect("create other state");

        assert_eq!(first.state.emotional_state.felt.calm, 0.2);
        assert_eq!(second.state.emotional_state.felt.calm, 0.9);
        assert_eq!(second.state.relationship_state.trust, 0.8);
        assert_eq!(other.state.relationship_state.trust, -0.3);
    }

    #[test]
    fn atomic_replace_is_retryable_and_visible_to_same_persona_sessions() {
        let database = Database::open_in_memory().expect("open database");
        let character_id = CharacterId::new();
        let persona_id = PersonaId::new();
        let first_id = ConversationId::new();
        let second_id = ConversationId::new();
        insert_character(&database, character_id);
        insert_conversation(&database, first_id, character_id);
        insert_conversation(&database, second_id, character_id);
        let first_owner = owner(first_id, character_id, Some(persona_id));
        let second_owner = owner(second_id, character_id, Some(persona_id));
        let first = database
            .create(first_owner, initial(0.2, 0.1), TimestampMillis::new(10))
            .expect("create first");
        database
            .create(second_owner, initial(0.9, 0.9), TimestampMillis::new(20))
            .expect("create second");

        let mut next = first.state.clone();
        next.emotional_state.felt.calm = 0.4;
        next.relationship_state.trust = 0.7;
        next.updated_at = TimestampMillis::new(40);
        next.emotional_state.updated_at = TimestampMillis::new(40);
        next.relationship_state.last_interaction_at = TimestampMillis::new(40);
        let replacement = CompanionStateReplacement {
            expected_session_revision: first.session_revision,
            expected_relationship_revision: first.relationship_revision,
            state: next,
            applied_at: TimestampMillis::new(40),
        };
        let operation_id = OperationRecordId::new();
        let receipt = database
            .replace(first_owner, operation_id, replacement.clone())
            .expect("replace state");
        assert_eq!(
            database.replace(first_owner, operation_id, replacement.clone()),
            Ok(receipt)
        );

        let first_after = database
            .get(first_owner)
            .expect("load first")
            .expect("first state");
        let second_after = database
            .get(second_owner)
            .expect("load second")
            .expect("second state");
        assert_eq!(first_after.state.emotional_state.felt.calm, 0.4);
        assert_eq!(second_after.state.emotional_state.felt.calm, 0.9);
        assert_eq!(second_after.state.relationship_state.trust, 0.7);

        let mut changed = replacement;
        changed.state.relationship_state.trust = 0.6;
        assert_eq!(
            database.replace(first_owner, operation_id, changed),
            Err(CompanionStateRepositoryError::OperationMismatch)
        );
    }

    #[test]
    fn stale_relationship_revision_rolls_back_session_emotion() {
        let database = Database::open_in_memory().expect("open database");
        let character_id = CharacterId::new();
        let conversation_id = ConversationId::new();
        insert_character(&database, character_id);
        insert_conversation(&database, conversation_id, character_id);
        let owner = owner(conversation_id, character_id, None);
        let initial = database
            .create(owner, initial(0.2, 0.1), TimestampMillis::new(10))
            .expect("create state");
        let mut changed = initial.state.clone();
        changed.emotional_state.felt.calm = 0.8;
        let replacement = CompanionStateReplacement {
            expected_session_revision: initial.session_revision,
            expected_relationship_revision: Revision::new(2),
            state: changed,
            applied_at: TimestampMillis::new(20),
        };
        assert_eq!(
            database.replace(owner, OperationRecordId::new(), replacement),
            Err(CompanionStateRepositoryError::Conflict)
        );
        let loaded = database.get(owner).expect("load").expect("state");
        assert_eq!(loaded.state.emotional_state.felt.calm, 0.2);
        assert_eq!(loaded.session_revision, Revision::INITIAL);
    }

    #[test]
    fn two_database_handles_enforce_both_runtime_revisions() {
        let path = std::env::temp_dir().join(format!(
            "lettuce-companion-state-cas-{}.db",
            OperationRecordId::new()
        ));
        let first = Database::open(&path).expect("first database");
        let character_id = CharacterId::new();
        let conversation_id = ConversationId::new();
        insert_character(&first, character_id);
        insert_conversation(&first, conversation_id, character_id);
        let second = Database::open(&path).expect("second database");
        let owner = owner(conversation_id, character_id, None);
        let snapshot = first
            .create(owner, initial(0.2, 0.1), TimestampMillis::new(10))
            .expect("create state");
        let stale = second.get(owner).expect("load stale").expect("state");

        let mut first_state = snapshot.state.clone();
        first_state.relationship_state.trust = 0.6;
        first
            .replace(
                owner,
                OperationRecordId::new(),
                CompanionStateReplacement {
                    expected_session_revision: snapshot.session_revision,
                    expected_relationship_revision: snapshot.relationship_revision,
                    state: first_state,
                    applied_at: TimestampMillis::new(20),
                },
            )
            .expect("first replace");
        let stale_session_revision = stale.session_revision;
        let stale_relationship_revision = stale.relationship_revision;
        let mut stale_state = stale.state;
        stale_state.emotional_state.felt.calm = 0.8;
        assert_eq!(
            second.replace(
                owner,
                OperationRecordId::new(),
                CompanionStateReplacement {
                    expected_session_revision: stale_session_revision,
                    expected_relationship_revision: stale_relationship_revision,
                    state: stale_state,
                    applied_at: TimestampMillis::new(21),
                },
            ),
            Err(CompanionStateRepositoryError::Conflict)
        );
        drop(first);
        drop(second);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn missing_normalized_vector_fails_closed() {
        let database = Database::open_in_memory().expect("open database");
        let character_id = CharacterId::new();
        let conversation_id = ConversationId::new();
        insert_character(&database, character_id);
        insert_conversation(&database, conversation_id, character_id);
        let owner = owner(conversation_id, character_id, None);
        database
            .create(owner, initial(0.2, 0.1), TimestampMillis::new(10))
            .expect("create state");
        database
            .connection()
            .expect("database lock")
            .execute(
                "DELETE FROM companion_emotion_vectors
                 WHERE conversation_id = ?1 AND kind = 'felt'",
                [conversation_id.to_string()],
            )
            .expect("delete vector");
        assert_eq!(
            database.get(owner),
            Err(CompanionStateRepositoryError::Corrupt)
        );
    }
}
