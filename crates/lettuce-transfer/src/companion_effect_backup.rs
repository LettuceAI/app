use std::collections::{BTreeMap, BTreeSet};

use lettuce_companions::{
    CompanionTurnEffect, CompanionTurnEffectOutcome, CompanionTurnEffectStatus,
};
use lettuce_conversations::MessageRole;
use lettuce_memory::{DynamicMemorySuffixRewind, MemorySpaceSnapshot, MemorySummary};
use lettuce_types::{
    CompanionEffectId, ContentHash, ConversationId, DynamicMemoryRunId, MemorySpaceId, OperationId,
    Revision, TimestampMillis,
};
use serde::{Deserialize, Serialize};

pub const COMPANION_EFFECT_BACKUP_VERSION: u32 = 1;
pub const MAX_BACKUP_COMPANION_EFFECTS: usize = 1_000_000;
pub const MAX_BACKUP_MEMORY_REWINDS: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompanionEffectBackup {
    pub version: u32,
    pub effects: Vec<CompanionTurnEffect>,
    pub rewinds: Vec<BackupDynamicMemoryRewind>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupDynamicMemoryRewind {
    pub operation_id: OperationId,
    pub request_digest: ContentHash,
    pub conversation_id: ConversationId,
    pub invalid_run_id: Option<DynamicMemoryRunId>,
    pub space_id: MemorySpaceId,
    pub source_memory_revision: Revision,
    pub resulting_memory_revision: Revision,
    pub restored_summary_run_id: Option<DynamicMemoryRunId>,
    pub resulting_memory: MemorySpaceSnapshot,
    pub resulting_summary: Option<MemorySummary>,
    pub invalidated_effect_ids: Vec<CompanionEffectId>,
    pub applied_at: TimestampMillis,
}

impl CompanionEffectBackup {
    pub fn canonicalize_and_validate(
        &mut self,
        history: &crate::ConversationHistoryBackup,
        runtime: &crate::ConversationRuntimeBackup,
    ) -> Result<(), CompanionEffectBackupError> {
        if self.version != COMPANION_EFFECT_BACKUP_VERSION
            || self.effects.len() > MAX_BACKUP_COMPANION_EFFECTS
            || self.rewinds.len() > MAX_BACKUP_MEMORY_REWINDS
        {
            return Err(CompanionEffectBackupError::InvalidData);
        }
        self.effects
            .sort_by_key(|effect| (effect.conversation_id, effect.created_at, effect.id));
        self.rewinds
            .sort_by_key(|rewind| (rewind.applied_at, rewind.operation_id));
        let messages = history
            .conversations
            .iter()
            .flat_map(|conversation| {
                conversation.messages.iter().map(|message| {
                    (
                        message.message.id,
                        (conversation.aggregate.conversation.id, message.message.role),
                    )
                })
            })
            .collect::<BTreeMap<_, _>>();
        let turns = runtime
            .conversations
            .iter()
            .flat_map(|conversation| {
                conversation
                    .turns
                    .iter()
                    .map(|turn| (turn.turn.id, conversation.conversation_id))
            })
            .collect::<BTreeMap<_, _>>();
        let conversations = history
            .conversations
            .iter()
            .map(|conversation| conversation.aggregate.conversation.id)
            .collect::<BTreeSet<_>>();
        let mut effects = BTreeMap::new();
        for effect in &self.effects {
            let expected_source_messages = effect
                .user_message_id
                .into_iter()
                .chain(std::iter::once(effect.assistant_message_id))
                .collect::<Vec<_>>();
            if effects.insert(effect.id, effect.conversation_id).is_some()
                || turns.get(&effect.turn_id) != Some(&effect.conversation_id)
                || messages.get(&effect.assistant_message_id)
                    != Some(&(effect.conversation_id, MessageRole::Assistant))
                || effect.user_message_id.is_some_and(|id| {
                    messages.get(&id) != Some(&(effect.conversation_id, MessageRole::User))
                })
                || effect.created_at > effect.updated_at
                || effect.seed.validate().is_err()
                || !valid_effect_terminal(effect)
                || effect.source_window.as_ref().is_some_and(|source| {
                    source.message_ids != expected_source_messages
                        || source.enqueued_at < effect.created_at
                        || source.message_ids.iter().any(|id| {
                            messages.get(id).map(|value| value.0) != Some(effect.conversation_id)
                        })
                })
            {
                return Err(CompanionEffectBackupError::InvalidData);
            }
        }
        let mut rewind_ids = BTreeSet::new();
        let mut invalidated = BTreeSet::new();
        for rewind in &self.rewinds {
            if !rewind_ids.insert(rewind.operation_id)
                || !conversations.contains(&rewind.conversation_id)
                || rewind.request_digest != rewind_digest(rewind)?
                || rewind.resulting_memory.validate().is_err()
                || rewind.resulting_memory.id != rewind.space_id
                || rewind.resulting_memory.revision != rewind.resulting_memory_revision
                || match rewind.invalid_run_id {
                    Some(_) => {
                        rewind.resulting_memory_revision
                            != rewind
                                .source_memory_revision
                                .next()
                                .map_err(|_| CompanionEffectBackupError::InvalidData)?
                    }
                    None => rewind.resulting_memory_revision != rewind.source_memory_revision,
                }
                || rewind.restored_summary_run_id.is_some() && rewind.resulting_summary.is_none()
                || rewind.resulting_summary.as_ref().is_some_and(|summary| {
                    summary.validate().is_err()
                        || summary.space_id != rewind.space_id
                        || summary.updated_at > rewind.applied_at
                        || summary.source_message_ids.iter().any(|id| {
                            messages.get(id).map(|value| value.0) != Some(rewind.conversation_id)
                        })
                })
                || rewind.invalidated_effect_ids.iter().any(|id| {
                    effects.get(id) != Some(&rewind.conversation_id)
                        || self
                            .effects
                            .iter()
                            .find(|effect| effect.id == *id)
                            .is_none_or(|effect| effect.updated_at > rewind.applied_at)
                        || !invalidated.insert(*id)
                })
            {
                return Err(CompanionEffectBackupError::InvalidData);
            }
        }
        if self.effects.iter().any(|effect| {
            (effect.status == CompanionTurnEffectStatus::Invalidated)
                != invalidated.contains(&effect.id)
        }) {
            return Err(CompanionEffectBackupError::InvalidData);
        }
        Ok(())
    }
}

fn rewind_digest(
    rewind: &BackupDynamicMemoryRewind,
) -> Result<ContentHash, CompanionEffectBackupError> {
    #[derive(Serialize)]
    struct VersionedRewind<'a> {
        format_version: u32,
        value: &'a DynamicMemorySuffixRewind,
    }

    let value = DynamicMemorySuffixRewind {
        operation_id: rewind.operation_id,
        conversation_id: rewind.conversation_id,
        invalid_run_id: rewind.invalid_run_id,
        expected_memory_revision: rewind.source_memory_revision,
        invalidated_effect_ids: rewind.invalidated_effect_ids.clone(),
        at: rewind.applied_at,
    };
    let encoded = serde_json::to_vec(&VersionedRewind {
        format_version: 1,
        value: &value,
    })
    .map_err(|_| CompanionEffectBackupError::InvalidData)?;
    ContentHash::parse(blake3::hash(&encoded).to_hex().to_string())
        .map_err(|_| CompanionEffectBackupError::InvalidData)
}

