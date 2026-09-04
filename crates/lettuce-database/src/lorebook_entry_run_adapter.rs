use std::str::FromStr;

use lettuce_creation::{
    LorebookEntryAttemptCheckpoint, LorebookEntryGenerationRun, LorebookEntryRunRepository,
    LorebookEntryRunRepositoryError, validate_lorebook_entry_attempts,
};
use lettuce_types::{
    CharacterId, ConversationId, JobId, LorebookId, ModelProfileId, PersonaId, PromptDocumentId,
    RequestId, Revision, TimestampMillis,
};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::{Database, decode_versioned, encode_versioned};

const RUN_FORMAT_VERSION: u32 = 1;
const ATTEMPTS_FORMAT_VERSION: u32 = 1;

fn failure(_: impl std::fmt::Debug) -> LorebookEntryRunRepositoryError {
    LorebookEntryRunRepositoryError::Failure
}

fn corrupt(_: impl std::fmt::Debug) -> LorebookEntryRunRepositoryError {
    LorebookEntryRunRepositoryError::Corrupt
}

fn parse_id<T: FromStr>(value: &str) -> Result<T, LorebookEntryRunRepositoryError> {
    value
        .parse()
        .map_err(|_| LorebookEntryRunRepositoryError::Corrupt)
}

fn load_in(
    transaction: &Transaction<'_>,
    request_id: RequestId,
) -> Result<Option<LorebookEntryGenerationRun>, LorebookEntryRunRepositoryError> {
    let row = transaction
        .query_row(
            "SELECT job_id, conversation_id, lorebook_id, character_id, persona_id,
                    model_profile_id, prompt_id, prompt_revision, created_at, run_json
             FROM creation_lorebook_entry_runs WHERE request_id = ?1",
            [request_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, String>(9)?,
                ))
            },
        )
        .optional()
        .map_err(corrupt)?;
    let Some((
        job_id,
        conversation_id,
        lorebook_id,
        character_id,
        persona_id,
        model_profile_id,
        prompt_id,
        prompt_revision,
        created_at,
        run_json,
    )) = row
    else {
        return Ok(None);
    };
    let run = decode_versioned::<LorebookEntryGenerationRun>(&run_json, RUN_FORMAT_VERSION)
        .map_err(corrupt)?;
    let prompt_revision = u64::try_from(prompt_revision)
        .ok()
        .filter(|revision| *revision > 0)
        .map(Revision::new)
        .ok_or(LorebookEntryRunRepositoryError::Corrupt)?;
    if run.request_id != request_id
        || run.job_id != parse_id::<JobId>(&job_id)?
        || run.conversation_id != parse_id::<ConversationId>(&conversation_id)?
        || run.lorebook_id != parse_id::<LorebookId>(&lorebook_id)?
        || run.character_id != parse_id::<CharacterId>(&character_id)?
        || run.persona_id
            != persona_id
                .as_deref()
                .map(parse_id::<PersonaId>)
                .transpose()?
        || run.profile.chat_profile.model_profile_id
            != parse_id::<ModelProfileId>(&model_profile_id)?
        || run.prompt_id != parse_id::<PromptDocumentId>(&prompt_id)?
        || run.prompt_revision != prompt_revision
        || run.created_at != TimestampMillis::new(created_at)
        || run.validate().is_err()
    {
        return Err(LorebookEntryRunRepositoryError::Corrupt);
    }
    Ok(Some(run))
}

