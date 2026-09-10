use lettuce_transfer::{
    ASR_LEARNING_DOCUMENT_VERSION, AsrLearningAudioAsset, AsrLearningDocument,
    AuthoredProfileBackup, BackupConversation, BackupConversationOutbox, BackupConversationRuntime,
    BackupConversationUsage, BackupGenerationAttemptRuntime, BackupGenerationCheckpoint,
    BackupGenerationTurn, BackupGlobalSettings, BackupJobInference, BackupLorebookBindings,
    BackupMemoryProjection, BackupMemoryProjectionState, BackupMemorySpace, BackupMessage,
    COMPANION_EFFECT_BACKUP_VERSION, COMPANION_STATE_BACKUP_VERSION,
    CONVERSATION_HISTORY_BACKUP_VERSION, CONVERSATION_OUTBOX_BACKUP_VERSION,
    CONVERSATION_RUNTIME_BACKUP_VERSION, CONVERSATION_USAGE_BACKUP_VERSION, CompanionEffectBackup,
    CompanionStateBackup, ConversationHistoryBackup, ConversationOutboxBackup,
    ConversationRuntimeBackup, ConversationUsageBackup, JOB_BACKUP_VERSION, JobBackup,
    MAX_BACKUP_AUTHORED_ROOTS, MAX_BACKUP_COMPANION_EFFECTS, MAX_BACKUP_COMPANION_RECEIPTS,
    MAX_BACKUP_COMPANION_RELATIONSHIPS, MAX_BACKUP_COMPANION_SESSIONS,
    MAX_BACKUP_CONVERSATION_OPERATIONS, MAX_BACKUP_CONVERSATION_OUTBOX_EVENTS,
    MAX_BACKUP_CONVERSATION_USAGE_EVENTS, MAX_BACKUP_CONVERSATIONS,
    MAX_BACKUP_GENERATION_CHECKPOINTS, MAX_BACKUP_GENERATION_TURNS, MAX_BACKUP_JOB_EVENTS,
    MAX_BACKUP_JOB_INFERENCE_EVENTS, MAX_BACKUP_JOBS, MAX_BACKUP_MEDIA_RECORDS,
    MAX_BACKUP_MEMORY_ACCESSES, MAX_BACKUP_MEMORY_PROJECTIONS, MAX_BACKUP_MEMORY_REWINDS,
    MAX_BACKUP_MEMORY_SPACES, MAX_BACKUP_MESSAGE_CANDIDATES, MAX_BACKUP_MESSAGE_REVISIONS,
    MAX_BACKUP_MESSAGES, MAX_BACKUP_TOOL_EXECUTIONS, MEMORY_BACKUP_VERSION,
    MEMORY_PROJECTION_BACKUP_VERSION, MemoryBackup, MemoryProjectionBackup,
    PROVIDER_BACKUP_GRAPH_VERSION, ProviderBackupGraph, ProviderBackupSelections,
    ProviderBackupSource, ProviderBackupSourceError,
};
use lettuce_types::{
    AssetId, CharacterId, ContentHash, ConversationId, GenerationAttemptId, GroupId, LorebookId,
    ModelProfileId, OperationRecordId, PersonaId, PromptDocumentId, Revision, TimestampMillis,
};
use rusqlite::{OptionalExtension, TransactionBehavior, params};

use crate::{Database, model_from_row, parse_id, provider_from_row};

fn backup_error(error: rusqlite::Error) -> ProviderBackupSourceError {
    if error == rusqlite::Error::InvalidQuery {
        ProviderBackupSourceError::InvalidData
    } else {
        ProviderBackupSourceError::Storage
    }
}

