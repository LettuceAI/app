use lettuce_types::{RequestId, Revision};
use lettuce_creation::{StagedLorebookPlanningRun, StagedLorebookRepositoryError};
use lettuce_transfer::BackupApiOperationReceipt;
use lettuce_types::TimestampMillis;
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Serialize, de::DeserializeOwned};

use crate::Database;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ApiOperationError {
    #[error("the operation key was already used for a different request")]
    Conflict,
    #[error("the operation receipt is invalid")]
    InvalidData,
    #[error("operation storage failed")]
    Storage,
}

#[derive(Debug)]
pub struct ApiOperationTransaction<'a, 'connection> {
    pub(crate) transaction: &'a Transaction<'connection>,
    pub(crate) changes: &'a crate::change_signal::ChangeSignal,
}

fn storage(_: impl std::fmt::Debug) -> ApiOperationError {
    ApiOperationError::Storage
}
fn invalid(_: impl std::fmt::Debug) -> ApiOperationError {
    ApiOperationError::InvalidData
}

pub(crate) fn lookup_in(
    connection: &Connection,
    command: &str,
    key: &str,
) -> Result<Option<BackupApiOperationReceipt>, ApiOperationError> {
    connection.query_row(
        "SELECT request_digest,result_json,committed_at FROM api_operation_receipts WHERE command=?1 AND client_operation_id=?2",
        params![command, key], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?)),
    ).optional().map_err(storage)?.map(|(request_digest, result, at)| Ok(BackupApiOperationReceipt {
        result_format_version: 2, command: command.to_owned(), client_operation_id: key.to_owned(), request_digest,
        result: crate::decode_versioned(&result, 2).map_err(invalid)?, committed_at: TimestampMillis::new(at),
    })).transpose()
}

pub(crate) fn insert_in(
    connection: &Connection,
    receipt: &BackupApiOperationReceipt,
) -> Result<(), ApiOperationError> {
    if receipt.result_format_version != 2
        || receipt.command.trim().is_empty()
        || receipt.client_operation_id.trim().is_empty()
        || receipt.request_digest.trim().is_empty()
    {
        return Err(ApiOperationError::InvalidData);
    }
    connection.execute("INSERT INTO api_operation_receipts(command,client_operation_id,request_digest,result_json,committed_at) VALUES (?1,?2,?3,?4,?5)",
        params![receipt.command, receipt.client_operation_id, receipt.request_digest,
            crate::encode_versioned(&receipt.result, 2).map_err(invalid)?, receipt.committed_at.get()],
    ).map_err(storage)?;
    Ok(())
}

pub(crate) fn receipts_in(
    connection: &Connection,
) -> Result<Vec<BackupApiOperationReceipt>, ApiOperationError> {
    let keys = connection.prepare("SELECT command,client_operation_id FROM api_operation_receipts ORDER BY command,client_operation_id")
        .map_err(storage)?.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(storage)?.collect::<rusqlite::Result<Vec<_>>>().map_err(storage)?;
    keys.into_iter()
        .map(|(command, key)| {
            lookup_in(connection, &command, &key)?.ok_or(ApiOperationError::InvalidData)
        })
        .collect()
}

impl Database {
    pub fn lookup_api_operation(
        &self,
        command: &str,
        key: &str,
    ) -> Result<Option<BackupApiOperationReceipt>, ApiOperationError> {
        let connection = self.connection().map_err(storage)?;
        lookup_in(&connection, command, key)
    }

    pub fn commit_api_operation<T, E>(
        &self,
        command: &str,
        key: &str,
        digest: &str,
        at: TimestampMillis,
        apply: impl FnOnce(&ApiOperationTransaction<'_, '_>) -> Result<T, E>,
    ) -> Result<T, E>
    where
        T: Serialize + DeserializeOwned,
        E: From<ApiOperationError>,
    {
        if command.trim().is_empty() || key.trim().is_empty() || digest.trim().is_empty() {
            return Err(ApiOperationError::InvalidData.into());
        }
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        if let Some(receipt) = lookup_in(&transaction, command, key)? {
            if receipt.request_digest != digest {
                return Err(ApiOperationError::Conflict.into());
            }
            return serde_json::from_value(receipt.result).map_err(|error| invalid(error).into());
        }
        let result = apply(&ApiOperationTransaction {
            changes: &self.changes,
            transaction: &transaction,
        })?;
        insert_in(
            &transaction,
            &BackupApiOperationReceipt {
                result_format_version: 2,
                command: command.to_owned(),
                client_operation_id: key.to_owned(),
                request_digest: digest.to_owned(),
                result: serde_json::to_value(&result).map_err(invalid)?,
                committed_at: at,
            },
        )?;
        transaction.commit().map_err(storage)?;
        Ok(result)
    }
}

impl ApiOperationTransaction<'_, '_> {
    pub fn create_lorebook(
        &self,
        metadata: lettuce_context::LorebookMetadataDraft,
        entries: Vec<lettuce_context::LorebookEntryDraft>,
        now: TimestampMillis,
    ) -> Result<lettuce_context::LorebookDetails, lettuce_context::LorebookRepositoryError> {
        crate::lorebook::lorebook_adapter::create_in(self.transaction, metadata, entries, now)
    }

