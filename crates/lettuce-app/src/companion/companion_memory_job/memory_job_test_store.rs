use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use lettuce_jobs::events::JobEventEnvelope;
use lettuce_jobs::*;
use lettuce_types::{JobId, Page};

use super::{CompanionPostTurnMemoryAdmission, CompanionPostTurnMemoryBatch, MemoryAdmissionStore};

pub(crate) struct MemoryJobs {
    jobs: InMemoryJobStore,
    batches: Mutex<BTreeMap<JobId, CompanionPostTurnMemoryBatch>>,
}

impl MemoryJobs {
    pub(crate) fn new() -> Self {
        Self::with_clock(Arc::new(SystemClock))
    }

    pub(crate) fn with_clock(clock: Arc<dyn Clock>) -> Self {
        Self {
            jobs: InMemoryJobStore::with_clock(clock),
            batches: Mutex::new(BTreeMap::new()),
        }
    }
}

impl MemoryAdmissionStore for MemoryJobs {
    fn admit_memory_batch(
        &self,
        spec: JobSpec,
        batch: CompanionPostTurnMemoryBatch,
    ) -> Result<CompanionPostTurnMemoryAdmission, StoreError> {
        let mut batches = self.batches.lock().map_err(|_| StoreError::Storage)?;
        let admitted = self.jobs.create_or_get(spec)?;
        let batch = batches.entry(admitted.job.id).or_insert(batch).clone();
        Ok(CompanionPostTurnMemoryAdmission {
            batch,
            job: admitted.job,
            created: admitted.created,
        })
    }
}

impl JobStore for MemoryJobs {
    fn create_or_get(&self, spec: NewJob) -> Result<CreateJobResult, StoreError> {
        self.jobs.create_or_get(spec)
    }

    fn get(&self, id: JobId) -> Result<Option<JobSnapshot>, StoreError> {
        self.jobs.get(id)
    }

    fn list(&self, query: JobQuery) -> Result<Page<JobSnapshot>, StoreError> {
        self.jobs.list(query)
    }

    fn events_since(
        &self,
        id: JobId,
        after: Option<EventSeq>,
        limit: u32,
    ) -> Result<Vec<JobEventEnvelope>, StoreError> {
        self.jobs.events_since(id, after, limit)
    }

    fn claim_next(
        &self,
        worker_id: WorkerId,
        now: Timestamp,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Option<Claim>, StoreError> {
        self.jobs.claim_next(worker_id, now, lease_for, allowed)
    }

    fn claim(
        &self,
        id: JobId,
        worker_id: WorkerId,
        now: Timestamp,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Option<Claim>, StoreError> {
        self.jobs.claim(id, worker_id, now, lease_for, allowed)
    }

    fn heartbeat(
        &self,
        claim: &ClaimRef,
        now: Timestamp,
        extend_for: Duration,
    ) -> Result<Claim, StoreError> {
        self.jobs.heartbeat(claim, now, extend_for)
    }

    fn append_and_transition(&self, mutation: JobMutation) -> Result<JobSnapshot, StoreError> {
        self.jobs.append_and_transition(mutation)
    }

    fn expired_claims(&self, now: Timestamp, limit: u32) -> Result<Vec<ExpiredClaim>, StoreError> {
        self.jobs.expired_claims(now, limit)
    }

    fn orphaned_claims(&self, now: Timestamp, limit: u32) -> Result<Vec<ExpiredClaim>, StoreError> {
        self.jobs.orphaned_claims(now, limit)
    }

    fn prune(
        &self,
        policy: lettuce_jobs::retention::RetentionPolicy,
        now: Timestamp,
    ) -> Result<PruneReport, StoreError> {
        JobStore::prune(&self.jobs, policy, now)
    }
}
