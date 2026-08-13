//! Recovery result vocabulary. Recovery decisions remain domain-owned; this
//! crate only records the lifecycle facts needed to make one safely.
//!
//! `RecoveryAction::Compensate` is a request for an app-owned follow-up
//! operation. Lease expiry records the job as [`crate::JobState::Interrupted`] before
//! returning that action; it never claims that domain compensation completed.

use crate::{ExpiredClaim, RecoveryPolicy};
use lettuce_types::JobId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    Resume,
    Restart,
    Compensate,
    MarkInterrupted,
}

impl From<RecoveryPolicy> for RecoveryAction {
    fn from(policy: RecoveryPolicy) -> Self {
        match policy {
            RecoveryPolicy::Resume => Self::Resume,
            RecoveryPolicy::Restart => Self::Restart,
            RecoveryPolicy::Compensate => Self::Compensate,
            RecoveryPolicy::MarkInterrupted => Self::MarkInterrupted,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryReport {
    pub expired: Vec<ExpiredClaim>,
    pub resumed: Vec<JobId>,
    pub restarted: Vec<JobId>,
    pub interrupted: Vec<JobId>,
}
