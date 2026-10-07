use std::collections::BTreeMap;
use std::sync::Mutex;

use lettuce_types::{ConversationBranchId, ConversationId};

#[derive(Debug, Clone, Copy, Default)]
struct ConversationQueue {
    active: bool,
    dirty: bool,
}

/// Coalesces post-turn memory cycles per branch while keeping different
/// branches independent.
#[derive(Debug, Default)]
pub struct PostTurnMemoryScheduler {
    queues: Mutex<BTreeMap<(ConversationId, ConversationBranchId), ConversationQueue>>,
}

impl PostTurnMemoryScheduler {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true when the caller must start driving the conversation.
    pub fn enqueue(
        &self,
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
    ) -> bool {
        let mut queues = self.lock();
        let queue = queues.entry((conversation_id, branch_id)).or_default();
        queue.dirty = true;
        if queue.active {
            return false;
        }
        queue.active = true;
        true
    }

    pub(crate) fn begin(&self, conversation_id: ConversationId, branch_id: ConversationBranchId) {
        if let Some(queue) = self.lock().get_mut(&(conversation_id, branch_id)) {
            queue.dirty = false;
        }
    }

    /// Returns true when another pass is due.
    pub(crate) fn finish(
        &self,
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
    ) -> bool {
        let mut queues = self.lock();
        match queues.get(&(conversation_id, branch_id)) {
            Some(queue) if queue.dirty => true,
            _ => {
                queues.remove(&(conversation_id, branch_id));
                false
            }
        }
    }

    pub(crate) fn stop(&self, conversation_id: ConversationId, branch_id: ConversationBranchId) {
        self.lock().remove(&(conversation_id, branch_id));
    }

    #[must_use]
    pub fn is_active(
        &self,
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
    ) -> bool {
        self.lock()
            .get(&(conversation_id, branch_id))
            .is_some_and(|queue| queue.active)
    }

    fn lock(
        &self,
    ) -> std::sync::MutexGuard<
        '_,
        BTreeMap<(ConversationId, ConversationBranchId), ConversationQueue>,
    > {
        self.queues
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Clears the conversation's queue unless the drive finished normally.
pub(crate) struct DriveGuard<'a> {
    scheduler: &'a PostTurnMemoryScheduler,
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
    released: bool,
}

impl<'a> DriveGuard<'a> {
    pub(crate) fn new(
        scheduler: &'a PostTurnMemoryScheduler,
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
    ) -> Self {
        Self {
            scheduler,
            conversation_id,
            branch_id,
            released: false,
        }
    }

    pub(crate) fn release(mut self) {
        self.released = true;
    }
}

impl Drop for DriveGuard<'_> {
    fn drop(&mut self) {
        if !self.released {
            self.scheduler.stop(self.conversation_id, self.branch_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branches_keep_separate_queued_passes() {
        let scheduler = PostTurnMemoryScheduler::new();
        let conversation = ConversationId::new();
        let parent = ConversationBranchId::new();
        let child = ConversationBranchId::new();
        assert!(scheduler.enqueue(conversation, parent));
        scheduler.begin(conversation, parent);
        assert!(scheduler.enqueue(conversation, child));
        assert!(!scheduler.finish(conversation, parent));
        assert!(scheduler.is_active(conversation, child));
        scheduler.begin(conversation, child);
        assert!(!scheduler.finish(conversation, child));
    }

    #[test]
    fn turns_during_a_pass_coalesce_into_one_more_pass() {
        let scheduler = PostTurnMemoryScheduler::new();
        let conversation = ConversationId::new();
        let branch = ConversationBranchId::new();
        assert!(scheduler.enqueue(conversation, branch));
        scheduler.begin(conversation, branch);
        assert!(!scheduler.enqueue(conversation, branch));
        assert!(!scheduler.enqueue(conversation, branch));
        assert!(scheduler.finish(conversation, branch));
        scheduler.begin(conversation, branch);
        assert!(!scheduler.finish(conversation, branch));
        assert!(!scheduler.is_active(conversation, branch));
        assert!(scheduler.enqueue(conversation, branch));
        scheduler.stop(conversation, branch);
        assert!(scheduler.enqueue(conversation, branch));
    }
}
