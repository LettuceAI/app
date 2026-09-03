use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex},
    time::Duration,
};

use lettuce_types::{JobId, Page, PageRequest, TimestampMillis};

use crate::{
    AttemptNo, CancellationPolicy, CancellationReason, CancellationView, ChildLink, Claim,
    ClaimRef, EventSeq, JobError, JobKind, JobOutcome, JobSnapshot, JobSpec, JobState, NewJob,
    OutcomeRef, ProgressSnapshot, ResourceAvailability, SpecError, StageSnapshot, SubjectId,
    Timestamp, WorkerId,
    events::{JobEvent, JobEventEnvelope},
    timestamp_after,
};

/// Clock is injectable so lifecycle tests never depend on wall-clock timing.
pub trait Clock: Send + Sync + std::fmt::Debug {
    fn now(&self) -> Timestamp;
}

#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        TimestampMillis::now().unwrap_or(TimestampMillis::UNIX_EPOCH)
    }
}

#[derive(Debug, Clone)]
pub struct FakeClock {
    now: Arc<Mutex<Timestamp>>,
}

impl FakeClock {
    #[must_use]
    pub fn new(now: Timestamp) -> Self {
        Self {
            now: Arc::new(Mutex::new(now)),
        }
    }

    pub fn set(&self, now: Timestamp) {
        *self
            .now
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = now;
    }

