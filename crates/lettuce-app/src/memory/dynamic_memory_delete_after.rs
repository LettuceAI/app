use std::collections::HashSet;

use lettuce_companions::{
    CompanionTurnEffect, CompanionTurnEffectRepository, CompanionTurnEffectRepositoryError,
    CompanionTurnEffectStatus,
};
use lettuce_conversations::{
    Conversation, ConversationOutboxEvent, ConversationRepository, ConversationRepositoryError,
    DescendantPolicy, ForkBranch, MessageVisibility, OperationKind, OperationResultRef,
    OperationToken, TombstoneMessage, TombstoneMessageResult,
};
use lettuce_jobs::JobStore;
use lettuce_memory::{
    DynamicMemoryApprovalRepository, DynamicMemoryRunRepository, DynamicMemoryRunRepositoryError,
    DynamicMemorySuffixRewind, DynamicMemorySuffixRewindError, DynamicMemorySuffixRewindReceipt,
    DynamicMemorySuffixRewindRepository, MemoryRepository, MemoryRepositoryError,
    PendingSuffixRewind, PendingSuffixRewindRepository,
};
use lettuce_types::{ConversationId, MessageId, OperationId, PageLimit, PageRequest, Revision};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteAfterMessages {
    pub conversation_id: ConversationId,
    pub after_message_id: MessageId,
    pub expected_revision: Revision,
    pub operation: OperationToken,
    pub summary_message_interval: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DynamicMemoryDeleteAfterResult {
    pub conversation: Conversation,
    pub tombstone: Option<TombstoneMessageResult>,
    pub branch_id: Option<lettuce_types::ConversationBranchId>,
    pub rewind: Option<DynamicMemorySuffixRewindReceipt>,
    pub retained_effects: Vec<CompanionTurnEffect>,
    pub rebuild_admission: Option<crate::CompanionPostTurnMemoryAdmission>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DynamicMemoryDeleteAfterError {
    #[error("delete-after conversation mutation failed: {0}")]
    Conversation(#[from] ConversationRepositoryError),
    #[error("delete-after memory run lookup failed: {0}")]
    Runs(#[from] DynamicMemoryRunRepositoryError),
    #[error("delete-after companion effect lookup failed: {0:?}")]
    Effects(CompanionTurnEffectRepositoryError),
    #[error("delete-after memory lookup failed: {0}")]
    Memory(#[from] MemoryRepositoryError),
    #[error("delete-after memory rewind failed: {0}")]
    Rewind(#[from] DynamicMemorySuffixRewindError),
    #[error("delete-after memory rebuild admission failed: {0}")]
    Admission(#[from] crate::CompanionPostTurnMemoryAdmissionError),
    #[error("delete-after durable result is inconsistent")]
    InvalidResult,
    #[error(
        "a delete-after memory rewind is owed for conversation {conversation_id} and failed: {reason}"
    )]
    OwedRewind {
        conversation_id: ConversationId,
        reason: String,
    },
}

/// The owed rewinds one `complete_pending` pass finished and the
/// conversations whose rewind failed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PendingRewindReport {
    pub completed: usize,
    pub failed: Vec<(ConversationId, DynamicMemoryDeleteAfterError)>,
}

impl PendingRewindReport {
    /// The first failure as the typed error a caller of one conversation
    /// returns.
    pub fn into_result(self) -> Result<usize, DynamicMemoryDeleteAfterError> {
        match self.failed.into_iter().next() {
            None => Ok(self.completed),
            Some((conversation_id, error)) => Err(DynamicMemoryDeleteAfterError::OwedRewind {
                conversation_id,
                reason: error.to_string(),
            }),
        }
    }
}

#[derive(Debug)]
pub struct DynamicMemoryDeleteAfterCoordinator<'a, R: ?Sized, J: ?Sized> {
    repository: &'a R,
    jobs: &'a J,
}

impl<'a, R: ?Sized, J: ?Sized> DynamicMemoryDeleteAfterCoordinator<'a, R, J> {
    #[must_use]
    pub const fn new(repository: &'a R, jobs: &'a J) -> Self {
        Self { repository, jobs }
    }
}

impl<R, J> DynamicMemoryDeleteAfterCoordinator<'_, R, J>
where
    R: ConversationRepository
        + lettuce_conversations::ConversationOverviewReader
        + DynamicMemoryRunRepository
        + DynamicMemorySuffixRewindRepository
        + MemoryRepository
        + CompanionTurnEffectRepository
        + DynamicMemoryApprovalRepository
        + PendingSuffixRewindRepository
        + ?Sized,
    J: JobStore + ?Sized,
{
    /// Removes what follows the anchor on the selected branch. Rewinds a
    /// crash left owed for the conversation finish first. When every
    /// message after the anchor belongs to the selected branch alone, they
    /// are tombstoned and the memory rewind that owes is recorded in the
    /// same transaction, then rewound and cleared. When any of them is also
    /// shown by another branch (an ancestor, or a branch forked from one of
    /// them), a new branch is forked at the anchor and selected instead and
    /// nothing is deleted. An anchor with nothing after it records a
    /// durable no-op. Repeating the command replays it.
    pub fn delete_after(
        &self,
        command: &DeleteAfterMessages,
        now: lettuce_types::TimestampMillis,
    ) -> Result<DynamicMemoryDeleteAfterResult, DynamicMemoryDeleteAfterError> {
        if command.summary_message_interval == 0 {
            return Err(DynamicMemoryDeleteAfterError::InvalidResult);
        }
        if self.recorded(command)?.is_none() {
            self.complete_pending(Some(command.conversation_id), now)?
                .into_result()?;
        }
        self.delete_after_recorded(command, now)
    }

    fn delete_after_recorded(
        &self,
        command: &DeleteAfterMessages,
        now: lettuce_types::TimestampMillis,
    ) -> Result<DynamicMemoryDeleteAfterResult, DynamicMemoryDeleteAfterError> {
        let aggregate = lettuce_conversations::ConversationReader::get(
            self.repository,
            command.conversation_id,
        )?;
        let first_removed = match self.recorded(command)? {
            Some(Recorded::Empty) => {
                return Ok(unchanged(aggregate.conversation));
            }
            Some(Recorded::Branch(branch_id)) => {
                let branch = aggregate
                    .branches
                    .iter()
                    .find(|branch| branch.id == branch_id)
                    .ok_or(DynamicMemoryDeleteAfterError::InvalidResult)?;
                if branch.fork_message_id != Some(command.after_message_id) {
                    return Err(ConversationRepositoryError::Conflict.into());
                }
                return Ok(DynamicMemoryDeleteAfterResult {
                    branch_id: Some(branch_id),
                    ..unchanged(aggregate.conversation)
                });
            }
            Some(Recorded::Suffix(message_id)) => message_id,
            None => {
                let active_branch_id = aggregate.conversation.active_branch_id;
                let suffix = self.scan_suffix(
                    command.conversation_id,
                    active_branch_id,
                    command.after_message_id,
                )?;
                match suffix.first_visible {
                    None => return self.record_empty(command, now),
                    Some(_)
                        if self.repository.suffix_shared_with_other_branches(
                            command.conversation_id,
                            active_branch_id,
                            command.after_message_id,
                        )? =>
                    {
                        return self.fork_at_anchor(command, suffix.anchor_branch_id, now);
                    }
                    Some((message_id, _)) => message_id,
                }
            }
        };
        let pending = self.pending(command, first_removed);
        let tombstone = self.repository.tombstone_suffix(&pending, now)?;
        let result = self.rewind(command, tombstone)?;
        self.repository.clear_pending_suffix_rewind(&pending)?;
        Ok(result)
    }

    #[cfg(test)]
    pub(crate) fn rewind_without_clearing(
        &self,
        command: &DeleteAfterMessages,
        first_removed: MessageId,
        now: lettuce_types::TimestampMillis,
    ) -> Result<(), DynamicMemoryDeleteAfterError> {
        let pending = self.pending(command, first_removed);
        let tombstone = self.repository.tombstone_suffix(&pending, now)?;
        self.rewind(command, tombstone)?;
        Ok(())
    }

    fn pending(&self, command: &DeleteAfterMessages, message_id: MessageId) -> PendingSuffixRewind {
        PendingSuffixRewind {
            after_message_id: command.after_message_id,
            tombstone: TombstoneMessage {
                conversation_id: command.conversation_id,
                message_id,
                expected_revision: command.expected_revision,
                operation: command.operation.clone(),
                descendants: DescendantPolicy::Tombstone,
            },
            summary_message_interval: command.summary_message_interval,
        }
    }

    fn record_empty(
        &self,
        command: &DeleteAfterMessages,
        now: lettuce_types::TimestampMillis,
    ) -> Result<DynamicMemoryDeleteAfterResult, DynamicMemoryDeleteAfterError> {
        let pending = self.pending(command, command.after_message_id);
        let recorded = self.repository.record_empty_suffix(&pending, now)?;
        Ok(unchanged(recorded.value))
    }

    fn fork_at_anchor(
        &self,
        command: &DeleteAfterMessages,
        source_branch_id: lettuce_types::ConversationBranchId,
        now: lettuce_types::TimestampMillis,
    ) -> Result<DynamicMemoryDeleteAfterResult, DynamicMemoryDeleteAfterError> {
        let forked = self.repository.fork_branch(
            &ForkBranch {
                conversation_id: command.conversation_id,
                source_branch_id,
                at_message_id: Some(command.after_message_id),
                expected_revision: command.expected_revision,
                operation: command.operation.clone(),
            },
            now,
        )?;
        Ok(DynamicMemoryDeleteAfterResult {
            branch_id: Some(forked.value.branch.id),
            ..unchanged(forked.value.conversation)
        })
    }

    fn recorded(
        &self,
        command: &DeleteAfterMessages,
    ) -> Result<Option<Recorded>, DynamicMemoryDeleteAfterError> {
        for kind in [OperationKind::Tombstone, OperationKind::Fork] {
            let Some(record) = self.repository.operation_record(
                command.conversation_id,
                kind,
                &command.operation,
            )?
            else {
                continue;
            };
            if record.operation.request_digest != command.operation.request_digest {
                return Err(ConversationRepositoryError::Conflict.into());
            }
            return Ok(Some(match record.result {
                OperationResultRef::Message(message_id)
                    if message_id == command.after_message_id =>
                {
                    Recorded::Empty
                }
                OperationResultRef::Message(message_id) => Recorded::Suffix(message_id),
                OperationResultRef::Branch(branch_id) => Recorded::Branch(branch_id),
                _ => return Err(DynamicMemoryDeleteAfterError::InvalidResult),
            }));
        }
        Ok(None)
    }

    /// Finishes every delete-after whose memory rewind is still owed, of one
    /// conversation or of all, oldest first. A rewind that fails is recorded
    /// on its conversation and reported; that conversation's later owed
    /// rewinds wait, and the other conversations go on.
    pub fn complete_pending(
        &self,
        conversation_id: Option<ConversationId>,
        now: lettuce_types::TimestampMillis,
    ) -> Result<PendingRewindReport, DynamicMemoryDeleteAfterError> {
        let pending = self.repository.pending_suffix_rewinds(conversation_id)?;
        let mut report = PendingRewindReport::default();
        for owed in &pending {
            let conversation = owed.tombstone.conversation_id;
            if report
                .failed
                .iter()
                .any(|(failed, _)| *failed == conversation)
            {
                continue;
            }
            let outcome = self.delete_after_recorded(
                &DeleteAfterMessages {
                    conversation_id: conversation,
                    after_message_id: owed.after_message_id,
                    expected_revision: owed.tombstone.expected_revision,
                    operation: owed.tombstone.operation.clone(),
                    summary_message_interval: owed.summary_message_interval,
                },
                now,
            );
            match outcome {
                Ok(_) => report.completed += 1,
                Err(error) => {
                    self.repository
                        .fail_pending_suffix_rewind(owed, failure_of(&error))?;
                    report.failed.push((conversation, error));
                }
            }
        }
        Ok(report)
    }

    fn rewind(
        &self,
        command: &DeleteAfterMessages,
        tombstone: TombstoneMessageResult,
    ) -> Result<DynamicMemoryDeleteAfterResult, DynamicMemoryDeleteAfterError> {
        let (removed_message_ids, rewind_at) = removed_messages(&tombstone)?;
        let operation_id = OperationId::from_uuid(tombstone.operation.id.as_uuid());
        let active_space = self
            .repository
            .get_for_branch(command.conversation_id, tombstone.value.message.branch_id)?
            .map(|memory| memory.id);
        let runs = self
            .repository
            .list_dynamic_memory_runs(command.conversation_id)?
            .into_iter()
            .filter(|run| Some(run.space_id) == active_space)
            .collect::<Vec<_>>();
        let effects = self
            .repository
            .list_for_conversation(command.conversation_id)
            .map_err(DynamicMemoryDeleteAfterError::Effects)?;
        let removed = removed_message_ids.iter().copied().collect::<HashSet<_>>();
        let invalid_run_index = runs.iter().position(|run| {
            run.source_messages
                .iter()
                .any(|source| removed.contains(&source.message_id))
        });
        let invalid_sources = invalid_run_index
            .map(|index| {
                runs[index..]
                    .iter()
                    .flat_map(|run| run.source_messages.iter().map(|source| source.message_id))
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default();
        let mut invalidated_effect_ids = Vec::new();
        let mut retained_effects = Vec::new();
        for effect in effects {
            let message_ids = effect_message_ids(&effect);
            if message_ids.iter().any(|id| removed.contains(id)) {
                invalidated_effect_ids.push(effect.id);
            } else if effect.status != CompanionTurnEffectStatus::Invalidated
                && message_ids.iter().any(|id| invalid_sources.contains(id))
            {
                retained_effects.push(effect);
            }
        }
        invalidated_effect_ids.sort_unstable();
        retained_effects.sort_by_key(|effect| (effect.created_at, effect.id));

        let existing = self
            .repository
            .get_dynamic_memory_suffix_rewind(operation_id)?;
        let rewind = if let Some(receipt) = existing {
            if receipt.conversation_id != command.conversation_id
                || receipt.invalid_run_id != invalid_run_index.map(|index| runs[index].id)
                || receipt.invalidated_effect_ids != invalidated_effect_ids
            {
                return Err(DynamicMemoryDeleteAfterError::InvalidResult);
            }
            Some(receipt)
        } else if invalid_run_index.is_some() || !invalidated_effect_ids.is_empty() {
            let memory = self
                .repository
                .get_for_branch(command.conversation_id, tombstone.value.message.branch_id)?
                .ok_or(DynamicMemoryDeleteAfterError::InvalidResult)?;
            Some(
                self.repository
                    .rewind_dynamic_memory_suffix(DynamicMemorySuffixRewind {
                        operation_id,
                        conversation_id: command.conversation_id,
                        invalid_run_id: invalid_run_index.map(|index| runs[index].id),
                        expected_memory_revision: memory.revision,
                        invalidated_effect_ids,
                        at: rewind_at,
                    })?,
            )
        } else {
            None
        };
        let rebuild_admission = if rewind.is_some() && !retained_effects.is_empty() {
            crate::CompanionPostTurnMemoryAdmissionCoordinator::new(self.repository, self.jobs)
                .rebuild_and_admit(
                    command.conversation_id,
                    operation_id,
                    command.summary_message_interval,
                    retained_effects.clone(),
                )?
        } else {
            None
        };
        Ok(DynamicMemoryDeleteAfterResult {
            conversation: tombstone.value.conversation.clone(),
            tombstone: Some(tombstone),
            branch_id: None,
            rewind,
            retained_effects,
            rebuild_admission,
        })
    }

    fn scan_suffix(
        &self,
        conversation_id: ConversationId,
        branch_id: lettuce_types::ConversationBranchId,
        anchor_id: MessageId,
    ) -> Result<Suffix, DynamicMemoryDeleteAfterError> {
        let mut cursor = None;
        let mut newer = None;
        loop {
            let page = self.repository.timeline_page(
                conversation_id,
                branch_id,
                &PageRequest {
                    cursor,
                    limit: PageLimit::new(200),
                },
            )?;
            for item in page.items {
                if item.message.id == anchor_id {
                    return Ok(Suffix {
                        anchor_branch_id: item.message.branch_id,
                        first_visible: newer,
                    });
                }
                if item.message.visibility != MessageVisibility::Tombstoned {
                    newer = Some((item.message.id, item.message.branch_id));
                }
            }
            let Some(next) = page.next_cursor else {
                return Err(DynamicMemoryDeleteAfterError::Conversation(
                    ConversationRepositoryError::NotFound,
                ));
            };
            cursor = Some(next);
        }
    }
}

enum Recorded {
    Empty,
    Suffix(MessageId),
    Branch(lettuce_types::ConversationBranchId),
}

struct Suffix {
    anchor_branch_id: lettuce_types::ConversationBranchId,
    first_visible: Option<(MessageId, lettuce_types::ConversationBranchId)>,
}

fn unchanged(conversation: Conversation) -> DynamicMemoryDeleteAfterResult {
    DynamicMemoryDeleteAfterResult {
        conversation,
        tombstone: None,
        branch_id: None,
        rewind: None,
        retained_effects: Vec::new(),
        rebuild_admission: None,
    }
}

fn failure_of(error: &DynamicMemoryDeleteAfterError) -> lettuce_memory::OwedRewindFailure {
    use lettuce_memory::OwedRewindFailure;
    match error {
        DynamicMemoryDeleteAfterError::Rewind(DynamicMemorySuffixRewindError::Conflict) => {
            OwedRewindFailure::Conflict
        }
        DynamicMemoryDeleteAfterError::InvalidResult => OwedRewindFailure::Inconsistent,
        DynamicMemoryDeleteAfterError::Rewind(DynamicMemorySuffixRewindError::Storage)
        | DynamicMemoryDeleteAfterError::Conversation(ConversationRepositoryError::Storage) => {
            OwedRewindFailure::Storage
        }
        _ => OwedRewindFailure::Other,
    }
}

fn effect_message_ids(effect: &CompanionTurnEffect) -> Vec<MessageId> {
    effect
        .source_window
        .as_ref()
        .map(|window| window.message_ids.clone())
        .unwrap_or_else(|| {
            effect
                .user_message_id
                .into_iter()
                .chain(std::iter::once(effect.assistant_message_id))
                .collect()
        })
}

fn removed_messages(
    tombstone: &TombstoneMessageResult,
) -> Result<(Vec<MessageId>, lettuce_types::TimestampMillis), DynamicMemoryDeleteAfterError> {
    let mut found = None;
    for record in &tombstone.outbox {
        if let ConversationOutboxEvent::MessageTombstoned {
            conversation_id,
            message_id,
            descendants,
            affected_message_ids,
            at,
            ..
        } = &record.event
        {
            if found.is_some()
                || *conversation_id != tombstone.value.conversation.id
                || *message_id != tombstone.value.message.id
                || *descendants != DescendantPolicy::Tombstone
            {
                return Err(DynamicMemoryDeleteAfterError::InvalidResult);
            }
            let mut removed = Vec::with_capacity(affected_message_ids.len() + 1);
            removed.push(*message_id);
            removed.extend(affected_message_ids.iter().copied());
            if removed.iter().copied().collect::<HashSet<_>>().len() != removed.len() {
                return Err(DynamicMemoryDeleteAfterError::InvalidResult);
            }
            found = Some((removed, *at));
        }
    }
    found.ok_or(DynamicMemoryDeleteAfterError::InvalidResult)
}
