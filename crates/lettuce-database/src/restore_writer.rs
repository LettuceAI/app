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
        for history in histories {
            let conversation_id = history.aggregate.conversation.id;
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
                    history,
                    turns: &turns,
                    usage: &events,
                    snapshots: Vec::new(),
                    creation: crate::conversation_history_writer::HistoricalCreation::Exact {
                        operations,
                        events: events_out,
                    },
                    memory,
                    memory_projections: &projections,
                    companion: None,
                },
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
