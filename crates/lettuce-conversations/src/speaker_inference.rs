use lettuce_types::{
    ConversationId, GenerationAttemptId, GenerationTurnId, JobId, TimestampMillis, UsageEventId,
};

use crate::{
    ConversationRepositoryError, InferenceRequest, SelectedSpeakerDecision, ValidationError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeakerInferenceBinding {
    pub conversation_id: ConversationId,
    pub turn_id: GenerationTurnId,
    pub attempt_id: GenerationAttemptId,
    pub job_id: JobId,
    pub request_fingerprint: [u8; 32],
}

impl SpeakerInferenceBinding {
    pub fn from_request(
        conversation_id: ConversationId,
        request: &InferenceRequest,
    ) -> Result<Self, ValidationError> {
        request.validate()?;
        let encoded_profile =
            serde_json::to_value(&request.profile).map_err(|_| ValidationError::InvalidValue {
                field: "speaker_inference.profile",
            })?;
        let decoded_profile: crate::ResolvedInferenceProfile =
            serde_json::from_value(encoded_profile).map_err(|_| ValidationError::InvalidValue {
                field: "speaker_inference.profile",
            })?;
        if decoded_profile != request.profile {
            return Err(ValidationError::InvalidValue {
                field: "speaker_inference.profile",
            });
        }
        let job_id = request
            .cancellation
            .ok_or(ValidationError::InvalidReference {
                field: "speaker_inference.job",
            })?;
        let encoded = serde_json::to_vec(&(
            conversation_id,
            request.turn_id,
            request.attempt_id,
            request.operation,
            &request.profile,
            &request.context,
            job_id,
            &request.tools,
        ))
        .map_err(|_| ValidationError::InvalidValue {
            field: "speaker_inference.request",
        })?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"lettuce-conversation-speaker-request-v1\0");
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeakerInferenceRecord {
    pub binding: SpeakerInferenceBinding,
    pub usage_event_id: UsageEventId,
    pub admitted_at: TimestampMillis,
    pub decision: Option<SelectedSpeakerDecision>,
    pub settled_at: Option<TimestampMillis>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeakerInferenceAdmission {
    pub record: SpeakerInferenceRecord,
    pub created: bool,
}

pub trait SpeakerInferenceRepository: Send + Sync {
    fn speaker_inference(
        &self,
        binding: &SpeakerInferenceBinding,
    ) -> Result<Option<SpeakerInferenceRecord>, ConversationRepositoryError>;

    fn admit_speaker_inference(
        &self,
        conversation_id: ConversationId,
        request: &InferenceRequest,
        now: TimestampMillis,
    ) -> Result<SpeakerInferenceAdmission, ConversationRepositoryError>;

    fn settle_speaker_inference(
        &self,
        binding: &SpeakerInferenceBinding,
        decision: &SelectedSpeakerDecision,
        now: TimestampMillis,
    ) -> Result<SpeakerInferenceRecord, ConversationRepositoryError>;
}
