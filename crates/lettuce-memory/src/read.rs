use lettuce_types::{ConversationBranchId, ConversationId, Revision};

use crate::{
    DynamicMemoryAttempt, DynamicMemoryPendingApproval, DynamicMemoryRun, MemoryRepositoryError,
    MemorySpaceSnapshot, MemorySummary, MemoryToolResult,
};

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryActivityCycle {
    pub run: DynamicMemoryRun,
    pub latest_attempt: DynamicMemoryAttempt,
    pub results: Vec<MemoryToolResult>,
    pub reverted: bool,
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
}

pub trait MemoryReadRepository: Send + Sync {
    fn read_memory_scope(
        &self,
        conversation_id: ConversationId,
        conversation_revision: Revision,
    ) -> Result<MemoryReadScope, MemoryRepositoryError>;
}
