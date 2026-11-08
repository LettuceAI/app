use std::collections::HashSet;
use std::str::FromStr;

use lettuce_companions::{
    EmotionalState, GroupCompanionAuthor, GroupCompanionCheckpoint, GroupCompanionEffect,
    GroupCompanionEvent, GroupCompanionMember, GroupCompanionMessageSource,
    GroupCompanionRepository, GroupCompanionRepositoryError, GroupCompanionTurnWriter,
    GroupMemberRelationship, PreparedGroupCompanionTurn, RelationshipState, reduce_group_event,
};
use lettuce_conversations::{
    ContinueConversation, ContinueConversationResult, ConversationRepositoryError,
    SendConversation, SendConversationResult,
};
use lettuce_types::{
    CharacterId, ConversationId, MessageCandidateId, MessageId, MessageRevisionId, PersonaId,
    Revision, TimestampMillis,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::Database;

type Error = GroupCompanionRepositoryError;

fn failure(error: impl std::fmt::Debug) -> Error {
    Error::Failure {
        reason: format!("{error:?}"),
    }
}

fn corrupt(error: impl std::fmt::Debug) -> Error {
    Error::Corrupt {
        reason: format!("{error:?}"),
    }
}

fn invalid(reason: &str) -> Error {
    Error::Invalid {
        reason: reason.to_owned(),
    }
}

fn parse_revision(value: i64) -> Result<Revision, Error> {
    let value = u64::try_from(value).map_err(corrupt)?;
    (value > 0)
        .then(|| Revision::new(value))
        .ok_or_else(|| Error::Corrupt {
            reason: "a stored companion revision is zero".to_owned(),
        })
}

fn persona_key(persona_id: Option<PersonaId>) -> String {
    persona_id
        .map(|value| value.to_string())
        .unwrap_or_else(|| "__default__".to_owned())
}

type CheckpointRow = (
    String,
    String,
    Option<String>,
    Option<String>,
    String,
    String,
    i64,
);

fn load_checkpoint(
    connection: &Connection,
    conversation_id: ConversationId,
    row: CheckpointRow,
) -> Result<GroupCompanionCheckpoint, Error> {
    let message_id = MessageId::from_str(&row.0).map_err(corrupt)?;
    let character_id = CharacterId::from_str(&row.1).map_err(corrupt)?;
    let source = GroupCompanionMessageSource {
        revision_id: row
            .2
            .map(|value| MessageRevisionId::from_str(&value))
            .transpose()
            .map_err(corrupt)?,
        candidate_id: row
            .3
            .map(|value| MessageCandidateId::from_str(&value))
            .transpose()
            .map_err(corrupt)?,
    };
    let emotional_state: EmotionalState = serde_json::from_str(&row.4).map_err(corrupt)?;
    let member_relationships: Vec<GroupMemberRelationship> =
        serde_json::from_str(&row.5).map_err(corrupt)?;
    let effects = load_effects(connection, conversation_id, message_id, character_id)?;
    Ok(GroupCompanionCheckpoint {
        message_id,
        character_id,
        source,
        emotional_state,
        member_relationships,
        effects,
        created_at: TimestampMillis::new(row.6),
    })
}

fn load_effects(
    connection: &Connection,
    conversation_id: ConversationId,
    message_id: MessageId,
    character_id: CharacterId,
) -> Result<Vec<GroupCompanionEffect>, Error> {
    let mut statement = connection
        .prepare(
            "SELECT effect_json FROM group_companion_effects
             WHERE conversation_id = ?1 AND message_id = ?2 AND character_id = ?3
             ORDER BY ordinal",
        )
        .map_err(failure)?;
    let values = statement
        .query_map(
            params![
                conversation_id.to_string(),
                message_id.to_string(),
                character_id.to_string()
            ],
            |row| row.get::<_, String>(0),
        )
        .map_err(failure)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(failure)?;
    values
        .into_iter()
        .map(|value| serde_json::from_str(&value).map_err(corrupt))
        .collect()
}

fn load_checkpoints(
    connection: &Connection,
    conversation_id: ConversationId,
    rows: Vec<CheckpointRow>,
) -> Result<Vec<GroupCompanionCheckpoint>, Error> {
    rows.into_iter()
        .map(|row| load_checkpoint(connection, conversation_id, row))
        .collect()
}

pub(crate) fn load_checkpoints_for_message_in(
    connection: &Connection,
    conversation_id: ConversationId,
    message_id: MessageId,
) -> Result<Vec<GroupCompanionCheckpoint>, Error> {
    let rows = {
        let mut statement = connection
            .prepare(
                "SELECT message_id, character_id, source_revision_id,
                        source_candidate_id, emotional_state_json,
                        member_relationships_json, created_at
                 FROM group_companion_checkpoints
                 WHERE conversation_id = ?1 AND message_id = ?2
                 ORDER BY character_id",
            )
            .map_err(failure)?;
        statement
            .query_map(
                params![conversation_id.to_string(), message_id.to_string()],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .map_err(failure)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(failure)?
    };
    load_checkpoints(connection, conversation_id, rows)
}

pub(crate) fn load_persona_applications_for_message_in(
    connection: &Connection,
    conversation_id: ConversationId,
    message_id: MessageId,
) -> Result<Vec<(CharacterId, String, TimestampMillis)>, Error> {
    let mut statement = connection
        .prepare(
            "SELECT character_id, persona_key, applied_at
             FROM group_companion_persona_applications
             WHERE conversation_id = ?1 AND message_id = ?2
             ORDER BY character_id",
        )
        .map_err(failure)?;
    statement
        .query_map(
            params![conversation_id.to_string(), message_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .map_err(failure)?
        .map(|row| {
            let (character_id, persona_key, applied_at) = row.map_err(failure)?;
            Ok((
                CharacterId::from_str(&character_id).map_err(corrupt)?,
                persona_key,
                TimestampMillis::new(applied_at),
            ))
        })
        .collect()
}

impl GroupCompanionRepository for Database {
    fn latest_checkpoints(
        &self,
        conversation_id: ConversationId,
        path: &[MessageId],
    ) -> Result<Vec<GroupCompanionCheckpoint>, Error> {
        let path = serde_json::to_string(&path.iter().map(ToString::to_string).collect::<Vec<_>>())
            .map_err(corrupt)?;
        let connection = self.connection().map_err(failure)?;
        let rows = {
            let mut statement = connection
                .prepare(
                    "SELECT c.message_id, c.character_id, c.source_revision_id,
                            c.source_candidate_id, c.emotional_state_json,
                            c.member_relationships_json, c.created_at
                     FROM group_companion_checkpoints c
                     JOIN json_each(?2) p ON p.value = c.message_id
                     WHERE c.conversation_id = ?1
                       AND p.key = (
                           SELECT max(p2.key)
                           FROM group_companion_checkpoints c2
                           JOIN json_each(?2) p2 ON p2.value = c2.message_id
                           WHERE c2.conversation_id = c.conversation_id
                             AND c2.character_id = c.character_id
                       )
                     ORDER BY c.character_id",
                )
                .map_err(failure)?;
            statement
                .query_map(params![conversation_id.to_string(), path], |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                })
                .map_err(failure)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(failure)?
        };
        load_checkpoints(&connection, conversation_id, rows)
    }

    fn checkpoints_for_message(
        &self,
        conversation_id: ConversationId,
        message_id: MessageId,
    ) -> Result<Vec<GroupCompanionCheckpoint>, Error> {
        let connection = self.connection().map_err(failure)?;
        load_checkpoints_for_message_in(&connection, conversation_id, message_id)
    }

    fn path_checkpoint_sources(
        &self,
        conversation_id: ConversationId,
        path: &[MessageId],
    ) -> Result<Vec<(MessageId, GroupCompanionMessageSource)>, Error> {
        let path = serde_json::to_string(&path.iter().map(ToString::to_string).collect::<Vec<_>>())
            .map_err(corrupt)?;
        let connection = self.connection().map_err(failure)?;
        let mut statement = connection
            .prepare(
                "SELECT DISTINCT c.message_id, c.source_revision_id, c.source_candidate_id
                 FROM group_companion_checkpoints c
                 JOIN json_each(?2) p ON p.value = c.message_id
                 WHERE c.conversation_id = ?1
                 ORDER BY p.key",
            )
            .map_err(failure)?;
        statement
            .query_map(params![conversation_id.to_string(), path], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(failure)?
            .map(|row| {
                let (message_id, revision_id, candidate_id) = row.map_err(failure)?;
                Ok((
                    MessageId::from_str(&message_id).map_err(corrupt)?,
                    GroupCompanionMessageSource {
                        revision_id: revision_id
                            .map(|value| MessageRevisionId::from_str(&value))
                            .transpose()
                            .map_err(corrupt)?,
                        candidate_id: candidate_id
                            .map(|value| MessageCandidateId::from_str(&value))
                            .transpose()
                            .map_err(corrupt)?,
                    },
                ))
            })
            .collect()
    }

    fn persona_relationship(
        &self,
        character_id: CharacterId,
        persona_id: Option<PersonaId>,
    ) -> Result<Option<(Revision, RelationshipState)>, Error> {
        let connection = self.connection().map_err(failure)?;
        connection
            .query_row(
                "SELECT closeness, trust, affection, tension, stability, interaction_count,
                        last_interaction_at, revision
                 FROM companion_relationship_states
                 WHERE character_id = ?1 AND persona_key = ?2",
                params![character_id.to_string(), persona_key(persona_id)],
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
            .map(|(state, revision)| Ok((parse_revision(revision)?, state)))
            .transpose()
    }

    fn recorded_persona_applications(
        &self,
        conversation_id: ConversationId,
        path: &[MessageId],
        persona_id: Option<PersonaId>,
    ) -> Result<Vec<(MessageId, CharacterId)>, Error> {
        let path = serde_json::to_string(&path.iter().map(ToString::to_string).collect::<Vec<_>>())
            .map_err(corrupt)?;
        let connection = self.connection().map_err(failure)?;
        let mut statement = connection
            .prepare(
                "SELECT a.message_id, a.character_id
                 FROM group_companion_persona_applications a
                 JOIN json_each(?2) p ON p.value = a.message_id
                 WHERE a.conversation_id = ?1 AND a.persona_key = ?3
                 ORDER BY p.key, a.character_id",
            )
            .map_err(failure)?;
        statement
            .query_map(
                params![conversation_id.to_string(), path, persona_key(persona_id)],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .map_err(failure)?
            .map(|row| {
                let (message_id, character_id) = row.map_err(failure)?;
                Ok((
                    MessageId::from_str(&message_id).map_err(corrupt)?,
                    CharacterId::from_str(&character_id).map_err(corrupt)?,
                ))
            })
            .collect()
    }
}

#[allow(dead_code)]
pub(crate) fn replace_message_checkpoints_in(
    tx: &Transaction<'_>,
    conversation_id: ConversationId,
    message_id: MessageId,
    checkpoints: &[GroupCompanionCheckpoint],
) -> Result<(), Error> {
    let mut characters = HashSet::new();
    for checkpoint in checkpoints {
        if checkpoint.message_id != message_id || !characters.insert(checkpoint.character_id) {
            return Err(invalid(
                "a checkpoint belongs to another message or repeats a character",
            ));
        }
        checkpoint.validate()?;
    }
    tx.execute(
        "DELETE FROM group_companion_checkpoints
         WHERE conversation_id = ?1 AND message_id = ?2",
        params![conversation_id.to_string(), message_id.to_string()],
    )
    .map_err(failure)?;
    for checkpoint in checkpoints {
        let emotional_state =
            serde_json::to_string(&checkpoint.emotional_state).map_err(corrupt)?;
        let member_relationships =
            serde_json::to_string(&checkpoint.member_relationships).map_err(corrupt)?;
        tx.execute(
            "INSERT INTO group_companion_checkpoints (
               conversation_id, message_id, character_id, source_revision_id,
               source_candidate_id, emotional_state_json, member_relationships_json,
               created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                conversation_id.to_string(),
                message_id.to_string(),
                checkpoint.character_id.to_string(),
                checkpoint.source.revision_id.map(|value| value.to_string()),
                checkpoint
                    .source
                    .candidate_id
                    .map(|value| value.to_string()),
                emotional_state,
                member_relationships,
                checkpoint.created_at.get()
            ],
        )
        .map_err(failure)?;
        for (ordinal, effect) in checkpoint.effects.iter().enumerate() {
            let effect_json = serde_json::to_string(effect).map_err(corrupt)?;
            tx.execute(
                "INSERT INTO group_companion_effects (
                   conversation_id, message_id, character_id, ordinal, effect_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    conversation_id.to_string(),
                    message_id.to_string(),
                    checkpoint.character_id.to_string(),
                    i64::try_from(ordinal).map_err(corrupt)?,
                    effect_json
                ],
            )
            .map_err(failure)?;
        }
    }
    Ok(())
}

/// Returns true when the application row was inserted, false when it already existed.
#[allow(dead_code)]
pub(crate) fn record_persona_application_in(
    tx: &Transaction<'_>,
    conversation_id: ConversationId,
    message_id: MessageId,
    character_id: CharacterId,
    persona_key: &str,
    applied_at: TimestampMillis,
) -> Result<bool, Error> {
    let inserted = tx
        .execute(
            "INSERT OR IGNORE INTO group_companion_persona_applications (
               conversation_id, message_id, character_id, persona_key, applied_at
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                conversation_id.to_string(),
                message_id.to_string(),
                character_id.to_string(),
                persona_key,
                applied_at.get()
            ],
        )
        .map_err(failure)?;
    Ok(inserted > 0)
}

pub(crate) fn write_persona_relationship_in(
    tx: &Transaction<'_>,
    character_id: CharacterId,
    persona_id: Option<PersonaId>,
    expected: Option<Revision>,
    state: &RelationshipState,
    now: TimestampMillis,
) -> Result<(), Error> {
    let key = persona_key(persona_id);
    match expected {
        None => {
            let inserted = tx
                .execute(
                    "INSERT OR IGNORE INTO companion_relationship_states (
                       character_id, persona_key, persona_id, closeness, trust, affection,
                       tension, stability, interaction_count, last_interaction_at, revision,
                       created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11, ?11)",
                    params![
                        character_id.to_string(),
                        key,
                        persona_id.map(|value| value.to_string()),
                        state.closeness,
                        state.trust,
                        state.affection,
                        state.tension,
                        state.stability,
                        i64::from(state.interaction_count),
                        state.last_interaction_at.get(),
                        now.get()
                    ],
                )
                .map_err(failure)?;
            if inserted == 0 {
                return Err(Error::Conflict);
            }
        }
        Some(revision) => {
            let updated = tx
                .execute(
                    "UPDATE companion_relationship_states
                     SET closeness = ?3, trust = ?4, affection = ?5, tension = ?6,
                         stability = ?7, interaction_count = ?8, last_interaction_at = ?9,
                         revision = ?10, updated_at = ?11
                     WHERE character_id = ?1 AND persona_key = ?2 AND revision = ?12",
                    params![
                        character_id.to_string(),
                        key,
                        state.closeness,
                        state.trust,
                        state.affection,
                        state.tension,
                        state.stability,
                        i64::from(state.interaction_count),
                        state.last_interaction_at.get(),
                        i64::try_from(
                            revision
                                .next()
                                .map_err(|_| Error::Corrupt {
                                    reason: "a companion revision overflowed".to_owned(),
                                })?
                                .get(),
                        )
                        .map_err(corrupt)?,
                        now.get(),
                        i64::try_from(revision.get()).map_err(corrupt)?
                    ],
                )
                .map_err(failure)?;
            if updated == 0 {
                return Err(Error::Conflict);
            }
        }
    }
    Ok(())
}

fn conversation_error(error: Error) -> ConversationRepositoryError {
    match error {
        Error::Conflict => ConversationRepositoryError::Conflict,
        Error::Invalid { reason } | Error::Corrupt { reason } | Error::Failure { reason } => {
            ConversationRepositoryError::Storage { reason }
        }
    }
}

pub(crate) fn load_message_source(
    tx: &Transaction<'_>,
    conversation_id: ConversationId,
    message_id: MessageId,
) -> Result<GroupCompanionMessageSource, Error> {
    let (revision_id, candidate_id) = tx
        .query_row(
            "SELECT active_revision_id, active_candidate_id
             FROM conversation_messages
             WHERE conversation_id = ?1 AND id = ?2",
            params![conversation_id.to_string(), message_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                ))
            },
        )
        .map_err(failure)?;
    Ok(GroupCompanionMessageSource {
        revision_id: revision_id
            .map(|value| MessageRevisionId::from_str(&value))
            .transpose()
            .map_err(corrupt)?,
        candidate_id: candidate_id
            .map(|value| MessageCandidateId::from_str(&value))
            .transpose()
            .map_err(corrupt)?,
    })
}

