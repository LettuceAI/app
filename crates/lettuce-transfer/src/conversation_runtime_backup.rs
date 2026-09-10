use std::collections::{BTreeMap, BTreeSet};

use lettuce_conversations::{
    GenerationCheckpointEnvelope, GenerationTurn, InitialInferenceRecord, ToolExecution,
    TrustedArtifactDescriptor,
};
use lettuce_types::{ConversationId, TimestampMillis};
use serde::{Deserialize, Serialize};

pub const CONVERSATION_RUNTIME_BACKUP_VERSION: u32 = 1;
pub const MAX_BACKUP_GENERATION_TURNS: usize = 200_000;
pub const MAX_BACKUP_GENERATION_CHECKPOINTS: usize = 1_000_000;
pub const MAX_BACKUP_TOOL_EXECUTIONS: usize = 400_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationRuntimeBackup {
    pub version: u32,
    pub conversations: Vec<BackupConversationRuntime>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupConversationRuntime {
    pub conversation_id: ConversationId,
    pub turns: Vec<BackupGenerationTurn>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupGenerationTurn {
    pub turn: GenerationTurn,
    pub attempts: Vec<BackupGenerationAttemptRuntime>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupGenerationAttemptRuntime {
    pub attempt_id: lettuce_types::GenerationAttemptId,
    pub checkpoints: Vec<BackupGenerationCheckpoint>,
    pub speaker_inference: Option<lettuce_conversations::SpeakerInferenceRecord>,
    pub initial_inference: Option<InitialInferenceRecord>,
    pub tools: Vec<ToolExecution>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupGenerationCheckpoint {
    pub envelope: GenerationCheckpointEnvelope,
    pub created_at: TimestampMillis,
}

impl ConversationRuntimeBackup {
    pub(crate) fn artifact_descriptors(
        &self,
    ) -> Result<Vec<TrustedArtifactDescriptor>, ConversationRuntimeBackupError> {
        let mut snapshots = BTreeMap::new();
        let mut replays = BTreeMap::new();
        for conversation in &self.conversations {
            for value in &conversation.turns {
                for model in [
                    value.turn.requested_model_override.as_ref(),
                    value.turn.resolved_model.as_ref(),
                    value
                        .turn
                        .selected_speaker
                        .as_ref()
                        .and_then(|speaker| speaker.decision_model.as_ref()),
                ]
                .into_iter()
                .flatten()
                {
                    crate::conversation_backup::insert_snapshot(
                        &mut snapshots,
                        &model.snapshot_ref,
                    )
                    .map_err(|_| ConversationRuntimeBackupError::InvalidData)?;
                }
                for attempt in &value.attempts {
                    if let Some(model) = attempt
                        .speaker_inference
                        .as_ref()
                        .and_then(|record| record.decision.as_ref())
                        .and_then(|decision| decision.decision_model.as_ref())
                    {
                        crate::conversation_backup::insert_snapshot(
                            &mut snapshots,
                            &model.snapshot_ref,
                        )
                        .map_err(|_| ConversationRuntimeBackupError::InvalidData)?;
                    }
                    if let Some(lettuce_conversations::InitialInferenceResult::Response(outcome)) =
                        attempt
                            .initial_inference
                            .as_ref()
                            .and_then(|record| record.result.as_ref())
                    {
                        for reference in outcome.candidates.iter().flat_map(|candidate| {
                            candidate.provider_replay.iter().chain(
                                candidate
                                    .tool_calls
                                    .iter()
                                    .filter_map(|call| call.provider_replay.as_ref()),
                            )
                        }) {
                            crate::conversation_backup::insert_replay(&mut replays, reference)
                                .map_err(|_| ConversationRuntimeBackupError::InvalidData)?;
                        }
                    }
                    for tool in &attempt.tools {
                        if let Some(reference) = &tool.provider_replay {
                            crate::conversation_backup::insert_replay(&mut replays, reference)
                                .map_err(|_| ConversationRuntimeBackupError::InvalidData)?;
                        }
                    }
                }
            }
        }
        Ok(snapshots
            .into_values()
            .map(TrustedArtifactDescriptor::Snapshot)
            .chain(replays.into_values().map(TrustedArtifactDescriptor::Replay))
            .collect())
    }

    pub fn canonicalize_and_validate(
        &mut self,
        history: &crate::ConversationHistoryBackup,
    ) -> Result<(), ConversationRuntimeBackupError> {
        if self.version != CONVERSATION_RUNTIME_BACKUP_VERSION {
            return Err(ConversationRuntimeBackupError::InvalidData);
        }
        let history_by_id = history
            .conversations
            .iter()
            .map(|value| (value.aggregate.conversation.id, value))
            .collect::<BTreeMap<_, _>>();
        self.conversations
            .sort_by_key(|value| value.conversation_id);
        if self.conversations.len() != history_by_id.len() {
            return Err(ConversationRuntimeBackupError::InvalidData);
        }
        let artifact_descriptors = history
            .artifact_descriptors()
            .map_err(|_| ConversationRuntimeBackupError::InvalidData)?;
        let runtime_descriptors = self.artifact_descriptors()?;
        let replay_ids = artifact_descriptors
            .into_iter()
            .chain(runtime_descriptors)
            .filter_map(|descriptor| match descriptor {
                TrustedArtifactDescriptor::Replay(reference) => Some(reference.artifact_id),
                TrustedArtifactDescriptor::Snapshot(_) => None,
            })
            .collect::<BTreeSet<_>>();
        let mut conversation_ids = BTreeSet::new();
        let mut turn_ids = BTreeSet::new();
        let mut attempt_ids = BTreeSet::new();
        let mut job_ids = BTreeSet::new();
        let mut tool_ids = BTreeSet::new();
        let mut turn_count = 0_usize;
        let mut checkpoint_count = 0_usize;
        let mut tool_count = 0_usize;
        for runtime in &mut self.conversations {
            let history = history_by_id
                .get(&runtime.conversation_id)
                .ok_or(ConversationRuntimeBackupError::InvalidData)?;
            if !conversation_ids.insert(runtime.conversation_id) {
                return Err(ConversationRuntimeBackupError::InvalidData);
            }
            runtime
                .turns
                .sort_by_key(|value| (value.turn.created_at, value.turn.id));
            let branch_ids = history
                .aggregate
                .branches
                .iter()
                .map(|branch| branch.id)
                .collect::<BTreeSet<_>>();
            let message_ids = history
                .messages
                .iter()
                .map(|message| message.message.id)
                .collect::<BTreeSet<_>>();
            let candidate_ids = history
                .messages
                .iter()
                .flat_map(|message| message.candidates.iter().map(|candidate| candidate.id))
                .collect::<BTreeSet<_>>();
            let candidate_owners = history
                .messages
                .iter()
                .flat_map(|message| {
                    message
                        .candidates
                        .iter()
                        .map(|candidate| (candidate.id, (candidate.turn_id, candidate.attempt_id)))
                })
                .collect::<BTreeMap<_, _>>();
            for value in &mut runtime.turns {
                if value.turn.conversation_id != runtime.conversation_id
                    || !branch_ids.contains(&value.turn.branch_id)
                    || !turn_ids.insert(value.turn.id)
                    || value
                        .turn
                        .validate(matches!(
                            history.aggregate.conversation.kind,
                            lettuce_conversations::ConversationKind::Group(_)
                        ))
                        .is_err()
                    || !value
                        .turn
                        .candidate_ids
                        .iter()
                        .all(|id| candidate_ids.contains(id))
                    || value.turn.candidate_ids.iter().any(|id| {
                        candidate_owners
                            .get(id)
                            .is_none_or(|(turn_id, _)| *turn_id != value.turn.id)
                    })
                {
                    return Err(ConversationRuntimeBackupError::InvalidData);
                }
                validate_turn_message_links(&value.turn, &message_ids, &candidate_ids)?;
                value.attempts.sort_by_key(|attempt| {
                    value
                        .turn
                        .attempts
                        .iter()
                        .position(|item| item.id == attempt.attempt_id)
                        .unwrap_or(usize::MAX)
                });
                if value.attempts.len() != value.turn.attempts.len() {
                    return Err(ConversationRuntimeBackupError::InvalidData);
                }
                for (attempt, runtime) in value.turn.attempts.iter().zip(&mut value.attempts) {
                    if runtime.attempt_id != attempt.id
                        || !attempt_ids.insert(attempt.id)
                        || attempt.job_id.is_some_and(|job_id| !job_ids.insert(job_id))
                        || !attempt
                            .candidate_ids
                            .iter()
                            .all(|id| candidate_ids.contains(id))
                        || attempt.candidate_ids.iter().any(|id| {
                            candidate_owners
                                .get(id)
                                .is_none_or(|(turn_id, attempt_id)| {
                                    *turn_id != value.turn.id || *attempt_id != attempt.id
                                })
                        })
                    {
                        return Err(ConversationRuntimeBackupError::InvalidData);
                    }
                    validate_attempt_runtime(
                        runtime,
                        &value.turn,
                        attempt.job_id,
                        &replay_ids,
                        &mut tool_ids,
                    )?;
                    checkpoint_count = checkpoint_count
                        .checked_add(runtime.checkpoints.len())
                        .ok_or(ConversationRuntimeBackupError::LimitExceeded)?;
                    tool_count = tool_count
                        .checked_add(runtime.tools.len())
                        .ok_or(ConversationRuntimeBackupError::LimitExceeded)?;
                }
            }
            turn_count = turn_count
                .checked_add(runtime.turns.len())
                .ok_or(ConversationRuntimeBackupError::LimitExceeded)?;
        }
        if turn_count > MAX_BACKUP_GENERATION_TURNS
            || checkpoint_count > MAX_BACKUP_GENERATION_CHECKPOINTS
            || tool_count > MAX_BACKUP_TOOL_EXECUTIONS
        {
            return Err(ConversationRuntimeBackupError::LimitExceeded);
        }
        Ok(())
    }
}

fn validate_turn_message_links(
    turn: &GenerationTurn,
    messages: &BTreeSet<lettuce_types::MessageId>,
    candidates: &BTreeSet<lettuce_types::MessageCandidateId>,
) -> Result<(), ConversationRuntimeBackupError> {
    let input_valid = match turn.input {
        lettuce_conversations::GenerationInput::UserMessage { message_id }
        | lettuce_conversations::GenerationInput::ExistingHead {
            head_message_id: message_id,
        } => messages.contains(&message_id),
        lettuce_conversations::GenerationInput::ExistingCandidate {
            message_id,
            candidate_id,
        } => messages.contains(&message_id) && candidates.contains(&candidate_id),
    };
    let target_valid = match turn.target {
        lettuce_conversations::GenerationTarget::NewAssistant {
            parent_message_id, ..
        } => parent_message_id.is_none_or(|id| messages.contains(&id)),
        lettuce_conversations::GenerationTarget::ExistingCandidate {
            message_id,
            prior_candidate_id,
        } => messages.contains(&message_id) && candidates.contains(&prior_candidate_id),
    };
    if !input_valid || !target_valid {
        return Err(ConversationRuntimeBackupError::InvalidData);
    }
    Ok(())
}

fn validate_attempt_runtime(
    runtime: &mut BackupGenerationAttemptRuntime,
    turn: &GenerationTurn,
    job_id: Option<lettuce_types::JobId>,
    replay_ids: &BTreeSet<lettuce_types::ReplayArtifactId>,
    tool_ids: &mut BTreeSet<lettuce_types::ToolExecutionId>,
) -> Result<(), ConversationRuntimeBackupError> {
    runtime
        .checkpoints
        .sort_by_key(|value| value.envelope.sequence);
    let mut previous = None;
    for checkpoint in &runtime.checkpoints {
        if checkpoint.envelope.turn_id != turn.id
            || checkpoint.envelope.attempt_id != runtime.attempt_id
            || checkpoint.envelope.job_id != job_id
            || checkpoint.envelope.validate_after(previous).is_err()
        {
            return Err(ConversationRuntimeBackupError::InvalidData);
        }
        previous = Some(&checkpoint.envelope);
    }
    if let Some(record) = &runtime.initial_inference {
        if record.binding.conversation_id != turn.conversation_id
            || record.binding.turn_id != turn.id
            || record.binding.attempt_id != runtime.attempt_id
            || Some(record.binding.job_id) != job_id
            || record.request.stream_sink.is_some()
            || lettuce_conversations::InitialInferenceBinding::from_request(
                turn.conversation_id,
                &record.request,
            )
            .as_ref()
                != Ok(&record.binding)
            || record
                .result
                .as_ref()
                .is_some_and(|result| result.validate().is_err())
            || record.result.is_some() != record.settled_at.is_some()
            || record
                .settled_at
                .is_some_and(|settled| settled < record.admitted_at)
        {
            return Err(ConversationRuntimeBackupError::InvalidData);
        }
    }
    if let Some(record) = &runtime.speaker_inference {
        if record.binding.conversation_id != turn.conversation_id
            || record.binding.turn_id != turn.id
            || record.binding.attempt_id != runtime.attempt_id
            || Some(record.binding.job_id) != job_id
            || record.decision.is_some() != record.settled_at.is_some()
            || record
                .settled_at
                .is_some_and(|settled| settled < record.admitted_at)
            || record
                .decision
                .as_ref()
                .is_some_and(|decision| decision.validate_for_persistence().is_err())
        {
            return Err(ConversationRuntimeBackupError::InvalidData);
        }
    }
    runtime.tools.sort_by_key(|tool| (tool.ordinal, tool.id));
    for (ordinal, tool) in runtime.tools.iter().enumerate() {
        if tool.conversation_id != turn.conversation_id
            || tool.turn_id != turn.id
            || tool.attempt_id != runtime.attempt_id
            || usize::from(tool.ordinal) != ordinal
            || !tool_ids.insert(tool.id)
            || tool.validate().is_err()
            || tool
                .provider_replay
                .as_ref()
                .is_some_and(|reference| !replay_ids.contains(&reference.artifact_id))
        {
            return Err(ConversationRuntimeBackupError::InvalidData);
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ConversationRuntimeBackupError {
    #[error("conversation runtime backup exceeds its limit")]
    LimitExceeded,
    #[error("conversation runtime backup is invalid")]
    InvalidData,
}
