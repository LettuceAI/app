use lettuce_types::{
    ConversationId, GenerationAttemptId, GenerationTurnId, JobId, TimestampMillis, UsageEventId,
};
use serde::{Deserialize, Serialize};

use crate::{
    ConversationRepositoryError, InferenceOutcome, InferenceRequest, PortError, ValidationError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitialInferenceBinding {
    pub conversation_id: ConversationId,
    pub turn_id: GenerationTurnId,
    pub attempt_id: GenerationAttemptId,
    pub job_id: JobId,
    pub request_fingerprint: [u8; 32],
}

impl InitialInferenceBinding {
    pub fn from_request(
        conversation_id: ConversationId,
        request: &InferenceRequest,
    ) -> Result<Self, ValidationError> {
        request.validate()?;
        let encoded_profile =
            serde_json::to_value(&request.profile).map_err(|_| ValidationError::InvalidValue {
                field: "initial_inference.profile",
            })?;
        let decoded_profile: crate::ResolvedInferenceProfile =
            serde_json::from_value(encoded_profile).map_err(|_| ValidationError::InvalidValue {
                field: "initial_inference.profile",
            })?;
        if decoded_profile != request.profile {
            return Err(ValidationError::InvalidValue {
                field: "initial_inference.profile",
            });
        }
        let job_id = request
            .cancellation
            .ok_or(ValidationError::InvalidReference {
                field: "initial_inference.job",
            })?;
        let encoded = serde_json::to_vec(&(
            conversation_id,
            request.turn_id,
            request.attempt_id,
            request.operation,
            &request.profile,
            &request.context,
            job_id,
            &request.media_grants,
            &request.tools,
        ))
        .map_err(|_| ValidationError::InvalidValue {
            field: "initial_inference.request",
        })?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"lettuce-conversation-initial-request-v1\0");
        hasher.update(&encoded);
        Ok(Self {
            conversation_id,
            turn_id: request.turn_id,
            attempt_id: request.attempt_id,
            job_id,
            request_fingerprint: *hasher.finalize().as_bytes(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum InitialInferenceResult {
    Response(InferenceOutcome),
    Failed(PortError),
}

impl InitialInferenceResult {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if let Self::Response(outcome) = self {
            outcome.validate()?;
            let mut references = std::collections::BTreeMap::new();
            for candidate in &outcome.candidates {
                for reference in candidate.provider_replay.iter().chain(
                    candidate
                        .tool_calls
                        .iter()
                        .filter_map(|call| call.provider_replay.as_ref()),
                ) {
                    if references
                        .insert(reference.artifact_id, reference)
                        .is_some_and(|prior| prior != reference)
                    {
                        return Err(ValidationError::InvalidReference {
                            field: "initial_inference.replay",
                        });
                    }
                    if reference.retention != crate::ReplayRetention::Conversation {
                        return Err(ValidationError::InvalidReference {
                            field: "initial_inference.replay",
                        });
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitialInferenceRecord {
    pub binding: InitialInferenceBinding,
    pub usage_event_id: UsageEventId,
    pub admitted_at: TimestampMillis,
    pub result: Option<InitialInferenceResult>,
    pub settled_at: Option<TimestampMillis>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitialInferenceAdmission {
    pub record: InitialInferenceRecord,
    pub created: bool,
}

pub trait InitialInferenceRepository: Send + Sync {
    fn initial_inference(
        &self,
        binding: &InitialInferenceBinding,
    ) -> Result<Option<InitialInferenceRecord>, ConversationRepositoryError>;

    fn admit_initial_inference(
        &self,
        conversation_id: ConversationId,
        request: &InferenceRequest,
        now: TimestampMillis,
    ) -> Result<InitialInferenceAdmission, ConversationRepositoryError>;

    fn settle_initial_inference(
        &self,
        binding: &InitialInferenceBinding,
        result: &InitialInferenceResult,
        now: TimestampMillis,
    ) -> Result<InitialInferenceRecord, ConversationRepositoryError>;
}