    pub fn create_prompt(
        &self,
        metadata: lettuce_context::PromptMetadataDraft,
        entries: Vec<lettuce_context::PromptEntryDraft>,
        now: TimestampMillis,
    ) -> Result<lettuce_context::PromptDocument, lettuce_context::PromptRepositoryError> {
        crate::catalog::prompt_adapter::create_user_draft_in(
            self.transaction,
            metadata,
            entries,
            now,
        )
    }

    pub fn edit_soul_growth(
        &self,
        owner: lettuce_companions::SoulOwner,
        edit: lettuce_companions::SoulUserEdit,
        at: TimestampMillis,
    ) -> Result<Option<lettuce_companions::SoulState>, lettuce_companions::SoulRepositoryError> {
        let state = crate::companion::soul_adapter::get_in(self.transaction, owner)?;
        if let Some(state) = &state
            && let Some(change) = lettuce_companions::prepare_user_edit(state, edit, at)
                .map_err(lettuce_companions::SoulRepositoryError::Invalid)?
        {
            crate::companion::soul_adapter::apply_in(self.transaction, owner, lettuce_types::OperationRecordId::new(), change)?;
        }
        Ok(state)
    }

    pub fn upsert_companion_note(
        &self, note: lettuce_companions::CompanionScheduledNote,
    ) -> Result<lettuce_companions::CompanionScheduledNote, lettuce_companions::CompanionScheduledNoteError> {
        if let Some(existing) = crate::memory::scheduled_note_adapter::load_in(self.transaction, note.id)?
            && existing.character_id != note.character_id
        {
            return Err(lettuce_companions::CompanionScheduledNoteError::NotFound);
        }
        crate::memory::scheduled_note_adapter::upsert_in(self.transaction, note)
    }

    pub fn delete_companion_note(&self, id: uuid::Uuid) -> Result<(), lettuce_companions::CompanionScheduledNoteError> {
        crate::memory::scheduled_note_adapter::delete_in(self.transaction, id)
    }

    pub fn skip_memory_approval(&self, conversation_id: lettuce_types::ConversationId, branch_id: lettuce_types::ConversationBranchId, at: TimestampMillis) -> Result<(), ApiOperationError> {
        self.transaction.execute("UPDATE dynamic_memory_pending_approvals SET pending=0, skipped=1, updated_at=?3 WHERE conversation_id=?1 AND branch_id=?2 AND pending=1",
            params![conversation_id.to_string(), branch_id.to_string(), at.get()]).map_err(storage)?;
        Ok(())
    }

    pub fn admit_soul_writer(
        &self, spec: lettuce_jobs::JobSpec, mut run: lettuce_companions::CompanionSoulWriterRun,
    ) -> Result<lettuce_jobs::JobSnapshot, ApiOperationError> {
        let key = format!("companion-soul-writer-{}", run.request_id);
        let (job, _, _) = crate::job_adapter::admit_job_detail_in(
            self.transaction, spec, &key, &key, &serde_json::json!({"kind": "companion_soul_writer"}),
        ).map_err(storage)?;
        run.job_id = job.id;
        crate::companion::soul_writer_adapter::admit_run_in(self.transaction, run).map_err(storage)?;
        Ok(job)
    }

    pub fn create_audio_provider(
        &self,
        provider: &lettuce_speech::AudioProvider,
    ) -> Result<(), lettuce_speech::TtsConfigurationRepositoryError> {
        crate::media::tts_adapter::insert_audio_provider(self.transaction, provider)
    }

    pub fn save_vocabulary(
        &self,
        term: lettuce_speech::AsrVocabularyTerm,
    ) -> Result<lettuce_speech::AsrVocabularyTerm, lettuce_speech::AsrLearningRepositoryError> {
        crate::media::speech_learning_adapter::save_vocabulary_in(self.transaction, term)
    }

    pub fn save_correction_draft(
        &self,
        draft: lettuce_speech::AsrCorrectionDraft,
        now: TimestampMillis,
    ) -> Result<lettuce_speech::AsrCorrectionRule, lettuce_speech::AsrLearningRepositoryError> {
        crate::media::speech_learning_adapter::save_correction_draft_in(
            self.transaction,
            draft,
            now,
        )
    }

    pub fn save_ignored_suggestion(
        &self,
        suggestion: lettuce_speech::AsrIgnoredSuggestion,
    ) -> Result<lettuce_speech::AsrIgnoredSuggestion, lettuce_speech::AsrLearningRepositoryError>
    {
        crate::media::speech_learning_adapter::save_ignored_suggestion_in(
            self.transaction,
            suggestion,
        )
    }

    pub fn save_voice_example(
        &self,
        example: lettuce_speech::AsrVoiceExample,
    ) -> Result<lettuce_speech::AsrVoiceExample, lettuce_speech::AsrLearningRepositoryError> {
        crate::media::speech_learning_adapter::save_voice_example_in(self.transaction, example)
    }