fn valid_effect_terminal(effect: &CompanionTurnEffect) -> bool {
    match effect.status {
        CompanionTurnEffectStatus::Processing => valid_processing_effect(effect),
        CompanionTurnEffectStatus::Ready => valid_ready_effect(effect),
        CompanionTurnEffectStatus::Failed => valid_failed_effect(effect),
        CompanionTurnEffectStatus::Invalidated => {
            valid_processing_effect(effect)
                || valid_ready_effect(effect)
                || valid_failed_effect(effect)
        }
    }
}

fn valid_processing_effect(effect: &CompanionTurnEffect) -> bool {
    effect.summary.is_none()
        && effect.memory_changes == Default::default()
        && effect.source_window.is_none()
}

fn valid_ready_effect(effect: &CompanionTurnEffect) -> bool {
    effect.source_window.clone().is_some_and(|source_window| {
        CompanionTurnEffectOutcome::Ready {
            summary: effect.summary.clone(),
            memory_changes: effect.memory_changes.clone(),
            source_window,
        }
        .validate()
        .is_ok()
    })
}

fn valid_failed_effect(effect: &CompanionTurnEffect) -> bool {
    effect.summary.clone().is_some_and(|summary| {
        effect.memory_changes == Default::default()
            && effect.source_window.is_none()
            && CompanionTurnEffectOutcome::Failed { summary }
                .validate()
                .is_ok()
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CompanionEffectBackupError {
    #[error("companion effect backup is invalid")]
    InvalidData,
}
