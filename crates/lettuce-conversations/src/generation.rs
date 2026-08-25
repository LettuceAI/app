use lettuce_types::{
    ConversationBranchId, ConversationId, ConversationParticipantId, GenerationAttemptId,
    GenerationTurnId, JobId, LorebookEntryId, MemoryRevisionId, MessageCandidateId, MessageId,
    PromptDocumentId, PromptEntryId, RequestId, Revision, TimestampMillis, UsageEventId,
};
use serde::{Deserialize, Serialize};

use crate::ValidationError;
use crate::snapshot::{ModelSelectionSnapshot, ValidateSnapshot};
use crate::validation::{
    MAX_ATTEMPTS, MAX_PARTS, MAX_REASONING_BYTES, validate_collection,
    validate_revision_timestamps, validate_text, validate_unique,
};

pub use lettuce_jobs::IdempotencyKey;

/// Derives the stable, attempt-scoped scheduler key used when starting a
/// generation job.  Including both IDs prevents retry/recovery attempts from
/// coalescing with their parent attempt.
#[must_use]
pub fn attempt_job_idempotency_key(
    turn_id: GenerationTurnId,
    attempt_id: GenerationAttemptId,
) -> IdempotencyKey {
    IdempotencyKey::new(format!("generation.{turn_id}.{attempt_id}"))
        .expect("generated attempt job key is valid")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationOperation {
    Send,
    Continue,
    Regenerate,
}

/// The durable output target of a generation turn.  This belongs to the turn
/// itself because retries and recovery must be able to reconstruct the exact
/// message/candidate identity without consulting a command envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum GenerationTarget {
    NewAssistant {
        message_id: MessageId,
        parent_message_id: Option<MessageId>,
    },
    ExistingCandidate {
        message_id: MessageId,
        prior_candidate_id: MessageCandidateId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum GenerationInput {
    UserMessage {
        message_id: MessageId,
    },
    ExistingHead {
        head_message_id: MessageId,
    },
    ExistingCandidate {
        message_id: MessageId,
        candidate_id: MessageCandidateId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationTurnStatus {
    Created,
    Preparing,
    SelectingSpeaker,
    ContextPrepared,
    Running,
    CancellationRequested,
    Finalizing,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
    Recovering,
}

impl GenerationTurnStatus {
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        use GenerationTurnStatus::*;
        matches!(
            (self, next),
            (Created, Preparing | CancellationRequested | Cancelled)
                | (
                    Preparing,
                    SelectingSpeaker
                        | ContextPrepared
                        | CancellationRequested
                        | Failed
                        | Interrupted
                )
                | (
                    SelectingSpeaker,
                    ContextPrepared | CancellationRequested | Failed | Interrupted
                )
                | (
                    ContextPrepared,
                    Running | CancellationRequested | Failed | Interrupted
                )
                | (
                    Running,
                    Finalizing | CancellationRequested | Failed | Interrupted
                )
                | (CancellationRequested, Cancelled | Failed | Interrupted)
                | (
                    Finalizing,
                    Succeeded | Failed | Cancelled | Interrupted | Recovering
                )
                | (Interrupted, Recovering | Failed | Cancelled)
                | (Recovering, Preparing | Running | Failed | Cancelled)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum GenerationTransitionError {
    #[error("generation turn lifecycle transition is not allowed")]
    Illegal,
    #[error("generation turn revision overflow")]
    Overflow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptAttribution {
    pub document_id: PromptDocumentId,
    pub revision: Revision,
    /// Exact entries selected from the immutable prompt snapshot for this
    /// assembly.  Keeping entry identity alongside document provenance makes
    /// replay/audit output explainable without re-running activation.
    #[serde(default)]
    pub selected_entry_ids: Vec<PromptEntryId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LorebookAttribution {
    pub lorebook_id: lettuce_types::LorebookId,
    pub revision: Revision,
    /// Exact entries activated from the immutable lorebook snapshot.
    #[serde(default)]
    pub activated_entry_ids: Vec<LorebookEntryId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryAttribution {
    pub revision_id: MemoryRevisionId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedSpeakerDecision {
    pub participant_id: ConversationParticipantId,
    pub method: SpeakerDecisionMethod,
    pub fallback: SpeakerFallback,
    pub reference: Option<SpeakerDecisionReference>,
    pub rationale_summary: Option<String>,
    pub decision_model: Option<ModelSelectionSnapshot>,
    pub usage_event_id: Option<UsageEventId>,
}

impl SelectedSpeakerDecision {
    pub fn validate_for_persistence(&self) -> Result<(), ValidationError> {
        if let Some(rationale) = &self.rationale_summary {
            validate_text(
                "selected_speaker.rationale_summary",
                rationale,
                MAX_REASONING_BYTES,
                false,
            )?;
        }
        if let Some(model) = &self.decision_model {
            model.validate_snapshot("selected_speaker.decision_model")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeakerDecisionMethod {
    Explicit,
    Llm,
    Heuristic,
    RoundRobin,
    Director,
    DirectorAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeakerFallback {
    None,
    Heuristic,
    RoundRobin,
    FirstEligible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "id",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum SpeakerDecisionReference {
    Attempt(GenerationAttemptId),
    Message(MessageId),
    Prompt(PromptDocumentId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationFailureCode {
    InvalidConversation,
    MissingModel,
    ContextUnavailable,
    SpeakerUnavailable,
    ProviderUnavailable,
    ProviderRejected,
    EmptyOutput,
    Cancelled,
    TimedOut,
    RecoveryUnavailable,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationAttempt {
    pub id: GenerationAttemptId,
    pub turn_id: GenerationTurnId,
    pub ordinal: u16,
    pub parent_attempt_id: Option<GenerationAttemptId>,
    pub status: GenerationAttemptStatus,
    /// A unique key for this attempt's job.  Retries and recovery children
    /// must never reuse the parent attempt's key.
    pub job_idempotency_key: IdempotencyKey,
    pub job_id: Option<JobId>,
    pub started_at: Option<TimestampMillis>,
    pub finished_at: Option<TimestampMillis>,
    pub candidate_ids: Vec<MessageCandidateId>,
    pub usage_event_id: Option<UsageEventId>,
    pub failure: Option<GenerationFailureCode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationAttemptStatus {
    Created,
    Preparing,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}

impl GenerationAttempt {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.job_idempotency_key.as_str().is_empty() {
            return Err(ValidationError::Blank {
                field: "generation_attempt.job_idempotency_key",
            });
        }
        if self.ordinal == 0 && self.parent_attempt_id.is_some() {
            return Err(ValidationError::InvalidReference {
                field: "generation_attempt.parent",
            });
        }
        if self.ordinal > 0 && self.parent_attempt_id.is_none() {
            return Err(ValidationError::InvalidReference {
                field: "generation_attempt.parent",
            });
        }
        validate_collection(
            "generation_attempt.candidates",
            &self.candidate_ids,
            MAX_PARTS,
        )?;
        validate_unique(
            "generation_attempt.candidate_ids",
            self.candidate_ids.iter().copied(),
        )?;
        match self.status {
            GenerationAttemptStatus::Created | GenerationAttemptStatus::Preparing
                if self.started_at.is_some() || self.finished_at.is_some() =>
            {
                return Err(ValidationError::Invariant {
                    field: "generation_attempt.timestamps",
                });
            }
            GenerationAttemptStatus::Running
                if self.started_at.is_none() || self.finished_at.is_some() =>
            {
                return Err(ValidationError::Invariant {
                    field: "generation_attempt.timestamps",
                });
            }
            GenerationAttemptStatus::Succeeded
            | GenerationAttemptStatus::Failed
            | GenerationAttemptStatus::Cancelled
            | GenerationAttemptStatus::Interrupted
                if self.started_at.is_none() || self.finished_at.is_none() =>
            {
                return Err(ValidationError::Invariant {
                    field: "generation_attempt.timestamps",
                });
            }
            _ => {}
        }
        if self.failure.is_some() != matches!(self.status, GenerationAttemptStatus::Failed) {
            return Err(ValidationError::Invariant {
                field: "generation_attempt.failure",
            });
        }
        if matches!(
            self.status,
            GenerationAttemptStatus::Succeeded
                | GenerationAttemptStatus::Failed
                | GenerationAttemptStatus::Cancelled
                | GenerationAttemptStatus::Interrupted
        ) && self.usage_event_id.is_none()
        {
            return Err(ValidationError::InvalidReference {
                field: "generation_attempt.usage_event_id",
            });
        }
        Ok(())
    }

    /// Validates that this is a newly-created child attempt for recovery.
    /// Recovery never resumes or mutates the interrupted attempt: it appends
    /// a fresh attempt with a new identity and the interrupted attempt as its
    /// parent.
    pub fn validate_against(
        &self,
        previous_attempt: &GenerationAttempt,
        turn: &GenerationTurn,
    ) -> Result<(), ValidationError> {
        if previous_attempt.turn_id != turn.id || self.turn_id != turn.id {
            return Err(ValidationError::InvalidReference {
                field: "generation_recovery.turn",
            });
        }
        if previous_attempt.status != GenerationAttemptStatus::Interrupted {
            return Err(ValidationError::InvalidValue {
                field: "generation_recovery.previous_status",
            });
        }
        if self.id == previous_attempt.id {
            return Err(ValidationError::Invariant {
                field: "generation_recovery.new_attempt_id",
            });
        }
        let expected_ordinal =
            previous_attempt
                .ordinal
                .checked_add(1)
                .ok_or(ValidationError::OutOfBounds {
                    field: "generation_recovery.ordinal",
                })?;
        if self.ordinal != expected_ordinal {
            return Err(ValidationError::Invariant {
                field: "generation_recovery.ordinal",
            });
        }
        if self.parent_attempt_id != Some(previous_attempt.id) {
            return Err(ValidationError::InvalidReference {
                field: "generation_recovery.parent",
            });
        }
        if self.status != GenerationAttemptStatus::Created
            || self.job_id.is_some()
            || self.started_at.is_some()
            || self.finished_at.is_some()
            || !self.candidate_ids.is_empty()
            || self.usage_event_id.is_some()
            || self.failure.is_some()
        {
            return Err(ValidationError::Invariant {
                field: "generation_recovery.fresh_attempt",
            });
        }
        if self.job_idempotency_key == previous_attempt.job_idempotency_key {
            return Err(ValidationError::Duplicate {
                field: "generation_recovery.job_idempotency_key",
            });
        }
        if !turn
            .attempts
            .iter()
            .any(|attempt| attempt.id == previous_attempt.id)
        {
            return Err(ValidationError::InvalidReference {
                field: "generation_recovery.previous_attempt",
            });
        }
        if turn.attempts.iter().any(|attempt| attempt.id == self.id) {
            return Err(ValidationError::Invariant {
                field: "generation_recovery.duplicate_attempt",
            });
        }
        self.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationTurn {
    pub id: GenerationTurnId,
    pub conversation_id: ConversationId,
    pub branch_id: ConversationBranchId,
    pub operation: GenerationOperation,
    pub input: GenerationInput,
    pub target: GenerationTarget,
    pub swap_roles: bool,
    /// A retry is a new turn, linked to the terminal turn it replaces.  A
    /// normal send/continue/regenerate turn has no source turn.
    pub retry_of_turn_id: Option<GenerationTurnId>,
    pub idempotency_key: IdempotencyKey,
    pub correlation_id: Option<RequestId>,
    pub status: GenerationTurnStatus,
    pub selected_speaker: Option<SelectedSpeakerDecision>,
    pub guidance: Option<String>,
    pub requested_model_override: Option<ModelSelectionSnapshot>,
    pub forced_speaker: Option<ConversationParticipantId>,
    pub resolved_model: Option<ModelSelectionSnapshot>,
    pub prompt: Option<PromptAttribution>,
    pub lorebooks: Vec<LorebookAttribution>,
    pub memory: Option<MemoryAttribution>,
    pub candidate_ids: Vec<MessageCandidateId>,
    pub selected_candidate_id: Option<MessageCandidateId>,
    pub attempts: Vec<GenerationAttempt>,
    pub failure: Option<GenerationFailureCode>,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

impl GenerationTurn {
    pub fn transition_to(
        &mut self,
        next: GenerationTurnStatus,
        now: TimestampMillis,
    ) -> Result<GenerationTransition, GenerationTransitionError> {
        if !self.status.can_transition_to(next) {
            return Err(GenerationTransitionError::Illegal);
        }
        let from = self.status;
        self.status = next;
        self.revision = self
            .revision
            .next()
            .map_err(|_| GenerationTransitionError::Overflow)?;
        self.updated_at = now;
        Ok(GenerationTransition {
            turn_id: self.id,
            from,
            to: next,
            revision: self.revision,
            at: now,
        })
    }

    pub fn validate(&self, is_group: bool) -> Result<(), ValidationError> {
        validate_revision_timestamps(
            "generation_turn.timestamps",
            self.revision,
            self.created_at,
            self.updated_at,
        )?;
        validate_collection("generation_turn.candidates", &self.candidate_ids, MAX_PARTS)?;
        validate_collection("generation_turn.attempts", &self.attempts, MAX_ATTEMPTS)?;
        validate_unique(
            "generation_turn.candidate_ids",
            self.candidate_ids.iter().copied(),
        )?;
        if is_group && self.swap_roles {
            return Err(ValidationError::InvalidReference {
                field: "generation_turn.swap_roles",
            });
        }
        if matches!(self.operation, GenerationOperation::Continue)
            && (self.guidance.is_some() || self.requested_model_override.is_some())
        {
            return Err(ValidationError::InvalidReference {
                field: "generation_turn.continue_request_overrides",
            });
        }
        if self.retry_of_turn_id == Some(self.id) {
            return Err(ValidationError::InvalidReference {
                field: "generation_turn.retry_of_turn_id",
            });
        }
        let target_is_coherent = match (&self.operation, &self.input, &self.target) {
            (
                GenerationOperation::Send,
                GenerationInput::UserMessage {
                    message_id: user_message_id,
                },
                GenerationTarget::NewAssistant {
                    message_id,
                    parent_message_id: Some(parent_message_id),
                },
            ) => message_id != user_message_id && parent_message_id == user_message_id,
            (
                GenerationOperation::Continue,
                GenerationInput::ExistingHead { head_message_id },
                GenerationTarget::NewAssistant {
                    message_id,
                    parent_message_id: Some(parent_message_id),
                },
            ) => message_id != head_message_id && parent_message_id == head_message_id,
            (
                GenerationOperation::Regenerate,
                GenerationInput::ExistingCandidate {
                    message_id,
                    candidate_id,
                },
                GenerationTarget::ExistingCandidate {
                    message_id: target_message_id,
                    prior_candidate_id,
                },
            ) => message_id == target_message_id && candidate_id == prior_candidate_id,
            _ => false,
        };
        if !target_is_coherent {
            return Err(ValidationError::InvalidReference {
                field: "generation_turn.operation_input_target",
            });
        }
        if !is_group && (self.selected_speaker.is_some() || self.forced_speaker.is_some()) {
            return Err(ValidationError::InvalidReference {
                field: "generation_turn.direct_speaker",
            });
        }
        if let Some(selected_speaker) = &self.selected_speaker {
            selected_speaker.validate_for_persistence()?;
        }
        if let Some(guidance) = &self.guidance {
            validate_text(
                "generation_turn.guidance",
                guidance,
                MAX_REASONING_BYTES,
                false,
            )?;
        }
        if let Some(model) = &self.requested_model_override {
            model.validate_snapshot("generation_turn.requested_model_override")?;
        }
        validate_unique(
            "generation_turn.attempt_ids",
            self.attempts.iter().map(|attempt| attempt.id),
        )?;
        validate_unique(
            "generation_turn.job_idempotency_keys",
            self.attempts
                .iter()
                .map(|attempt| attempt.job_idempotency_key.clone()),
        )?;
        for (ordinal, attempt) in self.attempts.iter().enumerate() {
            if attempt.ordinal as usize != ordinal || attempt.turn_id != self.id {
                return Err(ValidationError::Invariant {
                    field: "generation_turn.attempt_ordinals",
                });
            }
            if attempt.job_idempotency_key != attempt_job_idempotency_key(self.id, attempt.id) {
                return Err(ValidationError::Invariant {
                    field: "generation_turn.attempt_job_idempotency_key",
                });
            }
            let expected_parent = ordinal
                .checked_sub(1)
                .and_then(|index| self.attempts.get(index))
                .map(|attempt| attempt.id);
            if attempt.parent_attempt_id != expected_parent {
                return Err(ValidationError::InvalidReference {
                    field: "generation_turn.attempt_parent",
                });
            }
        }
        if let Some(model) = &self.resolved_model {
            model.validate_snapshot("generation_turn.model")?;
        }
        if let Some(attribution) = &self.prompt {
            if attribution.revision.get() == 0 {
                return Err(ValidationError::ZeroRevision);
            }
            validate_collection(
                "generation_turn.prompt_entry_ids",
                &attribution.selected_entry_ids,
                crate::validation::MAX_DOCUMENT_ENTRIES,
            )?;
            validate_unique(
                "generation_turn.prompt_entry_ids",
                attribution.selected_entry_ids.iter().copied(),
            )?;
        }
        for book in &self.lorebooks {
            if book.revision.get() == 0 {
                return Err(ValidationError::ZeroRevision);
            }
            validate_collection(
                "generation_turn.lorebook_entry_ids",
                &book.activated_entry_ids,
                crate::validation::MAX_DOCUMENT_ENTRIES,
            )?;
            validate_unique(
                "generation_turn.lorebook_entry_ids",
                book.activated_entry_ids.iter().copied(),
            )?;
        }
        for attempt in &self.attempts {
            attempt.validate()?;
        }
        if self.candidate_ids.iter().any(|candidate_id| {
            !self
                .attempts
                .iter()
                .any(|attempt| attempt.candidate_ids.contains(candidate_id))
        }) {
            return Err(ValidationError::InvalidReference {
                field: "generation_turn.candidate_attempt",
            });
        }
        if self
            .selected_candidate_id
            .is_some_and(|id| !self.candidate_ids.contains(&id))
        {
            return Err(ValidationError::InvalidReference {
                field: "generation_turn.selected_candidate",
            });
        }
        if self.failure.is_some() != matches!(self.status, GenerationTurnStatus::Failed) {
            return Err(ValidationError::Invariant {
                field: "generation_turn.failure",
            });
        }
        #[allow(clippy::collapsible_match)]
        match self.status {
            GenerationTurnStatus::Succeeded => {
                if self.selected_candidate_id.is_none()
                    || self
                        .attempts
                        .iter()
                        .any(|attempt| attempt.usage_event_id.is_none())
                {
                    return Err(ValidationError::InvalidReference {
                        field: "generation_turn.success_refs",
                    });
                }
            }
            GenerationTurnStatus::Failed
            | GenerationTurnStatus::Cancelled
            | GenerationTurnStatus::Interrupted => {
                if self
                    .attempts
                    .iter()
                    .any(|attempt| attempt.usage_event_id.is_none())
                {
                    return Err(ValidationError::InvalidReference {
                        field: "generation_turn.terminal_usage",
                    });
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Validates a newly-created retry against its terminal source turn.
    /// Retrying never reopens or appends to the source turn.
    pub fn validate_retry_against(
        &self,
        source: &GenerationTurn,
        is_group: bool,
    ) -> Result<(), ValidationError> {
        self.validate(is_group)?;
        source.validate(is_group)?;
        if self.id == source.id
            || self.retry_of_turn_id != Some(source.id)
            || self.conversation_id != source.conversation_id
            || self.branch_id != source.branch_id
            || self.operation != source.operation
            || self.input != source.input
            || self.target != source.target
            || self.guidance != source.guidance
            || self.requested_model_override != source.requested_model_override
            || self.forced_speaker != source.forced_speaker
            || self.swap_roles != source.swap_roles
            || !matches!(
                source.status,
                GenerationTurnStatus::Failed | GenerationTurnStatus::Cancelled
            )
        {
            return Err(ValidationError::InvalidReference {
                field: "generation_turn.retry_source",
            });
        }
        Ok(())
    }

    pub fn validate_speakers_against_participants(
        &self,
        participants: &[crate::model::ConversationParticipant],
    ) -> Result<(), ValidationError> {
        for participant_id in self.forced_speaker.into_iter().chain(
            self.selected_speaker
                .iter()
                .map(|speaker| speaker.participant_id),
        ) {
            if !participants.iter().any(|participant| {
                participant.id == participant_id
                    && participant.role == crate::model::ParticipantRole::Character
            }) {
                return Err(ValidationError::InvalidReference {
                    field: "generation_turn.speaker_character",
                });
            }
        }
        Ok(())
    }

    /// Group regeneration must retain the target author unless an explicit
    /// forced speaker is supplied; repository code can call this before
    /// materializing the selected-speaker decision.
    pub fn validate_regeneration_speaker(
        &self,
        target_author: Option<ConversationParticipantId>,
        selected_speaker: Option<ConversationParticipantId>,
    ) -> Result<(), ValidationError> {
        let expected = self.forced_speaker.or(target_author);
        if selected_speaker != expected {
            return Err(ValidationError::InvalidReference {
                field: "generation_turn.regenerate_speaker",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenerationTransition {
    pub turn_id: GenerationTurnId,
    pub from: GenerationTurnStatus,
    pub to: GenerationTurnStatus,
    pub revision: Revision,
    pub at: TimestampMillis,
}

/// Safe events that may be persisted in the repository/job event stream.
/// They carry stages, counters, and typed references only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum GenerationCheckpointEvent {
    Stage { status: GenerationTurnStatus },
    Progress { emitted_parts: u32 },
    CandidateReady { candidate_id: MessageCandidateId },
    UsageRecorded { usage_event_id: UsageEventId },
    Completed,
    Failed { code: GenerationFailureCode },
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationCheckpointEnvelope {
    pub turn_id: GenerationTurnId,
    pub attempt_id: GenerationAttemptId,
    pub job_id: Option<JobId>,
    pub correlation_id: Option<RequestId>,
    pub sequence: u64,
    pub event: GenerationCheckpointEvent,
}

impl GenerationCheckpointEnvelope {
    pub fn validate_after(&self, previous: Option<&Self>) -> Result<(), ValidationError> {
        if self.sequence == 0 {
            return Err(ValidationError::OutOfBounds {
                field: "generation_checkpoint.sequence",
            });
        }
        self.event.validate()?;
        if let Some(previous) = previous {
            if self.turn_id != previous.turn_id
                || self.attempt_id != previous.attempt_id
                || self.sequence != previous.sequence.saturating_add(1)
            {
                return Err(ValidationError::Invariant {
                    field: "generation_checkpoint.sequence",
                });
            }
        } else if self.sequence != 1 {
            return Err(ValidationError::Invariant {
                field: "generation_checkpoint.first_sequence",
            });
        }
        Ok(())
    }
}

impl GenerationCheckpointEvent {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if let Self::Progress { emitted_parts } = self {
            if *emitted_parts > u32::try_from(MAX_PARTS).unwrap_or(u32::MAX) {
                return Err(ValidationError::OutOfBounds {
                    field: "generation_checkpoint.emitted_parts",
                });
            }
        }
        Ok(())
    }
}

/// UI streaming is explicitly separate from durable checkpoints. This type
/// is not serializable and must never be passed to a repository or JobPort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationStreamEventEnvelope {
    pub operation: GenerationOperation,
    pub turn_id: GenerationTurnId,
    pub attempt_id: GenerationAttemptId,
    pub sequence: u64,
    pub event: GenerationStreamEvent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenerationStreamEvent {
    TextDelta { text: String },
    ReasoningDelta { text: String },
}

impl GenerationStreamEventEnvelope {
    pub fn validate_after(&self, previous: Option<&Self>) -> Result<(), ValidationError> {
        if let Some(previous) = previous {
            if self.operation != previous.operation
                || self.turn_id != previous.turn_id
                || self.attempt_id != previous.attempt_id
                || self.sequence != previous.sequence.saturating_add(1)
            {
                return Err(ValidationError::Invariant {
                    field: "generation_stream.sequence",
                });
            }
        }
        self.validate()
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.sequence == 0 {
            return Err(ValidationError::OutOfBounds {
                field: "generation_stream.sequence",
            });
        }
        match &self.event {
            GenerationStreamEvent::TextDelta { text } => {
                validate_text("generation_stream.text", text, MAX_REASONING_BYTES, true)
            }
            GenerationStreamEvent::ReasoningDelta { text } => validate_text(
                "generation_stream.reasoning",
                text,
                MAX_REASONING_BYTES,
                true,
            ),
        }
    }
}
