use std::str::FromStr;

use blake3::Hasher;
use lettuce_companions::{
    CompanionRuntimeState, CompanionStateApplyReceipt, CompanionStateOwner,
    CompanionStateReplacement, CompanionStateRepository, CompanionStateRepositoryError,
    CompanionStateSnapshot, EmotionVector, EmotionalState, RelationshipState,
    validate_runtime_state,
};
use lettuce_types::{
    CharacterId, ConversationId, OperationRecordId, PersonaId, Revision, TimestampMillis,
};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::Database;

type Error = CompanionStateRepositoryError;

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

impl CompanionStateRepository for Database {
    fn create(
        &self,
        owner: CompanionStateOwner,
        initial: CompanionRuntimeState,
        now: TimestampMillis,
    ) -> Result<CompanionStateSnapshot, Error> {
        validate_runtime_state(&initial)?;
        let initial_hash = state_hash(&initial);
        let mut connection = self.connection().map_err(failure)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
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
        if let Some(existing) = get_in(&tx, owner)? {
            let stored_hash = tx
                .query_row(
                    "SELECT initial_hash FROM companion_session_states WHERE conversation_id = ?1",
                    [owner.conversation_id.to_string()],
                    |row| row.get::<_, Vec<u8>>(0),
                )
                .map_err(corrupt)?;
            if stored_hash.as_slice() != initial_hash {
                return Err(Error::AlreadyExists);
            }
            tx.commit().map_err(failure)?;
            return Ok(existing);
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
        .map_err(|error| {
            if error.to_string().contains("UNIQUE constraint failed") {
                Error::AlreadyExists
            } else {
                failure(error)
            }
        })?;
        replace_vectors(&tx, owner.conversation_id, emotional)?;
        replace_signals(
            &tx,
            owner.conversation_id,
            emotional,
            &initial.active_signals,
        )?;
        let snapshot = get_in(&tx, owner)?.ok_or(Error::Corrupt)?;
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
        let current = get_in(&tx, owner)?.ok_or(Error::NotFound)?;
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
                    owner.character_id.to_string(), key, relationship.closeness, relationship.trust,
                    relationship.affection, relationship.tension, relationship.stability,
                    i64::from(relationship.interaction_count), relationship.last_interaction_at.get(),
                    sql_revision(next_relationship)?, replacement.applied_at.get(),
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
        replace_vectors(&tx, owner.conversation_id, emotional)?;
        replace_signals(
            &tx,
            owner.conversation_id,
            emotional,
            &replacement.state.active_signals,
        )?;
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
                sql_revision(current.session_revision)?,
                sql_revision(next_session)?,
                sql_revision(current.relationship_revision)?,
                sql_revision(next_relationship)?,
                replacement.applied_at.get(),
                hash.as_slice()
            ],
        )
        .map_err(failure)?;
        let receipt = CompanionStateApplyReceipt {
            operation_id,
            owner,
            expected_session_revision: current.session_revision,
            resulting_session_revision: next_session,
            expected_relationship_revision: current.relationship_revision,
            resulting_relationship_revision: next_relationship,
            applied_at: replacement.applied_at,
        };
        tx.commit().map_err(failure)?;
        Ok(receipt)
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