fn apply_prepared_group_companion_turn(
    tx: &Transaction<'_>,
    prepared: &PreparedGroupCompanionTurn,
    user_message_id: Option<MessageId>,
    now: TimestampMillis,
) -> Result<(), Error> {
    let conversation_id = prepared.conversation_id;
    let persona_key = persona_key(prepared.persona_id);
    let members = prepared
        .members
        .iter()
        .map(|(character_id, config)| GroupCompanionMember {
            character_id: *character_id,
            config,
        })
        .collect::<Vec<_>>();
    let mut newly_recorded = HashSet::new();
    for (message_id, step) in &prepared.applied {
        replace_message_checkpoints_in(tx, conversation_id, *message_id, &step.checkpoints)?;
        for (character_id, _) in &step.persona_relationships {
            if record_persona_application_in(
                tx,
                conversation_id,
                *message_id,
                *character_id,
                &persona_key,
                now,
            )? {
                newly_recorded.insert(*character_id);
            }
        }
    }
    let persona_relationships = match &prepared.user_event {
        Some(event) => {
            let Some(message_id) = user_message_id else {
                return Err(invalid("a group companion send has no user message"));
            };
            let source = load_message_source(tx, conversation_id, message_id)?;
            let group_event = GroupCompanionEvent {
                message_id,
                source,
                author: GroupCompanionAuthor::Persona {
                    persona_id: prepared.persona_id,
                },
                present: event.present.clone(),
                addressed: event.addressed.clone(),
                signals: event.signals.clone(),
                at: event.at,
            };
            let mut states = prepared.states.clone();
            let mut persona_relationships = prepared.persona_relationships.clone();
            let step = reduce_group_event(
                &members,
                &mut states,
                &mut persona_relationships,
                &group_event,
            );
            replace_message_checkpoints_in(tx, conversation_id, message_id, &step.checkpoints)?;
            for (character_id, _) in &step.persona_relationships {
                if record_persona_application_in(
                    tx,
                    conversation_id,
                    message_id,
                    *character_id,
                    &persona_key,
                    now,
                )? {
                    newly_recorded.insert(*character_id);
                }
            }
            persona_relationships
        }
        None => prepared.persona_relationships.clone(),
    };
    for character_id in newly_recorded {
        let Some(state) = persona_relationships.get(&character_id) else {
            continue;
        };
        write_persona_relationship_in(
            tx,
            character_id,
            prepared.persona_id,
            prepared
                .persona_revisions
                .get(&character_id)
                .copied()
                .flatten(),
            state,
            now,
        )?;
    }
    Ok(())
}