    pub fn advance(&self, duration: Duration) {
        let current = self.now();
        let next = timestamp_after(current, duration).expect("test clock arithmetic is valid");
        self.set(next);
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Timestamp {
        *self
            .now
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    #[error("job storage operation failed")]
    Storage,
    #[error("stored job data is invalid")]
    InvalidData,
    #[error("job was not found")]
    NotFound,
    #[error("idempotency key conflicts with a different submission")]
    IdempotencyConflict,
    #[error("parent job was not found")]
    ParentNotFound,
    #[error("parent job is terminal")]
    ParentTerminal,
    #[error("job has no eligible resource claim")]
    ResourceUnavailable,
    #[error("job has no active claim")]
    NotClaimed,
    #[error("worker lease is stale or does not own this job")]
    StaleLease,
    #[error("lease has expired")]
    LeaseExpired,
    #[error("job is already terminal")]
    AlreadyTerminal,
    #[error("illegal state transition")]
    IllegalTransition,
    #[error("cancellation is too late for the current irreversible stage")]
    TooLate,
    #[error("job cannot be cancelled by its policy")]
    NotCancellable,
    #[error("progress is invalid or regressed")]
    InvalidProgress,
    #[error("event sequence overflow")]
    SequenceOverflow,
    #[error("attempt number overflow")]
    AttemptOverflow,
    #[error("timestamp arithmetic overflow")]
    TimeOverflow,
    #[error("invalid page cursor")]
    InvalidCursor,
    #[error("page or event limit is invalid")]
    InvalidLimit,
    #[error("required child jobs are not complete")]
    ChildrenIncomplete,
    #[error("job specification is invalid")]
    InvalidSpec(#[from] SpecError),
    #[error("lease duration must be greater than zero")]
    InvalidLeaseDuration,
    #[error("mutation timestamp is older than the job snapshot")]
    TimestampRegression,
    #[error("outcome variant does not match the terminal mutation")]
    InvalidOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateJobResult {
    pub job: JobSnapshot,
    pub created: bool,
}

impl CreateJobResult {
    #[must_use]
    pub fn is_new(&self) -> bool {
        self.created
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JobQuery {
    pub state: Option<JobState>,
    pub kind: Option<JobKind>,
    pub subject: Option<SubjectId>,
    pub page: PageRequest,
}

impl JobQuery {
    #[must_use]
    pub fn page(page: PageRequest) -> Self {
        Self {
            page,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone)]
struct JobRecord {
    spec: JobSpec,
    snapshot: JobSnapshot,
    events: Vec<JobEventEnvelope>,
}

/// Persistence-neutral aggregate used by durable [`JobStore`] adapters.
/// Lifecycle mutations must still be applied through [`InMemoryJobStore`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredJobRecord {
    pub spec: JobSpec,
    pub snapshot: JobSnapshot,
    pub events: Vec<JobEventEnvelope>,
}

#[derive(Debug)]
struct StoreInner {
    jobs: BTreeMap<JobId, JobRecord>,
    idempotency: HashMap<crate::IdempotencyKey, JobId>,
    clock: Arc<dyn Clock>,
}

/// Deterministic, thread-safe reference store. The mutex is deliberately held
/// across each mutation so the snapshot and its matching event are atomic.
#[derive(Debug, Clone)]
pub struct InMemoryJobStore {
    inner: Arc<Mutex<StoreInner>>,
}

impl Default for InMemoryJobStore {
    fn default() -> Self {
        Self::with_clock(Arc::new(SystemClock))
    }
}

impl InMemoryJobStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_clock(clock: Arc<dyn Clock>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(StoreInner {
                jobs: BTreeMap::new(),
                idempotency: HashMap::new(),
                clock,
            })),
        }
    }

    pub fn restore(records: Vec<StoredJobRecord>) -> Result<Self, StoreError> {
        let mut jobs = BTreeMap::new();
        let mut idempotency = HashMap::new();
        for stored in records {
            stored.spec.validate()?;
            if stored.snapshot.kind != stored.spec.kind
                || stored.snapshot.subject != stored.spec.subject
                || stored.snapshot.idempotency_key != stored.spec.idempotency_key
                || stored.snapshot.parent_id != stored.spec.parent_id
                || stored.snapshot.recovery_policy != stored.spec.recovery_policy
                || stored.snapshot.cancellation_policy != stored.spec.cancellation_policy
                || stored.snapshot.resources != stored.spec.resources
                || stored.events.is_empty()
            {
                return Err(StoreError::InvalidData);
            }
            for (index, event) in stored.events.iter().enumerate() {
                let expected = u64::try_from(index)
                    .ok()
                    .and_then(|value| value.checked_add(1))
                    .ok_or(StoreError::InvalidData)?;
                if event.job_id != stored.snapshot.id
                    || event.seq != EventSeq::new(expected)
                    || event.correlation_id != stored.spec.correlation_id
                    || event.at < stored.snapshot.created_at
                    || event.at > stored.snapshot.updated_at
                {
                    return Err(StoreError::InvalidData);
                }
            }
            let id = stored.snapshot.id;
            if jobs
                .insert(
                    id,
                    JobRecord {
                        spec: stored.spec.clone(),
                        snapshot: stored.snapshot,
                        events: stored.events,
                    },
                )
                .is_some()
            {
                return Err(StoreError::InvalidData);
            }
            if let Some(key) = stored.spec.idempotency_key
                && idempotency.insert(key, id).is_some()
            {
                return Err(StoreError::InvalidData);
            }
        }
        if jobs.values().any(|record| {
            record
                .snapshot
                .parent_id
                .is_some_and(|parent_id| !jobs.contains_key(&parent_id))
                || record
                    .snapshot
                    .children
                    .iter()
                    .any(|child| !jobs.contains_key(&child.child_id))
        }) {
            return Err(StoreError::InvalidData);
        }
        Ok(Self {
            inner: Arc::new(Mutex::new(StoreInner {
                jobs,
                idempotency,
                clock: Arc::new(SystemClock),
            })),
        })
    }

    #[must_use]
    pub fn stored_records(&self) -> Vec<StoredJobRecord> {
        self.lock()
            .jobs
            .values()
            .map(|record| StoredJobRecord {
                spec: record.spec.clone(),
                snapshot: record.snapshot.clone(),
                events: record.events.clone(),
            })
            .collect()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, StoreInner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn append_event(
        record: &mut JobRecord,
        event: JobEvent,
        at: Timestamp,
    ) -> Result<(), StoreError> {
        if at < record.snapshot.updated_at {
            return Err(StoreError::TimestampRegression);
        }
        let seq = match record.events.last() {
            Some(last) => last.seq.next().map_err(|_| StoreError::SequenceOverflow)?,
            None => EventSeq::FIRST,
        };
        record.events.push(JobEventEnvelope {
            job_id: record.snapshot.id,
            seq,
            at,
            correlation_id: record.spec.correlation_id,
            event,
        });
        record.snapshot.updated_at = at;
        Ok(())
    }

    fn transition(record: &mut JobRecord, next: JobState) -> Result<(), StoreError> {
        record
            .snapshot
            .state
            .can_transition_to(next)
            .map_err(|_| StoreError::IllegalTransition)?;
        record.snapshot.state = next;
        Ok(())
    }

    fn check_timestamp(record: &JobRecord, at: Timestamp) -> Result<(), StoreError> {
        if at < record.snapshot.updated_at {
            return Err(StoreError::TimestampRegression);
        }
        Ok(())
    }

    fn claim_matches(
        record: &JobRecord,
        claim: &ClaimRef,
        at: Timestamp,
    ) -> Result<(), StoreError> {
        if record.snapshot.claim.as_ref() != Some(claim) {
            return Err(StoreError::StaleLease);
        }
        if record.snapshot.state == JobState::Queued {
            return Err(StoreError::NotClaimed);
        }
        Self::check_timestamp(record, at)?;
        if record
            .snapshot
            .lease_expires_at
            .is_some_and(|expires_at| at > expires_at)
        {
            return Err(StoreError::LeaseExpired);
        }
        Ok(())
    }

    fn ensure_children_complete(inner: &StoreInner, record: &JobRecord) -> Result<(), StoreError> {
        for child in &record.snapshot.children {
            if child.required
                && inner
                    .jobs
                    .get(&child.child_id)
                    .is_none_or(|job| !matches!(job.snapshot.state, JobState::Succeeded))
            {
                return Err(StoreError::ChildrenIncomplete);
            }
        }
        Ok(())
    }

    fn finish_terminal(
        record: &mut JobRecord,
        state: JobState,
        event: JobEvent,
        at: Timestamp,
    ) -> Result<(), StoreError> {
        if record.snapshot.state.is_terminal() {
            return Err(StoreError::AlreadyTerminal);
        }
        Self::transition(record, state)?;
        record.snapshot.claim = None;
        record.snapshot.lease_expires_at = None;
        Self::append_event(record, event, at)
    }

    pub fn create_or_get(&self, spec: NewJob) -> Result<CreateJobResult, StoreError> {
        spec.validate()?;
        let mut inner = self.lock();
        if let Some(key) = &spec.idempotency_key {
            if let Some(existing_id) = inner.idempotency.get(key).copied() {
                let existing = inner.jobs.get(&existing_id).ok_or(StoreError::NotFound)?;
                if !same_submission(&existing.spec, &spec) {
                    return Err(StoreError::IdempotencyConflict);
                }
                return Ok(CreateJobResult {
                    job: existing.snapshot.clone(),
                    created: false,
                });
            }
        }

        let parent_id = spec.parent_id;
        if let Some(parent_id) = parent_id {
            let parent = inner
                .jobs
                .get(&parent_id)
                .ok_or(StoreError::ParentNotFound)?;
            if parent.snapshot.is_terminal() {
                return Err(StoreError::ParentTerminal);
            }
        }
        let now = inner.clock.now();
        let id = JobId::new();
        let snapshot = JobSnapshot {
            id,
            kind: spec.kind,
            subject: spec.subject.clone(),
            idempotency_key: spec.idempotency_key.clone(),
            state: JobState::Queued,
            stage: StageSnapshot::default(),
            progress: ProgressSnapshot::default(),
            attempt: AttemptNo::QUEUED,
            recovery_policy: spec.recovery_policy,
            cancellation_policy: spec.cancellation_policy,
            resources: spec.resources.clone(),
            cancellation: CancellationView::default(),
            outcome: None,
            error: None,
            parent_id,
            children: Vec::new(),
            claim: None,
            lease_expires_at: None,
            checkpoint_ref: None,
            created_at: now,
            updated_at: now,
        };
        let mut record = JobRecord {
            spec: spec.clone(),
            snapshot,
            events: Vec::new(),
        };
        Self::append_event(
            &mut record,
            JobEvent::Created {
                kind: spec.kind,
                subject: spec.subject,
                input_ref: spec.input_ref,
                parent_id,
                priority: spec.priority,
            },
            now,
        )?;
        Self::append_event(&mut record, JobEvent::Queued, now)?;
        let created = record.snapshot.clone();
        inner.jobs.insert(id, record);
        if let Some(key) = spec.idempotency_key {
            inner.idempotency.insert(key, id);
        }
        if let Some(parent_id) = parent_id {
            let child = ChildLink {
                child_id: id,
                required: true,
            };
            let parent = inner
                .jobs
                .get_mut(&parent_id)
                .ok_or(StoreError::ParentNotFound)?;
            parent.snapshot.children.push(child.clone());
            Self::append_event(parent, JobEvent::ChildAttached { child }, now)?;
        }
        Ok(CreateJobResult {
            job: created,
            created: true,
        })
    }

    pub fn get(&self, id: JobId) -> Result<Option<JobSnapshot>, StoreError> {
        Ok(self
            .lock()
            .jobs
            .get(&id)
            .map(|record| record.snapshot.clone()))
    }

    pub fn list(&self, query: JobQuery) -> Result<Page<JobSnapshot>, StoreError> {
        let inner = self.lock();
        let limit = query.page.limit.get();
        let start = parse_cursor(query.page.cursor.as_deref())?;
        let mut values: Vec<_> = inner
            .jobs
            .values()
            .filter(|record| {
                query
                    .state
                    .is_none_or(|state| record.snapshot.state == state)
            })
            .filter(|record| query.kind.is_none_or(|kind| record.snapshot.kind == kind))
            .filter(|record| {
                query
                    .subject
                    .as_ref()
                    .is_none_or(|subject| record.snapshot.subject.id == subject.clone())
            })
            .map(|record| record.snapshot.clone())
            .collect();
        values.sort_by_key(|snapshot| (snapshot.created_at, snapshot.id));
        let total = values.len();
        let end = start.saturating_add(usize::from(limit));
        let items = values
            .into_iter()
            .skip(start)
            .take(usize::from(limit))
            .collect::<Vec<_>>();
        let next_cursor = (end < total).then(|| end.to_string());
        Ok(Page { items, next_cursor })
    }

    pub fn events_since(
        &self,
        id: JobId,
        after: Option<EventSeq>,
        limit: u32,
    ) -> Result<Vec<JobEventEnvelope>, StoreError> {
        if limit == 0 || limit > 1_000 {
            return Err(StoreError::InvalidLimit);
        }
        let inner = self.lock();
        let record = inner.jobs.get(&id).ok_or(StoreError::NotFound)?;
        Ok(record
            .events
            .iter()
            .filter(|event| after.is_none_or(|cursor| event.seq > cursor))
            .take(limit as usize)
            .cloned()
            .collect())
    }

    pub fn claim_next(
        &self,
        worker_id: WorkerId,
        now: Timestamp,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Option<Claim>, StoreError> {
        if lease_for.is_zero() {
            return Err(StoreError::InvalidLeaseDuration);
        }
        let mut inner = self.lock();
        let selected = inner
            .jobs
            .values()
            .filter(|record| record.snapshot.state == JobState::Queued)
            .filter(|record| {
                record
                    .spec
                    .resources
                    .iter()
                    .all(|resource| allowed.allows(*resource))
            })
            .max_by_key(|record| {
                (
                    record.spec.priority,
                    std::cmp::Reverse(record.snapshot.created_at),
                )
            })
            .map(|record| record.snapshot.id);
        let Some(id) = selected else {
            return Ok(None);
        };
        Self::claim_selected(&mut inner, id, worker_id, now, lease_for).map(Some)
    }

    pub fn claim(
        &self,
        id: JobId,
        worker_id: WorkerId,
        now: Timestamp,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Option<Claim>, StoreError> {
        if lease_for.is_zero() {
            return Err(StoreError::InvalidLeaseDuration);
        }
        let mut inner = self.lock();
        let Some(record) = inner.jobs.get(&id) else {
            return Err(StoreError::NotFound);
        };
        if record.snapshot.state != JobState::Queued
            || !record
                .spec
                .resources
                .iter()
                .all(|resource| allowed.allows(*resource))
        {
            return Ok(None);
        }
        Self::claim_selected(&mut inner, id, worker_id, now, lease_for).map(Some)
    }

    fn claim_selected(
        inner: &mut StoreInner,
        id: JobId,
        worker_id: WorkerId,
        now: Timestamp,
        lease_for: Duration,
    ) -> Result<Claim, StoreError> {
        let lease_expires_at =
            timestamp_after(now, lease_for).map_err(|_| StoreError::TimeOverflow)?;
        let record = inner.jobs.get_mut(&id).ok_or(StoreError::NotFound)?;
        Self::check_timestamp(record, now)?;
        let attempt = record
            .snapshot
            .attempt
            .next()
            .map_err(|_| StoreError::AttemptOverflow)?;
        let claim = ClaimRef {
            job_id: id,
            worker_id,
            attempt,
            lease_id: crate::LeaseId::new(),
        };
        Self::transition(record, JobState::Claimed)?;
        record.snapshot.attempt = attempt;
        record.snapshot.claim = Some(claim.clone());
        record.snapshot.lease_expires_at = Some(lease_expires_at);
        Self::append_event(
            record,
            JobEvent::Claimed {
                worker_id,
                attempt,
                lease_id: claim.lease_id,
                lease_expires_at,
                input_ref: record.spec.input_ref.clone(),
                recovery_policy: record.spec.recovery_policy,
                cancellation_policy: record.spec.cancellation_policy,
                resources: record.spec.resources.clone(),
            },
            now,
        )?;
        Ok(Claim {
            claim,
            lease_expires_at,
            input_ref: record.spec.input_ref.clone(),
            recovery_policy: record.spec.recovery_policy,
            cancellation_policy: record.spec.cancellation_policy,
            resources: record.spec.resources.clone(),
        })
    }

    pub fn heartbeat(
        &self,
        claim: &ClaimRef,
        now: Timestamp,
        extend_for: Duration,
    ) -> Result<Claim, StoreError> {
        if extend_for.is_zero() {
            return Err(StoreError::InvalidLeaseDuration);
        }
        let mut inner = self.lock();
        let record = inner
            .jobs
            .get_mut(&claim.job_id)
            .ok_or(StoreError::NotFound)?;
        Self::claim_matches(record, claim, now)?;
        let old_expiry = record
            .snapshot
            .lease_expires_at
            .ok_or(StoreError::StaleLease)?;
        if now > old_expiry {
            return Err(StoreError::LeaseExpired);
        }
        let lease_expires_at =
            timestamp_after(now, extend_for).map_err(|_| StoreError::TimeOverflow)?;
        let lease_expires_at = lease_expires_at.max(old_expiry);
        record.snapshot.lease_expires_at = Some(lease_expires_at);
        record.snapshot.updated_at = now;
        Ok(Claim {
            claim: claim.clone(),
            lease_expires_at,
            input_ref: record.spec.input_ref.clone(),
            recovery_policy: record.spec.recovery_policy,
            cancellation_policy: record.spec.cancellation_policy,
            resources: record.spec.resources.clone(),
        })
    }

    pub fn append_and_transition(&self, mutation: JobMutation) -> Result<JobSnapshot, StoreError> {
        let mut inner = self.lock();
        let id = mutation.job_id();
        Self::apply_mutation(&mut inner, mutation)?;
        inner
            .jobs
            .get(&id)
            .map(|record| record.snapshot.clone())
            .ok_or(StoreError::NotFound)
    }

    fn apply_mutation(inner: &mut StoreInner, mutation: JobMutation) -> Result<(), StoreError> {
        match mutation {
            JobMutation::RequestCancellation { id, reason, at } => {
                let record = inner.jobs.get_mut(&id).ok_or(StoreError::NotFound)?;
                Self::check_timestamp(record, at)?;
                if record.snapshot.state.is_terminal() {
                    return Err(StoreError::AlreadyTerminal);
                }
                if record.spec.cancellation_policy == CancellationPolicy::NotCancellable {
                    return Err(StoreError::NotCancellable);
                }
                if record.spec.cancellation_policy == CancellationPolicy::UntilIrreversibleStage
                    && record.snapshot.stage.irreversible
                    && matches!(record.snapshot.state, JobState::Running | JobState::Claimed)
                {
                    return Err(StoreError::TooLate);
                }
                if record.snapshot.cancellation.requested {
                    return Ok(());
                }
                if record.snapshot.state != JobState::CancellationRequested {
                    Self::transition(record, JobState::CancellationRequested)?;
                }
                record.snapshot.cancellation = CancellationView {
                    requested: true,
                    reason: Some(reason),
                    requested_at: Some(at),
                };
                Self::append_event(record, JobEvent::CancellationRequested { reason }, at)?;
            }
            JobMutation::Start { claim, at } => {
                let record = inner
                    .jobs
                    .get_mut(&claim.job_id)
                    .ok_or(StoreError::NotFound)?;
                Self::claim_matches(record, &claim, at)?;
                Self::transition(record, JobState::Running)?;
                Self::append_event(record, JobEvent::Started, at)?;
            }
            JobMutation::StageChanged { claim, stage, at } => {
                let record = inner
                    .jobs
                    .get_mut(&claim.job_id)
                    .ok_or(StoreError::NotFound)?;
                Self::claim_matches(record, &claim, at)?;
                if !matches!(
                    record.snapshot.state,
                    JobState::Running | JobState::CancellationRequested
                ) {
                    return Err(StoreError::IllegalTransition);
                }
                record.snapshot.stage = stage.clone();
                record.snapshot.progress = ProgressSnapshot::default();
                Self::append_event(record, JobEvent::StageChanged { stage }, at)?;
            }
            JobMutation::Progress {
                claim,
                progress,
                at,
            } => {
                let record = inner
                    .jobs
                    .get_mut(&claim.job_id)
                    .ok_or(StoreError::NotFound)?;
                Self::claim_matches(record, &claim, at)?;
                if record.snapshot.state != JobState::Running {
                    return Err(StoreError::IllegalTransition);
                }
                let progress = progress.preserving_omitted_from(&record.snapshot.progress);
                progress
                    .validate()
                    .map_err(|_| StoreError::InvalidProgress)?;
                if !progress.has_compatible_totals_with(&record.snapshot.progress) {
                    return Err(StoreError::InvalidProgress);
                }
                if !progress.is_monotonic_from(&record.snapshot.progress) {
                    return Err(StoreError::InvalidProgress);
                }
                record.snapshot.progress = progress.clone();
                Self::append_event(record, JobEvent::Progressed { progress }, at)?;
            }
            JobMutation::RequestCleanup { claim, at } => {
                let record = inner
                    .jobs
                    .get_mut(&claim.job_id)
                    .ok_or(StoreError::NotFound)?;
                Self::claim_matches(record, &claim, at)?;
                if !record.snapshot.cancellation.requested
                    || record.snapshot.state != JobState::CancellationRequested
                {
                    return Err(StoreError::IllegalTransition);
                }
                Self::transition(record, JobState::CleaningUp)?;
                Self::append_event(record, JobEvent::CleanupStarted, at)?;
            }
            JobMutation::Succeed { claim, outcome, at } => {
                if !matches!(&outcome, JobOutcome::Success { .. }) {
                    return Err(StoreError::InvalidOutcome);
                }
                let record = inner.jobs.get(&claim.job_id).ok_or(StoreError::NotFound)?;
                Self::claim_matches(record, &claim, at)?;
                Self::ensure_children_complete(inner, record)?;
                let record = inner
                    .jobs
                    .get_mut(&claim.job_id)
                    .ok_or(StoreError::NotFound)?;
                Self::finish_terminal(
                    record,
                    JobState::Succeeded,
                    JobEvent::Succeeded {
                        outcome: outcome.clone(),
                    },
                    at,
                )?;
                record.snapshot.outcome = Some(outcome);
                // The snapshot mutation above is the branch result.
                // `apply_mutation` performs the common final `Ok(())` below.
            }
            JobMutation::PartiallySucceed { claim, outcome, at } => {
                if !matches!(&outcome, JobOutcome::Partial { .. }) {
                    return Err(StoreError::InvalidOutcome);
                }
                let record = inner
                    .jobs
                    .get_mut(&claim.job_id)
                    .ok_or(StoreError::NotFound)?;
                Self::claim_matches(record, &claim, at)?;
                Self::finish_terminal(
                    record,
                    JobState::Succeeded,
                    JobEvent::PartiallySucceeded {
                        outcome: outcome.clone(),
                    },
                    at,
                )?;
                record.snapshot.outcome = Some(outcome);
                // The snapshot mutation above is the branch result.
            }
            JobMutation::Fail { claim, error, at } => {
                let record = inner
                    .jobs
                    .get_mut(&claim.job_id)
                    .ok_or(StoreError::NotFound)?;
                Self::claim_matches(record, &claim, at)?;
                Self::finish_terminal(
                    record,
                    JobState::Failed,
                    JobEvent::Failed {
                        error: error.clone(),
                    },
                    at,
                )?;
                record.snapshot.error = Some(error);
                // The snapshot mutation above is the branch result.
            }
            JobMutation::FinishCancellation { claim, at } => {
                let record = inner
                    .jobs
                    .get_mut(&claim.job_id)
                    .ok_or(StoreError::NotFound)?;
                Self::claim_matches(record, &claim, at)?;
                if record.snapshot.state != JobState::CleaningUp {
                    return Err(StoreError::IllegalTransition);
                }
                Self::finish_terminal(record, JobState::Cancelled, JobEvent::Cancelled, at)?;
            }
            JobMutation::FinishQueuedCancellation { id, at } => {
                let record = inner.jobs.get_mut(&id).ok_or(StoreError::NotFound)?;
                Self::check_timestamp(record, at)?;
                if record.snapshot.state != JobState::CancellationRequested {
                    return Err(StoreError::IllegalTransition);
                }
                Self::transition(record, JobState::CleaningUp)?;
                Self::append_event(record, JobEvent::CleanupStarted, at)?;
                Self::finish_terminal(record, JobState::Cancelled, JobEvent::Cancelled, at)?;
            }
            JobMutation::Interrupt { claim, at } => {
                let record = inner
                    .jobs
                    .get_mut(&claim.job_id)
                    .ok_or(StoreError::NotFound)?;
                Self::claim_matches(record, &claim, at)?;
                Self::finish_terminal(record, JobState::Interrupted, JobEvent::Interrupted, at)?;
            }
            JobMutation::Checkpoint {
                claim,
                checkpoint,
                at,
            } => {
                let record = inner
                    .jobs
                    .get_mut(&claim.job_id)
                    .ok_or(StoreError::NotFound)?;
                Self::claim_matches(record, &claim, at)?;
                if !matches!(
                    record.snapshot.state,
                    JobState::Running | JobState::CancellationRequested
                ) {
                    return Err(StoreError::IllegalTransition);
                }
                record.snapshot.checkpoint_ref = Some(checkpoint.clone());
                Self::append_event(record, JobEvent::Checkpointed { checkpoint }, at)?;
            }
            JobMutation::RetryScheduled { claim, at } => {
                let record = inner
                    .jobs
                    .get_mut(&claim.job_id)
                    .ok_or(StoreError::NotFound)?;
                Self::claim_matches(record, &claim, at)?;
                if !matches!(record.snapshot.state, JobState::Running | JobState::Claimed) {
                    return Err(StoreError::IllegalTransition);
                }
                let next_attempt = record
                    .snapshot
                    .attempt
                    .next()
                    .map_err(|_| StoreError::AttemptOverflow)?;
                Self::transition(record, JobState::Queued)?;
                record.snapshot.claim = None;
                record.snapshot.lease_expires_at = None;
                Self::append_event(
                    record,
                    JobEvent::RetryScheduled {
                        attempt: next_attempt,
                    },
                    at,
                )?;
            }
            JobMutation::AttachChild {
                parent_id,
                child,
                at,
            } => {
                let record = inner.jobs.get_mut(&parent_id).ok_or(StoreError::NotFound)?;
                Self::check_timestamp(record, at)?;
                if record.snapshot.is_terminal() {
                    return Err(StoreError::ParentTerminal);
                }
                record.snapshot.children.push(child.clone());
                Self::append_event(record, JobEvent::ChildAttached { child }, at)?;
            }
        }
        Ok(())
    }

    pub fn request_cancel(
        &self,
        id: JobId,
        reason: CancellationReason,
    ) -> Result<JobSnapshot, StoreError> {
        let now = self.lock().clock.now();
        self.append_and_transition(JobMutation::RequestCancellation {
            id,
            reason,
            at: now,
        })
    }

    pub fn finish_queued_cancellation(&self, id: JobId) -> Result<JobSnapshot, StoreError> {
        let now = self.lock().clock.now();
        self.append_and_transition(JobMutation::FinishQueuedCancellation { id, at: now })
    }

    pub fn expired_claims(
        &self,
        now: Timestamp,
        limit: u32,
    ) -> Result<Vec<ExpiredClaim>, StoreError> {
        if limit == 0 || limit > 1_000 {
            return Err(StoreError::InvalidLimit);
        }
        let mut inner = self.lock();
        let mut expired = Vec::new();
        let ids: Vec<_> = inner
            .jobs
            .values()
            .filter_map(|record| {
                let claim = record.snapshot.claim.clone()?;
                let expiry = record.snapshot.lease_expires_at?;
                (now > expiry).then_some((record.snapshot.id, claim, expiry))
            })
            .take(limit as usize)
            .collect();
        for (id, _, _) in &ids {
            let record = inner.jobs.get(id).ok_or(StoreError::NotFound)?;
            Self::check_timestamp(record, now)?;
            let required_events = if record.snapshot.state == JobState::CleaningUp {
                // Cleanup has already started. Expiry must finish it as an
                // interruption, never requeue work that may have performed
                // irreversible cleanup side effects.
                2
            } else if record.snapshot.state == JobState::CancellationRequested {
                3
            } else if matches!(
                record.spec.recovery_policy,
                crate::RecoveryPolicy::Compensate | crate::RecoveryPolicy::MarkInterrupted
            ) {
                2
            } else {
                1
            };
            let available =
                u64::MAX.saturating_sub(record.events.last().map_or(0, |event| event.seq.get()));
            if available < required_events as u64 {
                return Err(StoreError::SequenceOverflow);
            }
            if record.snapshot.state == JobState::CleaningUp {
                record
                    .snapshot
                    .state
                    .can_transition_to(JobState::Interrupted)
                    .map_err(|_| StoreError::IllegalTransition)?;
            } else if record.snapshot.state == JobState::CancellationRequested {
                record
                    .snapshot
                    .state
                    .can_transition_to(JobState::CleaningUp)
                    .map_err(|_| StoreError::IllegalTransition)?;
                JobState::CleaningUp
                    .can_transition_to(JobState::Interrupted)
                    .map_err(|_| StoreError::IllegalTransition)?;
            } else {
                let target = match record.spec.recovery_policy {
                    crate::RecoveryPolicy::Resume | crate::RecoveryPolicy::Restart => {
                        JobState::Queued
                    }
                    crate::RecoveryPolicy::Compensate | crate::RecoveryPolicy::MarkInterrupted => {
                        JobState::Interrupted
                    }
                };
                record
                    .snapshot
                    .state
                    .can_transition_to(target)
                    .map_err(|_| StoreError::IllegalTransition)?;
            }
        }
        for (id, claim, expired_at) in ids {
            let record = inner.jobs.get_mut(&id).ok_or(StoreError::NotFound)?;
            let old_state = record.snapshot.state;
            if !matches!(
                old_state,
                JobState::Claimed
                    | JobState::Running
                    | JobState::CancellationRequested
                    | JobState::CleaningUp
            ) {
                continue;
            }
            let recovery = record.spec.recovery_policy;
            Self::append_event(
                record,
                JobEvent::LeaseExpired {
                    lease_id: claim.lease_id,
                },
                now,
            )?;
            if old_state == JobState::CleaningUp {
                Self::finish_terminal(record, JobState::Interrupted, JobEvent::Interrupted, now)?;
            } else if old_state == JobState::CancellationRequested {
                Self::transition(record, JobState::CleaningUp)?;
                Self::append_event(record, JobEvent::CleanupStarted, now)?;
                Self::finish_terminal(record, JobState::Interrupted, JobEvent::Interrupted, now)?;
            } else {
                let target = match recovery {
                    crate::RecoveryPolicy::Resume | crate::RecoveryPolicy::Restart => {
                        JobState::Queued
                    }
                    crate::RecoveryPolicy::Compensate | crate::RecoveryPolicy::MarkInterrupted => {
                        JobState::Interrupted
                    }
                };
                Self::transition(record, target)?;
                record.snapshot.claim = None;
                record.snapshot.lease_expires_at = None;
                if target == JobState::Interrupted {
                    Self::append_event(record, JobEvent::Interrupted, now)?;
                }
            }
            expired.push(ExpiredClaim {
                claim,
                expired_at,
                job_id: id,
                recovery,
            });
        }
        Ok(expired)
    }

    pub fn prune(&self, policy: crate::retention::RetentionPolicy, now: Timestamp) -> PruneReport {
        let mut inner = self.lock();
        let threshold = policy.keep_terminal_for.and_then(|duration| {
            i64::try_from(duration.as_millis())
                .ok()
                .and_then(|millis| now.get().checked_sub(millis))
                .map(Timestamp::new)
        });
        if policy.keep_terminal_for.is_none() {
            return PruneReport {
                removed: Vec::new(),
            };
        }
        let mut eligible: std::collections::BTreeSet<JobId> = inner
            .jobs
            .values()
            .filter(|record| record.snapshot.is_terminal())
            .filter(|record| threshold.is_none_or(|value| record.snapshot.updated_at <= value))
            .map(|record| record.snapshot.id)
            .collect();

        // Retain any eligible node that has a reference outside the eligible
        // terminal graph. Recompute until stable so a blocked parent also
        // blocks its children, while an entirely old terminal graph is pruned
        // as one unit. Optional links are treated as references too.
        loop {
            let blocked: Vec<_> = eligible
                .iter()
                .copied()
                .filter(|id| {
                    let Some(record) = inner.jobs.get(id) else {
                        return false;
                    };
                    let has_external_parent = record
                        .snapshot
                        .parent_id
                        .is_some_and(|parent_id| !eligible.contains(&parent_id))
                        || inner.jobs.values().any(|parent| {
                            !eligible.contains(&parent.snapshot.id)
                                && parent
                                    .snapshot
                                    .children
                                    .iter()
                                    .any(|child| child.child_id == *id)
                        });
                    let has_external_child = record
                        .snapshot
                        .children
                        .iter()
                        .any(|child| !eligible.contains(&child.child_id))
                        || inner.jobs.values().any(|child| {
                            child.snapshot.parent_id == Some(*id)
                                && !eligible.contains(&child.snapshot.id)
                        });
                    has_external_parent || has_external_child
                })
                .collect();
            if blocked.is_empty() {
                break;
            }
            for id in blocked {
                eligible.remove(&id);
            }
        }
        let mut removed = Vec::new();
        let ids: Vec<_> = eligible.into_iter().collect();
        for id in ids {
            if let Some(record) = inner.jobs.remove(&id) {
                if let Some(key) = record.spec.idempotency_key {
                    inner.idempotency.remove(&key);
                }
                removed.push(id);
            }
        }
        PruneReport { removed }
    }
}

fn parse_cursor(cursor: Option<&str>) -> Result<usize, StoreError> {
    cursor.map_or(Ok(0), |value| {
        value
            .parse::<usize>()
            .map_err(|_| StoreError::InvalidCursor)
    })
}

fn same_submission(left: &JobSpec, right: &JobSpec) -> bool {
    left.kind == right.kind
        && left.subject == right.subject
        && left.parent_id == right.parent_id
        && left.input_ref == right.input_ref
        && left.priority == right.priority
        && left.recovery_policy == right.recovery_policy
        && left.cancellation_policy == right.cancellation_policy
        && left.resources == right.resources
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpiredClaim {
    pub job_id: JobId,
    pub claim: ClaimRef,
    pub expired_at: Timestamp,
    pub recovery: crate::RecoveryPolicy,
}

impl ExpiredClaim {
    /// Returns the app-owned action requested by the persisted recovery policy.
    /// This does not mean that the action has been performed.
    #[must_use]
    pub fn recovery_action(&self) -> crate::recovery::RecoveryAction {
        self.recovery.into()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PruneReport {
    pub removed: Vec<JobId>,
}

#[derive(Debug, Clone)]
pub enum JobMutation {
    RequestCancellation {
        id: JobId,
        reason: CancellationReason,
        at: Timestamp,
    },
    Start {
        claim: ClaimRef,
        at: Timestamp,
    },
    StageChanged {
        claim: ClaimRef,
        stage: StageSnapshot,
        at: Timestamp,
    },
    Progress {
        claim: ClaimRef,
        progress: ProgressSnapshot,
        at: Timestamp,
    },
    RequestCleanup {
        claim: ClaimRef,
        at: Timestamp,
    },
    Succeed {
        claim: ClaimRef,
        outcome: JobOutcome,
        at: Timestamp,
    },
    PartiallySucceed {
        claim: ClaimRef,
        outcome: JobOutcome,
        at: Timestamp,
    },
    Fail {
        claim: ClaimRef,
        error: JobError,
        at: Timestamp,
    },
    FinishCancellation {
        claim: ClaimRef,
        at: Timestamp,
    },
    Interrupt {
        claim: ClaimRef,
        at: Timestamp,
    },
    Checkpoint {
        claim: ClaimRef,
        checkpoint: OutcomeRef,
        at: Timestamp,
    },
    RetryScheduled {
        claim: ClaimRef,
        at: Timestamp,
    },
    FinishQueuedCancellation {
        id: JobId,
        at: Timestamp,
    },
    AttachChild {
        parent_id: JobId,
        child: ChildLink,
        at: Timestamp,
    },
}

impl JobMutation {
    #[must_use]
    pub const fn job_id(&self) -> JobId {
        match self {
            Self::RequestCancellation { id, .. }
            | Self::FinishQueuedCancellation { id, .. }
            | Self::AttachChild { parent_id: id, .. } => *id,
            Self::Start { claim, .. }
            | Self::StageChanged { claim, .. }
            | Self::Progress { claim, .. }
            | Self::RequestCleanup { claim, .. }
            | Self::Succeed { claim, .. }
            | Self::PartiallySucceed { claim, .. }
            | Self::Fail { claim, .. }
            | Self::FinishCancellation { claim, .. }
            | Self::Interrupt { claim, .. }
            | Self::Checkpoint { claim, .. }
            | Self::RetryScheduled { claim, .. } => claim.job_id,
        }
    }
}

pub trait JobStore: Send + Sync {
    fn create_or_get(&self, spec: NewJob) -> Result<CreateJobResult, StoreError>;
    fn get(&self, id: JobId) -> Result<Option<JobSnapshot>, StoreError>;
    fn list(&self, query: JobQuery) -> Result<Page<JobSnapshot>, StoreError>;
    fn events_since(
        &self,
        id: JobId,
        after: Option<EventSeq>,
        limit: u32,
    ) -> Result<Vec<JobEventEnvelope>, StoreError>;
    fn claim_next(
        &self,
        worker_id: WorkerId,
        now: Timestamp,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Option<Claim>, StoreError>;
    fn claim(
        &self,
        id: JobId,
        worker_id: WorkerId,
        now: Timestamp,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Option<Claim>, StoreError>;
    fn heartbeat(
        &self,
        claim: &ClaimRef,
        now: Timestamp,
        extend_for: Duration,
    ) -> Result<Claim, StoreError>;
    fn append_and_transition(&self, mutation: JobMutation) -> Result<JobSnapshot, StoreError>;
    fn expired_claims(&self, now: Timestamp, limit: u32) -> Result<Vec<ExpiredClaim>, StoreError>;
    fn prune(
        &self,
        policy: crate::retention::RetentionPolicy,
        now: Timestamp,
    ) -> Result<PruneReport, StoreError>;
}

impl JobStore for InMemoryJobStore {
    fn create_or_get(&self, spec: NewJob) -> Result<CreateJobResult, StoreError> {
        Self::create_or_get(self, spec)
    }

    fn get(&self, id: JobId) -> Result<Option<JobSnapshot>, StoreError> {
        Self::get(self, id)
    }

    fn list(&self, query: JobQuery) -> Result<Page<JobSnapshot>, StoreError> {
        Self::list(self, query)
    }

    fn events_since(
        &self,
        id: JobId,
        after: Option<EventSeq>,
        limit: u32,
    ) -> Result<Vec<JobEventEnvelope>, StoreError> {
        Self::events_since(self, id, after, limit)
    }

    fn claim_next(
        &self,
        worker_id: WorkerId,
        now: Timestamp,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Option<Claim>, StoreError> {
        Self::claim_next(self, worker_id, now, lease_for, allowed)
    }

    fn claim(
        &self,
        id: JobId,
        worker_id: WorkerId,
        now: Timestamp,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Option<Claim>, StoreError> {
        Self::claim(self, id, worker_id, now, lease_for, allowed)
    }

    fn heartbeat(
        &self,
        claim: &ClaimRef,
        now: Timestamp,
        extend_for: Duration,
    ) -> Result<Claim, StoreError> {
        Self::heartbeat(self, claim, now, extend_for)
    }

    fn append_and_transition(&self, mutation: JobMutation) -> Result<JobSnapshot, StoreError> {
        Self::append_and_transition(self, mutation)
    }

    fn expired_claims(&self, now: Timestamp, limit: u32) -> Result<Vec<ExpiredClaim>, StoreError> {
        Self::expired_claims(self, now, limit)
    }
    fn prune(
        &self,
        policy: crate::retention::RetentionPolicy,
        now: Timestamp,
    ) -> Result<PruneReport, StoreError> {
        Ok(Self::prune(self, policy, now))
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, thread, time::Duration};

    use lettuce_types::{AssetId, OperationId, PageLimit, PageRequest};
    use uuid::Uuid;

    use super::*;
    use crate::JobErrorCode;
    use crate::{
        FiniteFraction, IdempotencyKey, JobSubject, RecoveryPolicy, ResourceClass, SubjectKind,
        UnitsProgress, WarningCode,
    };

    fn spec(key: &str) -> JobSpec {
        JobSpec::new(
            JobKind::ArtifactInstall,
            JobSubject::new(SubjectKind::ArtifactInstall, "artifact-1").expect("subject"),
            OutcomeRef::ArtifactInstallation(AssetId::from_uuid(Uuid::nil())),
        )
        .with_resources(vec![ResourceClass::Network, ResourceClass::DiskWrite])
        .with_idempotency_key(IdempotencyKey::new(key).expect("key"))
    }

    fn test_store() -> InMemoryJobStore {
        InMemoryJobStore::with_clock(Arc::new(FakeClock::new(Timestamp::new(0))))
    }

    fn running_job(store: &InMemoryJobStore) -> Claim {
        let created = store.create_or_get(spec("running")).expect("create");
        let claim = store
            .claim_next(
                WorkerId::new(),
                Timestamp::new(10),
                Duration::from_secs(30),
                &ResourceAvailability::all(),
            )
            .expect("claim")
            .expect("claim exists");
        assert_eq!(created.job.state, JobState::Queued);
        store
            .append_and_transition(JobMutation::Start {
                claim: claim.claim.clone(),
                at: Timestamp::new(11),
            })
            .expect("start");
        claim
    }

    #[test]
    fn create_is_idempotent_and_event_is_first() {
        let store = test_store();
        let first = store.create_or_get(spec("same")).expect("create");
        let second = store.create_or_get(spec("same")).expect("get");
        assert!(first.created);
        assert!(!second.created);
        assert_eq!(first.job.id, second.job.id);
        assert_eq!(
            first.job.idempotency_key.as_ref().map(|key| key.as_str()),
            Some("same")
        );
        let events = store.events_since(first.job.id, None, 10).expect("events");
        assert_eq!(events[0].seq, EventSeq::FIRST);
        assert!(matches!(events[0].event, JobEvent::Created { .. }));
        assert!(matches!(events[1].event, JobEvent::Queued));
    }

    #[test]
    fn concurrent_idempotent_creates_return_one_job() {
        let store = Arc::new(test_store());
        let mut threads = Vec::new();
        for _ in 0..8 {
            let store = Arc::clone(&store);
            threads.push(thread::spawn(move || {
                store.create_or_get(spec("race")).expect("create").job.id
            }));
        }
        let ids: Vec<_> = threads
            .into_iter()
            .map(|thread| thread.join().expect("join"))
            .collect();
        assert!(ids.windows(2).all(|pair| pair[0] == pair[1]));
    }

    #[test]
    fn exact_claim_leaves_other_queued_jobs_untouched() {
        let store = test_store();
        let first = store.create_or_get(spec("first")).expect("first").job;
        let second = store.create_or_get(spec("second")).expect("second").job;

        assert!(
            store
                .claim(
                    second.id,
                    WorkerId::new(),
                    Timestamp::new(10),
                    Duration::from_secs(30),
                    &ResourceAvailability::none(),
                )
                .expect("resource check")
                .is_none()
        );
        let claim = store
            .claim(
                second.id,
                WorkerId::new(),
                Timestamp::new(10),
                Duration::from_secs(30),
                &ResourceAvailability::all(),
            )
            .expect("claim")
            .expect("eligible job");

        assert_eq!(claim.claim.job_id, second.id);
        assert_eq!(
            store
                .get(first.id)
                .expect("first")
                .expect("first job")
                .state,
            JobState::Queued
        );
        assert_eq!(
            store
                .get(second.id)
                .expect("second")
                .expect("second job")
                .state,
            JobState::Claimed
        );
    }

    #[test]
    fn stale_lease_cannot_update_or_settle() {
        let store = test_store();
        let claim = running_job(&store);
        let stale = ClaimRef {
            lease_id: crate::LeaseId::new(),
            ..claim.claim.clone()
        };
        let progress = ProgressSnapshot {
            fraction: Some(FiniteFraction::new(0.5).expect("fraction")),
            ..ProgressSnapshot::default()
        };
        assert_eq!(
            store.append_and_transition(JobMutation::Progress {
                claim: stale.clone(),
                progress,
                at: Timestamp::new(12)
            }),
            Err(StoreError::StaleLease)
        );
        let outcome = JobOutcome::Success {
            result_ref: OutcomeRef::ArtifactInstallation(AssetId::new()),
        };
        assert_eq!(
            store.append_and_transition(JobMutation::Succeed {
                claim: stale,
                outcome,
                at: Timestamp::new(12)
            }),
            Err(StoreError::StaleLease)
        );
    }

    #[test]
    fn progress_is_monotonic_and_stage_change_resets_it() {
        let store = test_store();
        let claim = running_job(&store);
        let half = ProgressSnapshot {
            fraction: Some(FiniteFraction::new(0.5).expect("fraction")),
            ..ProgressSnapshot::default()
        };
        store
            .append_and_transition(JobMutation::Progress {
                claim: claim.claim.clone(),
                progress: half,
                at: Timestamp::new(12),
            })
            .expect("progress");
        let lower = ProgressSnapshot {
            fraction: Some(FiniteFraction::new(0.2).expect("fraction")),
            ..ProgressSnapshot::default()
        };
        assert_eq!(
            store.append_and_transition(JobMutation::Progress {
                claim: claim.claim.clone(),
                progress: lower,
                at: Timestamp::new(13)
            }),
            Err(StoreError::InvalidProgress)
        );
        let stage = StageSnapshot::new("verify", false).expect("stage");
        store
            .append_and_transition(JobMutation::StageChanged {
                claim: claim.claim.clone(),
                stage,
                at: Timestamp::new(14),
            })
            .expect("stage");
        let low = ProgressSnapshot {
            fraction: Some(FiniteFraction::new(0.2).expect("fraction")),
            ..ProgressSnapshot::default()
        };
        store
            .append_and_transition(JobMutation::Progress {
                claim: claim.claim.clone(),
                progress: low,
                at: Timestamp::new(15),
            })
            .expect("reset");
    }

    #[test]
    fn cancellation_requires_cleanup_then_terminal() {
        let store = test_store();
        let claim = running_job(&store);
        let id = claim.claim.job_id;
        store
            .append_and_transition(JobMutation::RequestCancellation {
                id,
                reason: CancellationReason::User,
                at: Timestamp::new(12),
            })
            .expect("cancel request");
        assert_eq!(
            store.get(id).expect("get").expect("job").state,
            JobState::CancellationRequested
        );
        assert_eq!(
            store.append_and_transition(JobMutation::FinishCancellation {
                claim: claim.claim.clone(),
                at: Timestamp::new(13)
            }),
            Err(StoreError::IllegalTransition)
        );
        store
            .append_and_transition(JobMutation::RequestCleanup {
                claim: claim.claim.clone(),
                at: Timestamp::new(13),
            })
            .expect("cleanup");
        let snapshot = store
            .append_and_transition(JobMutation::FinishCancellation {
                claim: claim.claim.clone(),
                at: Timestamp::new(14),
            })
            .expect("finish");
        assert_eq!(snapshot.state, JobState::Cancelled);
        assert!(store.request_cancel(id, CancellationReason::User).is_err());
    }

    #[test]
    fn queued_cancellation_is_request_then_cleanup_then_terminal() {
        let store = test_store();
        let created = store.create_or_get(spec("queued-cancel")).expect("create");
        store
            .request_cancel(created.job.id, CancellationReason::User)
            .expect("request");
        assert_eq!(
            store
                .finish_queued_cancellation(created.job.id)
                .expect("cleanup")
                .state,
            JobState::Cancelled
        );
        let events = store
            .events_since(created.job.id, None, 20)
            .expect("events");
        assert!(
            events
                .iter()
                .any(|event| matches!(event.event, JobEvent::CleanupStarted))
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event.event, JobEvent::Cancelled))
        );
        assert!(
            store
                .claim_next(
                    WorkerId::new(),
                    Timestamp::new(1),
                    Duration::from_secs(1),
                    &ResourceAvailability::all()
                )
                .expect("claim")
                .is_none()
        );
    }

    #[test]
    fn heartbeat_extends_only_the_current_lease() {
        let clock = Arc::new(FakeClock::new(Timestamp::new(100)));
        let store = InMemoryJobStore::with_clock(clock.clone());
        let created = store.create_or_get(spec("heartbeat")).expect("create");
        let claim = store
            .claim_next(
                WorkerId::new(),
                Timestamp::new(100),
                Duration::from_millis(10),
                &ResourceAvailability::all(),
            )
            .expect("claim")
            .expect("claim exists");
        assert_eq!(
            store.heartbeat(&claim.claim, Timestamp::new(100), Duration::ZERO),
            Err(StoreError::InvalidLeaseDuration)
        );
        let extended = store
            .heartbeat(
                &claim.claim,
                Timestamp::new(105),
                Duration::from_millis(100),
            )
            .expect("heartbeat");
        assert_eq!(extended.lease_expires_at, Timestamp::new(205));
        assert_eq!(
            store
                .get(created.job.id)
                .expect("get")
                .expect("job")
                .lease_expires_at,
            Some(Timestamp::new(205))
        );
        store
            .append_and_transition(JobMutation::Start {
                claim: claim.claim.clone(),
                at: Timestamp::new(106),
            })
            .expect("start");
        store
            .append_and_transition(JobMutation::Progress {
                claim: claim.claim.clone(),
                progress: ProgressSnapshot {
                    fraction: Some(FiniteFraction::new(0.1).expect("fraction")),
                    ..ProgressSnapshot::default()
                },
                at: Timestamp::new(111),
            })
            .expect("old lease expiry is superseded by heartbeat");
        assert_eq!(
            store.append_and_transition(JobMutation::Progress {
                claim: claim.claim.clone(),
                progress: ProgressSnapshot {
                    fraction: Some(FiniteFraction::new(0.2).expect("fraction")),
                    ..ProgressSnapshot::default()
                },
                at: Timestamp::new(206),
            }),
            Err(StoreError::LeaseExpired)
        );
        assert!(
            store
                .heartbeat(&claim.claim, Timestamp::new(206), Duration::from_millis(10))
                .is_err()
        );
        clock.set(Timestamp::new(206));
        assert_eq!(
            store
                .expired_claims(Timestamp::new(206), 10)
                .expect("expiry")
                .len(),
            1
        );
    }

    #[test]
    fn irreversible_stage_rejects_late_cancellation() {
        let store = test_store();
        let claim = running_job(&store);
        store
            .append_and_transition(JobMutation::StageChanged {
                claim: claim.claim.clone(),
                stage: StageSnapshot::new("cutover", true).expect("stage"),
                at: Timestamp::new(12),
            })
            .expect("stage");
        assert!(
            store
                .append_and_transition(JobMutation::RequestCancellation {
                    id: claim.claim.job_id,
                    reason: CancellationReason::User,
                    at: Timestamp::new(13),
                })
                .is_ok()
        );

        let strict = test_store();
        let strict_job = JobSpec::new(
            JobKind::ArtifactInstall,
            JobSubject::new(SubjectKind::ArtifactInstall, "strict").expect("subject"),
            OutcomeRef::ArtifactInstallation(AssetId::new()),
        )
        .with_resources(vec![ResourceClass::Network])
        .with_policies(
            RecoveryPolicy::Restart,
            CancellationPolicy::UntilIrreversibleStage,
        );
        let strict_id = strict.create_or_get(strict_job).expect("create").job.id;
        let strict_claim = strict
            .claim_next(
                WorkerId::new(),
                Timestamp::new(1),
                Duration::from_secs(30),
                &ResourceAvailability::all(),
            )
            .expect("claim")
            .expect("claim exists");
        strict
            .append_and_transition(JobMutation::Start {
                claim: strict_claim.claim.clone(),
                at: Timestamp::new(2),
            })
            .expect("start");
        strict
            .append_and_transition(JobMutation::StageChanged {
                claim: strict_claim.claim,
                stage: StageSnapshot::new("cutover", true).expect("stage"),
                at: Timestamp::new(3),
            })
            .expect("stage");
        assert_eq!(
            strict.append_and_transition(JobMutation::RequestCancellation {
                id: strict_id,
                reason: CancellationReason::User,
                at: Timestamp::new(4),
            }),
            Err(StoreError::TooLate)
        );
    }

    #[test]
    fn retry_claim_attempt_increments_exactly_once() {
        let store = test_store();
        let claim = running_job(&store);
        assert_eq!(claim.claim.attempt, AttemptNo::new(1));
        store
            .append_and_transition(JobMutation::RetryScheduled {
                claim: claim.claim,
                at: Timestamp::new(12),
            })
            .expect("retry");
        let next = store
            .claim_next(
                WorkerId::new(),
                Timestamp::new(13),
                Duration::from_secs(30),
                &ResourceAvailability::all(),
            )
            .expect("claim")
            .expect("claim exists");
        assert_eq!(next.claim.attempt, AttemptNo::new(2));
        assert_eq!(
            store
                .get(next.claim.job_id)
                .expect("get")
                .expect("job")
                .attempt,
            AttemptNo::new(2)
        );
    }

    #[test]
    fn invalid_lease_durations_and_timestamp_regressions_are_rejected() {
        let store = test_store();
        let created = store.create_or_get(spec("time")).expect("create");
        assert_eq!(
            store.claim_next(
                WorkerId::new(),
                Timestamp::new(0),
                Duration::ZERO,
                &ResourceAvailability::all()
            ),
            Err(StoreError::InvalidLeaseDuration)
        );
        let claim = store
            .claim_next(
                WorkerId::new(),
                Timestamp::new(1),
                Duration::from_secs(10),
                &ResourceAvailability::all(),
            )
            .expect("claim")
            .expect("claim");
        assert_eq!(
            store.heartbeat(&claim.claim, Timestamp::new(0), Duration::from_secs(20)),
            Err(StoreError::TimestampRegression)
        );
        assert_eq!(
            store.append_and_transition(JobMutation::Start {
                claim: claim.claim,
                at: Timestamp::new(0)
            }),
            Err(StoreError::TimestampRegression)
        );
        assert_eq!(
            store
                .get(created.job.id)
                .expect("get")
                .expect("job")
                .updated_at,
            Timestamp::new(1)
        );
    }

    #[test]
    fn outcome_mutations_accept_only_matching_variants() {
        let store = test_store();
        let claim = running_job(&store);
        let partial = JobOutcome::Partial {
            result_ref: OutcomeRef::ArtifactInstallation(AssetId::new()),
            warnings: vec![WarningCode::PartialOutput],
        };
        assert_eq!(
            store.append_and_transition(JobMutation::Succeed {
                claim: claim.claim.clone(),
                outcome: partial.clone(),
                at: Timestamp::new(12)
            }),
            Err(StoreError::InvalidOutcome)
        );
        assert_eq!(
            store.append_and_transition(JobMutation::PartiallySucceed {
                claim: claim.claim,
                outcome: JobOutcome::Success {
                    result_ref: OutcomeRef::ArtifactInstallation(AssetId::new())
                },
                at: Timestamp::new(12)
            }),
            Err(StoreError::InvalidOutcome)
        );
    }

    #[test]
    fn checkpoint_is_authoritative_and_progress_dimensions_cannot_disappear() {
        let store = test_store();
        let claim = running_job(&store);
        let checkpoint = OutcomeRef::Checkpoint(crate::OutcomeId::new());
        store
            .append_and_transition(JobMutation::Checkpoint {
                claim: claim.claim.clone(),
                checkpoint: checkpoint.clone(),
                at: Timestamp::new(12),
            })
            .expect("checkpoint");
        assert_eq!(
            store
                .get(claim.claim.job_id)
                .expect("get")
                .expect("job")
                .checkpoint_ref,
            Some(checkpoint)
        );
        store
            .append_and_transition(JobMutation::Progress {
                claim: claim.claim.clone(),
                progress: ProgressSnapshot {
                    units: Some(UnitsProgress::new(2, Some(10)).expect("progress")),
                    ..ProgressSnapshot::default()
                },
                at: Timestamp::new(13),
            })
            .expect("progress");
        let updated = store
            .append_and_transition(JobMutation::Progress {
                claim: claim.claim.clone(),
                progress: ProgressSnapshot::default(),
                at: Timestamp::new(14),
            })
            .expect("omitted dimensions preserve their previous values");
        assert_eq!(
            updated.progress.units,
            Some(UnitsProgress::new(2, Some(10)).expect("progress"))
        );
    }

    #[test]
    fn spec_validation_and_cancellation_policies_are_enforced() {
        let store = test_store();
        let invalid = JobSpec::new(
            JobKind::Maintenance,
            JobSubject::new(SubjectKind::Maintenance, "invalid").expect("subject"),
            OutcomeRef::Checkpoint(crate::OutcomeId::new()),
        );
        assert_eq!(
            store.create_or_get(invalid),
            Err(StoreError::InvalidSpec(SpecError::EmptyResources))
        );
        let duplicate = JobSpec::new(
            JobKind::Maintenance,
            JobSubject::new(SubjectKind::Maintenance, "duplicate").expect("subject"),
            OutcomeRef::Checkpoint(crate::OutcomeId::new()),
        )
        .with_resources(vec![ResourceClass::Cpu, ResourceClass::Cpu]);
        assert_eq!(
            store.create_or_get(duplicate),
            Err(StoreError::InvalidSpec(SpecError::DuplicateResource))
        );
        let not_cancellable = JobSpec::new(
            JobKind::Maintenance,
            JobSubject::new(SubjectKind::Maintenance, "no-cancel").expect("subject"),
            OutcomeRef::Checkpoint(crate::OutcomeId::new()),
        )
        .with_resources(vec![ResourceClass::Cpu])
        .with_policies(RecoveryPolicy::Restart, CancellationPolicy::NotCancellable);
        let id = store.create_or_get(not_cancellable).expect("create").job.id;
        assert_eq!(
            store.request_cancel(id, CancellationReason::User),
            Err(StoreError::NotCancellable)
        );
    }

    #[test]
    fn retention_none_keeps_jobs_and_required_parent_graph() {
        let store = test_store();
        let standalone = store.create_or_get(spec("retain")).expect("create").job.id;
        let claim = store
            .claim_next(
                WorkerId::new(),
                Timestamp::new(1),
                Duration::from_secs(30),
                &ResourceAvailability::all(),
            )
            .expect("claim")
            .expect("claim");
        store
            .append_and_transition(JobMutation::Start {
                claim: claim.claim.clone(),
                at: Timestamp::new(2),
            })
            .expect("start");
        store
            .append_and_transition(JobMutation::Succeed {
                claim: claim.claim,
                outcome: JobOutcome::Success {
                    result_ref: OutcomeRef::ArtifactInstallation(AssetId::new()),
                },
                at: Timestamp::new(3),
            })
            .expect("finish");
        let report = store.prune(
            crate::retention::RetentionPolicy {
                keep_terminal_for: None,
            },
            Timestamp::new(100),
        );
        assert!(report.removed.is_empty());
        assert!(store.get(standalone).expect("get").is_some());
    }

    #[test]
    fn event_history_is_contiguous_and_terminal_snapshot_matches_event() {
        let store = test_store();
        let claim = running_job(&store);
        let id = claim.claim.job_id;
        let outcome = JobOutcome::Success {
            result_ref: OutcomeRef::ArtifactInstallation(AssetId::new()),
        };
        let snapshot = store
            .append_and_transition(JobMutation::Succeed {
                claim: claim.claim,
                outcome: outcome.clone(),
                at: Timestamp::new(12),
            })
            .expect("finish");
        assert_eq!(snapshot.outcome, Some(outcome.clone()));
        let events = store.events_since(id, None, 100).expect("events");
        for (index, event) in events.iter().enumerate() {
            assert_eq!(event.seq.get(), (index + 1) as u64);
        }
        assert!(
            matches!(events.last().map(|event| &event.event), Some(JobEvent::Succeeded { outcome: actual }) if actual == &outcome)
        );
    }

    #[test]
    fn terminal_state_and_events_are_immutable() {
        let store = test_store();
        let claim = running_job(&store);
        let id = claim.claim.job_id;
        store
            .append_and_transition(JobMutation::Fail {
                claim: claim.claim.clone(),
                error: JobError::new(JobErrorCode::WorkerFailed, false, "worker failed")
                    .expect("error"),
                at: Timestamp::new(12),
            })
            .expect("fail");
        let before = store.events_since(id, None, 100).expect("events").len();
        assert_eq!(
            store.append_and_transition(JobMutation::Interrupt {
                claim: claim.claim,
                at: Timestamp::new(13)
            }),
            Err(StoreError::StaleLease)
        );
        assert_eq!(
            store.events_since(id, None, 100).expect("events").len(),
            before
        );
    }

    #[test]
    fn lease_expiry_returns_job_to_queue_and_rejects_old_claim() {
        let clock = Arc::new(FakeClock::new(Timestamp::new(100)));
        let store = InMemoryJobStore::with_clock(clock.clone());
        store.create_or_get(spec("expiry")).expect("create");
        let claim = store
            .claim_next(
                WorkerId::new(),
                Timestamp::new(100),
                Duration::from_secs(1),
                &ResourceAvailability::all(),
            )
            .expect("claim")
            .expect("claim");
        clock.set(Timestamp::new(2_000));
        let expired = store
            .expired_claims(Timestamp::new(2_000), 10)
            .expect("expired");
        assert_eq!(expired.len(), 1);
        assert_eq!(
            store
                .get(claim.claim.job_id)
                .expect("get")
                .expect("job")
                .state,
            JobState::Queued
        );
        assert_eq!(
            store.append_and_transition(JobMutation::Start {
                claim: claim.claim,
                at: Timestamp::new(2_001)
            }),
            Err(StoreError::StaleLease)
        );
    }

    #[test]
    fn lease_expiry_honors_recovery_policy_and_cancellation_state() {
        for (policy, expected) in [
            (RecoveryPolicy::Resume, JobState::Queued),
            (RecoveryPolicy::Restart, JobState::Queued),
            (RecoveryPolicy::Compensate, JobState::Interrupted),
            (RecoveryPolicy::MarkInterrupted, JobState::Interrupted),
        ] {
            let clock = Arc::new(FakeClock::new(Timestamp::new(0)));
            let store = InMemoryJobStore::with_clock(clock.clone());
            let job = JobSpec::new(
                JobKind::ArtifactInstall,
                JobSubject::new(SubjectKind::ArtifactInstall, "policy").expect("subject"),
                OutcomeRef::ArtifactInstallation(AssetId::new()),
            )
            .with_resources(vec![ResourceClass::Network])
            .with_policies(policy, CancellationPolicy::Cooperative);
            let id = store.create_or_get(job).expect("create").job.id;
            let claim = store
                .claim_next(
                    WorkerId::new(),
                    Timestamp::new(1),
                    Duration::from_secs(1),
                    &ResourceAvailability::all(),
                )
                .expect("claim")
                .expect("claim exists");
            clock.set(Timestamp::new(2_000));
            let expired = store
                .expired_claims(Timestamp::new(2_000), 10)
                .expect("expire");
            assert_eq!(expired[0].recovery, policy);
            assert_eq!(expired[0].recovery_action(), policy.into());
            assert_eq!(store.get(id).expect("get").expect("job").state, expected);
            assert_eq!(claim.claim.attempt, AttemptNo::new(1));
        }

        let clock = Arc::new(FakeClock::new(Timestamp::new(0)));
        let store = InMemoryJobStore::with_clock(clock.clone());
        let id = store
            .create_or_get(spec("cancel-expiry"))
            .expect("create")
            .job
            .id;
        let claim = store
            .claim_next(
                WorkerId::new(),
                Timestamp::new(1),
                Duration::from_secs(1),
                &ResourceAvailability::all(),
            )
            .expect("claim")
            .expect("claim exists");
        store
            .append_and_transition(JobMutation::RequestCancellation {
                id,
                reason: CancellationReason::User,
                at: Timestamp::new(2),
            })
            .expect("request cancellation");
        clock.set(Timestamp::new(2_000));
        store
            .expired_claims(Timestamp::new(2_000), 10)
            .expect("expire");
        assert_eq!(
            store.get(id).expect("get").expect("job").state,
            JobState::Interrupted
        );
        assert!(
            store
                .events_since(id, None, 100)
                .expect("events")
                .iter()
                .any(|event| matches!(event.event, JobEvent::CleanupStarted))
        );
        assert_eq!(claim.claim.attempt, AttemptNo::new(1));
    }

    #[test]
    fn expired_cleanup_is_interrupted_without_aborting_other_expired_jobs() {
        let clock = Arc::new(FakeClock::new(Timestamp::new(0)));
        let store = InMemoryJobStore::with_clock(clock.clone());
        let cleanup_id = store
            .create_or_get(spec("cleanup-expiry"))
            .expect("create cleanup job")
            .job
            .id;
        clock.set(Timestamp::new(1));
        let queued_id = store
            .create_or_get(spec("queued-expiry"))
            .expect("create queued job")
            .job
            .id;

        let cleanup_claim = store
            .claim_next(
                WorkerId::new(),
                Timestamp::new(1),
                Duration::from_secs(1),
                &ResourceAvailability::all(),
            )
            .expect("claim cleanup job")
            .expect("cleanup claim exists");
        store
            .append_and_transition(JobMutation::Start {
                claim: cleanup_claim.claim.clone(),
                at: Timestamp::new(2),
            })
            .expect("start cleanup job");
        store
            .append_and_transition(JobMutation::RequestCancellation {
                id: cleanup_id,
                reason: CancellationReason::User,
                at: Timestamp::new(3),
            })
            .expect("request cancellation");
        store
            .append_and_transition(JobMutation::RequestCleanup {
                claim: cleanup_claim.claim,
                at: Timestamp::new(4),
            })
            .expect("start cleanup");

        let queued_claim = store
            .claim_next(
                WorkerId::new(),
                Timestamp::new(5),
                Duration::from_secs(1),
                &ResourceAvailability::all(),
            )
            .expect("claim queued job")
            .expect("queued claim exists");
        assert_eq!(queued_claim.claim.job_id, queued_id);

        let expired = store
            .expired_claims(Timestamp::new(2_000), 10)
            .expect("both expired claims are processed");
        assert_eq!(expired.len(), 2);
        assert_eq!(
            store.get(cleanup_id).expect("get").expect("job").state,
            JobState::Interrupted
        );
        assert_eq!(
            store.get(queued_id).expect("get").expect("job").state,
            JobState::Queued
        );
        let cleanup_events = store
            .events_since(cleanup_id, None, 100)
            .expect("cleanup events");
        assert!(matches!(
            cleanup_events.last().map(|event| &event.event),
            Some(JobEvent::Interrupted)
        ));
    }

    #[test]
    fn retention_prunes_an_old_terminal_parent_and_child_together() {
        let clock = Arc::new(FakeClock::new(Timestamp::new(0)));
        let store = InMemoryJobStore::with_clock(clock.clone());
        let parent = store
            .create_or_get(spec("retention-parent"))
            .expect("parent")
            .job
            .id;
        clock.set(Timestamp::new(1));
        let child = store
            .create_or_get(spec("retention-child").with_parent(parent))
            .expect("child")
            .job
            .id;

        let parent_claim = store
            .claim_next(
                WorkerId::new(),
                Timestamp::new(2),
                Duration::from_secs(30),
                &ResourceAvailability::all(),
            )
            .expect("claim parent")
            .expect("parent claim exists");
        store
            .append_and_transition(JobMutation::Start {
                claim: parent_claim.claim.clone(),
                at: Timestamp::new(2),
            })
            .expect("start parent");
        let child_claim = store
            .claim_next(
                WorkerId::new(),
                Timestamp::new(3),
                Duration::from_secs(30),
                &ResourceAvailability::all(),
            )
            .expect("claim child")
            .expect("child claim exists");
        store
            .append_and_transition(JobMutation::Start {
                claim: child_claim.claim.clone(),
                at: Timestamp::new(3),
            })
            .expect("start child");
        let outcome = || JobOutcome::Success {
            result_ref: OutcomeRef::ArtifactInstallation(AssetId::new()),
        };
        store
            .append_and_transition(JobMutation::Succeed {
                claim: child_claim.claim,
                outcome: outcome(),
                at: Timestamp::new(4),
            })
            .expect("finish child");
        store
            .append_and_transition(JobMutation::Succeed {
                claim: parent_claim.claim,
                outcome: outcome(),
                at: Timestamp::new(5),
            })
            .expect("finish parent");

        let report = store.prune(
            crate::retention::RetentionPolicy {
                keep_terminal_for: Some(Duration::ZERO),
            },
            Timestamp::new(100),
        );
        assert!(report.removed.contains(&parent));
        assert!(report.removed.contains(&child));
        assert!(store.get(parent).expect("get parent").is_none());
        assert!(store.get(child).expect("get child").is_none());
    }

    #[test]
    fn retention_keeps_terminal_child_referenced_by_nonterminal_parent() {
        let clock = Arc::new(FakeClock::new(Timestamp::new(0)));
        let store = InMemoryJobStore::with_clock(clock.clone());
        let parent = store
            .create_or_get(spec("retention-running-parent"))
            .expect("parent")
            .job
            .id;
        clock.set(Timestamp::new(1));
        let child = store
            .create_or_get(spec("retention-running-child").with_parent(parent))
            .expect("child")
            .job
            .id;
        let parent_claim = store
            .claim_next(
                WorkerId::new(),
                Timestamp::new(2),
                Duration::from_secs(30),
                &ResourceAvailability::all(),
            )
            .expect("claim parent")
            .expect("parent claim exists");
        store
            .append_and_transition(JobMutation::Start {
                claim: parent_claim.claim,
                at: Timestamp::new(2),
            })
            .expect("start parent");
        let child_claim = store
            .claim_next(
                WorkerId::new(),
                Timestamp::new(3),
                Duration::from_secs(30),
                &ResourceAvailability::all(),
            )
            .expect("claim child")
            .expect("child claim exists");
        store
            .append_and_transition(JobMutation::Start {
                claim: child_claim.claim.clone(),
                at: Timestamp::new(3),
            })
            .expect("start child");
        store
            .append_and_transition(JobMutation::Succeed {
                claim: child_claim.claim,
                outcome: JobOutcome::Success {
                    result_ref: OutcomeRef::ArtifactInstallation(AssetId::new()),
                },
                at: Timestamp::new(4),
            })
            .expect("finish child");

        let report = store.prune(
            crate::retention::RetentionPolicy {
                keep_terminal_for: Some(Duration::ZERO),
            },
            Timestamp::new(100),
        );
        assert!(report.removed.is_empty());
        assert!(store.get(parent).expect("get parent").is_some());
        assert!(store.get(child).expect("get child").is_some());
    }

    #[test]
    fn retention_keeps_eligible_parent_with_a_noneligible_optional_child() {
        let clock = Arc::new(FakeClock::new(Timestamp::new(0)));
        let store = InMemoryJobStore::with_clock(clock.clone());
        let parent = store
            .create_or_get(spec("retention-optional-parent"))
            .expect("parent")
            .job
            .id;
        clock.set(Timestamp::new(1));
        let child = store
            .create_or_get(spec("retention-optional-child"))
            .expect("child")
            .job
            .id;
        store
            .append_and_transition(JobMutation::AttachChild {
                parent_id: parent,
                child: ChildLink {
                    child_id: child,
                    required: false,
                },
                at: Timestamp::new(2),
            })
            .expect("attach optional child");
        let parent_claim = store
            .claim_next(
                WorkerId::new(),
                Timestamp::new(3),
                Duration::from_secs(30),
                &ResourceAvailability::all(),
            )
            .expect("claim parent")
            .expect("parent claim exists");
        store
            .append_and_transition(JobMutation::Start {
                claim: parent_claim.claim.clone(),
                at: Timestamp::new(3),
            })
            .expect("start parent");
        store
            .append_and_transition(JobMutation::Succeed {
                claim: parent_claim.claim,
                outcome: JobOutcome::Success {
                    result_ref: OutcomeRef::ArtifactInstallation(AssetId::new()),
                },
                at: Timestamp::new(4),
            })
            .expect("finish parent");

        let report = store.prune(
            crate::retention::RetentionPolicy {
                keep_terminal_for: Some(Duration::ZERO),
            },
            Timestamp::new(100),
        );
        assert!(report.removed.is_empty());
        assert!(store.get(parent).expect("get parent").is_some());
        assert!(store.get(child).expect("get child").is_some());
    }

    #[test]
    fn event_cursor_and_page_cursor_are_bounded() {
        let store = test_store();
        let created = store.create_or_get(spec("page")).expect("create");
        assert_eq!(
            store
                .events_since(created.job.id, Some(EventSeq::new(1)), 10)
                .expect("events")
                .len(),
            1
        );
        assert_eq!(
            store.events_since(created.job.id, None, 0),
            Err(StoreError::InvalidLimit)
        );
        let page = store
            .list(JobQuery::page(PageRequest {
                cursor: None,
                limit: PageLimit::new(1),
            }))
            .expect("page");
        assert_eq!(page.items.len(), 1);
        assert!(
            store
                .list(JobQuery::page(PageRequest {
                    cursor: Some("bad".into()),
                    limit: PageLimit::new(1)
                }))
                .is_err()
        );
    }

    #[test]
    fn serialized_events_have_no_path_or_raw_payload_field() {
        let store = test_store();
        let created = store.create_or_get(spec("canary")).expect("create");
        let json = serde_json::to_string(
            &store
                .events_since(created.job.id, None, 10)
                .expect("events"),
        )
        .expect("serialize");
        assert!(!json.contains("/tmp"));
        assert!(!json.contains("prompt"));
    }

    #[test]
    fn parent_requires_successful_required_children() {
        let clock = Arc::new(FakeClock::new(Timestamp::new(1)));
        let store = InMemoryJobStore::with_clock(clock.clone());
        let parent_spec = JobSpec::new(
            JobKind::TransferExport,
            JobSubject::new(SubjectKind::TransferPlan, "parent").expect("subject"),
            OutcomeRef::TransferReport(OperationId::new()),
        )
        .with_resources(vec![ResourceClass::DiskRead, ResourceClass::DiskWrite]);
        let parent = store.create_or_get(parent_spec).expect("parent");
        clock.set(Timestamp::new(2));
        let child = store
            .create_or_get(
                JobSpec::new(
                    JobKind::ArtifactVerify,
                    JobSubject::new(SubjectKind::ArtifactInstall, "child").expect("subject"),
                    OutcomeRef::ArtifactInstallation(AssetId::new()),
                )
                .with_resources(vec![ResourceClass::DiskRead, ResourceClass::Cpu])
                .with_parent(parent.job.id),
            )
            .expect("child");
        let parent_claim = store
            .claim_next(
                WorkerId::new(),
                Timestamp::new(3),
                Duration::from_secs(30),
                &ResourceAvailability::all(),
            )
            .expect("claim")
            .expect("parent claim");
        store
            .append_and_transition(JobMutation::Start {
                claim: parent_claim.claim.clone(),
                at: Timestamp::new(3),
            })
            .expect("start parent");
        // Child is queued and therefore the parent cannot be marked successful.
        assert_eq!(
            store.append_and_transition(JobMutation::Succeed {
                claim: parent_claim.claim.clone(),
                outcome: JobOutcome::Success {
                    result_ref: OutcomeRef::TransferReport(OperationId::new())
                },
                at: Timestamp::new(4)
            }),
            Err(StoreError::ChildrenIncomplete)
        );
        let child_claim = store
            .claim_next(
                WorkerId::new(),
                Timestamp::new(4),
                Duration::from_secs(30),
                &ResourceAvailability::all(),
            )
            .expect("claim")
            .expect("child claim");
        store
            .append_and_transition(JobMutation::Start {
                claim: child_claim.claim.clone(),
                at: Timestamp::new(4),
            })
            .expect("start child");
        store
            .append_and_transition(JobMutation::Succeed {
                claim: child_claim.claim,
                outcome: JobOutcome::Success {
                    result_ref: OutcomeRef::ArtifactInstallation(AssetId::from_uuid(Uuid::nil())),
                },
                at: Timestamp::new(5),
            })
            .expect("finish child");
        assert_eq!(
            store
                .append_and_transition(JobMutation::Succeed {
                    claim: parent_claim.claim,
                    outcome: JobOutcome::Success {
                        result_ref: OutcomeRef::TransferReport(OperationId::new())
                    },
                    at: Timestamp::new(6)
                })
                .expect("finish parent")
                .state,
            JobState::Succeeded
        );
        assert_eq!(child.job.parent_id, Some(parent.job.id));
    }
}
