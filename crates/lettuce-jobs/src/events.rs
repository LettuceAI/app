//! Stable, safe event vocabulary. Event payloads contain typed references and
//! counters only; prompts, paths, provider bodies and arbitrary JSON have no
//! representation here.

use crate::{
    AttemptNo, CancellationReason, ChildLink, CorrelationId, EventSeq, JobError, JobKind,
    JobOutcome, JobPriority, JobSubject, LeaseId, ProgressSnapshot, StageSnapshot, Timestamp,
    WorkerId,
};
use lettuce_types::JobId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum JobEvent {
    Created {
        kind: JobKind,
        subject: JobSubject,
        input_ref: crate::OutcomeRef,
        parent_id: Option<JobId>,
        priority: JobPriority,
    },
    Queued,
    Started,
    Claimed {
        worker_id: WorkerId,
        attempt: AttemptNo,
        lease_id: LeaseId,
        lease_expires_at: Timestamp,
        input_ref: crate::OutcomeRef,
        recovery_policy: crate::RecoveryPolicy,
        cancellation_policy: crate::CancellationPolicy,
        resources: Vec<crate::ResourceClass>,
    },
    StageChanged {
        stage: StageSnapshot,
    },
    Progressed {
        progress: ProgressSnapshot,
    },
    CancellationRequested {
        reason: CancellationReason,
    },
    CleanupStarted,
    Checkpointed {
        checkpoint: crate::OutcomeRef,
    },
    RetryScheduled {
        attempt: AttemptNo,
    },
    ChildAttached {
        child: ChildLink,
    },
    Succeeded {
        outcome: JobOutcome,
    },
    PartiallySucceeded {
        outcome: JobOutcome,
    },
    Failed {
        error: JobError,
    },
    Cancelled,
    Interrupted,
    LeaseExpired {
        lease_id: LeaseId,
    },
    RecoveryStarted,
    RecoveryResolved,
    RetentionPruned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobEventEnvelope {
    pub job_id: JobId,
    pub seq: EventSeq,
    pub at: Timestamp,
    pub correlation_id: CorrelationId,
    pub event: JobEvent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventCursor {
    pub after: Option<EventSeq>,
    pub limit: u32,
}

impl Default for EventCursor {
    fn default() -> Self {
        Self {
            after: None,
            limit: 200,
        }
    }
}

/// A safe reason that can be surfaced by adapters without accepting free-form
/// user text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventNote {
    StateChanged,
    WorkerRecovered,
    CleanupCompleted,
}