impl GroupCompanionTurnWriter for Database {
    fn begin_group_companion_send(
        &self,
        command: &SendConversation,
        prepared: PreparedGroupCompanionTurn,
        now: TimestampMillis,
    ) -> Result<SendConversationResult, ConversationRepositoryError> {
        crate::conversation::conversation_mutations::begin_send_with_hook(
            self,
            command,
            now,
            |tx, _turn_id, user_message_id| {
                apply_prepared_group_companion_turn(tx, &prepared, Some(user_message_id), now)
                    .map_err(conversation_error)
            },
        )
    }

    fn begin_group_companion_continue(
        &self,
        command: &ContinueConversation,
        prepared: PreparedGroupCompanionTurn,
        now: TimestampMillis,
    ) -> Result<ContinueConversationResult, ConversationRepositoryError> {
        crate::conversation::conversation_mutations::begin_continue_with_hook(
            self,
            command,
            now,
            |tx, _turn_id| {
                apply_prepared_group_companion_turn(tx, &prepared, None, now)
                    .map_err(conversation_error)
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use lettuce_companions::{
        CompanionEmotionDelta, CompanionSignalChanges, CompanionSoulConfig, EmotionVector,
        GroupCompanionEffectReason, GroupCompanionRelationshipChange, GroupCompanionStep,
        GroupCompanionTarget, RelationshipDelta, RelationshipState,
    };
    use lettuce_types::{ConversationBranchId, ConversationParticipantId};
    use rusqlite::TransactionBehavior;

    struct Fixture {
        database: Database,
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
        user_participant: ConversationParticipantId,
        character_participants: [ConversationParticipantId; 2],
        characters: [CharacterId; 2],
    }

    impl Fixture {
        fn new() -> Self {
            let database = Database::open_in_memory().expect("database");
            let conversation_id = ConversationId::new();
            let branch_id = ConversationBranchId::new();
            let user_participant = ConversationParticipantId::new();
            let first_character = CharacterId::new();
            let second_character = CharacterId::new();
            let first_participant = ConversationParticipantId::new();
            let second_participant = ConversationParticipantId::new();
            {
                let mut connection = database.connection().expect("connection");
                let tx = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .expect("transaction");
                for (character_id, name) in
                    [(first_character, "First"), (second_character, "Second")]
                {
                    tx.execute(
                        "INSERT INTO characters (
                           id, status, name, normalized_name, profile_json,
                           provenance_json, defaults_json, interaction_mode,
                           memory_policy, voice_autoplay, presentation_json, revision,
                           created_at, updated_at
                         ) VALUES (?1, 'active', ?2, ?3, '{}', '{}', '{}',
                                   'companion', 'manual', 0, '{}', 1, 1, 1)",
                        params![character_id.to_string(), name, name.to_ascii_lowercase()],
                    )
                    .expect("character");
                }
                tx.execute(
                    "INSERT INTO conversations (
                       id, kind, lifecycle, title, active_branch_id, kind_json,
                       revision, created_at, updated_at
                     ) VALUES (?1, 'group', 'active', 'Group', ?2,
                               '{\"format_version\":1}', 1, 1, 1)",
                    params![conversation_id.to_string(), branch_id.to_string()],
                )
                .expect("conversation");
                tx.execute(
                    "INSERT INTO conversation_branches (
                       conversation_id, id, status, revision, created_at, updated_at
                     ) VALUES (?1, ?2, 'active', 1, 1, 1)",
                    params![conversation_id.to_string(), branch_id.to_string()],
                )
                .expect("branch");
                tx.execute(
                    "INSERT INTO conversation_participants (
                       conversation_id, id, role, ordinal, source_kind, source_id,
                       enabled, muted, display_name, model_selection_json, revision,
                       created_at, updated_at
                     ) VALUES (?1, ?2, 'user', 0, 'user', NULL, 1, 0, 'User',
                               '{\"format_version\":1}', 1, 1, 1)",
                    params![conversation_id.to_string(), user_participant.to_string()],
                )
                .expect("user participant");
                for (ordinal, (participant, character, name)) in [
                    (first_participant, first_character, "First"),
                    (second_participant, second_character, "Second"),
                ]
                .into_iter()
                .enumerate()
                {
                    tx.execute(
                        "INSERT INTO conversation_participants (
                           conversation_id, id, role, ordinal, source_kind, source_id,
                           enabled, muted, display_name, model_selection_json, revision,
                           created_at, updated_at
                         ) VALUES (?1, ?2, 'character', ?3, 'character', ?4, 1, 0, ?5,
                                   '{\"format_version\":1}', 1, 1, 1)",
                        params![
                            conversation_id.to_string(),
                            participant.to_string(),
                            i64::try_from(ordinal + 1).expect("ordinal"),
                            character.to_string(),
                            name
                        ],
                    )
                    .expect("character participant");
                }
                tx.commit().expect("commit");
            }
            Self {
                database,
                conversation_id,
                branch_id,
                user_participant,
                character_participants: [first_participant, second_participant],
                characters: [first_character, second_character],
            }
        }

        fn append_message(&self, assistant: bool, ordinal: i64, now: i64) -> MessageId {
            let message_id = MessageId::new();
            let revision_id = MessageRevisionId::new();
            let (role, author) = if assistant {
                ("assistant", self.character_participants[0])
            } else {
                ("user", self.user_participant)
            };
            let mut connection = self.database.connection().expect("connection");
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .expect("transaction");
            tx.execute(
                "INSERT INTO conversation_messages (
                   conversation_id, id, branch_id, author_participant_id, role,
                   timeline_ordinal, logical_time, effective_time, visibility, pinned,
                   scene_edited, active_revision_id, revision, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, 'visible', 0, 0, ?8, 1, ?7, ?7)",
                params![
                    self.conversation_id.to_string(),
                    message_id.to_string(),
                    self.branch_id.to_string(),
                    author.to_string(),
                    role,
                    ordinal,
                    now,
                    revision_id.to_string()
                ],
            )
            .expect("message");
            tx.execute(
                "INSERT INTO conversation_message_revisions (
                   conversation_id, id, message_id, branch_id, sequence, parts_json,
                   authored_at
                 ) VALUES (?1, ?2, ?3, ?4, 1, '{\"format_version\":1}', ?5)",
                params![
                    self.conversation_id.to_string(),
                    revision_id.to_string(),
                    message_id.to_string(),
                    self.branch_id.to_string(),
                    now
                ],
            )
            .expect("revision");
            tx.commit().expect("commit");
            message_id
        }

        fn write(&self, message_id: MessageId, checkpoints: &[GroupCompanionCheckpoint]) {
            let mut connection = self.database.connection().expect("connection");
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .expect("transaction");
            replace_message_checkpoints_in(&tx, self.conversation_id, message_id, checkpoints)
                .expect("replace checkpoints");
            tx.commit().expect("commit");
        }
    }

    fn state(signal: &str) -> EmotionalState {
        EmotionalState {
            felt: EmotionVector {
                warmth: 0.4,
                ..EmotionVector::default()
            },
            expressed: EmotionVector {
                warmth: 0.3,
                ..EmotionVector::default()
            },
            blocked: EmotionVector::default(),
            momentum: EmotionVector {
                warmth: 0.1,
                ..EmotionVector::default()
            },
            active_drivers: vec![signal.into()],
            confidence: 0.6,
            updated_at: TimestampMillis::new(10),
        }
    }

    fn relationship(target_character_id: CharacterId, trust: f64) -> GroupMemberRelationship {
        GroupMemberRelationship {
            target_character_id,
            state: RelationshipState {
                closeness: 0.2,
                trust,
                affection: 0.1,
                tension: 0.05,
                stability: 0.7,
                interaction_count: 2,
                last_interaction_at: TimestampMillis::new(10),
            },
        }
    }

    fn effect(target: CharacterId, signal: &str) -> GroupCompanionEffect {
        GroupCompanionEffect {
            reason: GroupCompanionEffectReason::Witnessed,
            social_event: None,
            emotion_delta: CompanionEmotionDelta::default(),
            relationship_change: Some(GroupCompanionRelationshipChange {
                target: GroupCompanionTarget::Character {
                    character_id: target,
                },
                delta: RelationshipDelta::default(),
            }),
            signal_changes: CompanionSignalChanges {
                added: vec![signal.into()],
                removed: Vec::new(),
            },
        }
    }

    fn checkpoint(
        message_id: MessageId,
        character_id: CharacterId,
        target: CharacterId,
        signal: &str,
        effects: Vec<GroupCompanionEffect>,
    ) -> GroupCompanionCheckpoint {
        GroupCompanionCheckpoint {
            message_id,
            character_id,
            source: GroupCompanionMessageSource {
                revision_id: Some(MessageRevisionId::new()),
                candidate_id: None,
            },
            emotional_state: state(signal),
            member_relationships: vec![relationship(target, 0.3)],
            effects,
            created_at: TimestampMillis::new(10),
        }
    }

    fn sorted(mut values: Vec<GroupCompanionCheckpoint>) -> Vec<GroupCompanionCheckpoint> {
        values.sort_by_key(|value| value.character_id.to_string());
        values
    }

    #[test]
    fn replace_round_trips_state_relationships_and_effects_in_order() {
        let fixture = Fixture::new();
        let message_id = fixture.append_message(false, 1, 10);
        let [first, second] = fixture.characters;
        let first_checkpoint = checkpoint(
            message_id,
            first,
            second,
            "first",
            vec![effect(second, "one"), effect(second, "two")],
        );
        let second_checkpoint = checkpoint(
            message_id,
            second,
            first,
            "second",
            vec![effect(first, "three")],
        );
        fixture.write(
            message_id,
            &[first_checkpoint.clone(), second_checkpoint.clone()],
        );
        assert_eq!(
            fixture
                .database
                .checkpoints_for_message(fixture.conversation_id, message_id)
                .expect("checkpoints"),
            sorted(vec![first_checkpoint, second_checkpoint])
        );
    }

    #[test]
    fn latest_checkpoints_take_the_last_path_message_per_character() {
        let fixture = Fixture::new();
        let [first, second] = fixture.characters;
        let first_message = fixture.append_message(false, 1, 10);
        let second_message = fixture.append_message(false, 2, 20);
        let third_message = fixture.append_message(false, 3, 30);
        let first_zero = checkpoint(first_message, first, second, "first-zero", Vec::new());
        let first_one = checkpoint(second_message, first, second, "first-one", Vec::new());
        let first_two = checkpoint(third_message, first, second, "first-two", Vec::new());
        let second_zero = checkpoint(first_message, second, first, "second-zero", Vec::new());
        let second_two = checkpoint(third_message, second, first, "second-two", Vec::new());
        fixture.write(first_message, &[first_zero.clone(), second_zero.clone()]);
        fixture.write(second_message, &[first_one.clone()]);
        fixture.write(third_message, &[first_two, second_two]);

        assert_eq!(
            fixture
                .database
                .latest_checkpoints(fixture.conversation_id, &[first_message, second_message])
                .expect("latest"),
            sorted(vec![first_one.clone(), second_zero.clone()])
        );
        assert_eq!(
            fixture
                .database
                .latest_checkpoints(fixture.conversation_id, &[second_message])
                .expect("latest"),
            vec![first_one]
        );
    }

    #[test]
    fn replacing_removes_the_previous_rows_and_effects() {
        let fixture = Fixture::new();
        let message_id = fixture.append_message(false, 1, 10);
        let [first, second] = fixture.characters;
        fixture.write(
            message_id,
            &[
                checkpoint(
                    message_id,
                    first,
                    second,
                    "first",
                    vec![effect(second, "one"), effect(second, "two")],
                ),
                checkpoint(
                    message_id,
                    second,
                    first,
                    "second",
                    vec![effect(first, "three")],
                ),
            ],
        );
        let replacement = checkpoint(message_id, first, second, "replacement", Vec::new());
        fixture.write(message_id, &[replacement.clone()]);
        assert_eq!(
            fixture
                .database
                .checkpoints_for_message(fixture.conversation_id, message_id)
                .expect("checkpoints"),
            vec![replacement]
        );
        let effects: i64 = fixture
            .database
            .connection()
            .expect("connection")
            .query_row("SELECT count(*) FROM group_companion_effects", [], |row| {
                row.get(0)
            })
            .expect("effect count");
        assert_eq!(effects, 0);
    }

    #[test]
    fn persona_application_is_recorded_once_per_key() {
        let fixture = Fixture::new();
        let message_id = fixture.append_message(false, 1, 10);
        let character_id = fixture.characters[0];
        let apply = || {
            let mut connection = fixture.database.connection().expect("connection");
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .expect("transaction");
            let inserted = record_persona_application_in(
                &tx,
                fixture.conversation_id,
                message_id,
                character_id,
                "persona",
                TimestampMillis::new(20),
            )
            .expect("apply");
            tx.commit().expect("commit");
            inserted
        };
        assert!(apply());
        assert!(!apply());
    }

    #[test]
    fn invalid_checkpoint_writes_nothing() {
        let fixture = Fixture::new();
        let message_id = fixture.append_message(false, 1, 10);
        let [first, second] = fixture.characters;
        let existing = checkpoint(
            message_id,
            first,
            second,
            "existing",
            vec![effect(second, "one")],
        );
        fixture.write(message_id, &[existing.clone()]);

        let mut invalid = checkpoint(
            message_id,
            first,
            second,
            "invalid",
            vec![effect(second, "two")],
        );
        invalid.member_relationships[0].target_character_id = first;
        let mut connection = fixture.database.connection().expect("connection");
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("transaction");
        assert_eq!(
            replace_message_checkpoints_in(
                &tx,
                fixture.conversation_id,
                message_id,
                &[invalid]
            ),
            Err(GroupCompanionRepositoryError::Invalid {
                reason: "a member relationship repeats a target, targets the owner or has an out-of-range value".to_owned(),
            })
        );
        tx.commit().expect("commit");
        drop(connection);
        assert_eq!(
            fixture
                .database
                .checkpoints_for_message(fixture.conversation_id, message_id)
                .expect("checkpoints"),
            vec![existing]
        );
    }

    #[test]
    fn deleting_a_character_removes_its_checkpoints() {
        let fixture = Fixture::new();
        let message_id = fixture.append_message(false, 1, 10);
        let [first, second] = fixture.characters;
        let first_checkpoint = checkpoint(
            message_id,
            first,
            second,
            "first",
            vec![effect(second, "one")],
        );
        let second_checkpoint = checkpoint(
            message_id,
            second,
            first,
            "second",
            vec![effect(first, "two")],
        );
        fixture.write(message_id, &[first_checkpoint, second_checkpoint.clone()]);
        fixture
            .database
            .connection()
            .expect("connection")
            .execute("DELETE FROM characters WHERE id = ?1", [first.to_string()])
            .expect("delete character");
        assert_eq!(
            fixture
                .database
                .checkpoints_for_message(fixture.conversation_id, message_id)
                .expect("checkpoints"),
            vec![second_checkpoint]
        );
    }

    #[test]
    fn group_companion_path_checkpoint_sources_lists_message_sources() {
        let fixture = Fixture::new();
        let [first, second] = fixture.characters;
        let first_message = fixture.append_message(false, 1, 10);
        let second_message = fixture.append_message(false, 2, 20);
        let source = GroupCompanionMessageSource {
            revision_id: Some(MessageRevisionId::new()),
            candidate_id: None,
        };
        let mut first_checkpoint = checkpoint(first_message, first, second, "first", Vec::new());
        first_checkpoint.source = source.clone();
        let mut second_checkpoint = checkpoint(first_message, second, first, "second", Vec::new());
        second_checkpoint.source = source.clone();
        fixture.write(first_message, &[first_checkpoint, second_checkpoint]);
        let other = checkpoint(second_message, first, second, "other", Vec::new());
        fixture.write(second_message, &[other.clone()]);

        assert_eq!(
            fixture
                .database
                .path_checkpoint_sources(fixture.conversation_id, &[first_message, second_message])
                .expect("sources"),
            vec![(first_message, source), (second_message, other.source)]
        );
    }

    #[test]
    fn group_companion_persona_relationship_writes_and_conflicts() {
        let fixture = Fixture::new();
        let character_id = fixture.characters[0];
        let mut state = relationship(character_id, 0.4).state;
        state.last_interaction_at = TimestampMillis::new(20);
        let write = |expected, expected_state: &RelationshipState| {
            let mut connection = fixture.database.connection().expect("connection");
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .expect("transaction");
            let result = write_persona_relationship_in(
                &tx,
                character_id,
                None,
                expected,
                expected_state,
                TimestampMillis::new(30),
            );
            tx.commit().expect("commit");
            result
        };
        assert_eq!(write(None, &state), Ok(()));
        assert_eq!(
            fixture
                .database
                .persona_relationship(character_id, None)
                .expect("relationship"),
            Some((Revision::INITIAL, state.clone()))
        );
        assert_eq!(write(None, &state), Err(Error::Conflict));
        state.trust = 0.8;
        assert_eq!(write(Some(Revision::INITIAL), &state), Ok(()));
        assert_eq!(
            fixture
                .database
                .persona_relationship(character_id, None)
                .expect("relationship"),
            Some((Revision::new(2), state.clone()))
        );
        assert_eq!(write(Some(Revision::INITIAL), &state), Err(Error::Conflict));
    }

    #[test]
    fn group_companion_turn_fails_on_a_stale_relationship_revision() {
        let fixture = Fixture::new();
        let [first, second] = fixture.characters;
        let message_id = fixture.append_message(false, 1, 10);
        let mut state = relationship(second, 0.6).state;
        state.last_interaction_at = TimestampMillis::new(20);
        let prepared = PreparedGroupCompanionTurn {
            conversation_id: fixture.conversation_id,
            persona_id: None,
            members: vec![
                (first, CompanionSoulConfig::default()),
                (second, CompanionSoulConfig::default()),
            ],
            applied: vec![(
                message_id,
                GroupCompanionStep {
                    checkpoints: Vec::new(),
                    persona_relationships: vec![(first, state.clone())],
                },
            )],
            states: BTreeMap::new(),
            persona_relationships: BTreeMap::from([(first, state.clone())]),
            persona_revisions: BTreeMap::from([(first, Some(Revision::INITIAL))]),
            user_event: None,
        };
        {
            let mut connection = fixture.database.connection().expect("connection");
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .expect("transaction");
            write_persona_relationship_in(&tx, first, None, None, &state, TimestampMillis::new(30))
                .expect("initial relationship");
            write_persona_relationship_in(
                &tx,
                first,
                None,
                Some(Revision::INITIAL),
                &state,
                TimestampMillis::new(31),
            )
            .expect("second relationship");
            tx.commit().expect("commit");
        }
        let mut connection = fixture.database.connection().expect("connection");
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("transaction");
        assert_eq!(
            apply_prepared_group_companion_turn(&tx, &prepared, None, TimestampMillis::new(40)),
            Err(Error::Conflict)
        );
    }
}
