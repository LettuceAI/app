use std::collections::BTreeMap;

use lettuce_conversations::{
    ConversationRepositoryError, GenerationAttemptStatus, GenerationTurnStatus, InferenceRequest,
    InitialInferenceAdmission, InitialInferenceBinding, InitialInferenceRecord,
    InitialInferenceRepository, InitialInferenceResult, ReplayArtifactRef,
};
use lettuce_types::{ConversationId, TimestampMillis, UsageEventId};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::{
    Database, conversation_artifact_adapter, conversation_mutations,
    conversation_vertical_slice as slice,
};

fn replay_references(result: &InitialInferenceResult) -> BTreeMap<String, ReplayArtifactRef> {
    let mut references = BTreeMap::new();
    if let InitialInferenceResult::Response(outcome) = result {
        for candidate in &outcome.candidates {
            for reference in candidate.provider_replay.iter().chain(
                candidate
                    .tool_calls
                    .iter()
                    .filter_map(|call| call.provider_replay.as_ref()),
            ) {
                references.insert(reference.artifact_id.to_string(), reference.clone());
            }
        }
    }
    references
}

fn verify_usage(
    transaction: &Transaction<'_>,
    record: &InitialInferenceRecord,
    result: &InitialInferenceResult,
) -> Result<(), ConversationRepositoryError> {
    let (admission, evidence): (String, Option<String>) = transaction
        .query_row(
            "SELECT record_json, result_json FROM job_inference_usage WHERE id = ?1",
            [record.usage_event_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(slice::db)?
        .ok_or(ConversationRepositoryError::Conflict)?;
    let admission: lettuce_usage::JobInferenceUsage =
        crate::decode_versioned(&admission, 1).map_err(|_| ConversationRepositoryError::Storage)?;
    let evidence: lettuce_usage::JobInferenceUsageResult =
        crate::decode_versioned(&evidence.ok_or(ConversationRepositoryError::Conflict)?, 1)
            .map_err(|_| ConversationRepositoryError::Storage)?;
    let turn = conversation_mutations::load_turn(
        transaction,
        record.binding.conversation_id,
        record.binding.turn_id,
    )?;
    let model = turn
        .resolved_model
        .as_ref()
        .ok_or(ConversationRepositoryError::Conflict)?;
    if admission.model_profile_id != model.source_id
        || admission.model_revision != model.source_revision
        || admission.provider_account_id != model.provider_account_id
        || admission.provider_account_revision != model.provider_account_revision
        || admission.result.is_some()
    {
        return Err(ConversationRepositoryError::Conflict);
    }
    if admission.id != record.usage_event_id
        || admission.job_id != record.binding.job_id
        || admission.logical_attempt_id != record.binding.attempt_id
        || admission.admitted_at != record.admitted_at
    {
        return Err(ConversationRepositoryError::Conflict);
    }
    let matches = match (result, evidence) {
        (
            InitialInferenceResult::Response(outcome),
            lettuce_usage::JobInferenceUsageResult::Response {
                usage,
                provider_response_id,
            },
        ) => outcome.usage == usage && outcome.provider_response_id == provider_response_id,
        (
            InitialInferenceResult::Failed(lettuce_conversations::PortError::Cancelled),
            lettuce_usage::JobInferenceUsageResult::Cancelled,
        ) => true,
        (
            InitialInferenceResult::Failed(
                lettuce_conversations::PortError::Cancelled
                | lettuce_conversations::PortError::Rejected,
            ),
            lettuce_usage::JobInferenceUsageResult::Response { .. },
        ) => true,
        (
            InitialInferenceResult::Failed(error),
            lettuce_usage::JobInferenceUsageResult::InferenceFailed,
        ) => *error != lettuce_conversations::PortError::Cancelled,
        _ => false,
    };
    if !matches {
        return Err(ConversationRepositoryError::Conflict);
    }
    Ok(())
}

fn load(
    transaction: &Transaction<'_>,
    binding: &InitialInferenceBinding,
) -> Result<Option<InitialInferenceRecord>, ConversationRepositoryError> {
    let row = transaction.query_row(
        "SELECT job_id, request_fingerprint, admitted_at, result_json, settled_at, usage_event_id FROM generation_initial_dispatches WHERE conversation_id = ?1 AND turn_id = ?2 AND attempt_id = ?3",
        params![binding.conversation_id.to_string(), binding.turn_id.to_string(), binding.attempt_id.to_string()],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?, row.get::<_, i64>(2)?, row.get::<_, Option<String>>(3)?, row.get::<_, Option<i64>>(4)?, row.get::<_, String>(5)?)),
    ).optional().map_err(slice::db)?;
    let Some((job_id, fingerprint, admitted_at, result, settled_at, usage_event_id)) = row else {
        return Ok(None);
    };
    if job_id != binding.job_id.to_string() || fingerprint != binding.request_fingerprint {
        return Err(ConversationRepositoryError::Conflict);
    }
    let result: Option<InitialInferenceResult> =
        result.as_deref().map(slice::decode).transpose()?;
    if result.is_some() != settled_at.is_some() || settled_at.is_some_and(|at| at < admitted_at) {
        return Err(ConversationRepositoryError::Storage);
    }
    let mut expected_refs = BTreeMap::new();
    if let Some(result) = &result {
        result
            .validate()
            .map_err(|_| ConversationRepositoryError::Storage)?;
        expected_refs = replay_references(result);
        for reference in expected_refs.values() {
            conversation_artifact_adapter::verify_replay_in_transaction(transaction, reference)
                .map_err(ConversationRepositoryError::ArtifactReference)?;
        }
    }
    let mut statement = transaction.prepare("SELECT artifact_id FROM generation_initial_replay_refs WHERE conversation_id = ?1 AND turn_id = ?2 AND attempt_id = ?3 ORDER BY artifact_id").map_err(slice::db)?;
    let stored_refs = statement
        .query_map(
            params![
                binding.conversation_id.to_string(),
                binding.turn_id.to_string(),
                binding.attempt_id.to_string()
            ],
            |row| row.get::<_, String>(0),
        )
        .map_err(slice::db)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(slice::db)?;
    if stored_refs != expected_refs.into_keys().collect::<Vec<_>>() {
        return Err(ConversationRepositoryError::Storage);
    }
    let record = InitialInferenceRecord {
        binding: binding.clone(),
        usage_event_id: usage_event_id
            .parse()
            .map_err(|_| ConversationRepositoryError::Storage)?,
        admitted_at: TimestampMillis::new(admitted_at),
        result,
        settled_at: settled_at.map(TimestampMillis::new),
    };
    if let Some(result) = &record.result {
        verify_usage(transaction, &record, result)?;
    }
    Ok(Some(record))
}

