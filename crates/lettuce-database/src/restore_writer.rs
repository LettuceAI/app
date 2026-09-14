use std::collections::BTreeMap;

use lettuce_characters::{CreateCharacterPlan, CreateGroupPlan};
use lettuce_conversations::{GenerationAttemptStatus, GenerationTurn, GenerationTurnStatus};
use lettuce_transfer::{
    BackupConversationArtifact, ProviderBackupGraph, ProviderBackupRestoreWriteError,
    ProviderBackupRestoreWriter,
};
use lettuce_types::{Revision, UsageEventId};
use rusqlite::{TransactionBehavior, params};

use crate::Database;
use crate::lorebook_adapter::OwnerKind;

type Error = ProviderBackupRestoreWriteError;

fn storage<E>(_: E) -> Error {
    Error::Storage
}

fn invalid<E>(_: E) -> Error {
    Error::InvalidData
}

fn sql_revision(revision: Revision) -> Result<i64, Error> {
    i64::try_from(revision.get()).map_err(invalid)
}

/// Work that was in progress when the backup was taken is restored as
/// interrupted (user decision 2026-09-14); a turn that never started an attempt
/// did no work and is left out.
fn settled_turn(turn: &GenerationTurn) -> Option<GenerationTurn> {
    let terminal_turn = matches!(
        turn.status,
        GenerationTurnStatus::Succeeded
            | GenerationTurnStatus::Failed
            | GenerationTurnStatus::Cancelled
            | GenerationTurnStatus::Interrupted
    );
    if !terminal_turn && turn.attempts.is_empty() {
        return None;
    }
    let mut turn = turn.clone();
    let updated_at = turn.updated_at;
    for attempt in &mut turn.attempts {
        if matches!(
            attempt.status,
            GenerationAttemptStatus::Succeeded
                | GenerationAttemptStatus::Failed
                | GenerationAttemptStatus::Cancelled
                | GenerationAttemptStatus::Interrupted
        ) {
            continue;
        }
        let started_at = attempt.started_at.unwrap_or(updated_at);
        attempt.status = GenerationAttemptStatus::Interrupted;
        attempt.failure = None;
        attempt.started_at = Some(started_at);
        attempt.finished_at = Some(updated_at.max(started_at));
        attempt.usage_event_id.get_or_insert_with(|| {
            UsageEventId::from_uuid(uuid::Uuid::new_v5(
                &attempt.id.as_uuid(),
                b"restore-interrupted",
            ))
        });
    }
    if !terminal_turn {
        turn.status = GenerationTurnStatus::Interrupted;
        turn.failure = None;
        turn.selected_candidate_id = None;
    }
    Some(turn)
}

