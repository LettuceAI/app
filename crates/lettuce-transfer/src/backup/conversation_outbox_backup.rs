use std::collections::{BTreeMap, BTreeSet};

use lettuce_conversations::{
    ConversationOutboxEvent, ConversationOutboxRecord, OperationKind, OperationRecord,
    OperationResultRef,
};
use lettuce_types::{ConversationId, GenerationAttemptId, GenerationTurnId, UsageEventId};
use serde::{Deserialize, Serialize};

pub const CONVERSATION_OUTBOX_BACKUP_VERSION: u32 = 1;
pub const MAX_BACKUP_CONVERSATION_OPERATIONS: usize = 1_000_000;
pub const MAX_BACKUP_CONVERSATION_OUTBOX_EVENTS: usize = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationOutboxBackup {
    pub version: u32,
    pub conversations: Vec<BackupConversationOutbox>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupConversationOutbox {
    pub conversation_id: ConversationId,
    pub operations: Vec<OperationRecord>,
    pub events: Vec<ConversationOutboxRecord>,
}

impl ConversationOutboxBackup {
    pub fn canonicalize_and_validate(
        &mut self,
        history: &crate::ConversationHistoryBackup,
        runtime: &crate::ConversationRuntimeBackup,
        usage: &crate::ConversationUsageBackup,
    ) -> Result<(), ConversationOutboxBackupError> {
        if self.version != CONVERSATION_OUTBOX_BACKUP_VERSION {
            return Err(ConversationOutboxBackupError::InvalidData);
        }
        self.conversations
            .sort_by_key(|value| value.conversation_id);
        let history = history
            .conversations
            .iter()
            .map(|value| (value.aggregate.conversation.id, value))
            .collect::<BTreeMap<_, _>>();
        let runtime = runtime
            .conversations
            .iter()
            .map(|value| (value.conversation_id, value))
            .collect::<BTreeMap<_, _>>();
        if self.conversations.len() != history.len() {
            return Err(ConversationOutboxBackupError::InvalidData);
        }
        let usage = usage
            .events
            .iter()
            .map(|entry| (entry.event.id, entry.event.record.attempt_id))
            .collect::<BTreeMap<_, _>>();
        let mut conversation_ids = BTreeSet::new();
        let mut operation_ids = BTreeSet::new();
        let mut operation_count = 0_usize;
        let mut event_count = 0_usize;
        for journal in &mut self.conversations {
            let history = history
                .get(&journal.conversation_id)
                .ok_or(ConversationOutboxBackupError::InvalidData)?;
            let runtime = runtime
                .get(&journal.conversation_id)
                .ok_or(ConversationOutboxBackupError::InvalidData)?;
            if !conversation_ids.insert(journal.conversation_id) {
                return Err(ConversationOutboxBackupError::InvalidData);
            }
            journal
                .operations
                .sort_by_key(|operation| (operation.created_at, operation.id));
            journal
                .events
                .sort_by_key(|event| (event.sequence, event.id));
            operation_count = operation_count
                .checked_add(journal.operations.len())
                .ok_or(ConversationOutboxBackupError::LimitExceeded)?;
            event_count = event_count
                .checked_add(journal.events.len())
                .ok_or(ConversationOutboxBackupError::LimitExceeded)?;
            let references = ConversationReferences::new(history, runtime);
            let mut operation_keys = BTreeSet::new();
            let mut operations = BTreeMap::new();
            for operation in &journal.operations {
                if operation.conversation_id != journal.conversation_id
                    || !operation_ids.insert(operation.id)
                    || !operation_keys.insert((
                        operation_kind_rank(operation.kind),
                        operation.operation.key.as_str().to_owned(),
                    ))
                    || !references.has_operation_result(&operation.result)
                    || operations.insert(operation.id, operation.kind).is_some()
                {
                    return Err(ConversationOutboxBackupError::InvalidData);
                }
            }
            let mut event_ids = BTreeSet::new();
            for (index, event) in journal.events.iter().enumerate() {
                let expected_sequence = u64::try_from(index + 1)
                    .map_err(|_| ConversationOutboxBackupError::LimitExceeded)?;
                let operation_kind = operations
                    .get(&event.operation_record_id)
                    .ok_or(ConversationOutboxBackupError::InvalidData)?;
                if event.conversation_id != journal.conversation_id
                    || event.sequence != expected_sequence
                    || event.conversation_revision > history.aggregate.conversation.revision
                    || !event_ids.insert(event.id)
                    || event.validate().is_err()
                    || event_timestamp(&event.event) != event.at
                    || matches!(
                        event.event,
                        ConversationOutboxEvent::ConversationCreated { .. }
                    ) != (*operation_kind == OperationKind::Create)
                    || !references.has_event_refs(&event.event, &usage)
                {
                    return Err(ConversationOutboxBackupError::InvalidData);
                }
            }
            if !matches!(
                journal.events.first().map(|event| &event.event),
                Some(ConversationOutboxEvent::ConversationCreated { .. })
            ) {
                return Err(ConversationOutboxBackupError::InvalidData);
            }
        }
        if operation_count > MAX_BACKUP_CONVERSATION_OPERATIONS
            || event_count > MAX_BACKUP_CONVERSATION_OUTBOX_EVENTS
        {
            return Err(ConversationOutboxBackupError::LimitExceeded);
        }
        Ok(())
    }
}

struct ConversationReferences {
    conversation_id: ConversationId,
    branches: BTreeSet<lettuce_types::ConversationBranchId>,
    participants: BTreeSet<lettuce_types::ConversationParticipantId>,
    messages: BTreeSet<lettuce_types::MessageId>,
    revisions: BTreeSet<lettuce_types::MessageRevisionId>,
    candidates: BTreeSet<lettuce_types::MessageCandidateId>,
    turns: BTreeSet<GenerationTurnId>,
    attempts: BTreeMap<GenerationAttemptId, GenerationTurnId>,
}

impl ConversationReferences {
    fn new(
        history: &crate::BackupConversation,
        runtime: &crate::BackupConversationRuntime,
    ) -> Self {
        Self {
            conversation_id: history.aggregate.conversation.id,
            branches: history
                .aggregate
                .branches
                .iter()
                .map(|branch| branch.id)
                .collect(),
            participants: history
                .aggregate
                .conversation
                .participants
                .iter()
                .map(|participant| participant.id)
                .collect(),
            messages: history
                .messages
                .iter()
                .map(|message| message.message.id)
                .collect(),
            revisions: history
                .messages
                .iter()
                .flat_map(|message| message.revisions.iter().map(|revision| revision.id))
                .collect(),
            candidates: history
                .messages
                .iter()
                .flat_map(|message| message.candidates.iter().map(|candidate| candidate.id))
                .collect(),
            turns: runtime.turns.iter().map(|turn| turn.turn.id).collect(),
            attempts: runtime
                .turns
                .iter()
                .flat_map(|turn| {
                    turn.turn
                        .attempts
                        .iter()
                        .map(|attempt| (attempt.id, turn.turn.id))
                })
                .collect(),
        }
    }

    fn has_operation_result(&self, result: &OperationResultRef) -> bool {
        match result {
            OperationResultRef::Conversation(id) => *id == self.conversation_id,
            OperationResultRef::Turn(id) => self.turns.contains(id),
            OperationResultRef::Message(id) => self.messages.contains(id),
            OperationResultRef::Candidate(id) => self.candidates.contains(id),
            OperationResultRef::Branch(id) => self.branches.contains(id),
        }
    }

    fn owns_attempt(&self, turn_id: GenerationTurnId, attempt_id: GenerationAttemptId) -> bool {
        self.turns.contains(&turn_id) && self.attempts.get(&attempt_id) == Some(&turn_id)
    }

    fn owns_usage(
        &self,
        attempt_id: GenerationAttemptId,
        usage_id: UsageEventId,
        usage: &BTreeMap<UsageEventId, GenerationAttemptId>,
    ) -> bool {
        self.attempts.contains_key(&attempt_id) && usage.get(&usage_id) == Some(&attempt_id)
    }

    fn has_event_refs(
        &self,
        event: &ConversationOutboxEvent,
        usage: &BTreeMap<UsageEventId, GenerationAttemptId>,
    ) -> bool {
        match event {
            ConversationOutboxEvent::ConversationCreated {
                root_branch_id,
                head_message_id,
                ..
            } => {
                self.branches.contains(root_branch_id)
                    && head_message_id
                        .as_ref()
                        .is_none_or(|id| self.messages.contains(id))
            }
            ConversationOutboxEvent::MessageCommitted {
                branch_id,
                message_id,
                revision_id,
                candidate_id,
                ..
            } => {
                self.branches.contains(branch_id)
                    && self.messages.contains(message_id)
                    && revision_id
                        .as_ref()
                        .is_none_or(|id| self.revisions.contains(id))
                    && candidate_id
                        .as_ref()
                        .is_none_or(|id| self.candidates.contains(id))
            }
            ConversationOutboxEvent::MessageRevised {
                branch_id,
                message_id,
                revision_id,
                ..
            } => {
                self.branches.contains(branch_id)
                    && self.messages.contains(message_id)
                    && self.revisions.contains(revision_id)
            }
            ConversationOutboxEvent::MessageTombstoned {
                branch_id,
                message_id,
                affected_message_ids,
                affected_revision_ids,
                ..
            } => {
                self.branches.contains(branch_id)
                    && self.messages.contains(message_id)
                    && affected_message_ids
                        .iter()
                        .all(|id| self.messages.contains(id))
                    && affected_revision_ids
                        .iter()
                        .all(|id| self.revisions.contains(id))
            }
            ConversationOutboxEvent::TurnFinalized {
                branch_id,
                turn_id,
                attempt_id,
                message_id,
                candidate_id,
                revision_id,
                usage_event_id,
                ..
            } => {
                self.branches.contains(branch_id)
                    && self.owns_attempt(*turn_id, *attempt_id)
                    && self.messages.contains(message_id)
                    && self.candidates.contains(candidate_id)
                    && revision_id
                        .as_ref()
                        .is_none_or(|id| self.revisions.contains(id))
                    && self.owns_usage(*attempt_id, *usage_event_id, usage)
            }
            ConversationOutboxEvent::TurnFailed {
                branch_id,
                turn_id,
                attempt_id,
                usage_event_id,
                ..
            }
            | ConversationOutboxEvent::TurnInterrupted {
                branch_id,
                turn_id,
                attempt_id,
                usage_event_id,
                ..
            }
            | ConversationOutboxEvent::TurnCancelled {
                branch_id,
                turn_id,
                attempt_id,
                usage_event_id,
                ..
            } => {
                self.branches.contains(branch_id)
                    && self.owns_attempt(*turn_id, *attempt_id)
                    && self.owns_usage(*attempt_id, *usage_event_id, usage)
            }
            ConversationOutboxEvent::TurnRecovering {
                branch_id,
                turn_id,
                previous_attempt_id,
                attempt_id,
                ..
            } => {
                self.branches.contains(branch_id)
                    && self.owns_attempt(*turn_id, *previous_attempt_id)
                    && self.owns_attempt(*turn_id, *attempt_id)
            }
            ConversationOutboxEvent::TurnCancellationRequested {
                branch_id,
                turn_id,
                attempt_id,
                ..
            } => self.branches.contains(branch_id) && self.owns_attempt(*turn_id, *attempt_id),
            ConversationOutboxEvent::BranchForked { branch_id, .. }
            | ConversationOutboxEvent::BranchSelected { branch_id, .. } => {
                self.branches.contains(branch_id)
            }
            ConversationOutboxEvent::CandidateChosen {
                message_id,
                candidate_id,
                ..
            } => self.messages.contains(message_id) && self.candidates.contains(candidate_id),
            ConversationOutboxEvent::ParticipantPolicyChanged { participant_id, .. } => {
                self.participants.contains(participant_id)
            }
            ConversationOutboxEvent::MessageFlagsChanged { message_id, .. } => {
                self.messages.contains(message_id)
            }
            ConversationOutboxEvent::AssetReferencesChanged {
                message_revision_id,
                candidate_id,
                ..
            } => {
                message_revision_id
                    .as_ref()
                    .is_none_or(|id| self.revisions.contains(id))
                    && candidate_id
                        .as_ref()
                        .is_none_or(|id| self.candidates.contains(id))
            }
            ConversationOutboxEvent::ConversationLifecycleChanged { .. }
            | ConversationOutboxEvent::TitleChanged { .. }
            | ConversationOutboxEvent::SettingsChanged { .. }
            | ConversationOutboxEvent::ConversationTombstoned { .. } => true,
        }
    }
}

fn event_timestamp(event: &ConversationOutboxEvent) -> lettuce_types::TimestampMillis {
    match event {
        ConversationOutboxEvent::ConversationCreated { at, .. }
        | ConversationOutboxEvent::MessageCommitted { at, .. }
        | ConversationOutboxEvent::MessageRevised { at, .. }
        | ConversationOutboxEvent::MessageTombstoned { at, .. }
        | ConversationOutboxEvent::TurnFailed { at, .. }
        | ConversationOutboxEvent::TurnInterrupted { at, .. }
        | ConversationOutboxEvent::TurnRecovering { at, .. }
        | ConversationOutboxEvent::TurnCancellationRequested { at, .. }
        | ConversationOutboxEvent::TurnCancelled { at, .. }
        | ConversationOutboxEvent::BranchForked { at, .. }
        | ConversationOutboxEvent::CandidateChosen { at, .. }
        | ConversationOutboxEvent::BranchSelected { at, .. }
        | ConversationOutboxEvent::ConversationLifecycleChanged { at, .. }
        | ConversationOutboxEvent::TitleChanged { at, .. }
        | ConversationOutboxEvent::SettingsChanged { at, .. }
        | ConversationOutboxEvent::ParticipantPolicyChanged { at, .. }
        | ConversationOutboxEvent::MessageFlagsChanged { at, .. }
        | ConversationOutboxEvent::ConversationTombstoned { at, .. }
        | ConversationOutboxEvent::AssetReferencesChanged { at, .. } => *at,
        ConversationOutboxEvent::TurnFinalized { effective_time, .. } => *effective_time,
    }
}

fn operation_kind_rank(kind: OperationKind) -> u8 {
    match kind {
        OperationKind::Create => 0,
        OperationKind::Send => 1,
        OperationKind::Continue => 2,
        OperationKind::Regenerate => 3,
        OperationKind::Retry => 4,
        OperationKind::Checkpoint => 5,
        OperationKind::Cancel => 6,
        OperationKind::Finalize => 7,
        OperationKind::Fail => 8,
        OperationKind::Interrupt => 9,
        OperationKind::Recover => 10,
        OperationKind::ChooseCandidate => 11,
        OperationKind::Edit => 12,
        OperationKind::Flags => 13,
        OperationKind::Fork => 14,
        OperationKind::SelectBranch => 15,
        OperationKind::Tombstone => 16,
        OperationKind::Archive => 17,
        OperationKind::Restore => 18,
        OperationKind::Rename => 19,
        OperationKind::ParticipantPolicy => 20,
        OperationKind::Settings => 21,
        OperationKind::AttachJob => 22,
        OperationKind::PrepareGeneration => 23,
        OperationKind::ResolveSpeaker => 24,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ConversationOutboxBackupError {
    #[error("conversation outbox backup exceeds its limit")]
    LimitExceeded,
    #[error("conversation outbox backup is invalid")]
    InvalidData,
}
