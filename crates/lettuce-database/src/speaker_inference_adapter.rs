use lettuce_conversations::{
    ConversationRepositoryError, GenerationAttemptStatus, GenerationTurnStatus, InferenceRequest,
    SpeakerDecisionMethod, SpeakerInferenceAdmission, SpeakerInferenceBinding,
    SpeakerInferenceRecord, SpeakerInferenceRepository,
};
use lettuce_types::{ConversationId, TimestampMillis, UsageEventId};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::{Database, conversation_mutations, conversation_vertical_slice as slice};

fn verify_usage(
    transaction: &Transaction<'_>,
    record: &SpeakerInferenceRecord,
    decision: &lettuce_conversations::SelectedSpeakerDecision,
) -> Result<(), ConversationRepositoryError> {
    let usage: Option<(String, String)> = transaction
        .query_row(
            "SELECT record_json, result_json FROM job_inference_usage WHERE id = ?1 AND job_id = ?2 AND admitted_at = ?3 AND result_json IS NOT NULL",
            params![record.usage_event_id.to_string(), record.binding.job_id.to_string(), record.admitted_at.get()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(slice::db)?;
    let Some((admission, result)) = usage else {
        return Err(ConversationRepositoryError::Conflict);
    };
    let admission: lettuce_usage::JobInferenceUsage =
        crate::decode_versioned(&admission, 1).map_err(|_| ConversationRepositoryError::Storage)?;
    let result: lettuce_usage::JobInferenceUsageResult =
        crate::decode_versioned(&result, 1).map_err(|_| ConversationRepositoryError::Storage)?;
    let result_matches = match decision.fallback {
        lettuce_conversations::SpeakerFallback::None => {
            matches!(
                result,
                lettuce_usage::JobInferenceUsageResult::Response { .. }
            )
        }
        lettuce_conversations::SpeakerFallback::Heuristic => matches!(
            result,
            lettuce_usage::JobInferenceUsageResult::Response { .. }
                | lettuce_usage::JobInferenceUsageResult::InferenceFailed
        ),
        _ => false,
    };
    if admission.id != record.usage_event_id
        || admission.job_id != record.binding.job_id
        || admission.logical_attempt_id != record.binding.attempt_id
        || admission.admitted_at != record.admitted_at
        || admission.result.is_some()
        || decision.method != SpeakerDecisionMethod::Llm
        || decision.usage_event_id != Some(record.usage_event_id)
        || !result_matches
    {
        return Err(ConversationRepositoryError::Conflict);
    }
    Ok(())
}

fn load(
    transaction: &Transaction<'_>,
    binding: &SpeakerInferenceBinding,
) -> Result<Option<SpeakerInferenceRecord>, ConversationRepositoryError> {
    let row = transaction
        .query_row(
            "SELECT job_id, request_fingerprint, admitted_at, decision_json, settled_at, usage_event_id FROM generation_speaker_dispatches WHERE conversation_id = ?1 AND turn_id = ?2 AND attempt_id = ?3",
            params![binding.conversation_id.to_string(), binding.turn_id.to_string(), binding.attempt_id.to_string()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?, row.get::<_, i64>(2)?, row.get::<_, Option<String>>(3)?, row.get::<_, Option<i64>>(4)?, row.get::<_, String>(5)?)),
        )
        .optional()
        .map_err(slice::db)?;
    let Some((job_id, fingerprint, admitted_at, decision, settled_at, usage_event_id)) = row else {
        return Ok(None);
    };
    if job_id != binding.job_id.to_string() || fingerprint != binding.request_fingerprint {
        return Err(ConversationRepositoryError::Conflict);
    }
    let decision: Option<lettuce_conversations::SelectedSpeakerDecision> =
        decision.as_deref().map(slice::decode).transpose()?;
    if decision.is_some() != settled_at.is_some() || settled_at.is_some_and(|at| at < admitted_at) {
        return Err(ConversationRepositoryError::Storage);
    }
    let record = SpeakerInferenceRecord {
        binding: binding.clone(),
        usage_event_id: usage_event_id
            .parse()
            .map_err(|_| ConversationRepositoryError::Storage)?,
        admitted_at: TimestampMillis::new(admitted_at),
        decision,
        settled_at: settled_at.map(TimestampMillis::new),
    };
    if let Some(decision) = &record.decision {
        decision
            .validate_for_persistence()
            .map_err(|_| ConversationRepositoryError::Storage)?;
        verify_usage(transaction, &record, decision)?;
    }
    Ok(Some(record))
}

impl SpeakerInferenceRepository for Database {
    fn speaker_inference(
        &self,
        binding: &SpeakerInferenceBinding,
    ) -> Result<Option<SpeakerInferenceRecord>, ConversationRepositoryError> {
        let mut connection = self
            .connection()
            .map_err(|_| ConversationRepositoryError::Storage)?;
        let transaction = connection.transaction().map_err(slice::db)?;
        let record = load(&transaction, binding)?;
        transaction.commit().map_err(slice::db)?;
        Ok(record)
    }

    fn admit_speaker_inference(
        &self,
        conversation_id: ConversationId,
        request: &InferenceRequest,
        now: TimestampMillis,
    ) -> Result<SpeakerInferenceAdmission, ConversationRepositoryError> {
        let binding = SpeakerInferenceBinding::from_request(conversation_id, request)
            .map_err(ConversationRepositoryError::Invalid)?;
        let mut connection = self
            .connection()
            .map_err(|_| ConversationRepositoryError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(slice::db)?;
        if let Some(record) = load(&transaction, &binding)? {
            transaction.commit().map_err(slice::db)?;
            return Ok(SpeakerInferenceAdmission {
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
        if turn.status != GenerationTurnStatus::SelectingSpeaker
            || attempt.status != GenerationAttemptStatus::Preparing
            || attempt.job_id != Some(binding.job_id)
            || turn.operation != request.operation
            || turn.selected_speaker.is_some()
        {
            return Err(ConversationRepositoryError::Conflict);
        }
        let job_exists: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM jobs WHERE id = ?1)",
                [binding.job_id.to_string()],
                |row| row.get(0),
            )
            .map_err(slice::db)?;
        if !job_exists {
            return Err(ConversationRepositoryError::Conflict);
        }
        let usage_event_id = UsageEventId::new();
        transaction
            .execute(
                "INSERT INTO generation_speaker_dispatches (usage_event_id, conversation_id, turn_id, attempt_id, job_id, request_fingerprint, admitted_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![usage_event_id.to_string(), conversation_id.to_string(), request.turn_id.to_string(), request.attempt_id.to_string(), binding.job_id.to_string(), &binding.request_fingerprint[..], now.get()],
            )
            .map_err(slice::db)?;
        transaction.commit().map_err(slice::db)?;
        Ok(SpeakerInferenceAdmission {
            record: SpeakerInferenceRecord {
                binding,
                usage_event_id,
                admitted_at: now,
                decision: None,
                settled_at: None,
            },
            created: true,
        })
    }

    fn settle_speaker_inference(
        &self,
        binding: &SpeakerInferenceBinding,
        decision: &lettuce_conversations::SelectedSpeakerDecision,
        now: TimestampMillis,
    ) -> Result<SpeakerInferenceRecord, ConversationRepositoryError> {
        decision
            .validate_for_persistence()
            .map_err(ConversationRepositoryError::Invalid)?;
        if decision.method != SpeakerDecisionMethod::Llm
            || !matches!(
                decision.fallback,
                lettuce_conversations::SpeakerFallback::None
                    | lettuce_conversations::SpeakerFallback::Heuristic
            )
        {
            return Err(ConversationRepositoryError::Conflict);
        }
        let mut connection = self
            .connection()
            .map_err(|_| ConversationRepositoryError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(slice::db)?;
        let mut record =
            load(&transaction, binding)?.ok_or(ConversationRepositoryError::NotFound)?;
        if let Some(prior) = &record.decision {
            if prior != decision {
                return Err(ConversationRepositoryError::Conflict);
            }
            transaction.commit().map_err(slice::db)?;
            return Ok(record);
        }
        let valid_participant: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM conversation_participants WHERE conversation_id = ?1 AND id = ?2 AND role = 'character' AND enabled = 1 AND muted = 0)",
                params![binding.conversation_id.to_string(), decision.participant_id.to_string()],
                |row| row.get(0),
            )
            .map_err(slice::db)?;
        if !valid_participant || now < record.admitted_at {
            return Err(ConversationRepositoryError::Conflict);
        }
        verify_usage(&transaction, &record, decision)?;
        transaction
            .execute(
                "UPDATE generation_speaker_dispatches SET decision_json = ?4, settled_at = ?5 WHERE conversation_id = ?1 AND turn_id = ?2 AND attempt_id = ?3 AND decision_json IS NULL",
                params![binding.conversation_id.to_string(), binding.turn_id.to_string(), binding.attempt_id.to_string(), slice::encode(decision)?, now.get()],
            )
            .map_err(slice::db)?;
        record.decision = Some(decision.clone());
        record.settled_at = Some(now);
        transaction.commit().map_err(slice::db)?;
        Ok(record)
    }
}
