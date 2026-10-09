use lettuce_types::{GenerationAttemptId, JobId, UsageEventId};
use serde::{Deserialize, Serialize};

use crate::UsageLedgerError;

pub const USAGE_AUTO_COST_KEY_PREFIX: &str = "usage-auto-cost:";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsageCostTarget {
    pub event_id: UsageEventId,
    pub attempt_id: GenerationAttemptId,
    pub job_id: JobId,
}

#[derive(Debug, Clone, Copy)]
pub enum UsageCostScope {
    Recalculate,
    Automatic { job_id: Option<JobId> },
}

pub trait UsageCostTargetReader: Send + Sync {
    fn missing_cost_targets(
        &self,
        scope: UsageCostScope,
    ) -> Result<Vec<UsageCostTarget>, UsageLedgerError>;

    fn cleared_cost_target(&self, target: &UsageCostTarget) -> Result<bool, UsageLedgerError>;
}
