use std::collections::HashSet;

use lettuce_conversations::{
    InferenceUsage, MessagePart, MessageRenderSource, ProposedToolCall, ReplayArtifactRef,
    ReplayRetention, ResolvedInferenceProfile, ToolPolicy, ToolRequest,
};
use lettuce_types::{
    ConversationBranchId, ConversationId, DynamicMemoryAttemptId, DynamicMemoryRunId, JobId,
    MemorySpaceId, MessageId, Revision, TimestampMillis, ToolExecutionId,
};
use serde::{Deserialize, Serialize};

use crate::{
    DynamicMemoryToolOptions, dynamic_memory_tool_request_for_run, dynamic_memory_tool_shape,
};

/// The largest recursive memory loop hard cap; the user's cap applies below it.
pub const MAX_DYNAMIC_MEMORY_INFERENCE_ROUNDS: u8 = 100;
pub const MAX_DYNAMIC_MEMORY_ATTEMPT_TOOL_CALLS: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DynamicMemoryStructuredFallbackFormat {
    Json,
    Xml,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicMemorySummaryWindow {
    pub message_interval: u32,
    pub start: u64,
    pub end: u64,
}

impl DynamicMemorySummaryWindow {
    fn validate(self, source_message_count: usize) -> Result<(), DynamicMemoryRunError> {
        if self.message_interval == 0 {
            return Err(DynamicMemoryRunError::InvalidRun {
                reason: "summary window message interval is zero",
            });
        }
        if self.end <= self.start {
            return Err(DynamicMemoryRunError::InvalidRun {
                reason: "summary window end is not after its start",
            });
        }
        if self.end - self.start != u64::try_from(source_message_count).unwrap_or(u64::MAX) {
            return Err(DynamicMemoryRunError::InvalidRun {
                reason: "summary window length differs from the source message count",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicMemoryAttemptStatus {
    Created,
    Processing,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}

impl DynamicMemoryAttemptStatus {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::Interrupted
        )
    }

    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Created, Self::Processing | Self::Cancelled)
                | (
                    Self::Processing,
                    Self::Succeeded | Self::Failed | Self::Cancelled | Self::Interrupted
                )
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicMemoryAttemptFailureCode {
    ProviderUnavailable,
    ProviderRejected,
    EmptyResponse,
    TimedOut,
    RoundLimit,
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicMemorySourceMessage {
    pub message_id: MessageId,
    pub role: lettuce_conversations::MessageRole,
    pub render_source: MessageRenderSource,
    pub effective_time: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicMemoryRun {
    pub id: DynamicMemoryRunId,
    pub conversation_id: ConversationId,
    pub branch_id: ConversationBranchId,
    pub space_id: MemorySpaceId,
    pub starting_memory: crate::MemorySpaceSnapshot,
    pub source_messages: Vec<DynamicMemorySourceMessage>,
    pub profile: ResolvedInferenceProfile,
    pub time_awareness_enabled: bool,
    pub supersession_enabled: bool,
    pub structured_fallback_format: DynamicMemoryStructuredFallbackFormat,
    pub summary_window: DynamicMemorySummaryWindow,
    pub tool_request: ToolRequest,
    pub created_at: TimestampMillis,
}

impl DynamicMemoryRun {
    pub fn validate(&self) -> Result<(), DynamicMemoryRunError> {
        self.summary_window.validate(self.source_messages.len())?;
        if self.source_messages.is_empty() {
            return Err(DynamicMemoryRunError::InvalidRun {
                reason: "run has no source messages",
            });
        }
        if self
            .source_messages
            .iter()
            .map(|source| source.message_id)
            .collect::<HashSet<_>>()
            .len()
            != self.source_messages.len()
        {
            return Err(DynamicMemoryRunError::InvalidRun {
                reason: "run lists a source message twice",
            });
        }
        if self.source_messages.iter().any(|source| {
            !matches!(
                source.role,
                lettuce_conversations::MessageRole::User
                    | lettuce_conversations::MessageRole::Assistant
            )
        }) {
            return Err(DynamicMemoryRunError::InvalidRun {
                reason: "a source message is neither user nor assistant",
            });
        }
        if self.profile.tool_policy != ToolPolicy::Required {
            return Err(DynamicMemoryRunError::InvalidRun {
                reason: "inference profile does not require tool calls",
            });
        }
        if ![false, true].into_iter().any(|group| {
            dynamic_memory_tool_shape(&self.tool_request)
                == dynamic_memory_tool_shape(&dynamic_memory_tool_request_for_run(
                    DynamicMemoryToolOptions {
                        group,
                        supersession_enabled: self.supersession_enabled,
                        require_source_message_id: self.time_awareness_enabled,
                    },
                    &|key| key.to_owned(),
                ))
        }) {
            return Err(DynamicMemoryRunError::InvalidRun {
                reason: "tool request does not match the dynamic-memory tool contract",
            });
        }
        if self.starting_memory.id != self.space_id {
            return Err(DynamicMemoryRunError::InvalidRun {
                reason: "starting memory belongs to another memory space",
            });
        }
        if let Err(error) = self.starting_memory.validate() {
            tracing::warn!(%error, "dynamic-memory run starting memory failed validation");
            return Err(DynamicMemoryRunError::InvalidRun {
                reason: "starting memory snapshot failed validation",
            });
        }
        self.tool_request.validate().map_err(|error| {
            tracing::warn!(%error, "dynamic-memory run tool request failed validation");
            DynamicMemoryRunError::InvalidRun {
                reason: "tool request failed validation",
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewDynamicMemoryRunAttempt {
    pub run_id: DynamicMemoryRunId,
    pub attempt_id: DynamicMemoryAttemptId,
    pub conversation_id: ConversationId,
    pub branch_id: ConversationBranchId,
    pub space_id: MemorySpaceId,
    /// The memory space as the cycle sees it, after `cycle_start_change`.
    pub starting_memory: crate::MemorySpaceSnapshot,
    /// The cycle-start pass (pinned items restored, hot items decayed),
    /// committed in the same transaction as the run so a replay never decays
    /// twice.
    pub cycle_start_change: Option<crate::MemoryChangeSet>,
    pub source_messages: Vec<DynamicMemorySourceMessage>,
    pub profile: ResolvedInferenceProfile,
    pub time_awareness_enabled: bool,
    pub supersession_enabled: bool,
    pub structured_fallback_format: DynamicMemoryStructuredFallbackFormat,
    pub summary_window: DynamicMemorySummaryWindow,
    /// The memory tool contract with its catalog texts, frozen with the run.
    pub tool_request: lettuce_conversations::ToolRequest,
    pub job_id: JobId,
    pub job_attempt: Option<u32>,
    pub now: TimestampMillis,
}

impl NewDynamicMemoryRunAttempt {
    pub fn validate_cycle_start(&self) -> Result<(), DynamicMemoryRunError> {
        let Some(change) = &self.cycle_start_change else {
            return Ok(());
        };
        change.validate().map_err(|error| {
            tracing::warn!(%error, "dynamic-memory cycle start change failed validation");
            DynamicMemoryRunError::InvalidRun {
                reason: "cycle start change failed validation",
            }
        })?;
        if change.space_id != self.space_id {
            return Err(DynamicMemoryRunError::InvalidRun {
                reason: "cycle start change belongs to another memory space",
            });
        }
        if change
            .expected_revision
            .next()
            .ok()
            .is_none_or(|next| next != self.starting_memory.revision)
        {
            return Err(DynamicMemoryRunError::InvalidRun {
                reason: "cycle start change does not lead to the starting revision",
            });
        }
        if change.items != self.starting_memory.items {
            return Err(DynamicMemoryRunError::InvalidRun {
                reason: "cycle start change items differ from the starting memory",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewDynamicMemoryAttemptRecovery {
    pub run_id: DynamicMemoryRunId,
    pub parent_attempt_id: DynamicMemoryAttemptId,
    pub child_attempt_id: DynamicMemoryAttemptId,
    pub job_id: JobId,
    pub job_attempt: Option<u32>,
    pub now: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DynamicMemoryRunAttemptAdmission {
    pub run: DynamicMemoryRun,
    pub attempt: DynamicMemoryAttempt,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DynamicMemoryAttemptRecovery {
    pub run: DynamicMemoryRun,
    pub parent: DynamicMemoryAttempt,
    pub child: DynamicMemoryAttempt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicMemoryAttempt {
    pub id: DynamicMemoryAttemptId,
    pub run_id: DynamicMemoryRunId,
    pub ordinal: u16,
    pub retry_parent_id: Option<DynamicMemoryAttemptId>,
    pub job_id: JobId,
    pub job_attempt: Option<u32>,
    pub status: DynamicMemoryAttemptStatus,
    pub failure: Option<DynamicMemoryAttemptFailureCode>,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub started_at: Option<TimestampMillis>,
    pub finished_at: Option<TimestampMillis>,
    pub updated_at: TimestampMillis,
}

impl DynamicMemoryAttempt {
    pub fn validate(&self) -> Result<(), DynamicMemoryRunError> {
        if self.job_attempt == Some(0) {
            return Err(DynamicMemoryRunError::InvalidAttempt {
                reason: "job attempt is zero",
            });
        }
        if (self.ordinal == 0) != self.retry_parent_id.is_none() {
            return Err(DynamicMemoryRunError::InvalidAttempt {
                reason: "retry parent does not match the attempt ordinal",
            });
        }
        if self.revision.get() == 0 {
            return Err(DynamicMemoryRunError::InvalidAttempt {
                reason: "attempt revision is zero",
            });
        }
        if self.updated_at < self.created_at {
            return Err(DynamicMemoryRunError::InvalidAttempt {
                reason: "attempt was updated before it was created",
            });
        }
        match self.status {
            DynamicMemoryAttemptStatus::Created
                if self.started_at.is_some() || self.finished_at.is_some() =>
            {
                return Err(DynamicMemoryRunError::InvalidAttempt {
                    reason: "created attempt has a start or finish time",
                });
            }
            DynamicMemoryAttemptStatus::Processing
                if self.started_at.is_none() || self.finished_at.is_some() =>
            {
                return Err(DynamicMemoryRunError::InvalidAttempt {
                    reason: "processing attempt lacks a start time or has a finish time",
                });
            }
            DynamicMemoryAttemptStatus::Succeeded
            | DynamicMemoryAttemptStatus::Failed
            | DynamicMemoryAttemptStatus::Interrupted
                if self.started_at.is_none() || self.finished_at.is_none() =>
            {
                return Err(DynamicMemoryRunError::InvalidAttempt {
                    reason: "finished attempt lacks a start or finish time",
                });
            }
            DynamicMemoryAttemptStatus::Cancelled if self.finished_at.is_none() => {
                return Err(DynamicMemoryRunError::InvalidAttempt {
                    reason: "cancelled attempt has no finish time",
                });
            }
            _ => {}
        }
        if self.failure.is_some() != matches!(self.status, DynamicMemoryAttemptStatus::Failed) {
            return Err(DynamicMemoryRunError::InvalidAttempt {
                reason: "failure code is set only on failed attempts",
            });
        }
        if self
            .started_at
            .is_some_and(|started| started < self.created_at || started > self.updated_at)
        {
            return Err(DynamicMemoryRunError::InvalidAttempt {
                reason: "start time is outside the creation and update times",
            });
        }
        if self
            .finished_at
            .is_some_and(|finished| finished < self.created_at || finished > self.updated_at)
        {
            return Err(DynamicMemoryRunError::InvalidAttempt {
                reason: "finish time is outside the creation and update times",
            });
        }
        if matches!((self.started_at, self.finished_at), (Some(started), Some(finished)) if started > finished)
        {
            return Err(DynamicMemoryRunError::InvalidAttempt {
                reason: "attempt finished before it started",
            });
        }
        Ok(())
    }

    pub fn transition(
        &self,
        next: DynamicMemoryAttemptStatus,
        failure: Option<DynamicMemoryAttemptFailureCode>,
        at: TimestampMillis,
    ) -> Result<Self, DynamicMemoryRunError> {
        if !self.status.can_transition_to(next) {
            return Err(DynamicMemoryRunError::InvalidTransition {
                reason: "status cannot move to the requested status",
            });
        }
        if at < self.updated_at {
            return Err(DynamicMemoryRunError::InvalidTransition {
                reason: "transition time is before the last update",
            });
        }
        if failure.is_some() != matches!(next, DynamicMemoryAttemptStatus::Failed) {
            return Err(DynamicMemoryRunError::InvalidTransition {
                reason: "failure code is set only when moving to failed",
            });
        }
        let mut updated = self.clone();
        updated.status = next;
        updated.failure = failure;
        updated.revision = self.revision.next().map_err(|error| {
            tracing::warn!(%error, "dynamic-memory attempt revision overflowed");
            DynamicMemoryRunError::InvalidAttempt {
                reason: "attempt revision cannot advance",
            }
        })?;
        updated.updated_at = at;
        if next == DynamicMemoryAttemptStatus::Processing {
            updated.started_at = Some(at);
        }
        if next.is_terminal() {
            updated.finished_at = Some(at);
        }
        updated.validate()?;
        Ok(updated)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewDynamicMemoryToolCall {
    pub id: ToolExecutionId,
    pub definition_version: u32,
    pub call: ProposedToolCall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicMemoryRoundFinishReason {
    Stop,
    Length,
}

/// Which request a round answers: the recursive manager loop, or the single
/// category repair pass that runs after the loop and is never resumed from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicMemoryRoundKind {
    Manager,
    Repair,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewDynamicMemoryInferenceRound {
    pub ordinal: u8,
    pub kind: DynamicMemoryRoundKind,
    pub request_context: lettuce_conversations::ProviderNeutralContext,
    pub parts: Vec<MessagePart>,
    pub provider_replay: Option<ReplayArtifactRef>,
    pub usage: Option<InferenceUsage>,
    pub finish_reason: DynamicMemoryRoundFinishReason,
    pub provider_request_id: Option<String>,
    pub calls: Vec<NewDynamicMemoryToolCall>,
    pub admitted_at: TimestampMillis,
}

impl NewDynamicMemoryInferenceRound {
    pub fn validate(&self) -> Result<(), DynamicMemoryRunError> {
        self.request_context.validate().map_err(|error| {
            tracing::warn!(%error, "dynamic-memory round request context failed validation");
            DynamicMemoryRunError::InvalidRound {
                reason: "request context failed validation",
            }
        })?;
        if self.ordinal >= MAX_DYNAMIC_MEMORY_INFERENCE_ROUNDS {
            return Err(DynamicMemoryRunError::InvalidRound {
                reason: "round ordinal is at or above the round limit",
            });
        }
        if self.parts.is_empty() && self.calls.is_empty() {
            return Err(DynamicMemoryRunError::InvalidRound {
                reason: "round has neither parts nor tool calls",
            });
        }
        if self
            .provider_request_id
            .as_ref()
            .is_some_and(|id| id.trim().is_empty() || id.len() > 256)
        {
            return Err(DynamicMemoryRunError::InvalidRound {
                reason: "provider request id is empty or too long",
            });
        }
        for part in &self.parts {
            part.validate().map_err(|error| {
                tracing::warn!(%error, "dynamic-memory round part failed validation");
                DynamicMemoryRunError::InvalidRound {
                    reason: "a round part failed validation",
                }
            })?;
            if !matches!(
                part,
                MessagePart::Text { .. } | MessagePart::ReasoningSummary { .. }
            ) {
                return Err(DynamicMemoryRunError::InvalidRound {
                    reason: "a round part is neither text nor a reasoning summary",
                });
            }
        }
        if let Some(replay) = &self.provider_replay {
            replay.validate().map_err(|error| {
                tracing::warn!(%error, "dynamic-memory round provider replay failed validation");
                DynamicMemoryRunError::InvalidRound {
                    reason: "provider replay failed validation",
                }
            })?;
            if replay.retention != ReplayRetention::Conversation {
                return Err(DynamicMemoryRunError::InvalidRound {
                    reason: "provider replay is not retained with the conversation",
                });
            }
        }
        let mut ids = HashSet::new();
        let mut provider_ids = HashSet::new();
        for call in &self.calls {
            call.call.validate().map_err(|error| {
                tracing::warn!(%error, "dynamic-memory round tool call failed validation");
                DynamicMemoryRunError::InvalidCall {
                    reason: "tool call failed validation",
                }
            })?;
            if call.definition_version == 0 {
                return Err(DynamicMemoryRunError::InvalidCall {
                    reason: "tool definition version is zero",
                });
            }
            if !ids.insert(call.id) {
                return Err(DynamicMemoryRunError::InvalidCall {
                    reason: "tool execution id is repeated in the round",
                });
            }
            if call
                .call
                .provider_call_id
                .as_deref()
                .is_some_and(|id| !provider_ids.insert(id))
            {
                return Err(DynamicMemoryRunError::InvalidCall {
                    reason: "provider call id is repeated in the round",
                });
            }
            if call.call.provider_replay.as_ref() != self.provider_replay.as_ref() {
                return Err(DynamicMemoryRunError::InvalidCall {
                    reason: "tool call replay differs from the round replay",
                });
            }
            if call.definition_version != 1 {
                return Err(DynamicMemoryRunError::InvalidCall {
                    reason: "tool definition version is not supported",
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicMemoryInferenceRound {
    pub run_id: DynamicMemoryRunId,
    pub attempt_id: DynamicMemoryAttemptId,
    pub ordinal: u8,
    pub kind: DynamicMemoryRoundKind,
    pub first_call_ordinal: u16,
    pub request_context: lettuce_conversations::ProviderNeutralContext,
    pub parts: Vec<MessagePart>,
    pub provider_replay: Option<ReplayArtifactRef>,
    pub usage: Option<InferenceUsage>,
    pub finish_reason: DynamicMemoryRoundFinishReason,
    pub provider_request_id: Option<String>,
    pub calls: Vec<DynamicMemoryToolCallEvidence>,
    pub admitted_at: TimestampMillis,
}

impl DynamicMemoryInferenceRound {
    pub fn validate(&self) -> Result<(), DynamicMemoryRunError> {
        NewDynamicMemoryInferenceRound {
            ordinal: self.ordinal,
            kind: self.kind,
            request_context: self.request_context.clone(),
            parts: self.parts.clone(),
            provider_replay: self.provider_replay.clone(),
            usage: self.usage.clone(),
            finish_reason: self.finish_reason,
            provider_request_id: self.provider_request_id.clone(),
            calls: self
                .calls
                .iter()
                .map(|call| NewDynamicMemoryToolCall {
                    id: call.id,
                    definition_version: call.definition_version,
                    call: call.call.clone(),
                })
                .collect(),
            admitted_at: self.admitted_at,
        }
        .validate()?;
        if usize::from(self.first_call_ordinal)
            .checked_add(self.calls.len())
            .is_none_or(|count| count > MAX_DYNAMIC_MEMORY_ATTEMPT_TOOL_CALLS)
        {
            return Err(DynamicMemoryRunError::InvalidRound {
                reason: "round tool calls exceed the attempt tool call limit",
            });
        }
        for (offset, call) in self.calls.iter().enumerate() {
            let expected = self
                .first_call_ordinal
                .checked_add(u16::try_from(offset).map_err(|error| {
                    tracing::warn!(%error, "dynamic-memory tool call offset exceeds u16");
                    DynamicMemoryRunError::InvalidRound {
                        reason: "tool call offset does not fit the ordinal range",
                    }
                })?)
                .ok_or(DynamicMemoryRunError::InvalidRound {
                    reason: "tool call ordinal overflows",
                })?;
            if call.run_id != self.run_id {
                return Err(DynamicMemoryRunError::InvalidRound {
                    reason: "tool call belongs to another run",
                });
            }
            if call.attempt_id != self.attempt_id {
                return Err(DynamicMemoryRunError::InvalidRound {
                    reason: "tool call belongs to another attempt",
                });
            }
            if call.round_ordinal != self.ordinal {
                return Err(DynamicMemoryRunError::InvalidRound {
                    reason: "tool call belongs to another round",
                });
            }
            if call.ordinal != expected {
                return Err(DynamicMemoryRunError::InvalidRound {
                    reason: "tool call ordinal is not sequential",
                });
            }
            if call.admitted_at != self.admitted_at {
                return Err(DynamicMemoryRunError::InvalidRound {
                    reason: "tool call admission time differs from the round",
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicMemoryToolCallEvidence {
    pub id: ToolExecutionId,
    pub run_id: DynamicMemoryRunId,
    pub attempt_id: DynamicMemoryAttemptId,
    pub round_ordinal: u8,
    pub ordinal: u16,
    pub definition_version: u32,
    pub call: ProposedToolCall,
    pub admitted_at: TimestampMillis,
}

impl DynamicMemoryToolCallEvidence {
    pub fn validate(&self) -> Result<(), DynamicMemoryRunError> {
        self.call.validate().map_err(|error| {
            tracing::warn!(%error, "dynamic-memory tool call evidence failed validation");
            DynamicMemoryRunError::InvalidCall {
                reason: "tool call failed validation",
            }
        })?;
        if self.definition_version == 0 {
            return Err(DynamicMemoryRunError::InvalidCall {
                reason: "tool definition version is zero",
            });
        }
        Ok(())
    }
}

impl DynamicMemoryRunError {
    #[must_use]
    pub const fn reason(&self) -> &'static str {
        match self {
            Self::InvalidRun { reason }
            | Self::InvalidAttempt { reason }
            | Self::InvalidTransition { reason }
            | Self::InvalidRound { reason }
            | Self::InvalidCall { reason } => reason,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DynamicMemoryRunError {
    #[error("dynamic-memory run is invalid: {reason}")]
    InvalidRun { reason: &'static str },
    #[error("dynamic-memory attempt is invalid: {reason}")]
    InvalidAttempt { reason: &'static str },
    #[error("dynamic-memory attempt transition is invalid: {reason}")]
    InvalidTransition { reason: &'static str },
    #[error("dynamic-memory inference round is invalid: {reason}")]
    InvalidRound { reason: &'static str },
    #[error("dynamic-memory tool call is invalid: {reason}")]
    InvalidCall { reason: &'static str },
}