fn load_attempts_in(
    transaction: &Transaction<'_>,
    request_id: RequestId,
) -> Result<Option<Vec<LorebookEntryAttemptCheckpoint>>, LorebookEntryRunRepositoryError> {
    let encoded = transaction
        .query_row(
            "SELECT attempts_json FROM creation_lorebook_entry_runs WHERE request_id = ?1",
            [request_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(corrupt)?;
    let Some(encoded) = encoded else {
        return Ok(None);
    };
    let attempts =
        decode_versioned::<Vec<LorebookEntryAttemptCheckpoint>>(&encoded, ATTEMPTS_FORMAT_VERSION)
            .map_err(corrupt)?;
    validate_lorebook_entry_attempts(&attempts)
        .map_err(|_| LorebookEntryRunRepositoryError::Corrupt)?;
    Ok(Some(attempts))
}

impl LorebookEntryRunRepository for Database {
    fn admit_lorebook_entry_run(
        &self,
        run: LorebookEntryGenerationRun,
    ) -> Result<LorebookEntryGenerationRun, LorebookEntryRunRepositoryError> {
        run.validate()?;
        let encoded = encode_versioned(&run, RUN_FORMAT_VERSION).map_err(failure)?;
        let attempts = encode_versioned(
            &Vec::<LorebookEntryAttemptCheckpoint>::new(),
            ATTEMPTS_FORMAT_VERSION,
        )
        .map_err(failure)?;
        let mut connection = self.connection().map_err(failure)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let inserted = transaction
            .execute(
                "INSERT OR IGNORE INTO creation_lorebook_entry_runs (
                   request_id, job_id, conversation_id, lorebook_id, character_id, persona_id,
                   model_profile_id, prompt_id, prompt_revision, created_at, run_json, attempts_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    run.request_id.to_string(),
                    run.job_id.to_string(),
                    run.conversation_id.to_string(),
                    run.lorebook_id.to_string(),
                    run.character_id.to_string(),
                    run.persona_id.map(|id| id.to_string()),
                    run.profile.chat_profile.model_profile_id.to_string(),
                    run.prompt_id.to_string(),
                    i64::try_from(run.prompt_revision.get()).map_err(failure)?,
                    run.created_at.get(),
                    encoded,
                    attempts,
                ],
            )
            .map_err(failure)?;
        let stored = load_in(&transaction, run.request_id)?
            .ok_or(LorebookEntryRunRepositoryError::Failure)?;
        if inserted == 0 && stored != run {
            return Err(LorebookEntryRunRepositoryError::Conflict);
        }
        transaction.commit().map_err(failure)?;
        Ok(stored)
    }

    fn load_lorebook_entry_run(
        &self,
        request_id: RequestId,
    ) -> Result<LorebookEntryGenerationRun, LorebookEntryRunRepositoryError> {
        let mut connection = self.connection().map_err(failure)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(failure)?;
        let run =
            load_in(&transaction, request_id)?.ok_or(LorebookEntryRunRepositoryError::NotFound)?;
        transaction.commit().map_err(failure)?;
        Ok(run)
    }

    fn load_lorebook_entry_attempts(
        &self,
        request_id: RequestId,
    ) -> Result<Vec<LorebookEntryAttemptCheckpoint>, LorebookEntryRunRepositoryError> {
        let mut connection = self.connection().map_err(failure)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(failure)?;
        let attempts = load_attempts_in(&transaction, request_id)?
            .ok_or(LorebookEntryRunRepositoryError::NotFound)?;
        transaction.commit().map_err(failure)?;
        Ok(attempts)
    }

    fn commit_lorebook_entry_attempt(
        &self,
        request_id: RequestId,
        checkpoint: LorebookEntryAttemptCheckpoint,
    ) -> Result<Vec<LorebookEntryAttemptCheckpoint>, LorebookEntryRunRepositoryError> {
        checkpoint.validate()?;
        let mut connection = self.connection().map_err(failure)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let mut attempts = load_attempts_in(&transaction, request_id)?
            .ok_or(LorebookEntryRunRepositoryError::NotFound)?;
        if let Some(stored) = attempts.get(usize::from(checkpoint.ordinal)) {
            if stored == &checkpoint {
                transaction.commit().map_err(failure)?;
                return Ok(attempts);
            }
            return Err(LorebookEntryRunRepositoryError::Conflict);
        }
        if usize::from(checkpoint.ordinal) != attempts.len()
            || attempts.last().is_some_and(|attempt| {
                !matches!(
                    attempt.decision,
                    lettuce_creation::LorebookEntryAttemptDecision::StructuredFallback
                )
            })
        {
            return Err(LorebookEntryRunRepositoryError::Conflict);
        }
        attempts.push(checkpoint);
        validate_lorebook_entry_attempts(&attempts)?;
        let encoded = encode_versioned(&attempts, ATTEMPTS_FORMAT_VERSION).map_err(failure)?;
        let updated = transaction
            .execute(
                "UPDATE creation_lorebook_entry_runs SET attempts_json = ?2 WHERE request_id = ?1",
                params![request_id.to_string(), encoded],
            )
            .map_err(failure)?;
        if updated != 1 {
            return Err(LorebookEntryRunRepositoryError::Conflict);
        }
        transaction.commit().map_err(failure)?;
        Ok(attempts)
    }
}
