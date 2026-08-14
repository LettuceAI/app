use lettuce_types::{
    ConversationBranchId, ConversationId, ConversationParticipantId, GenerationAttemptId,
    GenerationTurnId, JobId, MemoryRevisionId, MessageCandidateId, MessageId, PromptDocumentId,
    RequestId, Revision, TimestampMillis, UsageEventId,
};
use serde::{Deserialize, Serialize};

use crate::ValidationError;
use crate::snapshot::{ModelSelectionSnapshot, ValidateSnapshot};
use crate::validation::{
    MAX_ATTEMPTS, MAX_PARTS, MAX_REASONING_BYTES, validate_collection,
    validate_revision_timestamps, validate_text, validate_unique,
};

pub use lettuce_jobs::IdempotencyKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationOperation {
    Send,
    Continue,
    Regenerate,
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LorebookAttribution {
    pub lorebook_id: lettuce_types::LorebookId,
    pub revision: Revision,
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
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}

impl GenerationAttempt {
    pub fn validate(&self) -> Result<(), ValidationError> {
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
            GenerationAttemptStatus::Created
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
    pub idempotency_key: IdempotencyKey,
    pub job_id: Option<JobId>,
    pub correlation_id: Option<RequestId>,
    pub status: GenerationTurnStatus,
    pub selected_speaker: Option<SelectedSpeakerDecision>,
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
        validate_unique(
            "generation_turn.attempt_ids",
            self.attempts.iter().map(|attempt| attempt.id),
        )?;
        for (ordinal, attempt) in self.attempts.iter().enumerate() {
            if attempt.ordinal as usize != ordinal || attempt.turn_id != self.id {
                return Err(ValidationError::Invariant {
                    field: "generation_turn.attempt_ordinals",
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
        if !is_group && self.selected_speaker.is_some() {
            return Err(ValidationError::InvalidReference {
                field: "generation_turn.direct_speaker",
            });
        }
        if let Some(model) = &self.resolved_model {
            model.validate_snapshot("generation_turn.model")?;
        }
        if let Some(attribution) = &self.prompt {
            if attribution.revision.get() == 0 {
                return Err(ValidationError::ZeroRevision);
            }
        }
        if self.lorebooks.iter().any(|book| book.revision.get() == 0) {
            return Err(ValidationError::ZeroRevision);
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