impl ProviderBackupRestoreWriter for Database {
    fn restore_provider_backup_graph(
        &self,
        graph: &ProviderBackupGraph,
        artifacts: &[BackupConversationArtifact],
    ) -> Result<(), Error> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let populated = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM provider_accounts) OR EXISTS(SELECT 1 FROM prompt_documents) OR EXISTS(SELECT 1 FROM personas) OR EXISTS(SELECT 1 FROM lorebooks) OR EXISTS(SELECT 1 FROM characters) OR EXISTS(SELECT 1 FROM groups) OR EXISTS(SELECT 1 FROM media_blobs) OR EXISTS(SELECT 1 FROM media_assets) OR EXISTS(SELECT 1 FROM model_profiles) OR EXISTS(SELECT 1 FROM audio_providers) OR EXISTS(SELECT 1 FROM user_voices) OR EXISTS(SELECT 1 FROM asr_vocabulary_terms) OR EXISTS(SELECT 1 FROM asr_corrections) OR EXISTS(SELECT 1 FROM asr_voice_examples) OR EXISTS(SELECT 1 FROM legacy_import_runs) OR EXISTS(SELECT 1 FROM backup_restore_admissions) OR EXISTS(SELECT 1 FROM jobs) OR EXISTS(SELECT 1 FROM conversations)",
                [],
                |row| row.get::<_, bool>(0),
            )
            .map_err(storage)?;
        if populated {
            return Err(Error::TargetNotEmpty);
        }
        transaction
            .execute_batch("PRAGMA defer_foreign_keys = ON")
            .map_err(storage)?;
        let authored = &graph.authored;
        for blob in &authored.media_blobs {
            crate::insert_media_blob_row(&transaction, blob).map_err(invalid)?;
        }
        for asset in &authored.media_assets {
            crate::insert_media_asset_row(&transaction, asset).map_err(invalid)?;
        }
        for account in &graph.accounts {
            crate::insert_provider_account_row(&transaction, account).map_err(invalid)?;
        }
        for profile in &graph.profiles {
            crate::insert_model_profile_row(&transaction, profile).map_err(invalid)?;
        }
        for prompt in &graph.prompts {
            crate::prompt_adapter::insert_imported_document(&transaction, prompt)
                .map_err(invalid)?;
        }
        for provider in &graph.audio_providers {
            crate::tts_adapter::insert_audio_provider_row(&transaction, provider)
                .map_err(invalid)?;
        }
        for voice in &graph.user_voices {
            crate::tts_adapter::insert_user_voice_row(&transaction, voice).map_err(invalid)?;
        }
        for persona in &authored.personas {
            crate::persona_adapter::insert_persona(&transaction, persona.clone())
                .map_err(invalid)?;
        }
        for lorebook in &authored.lorebooks {
            crate::lorebook_adapter::insert_lorebook_details(&transaction, lorebook)
                .map_err(invalid)?;
        }
        for details in &authored.characters {
            crate::character_adapter::insert_character_plan(
                &transaction,
                &CreateCharacterPlan {
                    character: details.character.clone(),
                    scenes: details.scenes.clone(),
                    variants: details.variants.clone(),
                    starters: details.starters.clone(),
                },
            )
            .map_err(invalid)?;
        }
        for details in &authored.groups {
            crate::group_adapter::insert_group_rows(
                &transaction,
                &CreateGroupPlan {
                    group: details.group.clone(),
                    starting_scene: details.starting_scene.clone(),
                },
            )
            .map_err(invalid)?;
        }
        for owner in &authored.character_lorebooks {
            crate::lorebook_adapter::insert_bindings_in(
                &transaction,
                OwnerKind::Character,
                &owner.owner_id.to_string(),
                &owner.bindings,
            )
            .map_err(invalid)?;
        }
        for owner in &authored.persona_lorebooks {
            crate::lorebook_adapter::insert_bindings_in(
                &transaction,
                OwnerKind::Persona,
                &owner.owner_id.to_string(),
                &owner.bindings,
            )
            .map_err(invalid)?;
        }
        for owner in &authored.group_lorebooks {
            crate::lorebook_adapter::insert_bindings_in(
                &transaction,
                OwnerKind::Group,
                &owner.owner_id.to_string(),
                &owner.bindings,
            )
            .map_err(invalid)?;
        }
        let default = &authored.persona_default;
        transaction
            .execute(
                "UPDATE persona_defaults SET default_persona_id=?1,revision=?2,created_at=?3,updated_at=?4 WHERE id=1",
                params![
                    default.persona_id.map(|id| id.to_string()),
                    sql_revision(default.revision)?,
                    default.created_at.get(),
                    default.updated_at.get()
                ],
            )
            .map_err(invalid)?;
        let settings = &graph.settings;
        let selections = &graph.selections;
        let payload = serde_json::to_string(&settings.value).map_err(invalid)?;
        transaction
            .execute(
                "UPDATE app_settings SET default_model_profile_id=?1,dynamic_memory_model_profile_id=?2,group_speaker_model_profile_id=?3,default_prompt_document_id=?4,format_version=?5,payload_json=?6,revision=?7,created_at=?8,updated_at=?9 WHERE id=1",
                params![
                    selections.default_model_profile_id.map(|id| id.to_string()),
                    selections
                        .dynamic_memory_model_profile_id
                        .map(|id| id.to_string()),
                    selections
                        .group_speaker_model_profile_id
                        .map(|id| id.to_string()),
                    selections.default_prompt_document_id.map(|id| id.to_string()),
                    lettuce_settings::GLOBAL_SETTINGS_FORMAT_VERSION,
                    payload,
                    sql_revision(settings.revision)?,
                    settings.created_at.get(),
                    settings.updated_at.get()
                ],
            )
            .map_err(invalid)?;
        let learning = &graph.asr_learning;
        crate::speech_learning_adapter::insert_learning_batch_in(
            &transaction,
            lettuce_speech::AsrLearningBatch {
                vocabulary: learning.vocabulary.clone(),
                corrections: learning.corrections.clone(),
                ignored_suggestions: learning.ignored_suggestions.clone(),
                voice_examples: learning.voice_examples.clone(),
            },
        )
        .map_err(invalid)?;
        let jobs = lettuce_jobs::InMemoryJobStore::restore(graph.job_backup.jobs.clone())
            .map_err(invalid)?;
        crate::job_adapter::persist_changes(
            &transaction,
            &BTreeMap::new(),
            &crate::job_adapter::records_by_id(jobs.stored_records()),
        )
        .map_err(invalid)?;
        for inference in &graph.job_backup.inference {
            let evidence = &inference.evidence;
            let record = lettuce_usage::JobInferenceUsage {
                result: None,
                ..evidence.clone()
            };
            transaction
                .execute(
                    "INSERT INTO job_inference_usage (id, job_id, admitted_at, record_json, result_json) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        evidence.id.to_string(),
                        evidence.job_id.to_string(),
                        evidence.admitted_at.get(),
                        crate::encode_versioned(&record, 1).map_err(invalid)?,
                        evidence
                            .result
                            .as_ref()
                            .map(|result| crate::encode_versioned(result, 1))
                            .transpose()
                            .map_err(invalid)?
                    ],
                )
                .map_err(invalid)?;
            if let Some(basis) = &inference.cost_basis {
                transaction
                    .execute(
                        "INSERT INTO job_usage_costs (event_id, basis_json) VALUES (?1, ?2)",
                        params![
                            evidence.id.to_string(),
                            crate::encode_versioned(basis, 1).map_err(invalid)?
                        ],
                    )
                    .map_err(invalid)?;
            }
        }
        let restored_at = lettuce_types::TimestampMillis::now().map_err(storage)?;
        for artifact in artifacts {
            crate::conversation_artifact_adapter::insert_trusted_artifact_in(
                &transaction,
                &artifact.descriptor,
                &artifact.bytes,
                restored_at,
            )
            .map_err(invalid)?;
        }
        let runtime = graph
            .conversation_runtime
            .conversations
            .iter()
            .map(|runtime| (runtime.conversation_id, runtime))
            .collect::<BTreeMap<_, _>>();
        let outbox = graph
            .conversation_outbox
            .conversations
            .iter()
            .map(|outbox| (outbox.conversation_id, outbox))
            .collect::<BTreeMap<_, _>>();
        let mut usage = BTreeMap::<_, Vec<_>>::new();
        for entry in &graph.conversation_usage.events {
            usage
                .entry(entry.event.record.turn_id)
                .or_default()
                .push(entry.event.clone());
        }
        let mut histories = graph
            .conversation_history
            .conversations
            .iter()
            .collect::<Vec<_>>();
        histories.sort_by_key(|history| {
            (
                history.aggregate.conversation.created_at,
                history.aggregate.conversation.id,
            )
        });
        let mut tombstoned = Vec::new();
        for history in histories {
            let conversation_id = history.aggregate.conversation.id;
            let mut restored_history = history.clone();
            for message in &mut restored_history.messages {
                if message.message.visibility
                    == lettuce_conversations::MessageVisibility::Tombstoned
                {
                    message.message.visibility = lettuce_conversations::MessageVisibility::Hidden;
                    tombstoned.push((conversation_id, message.message.id));
                }
            }
            let turns = runtime
                .get(&conversation_id)
                .map(|runtime| {
                    runtime
                        .turns
                        .iter()
                        .filter_map(|turn| settled_turn(&turn.turn))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let attempt_runtime = runtime
                .get(&conversation_id)
                .map(|runtime| {
                    runtime
                        .turns
                        .iter()
                        .flat_map(|turn| turn.attempts.iter().cloned())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let events = turns
                .iter()
                .flat_map(|turn| usage.get(&turn.id).into_iter().flatten().cloned())
                .collect::<Vec<_>>();
            let memory = graph.memory.spaces.iter().find(|space| {
                space.conversation_id == conversation_id
                    || space.shared_conversation_ids.contains(&conversation_id)
            });
            let projections = memory
                .map(|space| {
                    graph
                        .memory_projections
                        .projections
                        .iter()
                        .filter(|projection| projection.space_id == space.snapshot.id)
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let (operations, events_out) = outbox
                .get(&conversation_id)
                .map_or((&[][..], &[][..]), |outbox| {
                    (outbox.operations.as_slice(), outbox.events.as_slice())
                });
            crate::conversation_history_writer::insert_historical_conversation(
                &transaction,
                crate::conversation_history_writer::HistoricalConversation {
                    history: &restored_history,
                    turns: &turns,
                    usage: &events,
                    snapshots: Vec::new(),
                    creation: crate::conversation_history_writer::HistoricalCreation::Exact {
                        operations,
                        events: events_out,
                    },
                    memory,
                    memory_projections: &projections,
                    runtime: &attempt_runtime,
                    companion: None,
                },
            )
            .map_err(invalid)?;
        }
        let persona_key = |persona_id: Option<lettuce_types::PersonaId>| {
            persona_id.map_or_else(|| "__default__".to_owned(), |id| id.to_string())
        };
        let companion = &graph.companion_state;
        for relationship in &companion.relationships {
            let state = &relationship.state;
            transaction
                .execute(
                    "INSERT INTO companion_relationship_states (character_id, persona_key, persona_id, closeness, trust, affection, tension, stability, interaction_count, last_interaction_at, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
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
                        sql_revision(relationship.revision)?,
                        relationship.created_at.get(),
                        relationship.updated_at.get()
                    ],
                )
                .map_err(invalid)?;
        }
        for session in &companion.sessions {
            let owner = session.owner;
            transaction
                .execute(
                    "INSERT INTO companion_session_states (conversation_id, character_id, persona_key, persona_id, initial_hash, confidence, emotional_updated_at, state_updated_at, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        owner.conversation_id.to_string(),
                        owner.character_id.to_string(),
                        persona_key(owner.persona_id),
                        owner.persona_id.map(|id| id.to_string()),
                        crate::hex_decode(session.initial_state_hash.as_str()).map_err(invalid)?,
                        session.emotional_state.confidence,
                        session.emotional_state.updated_at.get(),
                        session.state_updated_at.get(),
                        sql_revision(session.revision)?,
                        session.created_at.get(),
                        session.updated_at.get()
                    ],
                )
                .map_err(invalid)?;
            crate::state_adapter::replace_vectors(
                &transaction,
                owner.conversation_id,
                &session.emotional_state,
            )
            .map_err(invalid)?;
            crate::state_adapter::replace_signals(
                &transaction,
                owner.conversation_id,
                &session.emotional_state,
                &session.active_signals,
            )
            .map_err(invalid)?;
        }
        for episode in &companion.episodes {
            transaction
                .execute(
                    "INSERT INTO companion_continuity_episodes (conversation_id, character_id, persona_key, persona_id, episode_index, previous_conversation_id, started_at, ended_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        episode.conversation_id.to_string(),
                        episode.character_id.to_string(),
                        persona_key(episode.persona_id),
                        episode.persona_id.map(|id| id.to_string()),
                        i64::from(episode.episode_index),
                        episode.previous_conversation_id.map(|id| id.to_string()),
                        episode.started_at.get(),
                        episode.ended_at.map(lettuce_types::TimestampMillis::get),
                        episode.updated_at.get()
                    ],
                )
                .map_err(invalid)?;
        }
        for entry in &companion.receipts {
            let receipt = &entry.receipt;
            transaction
                .execute(
                    "INSERT INTO companion_state_apply_receipts (operation_id, conversation_id, character_id, persona_key, expected_session_revision, resulting_session_revision, expected_relationship_revision, resulting_relationship_revision, applied_at, change_hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![
                        receipt.operation_id.to_string(),
                        receipt.owner.conversation_id.to_string(),
                        receipt.owner.character_id.to_string(),
                        persona_key(receipt.owner.persona_id),
                        sql_revision(receipt.expected_session_revision)?,
                        sql_revision(receipt.resulting_session_revision)?,
                        sql_revision(receipt.expected_relationship_revision)?,
                        sql_revision(receipt.resulting_relationship_revision)?,
                        receipt.applied_at.get(),
                        crate::hex_decode(entry.change_hash.as_str()).map_err(invalid)?
                    ],
                )
                .map_err(invalid)?;
        }
        for soul in &companion.souls {
            for table in ["companion_soul_facts", "companion_soul_states"] {
                transaction
                    .execute(
                        &format!("DELETE FROM {table} WHERE character_id = ?1"),
                        [soul.character_id.to_string()],
                    )
                    .map_err(invalid)?;
            }
            transaction
                .execute(
                    "INSERT INTO companion_soul_states (character_id, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
                    params![
                        soul.character_id.to_string(),
                        sql_revision(soul.revision)?,
                        soul.created_at.get(),
                        soul.updated_at.get()
                    ],
                )
                .map_err(invalid)?;
            crate::soul_adapter::insert_facts(&transaction, soul.character_id, &soul.facts)
                .map_err(invalid)?;
            for receipt in &soul.receipts {
                transaction
                    .execute(
                        "INSERT INTO companion_soul_apply_receipts (operation_id, character_id, expected_revision, resulting_revision, applied_at, change_hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![
                            receipt.operation_id.to_string(),
                            soul.character_id.to_string(),
                            sql_revision(receipt.expected_revision)?,
                            sql_revision(receipt.resulting_revision)?,
                            receipt.applied_at.get(),
                            crate::hex_decode(receipt.change_hash.as_str()).map_err(invalid)?
                        ],
                    )
                    .map_err(invalid)?;
            }
        }
        for note in &companion.scheduled_notes {
            crate::scheduled_note_adapter::insert_restored_note_in(&transaction, note)
                .map_err(invalid)?;
        }
        for pool in &graph.memory.pools {
            transaction
                .execute(
                    "INSERT OR IGNORE INTO companion_memory_pools (character_id, space_id) VALUES (?1, ?2)",
                    params![pool.character_id.to_string(), pool.space_id.to_string()],
                )
                .map_err(invalid)?;
            let stored: String = transaction
                .query_row(
                    "SELECT space_id FROM companion_memory_pools WHERE character_id = ?1",
                    [pool.character_id.to_string()],
                    |row| row.get(0),
                )
                .map_err(invalid)?;
            if stored != pool.space_id.to_string() {
                return Err(Error::InvalidData);
            }
        }
        for receipt in &graph.memory.retrieval_accesses {
            let access = &receipt.access;
            transaction
                .execute(
                    "INSERT INTO memory_retrieval_accesses (conversation_id, turn_id, attempt_id, space_id, expected_revision, resulting_revision, selected_memory_ids_json, promoted_memory_ids_json, accessed_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        access.conversation_id.to_string(),
                        access.turn_id.to_string(),
                        access.attempt_id.to_string(),
                        access.space_id.to_string(),
                        sql_revision(access.expected_revision)?,
                        sql_revision(receipt.resulting_revision)?,
                        serde_json::to_string(&access.selected_memory_ids).map_err(invalid)?,
                        serde_json::to_string(&receipt.promoted_memory_ids).map_err(invalid)?,
                        access.accessed_at.get()
                    ],
                )
                .map_err(invalid)?;
        }
        for approval in &graph.dynamic_memory.pending_approvals {
            transaction
                .execute(
                    "INSERT INTO dynamic_memory_pending_approvals (conversation_id, prompted_message_count, pending, skipped, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        approval.conversation_id.to_string(),
                        i64::try_from(approval.prompted_message_count).map_err(invalid)?,
                        approval.pending,
                        approval.skipped,
                        approval.updated_at.get()
                    ],
                )
                .map_err(invalid)?;
        }
        let mut runs = graph.dynamic_memory.runs.iter().collect::<Vec<_>>();
        runs.sort_by_key(|entry| (entry.run.created_at, entry.run.id));
        for entry in runs {
            crate::dynamic_memory_run_adapter::insert_restored_run_in(&transaction, entry)
                .map_err(invalid)?;
        }
        for effect in &graph.companion_effects.effects {
            crate::state_adapter::insert_restored_effect_in(&transaction, effect)
                .map_err(invalid)?;
        }
        let mut rewinds = graph.companion_effects.rewinds.iter().collect::<Vec<_>>();
        rewinds.sort_by_key(|rewind| (rewind.applied_at, rewind.operation_id));
        for rewind in rewinds {
            crate::dynamic_memory_rewind_adapter::insert_restored_rewind_in(&transaction, rewind)
                .map_err(invalid)?;
        }
        for run in &companion.growth_runs {
            crate::growth_adapter::insert_restored_in(&transaction, run).map_err(invalid)?;
        }
        for run in &companion.consolidation_runs {
            crate::consolidation_adapter::insert_restored_in(&transaction, run).map_err(invalid)?;
        }
        for run in &companion.soul_writer_runs {
            crate::soul_writer_adapter::insert_restored_in(&transaction, run).map_err(invalid)?;
        }
        let creation = &graph.creation;
        for workflow in &creation.workflows {
            crate::creation_adapter::insert_restored_workflow_in(&transaction, workflow)
                .map_err(invalid)?;
        }
        for entry in &creation.lorebook_entry_runs {
            crate::lorebook_entry_run_adapter::insert_restored_in(
                &transaction,
                &entry.run,
                &entry.attempts,
            )
            .map_err(invalid)?;
        }
        for entry in &creation.lorebook_keyword_runs {
            crate::lorebook_keyword_run_adapter::insert_restored_in(
                &transaction,
                &entry.run,
                &entry.attempts,
            )
            .map_err(invalid)?;
        }
        for run in &creation.staged_lorebooks {
            crate::staged_lorebook_adapter::insert_restored_in(&transaction, run)
                .map_err(invalid)?;
        }
        for run in &creation.staged_lorebook_writer_runs {
            crate::staged_lorebook_writer_adapter::insert_restored_in(&transaction, run)
                .map_err(invalid)?;
        }
        for (conversation_id, message_id) in tombstoned {
            transaction
                .execute(
                    "UPDATE conversation_messages SET visibility = 'tombstoned' WHERE conversation_id = ?1 AND id = ?2",
                    params![conversation_id.to_string(), message_id.to_string()],
                )
                .map_err(invalid)?;
        }
        for entry in &graph.conversation_usage.events {
            if let Some(basis) = &entry.cost_basis {
                transaction
                    .execute(
                        "INSERT INTO usage_costs (event_id, basis_json) VALUES (?1, ?2)",
                        params![
                            entry.event.id.to_string(),
                            crate::encode_versioned(basis, 1).map_err(invalid)?
                        ],
                    )
                    .map_err(invalid)?;
            }
        }
        transaction.commit().map_err(|error| match error {
            rusqlite::Error::SqliteFailure(failure, _)
                if failure.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                Error::InvalidData
            }
            _ => Error::Storage,
        })
    }
}
