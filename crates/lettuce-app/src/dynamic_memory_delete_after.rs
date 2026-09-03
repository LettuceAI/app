use std::collections::HashSet;

use lettuce_companions::{
    CompanionTurnEffect, CompanionTurnEffectRepository, CompanionTurnEffectRepositoryError,
    CompanionTurnEffectStatus,
};
use lettuce_conversations::{
    Conversation, ConversationOutboxEvent, ConversationRepository, ConversationRepositoryError,
    DescendantPolicy, OperationToken, TombstoneMessage, TombstoneMessageResult,
};
use lettuce_jobs::JobStore;
use lettuce_memory::{
    DynamicMemoryApprovalRepository, DynamicMemoryRunRepository, DynamicMemoryRunRepositoryError,
    DynamicMemorySuffixRewind, DynamicMemorySuffixRewindError, DynamicMemorySuffixRewindReceipt,
    DynamicMemorySuffixRewindRepository, MemoryRepository, MemoryRepositoryError,
};
use lettuce_types::{ConversationId, MessageId, OperationId, PageLimit, PageRequest, Revision};

const DELETE_AFTER_SCAN_LIMIT: u16 = 512;

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
        + DynamicMemoryRunRepository
        + DynamicMemorySuffixRewindRepository
        + MemoryRepository
        + CompanionTurnEffectRepository
        + DynamicMemoryApprovalRepository
        + ?Sized,
    J: JobStore + ?Sized,
{
    pub fn delete_after(
        &self,
        command: &DeleteAfterMessages,
        now: lettuce_types::TimestampMillis,
    ) -> Result<DynamicMemoryDeleteAfterResult, DynamicMemoryDeleteAfterError> {
        if command.summary_message_interval == 0 {
            return Err(DynamicMemoryDeleteAfterError::InvalidResult);
        }
        let aggregate = lettuce_conversations::ConversationReader::get(
            self.repository,
            command.conversation_id,
        )?;
        let Some(first_removed_id) = self.first_descendant(
            command.conversation_id,
            aggregate.conversation.active_branch_id,
            command.after_message_id,
        )?
        else {
            if aggregate.conversation.revision != command.expected_revision {
                return Err(DynamicMemoryDeleteAfterError::Conversation(
                    ConversationRepositoryError::StaleRevision {
                        expected: command.expected_revision,
                        actual: aggregate.conversation.revision,
                    },
                ));
            }
            return Ok(DynamicMemoryDeleteAfterResult {
                conversation: aggregate.conversation,
                tombstone: None,
                rewind: None,
                retained_effects: Vec::new(),
                rebuild_admission: None,
            });
        };
        let tombstone = self.repository.tombstone_message(
            &TombstoneMessage {
                conversation_id: command.conversation_id,
                message_id: first_removed_id,
                expected_revision: command.expected_revision,
                operation: command.operation.clone(),
                descendants: DescendantPolicy::Tombstone,
            },
            now,
        )?;
        let (removed_message_ids, rewind_at) = removed_messages(&tombstone)?;
        let operation_id = OperationId::from_uuid(tombstone.operation.id.as_uuid());
        let runs = self
            .repository
            .list_dynamic_memory_runs(command.conversation_id, DELETE_AFTER_SCAN_LIMIT)?;
        let effects = self
            .repository
            .list_for_conversation(command.conversation_id, DELETE_AFTER_SCAN_LIMIT)
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
                .get_for_conversation(command.conversation_id)?
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
            rewind,
            retained_effects,
            rebuild_admission,
        })
    }

    fn first_descendant(
        &self,
        conversation_id: ConversationId,
        branch_id: lettuce_types::ConversationBranchId,
        anchor_id: MessageId,
    ) -> Result<Option<MessageId>, DynamicMemoryDeleteAfterError> {
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
                    return Ok(newer);
                }
                newer = Some(item.message.id);
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
