//! Immutable usage ledger contracts and legacy request cost calculation.

#![deny(unsafe_op_in_unsafe_fn)]

mod openrouter;
pub use openrouter::*;
mod pricing;
pub use pricing::*;
mod costing;
pub use costing::*;
mod job_usage;
pub use job_usage::*;

use lettuce_conversations::UsageRecord;
use lettuce_types::{GenerationAttemptId, GenerationTurnId, UsageEventId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageEvent {
    pub id: UsageEventId,
    pub record: UsageRecord,
}

pub trait UsageLedger: Send + Sync {
    /// Appends one immutable terminal event. An exact retry for the same
    /// turn/attempt returns the original event; changed evidence conflicts.
    fn record(&self, record: UsageRecord) -> Result<UsageEvent, UsageLedgerError>;

    fn get(&self, id: UsageEventId) -> Result<Option<UsageEvent>, UsageLedgerError>;

    fn get_for_attempt(
        &self,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
    ) -> Result<Option<UsageEvent>, UsageLedgerError>;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UsageLedgerError {
    #[error("usage event is invalid")]
    Invalid,
    #[error("usage event conflicts with immutable evidence")]
    Conflict,
    #[error("usage ledger storage failed")]
    Storage,
}