    pub fn import_learning_batch(
        &self,
        batch: lettuce_speech::AsrLearningBatch,
    ) -> Result<lettuce_speech::AsrLearningImportReceipt, lettuce_speech::AsrLearningRepositoryError>
    {
        crate::media::speech_learning_adapter::import_learning_batch_in(self.transaction, batch)
    }

    pub fn get_vocabulary(
        &self,
        id: lettuce_types::AsrVocabularyTermId,
    ) -> Result<Option<lettuce_speech::AsrVocabularyTerm>, lettuce_speech::AsrLearningRepositoryError>
    {
        Ok(crate::media::speech_learning_adapter::load_vocabulary(
            self.transaction,
            Some(id),
            None,
            &[],
        )?
        .into_iter()
        .next())
    }

    pub fn get_voice_example(
        &self,
        id: lettuce_types::AsrVoiceExampleId,
    ) -> Result<Option<lettuce_speech::AsrVoiceExample>, lettuce_speech::AsrLearningRepositoryError>
    {
        crate::media::speech_learning_adapter::get_voice_example_in(self.transaction, id)
    }

    pub fn ignore_suggestion(
        &self,
        suggestion: lettuce_speech::AsrLearnedSuggestion,
        now: TimestampMillis,
    ) -> Result<lettuce_speech::AsrIgnoredSuggestion, lettuce_speech::AsrLearningRepositoryError>
    {
        use lettuce_speech::AsrLearningRepositoryError;
        suggestion
            .validate()
            .map_err(|_| AsrLearningRepositoryError::InvalidData)?;
        let existing = crate::media::speech_learning_adapter::find_ignored_suggestion_in(
            self.transaction,
            &suggestion.normalized_wrong,
            &suggestion.normalized_correct,
            suggestion.language.as_deref(),
            &suggestion.scope,
            false,
        )?;
        let value = lettuce_speech::AsrIgnoredSuggestion {
            id: existing
                .as_ref()
                .map_or_else(lettuce_types::AsrIgnoredSuggestionId::new, |item| item.id),
            wrong: suggestion.wrong,
            normalized_wrong: suggestion.normalized_wrong,
            correct: suggestion.correct,
            normalized_correct: suggestion.normalized_correct,
            language: suggestion.language,
            scope: suggestion.scope,
            ignored_count: existing.as_ref().map_or(Ok(1), |item| {
                item.ignored_count
                    .checked_add(1)
                    .ok_or(AsrLearningRepositoryError::InvalidData)
            })?,
            last_ignored_at: now,
            created_at: existing.as_ref().map_or(now, |item| item.created_at),
            updated_at: existing
                .as_ref()
                .map_or(now, |item| now.max(item.updated_at)),
        };
        self.save_ignored_suggestion(value)
    }

    pub fn create_user_voice(
        &self,
        voice: &lettuce_speech::UserVoice,
    ) -> Result<(), lettuce_speech::TtsConfigurationRepositoryError> {
        crate::media::tts_adapter::insert_user_voice(self.transaction, voice)
    }
}

impl ApiOperationTransaction<'_, '_> {
    pub fn update_lorebook_metadata(
        &self,
        id: lettuce_types::LorebookId,
        expected: lettuce_types::Revision,
        metadata: lettuce_context::LorebookMetadataDraft,
        now: TimestampMillis,
    ) -> Result<lettuce_context::LorebookDetails, lettuce_context::LorebookRepositoryError> {
        metadata.validate()?;
        let mut details = crate::lorebook::lorebook_adapter::load_required(self.transaction, id)?;
        details.book.name = metadata.name;
        details.book.detection_policy = metadata.detection_policy;
        details.book.icon_asset_id = metadata.icon_asset_id;
        details.book.behavior_version = metadata.behavior_version;
        details.book.revision = expected
            .next()
            .map_err(|_| lettuce_context::LorebookRepositoryError::Conflict)?;
        details.book.updated_at = now;
        crate::lorebook::lorebook_adapter::replace_lorebook_details(
            self.transaction,
            expected,
            &details,
        )
    }

    pub fn mutate_lorebook_entries(
        &self,
        id: lettuce_types::LorebookId,
        expected: lettuce_types::Revision,
        mutations: Vec<lettuce_context::LorebookEntryMutation>,
        now: TimestampMillis,
    ) -> Result<lettuce_context::LorebookDetails, lettuce_context::LorebookRepositoryError> {
        crate::lorebook::lorebook_adapter::mutate_entries_in(
            self.transaction,
            id,
            expected,
            mutations,
            now,
        )
        .map(|result| result.details)
    }

    pub fn set_lorebook_status(
        &self,
        id: lettuce_types::LorebookId,
        expected: lettuce_types::Revision,
        status: lettuce_context::LifecycleStatus,
        now: TimestampMillis,
    ) -> Result<lettuce_context::LorebookDetails, lettuce_context::LorebookRepositoryError> {
        let mut details = crate::lorebook::lorebook_adapter::load_required(self.transaction, id)?;
        details.book.status = status;
        details.book.revision = expected
            .next()
            .map_err(|_| lettuce_context::LorebookRepositoryError::Conflict)?;
        details.book.updated_at = now;
        crate::lorebook::lorebook_adapter::replace_lorebook_details(
            self.transaction,
            expected,
            &details,
        )
    }

