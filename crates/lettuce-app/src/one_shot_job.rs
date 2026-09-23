//! A feature request that answers its caller directly (help me reply, scene
//! prompts): admitted once per request id, claimed, run and settled as a job
//! so its usage evidence and cancellation follow every other feature request.

use std::future::Future;
use std::time::Duration;

use lettuce_jobs::{
    CancellationPolicy, CancellationReason, ClaimRef, FiniteFraction, IdempotencyKey, JobError,
    JobKind, JobMutation, JobOutcome, JobPriority, JobSnapshot, JobSpec, JobState, JobStore,
    JobSubject, OutcomeRef, ProgressSnapshot, RecoveryPolicy, ResourceAvailability, ResourceClass,
    StageSnapshot, StoreError, SubjectKind, WorkerId, handle::JobHandle,
};
use lettuce_types::{RequestId, TimestampMillis};

#[derive(Debug)]
pub(crate) enum OneShotJobError {
    Jobs(StoreError),
    AlreadySettled,
    NotClaimed,
}

/// How a feature's error settles its job.
pub(crate) trait OneShotFailure: From<OneShotJobError> {
    fn is_cancelled(&self) -> bool;
    fn job_error(&self) -> JobError;
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct OneShotJob<'a> {
    /// Prefixes the request id in the idempotency key.
    pub(crate) name: &'a str,
    pub(crate) stage: &'static str,
    pub(crate) subject_kind: SubjectKind,
    pub(crate) subject: &'a str,
    pub(crate) request_id: RequestId,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct OneShotLease<'a> {
    pub(crate) worker_id: WorkerId,
    pub(crate) now: TimestampMillis,
    pub(crate) lease_for: Duration,
    pub(crate) allowed: &'a ResourceAvailability,
}

pub(crate) async fn run_one_shot_job<R, T, E, F, Fut>(
    repository: &R,
    job: OneShotJob<'_>,
    lease: OneShotLease<'_>,
    run: F,
) -> Result<(T, JobSnapshot), E>
where
    R: JobStore + ?Sized,
    E: OneShotFailure,
    F: FnOnce(JobHandle) -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    let jobs = |error| E::from(OneShotJobError::Jobs(error));
    let spec = JobSpec::new(
        JobKind::CreationRun,
        JobSubject::new(job.subject_kind, job.subject.to_owned())
            .expect("entity ids are safe job subjects"),
        OutcomeRef::Request(job.request_id),
    )
    .with_idempotency_key(
        IdempotencyKey::new(format!("{}-{}", job.name, job.request_id))
            .expect("request ids are safe idempotency keys"),
    )
    .with_resources(vec![ResourceClass::Network])
    .with_priority(JobPriority::Interactive)
    .with_policies(RecoveryPolicy::Restart, CancellationPolicy::Cooperative);
    let admitted = repository.create_or_get(spec).map_err(jobs)?.job;
    let at = lease.now.max(admitted.updated_at);
    let Some(claim) = repository
        .claim(
            admitted.id,
            lease.worker_id,
            at,
            lease.lease_for,
            lease.allowed,
        )
        .map_err(jobs)?
    else {
        if admitted.is_terminal() {
            return Err(OneShotJobError::AlreadySettled.into());
        }
        if admitted.state == JobState::Queued {
            repository
                .append_and_transition(JobMutation::RequestCancellation {
                    id: admitted.id,
                    reason: CancellationReason::User,
                    at,
                })
                .map_err(jobs)?;
            repository
                .append_and_transition(JobMutation::FinishQueuedCancellation {
                    id: admitted.id,
                    at,
                })
                .map_err(jobs)?;
        }
        return Err(OneShotJobError::NotClaimed.into());
    };
    repository
        .append_and_transition(JobMutation::Start {
            claim: claim.claim.clone(),
            at,
        })
        .map_err(jobs)?;
    repository
        .append_and_transition(JobMutation::StageChanged {
            claim: claim.claim.clone(),
            stage: StageSnapshot::new(job.stage, false).expect("constant job stage is valid"),
            at,
        })
        .map_err(jobs)?;
    let result = run(JobHandle::new(admitted.id)).await;
    settle(repository, claim.claim, job.request_id, result, at)
}

fn settle<R, T, E>(
    repository: &R,
    claim: ClaimRef,
    request_id: RequestId,
    result: Result<T, E>,
    at: TimestampMillis,
) -> Result<(T, JobSnapshot), E>
where
    R: JobStore + ?Sized,
    E: OneShotFailure,
{
    let jobs = |error| E::from(OneShotJobError::Jobs(error));
    match result {
        Ok(value) => {
            repository
                .append_and_transition(JobMutation::Progress {
                    claim: claim.clone(),
                    progress: ProgressSnapshot {
                        fraction: Some(
                            FiniteFraction::new(1.0).expect("constant job progress is valid"),
                        ),
                        ..ProgressSnapshot::default()
                    },
                    at,
                })
                .map_err(jobs)?;
            let job = repository
                .append_and_transition(JobMutation::Succeed {
                    outcome: JobOutcome::Success {
                        result_ref: OutcomeRef::Request(request_id),
                    },
                    claim,
                    at,
                })
                .map_err(jobs)?;
            Ok((value, job))
        }
        Err(error) if error.is_cancelled() => {
            repository
                .append_and_transition(JobMutation::RequestCancellation {
                    id: claim.job_id,
                    reason: CancellationReason::User,
                    at,
                })
                .map_err(jobs)?;
            repository
                .append_and_transition(JobMutation::RequestCleanup {
                    claim: claim.clone(),
                    at,
                })
                .map_err(jobs)?;
            repository
                .append_and_transition(JobMutation::FinishCancellation { claim, at })
                .map_err(jobs)?;
            Err(error)
        }
        Err(error) => {
            repository
                .append_and_transition(JobMutation::Fail {
                    claim,
                    error: error.job_error(),
                    at,
                })
                .map_err(jobs)?;
            Err(error)
        }
    }
}
