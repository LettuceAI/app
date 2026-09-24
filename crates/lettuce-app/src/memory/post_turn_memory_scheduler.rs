use std::collections::BTreeMap;
use std::sync::Mutex;

use lettuce_types::ConversationId;

#[derive(Debug, Clone, Copy, Default)]
struct ConversationQueue {
    active: bool,
    dirty: bool,
}

/// Coalesces post-turn memory cycles per conversation: while one runs, later
/// turns only ask for one more pass after it.
#[derive(Debug, Default)]
pub struct PostTurnMemoryScheduler {
    queues: Mutex<BTreeMap<ConversationId, ConversationQueue>>,
}

impl PostTurnMemoryScheduler {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true when the caller must start driving the conversation.
    pub fn enqueue(&self, conversation_id: ConversationId) -> bool {
        let mut queues = self.lock();
        let queue = queues.entry(conversation_id).or_default();
        queue.dirty = true;
        if queue.active {
            return false;
        }
        queue.active = true;
        true
    }

    pub(crate) fn begin(&self, conversation_id: ConversationId) {
        if let Some(queue) = self.lock().get_mut(&conversation_id) {
            queue.dirty = false;
        }
    }

    /// Returns true when another pass is due.
    pub(crate) fn finish(&self, conversation_id: ConversationId) -> bool {
        let mut queues = self.lock();
        match queues.get(&conversation_id) {
            Some(queue) if queue.dirty => true,
            _ => {
                queues.remove(&conversation_id);
                false
            }
        }
    }

    pub(crate) fn stop(&self, conversation_id: ConversationId) {
        self.lock().remove(&conversation_id);
    }

    #[must_use]
    pub fn is_active(&self, conversation_id: ConversationId) -> bool {
        self.lock()
            .get(&conversation_id)
            .is_some_and(|queue| queue.active)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<ConversationId, ConversationQueue>> {
        self.queues
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Clears the conversation's queue unless the drive finished normally.
pub(crate) struct DriveGuard<'a> {
    scheduler: &'a PostTurnMemoryScheduler,
    conversation_id: ConversationId,
    released: bool,
}

impl<'a> DriveGuard<'a> {
    pub(crate) fn new(
        scheduler: &'a PostTurnMemoryScheduler,
        conversation_id: ConversationId,
    ) -> Self {
        Self {
            scheduler,
            conversation_id,
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
            self.scheduler.stop(self.conversation_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turns_during_a_pass_coalesce_into_one_more_pass() {
        let scheduler = PostTurnMemoryScheduler::new();
        let conversation = ConversationId::new();
        assert!(scheduler.enqueue(conversation));
        scheduler.begin(conversation);
        assert!(!scheduler.enqueue(conversation));
        assert!(!scheduler.enqueue(conversation));
        assert!(scheduler.finish(conversation));
        scheduler.begin(conversation);
        assert!(!scheduler.finish(conversation));
        assert!(!scheduler.is_active(conversation));
        assert!(scheduler.enqueue(conversation));
        scheduler.stop(conversation);
        assert!(scheduler.enqueue(conversation));
    }
}
