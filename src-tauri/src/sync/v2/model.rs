use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub type Frontier = BTreeMap<String, i64>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct HybridTimestamp {
    pub wall_time_ms: i64,
    pub counter: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeRevision {
    pub change_id: String,
    pub origin_device_id: String,
    pub origin_sequence: i64,
    pub timestamp: HybridTimestamp,
    pub base_frontier: Frontier,
    pub schema_fingerprint: String,
    pub changeset_hash: String,
    pub changeset: Vec<u8>,
}

impl ChangeRevision {
    pub fn observes(&self, other: &Self) -> bool {
        self.origin_device_id == other.origin_device_id
            && self.origin_sequence > other.origin_sequence
            || self
                .base_frontier
                .get(&other.origin_device_id)
                .copied()
                .unwrap_or(0)
                >= other.origin_sequence
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyOutcome {
    Applied,
    Duplicate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyResult {
    pub outcome: ApplyOutcome,
    pub conflicts_created: usize,
    pub branches_created: usize,
}