impl InitialInferenceRepository for Database {
    fn initial_inference(
        &self,
        binding: &InitialInferenceBinding,
    ) -> Result<Option<InitialInferenceRecord>, ConversationRepositoryError> {
        let mut connection = self
            .connection()
            .map_err(|_| ConversationRepositoryError::Storage)?;
        let transaction = connection.transaction().map_err(slice::db)?;
        let result = load(&transaction, binding)?;
        transaction.commit().map_err(slice::db)?;
        Ok(result)
    }

    fn admit_initial_inference(
        &self,
        conversation_id: ConversationId,
        request: &InferenceRequest,
        now: TimestampMillis,
    ) -> Result<InitialInferenceAdmission, ConversationRepositoryError> {
        let binding = InitialInferenceBinding::from_request(conversation_id, request)
            .map_err(ConversationRepositoryError::Invalid)?;
        let mut connection = self
            .connection()
            .map_err(|_| ConversationRepositoryError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(slice::db)?;
        if let Some(record) = load(&transaction, &binding)? {
            transaction.commit().map_err(slice::db)?;
            return Ok(InitialInferenceAdmission {
                record,
                created: false,
            });
        }
        let turn =
            conversation_mutations::load_turn(&transaction, conversation_id, request.turn_id)?;
        let attempt = turn
            .attempts
            .iter()
            .find(|attempt| attempt.id == request.attempt_id)
            .ok_or(ConversationRepositoryError::Conflict)?;
        let model = turn
            .resolved_model
            .as_ref()
            .ok_or(ConversationRepositoryError::Conflict)?;
        let profile = &request.profile.chat_profile;
        if turn.status != GenerationTurnStatus::Running
            || attempt.status != GenerationAttemptStatus::Running
            || attempt.job_id != Some(binding.job_id)
            || turn.operation != request.operation
            || turn.prompt != request.context.attributions.prompt
            || turn.lorebooks != request.context.attributions.lorebooks
            || turn.memory != request.context.attributions.memory
            || model.source_id != profile.model_profile_id
            || model.source_revision != profile.model_revision
            || model.provider_account_id != profile.provider_account_id
            || model.provider_account_revision != profile.provider_account_revision
            || model.provider_protocol != profile.provider_protocol
            || model.external_model_id != profile.external_model_id
        {
            return Err(ConversationRepositoryError::Conflict);
        }
        let has_calls: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM tool_executions WHERE conversation_id = ?1 AND turn_id = ?2 AND attempt_id = ?3)",
            params![conversation_id.to_string(), request.turn_id.to_string(), request.attempt_id.to_string()], |row| row.get(0),
        ).map_err(slice::db)?;
        let job_exists: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM jobs WHERE id = ?1)",
                [binding.job_id.to_string()],
                |row| row.get(0),
            )
            .map_err(slice::db)?;
        if has_calls || !job_exists {
            return Err(ConversationRepositoryError::Conflict);
        }
        let usage_event_id = UsageEventId::new();
        transaction.execute(
            "INSERT INTO generation_initial_dispatches (conversation_id, turn_id, attempt_id, job_id, request_fingerprint, admitted_at, usage_event_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![conversation_id.to_string(), request.turn_id.to_string(), request.attempt_id.to_string(), binding.job_id.to_string(), &binding.request_fingerprint[..], now.get(), usage_event_id.to_string()],
        ).map_err(slice::db)?;
        transaction.commit().map_err(slice::db)?;
        Ok(InitialInferenceAdmission {
            record: InitialInferenceRecord {
                binding,
                usage_event_id,
                admitted_at: now,
                result: None,
                settled_at: None,
            },
            created: true,
        })
    }

    fn settle_initial_inference(
        &self,
        binding: &InitialInferenceBinding,
        result: &InitialInferenceResult,
        now: TimestampMillis,
    ) -> Result<InitialInferenceRecord, ConversationRepositoryError> {
        result
            .validate()
            .map_err(ConversationRepositoryError::Invalid)?;
        let mut connection = self
            .connection()
            .map_err(|_| ConversationRepositoryError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(slice::db)?;
        let mut record =
            load(&transaction, binding)?.ok_or(ConversationRepositoryError::NotFound)?;
        if let Some(prior) = &record.result {
            if prior != result {
                return Err(ConversationRepositoryError::Conflict);
            }
            transaction.commit().map_err(slice::db)?;
            return Ok(record);
        }
        if now < record.admitted_at {
            return Err(ConversationRepositoryError::Conflict);
        }
        verify_usage(&transaction, &record, result)?;
        if let InitialInferenceResult::Response(outcome) = result {
            for candidate in &outcome.candidates {
                for reference in candidate.provider_replay.iter().chain(
                    candidate
                        .tool_calls
                        .iter()
                        .filter_map(|call| call.provider_replay.as_ref()),
                ) {
                    conversation_artifact_adapter::verify_replay_in_transaction(
                        &transaction,
                        reference,
                    )
                    .map_err(ConversationRepositoryError::ArtifactReference)?;
                }
            }
        }
        for id in replay_references(result).keys() {
            transaction.execute(
                "INSERT INTO generation_initial_replay_refs (conversation_id, turn_id, attempt_id, artifact_id, retention) VALUES (?1, ?2, ?3, ?4, 'conversation')",
                params![binding.conversation_id.to_string(), binding.turn_id.to_string(), binding.attempt_id.to_string(), id],
            ).map_err(slice::db)?;
        }
        transaction.execute(
            "UPDATE generation_initial_dispatches SET result_json = ?4, settled_at = ?5 WHERE conversation_id = ?1 AND turn_id = ?2 AND attempt_id = ?3 AND result_json IS NULL",
            params![binding.conversation_id.to_string(), binding.turn_id.to_string(), binding.attempt_id.to_string(), slice::encode(result)?, now.get()],
        ).map_err(slice::db)?;
        record.result = Some(result.clone());
        record.settled_at = Some(now);
        transaction.commit().map_err(slice::db)?;
        Ok(record)
    }
}