fn read_companion_effects(
    transaction: &rusqlite::Transaction<'_>,
) -> Result<CompanionEffectBackup, ProviderBackupSourceError> {
    let identities = transaction
        .prepare(&format!(
            "SELECT conversation_id,assistant_message_id FROM companion_turn_effects ORDER BY conversation_id,created_at,id LIMIT {}",
            MAX_BACKUP_COMPANION_EFFECTS + 1
        ))
        .and_then(|mut statement| {
            statement
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(backup_error)?;
    if identities.len() > MAX_BACKUP_COMPANION_EFFECTS {
        return Err(ProviderBackupSourceError::InvalidData);
    }
    let effects = identities
        .into_iter()
        .map(|(conversation_id, message_id)| {
            let conversation_id = conversation_id
                .parse()
                .map_err(|_| ProviderBackupSourceError::InvalidData)?;
            let message_id = message_id
                .parse()
                .map_err(|_| ProviderBackupSourceError::InvalidData)?;
            crate::state_adapter::load_effect(transaction, conversation_id, message_id)
                .map_err(|_| ProviderBackupSourceError::InvalidData)?
                .ok_or(ProviderBackupSourceError::InvalidData)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let rows = transaction
        .prepare(&format!(
            "SELECT operation_id,request_digest,conversation_id,invalid_run_id,space_id,source_memory_revision,resulting_memory_revision,restored_summary_run_id,resulting_memory_json,resulting_summary_json,applied_at FROM dynamic_memory_suffix_rewinds ORDER BY applied_at,operation_id LIMIT {}",
            MAX_BACKUP_MEMORY_REWINDS + 1
        ))
        .and_then(|mut statement| {
            statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, Option<String>>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, Option<String>>(9)?,
                        row.get::<_, i64>(10)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(backup_error)?;
    if rows.len() > MAX_BACKUP_MEMORY_REWINDS {
        return Err(ProviderBackupSourceError::InvalidData);
    }
    let mut rewinds = Vec::with_capacity(rows.len());
    for row in rows {
        let operation_id: lettuce_types::OperationId = row
            .0
            .parse()
            .map_err(|_| ProviderBackupSourceError::InvalidData)?;
        let resulting_memory: lettuce_memory::MemorySpaceSnapshot =
            crate::decode_versioned(&row.8, 1)
                .map_err(|_| ProviderBackupSourceError::InvalidData)?;
        let resulting_summary = row
            .9
            .map(|value| crate::decode_versioned(&value, 1))
            .transpose()
            .map_err(|_| ProviderBackupSourceError::InvalidData)?;
        let invalidated_effect_ids = transaction
            .prepare(
                "SELECT effect_id FROM companion_turn_effect_invalidations WHERE operation_id=?1 ORDER BY ordinal",
            )
            .and_then(|mut statement| {
                statement
                    .query_map([operation_id.to_string()], |row| row.get::<_, String>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(backup_error)?
            .into_iter()
            .map(|value| {
                value
                    .parse()
                    .map_err(|_| ProviderBackupSourceError::InvalidData)
            })
            .collect::<Result<Vec<_>, _>>()?;
        rewinds.push(lettuce_transfer::BackupDynamicMemoryRewind {
            operation_id,
            request_digest: row
                .1
                .parse()
                .map_err(|_| ProviderBackupSourceError::InvalidData)?,
            conversation_id: row
                .2
                .parse()
                .map_err(|_| ProviderBackupSourceError::InvalidData)?,
            invalid_run_id: row
                .3
                .map(|value| value.parse())
                .transpose()
                .map_err(|_| ProviderBackupSourceError::InvalidData)?,
            space_id: row
                .4
                .parse()
                .map_err(|_| ProviderBackupSourceError::InvalidData)?,
            source_memory_revision: backup_revision(row.5)?,
            resulting_memory_revision: backup_revision(row.6)?,
            restored_summary_run_id: row
                .7
                .map(|value| value.parse())
                .transpose()
                .map_err(|_| ProviderBackupSourceError::InvalidData)?,
            resulting_memory,
            resulting_summary,
            invalidated_effect_ids,
            applied_at: TimestampMillis::new(row.10),
        });
    }
    Ok(CompanionEffectBackup {
        version: COMPANION_EFFECT_BACKUP_VERSION,
        effects,
        rewinds,
    })
}

fn read_memory(
    transaction: &rusqlite::Transaction<'_>,
) -> Result<MemoryBackup, ProviderBackupSourceError> {
    let owners = transaction
        .prepare(&format!(
            "SELECT conversation_id,space_id FROM conversation_memory_spaces ORDER BY conversation_id LIMIT {}",
            MAX_BACKUP_MEMORY_SPACES + 1
        ))
        .and_then(|mut statement| {
            statement
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(backup_error)?;
    if owners.len() > MAX_BACKUP_MEMORY_SPACES {
        return Err(ProviderBackupSourceError::InvalidData);
    }
    let spaces = owners
        .into_iter()
        .map(|(conversation_id, space_id)| {
            let conversation_id = conversation_id
                .parse()
                .map_err(|_| ProviderBackupSourceError::InvalidData)?;
            let space_id = space_id
                .parse()
                .map_err(|_| ProviderBackupSourceError::InvalidData)?;
            let snapshot = crate::memory_adapter::get_in(transaction, space_id)
                .map_err(|_| ProviderBackupSourceError::InvalidData)?
                .ok_or(ProviderBackupSourceError::InvalidData)?;
            let summary = crate::memory_adapter::get_summary_in(transaction, space_id)
                .map_err(|_| ProviderBackupSourceError::InvalidData)?;
            Ok(BackupMemorySpace {
                conversation_id,
                snapshot,
                summary,
            })
        })
        .collect::<Result<Vec<_>, ProviderBackupSourceError>>()?;
    let rows = transaction
        .prepare(&format!(
            "SELECT conversation_id,turn_id,attempt_id,space_id,expected_revision,resulting_revision,selected_memory_ids_json,accessed_at FROM memory_retrieval_accesses ORDER BY conversation_id,turn_id,attempt_id LIMIT {}",
            MAX_BACKUP_MEMORY_ACCESSES + 1
        ))
        .and_then(|mut statement| {
            statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, i64>(7)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(backup_error)?;
    if rows.len() > MAX_BACKUP_MEMORY_ACCESSES {
        return Err(ProviderBackupSourceError::InvalidData);
    }
    let retrieval_accesses = rows
        .into_iter()
        .map(|row| {
            Ok(lettuce_memory::MemoryRetrievalAccessReceipt {
                access: lettuce_memory::MemoryRetrievalAccess {
                    conversation_id: row
                        .0
                        .parse()
                        .map_err(|_| ProviderBackupSourceError::InvalidData)?,
                    turn_id: row
                        .1
                        .parse()
                        .map_err(|_| ProviderBackupSourceError::InvalidData)?,
                    attempt_id: row
                        .2
                        .parse()
                        .map_err(|_| ProviderBackupSourceError::InvalidData)?,
                    space_id: row
                        .3
                        .parse()
                        .map_err(|_| ProviderBackupSourceError::InvalidData)?,
                    expected_revision: backup_revision(row.4)?,
                    selected_memory_ids: serde_json::from_str(&row.6)
                        .map_err(|_| ProviderBackupSourceError::InvalidData)?,
                    accessed_at: TimestampMillis::new(row.7),
                },
                resulting_revision: backup_revision(row.5)?,
            })
        })
        .collect::<Result<Vec<_>, ProviderBackupSourceError>>()?;
    Ok(MemoryBackup {
        version: MEMORY_BACKUP_VERSION,
        spaces,
        retrieval_accesses,
    })
}

fn read_memory_projections(
    transaction: &rusqlite::Transaction<'_>,
) -> Result<MemoryProjectionBackup, ProviderBackupSourceError> {
    let rows = transaction
        .prepare(&format!(
            "SELECT space_id,memory_id,source_revision,dimensions,source_text,status,vector,updated_at FROM memory_embedding_projections ORDER BY space_id,memory_id,source_revision,dimensions LIMIT {}",
            MAX_BACKUP_MEMORY_PROJECTIONS + 1
        ))
        .and_then(|mut statement| {
            statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, Option<Vec<u8>>>(6)?,
                        row.get::<_, i64>(7)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(backup_error)?;
    if rows.len() > MAX_BACKUP_MEMORY_PROJECTIONS {
        return Err(ProviderBackupSourceError::InvalidData);
    }
    let projections = rows
        .into_iter()
        .map(|row| {
            let state = match (row.5.as_str(), row.6) {
                ("ready", Some(vector_le_bytes)) => {
                    let vector_le_hex = vector_le_bytes
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect();
                    BackupMemoryProjectionState::Ready { vector_le_hex }
                }
                ("repair_needed", None) => BackupMemoryProjectionState::RepairNeeded,
                _ => return Err(ProviderBackupSourceError::InvalidData),
            };
            Ok(BackupMemoryProjection {
                space_id: row
                    .0
                    .parse()
                    .map_err(|_| ProviderBackupSourceError::InvalidData)?,
                memory_id: row
                    .1
                    .parse()
                    .map_err(|_| ProviderBackupSourceError::InvalidData)?,
                source_revision: row.2,
                dimensions: u16::try_from(row.3)
                    .map_err(|_| ProviderBackupSourceError::InvalidData)?,
                source_text: row.4,
                state,
                updated_at: TimestampMillis::new(row.7),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(MemoryProjectionBackup {
        version: MEMORY_PROJECTION_BACKUP_VERSION,
        projections,
    })
}

fn read_companion_state(
    transaction: &rusqlite::Transaction<'_>,
) -> Result<CompanionStateBackup, ProviderBackupSourceError> {
    let relationships = transaction
        .prepare(&format!(
            "SELECT character_id,persona_key,persona_id,closeness,trust,affection,tension,stability,interaction_count,last_interaction_at,revision,created_at,updated_at FROM companion_relationship_states ORDER BY character_id,persona_key LIMIT {}",
            MAX_BACKUP_COMPANION_RELATIONSHIPS + 1
        ))
        .and_then(|mut statement| {
            statement
                .query_map([], |row| {
                    let character_id = row.get::<_, String>(0)?;
                    let persona_key = row.get::<_, String>(1)?;
                    let persona_id = row.get::<_, Option<String>>(2)?;
                    Ok((
                        character_id,
                        persona_key,
                        persona_id,
                        row.get::<_, f64>(3)?,
                        row.get::<_, f64>(4)?,
                        row.get::<_, f64>(5)?,
                        row.get::<_, f64>(6)?,
                        row.get::<_, f64>(7)?,
                        row.get::<_, i64>(8)?,
                        row.get::<_, i64>(9)?,
                        row.get::<_, i64>(10)?,
                        row.get::<_, i64>(11)?,
                        row.get::<_, i64>(12)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(backup_error)?;
    if relationships.len() > MAX_BACKUP_COMPANION_RELATIONSHIPS {
        return Err(ProviderBackupSourceError::InvalidData);
    }
    let relationships = relationships
        .into_iter()
        .map(
            |(
                character_id,
                persona_key,
                persona_id,
                closeness,
                trust,
                affection,
                tension,
                stability,
                interaction_count,
                last_interaction_at,
                revision,
                created_at,
                updated_at,
            )| {
                let persona_id = persona_id
                    .map(|value| value.parse::<PersonaId>())
                    .transpose()
                    .map_err(|_| ProviderBackupSourceError::InvalidData)?;
                validate_persona_key(&persona_key, persona_id)?;
                Ok(lettuce_transfer::BackupCompanionRelationship {
                    character_id: character_id
                        .parse()
                        .map_err(|_| ProviderBackupSourceError::InvalidData)?,
                    persona_id,
                    state: lettuce_companions::RelationshipState {
                        closeness,
                        trust,
                        affection,
                        tension,
                        stability,
                        interaction_count: u32::try_from(interaction_count)
                            .map_err(|_| ProviderBackupSourceError::InvalidData)?,
                        last_interaction_at: TimestampMillis::new(last_interaction_at),
                    },
                    revision: backup_revision(revision)?,
                    created_at: TimestampMillis::new(created_at),
                    updated_at: TimestampMillis::new(updated_at),
                })
            },
        )
        .collect::<Result<Vec<_>, _>>()?;
    let session_rows = transaction
        .prepare(&format!(
            "SELECT conversation_id,character_id,persona_key,persona_id,initial_hash,confidence,emotional_updated_at,state_updated_at,revision,created_at,updated_at FROM companion_session_states ORDER BY conversation_id LIMIT {}",
            MAX_BACKUP_COMPANION_SESSIONS + 1
        ))
        .and_then(|mut statement| {
            statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Vec<u8>>(4)?,
                        row.get::<_, f64>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, i64>(7)?,
                        row.get::<_, i64>(8)?,
                        row.get::<_, i64>(9)?,
                        row.get::<_, i64>(10)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(backup_error)?;
    if session_rows.len() > MAX_BACKUP_COMPANION_SESSIONS {
        return Err(ProviderBackupSourceError::InvalidData);
    }
    let mut sessions = Vec::with_capacity(session_rows.len());
    for row in session_rows {
        let conversation_id: ConversationId = row
            .0
            .parse()
            .map_err(|_| ProviderBackupSourceError::InvalidData)?;
        let persona_id = row
            .3
            .map(|value| value.parse::<PersonaId>())
            .transpose()
            .map_err(|_| ProviderBackupSourceError::InvalidData)?;
        validate_persona_key(&row.2, persona_id)?;
        sessions.push(lettuce_transfer::BackupCompanionSession {
            owner: lettuce_companions::CompanionStateOwner {
                conversation_id,
                character_id: row
                    .1
                    .parse()
                    .map_err(|_| ProviderBackupSourceError::InvalidData)?,
                persona_id,
            },
            initial_state_hash: backup_hash(&row.4)?,
            emotional_state: lettuce_companions::EmotionalState {
                felt: crate::state_adapter::read_vector(transaction, conversation_id, "felt")
                    .map_err(|_| ProviderBackupSourceError::InvalidData)?,
                expressed: crate::state_adapter::read_vector(
                    transaction,
                    conversation_id,
                    "expressed",
                )
                .map_err(|_| ProviderBackupSourceError::InvalidData)?,
                blocked: crate::state_adapter::read_vector(transaction, conversation_id, "blocked")
                    .map_err(|_| ProviderBackupSourceError::InvalidData)?,
                momentum: crate::state_adapter::read_vector(
                    transaction,
                    conversation_id,
                    "momentum",
                )
                .map_err(|_| ProviderBackupSourceError::InvalidData)?,
                active_drivers: crate::state_adapter::read_signals(
                    transaction,
                    conversation_id,
                    "driver",
                )
                .map_err(|_| ProviderBackupSourceError::InvalidData)?,
                confidence: row.5,
                updated_at: TimestampMillis::new(row.6),
            },
            active_signals: crate::state_adapter::read_signals(
                transaction,
                conversation_id,
                "active",
            )
            .map_err(|_| ProviderBackupSourceError::InvalidData)?,
            state_updated_at: TimestampMillis::new(row.7),
            revision: backup_revision(row.8)?,
            created_at: TimestampMillis::new(row.9),
            updated_at: TimestampMillis::new(row.10),
        });
    }
    let episodes = transaction
        .prepare(&format!(
            "SELECT conversation_id,character_id,persona_key,persona_id,episode_index,previous_conversation_id,started_at,ended_at,updated_at FROM companion_continuity_episodes ORDER BY conversation_id LIMIT {}",
            MAX_BACKUP_COMPANION_SESSIONS + 1
        ))
        .and_then(|mut statement| {
            statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, Option<i64>>(7)?,
                        row.get::<_, i64>(8)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(backup_error)?;
    if episodes.len() > MAX_BACKUP_COMPANION_SESSIONS {
        return Err(ProviderBackupSourceError::InvalidData);
    }
    let episodes = episodes
        .into_iter()
        .map(|row| {
            let persona_id = row
                .3
                .map(|value| value.parse::<PersonaId>())
                .transpose()
                .map_err(|_| ProviderBackupSourceError::InvalidData)?;
            validate_persona_key(&row.2, persona_id)?;
            Ok(lettuce_companions::CompanionContinuityEpisode {
                conversation_id: row
                    .0
                    .parse()
                    .map_err(|_| ProviderBackupSourceError::InvalidData)?,
                character_id: row
                    .1
                    .parse()
                    .map_err(|_| ProviderBackupSourceError::InvalidData)?,
                persona_id,
                episode_index: u32::try_from(row.4)
                    .map_err(|_| ProviderBackupSourceError::InvalidData)?,
                previous_conversation_id: row
                    .5
                    .map(|value| value.parse())
                    .transpose()
                    .map_err(|_| ProviderBackupSourceError::InvalidData)?,
                started_at: TimestampMillis::new(row.6),
                ended_at: row.7.map(TimestampMillis::new),
                updated_at: TimestampMillis::new(row.8),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let receipt_rows = transaction
        .prepare(&format!(
            "SELECT operation_id,conversation_id,character_id,persona_key,expected_session_revision,resulting_session_revision,expected_relationship_revision,resulting_relationship_revision,applied_at,change_hash FROM companion_state_apply_receipts ORDER BY applied_at,operation_id LIMIT {}",
            MAX_BACKUP_COMPANION_RECEIPTS + 1
        ))
        .and_then(|mut statement| {
            statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, i64>(7)?,
                        row.get::<_, i64>(8)?,
                        row.get::<_, Vec<u8>>(9)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(backup_error)?;
    if receipt_rows.len() > MAX_BACKUP_COMPANION_RECEIPTS {
        return Err(ProviderBackupSourceError::InvalidData);
    }
    let receipts = receipt_rows
        .into_iter()
        .map(|row| {
            let persona_id = (row.3 != "__default__")
                .then(|| row.3.parse::<PersonaId>())
                .transpose()
                .map_err(|_| ProviderBackupSourceError::InvalidData)?;
            validate_persona_key(&row.3, persona_id)?;
            Ok(lettuce_transfer::BackupCompanionStateReceipt {
                receipt: lettuce_companions::CompanionStateApplyReceipt {
                    operation_id: row
                        .0
                        .parse::<OperationRecordId>()
                        .map_err(|_| ProviderBackupSourceError::InvalidData)?,
                    owner: lettuce_companions::CompanionStateOwner {
                        conversation_id: row
                            .1
                            .parse()
                            .map_err(|_| ProviderBackupSourceError::InvalidData)?,
                        character_id: row
                            .2
                            .parse()
                            .map_err(|_| ProviderBackupSourceError::InvalidData)?,
                        persona_id,
                    },
                    expected_session_revision: backup_revision(row.4)?,
                    resulting_session_revision: backup_revision(row.5)?,
                    expected_relationship_revision: backup_revision(row.6)?,
                    resulting_relationship_revision: backup_revision(row.7)?,
                    applied_at: TimestampMillis::new(row.8),
                },
                change_hash: backup_hash(&row.9)?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CompanionStateBackup {
        version: COMPANION_STATE_BACKUP_VERSION,
        relationships,
        sessions,
        episodes,
        receipts,
    })
}

fn backup_revision(value: i64) -> Result<Revision, ProviderBackupSourceError> {
    let value = u64::try_from(value).map_err(|_| ProviderBackupSourceError::InvalidData)?;
    if value == 0 {
        return Err(ProviderBackupSourceError::InvalidData);
    }
    Ok(Revision::new(value))
}

fn backup_hash(value: &[u8]) -> Result<ContentHash, ProviderBackupSourceError> {
    let bytes: [u8; 32] = value
        .try_into()
        .map_err(|_| ProviderBackupSourceError::InvalidData)?;
    ContentHash::parse(blake3::Hash::from_bytes(bytes).to_hex().to_string())
        .map_err(|_| ProviderBackupSourceError::InvalidData)
}

fn validate_persona_key(
    key: &str,
    persona_id: Option<PersonaId>,
) -> Result<(), ProviderBackupSourceError> {
    let expected = persona_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| "__default__".to_owned());
    if key == expected {
        Ok(())
    } else {
        Err(ProviderBackupSourceError::InvalidData)
    }
}

impl ProviderBackupSource for Database {
    fn read_provider_backup_graph(&self) -> Result<ProviderBackupGraph, ProviderBackupSourceError> {
        let mut connection = self
            .connection()
            .map_err(|_| ProviderBackupSourceError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(backup_error)?;
        let accounts = transaction
            .prepare(
                "SELECT id,provider_kind,protocol,label,endpoint,enabled,api_key_secret_ref,secret_owner_id,secret_headers_json,config_json,revision,created_at,updated_at,streaming_enabled,allow_invalid_tls FROM provider_accounts ORDER BY id",
            )
            .and_then(|mut statement| {
                statement
                    .query_map([], provider_from_row)?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(backup_error)?;
        let profiles = transaction
            .prepare(
                "SELECT id,provider_account_id,external_model_id,display_name,kind,config_json,revision,created_at,updated_at FROM model_profiles ORDER BY id",
            )
            .and_then(|mut statement| {
                statement
                    .query_map([], model_from_row)?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(backup_error)?;
        let prompt_ids = transaction
            .prepare("SELECT id FROM prompt_documents ORDER BY id")
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| parse_id(row.get(0)?))?
                    .collect::<rusqlite::Result<Vec<PromptDocumentId>>>()
            })
            .map_err(backup_error)?;
        let prompts = prompt_ids
            .into_iter()
            .map(|id| {
                crate::prompt_adapter::load_document(&transaction, id)?
                    .ok_or(rusqlite::Error::InvalidQuery)
            })
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(backup_error)?;
        let (selections, settings) = transaction
            .query_row(
                "SELECT default_model_profile_id,dynamic_memory_model_profile_id,group_speaker_model_profile_id,default_prompt_document_id,format_version,payload_json,revision,created_at,updated_at FROM app_settings WHERE id=1",
                [],
                |row| {
                    if row.get::<_, u32>(4)? != lettuce_settings::GLOBAL_SETTINGS_FORMAT_VERSION {
                        return Err(rusqlite::Error::InvalidQuery);
                    }
                    let payload = row.get::<_, String>(5)?;
                    Ok((
                        ProviderBackupSelections {
                            default_model_profile_id: row
                                .get::<_, Option<String>>(0)?
                                .map(parse_id::<ModelProfileId>)
                                .transpose()?,
                            dynamic_memory_model_profile_id: row
                                .get::<_, Option<String>>(1)?
                                .map(parse_id::<ModelProfileId>)
                                .transpose()?,
                            group_speaker_model_profile_id: row
                                .get::<_, Option<String>>(2)?
                                .map(parse_id::<ModelProfileId>)
                                .transpose()?,
                            default_prompt_document_id: row
                                .get::<_, Option<String>>(3)?
                                .map(parse_id::<PromptDocumentId>)
                                .transpose()?,
                        },
                        BackupGlobalSettings {
                            value: serde_json::from_str(&payload)
                                .map_err(|_| rusqlite::Error::InvalidQuery)?,
                            revision: Revision::new(
                                u64::try_from(row.get::<_, i64>(6)?)
                                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                            ),
                            created_at: TimestampMillis::new(row.get(7)?),
                            updated_at: TimestampMillis::new(row.get(8)?),
                        },
                    ))
                },
            )
            .optional()
            .map_err(backup_error)?
            .ok_or(ProviderBackupSourceError::InvalidData)?;
        let audio_providers = transaction
            .prepare(&format!(
                "{} ORDER BY id",
                crate::tts_adapter::PROVIDER_SELECT
            ))
            .and_then(|mut statement| {
                statement
                    .query_map([], crate::tts_adapter::provider_from_row)?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(backup_error)?;
        let user_voices = transaction
            .prepare(&format!("{} ORDER BY id", crate::tts_adapter::VOICE_SELECT))
            .and_then(|mut statement| {
                statement
                    .query_map([], crate::tts_adapter::voice_from_row)?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(backup_error)?;
        let persona_ids = read_ids::<PersonaId>(
            &transaction,
            &format!(
                "SELECT id FROM personas ORDER BY id LIMIT {}",
                MAX_BACKUP_AUTHORED_ROOTS + 1
            ),
        )?;
        let personas = persona_ids
            .iter()
            .copied()
            .map(|id| {
                crate::persona_adapter::load_persona(&transaction, id)?
                    .ok_or(rusqlite::Error::InvalidQuery)
            })
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(backup_error)?;
        let persona_default =
            crate::persona_adapter::read_default(&transaction).map_err(backup_error)?;
        let lorebook_ids = read_ids::<LorebookId>(
            &transaction,
            &format!(
                "SELECT id FROM lorebooks ORDER BY id LIMIT {}",
                MAX_BACKUP_AUTHORED_ROOTS + 1
            ),
        )?;
        let lorebooks = lorebook_ids
            .iter()
            .copied()
            .map(|id| {
                crate::lorebook_adapter::load_details(&transaction, id)?
                    .ok_or(rusqlite::Error::InvalidQuery)
            })
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(backup_error)?;
        let character_ids = read_ids::<CharacterId>(
            &transaction,
            &format!(
                "SELECT id FROM characters ORDER BY id LIMIT {}",
                MAX_BACKUP_AUTHORED_ROOTS + 1
            ),
        )?;
        let characters = character_ids
            .iter()
            .copied()
            .map(|id| {
                crate::character_adapter::load_details(&transaction, id)?
                    .ok_or(rusqlite::Error::InvalidQuery)
            })
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(backup_error)?;
        let group_ids = read_ids::<GroupId>(
            &transaction,
            &format!(
                "SELECT id FROM groups ORDER BY id LIMIT {}",
                MAX_BACKUP_AUTHORED_ROOTS + 1
            ),
        )?;
        let groups = group_ids
            .iter()
            .copied()
            .map(|id| {
                crate::group_adapter::load_details(&transaction, id)?
                    .ok_or(rusqlite::Error::InvalidQuery)
            })
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(backup_error)?;
        let character_lorebooks = read_bindings(
            &transaction,
            crate::lorebook_adapter::OwnerKind::Character,
            &character_ids,
        )?;
        let persona_lorebooks = read_bindings(
            &transaction,
            crate::lorebook_adapter::OwnerKind::Persona,
            &persona_ids,
        )?;
        let group_lorebooks = read_bindings(
            &transaction,
            crate::lorebook_adapter::OwnerKind::Group,
            &group_ids,
        )?;
        let asset_ids = read_ids::<AssetId>(
            &transaction,
            &format!(
                "SELECT id FROM media_assets ORDER BY id LIMIT {}",
                MAX_BACKUP_MEDIA_RECORDS + 1
            ),
        )?;
        let media_assets = asset_ids
            .into_iter()
            .map(|id| {
                crate::load_asset_with_blob(&transaction, id)?.ok_or(rusqlite::Error::InvalidQuery)
            })
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(backup_error)?;
        let media_blobs = transaction
            .prepare(&format!(
                "SELECT {} FROM media_blobs ORDER BY id LIMIT {}",
                crate::MEDIA_BLOB_COLUMNS,
                MAX_BACKUP_MEDIA_RECORDS + 1
            ))
            .and_then(|mut statement| {
                statement
                    .query_map([], crate::media_from_row)?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(backup_error)?;
        let learning =
            crate::speech_learning_adapter::read_all_learning(&transaction).map_err(|error| {
                match error {
                    lettuce_speech::AsrLearningRepositoryError::InvalidData => {
                        ProviderBackupSourceError::InvalidData
                    }
                    _ => ProviderBackupSourceError::Storage,
                }
            })?;
        let media_assets_by_id = media_assets
            .iter()
            .map(|asset| (asset.id, asset))
            .collect::<std::collections::BTreeMap<_, _>>();
        let media_blobs_by_id = media_blobs
            .iter()
            .map(|blob| (blob.id, blob))
            .collect::<std::collections::BTreeMap<_, _>>();
        let audio_assets = learning
            .voice_examples
            .iter()
            .map(|example| example.audio_asset_id)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .map(|asset_id| {
                let asset = media_assets_by_id
                    .get(&asset_id)
                    .ok_or(ProviderBackupSourceError::InvalidData)?;
                let blob = media_blobs_by_id
                    .get(&asset.blob_id)
                    .ok_or(ProviderBackupSourceError::InvalidData)?;
                if blob.state != lettuce_media::BlobState::Ready
                    || blob.kind != lettuce_media::MediaKind::Audio
                {
                    return Err(ProviderBackupSourceError::InvalidData);
                }
                Ok(AsrLearningAudioAsset {
                    asset_id,
                    kind: asset.kind,
                    origin: asset.origin,
                    provenance: asset.provenance.clone(),
                    content_hash: blob.content_hash.clone(),
                    byte_size: blob.byte_size,
                    mime_type: blob.mime_type.clone(),
                    duration_ms: blob.duration_ms,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let asr_learning = AsrLearningDocument {
            version: ASR_LEARNING_DOCUMENT_VERSION,
            vocabulary: learning.vocabulary,
            corrections: learning.corrections,
            ignored_suggestions: learning.ignored_suggestions,
            voice_examples: learning.voice_examples,
            audio_assets,
        };
        asr_learning
            .validate()
            .map_err(|_| ProviderBackupSourceError::InvalidData)?;
        let conversation_history = read_conversation_history(&transaction)?;
        let conversation_runtime = read_conversation_runtime(&transaction, &conversation_history)?;
        let job_backup = read_job_backup(&transaction)?;
        let conversation_usage = read_conversation_usage(&transaction, &job_backup)?;
        let conversation_outbox = read_conversation_outbox(&transaction, &conversation_history)?;
        let companion_state = read_companion_state(&transaction)?;
        let companion_effects = read_companion_effects(&transaction)?;
        let memory = read_memory(&transaction)?;
        let memory_projections = read_memory_projections(&transaction)?;
        transaction.commit().map_err(backup_error)?;
        Ok(ProviderBackupGraph {
            version: PROVIDER_BACKUP_GRAPH_VERSION,
            accounts,
            profiles,
            prompts,
            selections,
            settings,
            audio_providers,
            user_voices,
            authored: AuthoredProfileBackup {
                personas,
                persona_default,
                lorebooks,
                characters,
                groups,
                character_lorebooks,
                persona_lorebooks,
                group_lorebooks,
                media_assets,
                media_blobs,
            },
            asr_learning,
            conversation_history,
            conversation_runtime,
            job_backup,
            conversation_usage,
            conversation_outbox,
            companion_state,
            companion_effects,
            memory,
            memory_projections,
        })
    }
}

fn read_conversation_outbox(
    transaction: &rusqlite::Transaction<'_>,
    history: &ConversationHistoryBackup,
) -> Result<ConversationOutboxBackup, ProviderBackupSourceError> {
    let mut conversations = history
        .conversations
        .iter()
        .map(|conversation| {
            (
                conversation.aggregate.conversation.id,
                BackupConversationOutbox {
                    conversation_id: conversation.aggregate.conversation.id,
                    operations: Vec::new(),
                    events: Vec::new(),
                },
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut statement = transaction
        .prepare(&format!(
            "SELECT id,conversation_id,kind,operation_key,request_digest,result_kind,result_id,result_json,created_at FROM conversation_operations ORDER BY conversation_id,created_at,id LIMIT {}",
            MAX_BACKUP_CONVERSATION_OPERATIONS + 1
        ))
        .map_err(backup_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, i64>(8)?,
            ))
        })
        .map_err(backup_error)?;
    let mut operation_count = 0_usize;
    for row in rows {
        let (
            id,
            conversation_id,
            kind,
            key,
            digest,
            result_kind,
            result_id,
            result_json,
            created_at,
        ) = row.map_err(backup_error)?;
        operation_count += 1;
        if operation_count > MAX_BACKUP_CONVERSATION_OPERATIONS {
            return Err(ProviderBackupSourceError::InvalidData);
        }
        let conversation_id: ConversationId = conversation_id
            .parse()
            .map_err(|_| ProviderBackupSourceError::InvalidData)?;
        let kind = crate::conversation_query::operation_kind(&kind)
            .map_err(|_| ProviderBackupSourceError::InvalidData)?;
        let result: lettuce_conversations::OperationResultRef =
            crate::conversation_vertical_slice::decode(&result_json)
                .map_err(|_| ProviderBackupSourceError::InvalidData)?;
        let (expected_kind, expected_id) = match &result {
            lettuce_conversations::OperationResultRef::Conversation(id) => {
                ("conversation", id.to_string())
            }
            lettuce_conversations::OperationResultRef::Turn(id) => ("turn", id.to_string()),
            lettuce_conversations::OperationResultRef::Message(id) => ("message", id.to_string()),
            lettuce_conversations::OperationResultRef::Candidate(id) => {
                ("candidate", id.to_string())
            }
            lettuce_conversations::OperationResultRef::Branch(id) => ("branch", id.to_string()),
        };
        if result_kind != expected_kind || result_id != expected_id {
            return Err(ProviderBackupSourceError::InvalidData);
        }
        crate::conversation_query::validate_operation_result_ownership(
            transaction,
            conversation_id,
            &result,
        )
        .map_err(|_| ProviderBackupSourceError::InvalidData)?;
        conversations
            .get_mut(&conversation_id)
            .ok_or(ProviderBackupSourceError::InvalidData)?
            .operations
            .push(lettuce_conversations::OperationRecord {
                id: id
                    .parse()
                    .map_err(|_| ProviderBackupSourceError::InvalidData)?,
                conversation_id,
                kind,
                operation: lettuce_conversations::OperationToken {
                    key: key
                        .parse()
                        .map_err(|_| ProviderBackupSourceError::InvalidData)?,
                    request_digest: digest
                        .parse()
                        .map_err(|_| ProviderBackupSourceError::InvalidData)?,
                },
                result,
                created_at: TimestampMillis::new(created_at),
            });
    }
    drop(statement);
    let mut statement = transaction
        .prepare(&format!(
            "SELECT conversation_id,id,sequence,conversation_revision,operation_record_id,at,event_json FROM conversation_outbox ORDER BY conversation_id,sequence,id LIMIT {}",
            MAX_BACKUP_CONVERSATION_OUTBOX_EVENTS + 1
        ))
        .map_err(backup_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(backup_error)?;
    let mut event_count = 0_usize;
    for row in rows {
        let (conversation_id, id, sequence, revision, operation_id, at, event_json) =
            row.map_err(backup_error)?;
        event_count += 1;
        if event_count > MAX_BACKUP_CONVERSATION_OUTBOX_EVENTS {
            return Err(ProviderBackupSourceError::InvalidData);
        }
        let conversation_id: ConversationId = conversation_id
            .parse()
            .map_err(|_| ProviderBackupSourceError::InvalidData)?;
        let record = lettuce_conversations::ConversationOutboxRecord {
            format_version: 1,
            id: id
                .parse()
                .map_err(|_| ProviderBackupSourceError::InvalidData)?,
            conversation_id,
            conversation_revision: Revision::new(
                u64::try_from(revision).map_err(|_| ProviderBackupSourceError::InvalidData)?,
            ),
            sequence: u64::try_from(sequence)
                .map_err(|_| ProviderBackupSourceError::InvalidData)?,
            operation_record_id: operation_id
                .parse()
                .map_err(|_| ProviderBackupSourceError::InvalidData)?,
            at: TimestampMillis::new(at),
            event: crate::conversation_vertical_slice::decode(&event_json)
                .map_err(|_| ProviderBackupSourceError::InvalidData)?,
        };
        record
            .validate()
            .map_err(|_| ProviderBackupSourceError::InvalidData)?;
        crate::conversation_query::validate_outbox_event_timestamp(&record)
            .and_then(|()| crate::conversation_query::validate_outbox_event(transaction, &record))
            .map_err(|_| ProviderBackupSourceError::InvalidData)?;
        conversations
            .get_mut(&conversation_id)
            .ok_or(ProviderBackupSourceError::InvalidData)?
            .events
            .push(record);
    }
    Ok(ConversationOutboxBackup {
        version: CONVERSATION_OUTBOX_BACKUP_VERSION,
        conversations: conversations.into_values().collect(),
    })
}

fn read_conversation_usage(
    transaction: &rusqlite::Transaction<'_>,
    jobs: &JobBackup,
) -> Result<ConversationUsageBackup, ProviderBackupSourceError> {
    let usage = crate::usage_adapter::load_all_usage_in(transaction)
        .map_err(|_| ProviderBackupSourceError::InvalidData)?;
    if usage.len() > MAX_BACKUP_CONVERSATION_USAGE_EVENTS {
        return Err(ProviderBackupSourceError::InvalidData);
    }
    let mut costs = std::collections::BTreeMap::new();
    let mut statement = transaction
        .prepare("SELECT event_id,basis_json FROM usage_costs ORDER BY event_id")
        .map_err(backup_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(backup_error)?;
    for row in rows {
        let (event_id, basis) = row.map_err(backup_error)?;
        let event_id: lettuce_types::UsageEventId = event_id
            .parse()
            .map_err(|_| ProviderBackupSourceError::InvalidData)?;
        let basis = crate::decode_versioned(&basis, 1)
            .map_err(|_| ProviderBackupSourceError::InvalidData)?;
        if costs.insert(event_id, basis).is_some() {
            return Err(ProviderBackupSourceError::InvalidData);
        }
    }
    let overlaps = jobs.inference.iter().fold(
        std::collections::BTreeMap::<_, Vec<_>>::new(),
        |mut values, dispatch| {
            values
                .entry(dispatch.evidence.logical_attempt_id)
                .or_default()
                .push(dispatch.evidence.id);
            values
        },
    );
    let events = usage
        .into_iter()
        .map(|event| BackupConversationUsage {
            cost_basis: costs.remove(&event.id),
            overlapping_job_inference_ids: overlaps
                .get(&event.record.attempt_id)
                .cloned()
                .unwrap_or_default(),
            event,
        })
        .collect();
    if !costs.is_empty() {
        return Err(ProviderBackupSourceError::InvalidData);
    }
    Ok(ConversationUsageBackup {
        version: CONVERSATION_USAGE_BACKUP_VERSION,
        events,
    })
}

fn read_job_backup(
    transaction: &rusqlite::Transaction<'_>,
) -> Result<JobBackup, ProviderBackupSourceError> {
    for (table, limit) in [
        ("jobs", MAX_BACKUP_JOBS),
        ("job_events", MAX_BACKUP_JOB_EVENTS),
        ("job_inference_usage", MAX_BACKUP_JOB_INFERENCE_EVENTS),
    ] {
        let count: i64 = transaction
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .map_err(backup_error)?;
        if usize::try_from(count).map_err(|_| ProviderBackupSourceError::InvalidData)? > limit {
            return Err(ProviderBackupSourceError::InvalidData);
        }
    }
    let records = crate::job_adapter::load_store(transaction)
        .map_err(|_| ProviderBackupSourceError::InvalidData)?
        .stored_records();
    let mut inference = Vec::new();
    let mut statement = transaction
        .prepare(
            "SELECT u.id,u.job_id,u.admitted_at,u.record_json,u.result_json,c.basis_json \
             FROM job_inference_usage u LEFT JOIN job_usage_costs c ON c.event_id=u.id \
             ORDER BY u.job_id,u.admitted_at,u.id",
        )
        .map_err(backup_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })
        .map_err(backup_error)?;
    for row in rows {
        let (id, job_id, admitted_at, record, result, cost_basis) = row.map_err(backup_error)?;
        let mut evidence: lettuce_usage::JobInferenceUsage = crate::decode_versioned(&record, 1)
            .map_err(|_| ProviderBackupSourceError::InvalidData)?;
        evidence.result = result
            .as_deref()
            .map(|value| crate::decode_versioned(value, 1))
            .transpose()
            .map_err(|_| ProviderBackupSourceError::InvalidData)?;
        if id != evidence.id.to_string()
            || job_id != evidence.job_id.to_string()
            || admitted_at != evidence.admitted_at.get()
        {
            return Err(ProviderBackupSourceError::InvalidData);
        }
        let cost_basis = cost_basis
            .as_deref()
            .map(|value| crate::decode_versioned(value, 1))
            .transpose()
            .map_err(|_| ProviderBackupSourceError::InvalidData)?;
        inference.push(BackupJobInference {
            evidence,
            cost_basis,
        });
    }
    Ok(JobBackup {
        version: JOB_BACKUP_VERSION,
        jobs: records,
        inference,
    })
}

fn read_conversation_runtime(
    transaction: &rusqlite::Transaction<'_>,
    history: &ConversationHistoryBackup,
) -> Result<ConversationRuntimeBackup, ProviderBackupSourceError> {
    let mut turn_count = 0_usize;
    let mut checkpoint_count = 0_usize;
    let mut tool_count = 0_usize;
    let conversations = history
        .conversations
        .iter()
        .map(|history| {
            let conversation_id = history.aggregate.conversation.id;
            let remaining = MAX_BACKUP_GENERATION_TURNS.saturating_sub(turn_count);
            let limit = remaining.saturating_add(1);
            let sql = format!(
                "{} WHERE conversation_id = ?1 ORDER BY created_at, id LIMIT ?2",
                crate::conversation_query::turn_select_sql()
            );
            let mut statement = transaction.prepare(&sql).map_err(backup_error)?;
            let turns = statement
                .query_map(
                    params![
                        conversation_id.to_string(),
                        i64::try_from(limit).map_err(|_| ProviderBackupSourceError::InvalidData)?
                    ],
                    |row| {
                        crate::conversation_query::hydrate_turn_row(transaction, row)
                            .map_err(|_| rusqlite::Error::InvalidQuery)
                    },
                )
                .map_err(backup_error)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(backup_error)?;
            if turns.len() > remaining {
                return Err(ProviderBackupSourceError::InvalidData);
            }
            turn_count += turns.len();
            let turns = turns
                .into_iter()
                .map(|turn| {
                    let attempts = turn
                        .attempts
                        .iter()
                        .map(|attempt| {
                            read_attempt_runtime(
                                transaction,
                                &turn,
                                attempt.id,
                                attempt.job_id,
                                &mut checkpoint_count,
                                &mut tool_count,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(BackupGenerationTurn { turn, attempts })
                })
                .collect::<Result<Vec<_>, ProviderBackupSourceError>>()?;
            Ok(BackupConversationRuntime {
                conversation_id,
                turns,
            })
        })
        .collect::<Result<Vec<_>, ProviderBackupSourceError>>()?;
    Ok(ConversationRuntimeBackup {
        version: CONVERSATION_RUNTIME_BACKUP_VERSION,
        conversations,
    })
}

fn read_attempt_runtime(
    transaction: &rusqlite::Transaction<'_>,
    turn: &lettuce_conversations::GenerationTurn,
    attempt_id: GenerationAttemptId,
    job_id: Option<lettuce_types::JobId>,
    checkpoint_count: &mut usize,
    tool_count: &mut usize,
) -> Result<BackupGenerationAttemptRuntime, ProviderBackupSourceError> {
    let remaining = MAX_BACKUP_GENERATION_CHECKPOINTS.saturating_sub(*checkpoint_count);
    let mut statement = transaction
        .prepare("SELECT sequence, job_id, correlation_id, event_json, created_at FROM generation_checkpoints WHERE conversation_id = ?1 AND turn_id = ?2 AND attempt_id = ?3 ORDER BY sequence LIMIT ?4")
        .map_err(backup_error)?;
    let checkpoints = statement
        .query_map(
            params![
                turn.conversation_id.to_string(),
                turn.id.to_string(),
                attempt_id.to_string(),
                i64::try_from(remaining.saturating_add(1))
                    .map_err(|_| ProviderBackupSourceError::InvalidData)?
            ],
            |row| {
                Ok(BackupGenerationCheckpoint {
                    envelope: lettuce_conversations::GenerationCheckpointEnvelope {
                        turn_id: turn.id,
                        attempt_id,
                        job_id: row
                            .get::<_, Option<String>>(1)?
                            .map(|value| value.parse().map_err(|_| rusqlite::Error::InvalidQuery))
                            .transpose()?,
                        correlation_id: row
                            .get::<_, Option<String>>(2)?
                            .map(|value| value.parse().map_err(|_| rusqlite::Error::InvalidQuery))
                            .transpose()?,
                        sequence: u64::try_from(row.get::<_, i64>(0)?)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        event: crate::decode_versioned(&row.get::<_, String>(3)?, 1)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    },
                    created_at: TimestampMillis::new(row.get(4)?),
                })
            },
        )
        .map_err(backup_error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(backup_error)?;
    if checkpoints.len() > remaining {
        return Err(ProviderBackupSourceError::InvalidData);
    }
    *checkpoint_count += checkpoints.len();
    let remaining = MAX_BACKUP_TOOL_EXECUTIONS.saturating_sub(*tool_count);
    let sql = format!(
        "{} WHERE e.conversation_id = ?1 AND e.turn_id = ?2 AND e.attempt_id = ?3 ORDER BY e.ordinal, e.id LIMIT ?4",
        crate::tool_adapter::SELECT_EXECUTION
    );
    let mut statement = transaction.prepare(&sql).map_err(backup_error)?;
    let tools = statement
        .query_map(
            params![
                turn.conversation_id.to_string(),
                turn.id.to_string(),
                attempt_id.to_string(),
                i64::try_from(remaining.saturating_add(1))
                    .map_err(|_| ProviderBackupSourceError::InvalidData)?
            ],
            |row| {
                crate::tool_adapter::hydrate(transaction, row)
                    .map_err(|_| rusqlite::Error::InvalidQuery)
            },
        )
        .map_err(backup_error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(backup_error)?;
    if tools.len() > remaining {
        return Err(ProviderBackupSourceError::InvalidData);
    }
    *tool_count += tools.len();
    let initial_inference = job_id
        .map(|job_id| {
            crate::initial_inference_adapter::load_for_attempt_in(
                transaction,
                turn.conversation_id,
                turn.id,
                attempt_id,
                job_id,
            )
        })
        .transpose()
        .map_err(|_| ProviderBackupSourceError::InvalidData)?
        .flatten();
    let speaker_inference = read_speaker_inference(
        transaction,
        turn.conversation_id,
        turn.id,
        attempt_id,
        job_id,
    )?;
    Ok(BackupGenerationAttemptRuntime {
        attempt_id,
        checkpoints,
        speaker_inference,
        initial_inference,
        tools,
    })
}

fn read_speaker_inference(
    transaction: &rusqlite::Transaction<'_>,
    conversation_id: ConversationId,
    turn_id: lettuce_types::GenerationTurnId,
    attempt_id: GenerationAttemptId,
    expected_job_id: Option<lettuce_types::JobId>,
) -> Result<Option<lettuce_conversations::SpeakerInferenceRecord>, ProviderBackupSourceError> {
    let row = transaction
        .query_row(
            "SELECT job_id, request_fingerprint, admitted_at, decision_json, settled_at, usage_event_id FROM generation_speaker_dispatches WHERE conversation_id = ?1 AND turn_id = ?2 AND attempt_id = ?3",
            params![conversation_id.to_string(), turn_id.to_string(), attempt_id.to_string()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?, row.get::<_, i64>(2)?, row.get::<_, Option<String>>(3)?, row.get::<_, Option<i64>>(4)?, row.get::<_, String>(5)?)),
        )
        .optional()
        .map_err(backup_error)?;
    let Some((job_id, fingerprint, admitted_at, decision, settled_at, usage_event_id)) = row else {
        return Ok(None);
    };
    let job_id = job_id
        .parse()
        .map_err(|_| ProviderBackupSourceError::InvalidData)?;
    let request_fingerprint: [u8; 32] = fingerprint
        .try_into()
        .map_err(|_| ProviderBackupSourceError::InvalidData)?;
    let decision = decision
        .as_deref()
        .map(crate::conversation_vertical_slice::decode)
        .transpose()
        .map_err(|_| ProviderBackupSourceError::InvalidData)?;
    if expected_job_id != Some(job_id) || decision.is_some() != settled_at.is_some() {
        return Err(ProviderBackupSourceError::InvalidData);
    }
    Ok(Some(lettuce_conversations::SpeakerInferenceRecord {
        binding: lettuce_conversations::SpeakerInferenceBinding {
            conversation_id,
            turn_id,
            attempt_id,
            job_id,
            request_fingerprint,
        },
        usage_event_id: usage_event_id
            .parse()
            .map_err(|_| ProviderBackupSourceError::InvalidData)?,
        admitted_at: TimestampMillis::new(admitted_at),
        decision,
        settled_at: settled_at.map(TimestampMillis::new),
    }))
}

fn read_conversation_history(
    transaction: &rusqlite::Transaction<'_>,
) -> Result<ConversationHistoryBackup, ProviderBackupSourceError> {
    let ids = read_ids::<ConversationId>(
        transaction,
        &format!(
            "SELECT id FROM conversations ORDER BY id LIMIT {}",
            MAX_BACKUP_CONVERSATIONS + 1
        ),
    )?;
    if ids.len() > MAX_BACKUP_CONVERSATIONS {
        return Err(ProviderBackupSourceError::InvalidData);
    }
    let mut message_count = 0_usize;
    let mut revision_count = 0_usize;
    let mut candidate_count = 0_usize;
    let conversations = ids
        .into_iter()
        .map(|conversation_id| {
            let aggregate = crate::conversation_vertical_slice::hydrate_conversation(
                transaction,
                conversation_id,
                || {},
            )
            .map_err(|_| ProviderBackupSourceError::InvalidData)?;
            let messages = read_backup_messages(
                transaction,
                conversation_id,
                &mut message_count,
                &mut revision_count,
                &mut candidate_count,
            )?;
            Ok(BackupConversation {
                aggregate,
                messages,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ConversationHistoryBackup {
        version: CONVERSATION_HISTORY_BACKUP_VERSION,
        conversations,
    })
}

fn read_backup_messages(
    transaction: &rusqlite::Transaction<'_>,
    conversation_id: ConversationId,
    message_count: &mut usize,
    revision_count: &mut usize,
    candidate_count: &mut usize,
) -> Result<Vec<BackupMessage>, ProviderBackupSourceError> {
    let remaining = MAX_BACKUP_MESSAGES.saturating_sub(*message_count);
    let limit = remaining.saturating_add(1);
    let mut statement = transaction
        .prepare("SELECT m.conversation_id, m.id, m.branch_id, m.parent_message_id, m.author_participant_id, m.role, m.logical_time, m.effective_time, m.visibility, m.pinned, m.scene_edited, m.timeline_ordinal, m.active_revision_id, m.active_candidate_id, m.revision, m.created_at, m.updated_at FROM conversation_messages AS m WHERE m.conversation_id = ?1 ORDER BY m.timeline_ordinal, m.id LIMIT ?2")
        .map_err(backup_error)?;
    let mut rows = statement
        .query(params![
            conversation_id.to_string(),
            i64::try_from(limit).map_err(|_| ProviderBackupSourceError::InvalidData)?
        ])
        .map_err(backup_error)?;
    let mut messages = Vec::new();
    while let Some(row) = rows.next().map_err(backup_error)? {
        let (item, ordinal) = crate::conversation_query::message_row(transaction, row)
            .map_err(|_| ProviderBackupSourceError::InvalidData)?;
        let revisions = read_backup_revisions(
            transaction,
            conversation_id,
            item.message.id,
            revision_count,
        )?;
        let candidates = read_backup_candidates(
            transaction,
            conversation_id,
            item.message.id,
            candidate_count,
        )?;
        messages.push(BackupMessage {
            message: item.message,
            timeline_ordinal: u64::try_from(ordinal)
                .map_err(|_| ProviderBackupSourceError::InvalidData)?,
            initial_origin: item.initial_origin,
            revisions,
            candidates,
        });
    }
    if messages.len() > remaining {
        return Err(ProviderBackupSourceError::InvalidData);
    }
    *message_count += messages.len();
    Ok(messages)
}

fn read_backup_revisions(
    transaction: &rusqlite::Transaction<'_>,
    conversation_id: ConversationId,
    message_id: lettuce_types::MessageId,
    count: &mut usize,
) -> Result<Vec<lettuce_conversations::MessageRevision>, ProviderBackupSourceError> {
    let remaining = MAX_BACKUP_MESSAGE_REVISIONS.saturating_sub(*count);
    let limit = remaining.saturating_add(1);
    let mut statement = transaction
        .prepare("SELECT conversation_id, id, message_id, branch_id, sequence, parts_json, authored_at, provider_replay_artifact_id, provider_replay_retention, source_turn_id FROM conversation_message_revisions WHERE conversation_id = ?1 AND message_id = ?2 ORDER BY sequence, id LIMIT ?3")
        .map_err(backup_error)?;
    let values = statement
        .query_map(
            params![
                conversation_id.to_string(),
                message_id.to_string(),
                i64::try_from(limit).map_err(|_| ProviderBackupSourceError::InvalidData)?
            ],
            |row| {
                crate::conversation_query::hydrate_revision_row(transaction, row)
                    .map_err(|_| rusqlite::Error::InvalidQuery)
            },
        )
        .map_err(backup_error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(backup_error)?;
    if values.len() > remaining {
        return Err(ProviderBackupSourceError::InvalidData);
    }
    *count += values.len();
    Ok(values)
}

fn read_backup_candidates(
    transaction: &rusqlite::Transaction<'_>,
    conversation_id: ConversationId,
    message_id: lettuce_types::MessageId,
    count: &mut usize,
) -> Result<Vec<lettuce_conversations::MessageCandidate>, ProviderBackupSourceError> {
    let remaining = MAX_BACKUP_MESSAGE_CANDIDATES.saturating_sub(*count);
    let limit = remaining.saturating_add(1);
    let mut statement = transaction
        .prepare("SELECT conversation_id, id, message_id, branch_id, turn_id, attempt_id, ordinal, parts_json, model_json, created_at, provider_replay_artifact_id, provider_replay_retention, author_participant_id FROM conversation_message_candidates WHERE conversation_id = ?1 AND message_id = ?2 ORDER BY ordinal, id LIMIT ?3")
        .map_err(backup_error)?;
    let values = statement
        .query_map(
            params![
                conversation_id.to_string(),
                message_id.to_string(),
                i64::try_from(limit).map_err(|_| ProviderBackupSourceError::InvalidData)?
            ],
            |row| {
                crate::conversation_query::hydrate_candidate_row(transaction, row)
                    .map_err(|_| rusqlite::Error::InvalidQuery)
            },
        )
        .map_err(backup_error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(backup_error)?;
    if values.len() > remaining {
        return Err(ProviderBackupSourceError::InvalidData);
    }
    *count += values.len();
    Ok(values)
}

fn read_ids<Id>(
    transaction: &rusqlite::Transaction<'_>,
    query: &str,
) -> Result<Vec<Id>, ProviderBackupSourceError>
where
    Id: std::str::FromStr,
{
    transaction
        .prepare(query)
        .and_then(|mut statement| {
            statement
                .query_map([], |row| parse_id(row.get(0)?))?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(backup_error)
}

fn read_bindings<Id>(
    transaction: &rusqlite::Transaction<'_>,
    kind: crate::lorebook_adapter::OwnerKind,
    owner_ids: &[Id],
) -> Result<Vec<BackupLorebookBindings<Id>>, ProviderBackupSourceError>
where
    Id: Copy + ToString,
{
    owner_ids
        .iter()
        .copied()
        .map(|owner_id| {
            crate::lorebook_adapter::read_bindings(transaction, kind, &owner_id.to_string())
                .map(|bindings| BackupLorebookBindings { owner_id, bindings })
                .map_err(|_| ProviderBackupSourceError::InvalidData)
        })
        .collect()
}