    pub fn delete_lorebook(
        &self,
        id: lettuce_types::LorebookId,
        expected: lettuce_types::Revision,
        now: TimestampMillis,
    ) -> Result<crate::SourceDeletion, crate::SourceDeleteError> {
        crate::catalog::source_delete::delete_lorebook_in(self.transaction, id, expected, now)
    }

    pub fn delete_prompt(
        &self,
        id: lettuce_types::PromptDocumentId,
        expected: lettuce_types::Revision,
        now: TimestampMillis,
    ) -> Result<crate::SourceDeletion, crate::SourceDeleteError> {
        crate::catalog::source_delete::delete_prompt_in(self.transaction, id, expected, now)
    }

    pub fn update_prompt(
        &self,
        id: lettuce_types::PromptDocumentId,
        expected: lettuce_types::Revision,
        metadata: lettuce_context::PromptMetadataDraft,
        edits: Vec<lettuce_context::PromptEntryEdit>,
        now: TimestampMillis,
    ) -> Result<lettuce_context::PromptDocument, lettuce_context::PromptRepositoryError> {
        crate::catalog::prompt_adapter::revise_document_in(
            self.transaction,
            id,
            expected,
            metadata,
            edits,
            now,
        )
    }
}

impl ApiOperationTransaction<'_, '_> {
    pub fn retry_staged_lorebook_planner(
        &self,
        request_id: RequestId,
        retry_id: RequestId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError> {
        crate::lorebook::staged_lorebook_adapter::retry_staged_lorebook_planner_in(
            self.transaction,
            request_id,
            retry_id,
            expected_revision,
            now,
        )
    }

    pub fn edit_staged_lorebook_outline(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        outline: Vec<lettuce_creation::StagedLorebookEntryPlan>,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError> {
        crate::lorebook::staged_lorebook_adapter::edit_staged_lorebook_outline_in(
            self.transaction,
            request_id,
            expected_revision,
            outline,
            now,
        )
    }

    pub fn cancel_staged_lorebook(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError> {
        crate::lorebook::staged_lorebook_adapter::cancel_staged_lorebook_in(
            self.transaction,
            request_id,
            expected_revision,
            now,
        )
    }

    pub fn commit_staged_lorebook(
        &self,
        request: lettuce_creation::StagedLorebookCommitRequest,
    ) -> Result<lettuce_creation::StagedLorebookCommitReceipt, StagedLorebookRepositoryError> {
        crate::lorebook::staged_lorebook_adapter::commit_staged_lorebook_in(
            self.transaction,
            request,
        )
    }

    pub fn start_staged_lorebook_planning(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError> {
        crate::lorebook::staged_lorebook_adapter::start_staged_lorebook_planning_in(
            self.transaction,
            request_id,
            expected_revision,
            now,
        )
    }

    pub fn approve_staged_lorebook_outline(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError> {
        crate::lorebook::staged_lorebook_adapter::approve_staged_lorebook_outline_in(
            self.transaction,
            request_id,
            expected_revision,
            now,
        )
    }

    pub fn edit_staged_lorebook_draft(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        edit: lettuce_creation::StagedLorebookDraftEdit,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError> {
        crate::lorebook::staged_lorebook_adapter::edit_staged_lorebook_draft_in(
            self.transaction,
            request_id,
            expected_revision,
            edit,
            now,
        )
    }

    pub fn set_staged_lorebook_draft_approved(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        plan_id: lettuce_types::LorebookEntryId,
        approved: bool,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError> {
        crate::lorebook::staged_lorebook_adapter::set_staged_lorebook_draft_approved_in(
            self.transaction,
            request_id,
            expected_revision,
            plan_id,
            approved,
            now,
        )
    }

    pub fn apply_staged_lorebook_coherence(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        accepted_change_ids: Vec<String>,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError> {
        crate::lorebook::staged_lorebook_adapter::apply_staged_lorebook_coherence_in(
            self.transaction,
            request_id,
            expected_revision,
            accepted_change_ids,
            now,
        )
    }

    pub fn start_staged_lorebook_draft_batch(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        inputs: Option<lettuce_creation::StagedLorebookWriterBatchInputs>,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError> {
        crate::lorebook::staged_lorebook_adapter::start_staged_lorebook_draft_batch_in(
            self.transaction,
            request_id,
            expected_revision,
            inputs,
            now,
        )
    }
}

impl ApiOperationTransaction<'_, '_> {
    pub fn reset_builtin_prompts(
        &self,
        request: lettuce_context::BuiltInReconcileRequest,
        expected: Option<(lettuce_types::PromptDocumentId, lettuce_types::Revision)>,
        now: TimestampMillis,
    ) -> Result<Vec<lettuce_context::BuiltInReconcileOutcome>, lettuce_context::PromptRepositoryError>
    {
        if let Some((id, revision)) = expected {
            let current = crate::catalog::prompt_adapter::load_document(self.transaction, id)
                .map_err(|error| {
                    lettuce_context::PromptRepositoryError::Failure(error.to_string())
                })?
                .ok_or(lettuce_context::PromptRepositoryError::NotFound)?;
            if current.revision != revision {
                return Err(lettuce_context::PromptRepositoryError::Conflict);
            }
        }
        crate::catalog::prompt_adapter::reconcile_built_ins_in(self.transaction, request, now)
            .map_err(|error| lettuce_context::PromptRepositoryError::Failure(error.to_string()))
    }
    pub fn set_default_prompt(
        &self,
        id: Option<lettuce_types::PromptDocumentId>,
        expected: lettuce_types::Revision,
        now: TimestampMillis,
    ) -> Result<
        (
            Option<lettuce_types::PromptDocumentId>,
            lettuce_types::Revision,
        ),
        lettuce_context::PromptRepositoryError,
    > {
        if let Some(id) = id {
            crate::catalog::prompt_adapter::load_document(self.transaction, id)
                .map_err(|error| {
                    lettuce_context::PromptRepositoryError::Failure(error.to_string())
                })?
                .ok_or(lettuce_context::PromptRepositoryError::NotFound)?;
        }
        let current: (Option<String>, i64) = self
            .transaction
            .query_row(
                "SELECT default_prompt_document_id,revision FROM app_settings WHERE id=1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|error| lettuce_context::PromptRepositoryError::Failure(error.to_string()))?;
        if u64::try_from(current.1).ok() != Some(expected.get()) {
            return Err(lettuce_context::PromptRepositoryError::Conflict);
        }
        let next = if current.0 == id.map(|id| id.to_string()) {
            expected
        } else {
            expected.next().map_err(|error| {
                lettuce_context::PromptRepositoryError::Failure(error.to_string())
            })?
        };
        self.transaction.execute("UPDATE app_settings SET default_prompt_document_id=?1,revision=?2,updated_at=?3 WHERE id=1", params![id.map(|id| id.to_string()), i64::try_from(next.get()).map_err(|error| lettuce_context::PromptRepositoryError::Failure(error.to_string()))?, now.get()]).map_err(|error| lettuce_context::PromptRepositoryError::Failure(error.to_string()))?;
        Ok((id, next))
    }
}

impl ApiOperationTransaction<'_, '_> {
    pub fn admit_staged_lorebook_batch(
        &self,
        request_id: RequestId,
        expected: Revision,
        inputs: lettuce_creation::StagedLorebookWriterBatchInputs,
        writers: Vec<(lettuce_jobs::NewJob, crate::LorebookJobInput)>,
        now: TimestampMillis,
    ) -> Result<Vec<lettuce_types::JobId>, lettuce_jobs::StoreError> {
        let current =
            crate::lorebook::staged_lorebook_adapter::load_in(self.transaction, request_id)
                .map_err(|_| lettuce_jobs::StoreError::InvalidData)?
                .ok_or(lettuce_jobs::StoreError::NotFound)?;
        if current
            .project
            .drafts
            .iter()
            .any(|draft| draft.status == lettuce_creation::StagedLorebookDraftStatus::Drafting)
        {
            return Err(lettuce_jobs::StoreError::AlreadyActive);
        }
        if current.project.revision != expected {
            return Err(lettuce_jobs::StoreError::IdempotencyConflict);
        }
        let project =
            crate::lorebook::staged_lorebook_adapter::start_staged_lorebook_draft_batch_in(
                self.transaction,
                request_id,
                expected,
                Some(inputs),
                now,
            )
            .map_err(|_| lettuce_jobs::StoreError::IllegalTransition)?;
        let plan_ids = project
            .project
            .drafts
            .iter()
            .filter(|draft| draft.status == lettuce_creation::StagedLorebookDraftStatus::Drafting)
            .map(|draft| draft.plan_id)
            .collect::<std::collections::HashSet<_>>();
        let provided = writers
            .iter()
            .filter_map(|(_, input)| match input {
                crate::LorebookJobInput::Writer(run)
                    if run.project_request_id == request_id
                        && run.project_revision == project.project.revision =>
                {
                    Some(run.plan_id)
                }
                _ => None,
            })
            .collect::<std::collections::HashSet<_>>();
        if provided != plan_ids || writers.len() != plan_ids.len() {
            return Err(lettuce_jobs::StoreError::InvalidData);
        }
        writers
            .into_iter()
            .map(|(spec, input)| {
                crate::job_adapter::admit_lorebook_job_in(self.transaction, spec, input, None)
                    .map(|(job, _, _)| job.id)
            })
            .collect()
    }
}

impl ApiOperationTransaction<'_, '_> {
    pub fn create_lorebook_project(
        &self,
        project: lettuce_creation::StagedLorebookProject,
    ) -> Result<lettuce_creation::StagedLorebookProject, StagedLorebookRepositoryError> {
        crate::lorebook::staged_lorebook_adapter::insert_pending_project_in(
            self.transaction,
            &project,
        )?;
        Ok(project)
    }
    pub fn cancel_pending_lorebook_project(
        &self,
        id: lettuce_types::CreationWorkflowId,
        expected: Revision,
        now: TimestampMillis,
    ) -> Result<lettuce_creation::StagedLorebookProject, StagedLorebookRepositoryError> {
        let project =
            crate::lorebook::staged_lorebook_adapter::pending_project_in(self.transaction, id)?
                .ok_or(StagedLorebookRepositoryError::Conflict)?;
        if project.revision != expected {
            return Err(StagedLorebookRepositoryError::Conflict);
        }
        let next = project
            .cancel(now)
            .map_err(|_| StagedLorebookRepositoryError::Conflict)?;
        self.transaction.execute("UPDATE creation_staged_lorebook_projects SET revision=?2,project_json=?3 WHERE project_id=?1", params![id.to_string(), i64::try_from(next.revision.get()).map_err(|_| StagedLorebookRepositoryError::Invalid)?, crate::encode_versioned(&next,1).map_err(|_| StagedLorebookRepositoryError::Invalid)?]).map_err(|_| StagedLorebookRepositoryError::Failure)?;
        Ok(next)
    }
    pub fn plan_pending_lorebook_project(
        &self,
        expected: Revision,
        spec: lettuce_jobs::NewJob,
        run: StagedLorebookPlanningRun,
    ) -> Result<lettuce_types::JobId, lettuce_jobs::StoreError> {
        let pending = crate::lorebook::staged_lorebook_adapter::pending_project_in(
            self.transaction,
            run.project.id,
        )
        .map_err(|_| lettuce_jobs::StoreError::InvalidData)?
        .ok_or(lettuce_jobs::StoreError::IdempotencyConflict)?;
        if pending.revision != expected
            || pending.stage != lettuce_creation::StagedLorebookStage::Created
            || pending
                .start_planning(run.project.updated_at)
                .map_err(|_| lettuce_jobs::StoreError::InvalidData)?
                != run.project
        {
            return Err(lettuce_jobs::StoreError::IdempotencyConflict);
        }
        let id = run.project.id;
        let (job, _, _) = crate::job_adapter::admit_lorebook_job_in(
            self.transaction,
            spec,
            crate::LorebookJobInput::Planner(Box::new(run)),
            None,
        )?;
        self.transaction
            .execute(
                "DELETE FROM creation_staged_lorebook_projects WHERE project_id=?1",
                [id.to_string()],
            )
            .map_err(|_| lettuce_jobs::StoreError::Storage)?;
        Ok(job.id)
    }
}

impl ApiOperationTransaction<'_, '_> {
    pub fn lorebook_project_jobs(
        &self,
        project: lettuce_types::CreationWorkflowId,
    ) -> Result<Vec<lettuce_jobs::JobSnapshot>, lettuce_jobs::StoreError> {
        crate::job_adapter::lorebook_project_jobs_in(self.transaction, project)
    }
    pub fn lorebook_exists(
        &self,
        id: lettuce_types::LorebookId,
    ) -> Result<bool, ApiOperationError> {
        self.transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM lorebooks WHERE id=?1)",
                [id.to_string()],
                |row| row.get(0),
            )
            .map_err(storage)
    }
    pub fn prompt_view_sources(
        &self,
        document: &lettuce_context::PromptDocument,
    ) -> Result<(Option<lettuce_types::PromptDocumentId>, bool), ApiOperationError> {
        let default = self
            .transaction
            .query_row(
                "SELECT default_prompt_document_id FROM app_settings WHERE id=1",
                [],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map_err(storage)?
            .flatten()
            .map(|id| id.parse().map_err(invalid))
            .transpose()?;
        let deleted = match document.provenance {
            lettuce_context::PromptProvenance::Derived { source, .. } => !self
                .transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM prompt_documents WHERE id=?1)",
                    [source.to_string()],
                    |row| row.get::<_, bool>(0),
                )
                .map_err(storage)?,
            _ => false,
        };
        Ok((default, deleted))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receipts_survive_backup_restore_and_device_carry_without_entering_sync() {
        use lettuce_sync::LocalChangeJournal;
        use lettuce_transfer::{ProviderBackupRestoreWriter, ProviderBackupSource};
        let source = Database::open_in_memory().expect("source");
        source
            .journal_current_state(TimestampMillis::new(1))
            .expect("baseline");
        let baseline: i64 = source
            .connection()
            .expect("connection")
            .query_row("SELECT count(*) FROM sync_changes", [], |row| row.get(0))
            .expect("baseline changes");
        let result: String = source
            .commit_api_operation(
                "future_create",
                "key",
                "digest",
                TimestampMillis::new(1),
                |_| Ok::<_, ApiOperationError>("original".into()),
            )
            .expect("receipt");
        let mut graph = source.read_provider_backup_graph().expect("backup graph");
        lettuce_transfer::canonicalize_and_validate(&mut graph).expect("valid graph");
        assert_eq!(graph.job_backup.api_operation_receipts.len(), 1);
        let restored = Database::open_in_memory().expect("restored");
        restored
            .restore_provider_backup_graph(&graph, &[])
            .expect("restore");
        let replay: String = restored
            .commit_api_operation(
                "future_create",
                "key",
                "digest",
                TimestampMillis::new(2),
                |_| -> Result<String, ApiOperationError> {
                    panic!("restored replay");
                },
            )
            .expect("restored receipt");
        assert_eq!(replay, result);
        source
            .journal_current_state(TimestampMillis::new(2))
            .expect("journal");
        let connection = source.connection().expect("connection");
        let changes: i64 = connection
            .query_row("SELECT count(*) FROM sync_changes", [], |row| row.get(0))
            .expect("journal count");
        assert_eq!(changes, baseline, "receipts are device-local bookkeeping");
        drop(connection);
        let root = std::env::temp_dir().join(format!(
            "lettuce-receipt-carry-{}",
            lettuce_types::OperationId::new()
        ));
        std::fs::create_dir_all(&root).expect("root");
        let previous_path = root.join("previous.sqlite3");
        let previous = Database::open(&previous_path).expect("previous");
        previous
            .commit_api_operation(
                "another_future_create",
                "old-key",
                "old-digest",
                TimestampMillis::new(1),
                |_| Ok::<_, ApiOperationError>("previous".to_owned()),
            )
            .expect("previous receipt");
        let next = Database::open(root.join("next.sqlite3")).expect("next");
        next.carry_device_local_state_from(&previous_path)
            .expect("carry receipts");
        assert_eq!(
            next.lookup_api_operation("another_future_create", "old-key")
                .expect("lookup")
                .expect("receipt")
                .result,
            serde_json::json!("previous")
        );
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn concurrent_same_key_applies_once() {
        let database = std::sync::Arc::new(Database::open_in_memory().expect("database"));
        let applied = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let threads = (0..8)
            .map(|_| {
                let database = database.clone();
                let applied = applied.clone();
                std::thread::spawn(move || {
                    database
                        .commit_api_operation(
                            "future_create",
                            "key",
                            "digest",
                            TimestampMillis::new(1),
                            |_| {
                                applied.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                                Ok::<_, ApiOperationError>("one result".to_owned())
                            },
                        )
                        .expect("concurrent replay")
                })
            })
            .collect::<Vec<_>>();
        for thread in threads {
            assert_eq!(thread.join().expect("thread"), "one result");
        }
        assert_eq!(applied.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn receipts_replay_conflict_and_rollback_with_the_callers_changes() {
        let database = Database::open_in_memory().expect("database");
        let apply =
            |transaction: &ApiOperationTransaction<'_, '_>| -> Result<String, ApiOperationError> {
                transaction.transaction.execute("INSERT INTO app_usage_days(day,active_ms,updated_at) VALUES ('2026-10-05',1,1)", []).map_err(storage)?;
                Ok("original".into())
            };
        let first: String = database
            .commit_api_operation(
                "future_create",
                "key",
                "digest",
                TimestampMillis::new(1),
                apply,
            )
            .expect("commit");
        database
            .connection()
            .expect("connection")
            .execute("DELETE FROM app_usage_days", [])
            .expect("later deletion");
        let replay: String = database
            .commit_api_operation(
                "future_create",
                "key",
                "digest",
                TimestampMillis::new(2),
                |_| -> Result<String, ApiOperationError> { panic!("replay must not apply") },
            )
            .expect("replay");
        assert_eq!(first, replay);
        let conflict = database.commit_api_operation(
            "future_create",
            "key",
            "changed",
            TimestampMillis::new(2),
            |_| -> Result<String, ApiOperationError> { panic!("conflict must not apply") },
        );
        assert_eq!(conflict, Err(ApiOperationError::Conflict));
        let failed = database.commit_api_operation("future_create", "failed", "digest", TimestampMillis::new(3), |transaction| -> Result<String, ApiOperationError> {
            transaction.transaction.execute("INSERT INTO app_usage_days(day,active_ms,updated_at) VALUES ('2026-10-05',1,1)", []).map_err(storage)?;
            Err(ApiOperationError::InvalidData)
        });
        assert_eq!(failed, Err(ApiOperationError::InvalidData));
        assert!(
            database
                .lookup_api_operation("future_create", "failed")
                .expect("lookup")
                .is_none()
        );
        let count: i64 = database
            .connection()
            .expect("connection")
            .query_row("SELECT count(*) FROM app_usage_days", [], |row| row.get(0))
            .expect("count");
        assert_eq!(count, 0);
    }
    #[test]
    fn library_and_pending_project_mutations_roll_back_when_receipt_write_fails() {
        use lettuce_context::*;
        use lettuce_transfer::ProviderBackupSource;
        let database = Database::open_in_memory().expect("database");
        let now = TimestampMillis::new(1);
        let metadata = LorebookMetadataDraft {
            name: "World".into(),
            detection_policy: DetectionPolicy::RecentMessageWindow,
            icon_asset_id: None,
            behavior_version: LorebookBehaviorVersion::LegacyV1,
        };
        let book =
            LorebookRepository::create(&database, metadata.clone(), vec![], now).expect("book");
        let prompt_metadata = PromptMetadataDraft {
            name: "Prompt".into(),
            purpose: PromptPurpose::DirectChat,
            condense: false,
            behavior_version: PromptBehaviorVersion::LegacyV1,
        };
        let prompt =
            PromptRepository::create_user_draft(&database, prompt_metadata.clone(), vec![], now)
                .expect("prompt");
        let project = lettuce_creation::StagedLorebookProject::create(
            lettuce_types::CreationWorkflowId::new(),
            "World building".into(),
            Some("World".into()),
            5,
            vec![],
            now,
        )
        .expect("project");
        database
            .commit_api_operation("pending_seed", "seed", "seed", now, |scope| {
                scope
                    .create_lorebook_project(project.clone())
                    .map_err(invalid)
            })
            .expect("pending project");
        database.connection().expect("connection").execute_batch("CREATE TEMP TRIGGER fail_slice6_receipt BEFORE INSERT ON api_operation_receipts BEGIN SELECT RAISE(ABORT,'late receipt failure'); END;").expect("fault");
        let before = database.read_provider_backup_graph().expect("before");
        for case in 0..11 {
            let result: Result<String, ApiOperationError> = database.commit_api_operation(
                "slice6_late_write",
                &case.to_string(),
                "digest",
                TimestampMillis::new(2),
                |scope| {
                    let at = TimestampMillis::new(2);
                    match case {
                        0 => {
                            scope
                                .create_lorebook(metadata.clone(), vec![], at)
                                .map_err(invalid)?;
                        }
                        1 => {
                            scope
                                .update_lorebook_metadata(
                                    book.book.id,
                                    book.book.revision,
                                    LorebookMetadataDraft {
                                        name: "Changed".into(),
                                        ..metadata.clone()
                                    },
                                    at,
                                )
                                .map_err(invalid)?;
                        }
                        2 => {
                            scope
                                .set_lorebook_status(
                                    book.book.id,
                                    book.book.revision,
                                    LifecycleStatus::Archived,
                                    at,
                                )
                                .map_err(invalid)?;
                        }
                        3 => {
                            scope
                                .delete_lorebook(book.book.id, book.book.revision, at)
                                .map_err(invalid)?;
                        }
                        4 => {
                            scope
                                .create_prompt(prompt_metadata.clone(), vec![], at)
                                .map_err(invalid)?;
                        }
                        5 => {
                            scope
                                .update_prompt(
                                    prompt.id,
                                    prompt.revision,
                                    PromptMetadataDraft {
                                        name: "Changed".into(),
                                        ..prompt_metadata.clone()
                                    },
                                    vec![],
                                    at,
                                )
                                .map_err(invalid)?;
                        }
                        6 => {
                            scope
                                .delete_prompt(prompt.id, prompt.revision, at)
                                .map_err(invalid)?;
                        }
                        7 => {
                            scope
                                .create_lorebook_project(
                                    lettuce_creation::StagedLorebookProject::create(
                                        lettuce_types::CreationWorkflowId::new(),
                                        "New world".into(),
                                        None,
                                        5,
                                        vec![],
                                        at,
                                    )
                                    .map_err(invalid)?,
                                )
                                .map_err(invalid)?;
                        }
                        8 => {
                            scope
                                .cancel_pending_lorebook_project(project.id, project.revision, at)
                                .map_err(invalid)?;
                        }
                        9 => {
                            scope
                                .reset_builtin_prompts(
                                    BuiltInReconcileRequest {
                                        mode: BuiltInReconcileMode::ResetToSeed,
                                        seeds: vec![BuiltInPromptSeed {
                                            key: "late_reset".into(),
                                            aliases: vec![],
                                            seed_version: 1,
                                            metadata: prompt_metadata.clone(),
                                            entries: vec![],
                                            required: false,
                                            protected: false,
                                        }],
                                    },
                                    None,
                                    at,
                                )
                                .map_err(invalid)?;
                        }
                        _ => {
                            scope
                                .mutate_lorebook_entries(
                                    book.book.id,
                                    book.book.revision,
                                    vec![LorebookEntryMutation::Replace {
                                        drafts: vec![LorebookEntryDraft {
                                            title: "District".into(),
                                            enabled: true,
                                            always_active: true,
                                            keywords: vec![],
                                            case_sensitive: false,
                                            match_mode: KeywordMatchMode::Literal,
                                            content: "District content".into(),
                                            priority: 0,
                                        }],
                                    }],
                                    at,
                                )
                                .map_err(invalid)?;
                        }
                    }
                    Ok("result".into())
                },
            );
            assert_eq!(result, Err(ApiOperationError::Storage), "case {case}");
            assert_eq!(
                database.read_provider_backup_graph().expect("after"),
                before,
                "case {case}"
            );
            assert!(
                database
                    .lookup_api_operation("slice6_late_write", &case.to_string())
                    .expect("receipt")
                    .is_none()
            );
        }
    }
}
