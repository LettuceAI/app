use lettuce_conversations::{
    InferenceUsage, MessagePart, ProposedToolCall, ReplayArtifactRef, ReplayRetention, ToolRequest,
};
use lettuce_types::{
    CreationProposalId, CreationTurnId, CreationWorkflowId, GenerationAttemptId, Revision,
    TimestampMillis, ToolExecutionId,
};
use serde::{Deserialize, Serialize};

use crate::{CreationStage, CreationTargetKind, creation_tool_request};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreationAttemptStatus {
    Created,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}

impl CreationAttemptStatus {
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
            (Self::Created, Self::Running | Self::Cancelled)
                | (
                    Self::Running,
                    Self::Succeeded | Self::Failed | Self::Cancelled | Self::Interrupted
                )
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreationAttemptFailureCode {
    ProviderUnavailable,
    ProviderRejected,
    EmptyResponse,
    TimedOut,
    RoundLimit,
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreationAttemptOwner {
    pub workflow_id: CreationWorkflowId,
    pub turn_id: CreationTurnId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCreationAttempt {
    pub id: GenerationAttemptId,
    pub owner: CreationAttemptOwner,
    pub base_proposal_id: CreationProposalId,
    pub planned_proposal_id: CreationProposalId,
    pub retry_parent_id: Option<GenerationAttemptId>,
    pub now: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreationInferenceAttempt {
    pub id: GenerationAttemptId,
    pub workflow_id: CreationWorkflowId,
    pub turn_id: CreationTurnId,
    pub ordinal: u16,
    pub retry_parent_id: Option<GenerationAttemptId>,
    pub base_proposal_id: CreationProposalId,
    pub planned_proposal_id: CreationProposalId,
    pub target: CreationTargetKind,
    pub stage: CreationStage,
    pub tool_request: ToolRequest,
    pub status: CreationAttemptStatus,
    pub failure: Option<CreationAttemptFailureCode>,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub started_at: Option<TimestampMillis>,
    pub finished_at: Option<TimestampMillis>,
    pub updated_at: TimestampMillis,
}

impl CreationInferenceAttempt {
    pub fn validate(&self) -> Result<(), CreationAttemptError> {
        self.tool_request
            .validate()
            .map_err(|_| CreationAttemptError::InvalidContract)?;
        if creation_tool_request(self.target, self.stage).as_ref() != Some(&self.tool_request) {
            return Err(CreationAttemptError::InvalidContract);
        }
        if (self.ordinal == 0) != self.retry_parent_id.is_none()
            || self.base_proposal_id == self.planned_proposal_id
            || self.revision.get() == 0
            || self.updated_at < self.created_at
        {
            return Err(CreationAttemptError::InvalidState);
        }
        match self.status {
            CreationAttemptStatus::Created
                if self.started_at.is_some() || self.finished_at.is_some() =>
            {
                return Err(CreationAttemptError::InvalidState);
            }
            CreationAttemptStatus::Running
                if self.started_at.is_none() || self.finished_at.is_some() =>
            {
                return Err(CreationAttemptError::InvalidState);
            }
            CreationAttemptStatus::Succeeded
            | CreationAttemptStatus::Failed
            | CreationAttemptStatus::Interrupted
                if self.started_at.is_none() || self.finished_at.is_none() =>
            {
                return Err(CreationAttemptError::InvalidState);
            }
            CreationAttemptStatus::Cancelled if self.finished_at.is_none() => {
                return Err(CreationAttemptError::InvalidState);
            }
            _ => {}
        }
        if self.failure.is_some() != matches!(self.status, CreationAttemptStatus::Failed)
            || self
                .started_at
                .is_some_and(|started| started < self.created_at || started > self.updated_at)
            || self
                .finished_at
                .is_some_and(|finished| finished < self.created_at || finished > self.updated_at)
            || matches!((self.started_at, self.finished_at), (Some(started), Some(finished)) if started > finished)
        {
            return Err(CreationAttemptError::InvalidState);
        }
        Ok(())
    }

    pub fn transition(
        &self,
        next: CreationAttemptStatus,
        failure: Option<CreationAttemptFailureCode>,
        at: TimestampMillis,
    ) -> Result<Self, CreationAttemptError> {
        if !self.status.can_transition_to(next) || at < self.updated_at {
            return Err(CreationAttemptError::InvalidTransition);
        }
        if failure.is_some() != matches!(next, CreationAttemptStatus::Failed) {
            return Err(CreationAttemptError::InvalidTransition);
        }
        let mut updated = self.clone();
        updated.status = next;
        updated.failure = failure;
        updated.revision = self
            .revision
            .next()
            .map_err(|_| CreationAttemptError::InvalidState)?;
        updated.updated_at = at;
        if next == CreationAttemptStatus::Running {
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
pub struct NewCreationToolCall {
    pub id: ToolExecutionId,
    pub definition_version: u32,
    pub call: ProposedToolCall,
}

pub const MAX_CREATION_INFERENCE_ROUNDS: u8 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreationRoundFinishReason {
    Stop,
    Length,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCreationInferenceRound {
    pub ordinal: u8,
    pub parts: Vec<MessagePart>,
    pub provider_replay: Option<ReplayArtifactRef>,
    pub usage: Option<InferenceUsage>,
    pub finish_reason: CreationRoundFinishReason,
    pub provider_request_id: Option<String>,
    pub calls: Vec<NewCreationToolCall>,
    pub admitted_at: TimestampMillis,
}

impl NewCreationInferenceRound {
    pub fn validate(&self) -> Result<(), CreationAttemptError> {
        if self.ordinal >= MAX_CREATION_INFERENCE_ROUNDS
            || (self.parts.is_empty() && self.calls.is_empty())
            || self.parts.len() > 64
            || self.calls.len() > lettuce_conversations::MAX_TOOL_CALLS_PER_RESPONSE
        {
            return Err(CreationAttemptError::InvalidRound);
        }
        if self
            .provider_request_id
            .as_ref()
            .is_some_and(|id| id.trim().is_empty() || id.len() > 256)
        {
            return Err(CreationAttemptError::InvalidRound);
        }
        for part in &self.parts {
            part.validate()
                .map_err(|_| CreationAttemptError::InvalidRound)?;
            if !matches!(
                part,
                MessagePart::Text { .. } | MessagePart::ReasoningSummary { .. }
            ) {
                return Err(CreationAttemptError::InvalidRound);
            }
        }
        if let Some(replay) = &self.provider_replay {
            replay
                .validate()
                .map_err(|_| CreationAttemptError::InvalidRound)?;
            if replay.retention != ReplayRetention::Conversation {
                return Err(CreationAttemptError::InvalidRound);
            }
        }
        let mut ids = std::collections::HashSet::new();
        let mut provider_ids = std::collections::HashSet::new();
        for call in &self.calls {
            call.call
                .validate()
                .map_err(|_| CreationAttemptError::InvalidCall)?;
            if call.definition_version == 0
                || !ids.insert(call.id)
                || call
                    .call
                    .provider_call_id
                    .as_deref()
                    .is_some_and(|id| !provider_ids.insert(id))
                || call.call.provider_replay.as_ref() != self.provider_replay.as_ref()
            {
                return Err(CreationAttemptError::InvalidCall);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreationInferenceRound {
    pub workflow_id: CreationWorkflowId,
    pub turn_id: CreationTurnId,
    pub attempt_id: GenerationAttemptId,
    pub ordinal: u8,
    pub first_call_ordinal: u16,
    pub parts: Vec<MessagePart>,
    pub provider_replay: Option<ReplayArtifactRef>,
    pub usage: Option<InferenceUsage>,
    pub finish_reason: CreationRoundFinishReason,
    pub provider_request_id: Option<String>,
    pub calls: Vec<CreationToolCallEvidence>,
    pub admitted_at: TimestampMillis,
}

impl CreationInferenceRound {
    pub fn validate(&self) -> Result<(), CreationAttemptError> {
        NewCreationInferenceRound {
            ordinal: self.ordinal,
            parts: self.parts.clone(),
            provider_replay: self.provider_replay.clone(),
            usage: self.usage.clone(),
            finish_reason: self.finish_reason,
            provider_request_id: self.provider_request_id.clone(),
            calls: self
                .calls
                .iter()
                .map(|call| NewCreationToolCall {
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
            .is_none_or(|count| count > crate::proposal::MAX_CREATION_OPERATIONS)
        {
            return Err(CreationAttemptError::InvalidRound);
        }
        for (offset, call) in self.calls.iter().enumerate() {
            let expected = self
                .first_call_ordinal
                .checked_add(u16::try_from(offset).map_err(|_| CreationAttemptError::InvalidRound)?)
                .ok_or(CreationAttemptError::InvalidRound)?;
            if call.workflow_id != self.workflow_id
                || call.turn_id != self.turn_id
                || call.attempt_id != self.attempt_id
                || call.round_ordinal != self.ordinal
                || call.ordinal != expected
                || call.admitted_at != self.admitted_at
            {
                return Err(CreationAttemptError::InvalidRound);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreationToolCallEvidence {
    pub id: ToolExecutionId,
    pub workflow_id: CreationWorkflowId,
    pub turn_id: CreationTurnId,
    pub attempt_id: GenerationAttemptId,
    pub round_ordinal: u8,
    pub ordinal: u16,
    pub definition_version: u32,
    pub call: ProposedToolCall,
    pub admitted_at: TimestampMillis,
}

impl CreationToolCallEvidence {
    pub fn validate(&self) -> Result<(), CreationAttemptError> {
        self.call
            .validate()
            .map_err(|_| CreationAttemptError::InvalidCall)?;
        if self.definition_version == 0 {
            return Err(CreationAttemptError::InvalidCall);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CreationAttemptError {
    #[error("creation inference contract is invalid")]
    InvalidContract,
    #[error("creation inference attempt state is invalid")]
    InvalidState,
    #[error("creation inference transition is invalid")]
    InvalidTransition,
    #[error("creation tool call is invalid")]
    InvalidCall,
    #[error("creation inference round is invalid")]
    InvalidRound,
}

#[cfg(test)]
mod tests {
    use lettuce_types::{
        CreationProposalId, CreationTurnId, CreationWorkflowId, GenerationAttemptId,
    };

    use super::*;

    fn attempt() -> CreationInferenceAttempt {
        CreationInferenceAttempt {
            id: GenerationAttemptId::new(),
            workflow_id: CreationWorkflowId::new(),
            turn_id: CreationTurnId::new(),
            ordinal: 0,
            retry_parent_id: None,
            base_proposal_id: CreationProposalId::new(),
            planned_proposal_id: CreationProposalId::new(),
            target: CreationTargetKind::Character,
            stage: CreationStage::Drafting,
            tool_request: creation_tool_request(
                CreationTargetKind::Character,
                CreationStage::Drafting,
            )
            .expect("tools"),
            status: CreationAttemptStatus::Created,
            failure: None,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(1),
            started_at: None,
            finished_at: None,
            updated_at: TimestampMillis::new(1),
        }
    }

    #[test]
    fn attempt_transitions_preserve_terminal_and_failure_invariants() {
        let created = attempt();
        created.validate().expect("created");
        let running = created
            .transition(
                CreationAttemptStatus::Running,
                None,
                TimestampMillis::new(2),
            )
            .expect("running");
        let failed = running
            .transition(
                CreationAttemptStatus::Failed,
                Some(CreationAttemptFailureCode::ProviderRejected),
                TimestampMillis::new(3),
            )
            .expect("failed");
        assert_eq!(failed.revision, Revision::new(3));
        assert_eq!(failed.finished_at, Some(TimestampMillis::new(3)));
        assert!(
            failed
                .transition(
                    CreationAttemptStatus::Running,
                    None,
                    TimestampMillis::new(4)
                )
                .is_err()
        );
        assert!(
            running
                .transition(CreationAttemptStatus::Failed, None, TimestampMillis::new(3))
                .is_err()
        );
    }

    #[test]
    fn attempt_rejects_a_tool_contract_from_another_target() {
        let mut attempt = attempt();
        attempt.tool_request =
            creation_tool_request(CreationTargetKind::Persona, CreationStage::Drafting)
                .expect("persona tools");
        assert_eq!(
            attempt.validate(),
            Err(CreationAttemptError::InvalidContract)
        );
    }
}
