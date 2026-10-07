use lettuce_types::{ConversationBranchId, ConversationId, DynamicMemoryRunId, JobId, Revision};

use crate::{
    DynamicMemoryAttempt, DynamicMemoryAttemptStatus, DynamicMemoryPendingApproval,
    DynamicMemoryRun, MemoryRepositoryError, MemorySpaceSnapshot, MemorySummary, MemoryToolResult,
};

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryActivityCycle {
    pub run: DynamicMemoryRun,
    pub latest_attempt: DynamicMemoryAttempt,
    pub results: Vec<MemoryToolResult>,
    pub checkpoint: Option<MemorySummary>,
    pub reverted: bool,
}

impl MemoryActivityCycle {
    #[must_use]
    pub fn in_flight(&self) -> bool {
        matches!(
            self.latest_attempt.status,
            DynamicMemoryAttemptStatus::Created | DynamicMemoryAttemptStatus::Processing
        )
    }

    /// Whether the cycle changed memory or published a summary.
    #[must_use]
    pub fn recorded(&self) -> bool {
        !self.results.is_empty() || self.checkpoint.is_some()
    }
}

/// The earliest later cycle that started from `run_id`'s result, which keeps
/// the cycle from being reverted. `cycles` is in creation order.
#[must_use]
pub fn cycle_revert_blocker(
    cycles: &[MemoryActivityCycle],
    run_id: DynamicMemoryRunId,
) -> Option<DynamicMemoryRunId> {
    let position = cycles.iter().position(|cycle| cycle.run.id == run_id)?;
    cycles[position + 1..]
        .iter()
        .find(|cycle| !cycle.reverted && (cycle.in_flight() || cycle.recorded()))
        .map(|cycle| cycle.run.id)
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryReadScope {
    pub conversation_id: ConversationId,
    pub branch_id: ConversationBranchId,
    pub memory: MemorySpaceSnapshot,
    pub pooled: bool,
    pub summary: Option<MemorySummary>,
    pub message_count: u64,
    pub summary_cursor: u64,
    pub approval: Option<DynamicMemoryPendingApproval>,
    pub cycles: Vec<MemoryActivityCycle>,
    pub space_conversations: Vec<ConversationId>,
    pub dismissed_job: Option<JobId>,
}

pub trait MemoryReadRepository: Send + Sync {
    fn read_memory_scope(
        &self,
        conversation_id: ConversationId,
        conversation_revision: Revision,
    ) -> Result<MemoryReadScope, MemoryRepositoryError>;
}
