use std::collections::HashSet;

use lettuce_conversations::{
    InferenceUsage, MessagePart, MessageRenderSource, ProposedToolCall, ReplayArtifactRef,
    ReplayRetention, ResolvedInferenceProfile, ToolPolicy, ToolRequest,
};
use lettuce_types::{
    ConversationId, DynamicMemoryAttemptId, DynamicMemoryRunId, JobId, MemorySpaceId, MessageId,
    Revision, TimestampMillis, ToolExecutionId,
};
use serde::{Deserialize, Serialize};

use crate::{dynamic_memory_tool_request, dynamic_memory_tool_request_for_run};

pub const MAX_DYNAMIC_MEMORY_SOURCE_MESSAGES: usize = 1024;
pub const MAX_DYNAMIC_MEMORY_INFERENCE_ROUNDS: u8 = 8;

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
        if self.message_interval == 0
            || self.end <= self.start
            || self.end - self.start != u64::try_from(source_message_count).unwrap_or(u64::MAX)
        {
            return Err(DynamicMemoryRunError::InvalidRun);
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
        if self.source_messages.is_empty()
            || self.source_messages.len() > MAX_DYNAMIC_MEMORY_SOURCE_MESSAGES
            || self
                .source_messages
                .iter()
                .map(|source| source.message_id)
                .collect::<HashSet<_>>()
                .len()
                != self.source_messages.len()
            || self.source_messages.iter().any(|source| {
                !matches!(
                    source.role,
                    lettuce_conversations::MessageRole::User
                        | lettuce_conversations::MessageRole::Assistant
                )
            })
            || self.profile.tool_policy != ToolPolicy::Required
            || self.tool_request
                != dynamic_memory_tool_request_for_run(
                    self.supersession_enabled,
                    self.time_awareness_enabled,
                )
            || self.starting_memory.id != self.space_id
            || self.starting_memory.validate().is_err()
        {
            return Err(DynamicMemoryRunError::InvalidRun);
        }
        self.tool_request
            .validate()
            .map_err(|_| DynamicMemoryRunError::InvalidRun)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewDynamicMemoryRunAttempt {
    pub run_id: DynamicMemoryRunId,
    pub attempt_id: DynamicMemoryAttemptId,
    pub conversation_id: ConversationId,
    pub space_id: MemorySpaceId,
    pub starting_memory: crate::MemorySpaceSnapshot,
    pub source_messages: Vec<DynamicMemorySourceMessage>,
    pub profile: ResolvedInferenceProfile,
    pub time_awareness_enabled: bool,
    pub supersession_enabled: bool,
    pub structured_fallback_format: DynamicMemoryStructuredFallbackFormat,
    pub summary_window: DynamicMemorySummaryWindow,
    pub job_id: JobId,
    pub now: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewDynamicMemoryAttemptRecovery {
    pub run_id: DynamicMemoryRunId,
    pub parent_attempt_id: DynamicMemoryAttemptId,
    pub child_attempt_id: DynamicMemoryAttemptId,
    pub job_id: JobId,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicMemoryAttempt {
    pub id: DynamicMemoryAttemptId,
    pub run_id: DynamicMemoryRunId,
    pub ordinal: u16,
    pub retry_parent_id: Option<DynamicMemoryAttemptId>,
    pub job_id: JobId,
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
        if (self.ordinal == 0) != self.retry_parent_id.is_none()
            || self.revision.get() == 0
            || self.updated_at < self.created_at
        {
            return Err(DynamicMemoryRunError::InvalidAttempt);
        }
        match self.status {
            DynamicMemoryAttemptStatus::Created
                if self.started_at.is_some() || self.finished_at.is_some() =>
            {
                return Err(DynamicMemoryRunError::InvalidAttempt);
            }
            DynamicMemoryAttemptStatus::Processing
                if self.started_at.is_none() || self.finished_at.is_some() =>
            {
                return Err(DynamicMemoryRunError::InvalidAttempt);
            }
            DynamicMemoryAttemptStatus::Succeeded
            | DynamicMemoryAttemptStatus::Failed
            | DynamicMemoryAttemptStatus::Interrupted
                if self.started_at.is_none() || self.finished_at.is_none() =>
            {
                return Err(DynamicMemoryRunError::InvalidAttempt);
            }
            DynamicMemoryAttemptStatus::Cancelled if self.finished_at.is_none() => {
                return Err(DynamicMemoryRunError::InvalidAttempt);
            }
            _ => {}
        }
        if self.failure.is_some() != matches!(self.status, DynamicMemoryAttemptStatus::Failed)
            || self
                .started_at
                .is_some_and(|started| started < self.created_at || started > self.updated_at)
            || self
                .finished_at
                .is_some_and(|finished| finished < self.created_at || finished > self.updated_at)
            || matches!((self.started_at, self.finished_at), (Some(started), Some(finished)) if started > finished)
        {
            return Err(DynamicMemoryRunError::InvalidAttempt);
        }
        Ok(())
    }

    pub fn transition(
        &self,
        next: DynamicMemoryAttemptStatus,
        failure: Option<DynamicMemoryAttemptFailureCode>,
        at: TimestampMillis,
    ) -> Result<Self, DynamicMemoryRunError> {
        if !self.status.can_transition_to(next)
            || at < self.updated_at
            || failure.is_some() != matches!(next, DynamicMemoryAttemptStatus::Failed)
        {
            return Err(DynamicMemoryRunError::InvalidTransition);
        }
        let mut updated = self.clone();
        updated.status = next;
        updated.failure = failure;
        updated.revision = self
            .revision
            .next()
            .map_err(|_| DynamicMemoryRunError::InvalidAttempt)?;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicMemoryRoundFinishReason {
    Stop,
    Length,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewDynamicMemoryInferenceRound {
    pub ordinal: u8,
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
        self.request_context
            .validate()
            .map_err(|_| DynamicMemoryRunError::InvalidRound)?;
        if self.ordinal >= MAX_DYNAMIC_MEMORY_INFERENCE_ROUNDS
            || (self.parts.is_empty() && self.calls.is_empty())
            || self.parts.len() > 64
            || self.calls.len() > lettuce_conversations::MAX_TOOL_CALLS_PER_RESPONSE
            || self
                .provider_request_id
                .as_ref()
                .is_some_and(|id| id.trim().is_empty() || id.len() > 256)
        {
            return Err(DynamicMemoryRunError::InvalidRound);
        }
        for part in &self.parts {
            part.validate()
                .map_err(|_| DynamicMemoryRunError::InvalidRound)?;
            if !matches!(
                part,
                MessagePart::Text { .. } | MessagePart::ReasoningSummary { .. }
            ) {
                return Err(DynamicMemoryRunError::InvalidRound);
            }
        }
        if let Some(replay) = &self.provider_replay {
            replay
                .validate()
                .map_err(|_| DynamicMemoryRunError::InvalidRound)?;
            if replay.retention != ReplayRetention::Conversation {
                return Err(DynamicMemoryRunError::InvalidRound);
            }
        }
        let mut ids = HashSet::new();
        let mut provider_ids = HashSet::new();
        let request = dynamic_memory_tool_request();
        for call in &self.calls {
            call.call
                .validate()
                .map_err(|_| DynamicMemoryRunError::InvalidCall)?;
            if call.definition_version == 0
                || !ids.insert(call.id)
                || call
                    .call
                    .provider_call_id
                    .as_deref()
                    .is_some_and(|id| !provider_ids.insert(id))
                || call.call.provider_replay.as_ref() != self.provider_replay.as_ref()
                || !request.definitions.iter().any(|definition| {
                    definition.name == call.call.name
                        && definition.version == call.definition_version
                })
            {
                return Err(DynamicMemoryRunError::InvalidCall);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicMemoryInferenceRound {
    pub run_id: DynamicMemoryRunId,
    pub attempt_id: DynamicMemoryAttemptId,
    pub ordinal: u8,
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
            .is_none_or(|count| count > lettuce_conversations::MAX_TOOL_CALLS_PER_RESPONSE)
        {
            return Err(DynamicMemoryRunError::InvalidRound);
        }
        for (offset, call) in self.calls.iter().enumerate() {
            let expected = self
                .first_call_ordinal
                .checked_add(
                    u16::try_from(offset).map_err(|_| DynamicMemoryRunError::InvalidRound)?,
                )
                .ok_or(DynamicMemoryRunError::InvalidRound)?;
            if call.run_id != self.run_id
                || call.attempt_id != self.attempt_id
                || call.round_ordinal != self.ordinal
                || call.ordinal != expected
                || call.admitted_at != self.admitted_at
            {
                return Err(DynamicMemoryRunError::InvalidRound);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
        self.call
            .validate()
            .map_err(|_| DynamicMemoryRunError::InvalidCall)?;
        if self.definition_version == 0 {
            return Err(DynamicMemoryRunError::InvalidCall);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DynamicMemoryRunError {
    #[error("dynamic-memory run is invalid")]
    InvalidRun,
    #[error("dynamic-memory attempt is invalid")]
    InvalidAttempt,
    #[error("dynamic-memory attempt transition is invalid")]
    InvalidTransition,
    #[error("dynamic-memory inference round is invalid")]
    InvalidRound,
    #[error("dynamic-memory tool call is invalid")]
    InvalidCall,
}
