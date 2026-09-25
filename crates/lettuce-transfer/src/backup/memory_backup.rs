use std::collections::{BTreeMap, BTreeSet};

use lettuce_memory::{MemoryRetrievalAccessReceipt, MemorySpaceSnapshot, MemorySummary};
use lettuce_types::ConversationId;
use serde::{Deserialize, Serialize};

pub const MEMORY_BACKUP_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryBackup {
    pub version: u32,
    pub spaces: Vec<BackupMemorySpace>,
    pub retrieval_accesses: Vec<MemoryRetrievalAccessReceipt>,
    /// The companion character that owns each shared memory pool.
    #[serde(default)]
    pub pools: Vec<BackupCompanionMemoryPool>,
    /// Pool spaces no conversation is bound to yet or anymore.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unbound_pools: Vec<MemorySpaceSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupCompanionMemoryPool {
    pub character_id: lettuce_types::CharacterId,
    pub space_id: lettuce_types::MemorySpaceId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupMemorySpace {
    pub conversation_id: ConversationId,
    pub snapshot: MemorySpaceSnapshot,
    pub summary: Option<MemorySummary>,
    /// Other conversations bound to the same space: a companion memory pool
    /// is shared by the companion's conversations; `conversation_id` owns the
    /// summary.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shared_conversation_ids: Vec<ConversationId>,
}

impl MemoryBackup {
    pub fn canonicalize_and_validate(
        &mut self,
        history: &crate::ConversationHistoryBackup,
        runtime: &crate::ConversationRuntimeBackup,
        effects: &crate::CompanionEffectBackup,
    ) -> Result<(), MemoryBackupError> {
        if self.version != MEMORY_BACKUP_VERSION {
            return Err(MemoryBackupError::InvalidData);
        }
        self.spaces
            .sort_by_key(|space| (space.conversation_id, space.snapshot.id));
        self.pools.sort_by_key(|pool| pool.character_id);
        self.unbound_pools.sort_by_key(|space| space.id);
        let space_ids = self
            .spaces
            .iter()
            .map(|space| space.snapshot.id)
            .chain(self.unbound_pools.iter().map(|space| space.id))
            .collect::<BTreeSet<_>>();
        if space_ids.len() != self.spaces.len() + self.unbound_pools.len() {
            return Err(MemoryBackupError::InvalidData);
        }
        let mut pool_characters = BTreeSet::new();
        let mut pool_spaces = BTreeSet::new();
        if self.pools.iter().any(|pool| {
            !space_ids.contains(&pool.space_id)
                || !pool_characters.insert(pool.character_id)
                || !pool_spaces.insert(pool.space_id)
        }) {
            return Err(MemoryBackupError::InvalidData);
        }
        self.retrieval_accesses.sort_by_key(|receipt| {
            (
                receipt.access.conversation_id,
                receipt.access.turn_id,
                receipt.access.attempt_id,
            )
        });
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
        let attempts = runtime
            .conversations
            .iter()
            .flat_map(|conversation| {
                conversation.turns.iter().flat_map(|turn| {
                    turn.turn
                        .attempts
                        .iter()
                        .map(|attempt| ((turn.turn.id, attempt.id), conversation.conversation_id))
                })
            })
            .collect::<BTreeMap<_, _>>();
        let known_conversations = history
            .conversations
            .iter()
            .map(|value| value.aggregate.conversation.id)
            .collect::<BTreeSet<_>>();
        let mut spaces = BTreeMap::new();
        let mut space_ids = BTreeSet::new();
        let mut memory_owners = BTreeMap::new();
        for space in &self.spaces {
            let bound = std::iter::once(space.conversation_id)
                .chain(space.shared_conversation_ids.iter().copied())
                .collect::<Vec<_>>();
            let pool = pool_spaces.contains(&space.snapshot.id);
            if bound.iter().any(|id| !known_conversations.contains(id))
                || space.snapshot.validate().is_err()
                || (!pool && !space.shared_conversation_ids.is_empty())
                || bound
                    .iter()
                    .any(|id| spaces.insert((*id, pool), space.snapshot.id).is_some())
                || !space_ids.insert(space.snapshot.id)
                || space.snapshot.items.iter().any(|item| {
                    memory_owners.insert(item.id, space.snapshot.id).is_some()
                        || item.source_message_id.is_some_and(|id| {
                            messages.get(&id).is_none_or(|(conversation_id, role)| {
                                !bound.contains(conversation_id)
                                    || item
                                        .source_role
                                        .is_some_and(|source_role| source_role != *role)
                            })
                        })
                })
                || space.summary.as_ref().is_some_and(|summary| {
                    summary.validate().is_err()
                        || summary.space_id != space.snapshot.id
                        || summary.source_message_ids.iter().any(|id| {
                            messages.get(id).map(|value| value.0) != Some(space.conversation_id)
                        })
                })
            {
                return Err(MemoryBackupError::InvalidData);
            }
        }
        for space in &self.unbound_pools {
            if !pool_spaces.contains(&space.id)
                || space.validate().is_err()
                || space
                    .items
                    .iter()
                    .any(|item| memory_owners.insert(item.id, space.id).is_some())
            {
                return Err(MemoryBackupError::InvalidData);
            }
        }
        for space in self
            .spaces
            .iter()
            .map(|space| &space.snapshot)
            .chain(&self.unbound_pools)
        {
            if space.items.iter().any(|item| {
                item.superseded_by
                    .into_iter()
                    .chain(item.supersedes.iter().copied())
                    .any(|id| {
                        memory_owners
                            .get(&id)
                            .is_some_and(|owner| *owner != space.id)
                    })
                    || item
                        .supersedes
                        .iter()
                        .copied()
                        .collect::<BTreeSet<_>>()
                        .len()
                        != item.supersedes.len()
            }) {
                return Err(MemoryBackupError::InvalidData);
            }
        }
        let binds = |conversation_id: ConversationId, space_id: lettuce_types::MemorySpaceId| {
            [false, true]
                .iter()
                .any(|pool| spaces.get(&(conversation_id, *pool)) == Some(&space_id))
        };
        let mut access_owners = BTreeSet::new();
        for receipt in &self.retrieval_accesses {
            let access = &receipt.access;
            if !access_owners.insert((access.conversation_id, access.turn_id, access.attempt_id))
                || attempts.get(&(access.turn_id, access.attempt_id))
                    != Some(&access.conversation_id)
                || !binds(access.conversation_id, access.space_id)
                || access.selected_memory_ids.is_empty()
                || access.selected_memory_ids.len() > 4096
                || access
                    .selected_memory_ids
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>()
                    .len()
                    != access.selected_memory_ids.len()
                || receipt.resulting_revision < access.expected_revision
                || self
                    .spaces
                    .iter()
                    .find(|space| space.snapshot.id == access.space_id)
                    .is_none_or(|space| space.snapshot.revision < receipt.resulting_revision)
                || access.selected_memory_ids.iter().any(|id| {
                    memory_owners
                        .get(id)
                        .is_some_and(|space_id| *space_id != access.space_id)
                })
                || receipt
                    .promoted_memory_ids
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>()
                    .len()
                    != receipt.promoted_memory_ids.len()
                || receipt
                    .promoted_memory_ids
                    .iter()
                    .any(|id| !access.selected_memory_ids.contains(id))
            {
                return Err(MemoryBackupError::InvalidData);
            }
        }
        for effect in &effects.effects {
            let bound = [false, true]
                .iter()
                .filter_map(|pool| spaces.get(&(effect.conversation_id, *pool)))
                .collect::<Vec<_>>();
            if bound.is_empty() {
                if effect.memory_changes == Default::default() {
                    continue;
                }
                return Err(MemoryBackupError::InvalidData);
            }
            if effect
                .memory_changes
                .added
                .iter()
                .chain(&effect.memory_changes.updated)
                .chain(&effect.memory_changes.superseded)
                .any(|id| {
                    memory_owners
                        .get(id)
                        .is_some_and(|owner| !bound.contains(&owner))
                })
            {
                return Err(MemoryBackupError::InvalidData);
            }
        }
        for rewind in &effects.rewinds {
            if !binds(rewind.conversation_id, rewind.space_id)
                || rewind.resulting_memory.items.iter().any(|item| {
                    memory_owners
                        .get(&item.id)
                        .is_some_and(|owner| *owner != rewind.space_id)
                })
            {
                return Err(MemoryBackupError::InvalidData);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MemoryBackupError {
    #[error("memory backup is invalid")]
    InvalidData,
}
